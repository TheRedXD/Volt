use rodio::{
    OutputStream,
    buffer::SamplesBuffer
};

use cpal::{Sample, SampleFormat};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::blerp;

fn soine() -> Vec<f64> {
    (0..44100)
        .map(|i| ((2.0 * std::f64::consts::PI) * (440.0 * f64::from(i)) / 44100.0).sin())
        .collect()
}

fn soiniet(i: i32) -> f64 {
    ((2.0 * std::f64::consts::PI) * (440.0 * f64::from(i)) / 44100.0).sin()
}

pub fn test() {
    std::thread::spawn(|| {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let source: Vec<f32> = blerp::f64_size_to_f32(
            &soine()
        );
        stream_handle.play_raw(SamplesBuffer::new(1, 44100, source)).unwrap();

        std::thread::sleep(std::time::Duration::from_secs(5));
    });
}

// CPAL TEST
// TODO: Implement sinewave playback
/*
    SolarLiner — Yesterday at 9:08 PM
    You have to be able to read and write any type of sample, as evidenced by the generic type T: Sample, this is because not all audio devices support all the types
    however cpal has a sample conversion trait FromSample and ToSample which can convert between sample formats
    So the first step is to have a fn basic_sine<T: Sample + FromSample<f64>>(data: &mut [T], _: &cpal::OutputCallbackInfo) function which writes to databy using the FromSample trait, like this, in a for loop: data[i] = T::from_sample(generate_sample(i)), with generate_sample being some kind of method or closure that generates a single f64, and is called for every sample that will be played by cpal
    The other problem is that i in that function refers to the index of the callback data, and is not monotonous (ie. not continuously increasing, it resets at each call of the callback function), so you'll have to persist a global sample count, and probably switch to a lambda closure rather than a simple function to be able to capture that sample counter
    (or make a proper struct, and have the function generate_sample be a method on that struct that generates each sample and keeps its own running sample counter)
    SolarLiner — Yesterday at 9:16 PM
    The docs for cpal (and any other crate that's published on crates.io) is here https://docs.rs/cpal/
    cpal - Rust
    How to use cpal
    And cpal has an example for generating a sine wave here: https://github.com/RustAudio/cpal/blob/0da7ae1d426d471cf352550d0048bac91c8bac38/examples/beep.rs#L97 (look in the run function, there's a next_valueclosure that does what you want)
*/

fn write_silence<T: Sample>(data: &mut [T], _: &cpal::OutputCallbackInfo) {
    for sample in data.iter_mut() {
        *sample = Sample::EQUILIBRIUM;
    }
}

pub fn cpaltest() {
    let err_fn = |err| eprintln!("an error occurred on the output audio stream: {err}");
    let host = cpal::default_host();
    let device = host.default_output_device().expect("no output device available");
    let mut supported_configs_range = device.supported_output_configs()
        .expect("error while querying configs");
    let supported_config = supported_configs_range.next()
        .expect("no supported config?!")
        .with_max_sample_rate();
    let sample_format = supported_config.sample_format();
    let config = supported_config.into();
    let stream = match sample_format {
        SampleFormat::F64 => device.build_output_stream(&config, write_silence::<f64>, err_fn, None),
        SampleFormat::I64 => device.build_output_stream(&config, write_silence::<i64>, err_fn, None),
        SampleFormat::U64 => device.build_output_stream(&config, write_silence::<u64>, err_fn, None),
        SampleFormat::F32 => device.build_output_stream(&config, write_silence::<f32>, err_fn, None),
        SampleFormat::I32 => device.build_output_stream(&config, write_silence::<i32>, err_fn, None),
        SampleFormat::U32 => device.build_output_stream(&config, write_silence::<u32>, err_fn, None),
        SampleFormat::I16 => device.build_output_stream(&config, write_silence::<i16>, err_fn, None),
        SampleFormat::U16 => device.build_output_stream(&config, write_silence::<u16>, err_fn, None),
        SampleFormat::I8 => device.build_output_stream(&config, write_silence::<i8>, err_fn, None),
        SampleFormat::U8 => device.build_output_stream(&config, write_silence::<u8>, err_fn, None),
        sample_format => panic!("Unsupported sample format '{sample_format}'")
    }.unwrap();
    stream.play().unwrap();
}