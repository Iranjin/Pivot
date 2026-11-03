use signalsmith_stretch::Stretch;

use crate::params::Quality;

const CHUNK_FRAMES: usize = 64;

pub struct PitchEngine {
    channels: usize,
    stretches: [Stretch; 3],
    latencies: [usize; 3],
    reported_latency: usize,
    active: usize,
    input: Vec<f32>,
    wet: Vec<f32>,
    dry: Vec<f32>,
    wet_pads: Vec<DelayLine>,
    dry_delays: Vec<DelayLine>,
}

impl PitchEngine {
    pub fn new(sample_rate: f32, channels: usize) -> Self {
        let rate = sample_rate as u32;
        let mut stretches = [
            Stretch::preset_cheaper(channels as u32, rate),
            Stretch::preset_default(channels as u32, rate),
            Stretch::new(
                channels as u32,
                (sample_rate * 0.14) as usize,
                (sample_rate * 0.02) as usize,
            ),
        ];
        {
            let warm_input = vec![0.0; CHUNK_FRAMES * channels];
            let mut warm_output = vec![0.0; CHUNK_FRAMES * channels];
            for stretch in &mut stretches {
                for _ in 0..4 {
                    stretch.process(&warm_input, &mut warm_output);
                }
                stretch.reset();
            }
        }
        let latencies = [
            stretches[0].input_latency() + stretches[0].output_latency(),
            stretches[1].input_latency() + stretches[1].output_latency(),
            stretches[2].input_latency() + stretches[2].output_latency(),
        ];
        let reported_latency = latencies.iter().copied().max().unwrap_or(1).max(1);
        let buffer_len = CHUNK_FRAMES * channels;

        Self {
            channels,
            stretches,
            latencies,
            reported_latency,
            active: 1,
            input: vec![0.0; buffer_len],
            wet: vec![0.0; buffer_len],
            dry: vec![0.0; buffer_len],
            wet_pads: (0..channels)
                .map(|_| DelayLine::new(reported_latency))
                .collect(),
            dry_delays: (0..channels)
                .map(|_| DelayLine::new(reported_latency))
                .collect(),
        }
    }

    pub fn set_quality(&mut self, quality: Quality) {
        let index = quality as usize;
        if index != self.active {
            self.active = index;
            self.stretches[index].reset();
            self.reset_delays();
        }
    }

    pub fn reset(&mut self) {
        for stretch in &mut self.stretches {
            stretch.reset();
        }
        self.reset_delays();
    }

    pub fn latency_samples(&self) -> u32 {
        self.reported_latency as u32
    }

    pub fn channel_count(&self) -> usize {
        self.channels
    }

    pub fn chunk_frames(&self) -> usize {
        CHUNK_FRAMES
    }

    pub fn input_buffer_mut(&mut self) -> &mut [f32] {
        &mut self.input
    }

    pub fn process_chunk(
        &mut self,
        frames: usize,
        transpose_semitones: f32,
        formant_multiplier: f32,
        formant_compensate: bool,
    ) {
        let samples = frames * self.channels;
        let active = self.active;

        {
            let PitchEngine {
                stretches,
                input,
                wet,
                ..
            } = self;
            let stretch = &mut stretches[active];
            stretch.set_transpose_factor_semitones(transpose_semitones, None);
            stretch.set_formant_factor(formant_multiplier, formant_compensate);
            stretch.process(&input[..samples], &mut wet[..samples]);
        }

        let pad_delay = self.reported_latency - self.latencies[active];
        let channel_count = self.channels;
        for (frame, input_frame) in self.input[..samples]
            .chunks_exact(channel_count)
            .enumerate()
        {
            for (channel, &input) in input_frame.iter().enumerate() {
                let index = frame * channel_count + channel;
                let wet = self.wet[index];
                self.wet[index] = self.wet_pads[channel].process(wet, pad_delay);
                self.dry[index] = self.dry_delays[channel].process(input, self.reported_latency);
            }
        }
    }

    pub fn wet(&self) -> &[f32] {
        &self.wet
    }

    pub fn dry(&self) -> &[f32] {
        &self.dry
    }

    fn reset_delays(&mut self) {
        for delay in &mut self.wet_pads {
            delay.reset();
        }
        for delay in &mut self.dry_delays {
            delay.reset();
        }
    }
}

struct DelayLine {
    buffer: Vec<f32>,
    write: usize,
    count: u64,
}

impl DelayLine {
    fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0.0; capacity.max(1)],
            write: 0,
            count: 0,
        }
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write = 0;
        self.count = 0;
    }

    fn process(&mut self, input: f32, delay: usize) -> f32 {
        if delay == 0 {
            return input;
        }

        let capacity = self.buffer.len();
        let output = if self.count >= delay as u64 {
            let read = (self.write + capacity - delay % capacity) % capacity;
            self.buffer[read]
        } else {
            0.0
        };

        self.buffer[self.write] = input;
        self.write = (self.write + 1) % capacity;
        self.count += 1;

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SAMPLE_RATE: f32 = 48_000.0;

    fn render(transpose: f32, frames: usize) -> Vec<f32> {
        let mut engine = PitchEngine::new(SAMPLE_RATE, 1);
        engine.set_quality(Quality::High);
        let mut output = vec![0.0; frames];
        let mut position = 0;

        while position < frames {
            let count = (frames - position).min(engine.chunk_frames());
            let input = engine.input_buffer_mut();
            for (frame, sample) in input.iter_mut().take(count).enumerate() {
                let index = position + frame;
                *sample = (TAU * 440.0 * index as f32 / SAMPLE_RATE).sin();
            }
            engine.process_chunk(count, transpose, 1.0, false);
            output[position..position + count].copy_from_slice(&engine.wet()[..count]);
            position += count;
        }

        output
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
    fn reports_a_stable_latency() {
        let engine = PitchEngine::new(SAMPLE_RATE, 1);
        assert_eq!(engine.latency_samples(), 6720);
    }

    #[test]
    fn shifts_an_octave() {
        let output = render(12.0, 48_000);
        let latency = PitchEngine::new(SAMPLE_RATE, 1).latency_samples() as usize;
        let shifted = frequency(&output[latency + 12_000..]);
        assert!((shifted - 880.0).abs() < 10.0, "{shifted}");
    }

    #[test]
    fn unity_is_stable() {
        let output = render(0.0, 48_000);
        assert!(output.iter().all(|sample| sample.is_finite()));
    }
}
