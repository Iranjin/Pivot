use std::f32::consts::TAU;
use std::sync::Arc;

use num_complex::Complex32;
use realfft::{RealFftPlanner, RealToComplex};

const FFT_SIZE: usize = 4096;
const HOP: usize = 2048;
const MIN_F0_HZ: f32 = 40.0;
const MAX_F0_HZ: f32 = 800.0;
const HARMONICS: usize = 5;

pub struct PitchAnalyzer {
    sample_rate: f32,
    fft: Arc<dyn RealToComplex<f32>>,
    ring: Vec<f32>,
    fft_input: Vec<f32>,
    spectrum: Vec<Complex32>,
    scratch: Vec<Complex32>,
    magnitude: Vec<f32>,
    window: Vec<f32>,
    write: usize,
    count: u64,
    f0_hz: f32,
    mono_confidence: f32,
}

impl PitchAnalyzer {
    pub fn new(sample_rate: f32) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let spectrum = fft.make_output_vec();
        let scratch = fft.make_scratch_vec();
        let window = (0..FFT_SIZE)
            .map(|i| 0.5 - 0.5 * (TAU * i as f32 / FFT_SIZE as f32).cos())
            .collect();

        Self {
            sample_rate,
            fft,
            ring: vec![0.0; FFT_SIZE],
            fft_input: vec![0.0; FFT_SIZE],
            spectrum,
            scratch,
            magnitude: vec![0.0; FFT_SIZE / 2 + 1],
            window,
            write: 0,
            count: 0,
            f0_hz: 0.0,
            mono_confidence: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.ring.fill(0.0);
        self.fft_input.fill(0.0);
        self.magnitude.fill(0.0);
        self.write = 0;
        self.count = 0;
        self.f0_hz = 0.0;
        self.mono_confidence = 0.0;
    }

    pub fn f0_hz(&self) -> f32 {
        self.f0_hz
    }

    pub fn monophonicity(&self) -> f32 {
        self.mono_confidence
    }

    pub fn process_sample(&mut self, sample: f32) {
        self.ring[self.write] = sample;
        self.write = (self.write + 1) & (FFT_SIZE - 1);
        self.count += 1;

        if self.count >= FFT_SIZE as u64
            && (self.count - FFT_SIZE as u64).is_multiple_of(HOP as u64)
        {
            self.analyze();
        }
    }

    fn analyze(&mut self) {
        let mask = FFT_SIZE - 1;
        for i in 0..FFT_SIZE {
            let index = (self.write + i) & mask;
            self.fft_input[i] = self.ring[index] * self.window[i];
        }

        if self
            .fft
            .process_with_scratch(&mut self.fft_input, &mut self.spectrum, &mut self.scratch)
            .is_err()
        {
            return;
        }

        for (magnitude, value) in self.magnitude.iter_mut().zip(&self.spectrum) {
            *magnitude = value.norm();
        }

        self.update_monophonicity();
    }

    fn update_monophonicity(&mut self) {
        let bins = self.magnitude.len();
        if bins < 8 {
            return;
        }

        let mut log_sum = 0.0;
        let mut linear_sum = 0.0;
        let mut total_energy = 0.0;
        for k in 1..bins - 1 {
            let magnitude = self.magnitude[k] + 1.0e-12;
            log_sum += magnitude.ln();
            linear_sum += magnitude;
            total_energy += magnitude * magnitude;
        }

        let count = (bins - 2) as f32;
        let geometric = (log_sum / count).exp();
        let arithmetic = linear_sum / count;
        let flatness = if arithmetic > 1.0e-12 {
            (geometric / arithmetic).clamp(0.0, 1.0)
        } else {
            1.0
        };

        let min_bin = ((MIN_F0_HZ * FFT_SIZE as f32 / self.sample_rate).round() as usize).max(1);
        let max_bin = ((MAX_F0_HZ * FFT_SIZE as f32 / self.sample_rate).round() as usize)
            .min(bins / HARMONICS);

        let mut best_bin = 0;
        let mut best_score = f32::NEG_INFINITY;
        let magnitude_threshold = self.magnitude.iter().copied().fold(0.0, f32::max) * 0.05;
        if max_bin > min_bin {
            for candidate in min_bin..=max_bin {
                if self.magnitude[candidate] < magnitude_threshold {
                    continue;
                }
                if self.magnitude[candidate] <= self.magnitude[candidate - 1]
                    || self.magnitude[candidate] < self.magnitude[candidate + 1]
                {
                    continue;
                }

                let mut strongest: f32 = 0.0;
                for harmonic in 1..=HARMONICS {
                    let bin = (candidate * harmonic).min(bins - 1);
                    strongest = strongest.max(self.magnitude[bin]);
                }
                if self.magnitude[candidate] < strongest * 0.25 {
                    continue;
                }

                let mut score = 0.0;
                for harmonic in 1..=HARMONICS {
                    let bin = candidate * harmonic;
                    if bin >= bins {
                        break;
                    }
                    score += (self.magnitude[bin] + 1.0e-12).ln();
                }
                if score > best_score {
                    best_score = score;
                    best_bin = candidate;
                }
            }
        }

        let mut harmonic_energy = 0.0;
        if best_bin > 0 {
            for harmonic in 1..=HARMONICS {
                let center = best_bin * harmonic;
                if center >= bins {
                    break;
                }
                let start = center.saturating_sub(1);
                let end = (center + 1).min(bins - 1);
                for bin in start..=end {
                    harmonic_energy += self.magnitude[bin] * self.magnitude[bin];
                }
            }
        }

        let confidence = if total_energy > 1.0e-12 {
            (harmonic_energy / total_energy).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let target = confidence * (1.0 - flatness).clamp(0.0, 1.0);
        self.mono_confidence += (target - self.mono_confidence) * 0.1;

        let refined_bin = if best_bin > 0 {
            let left = best_bin.saturating_sub(1);
            let right = (best_bin + 1).min(bins - 1);
            let a = (self.magnitude[left] + 1.0e-12).ln();
            let b = (self.magnitude[best_bin] + 1.0e-12).ln();
            let c = (self.magnitude[right] + 1.0e-12).ln();
            let denominator = a - 2.0 * b + c;
            let offset = if denominator.abs() > 1.0e-9 {
                (0.5 * (a - c) / denominator).clamp(-0.5, 0.5)
            } else {
                0.0
            };
            best_bin as f32 + offset
        } else {
            0.0
        };
        self.f0_hz = refined_bin * self.sample_rate / FFT_SIZE as f32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 48_000.0;

    #[test]
    fn detects_f0_and_monophonicity() {
        let mut analyzer = PitchAnalyzer::new(SAMPLE_RATE);
        for i in 0..96_000 {
            analyzer.process_sample((TAU * 220.0 * i as f32 / SAMPLE_RATE).sin());
        }

        assert!(
            (analyzer.f0_hz() - 220.0).abs() < 5.0,
            "{}",
            analyzer.f0_hz()
        );
        assert!(
            analyzer.monophonicity() > 0.5,
            "{}",
            analyzer.monophonicity()
        );
    }
}
