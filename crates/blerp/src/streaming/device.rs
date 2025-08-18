use crate::streaming::error::StreamingResult;
use cpal::{
    traits::{DeviceTrait, HostTrait},
    Device as CpalDevice,
};

pub struct DeviceManager {
    devices: Vec<Device>,
    default_output: Option<usize>, // Position of the default output device in the devices vector
}

#[derive(Clone)]
pub struct Device {
    pub name: String,
    pub cpal_device: CpalDevice,
}

impl DeviceManager {
    /// Creates a new device manager and enumerates available audio devices.
    ///
    /// # Errors
    /// Returns an error if the system cannot enumerate audio devices or if
    /// there's an issue accessing the default audio host.
    pub fn new() -> StreamingResult<Self> {
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

        Ok(Self { devices, default_output })
    }

    /// Returns a slice of all available audio output devices.
    #[must_use]
    pub fn get_available_devices(&self) -> &[Device] {
        &self.devices
    }

    /// Returns the default audio output device, if available.
    #[must_use]
    pub fn get_default_device(&self) -> Option<&Device> {
        let i = self.default_output?;

        self.devices.get(i)
    }

    /// Refreshes the list of available audio devices.
    ///
    /// # Errors
    /// Returns an error if the system cannot re-enumerate audio devices.
    pub fn refresh_devices(&mut self) -> StreamingResult<()> {
        *self = Self::new()?;

        Ok(())
    }
}
