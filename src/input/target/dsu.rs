use crate::input::{
    capability::{Capability, Gamepad},
    event::{native::NativeEvent, value::InputValue},
    target::{TargetDevice, TargetDeviceType},
};
use std::sync::{Arc, Mutex};

pub struct DsuTarget {
    // Aquí puedes agregar config más adelante (host, puerto, etc.)
}

impl DsuTarget {
    pub fn new() -> Self {
        DsuTarget {}
    }
}

impl TargetDevice for DsuTarget {
    fn get_target_device_type(&self) -> TargetDeviceType {
        TargetDeviceType {
            id: "dsu".to_string(),
            name: "DSU Server".to_string(),
            device_class: crate::input::device_class::DeviceClass::Gamepad,
        }
    }

    fn handle_native_event(&mut self, event: &NativeEvent) {
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
            _ => {} // Ignora eventos que no sean de IMU
        }
    }
}
