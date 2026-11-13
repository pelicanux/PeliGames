//! The same mono, 48 kHz RNNoise processing for calls and gameplay microphones.
pub struct RnNoise {
    state: Box<nnnoiseless::DenoiseState<'static>>,
}
impl RnNoise {
    pub fn new() -> Self {
        Self {
            state: nnnoiseless::DenoiseState::new(),
        }
    }
    pub fn process(&mut self, frame: &mut [i16; 480]) {
        // nnnoiseless uses floating point samples in the i16 amplitude range.
        let input = frame.map(f32::from);
        let mut output = [0.; 480];
        self.state.process_frame(&mut output, &input);
        for (dst, value) in frame.iter_mut().zip(output) {
            *dst = value.clamp(-32768., 32767.) as i16;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn silence_stays_silent_and_steady_noise_is_attenuated() {
        let mut processor = RnNoise::new();
        let mut silence = [0; 480];
        processor.process(&mut silence);
        assert_eq!(silence, [0; 480]);
        let mut seed = 42u32;
        let mut input_energy = 0f64;
        let mut output_energy = 0f64;
        for n in 0..100 {
            let mut frame = std::array::from_fn(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                ((seed >> 16) as i32 % 1001 - 500) as i16
            });
            if n >= 20 {
                input_energy += frame.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>();
            }
            processor.process(&mut frame);
            if n >= 20 {
                output_energy += frame.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>();
            }
        }
        assert!(
            output_energy < input_energy,
            "The filter must attenuate steady background noise"
        );
    }
}
