mod protocol;

use crate::input::{
    capability::{Capability, Gamepad},
    event::{native::NativeEvent, value::InputValue},
    target::{TargetInputDevice, TargetOutputDevice, InputError, OutputError},
    output_capability::OutputCapability,
    output_event::OutputEvent,
    composite_device::client::CompositeDeviceClient,
};

use std::{
    collections::{HashMap, HashSet},
    net::{SocketAddr, UdpSocket},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use super::dsu::protocol::{
    build_controller_data, build_controller_info, now_micros, ControllerHeader, MotionData,
};

#[derive(Debug)]
pub struct DsuTarget {
    server_id: u32,
    bind_addr: String,   // e.g. "0.0.0.0:26760"
    clients: Arc<Mutex<HashMap<SocketAddr, Instant>>>, // suscriptores y su ultimo contacto
    socket: Arc<UdpSocket>,

    // último estado IMU
    last_accel: [f32; 3],
    last_gyro:  [f32; 3],
    pkt_counter: u32,
}

impl DsuTarget {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // TODO: hacer esto configurable por YAML
        let bind_addr = "0.0.0.0:26760".to_string();

        let socket = UdpSocket::bind(&bind_addr)?;
        socket.set_nonblocking(true)?;
        let socket = Arc::new(socket);

        let clients: Arc<Mutex<HashMap<SocketAddr, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

        let this = Self {
            server_id: 0x1337_0001,
            bind_addr,
            clients: clients.clone(),
            socket: socket.clone(),
            last_accel: [0.0; 3],
            last_gyro:  [0.0; 3],
            pkt_counter: 0,
        };

        // Hilo receptor: procesa peticiones del cliente
        thread::spawn(move || {
            let mut buf = [0u8; 2048];
            let timeout = Duration::from_secs(5);
            loop {
                match socket.recv_from(&mut buf) {
                    Ok((n, addr)) => {
                        // Mínimo header (20 bytes)
                        if n < 20 { continue; }

                        // LE: bytes 16..20 = tipo de mensaje
                        let msg_type = u32::from_le_bytes(buf[16..20].try_into().unwrap());

                        // Registrar/renovar cliente en cualquier mensaje válido
                        {
                            let mut m = clients.lock().unwrap();
                            m.insert(addr, Instant::now());
                            // limpiar expirados
                            m.retain(|_, t| t.elapsed() <= timeout);
                        }

                        match msg_type {
                            0x100001 => { // Controller Info
                                log::info!("[DSU] ControllerInfo request from {addr}");
                                let hdr = ControllerHeader { slot: 0, ..Default::default() };
                                let pkt = build_controller_info(0x1337_0001, hdr);
                                let _ = socket.send_to(&pkt, addr);
                            }
                            0x100002 => { // Controller Data subscribe
                                log::info!("[DSU] ControllerData subscribe from {addr}");
                                // Nada más que registrar. El envío lo hace write_event() o un loop de envío.
                            }
                            _ => {
                                log::debug!("[DSU] Unknown msg_type=0x{msg_type:08x} from {addr}, len={n}");
                                // Ignorar otros por ahora
                            }
                        }
                    }
                    Err(_e) => {
                        // No bloqueante: dormir un poco para no quemar CPU
                        thread::sleep(Duration::from_millis(2));
                    }
                }
            }
        });

        Ok(this)
    }

    fn send_motion_to_clients(&mut self) {
        let now = now_micros();
        // Copiar clientes actuales (evitar tener lock durante send_to)
        let clients: Vec<SocketAddr> = {
            let mut m = self.clients.lock().unwrap();
            // limpiar timeouts (por si write_event se llama pero no llegan mensajes)
            m.retain(|_, t| t.elapsed() <= Duration::from_secs(5));
            m.keys().copied().collect()
        };

        if clients.is_empty() { return; }

        self.pkt_counter = self.pkt_counter.wrapping_add(1);

        // Por ahora: asignamos pitch=yaw=roll = gyro[x,y,z] directamente.
        // Si el mapeo no coincide con lo que espera el juego, ajustamos más tarde.
        let data = MotionData {
            connected: 1,
            packet_number: self.pkt_counter,
            accel: self.last_accel,
            gyro: self.last_gyro,  // (pitch,yaw,roll)
            timestamp_us: now,
        };

        let hdr = ControllerHeader { slot: 0, ..Default::default() };
        let pkt = build_controller_data(self.server_id, hdr, data);

        if !clients.is_empty() && (self.pkt_counter % 120 == 0) {
            // log cada ~120 paquetes para no inundar
            log::info!("[DSU] Sending motion to {} client(s). pn={}", clients.len(), self.pkt_counter);
        }

        for addr in clients {
            let _ = self.socket.send_to(&pkt, addr);
        }
    }
}

impl TargetInputDevice for DsuTarget {
    fn write_event(&mut self, event: NativeEvent) -> Result<(), InputError> {
        log::info!("[DSU] Event received: {:?}", event.as_capability());
        match (event.as_capability(), event.get_value()) {
            (Capability::Gamepad(Gamepad::Accelerometer), InputValue::Vector3 { x, y, z }) => {
                // El spec quiere "g". Nuestros eventos ya vienen en "g" (BmiImu usa g; AccelGyro3D multiplica por 10).
                // Si notas escalas raras en juegos, ajustamos aquí tras probar.
                self.last_accel = [x.unwrap_or(0.0) as f32, y.unwrap_or(0.0) as f32, z.unwrap_or(0.0) as f32];
                self.send_motion_to_clients();
            }
            (Capability::Gamepad(Gamepad::Gyro), InputValue::Vector3 { x, y, z }) => {
                // El spec quiere deg/s; BmiImu ya convierte a deg/s * 12.
                // Si la sensibilidad es exagerada, bajaremos esa escala más tarde.
                self.last_gyro = [x.unwrap_or(0.0) as f32, y.unwrap_or(0.0) as f32, z.unwrap_or(0.0) as f32];
                // Enviar al llegar gyro también, para latencia baja
                self.send_motion_to_clients();
            }
            _ => { /* ignoramos otros eventos */ }
        }
        Ok(())
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(vec![
            Capability::Gamepad(Gamepad::Gyro),
            Capability::Gamepad(Gamepad::Accelerometer),
        ])
    }

    fn on_composite_device_attached(
        &mut self,
        _device: CompositeDeviceClient,
    ) -> Result<(), InputError> {
        Ok(())
    }
}

impl TargetOutputDevice for DsuTarget {
    fn poll(
        &mut self,
        _composite_device: &Option<CompositeDeviceClient>,
    ) -> Result<Vec<OutputEvent>, OutputError> {
        Ok(vec![])
    }

    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(vec![])
    }

    fn on_output_capabilities_changed(
        &mut self,
        _capabilities: HashSet<OutputCapability>,
    ) -> Result<(), OutputError> {
        Ok(())
    }
}
