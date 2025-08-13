use std::f64::consts::TAU;

use cpal::FromSample;

/// Given a `frequency` in hertz and an `amplitude`, return a function over time (in seconds) that generates a sine wave.
pub fn sine_wave(frequency: f64, amplitude: f64) -> impl FnMut(f64) -> f64
where
    f64: FromSample<f64>,
{
    move |time| amplitude * (TAU * frequency * time).sin()
}

/// Given a `frequency` in hertz and an `amplitude`, return a function over time (in seconds) that generates a square wave.
pub fn square_wave(frequency: f64, amplitude: f64) -> impl FnMut(f64) -> f64
where
    f64: FromSample<f64>,
{
    move |time| {
        #[allow(clippy::cast_possible_truncation, reason = "truncation is intentional")]
        ((-1_f64).powi((2. * frequency * time) as _) * amplitude)
    }
}

/// Given a `frequency` in hertz and an `amplitude`, return a function over time (in seconds) that generates a triangle wave.
pub fn triangle_wave(frequency: f64, amplitude: f64) -> impl FnMut(f64) -> f64
where
    f64: FromSample<f64>,
{
    move |time| (2. * amplitude) * time.mul_add(frequency, -time.mul_add(frequency, 1. / 2.).floor()).abs()
}

/// Given a `frequency` in hertz and an `amplitude`, return a function over time (in seconds) that generates a sawtooth wave.
pub fn sawtooth_wave(frequency: f64, amplitude: f64) -> impl FnMut(f64) -> f64
where
    f64: FromSample<f64>,
{
    move |time| (2. * amplitude) * time.mul_add(frequency, -time.mul_add(frequency, 1. / 2.).floor())
}

/// Return a function that generates silence.
pub fn silence() -> impl FnMut(f64) -> f64 {
    move |_| 0.
}

#[derive(Clone, Copy, Debug)]
/// A sine wave with a given `amplitude` and `index`.
///
/// The `index` is used to determine the harmonic frequency of the wave. See [`harmonics`] for more information.
pub struct Harmonic {
    amplitude: f64,
    index: usize,
}

impl Harmonic {
    /// Create a new harmonic with the given `amplitude` and `index`.
    #[must_use]
    pub const fn new(amplitude: f64, index: usize) -> Self {
        Self { amplitude, index }
    }
}

/// Return a function over time (in seconds) that generates a wave resulting from the sum of the given harmonics.
///
/// Each harmonic is a sine wave with a given `amplitude` and `index`, with a frequency of `(index + 1) * fundamental_frequency`.
pub fn harmonics(fundamental_frequency: f64, harmonics: &[Harmonic]) -> impl FnMut(f64) -> f64 + use<'_> {
    move |time| {
        #[allow(clippy::cast_precision_loss, reason = "loss of precision is expected when harmonic indexes are large")]
        harmonics
            .iter()
            .map(|harmonic| sine_wave((harmonic.index as f64 + 1.) * fundamental_frequency, harmonic.amplitude)(time) / (harmonic.index as f64 + 1.))
            .sum()
    }
}
