use crate::input::{
    capability::{Capability, Gamepad},
    event::{native::NativeEvent, value::InputValue},
    target::{TargetInputDevice, TargetOutputDevice},
};

use crate::input::{
    output_event::OutputEvent,
    output_capability::OutputCapability,
    composite_device::client::CompositeDeviceClient,
};

use std::collections::HashSet;

pub struct DsuTarget {
    // Aquí puedes agregar config más adelante (host, puerto, etc.)
}

impl DsuTarget {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(DsuTarget {})
    }
}

// Implementa recepción de eventos (ej. giroscopio)
impl TargetInputDevice for DsuTarget {
    fn write_event(&mut self, event: NativeEvent) -> Result<(), crate::input::target::InputError> {
        let cap = event.capability();

        match cap {
            Capability::Gamepad(Gamepad::Gyro) | Capability::Gamepad(Gamepad::Accelerometer) => {
                if let InputValue::Vector3 { x, y, z } = event.value() {
                    println!(
                        "DSU Event - {:?}: x={:?}, y={:?}, z={:?}",
                        cap, x, y, z
                    );
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, crate::input::target::InputError> {
        Ok(vec![
            Capability::Gamepad(Gamepad::Gyro),
            Capability::Gamepad(Gamepad::Accelerometer),
        ])
    }

    fn on_capabilities_changed(
        &mut self,
        _capabilities: HashSet<Capability>,
    ) -> Result<(), crate::input::target::InputError> {
        Ok(())
    }

    fn on_composite_device_attached(
        &mut self,
        _device: CompositeDeviceClient,
    ) -> Result<(), crate::input::target::InputError> {
        Ok(())
    }
}

// Implementa salida (no usaremos nada aún)
impl TargetOutputDevice for DsuTarget {
    fn poll(
        &mut self,
        _composite_device: &Option<CompositeDeviceClient>,
    ) -> Result<Vec<OutputEvent>, crate::input::target::OutputError> {
        Ok(vec![])
    }

    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, crate::input::target::OutputError> {
        Ok(vec![])
    }

    fn on_output_capabilities_changed(
        &mut self,
        _capabilities: HashSet<OutputCapability>,
    ) -> Result<(), crate::input::target::OutputError> {
        Ok(())
    }
}
