pub struct Device {
    pub name: String,
}
pub struct DeviceEntry {
    pub id: String,
    pub device: Device,
}
pub struct DeviceHandler {
    pub devices: Vec<DeviceEntry>,
}
impl DeviceHandler {
    pub fn add_device(&mut self, id: String, device: Device) {
        self.devices.push(DeviceEntry { id, device });
    }
    pub fn get_devices(self) -> Vec<DeviceEntry> {
        self.devices
    }
}
