use crate::playback::error::PlaybackResult;
use cpal::{
    traits::{DeviceTrait, HostTrait},
    Device as CpalDevice, Host,
};

pub struct DeviceManager {
    host: Host,
    devices: Vec<Device>,
    default_output: Option<usize>, // Position of the default output device in the devices vector
}

pub struct Device {
    pub name: String,
    pub cpal_device: CpalDevice,
}

impl DeviceManager {
    pub fn new() -> PlaybackResult<Self> {
        let host = cpal::default_host();
        let devices: Vec<Device> = host
            .output_devices()?
            .filter_map(|d| {
                let name = d.name().ok()?;
                Some(Device { name, cpal_device: d })
            })
            .collect();
        let default_output = host
            .default_output_device()
            .and_then(|d| d.name().ok())
            .and_then(|name| devices.iter().position(|device| device.name == name));

        Ok(Self { host, devices, default_output })
    }

    pub fn get_available_devices(&self) -> &[Device] {
        &self.devices
    }

    pub fn get_default_device(&self) -> Option<&Device> {
        let i = self.default_output?;

        self.devices.get(i)
    }

    pub fn refresh_devices(&mut self) -> PlaybackResult<()> {
        *self = Self::new()?;

        Ok(())
    }
}
