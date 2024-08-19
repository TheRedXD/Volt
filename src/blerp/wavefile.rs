use std::{
    fs::File,
    path::Path,
    io::Write,
    io
};

pub enum WaveAudioFormat {
    PulseCodeModulation,
    FloatingPoint
}

pub fn form_wav_file_header(
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    sample_length: u32,
    audio_format: WaveAudioFormat
) -> io::Result<Vec<u8>> {
    let file_length: u32 = sample_length * (u32::from(bits_per_sample) / 8) + 44;
    let format_data_length: u32 = 16;
    let byte_rate: u32 = sample_rate * u32::from(channels) * u32::from(bits_per_sample) / 8;
    let block_align: u16 = channels * bits_per_sample / 8;
    let data_length: u32 = sample_length * (u32::from(bits_per_sample) / 8) * u32::from(channels);

    let audio_format_u16: u16 = match audio_format {
        WaveAudioFormat::PulseCodeModulation => 1,
        WaveAudioFormat::FloatingPoint => 3
    };

    let mut filebuf: Vec<u8> = Vec::with_capacity((file_length + 4) as usize);
    filebuf.write(b"RIFF")?;
    filebuf.write(&file_length.to_le_bytes())?;
    filebuf.write(b"WAVEfmt ")?;
    filebuf.write(&format_data_length.to_le_bytes())?;
    filebuf.write(&audio_format_u16.to_le_bytes())?;
    filebuf.write(&channels.to_le_bytes())?;
    filebuf.write(&sample_rate.to_le_bytes())?;
    filebuf.write(&byte_rate.to_le_bytes())?;
    filebuf.write(&block_align.to_le_bytes())?;
    filebuf.write(&bits_per_sample.to_le_bytes())?;
    filebuf.write(b"data")?;
    filebuf.write(&data_length.to_le_bytes())?;

    Ok(filebuf)
}

pub fn form_wav_file_data_f32(
    buffer: &[f32],
    header_buffer: Vec<u8>
) -> io::Result<Vec<u8>> {
    let mut filebuf = header_buffer;

    filebuf.extend(buffer.iter().flat_map(|value| value.to_le_bytes()));

    Ok(filebuf)
}

pub fn form_wav_file_data_f64(
    buffer: &[f64],
    header_buffer: Vec<u8>
) -> io::Result<Vec<u8>> {
    let mut filebuf = header_buffer;

    filebuf.extend(buffer.iter().flat_map(|value| value.to_le_bytes()));

    Ok(filebuf)
}

pub fn form_wav_file_data_f32tof64(
    buffer: &[f32],
    header_buffer: Vec<u8>
) -> io::Result<Vec<u8>> {
    let mut filebuf = header_buffer;

    filebuf.extend(buffer.iter().flat_map(|value| f64::from(*value).to_le_bytes()));

    Ok(filebuf)
}

pub fn form_wav_file_data_f64tof32(
    buffer: &[f64],
    header_buffer: Vec<u8>
) -> io::Result<Vec<u8>> {
    let mut filebuf = header_buffer;

    filebuf.extend(buffer.iter().flat_map(|value| (*value as f32).to_le_bytes()));

    Ok(filebuf)
}

pub fn form_wav_file_data_f32toi16(
    buffer: &[f32],
    header_buffer: Vec<u8>
) -> io::Result<Vec<u8>> {
    let mut filebuf = header_buffer;

    filebuf.extend(buffer.iter().flat_map(|value| ((value * 32767.0) as i16).to_le_bytes()));

    Ok(filebuf)
}

pub fn form_wav_file_data_f64toi16(
    buffer: &[f64],
    header_buffer: Vec<u8>
) -> io::Result<Vec<u8>> {
    let mut filebuf = header_buffer;

    filebuf.extend(buffer.iter().flat_map(|value| ((value * 32767.0) as i16).to_le_bytes()));

    Ok(filebuf)
}

pub fn form_wav_file_data_f32toi8(
    buffer: &[f32],
    header_buffer: Vec<u8>
) -> io::Result<Vec<u8>> {
    let mut filebuf = header_buffer;

    filebuf.extend(buffer.iter().flat_map(|value| ((value * 127.0) as i8).to_le_bytes()));

    Ok(filebuf)
}

pub fn form_wav_file_data_f64toi8(
    buffer: &[f64],
    header_buffer: Vec<u8>
) -> io::Result<Vec<u8>> {
    let mut filebuf = header_buffer;

    filebuf.extend(buffer.iter().flat_map(|value| ((value * 127.0) as i8).to_le_bytes()));

    Ok(filebuf)
}

pub fn form_wav_file_data_f32toi32(
    buffer: &[f32],
    header_buffer: Vec<u8>
) -> io::Result<Vec<u8>> {
    let mut filebuf = header_buffer;

    filebuf.extend(buffer.iter().flat_map(|value| ((value * 2_147_483_647.0) as i32).to_le_bytes()));

    Ok(filebuf)
}

pub fn form_wav_file_data_f64toi32(
    buffer: &[f64],
    header_buffer: Vec<u8>
) -> io::Result<Vec<u8>> {
    let mut filebuf = header_buffer;

    filebuf.extend(buffer.iter().flat_map(|value| ((value * 2_147_483_647.0) as i32).to_le_bytes()));

    Ok(filebuf)
}

pub fn form_wav_file_data_f32toi64(
    buffer: &[f32],
    header_buffer: Vec<u8>
) -> io::Result<Vec<u8>> {
    let mut filebuf = header_buffer;

    filebuf.extend(buffer.iter().flat_map(|value| ((value * 9_223_372_036_854_776_000.0) as i64).to_le_bytes()));

    Ok(filebuf)
}

pub fn form_wav_file_data_f64toi64(
    buffer: &[f64],
    header_buffer: Vec<u8>
) -> io::Result<Vec<u8>> {
    let mut filebuf = header_buffer;

    filebuf.extend(buffer.iter().flat_map(|value| ((value * 9_223_372_036_854_776_000.0) as i64).to_le_bytes()));

    Ok(filebuf)
}

pub fn write_wav_file_f64(
    location: &Path,
    buffer: &[f64],
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    sample_length: u32,
    audio_format: WaveAudioFormat
) -> io::Result<()> {
    let mut file = File::options().write(true).create(true).truncate(true).open(location)?;
    match audio_format {
        WaveAudioFormat::PulseCodeModulation => {
            let filebuf: Vec<u8> = form_wav_file_header(
                sample_rate,
                channels,
                bits_per_sample,
                sample_length,
                audio_format
            )?;
            let _bmatch: std::result::Result<(), &str> = match bits_per_sample {
                8 => {
                    let filebuf = form_wav_file_data_f64toi8(buffer, filebuf)?;
                    file.write(&filebuf)?;
                    std::result::Result::Ok(())
                },
                16 => {
                    let filebuf = form_wav_file_data_f64toi16(buffer, filebuf)?;
                    file.write(&filebuf)?;
                    std::result::Result::Ok(())
                },
                32 => {
                    let filebuf = form_wav_file_data_f64toi32(buffer, filebuf)?;
                    file.write(&filebuf)?;
                    std::result::Result::Ok(())
                },
                64 => {
                    let filebuf = form_wav_file_data_f64toi64(buffer, filebuf)?;
                    file.write(&filebuf)?;
                    std::result::Result::Ok(())
                },
                _ => std::result::Result::Err("Invalid bits per sample!")
            };
        },
        WaveAudioFormat::FloatingPoint => {
            let filebuf: Vec<u8> = form_wav_file_header(
                sample_rate,
                channels,
                bits_per_sample,
                sample_length,
                audio_format
            )?;
            let _bmatch: std::result::Result<(), &str> = match bits_per_sample {
                32 => {
                    let filebuf = form_wav_file_data_f64tof32(buffer, filebuf)?;
                    file.write(&filebuf)?;
                    std::result::Result::Ok(())
                },
                64 => {
                    let filebuf = form_wav_file_data_f64(buffer, filebuf)?;
                    file.write(&filebuf)?;
                    std::result::Result::Ok(())
                },
                _ => std::result::Result::Err("Invalid bits per sample!")
            };
        }
    }
    Ok(())
}

pub fn write_wav_file_f32(
    location: &Path,
    buffer: &[f32],
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    sample_length: u32,
    audio_format: WaveAudioFormat
) -> io::Result<()> {
    let mut file = File::options().write(true).create(true).truncate(true).open(location)?;
    match audio_format {
        WaveAudioFormat::PulseCodeModulation => {
            let filebuf: Vec<u8> = form_wav_file_header(
                sample_rate,
                channels,
                bits_per_sample,
                sample_length,
                audio_format
            )?;
            let _bmatch: std::result::Result<(), &str> = match bits_per_sample {
                8 => {
                    let filebuf = form_wav_file_data_f32toi8(buffer, filebuf)?;
                    file.write(&filebuf)?;
                    std::result::Result::Ok(())
                },
                16 => {
                    let filebuf = form_wav_file_data_f32toi16(buffer, filebuf)?;
                    file.write(&filebuf)?;
                    std::result::Result::Ok(())
                },
                32 => {
                    let filebuf = form_wav_file_data_f32toi32(buffer, filebuf)?;
                    file.write(&filebuf)?;
                    std::result::Result::Ok(())
                },
                64 => {
                    let filebuf = form_wav_file_data_f32toi64(buffer, filebuf)?;
                    file.write(&filebuf)?;
                    std::result::Result::Ok(())
                },
                _ => std::result::Result::Err("Invalid bits per sample!")
            };
        },
        WaveAudioFormat::FloatingPoint => {
            let filebuf: Vec<u8> = form_wav_file_header(
                sample_rate,
                channels,
                bits_per_sample,
                sample_length,
                audio_format
            )?;
            let _bmatch: std::result::Result<(), &str> = match bits_per_sample {
                32 => {
                    let filebuf = form_wav_file_data_f32(buffer, filebuf)?;
                    file.write(&filebuf)?;
                    std::result::Result::Ok(())
                },
                64 => {
                    let filebuf = form_wav_file_data_f32tof64(buffer, filebuf)?;
                    file.write(&filebuf)?;
                    std::result::Result::Ok(())
                },
                _ => std::result::Result::Err("Invalid bits per sample!")
            };
        }
    }
    Ok(())
}