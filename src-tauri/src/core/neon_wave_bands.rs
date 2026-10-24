//! Small, fixed-memory band-energy meter for visual animation (24 kHz mono PCM).
#[derive(Default)]
pub struct BandMeter {
    low: f32,
    upper: f32,
    energy: [f64; 3],
    samples: usize,
}
impl BandMeter {
    pub fn push(&mut self, bytes: &[u8]) {
        // Two one-pole crossover filters: bass < 180 Hz, mid < 2.5 kHz.
        const LOW: f32 = 0.04603;
        const UPPER: f32 = 0.4803;
        for pair in bytes.chunks_exact(2) {
            let value = i16::from_le_bytes([pair[0], pair[1]]) as f32 / 32768.0;
            self.low += LOW * (value - self.low);
            self.upper += UPPER * (value - self.upper);
            for (energy, value) in
                self.energy
                    .iter_mut()
                    .zip([self.low, self.upper - self.low, value - self.upper])
            {
                *energy += (value as f64).powi(2);
            }
            self.samples += 1;
        }
    }
    pub fn take(&mut self) -> [f32; 3] {
        let count = self.samples.max(1) as f64;
        let values = self.energy.map(|energy| {
            (((energy / count).sqrt() * 9.0).ln_1p() / 2.302585).clamp(0.0, 1.0) as f32
        });
        self.energy = [0.0; 3];
        self.samples = 0;
        values
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn tone(hz: f32) -> [f32; 3] {
        let mut meter = BandMeter::default();
        for i in 0..24000 {
            let sample = ((i as f32 * std::f32::consts::TAU * hz / 24000.0).sin() * 16000.0) as i16;
            meter.push(&sample.to_le_bytes());
        }
        meter.take()
    }
    #[test]
    fn silence_is_zero_and_clears_previous_frame() {
        let mut meter = BandMeter::default();
        meter.push(&[0; 960]);
        assert_eq!(meter.take(), [0.0; 3]);
        assert_eq!(meter.take(), [0.0; 3]);
    }
    #[test]
    fn separates_low_mid_and_high_tones() {
        for (hz, band) in [(60.0, 0), (800.0, 1), (9000.0, 2)] {
            let levels = tone(hz);
            assert!(
                levels[band] > levels[(band + 1) % 3] && levels[band] > levels[(band + 2) % 3],
                "{hz}: {levels:?}"
            );
            assert!(levels
                .into_iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(&v)));
        }
    }
}
