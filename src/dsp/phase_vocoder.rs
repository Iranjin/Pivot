use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use num_complex::Complex32;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

use crate::params::Quality;

const HOP_DIVISOR: usize = 8;
const OLA_MULTIPLIER: usize = 8;

pub struct PitchShifter {
    engines: [VocoderEngine; 3],
    active: usize,
}

impl PitchShifter {
    pub fn new(channels: usize) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let engines = [
            VocoderEngine::new(Quality::Low, channels, &mut planner),
            VocoderEngine::new(Quality::Balanced, channels, &mut planner),
            VocoderEngine::new(Quality::High, channels, &mut planner),
        ];

        Self {
            engines,
            active: Quality::Balanced as usize,
        }
    }

    pub fn set_quality(&mut self, quality: Quality) {
        let index = quality as usize;
        if index != self.active {
            self.active = index;
            self.engines[index].reset();
        }
    }

    pub fn reset(&mut self) {
        for engine in &mut self.engines {
            engine.reset();
        }
    }

    pub fn channel_count(&self) -> usize {
        self.engines[self.active].channels.len()
    }

    pub fn process_sample(&mut self, channel: usize, input: f32, ratio: f32) -> (f32, f32) {
        self.engines[self.active].process_sample(channel, input, ratio)
    }

    pub fn latency_samples(&self) -> u32 {
        self.engines[self.active].n as u32
    }
}

struct VocoderEngine {
    n: usize,
    channels: Vec<Channel>,
}

impl VocoderEngine {
    fn new(quality: Quality, channel_count: usize, planner: &mut RealFftPlanner<f32>) -> Self {
        let n = quality.fft_size();
        let forward = planner.plan_fft_forward(n);
        let inverse = planner.plan_fft_inverse(n);
        let channels = (0..channel_count)
            .map(|_| Channel::new(n, forward.clone(), inverse.clone()))
            .collect();

        Self { n, channels }
    }

    fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.reset();
        }
    }

    fn process_sample(&mut self, channel: usize, input: f32, ratio: f32) -> (f32, f32) {
        match self.channels.get_mut(channel) {
            Some(channel) => channel.process(input, ratio),
            None => (input, input),
        }
    }
}

struct Channel {
    n: usize,
    hop_a: usize,
    bins: usize,
    input_ring: Vec<f32>,
    input_write: usize,
    samples_seen: u64,
    fft_input: Vec<f32>,
    spectrum: Vec<Complex32>,
    fft_scratch: Vec<Complex32>,
    ifft_output: Vec<f32>,
    ifft_scratch: Vec<Complex32>,
    ola: Vec<f32>,
    ola_mask: usize,
    write_pos: f64,
    read_pos: f64,
    zero_pos: usize,
    window: Vec<f32>,
    phase: Vec<f32>,
    prev_phase: Vec<f32>,
    prev_synth: Vec<f32>,
    new_synth: Vec<f32>,
    magnitude: Vec<f32>,
    peaks: Vec<usize>,
    first_frame: bool,
    delay: Vec<f32>,
    delay_write: usize,
    delay_count: u64,
    forward: Arc<dyn RealToComplex<f32>>,
    inverse: Arc<dyn ComplexToReal<f32>>,
}

impl Channel {
    fn new(
        n: usize,
        forward: Arc<dyn RealToComplex<f32>>,
        inverse: Arc<dyn ComplexToReal<f32>>,
    ) -> Self {
        let hop_a = n / HOP_DIVISOR;
        let bins = n / 2 + 1;
        let ola_len = n * OLA_MULTIPLIER;
        let window = (0..n)
            .map(|i| 0.5 - 0.5 * (TAU * i as f32 / n as f32).cos())
            .collect();

        Self {
            n,
            hop_a,
            bins,
            input_ring: vec![0.0; n],
            input_write: 0,
            samples_seen: 0,
            fft_input: vec![0.0; n],
            spectrum: forward.make_output_vec(),
            fft_scratch: forward.make_scratch_vec(),
            ifft_output: inverse.make_output_vec(),
            ifft_scratch: inverse.make_scratch_vec(),
            ola: vec![0.0; ola_len],
            ola_mask: ola_len - 1,
            write_pos: 0.0,
            read_pos: 0.0,
            zero_pos: 0,
            window,
            phase: vec![0.0; bins],
            prev_phase: vec![0.0; bins],
            prev_synth: vec![0.0; bins],
            new_synth: vec![0.0; bins],
            magnitude: vec![0.0; bins],
            peaks: Vec::with_capacity(bins),
            first_frame: true,
            delay: vec![0.0; n],
            delay_write: 0,
            delay_count: 0,
            forward,
            inverse,
        }
    }

    fn reset(&mut self) {
        self.input_ring.fill(0.0);
        self.input_write = 0;
        self.samples_seen = 0;
        self.fft_input.fill(0.0);
        self.ifft_output.fill(0.0);
        self.ola.fill(0.0);
        self.write_pos = 0.0;
        self.read_pos = 0.0;
        self.zero_pos = 0;
        self.phase.fill(0.0);
        self.prev_phase.fill(0.0);
        self.prev_synth.fill(0.0);
        self.new_synth.fill(0.0);
        self.magnitude.fill(0.0);
        self.peaks.clear();
        self.first_frame = true;
        self.delay.fill(0.0);
        self.delay_write = 0;
        self.delay_count = 0;
    }

    fn process(&mut self, input: f32, ratio: f32) -> (f32, f32) {
        self.input_ring[self.input_write] = input;
        self.input_write = (self.input_write + 1) & (self.n - 1);
        self.samples_seen += 1;

        if self.samples_seen >= self.n as u64
            && (self.samples_seen - self.n as u64).is_multiple_of(self.hop_a as u64)
        {
            self.process_frame(ratio);
        }

        let wet = if self.samples_seen > self.n as u64 {
            let output = self.read_ola();
            self.read_pos += ratio as f64;
            output
        } else {
            0.0
        };

        (wet, self.delay_sample(input))
    }

    fn process_frame(&mut self, ratio: f32) {
        let mask = self.n - 1;
        for i in 0..self.n {
            let index = (self.input_write + i) & mask;
            self.fft_input[i] = self.input_ring[index] * self.window[i];
        }

        if self
            .forward
            .process_with_scratch(
                &mut self.fft_input,
                &mut self.spectrum,
                &mut self.fft_scratch,
            )
            .is_err()
        {
            return;
        }

        let bins = self.bins;
        let hop_s = self.hop_a as f32 * ratio;

        for k in 0..bins {
            let re = self.spectrum[k].re;
            let im = self.spectrum[k].im;
            self.magnitude[k] = re.hypot(im);
            self.phase[k] = im.atan2(re);
        }

        if self.first_frame {
            self.new_synth.copy_from_slice(&self.phase);
            self.prev_phase.copy_from_slice(&self.phase);
            self.first_frame = false;
        } else {
            for k in 0..bins {
                let expected = TAU * k as f32 * self.hop_a as f32 / self.n as f32;
                let delta = princarg(self.phase[k] - self.prev_phase[k] - expected);
                let frequency = (expected + delta) / self.hop_a as f32;

                self.new_synth[k] = self.prev_synth[k] + frequency * hop_s;
                self.prev_phase[k] = self.phase[k];
            }

            self.find_peaks();
            if !self.peaks.is_empty() {
                let mut peak_index = 0;
                for k in 0..bins {
                    while peak_index + 1 < self.peaks.len() {
                        let current = self.peaks[peak_index].abs_diff(k);
                        let next = self.peaks[peak_index + 1].abs_diff(k);
                        if next < current {
                            peak_index += 1;
                        } else {
                            break;
                        }
                    }

                    let peak = self.peaks[peak_index];
                    self.new_synth[k] = self.new_synth[peak] + (self.phase[k] - self.phase[peak]);
                }
            }
        }

        self.prev_synth.copy_from_slice(&self.new_synth);

        let dc = self.spectrum[0];
        let nyquist = self.spectrum[bins - 1];
        let cutoff = if ratio > 1.0 {
            (bins as f32 / ratio).min(bins as f32 - 1.0)
        } else {
            bins as f32
        };

        for k in 0..bins {
            let gain = if ratio > 1.0 {
                bandlimit_gain(k as f32, cutoff)
            } else {
                1.0
            };
            let magnitude = self.magnitude[k] * gain;
            let phase = self.new_synth[k];
            self.spectrum[k] = Complex32::new(magnitude * phase.cos(), magnitude * phase.sin());
        }
        self.spectrum[0] = Complex32::new(dc.re, 0.0);
        self.spectrum[bins - 1] = Complex32::new(nyquist.re, 0.0);

        if self
            .inverse
            .process_with_scratch(
                &mut self.spectrum,
                &mut self.ifft_output,
                &mut self.ifft_scratch,
            )
            .is_err()
        {
            return;
        }

        let scale = 8.0 * hop_s / (3.0 * self.n as f32 * self.n as f32);
        let base = self.write_pos.round() as usize;
        for i in 0..self.n {
            let index = (base + i) & self.ola_mask;
            self.ola[index] += self.ifft_output[i] * self.window[i] * scale;
        }
        self.write_pos += hop_s as f64;
    }

    fn find_peaks(&mut self) {
        self.peaks.clear();
        if self.bins < 3 {
            return;
        }

        for k in 1..self.bins - 1 {
            let magnitude = self.magnitude[k];
            if magnitude > self.magnitude[k - 1] && magnitude >= self.magnitude[k + 1] {
                self.peaks.push(k);
            }
        }

        if self.peaks.is_empty() {
            let mut max_index = 0;
            let mut max_magnitude = 0.0;
            for (k, &magnitude) in self.magnitude.iter().enumerate() {
                if magnitude > max_magnitude {
                    max_magnitude = magnitude;
                    max_index = k;
                }
            }
            if max_magnitude > 1.0e-9 {
                self.peaks.push(max_index);
            }
        }
    }

    fn read_ola(&mut self) -> f32 {
        let max_read = self.write_pos + (self.n / 2) as f64 - 2.0;
        if self.read_pos > max_read {
            self.read_pos = max_read;
        }

        let position = self.read_pos;
        let index = position.floor() as usize;
        let fraction = (position - index as f64) as f32;

        let a = self.ola[index & self.ola_mask];
        let b = self.ola[(index + 1) & self.ola_mask];
        let output = a + (b - a) * fraction;

        let zero_target = index.saturating_sub(2);
        while self.zero_pos < zero_target {
            self.ola[self.zero_pos & self.ola_mask] = 0.0;
            self.zero_pos += 1;
        }

        output
    }

    fn delay_sample(&mut self, input: f32) -> f32 {
        let output = if self.delay_count >= self.n as u64 {
            self.delay[self.delay_write]
        } else {
            0.0
        };

        self.delay[self.delay_write] = input;
        self.delay_write = (self.delay_write + 1) & (self.n - 1);
        self.delay_count += 1;

        output
    }
}

fn princarg(phase: f32) -> f32 {
    let mut wrapped = (phase + PI) % TAU;
    if wrapped < 0.0 {
        wrapped += TAU;
    }
    wrapped - PI
}

fn bandlimit_gain(bin: f32, cutoff: f32) -> f32 {
    let taper_start = cutoff * 0.9;
    if bin <= taper_start {
        1.0
    } else if bin >= cutoff {
        0.0
    } else {
        let position = (bin - taper_start) / (cutoff - taper_start);
        0.5 + 0.5 * (PI * position).cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 48_000.0;

    fn sine(frequency: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|i| (TAU * frequency * i as f32 / SAMPLE_RATE).sin())
            .collect()
    }

    fn render(ratio: f32, input: &[f32]) -> (Vec<f32>, usize) {
        let mut shifter = PitchShifter::new(1);
        shifter.set_quality(Quality::Low);
        let latency = shifter.latency_samples() as usize;
        let output = input
            .iter()
            .map(|&sample| shifter.process_sample(0, sample, ratio).0)
            .collect();

        (output, latency)
    }

    fn frequency(signal: &[f32]) -> f32 {
        let mut crossings = 0;
        for window in signal.windows(2) {
            if window[0] <= 0.0 && window[1] > 0.0 {
                crossings += 1;
            }
        }

        crossings as f32 * SAMPLE_RATE / signal.len() as f32
    }

    #[test]
    fn unity_ratio_matches_delayed_input() {
        let input = sine(440.0, 12_000);
        let (output, latency) = render(1.0, &input);

        let start = latency * 2;
        let mut error = 0.0;
        let mut signal = 0.0;
        for i in start..input.len() {
            let difference = output[i] - input[i - latency];
            error += difference * difference;
            signal += input[i - latency] * input[i - latency];
        }

        let relative_error = (error / signal).sqrt();
        assert!(relative_error < 1.0e-4, "relative error {relative_error}");
    }

    #[test]
    fn octave_up_and_down() {
        let input = sine(440.0, 24_000);
        let discard = 6_000;

        let (up, _) = render(2.0, &input);
        let up_frequency = frequency(&up[discard..]);
        assert!((up_frequency - 880.0).abs() < 10.0, "up: {up_frequency} Hz");

        let (down, _) = render(0.5, &input);
        let down_frequency = frequency(&down[discard..]);
        assert!(
            (down_frequency - 220.0).abs() < 10.0,
            "down: {down_frequency} Hz"
        );
    }

    #[test]
    fn extreme_ratios_stay_finite() {
        let input = sine(440.0, 12_000);
        for ratio in [0.25, 4.0] {
            let (output, _) = render(ratio, &input);
            assert!(output.iter().all(|sample| sample.is_finite()));
        }
    }

    #[test]
    fn quality_changes_latency() {
        let mut shifter = PitchShifter::new(1);
        shifter.set_quality(Quality::Low);
        assert_eq!(shifter.latency_samples(), 1024);
        shifter.set_quality(Quality::Balanced);
        assert_eq!(shifter.latency_samples(), 2048);
        shifter.set_quality(Quality::High);
        assert_eq!(shifter.latency_samples(), 4096);
    }
}
