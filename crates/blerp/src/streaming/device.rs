use crate::streaming::error::StreamingResult;
use cpal::traits::{DeviceTrait, HostTrait};
use tracing::error;

pub struct DeviceManager {
    devices: Vec<Device>,
    default_output: Option<Device>,
}

#[derive(Clone)]
pub struct Device {
    pub name: Option<String>,
    pub cpal_device: cpal::Device,
}

impl From<cpal::Device> for Device {
    fn from(device: cpal::Device) -> Self {
        Self {
            name: device.name().inspect_err(|err| error!("failed to get device name: {err}")).ok(),
            cpal_device: device,
        }
    }
}

impl DeviceManager {
    /// Creates a new device manager and enumerates available audio devices.
    ///
    /// # Errors
    /// Returns an error if the system cannot enumerate audio devices or if
    /// there's an issue accessing the default audio host.
    pub fn new() -> StreamingResult<Self> {
        let host = cpal::default_host();
        let devices = host.output_devices()?.map(Into::into).collect();
        let default_output = host.default_output_device().map(Into::into);
        Ok(Self { devices, default_output })
    }

    /// Returns a slice of all available audio output devices.
    #[must_use]
    pub fn get_available_devices(&self) -> &[Device] {
        &self.devices
    }

    /// Returns the default audio output device, if available.
    #[must_use]
    pub const fn get_default_device(&self) -> Option<&Device> {
        self.default_output.as_ref()
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
