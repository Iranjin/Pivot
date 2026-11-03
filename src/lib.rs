use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use nih_plug::prelude::*;

use crate::dsp::PitchShifter;
use crate::params::{KeyMorphParams, Quality};

mod dsp;
mod params;

const BYPASS_RAMP_SECONDS: f32 = 0.02;

struct KeyMorph {
    params: Arc<KeyMorphParams>,
    sample_rate: f32,
    shifter: Option<PitchShifter>,
    quality: Quality,
    pitch_semitones: f32,
    active_mix: f32,
}

impl Default for KeyMorph {
    fn default() -> Self {
        Self {
            params: Arc::new(KeyMorphParams::default()),
            sample_rate: 48_000.0,
            shifter: None,
            quality: Quality::Balanced,
            pitch_semitones: 0.0,
            active_mix: 1.0,
        }
    }
}

impl Plugin for KeyMorph {
    const NAME: &'static str = "KeyMorph";
    const VENDOR: &'static str = "KeyMorph";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        let channels = audio_io_layout
            .main_output_channels
            .map(|channels| channels.get() as usize)
            .unwrap_or(2);

        self.sample_rate = buffer_config.sample_rate;

        let mut shifter = PitchShifter::new(channels);
        self.quality = self.params.quality.value();
        shifter.set_quality(self.quality);
        context.set_latency_samples(shifter.latency_samples());
        self.shifter = Some(shifter);

        true
    }

    fn reset(&mut self) {
        if let Some(shifter) = &mut self.shifter {
            shifter.reset();
        }

        self.active_mix = if self.params.bypass.value() { 0.0 } else { 1.0 };
        self.pitch_semitones = self.params.transpose.value() as f32 + self.params.fine.value();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let Some(shifter) = self.shifter.as_mut() else {
            return ProcessStatus::Normal;
        };

        let quality = self.params.quality.value();
        if quality != self.quality {
            shifter.set_quality(quality);
            self.quality = quality;
            context.set_latency_samples(shifter.latency_samples());
        }

        let bypass_target = if self.params.bypass.value() { 0.0 } else { 1.0 };
        let bypass_step = 1.0 / (BYPASS_RAMP_SECONDS * self.sample_rate).max(1.0);
        let glide_samples = (self.params.glide.value() * 0.001 * self.sample_rate).max(0.0);
        let glide_coefficient = if glide_samples <= 1.0 {
            1.0
        } else {
            1.0 - (-1.0 / glide_samples).exp()
        };
        let transpose = self.params.transpose.value() as f32;

        for mut channel_samples in buffer.iter_samples() {
            let pitch_target = transpose + self.params.fine.smoothed.next();
            self.pitch_semitones += (pitch_target - self.pitch_semitones) * glide_coefficient;
            let ratio = (self.pitch_semitones / 12.0).exp2();

            let input_gain = self.params.input_gain.smoothed.next();
            let output_gain = self.params.output_gain.smoothed.next();
            let mix = self.params.mix.smoothed.next() * 0.01;

            if self.active_mix < bypass_target {
                self.active_mix = (self.active_mix + bypass_step).min(bypass_target);
            } else {
                self.active_mix = (self.active_mix - bypass_step).max(bypass_target);
            }

            let angle = mix * self.active_mix * FRAC_PI_2;
            let wet_gain = angle.sin();
            let dry_gain = angle.cos();

            for (channel, sample) in channel_samples.iter_mut().enumerate() {
                let input = *sample * input_gain;

                *sample = if channel < shifter.channel_count() {
                    let (wet, dry) = shifter.process_sample(channel, input, ratio);
                    (wet * wet_gain + dry * dry_gain) * output_gain
                } else {
                    input * output_gain
                };
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for KeyMorph {
    const CLAP_ID: &'static str = "com.keymorph.keymorph";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Real-time pitch and key shifting effect");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for KeyMorph {
    const VST3_CLASS_ID: [u8; 16] = *b"KeyMorphKeyMorph";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}

nih_export_clap!(KeyMorph);
nih_export_vst3!(KeyMorph);
