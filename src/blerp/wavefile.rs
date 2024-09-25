#![allow(clippy::unused_io_amount)]
#![allow(dead_code)]
use crate::error::VoltError;
use bytes::BufMut;
use std::{fs::File, io, io::Write, path::Path};

#[derive(Debug, Copy, Clone)]
pub enum WaveAudioFormat {
    PulseCodeModulation,
    FloatingPoint,
}

impl From<WaveAudioFormat> for u16 {
    fn from(value: WaveAudioFormat) -> Self {
        match value {
            WaveAudioFormat::PulseCodeModulation => 1,
            WaveAudioFormat::FloatingPoint => 3,
        }
    }
}

pub fn form_wav_file_header(
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    sample_length: u32,
    audio_format: WaveAudioFormat,
) -> io::Result<Vec<u8>> {
    let file_length: u32 = sample_length * (u32::from(bits_per_sample) / 8) + 44;
    let format_data_length: u32 = 16;
    let byte_rate: u32 = sample_rate * u32::from(channels) * u32::from(bits_per_sample) / 8;
    let block_align: u16 = channels * bits_per_sample / 8;
    let data_length: u32 = sample_length * (u32::from(bits_per_sample) / 8) * u32::from(channels);
    let audio_format_u16: u16 = audio_format.into();

    let mut filebuf = Vec::with_capacity((file_length + 4) as usize);
    filebuf.put_slice(b"RIFF");
    filebuf.put_u32_le(file_length);
    filebuf.put_slice(b"WAVEfmt ");
    filebuf.put_u32_le(format_data_length);
    filebuf.put_u16_le(audio_format_u16);
    filebuf.put_u16_le(channels);
    filebuf.put_u32_le(sample_rate);
    filebuf.put_u32_le(byte_rate);
    filebuf.put_u16_le(block_align);
    filebuf.put_u16_le(bits_per_sample);
    filebuf.put_slice(b"data");
    filebuf.put_u32_le(data_length);
    Ok(filebuf)
}

pub fn form_wav_file_data_f32(buffer: &[f32], header_buffer: &mut Vec<u8>) {
    buffer
        .iter()
        .for_each(|value| header_buffer.put_f32_le(*value));
}

pub fn form_wav_file_data_f64(buffer: &[f64], header_buffer: &mut Vec<u8>) {
    buffer
        .iter()
        .for_each(|value| header_buffer.put_f64_le(*value));
}

pub fn form_wav_file_data_f32tof64(buffer: &[f32], header_buffer: &mut Vec<u8>) {
    buffer
        .iter()
        .for_each(|value| header_buffer.put_f64_le(*value as _));
}

pub fn form_wav_file_data_f64tof32(buffer: &[f64], header_buffer: &mut Vec<u8>) {
    buffer
        .iter()
        .for_each(|value| header_buffer.put_f32_le(*value as _));
}

pub fn form_wav_file_data_f32toi16(buffer: &[f32], header_buffer: &mut Vec<u8>) {
    buffer
        .iter()
        .for_each(|value| header_buffer.put_i16_le((value * 32767.0) as _));
}

pub fn form_wav_file_data_f64toi16(buffer: &[f64], header_buffer: &mut Vec<u8>) {
    buffer
        .iter()
        .for_each(|value| header_buffer.put_i16_le((value * 32767.0) as _));
}

pub fn form_wav_file_data_f32toi8(buffer: &[f32], header_buffer: &mut Vec<u8>) {
    buffer
        .iter()
        .for_each(|value| header_buffer.put_i8((value * 127.0) as _));
}

pub fn form_wav_file_data_f64toi8(buffer: &[f64], header_buffer: &mut Vec<u8>) {
    buffer
        .iter()
        .for_each(|value| header_buffer.put_i8((value * 127.0) as _));
}

pub fn form_wav_file_data_f32toi32(buffer: &[f32], header_buffer: &mut Vec<u8>) {
    buffer
        .iter()
        .for_each(|value| header_buffer.put_i32_le((value * 2_147_483_647.0) as _));
}

pub fn form_wav_file_data_f64toi32(buffer: &[f64], header_buffer: &mut Vec<u8>) {
    buffer
        .iter()
        .for_each(|value| header_buffer.put_i32_le((value * 2_147_483_647.0) as _));
}

pub fn form_wav_file_data_f32toi64(buffer: &[f32], header_buffer: &mut Vec<u8>) {
    buffer
        .iter()
        .for_each(|value| header_buffer.put_i64_le((value * 9_223_372_036_854_776_000.0) as _));
}

pub fn form_wav_file_data_f64toi64(buffer: &[f64], header_buffer: &mut Vec<u8>) {
    buffer
        .iter()
        .for_each(|value| header_buffer.put_i64_le((value * 9_223_372_036_854_776_000.0) as _));
}

pub fn write_wav_file_f64(
    location: &Path,
    buffer: &[f64],
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    sample_length: u32,
    audio_format: WaveAudioFormat,
) -> crate::Result<()> {
    let mut file = File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(location)?;

    let mut filebuf = form_wav_file_header(
        sample_rate,
        channels,
        bits_per_sample,
        sample_length,
        audio_format,
    )?;

    match audio_format {
        WaveAudioFormat::PulseCodeModulation => match bits_per_sample {
            8 => form_wav_file_data_f64toi8(buffer, &mut filebuf),
            16 => form_wav_file_data_f64toi16(buffer, &mut filebuf),
            32 => form_wav_file_data_f64toi32(buffer, &mut filebuf),
            64 => form_wav_file_data_f64toi64(buffer, &mut filebuf),
            _ => return Err(VoltError::InvalidSampleFormat),
        },
        WaveAudioFormat::FloatingPoint => match bits_per_sample {
            32 => form_wav_file_data_f64tof32(buffer, &mut filebuf),
            64 => form_wav_file_data_f64(buffer, &mut filebuf),
            _ => return Err(VoltError::InvalidSampleFormat),
        },
    }

    file.write(&filebuf)?;
    Ok(())
}

pub fn write_wav_file_f32(
    location: &Path,
    buffer: &[f32],
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    sample_length: u32,
    audio_format: WaveAudioFormat,
) -> crate::Result<()> {
    let mut file = File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(location)?;

    let mut filebuf = form_wav_file_header(
        sample_rate,
        channels,
        bits_per_sample,
        sample_length,
        audio_format,
    )?;

    match audio_format {
        WaveAudioFormat::PulseCodeModulation => match bits_per_sample {
            8 => form_wav_file_data_f32toi8(buffer, &mut filebuf),
            16 => form_wav_file_data_f32toi16(buffer, &mut filebuf),
            32 => form_wav_file_data_f32toi32(buffer, &mut filebuf),
            64 => form_wav_file_data_f32toi64(buffer, &mut filebuf),
            _ => return Err(VoltError::InvalidSampleFormat),
        },
        WaveAudioFormat::FloatingPoint => match bits_per_sample {
            32 => form_wav_file_data_f32(buffer, &mut filebuf),
            64 => form_wav_file_data_f32tof64(buffer, &mut filebuf),
            _ => return Err(VoltError::InvalidSampleFormat),
        },
    };

    file.write(&filebuf)?;
    Ok(())
}
