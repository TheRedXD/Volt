use crate::error::VoltError;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use rodio::{Decoder, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

pub struct DeviceEntry {
    pub id: String,
    pub device: cpal::Device,
}

impl DeviceEntry {
    pub fn new(device: cpal::Device) -> Self {
        let name = device.name().unwrap_or_default();
        DeviceEntry { id: name, device }
    }

    pub fn name(&self) -> &str {
        &self.id
    }

    pub fn config(&self) -> crate::Result<cpal::SupportedStreamConfig> {
        let mut supported_configs_range = self.device.supported_output_configs()?;

        supported_configs_range
            .next()
            .map(|c| c.with_max_sample_rate())
            .ok_or(VoltError::Config)
    }

    pub fn beep(&self, duration: u64) -> crate::Result<()> {
        let config = self.config()?;

        match config.sample_format() {
            SampleFormat::I8 => play_beep::<i8>(&self.device, &config.into(), duration),
            SampleFormat::I16 => play_beep::<i16>(&self.device, &config.into(), duration),
            SampleFormat::I32 => play_beep::<i32>(&self.device, &config.into(), duration),
            SampleFormat::I64 => play_beep::<i64>(&self.device, &config.into(), duration),
            SampleFormat::U8 => play_beep::<u8>(&self.device, &config.into(), duration),
            SampleFormat::U16 => play_beep::<u16>(&self.device, &config.into(), duration),
            SampleFormat::U32 => play_beep::<u32>(&self.device, &config.into(), duration),
            SampleFormat::U64 => play_beep::<u64>(&self.device, &config.into(), duration),
            SampleFormat::F32 => play_beep::<f32>(&self.device, &config.into(), duration),
            SampleFormat::F64 => play_beep::<f64>(&self.device, &config.into(), duration),
            sample_format => panic!("Unsupported sample format '{sample_format}'"),
        }
    }

    pub fn play_file<P: AsRef<Path>>(&self, path: P) -> crate::Result<()> {
        let file = BufReader::new(File::open(path)?);
        let decoder = Decoder::new(file)?;
        let config = self.config()?;

        match config.sample_format() {
            SampleFormat::I8 => play_file::<i8>(&self.device, &config.into(), decoder),
            SampleFormat::I16 => play_file::<i16>(&self.device, &config.into(), decoder),
            SampleFormat::I32 => play_file::<i32>(&self.device, &config.into(), decoder),
            SampleFormat::I64 => play_file::<i64>(&self.device, &config.into(), decoder),
            SampleFormat::U8 => play_file::<u8>(&self.device, &config.into(), decoder),
            SampleFormat::U16 => play_file::<u16>(&self.device, &config.into(), decoder),
            SampleFormat::U32 => play_file::<u32>(&self.device, &config.into(), decoder),
            SampleFormat::U64 => play_file::<u64>(&self.device, &config.into(), decoder),
            SampleFormat::F32 => play_file::<f32>(&self.device, &config.into(), decoder),
            SampleFormat::F64 => play_file::<f64>(&self.device, &config.into(), decoder),
            sample_format => panic!("Unsupported sample format '{sample_format}'"),
        }
    }
}

pub fn play_beep<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    duration_secs: u64,
) -> crate::Result<()>
where
    T: SizedSample + FromSample<f32>,
{
    let sample_rate = config.sample_rate.0 as f32;
    let channels = config.channels as usize;
    let mut sample_clock = 0f32;
    let mut next_value = move || {
        sample_clock = (sample_clock + 1.0) % sample_rate;
        (sample_clock * 440.0 * 2.0 * std::f32::consts::PI / sample_rate).sin()
    };

    let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            write_data(data, channels, &mut next_value)
        },
        err_fn,
        None,
    )?;
    stream.play()?;

    std::thread::sleep(Duration::from_secs(duration_secs));

    Ok(())
}

pub fn play_file<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    decoder: Decoder<BufReader<File>>,
) -> crate::Result<()>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    // I'm sure there is a better way to handle the fallback value if the decoder can't
    // determine the song duration.
    let song_duration = decoder.total_duration().unwrap_or(Duration::from_secs(300));
    let mut samples = decoder.convert_samples();
    let mut next_value = move || samples.next().unwrap_or_default();
    let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            write_data(data, channels, &mut next_value)
        },
        err_fn,
        None,
    )?;

    stream.play()?;
    std::thread::sleep(song_duration);

    Ok(())
}

fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut dyn FnMut() -> f32)
where
    T: Sample + FromSample<f32>,
{
    for frame in output.chunks_mut(channels) {
        let value: T = T::from_sample(next_sample());
        for sample in frame.iter_mut() {
            *sample = value;
        }
    }
}
#[derive(Default)]
pub struct DeviceHandler {
    pub devices: Vec<DeviceEntry>,
}

impl DeviceHandler {
    pub fn add_device(&mut self, device: DeviceEntry) {
        self.devices.push(device);
    }

    pub fn devices(&self) -> &[DeviceEntry] {
        &self.devices
    }

    pub fn devices_mut(&mut self) -> &mut [DeviceEntry] {
        &mut self.devices
    }

    pub fn take_devices(&mut self) -> Vec<DeviceEntry> {
        std::mem::take(&mut self.devices)
    }
}

pub fn load_system_devices(devices: &mut Vec<DeviceEntry>) {
    devices.clear();
    let host = cpal::platform::default_host();

    if let Ok(host_devices) = host.output_devices() {
        host_devices
            .map(DeviceEntry::new)
            .for_each(|d| devices.push(d))
    }
}
