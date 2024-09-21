pub mod device;
pub mod processing;
pub mod wavefile;

pub fn f32_samples_mono_to_stereo(samples: &[f32]) -> Vec<f32> {
    return samples.iter().flat_map(|x| [*x, *x]).collect();
}

pub fn f64_samples_mono_to_stereo(samples: &[f64]) -> Vec<f64> {
    return samples.iter().flat_map(|x| [*x, *x]).collect();
}

pub fn f64_size_to_f32(samples: &[f64]) -> Vec<f32> {
    return samples.iter().flat_map(|x| [*x as f32]).collect();
}

pub fn f32_size_to_f64(samples: &[f32]) -> Vec<f64> {
    return samples.iter().flat_map(|x| [f64::from(*x)]).collect();
}
