use crc32fast::Hasher;
use std::time::{SystemTime, UNIX_EPOCH};

const PROTOCOL_VERSION: u16 = 1001;
const MSG_CONTROLLER_INFO: u32 = 0x100001;
const MSG_CONTROLLER_DATA: u32 = 0x100002;

// Estructura "común" al inicio de los payloads de info/data (11 bytes)
#[derive(Clone, Copy)]
pub struct ControllerHeader {
    pub slot: u8,            // 0..3
    pub state: u8,           // 0=not connected, 1=reserved?, 2=connected
    pub model: u8,           // 0=na, 1=partial gyro, 2=full gyro
    pub conn_type: u8,       // 0=na, 1=USB, 2=BT
    pub mac: [u8; 6],        // mac address or zeros
    pub battery: u8,         // 0..5, EE charging, EF charged
}

impl Default for ControllerHeader {
    fn default() -> Self {
        Self {
            slot: 0,
            state: 2,
            model: 2,
            conn_type: 1,
            mac: [0; 6],
            battery: 0x05,
        }
    }
}

fn le_u16(buf: &mut Vec<u8>, v: u16) { buf.extend_from_slice(&v.to_le_bytes()); }
fn le_u32(buf: &mut Vec<u8>, v: u32) { buf.extend_from_slice(&v.to_le_bytes()); }
fn le_u64(buf: &mut Vec<u8>, v: u64) { buf.extend_from_slice(&v.to_le_bytes()); }
fn le_f32(buf: &mut Vec<u8>, v: f32) { buf.extend_from_slice(&v.to_le_bytes()); }

fn header_and_type(server_id: u32, msg_type: u32, payload_len: u16) -> Vec<u8> {
    // Header: 16 bytes + 4 bytes "message type" = 20
    // We will fill CRC later.
    let mut out = Vec::with_capacity(20 + payload_len as usize);
    // "DSUS"
    out.extend_from_slice(b"DSUS");
    // version
    le_u16(&mut out, PROTOCOL_VERSION);
    // length WITHOUT header (16 bytes), but INCLUDING message type and payload
    // i.e. total_len - 16
    le_u16(&mut out, (4 + payload_len) as u16);
    // crc32 placeholder
    le_u32(&mut out, 0);
    // server id
    le_u32(&mut out, server_id);
    // message type
    le_u32(&mut out, msg_type);
    out
}

fn fill_crc32(packet: &mut [u8]) {
    // CRC32 over full packet with crc field zeroed.
    // crc field offset is 8..12
    for i in 8..12 { packet[i] = 0; }
    let mut hasher = Hasher::new();
    hasher.update(packet);
    let crc = hasher.finalize();
    packet[8..12].copy_from_slice(&crc.to_le_bytes());
}

pub fn build_controller_info(server_id: u32, hdr: ControllerHeader) -> Vec<u8> {
    // payload: 11 bytes "shared beginning" + 1 zero byte = 12
    let payload_len = 12u16;
    let mut pkt = header_and_type(server_id, MSG_CONTROLLER_INFO, payload_len);

    // shared beginning (11 bytes)
    pkt.push(hdr.slot);
    pkt.push(hdr.state);
    pkt.push(hdr.model);
    pkt.push(hdr.conn_type);
    pkt.extend_from_slice(&hdr.mac);
    pkt.push(hdr.battery);

    // trailing zero
    pkt.push(0);

    fill_crc32(&mut pkt);
    pkt
}

pub struct MotionData {
    pub connected: u8,   // 1 connected, 0 not
    pub packet_number: u32,
    pub accel: [f32; 3], // g
    pub gyro: [f32; 3],  // deg/s (pitch, yaw, roll per spec)
    pub timestamp_us: u64,
}

pub fn build_controller_data(server_id: u32, hdr: ControllerHeader, data: MotionData) -> Vec<u8> {
    // payload: shared 11 + connected(1) + pktnum(4) + btn bits(2) + home(1) + touch(1)
    // + sticks (4) + analog dpad(4) + analog XYAB(4) + analog R1/L1/R2/L2 (4)
    // + two touches(12) + timestamp(8) + accel(12) + gyro(12) = 11+1+4+2+1+1+4+4+4+4+12+8+12+12 = 100 - 20 header = 80 payload
    let payload_len = 80u16;
    let mut pkt = header_and_type(server_id, MSG_CONTROLLER_DATA, payload_len);

    // shared beginning (11)
    pkt.push(hdr.slot);
    pkt.push(hdr.state);
    pkt.push(hdr.model);
    pkt.push(hdr.conn_type);
    pkt.extend_from_slice(&hdr.mac);
    pkt.push(hdr.battery);

    // connected (1)
    pkt.push(data.connected);

    // packet number (4)
    le_u32(&mut pkt, data.packet_number);

    // buttons bitmasks (2) + home(1) + touch-button(1) — no botones: 0
    pkt.extend_from_slice(&[0, 0]); // bitmasks
    pkt.push(0); // HOME
    pkt.push(0); // TOUCH Button

    // sticks neutros (4 bytes)
    pkt.extend_from_slice(&[128, 128, 128, 128]);

    // analog dpad (4) neutro
    pkt.extend_from_slice(&[0, 0, 0, 0]);

    // analog Y,B,A,X (4) neutro
    pkt.extend_from_slice(&[0, 0, 0, 0]);

    // analog R1,L1,R2,L2 (4) neutro
    pkt.extend_from_slice(&[0, 0, 0, 0]);

    // two touches (6 bytes cada uno) — desactivados
    // touch: active(1)=0, id(1)=0, x(2)=0, y(2)=0
    pkt.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    pkt.extend_from_slice(&[0, 0, 0, 0, 0, 0]);

    // timestamp (8)
    le_u64(&mut pkt, data.timestamp_us);

    // accel (3 * f32)
    le_f32(&mut pkt, data.accel[0]);
    le_f32(&mut pkt, data.accel[1]);
    le_f32(&mut pkt, data.accel[2]);

    // gyro (pitch, yaw, roll) (3 * f32)
    le_f32(&mut pkt, data.gyro[0]);
    le_f32(&mut pkt, data.gyro[1]);
    le_f32(&mut pkt, data.gyro[2]);

    fill_crc32(&mut pkt);
    pkt
}

pub fn now_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}
