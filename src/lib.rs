use nih_plug::prelude::*;
use std::sync::Arc;

const BYPASS_RAMP_SECONDS: f32 = 0.02;

struct KeyMorph {
    params: Arc<KeyMorphParams>,
    sample_rate: f32,
    active_mix: f32,
}

#[derive(Params)]
struct KeyMorphParams {
    #[id = "bypass"]
    pub bypass: BoolParam,
    #[id = "input_gain"]
    pub input_gain: FloatParam,
    #[id = "output_gain"]
    pub output_gain: FloatParam,
}

impl Default for KeyMorph {
    fn default() -> Self {
        Self {
            params: Arc::new(KeyMorphParams::default()),
            sample_rate: 48_000.0,
            active_mix: 1.0,
        }
    }
}

impl Default for KeyMorphParams {
    fn default() -> Self {
        Self {
            bypass: BoolParam::new("Bypass", false),
            input_gain: gain_param("Input"),
            output_gain: gain_param("Output"),
        }
    }
}

fn gain_param(name: &'static str) -> FloatParam {
    FloatParam::new(
        name,
        util::db_to_gain(0.0),
        FloatRange::Skewed {
            min: util::db_to_gain(-24.0),
            max: util::db_to_gain(24.0),
            factor: FloatRange::gain_skew_factor(-24.0, 24.0),
        },
    )
    .with_smoother(SmoothingStyle::Logarithmic(20.0))
    .with_unit(" dB")
    .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
    .with_string_to_value(formatters::s2v_f32_gain_to_db())
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
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        true
    }

    fn reset(&mut self) {
        self.active_mix = if self.params.bypass.value() { 0.0 } else { 1.0 };
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let target = if self.params.bypass.value() { 0.0 } else { 1.0 };
        let step = 1.0 / (BYPASS_RAMP_SECONDS * self.sample_rate).max(1.0);

        for channel_samples in buffer.iter_samples() {
            let gain =
                self.params.input_gain.smoothed.next() * self.params.output_gain.smoothed.next();

            if self.active_mix < target {
                self.active_mix = (self.active_mix + step).min(target);
            } else {
                self.active_mix = (self.active_mix - step).max(target);
            }

            for sample in channel_samples {
                let dry = *sample;
                *sample = dry * (1.0 - self.active_mix) + dry * gain * self.active_mix;
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
