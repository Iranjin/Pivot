use std::f32::consts::TAU;
use std::sync::Arc;

use num_complex::Complex32;
use realfft::{RealFftPlanner, RealToComplex};

use crate::params::KeyMode;

const FFT_SIZE: usize = 4096;
const HOP: usize = 2048;
const MIN_FREQ: f32 = 55.0;
const MAX_FREQ: f32 = 5000.0;

const MAJOR_PROFILE: [f32; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const MINOR_PROFILE: [f32; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

#[derive(Clone, Copy, Debug, Default)]
pub struct KeyEstimate {
    pub root: u8,
    pub mode: KeyMode,
    pub confidence: f32,
}

pub struct KeyDetector {
    sample_rate: f32,
    fft: Arc<dyn RealToComplex<f32>>,
    ring: Vec<f32>,
    fft_input: Vec<f32>,
    spectrum: Vec<Complex32>,
    scratch: Vec<Complex32>,
    window: Vec<f32>,
    write: usize,
    count: u64,
    chroma: [f32; 12],
    tuning_cents: f32,
    estimate: KeyEstimate,
}

impl KeyDetector {
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
            window,
            write: 0,
            count: 0,
            chroma: [0.0; 12],
            tuning_cents: 0.0,
            estimate: KeyEstimate::default(),
        }
    }

    pub fn reset(&mut self) {
        self.ring.fill(0.0);
        self.fft_input.fill(0.0);
        self.write = 0;
        self.count = 0;
        self.chroma = [0.0; 12];
        self.tuning_cents = 0.0;
        self.estimate = KeyEstimate::default();
    }

    pub fn estimate(&self) -> KeyEstimate {
        self.estimate
    }

    pub fn process_sample(&mut self, sample: f32) -> bool {
        self.ring[self.write] = sample;
        self.write = (self.write + 1) & (FFT_SIZE - 1);
        self.count += 1;

        if self.count >= FFT_SIZE as u64
            && (self.count - FFT_SIZE as u64).is_multiple_of(HOP as u64)
        {
            self.analyze();
            true
        } else {
            false
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

        self.update_tuning();
        self.update_chroma();
        self.update_key();
    }

    fn update_tuning(&mut self) {
        let max_bin =
            ((2000.0 * FFT_SIZE as f32 / self.sample_rate) as usize).min(FFT_SIZE / 2 - 1);

        let mut peak_bin = 0;
        let mut peak_magnitude = 0.0;
        for k in 1..max_bin {
            let magnitude = self.spectrum[k].norm();
            if magnitude > peak_magnitude {
                peak_magnitude = magnitude;
                peak_bin = k;
            }
        }

        if peak_bin > 0 && peak_magnitude > 1.0e-6 {
            let frequency = self.refined_frequency(peak_bin);
            let midi = 69.0 + 12.0 * (frequency / 440.0).log2();
            let deviation = ((midi - midi.round()) * 100.0).clamp(-50.0, 50.0);
            self.tuning_cents += (deviation - self.tuning_cents) * 0.05;
        }
    }

    fn refined_frequency(&self, bin: usize) -> f32 {
        let left = bin.saturating_sub(1);
        let right = (bin + 1).min(FFT_SIZE / 2);
        let a = (self.spectrum[left].norm() + 1.0e-12).ln();
        let b = (self.spectrum[bin].norm() + 1.0e-12).ln();
        let c = (self.spectrum[right].norm() + 1.0e-12).ln();
        let denominator = a - 2.0 * b + c;
        let offset = if denominator.abs() > 1.0e-9 {
            (0.5 * (a - c) / denominator).clamp(-0.5, 0.5)
        } else {
            0.0
        };

        (bin as f32 + offset) * self.sample_rate / FFT_SIZE as f32
    }

    fn update_chroma(&mut self) {
        let mut frame = [0.0f32; 12];
        let mut total = 0.0;
        let nyquist = FFT_SIZE / 2;

        for k in 1..nyquist {
            let frequency = self.refined_frequency(k);
            if !(MIN_FREQ..=MAX_FREQ).contains(&frequency) {
                continue;
            }

            let magnitude = self.spectrum[k].norm();
            if magnitude <= self.spectrum[k - 1].norm() || magnitude < self.spectrum[k + 1].norm() {
                continue;
            }

            let midi = 69.0 + 12.0 * (frequency / 440.0).log2() - self.tuning_cents / 100.0;
            let pitch_class = (midi.round() as i32).rem_euclid(12) as usize;
            frame[pitch_class] += magnitude;
            total += magnitude;
        }

        if total > 1.0e-9 {
            for value in &mut frame {
                *value /= total;
            }
        }

        for (smoothed, value) in self.chroma.iter_mut().zip(frame) {
            *smoothed = *smoothed * 0.95 + value * 0.05;
        }
    }

    fn update_key(&mut self) {
        let mean_chroma = self.chroma.iter().sum::<f32>() / 12.0;
        let chroma_variance = self
            .chroma
            .iter()
            .map(|value| (value - mean_chroma).powi(2))
            .sum::<f32>()
            / 12.0;
        let chroma_std = chroma_variance.sqrt();

        let mut best = (0u8, KeyMode::Major, f32::NEG_INFINITY);
        let mut second = f32::NEG_INFINITY;

        for root in 0..12u8 {
            for (mode, profile) in [
                (KeyMode::Major, &MAJOR_PROFILE),
                (KeyMode::Minor, &MINOR_PROFILE),
            ] {
                let mean_profile = profile.iter().sum::<f32>() / 12.0;
                let profile_variance = profile
                    .iter()
                    .map(|value| (value - mean_profile).powi(2))
                    .sum::<f32>()
                    / 12.0;
                let profile_std = profile_variance.sqrt();

                let mut covariance = 0.0;
                for (degree, &profile_value) in profile.iter().enumerate() {
                    let chroma_value = self.chroma[(root as usize + degree) % 12] - mean_chroma;
                    covariance += chroma_value * (profile_value - mean_profile);
                }

                let score = if chroma_std > 1.0e-9 && profile_std > 1.0e-9 {
                    covariance / (12.0 * chroma_std * profile_std)
                } else {
                    0.0
                };

                if score > best.2 {
                    second = best.2;
                    best = (root, mode, score);
                } else if score > second {
                    second = score;
                }
            }
        }

        let margin = ((best.2 - second).max(0.0) * 3.0).clamp(0.0, 1.0);
        let confidence = (best.2.clamp(0.0, 1.0) * 0.6 + margin * 0.4).clamp(0.0, 1.0);
        self.estimate = KeyEstimate {
            root: best.0,
            mode: best.1,
            confidence,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 48_000.0;

    fn chord(frequencies: &[f32], frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|i| {
                let time = i as f32 / SAMPLE_RATE;
                frequencies
                    .iter()
                    .map(|&frequency| (TAU * frequency * time).sin())
                    .sum::<f32>()
                    / frequencies.len() as f32
            })
            .collect()
    }

    fn detect(input: &[f32]) -> KeyEstimate {
        let mut detector = KeyDetector::new(SAMPLE_RATE);
        for &sample in input {
            detector.process_sample(sample);
        }
        detector.estimate()
    }

    #[test]
    fn detects_c_major() {
        let input = chord(&[261.63, 329.63, 392.0], 288_000);
        let estimate = detect(&input);
        assert_eq!(estimate.root, 0);
        assert_eq!(estimate.mode, KeyMode::Major);
        assert!(estimate.confidence > 0.1, "{}", estimate.confidence);
    }

    #[test]
    fn detects_a_minor() {
        let input = chord(&[220.0, 261.63, 329.63], 288_000);
        let estimate = detect(&input);
        assert_eq!(estimate.root, 9);
        assert_eq!(estimate.mode, KeyMode::Minor);
        assert!(estimate.confidence > 0.1, "{}", estimate.confidence);
    }

    #[test]
    fn silence_is_low_confidence() {
        let input = vec![0.0; 48_000];
        let estimate = detect(&input);
        assert!(estimate.confidence < 0.5);
    }
}
