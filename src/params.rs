use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Low,
    Balanced,
    High,
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyMode {
    #[default]
    Major,
    Minor,
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scale {
    #[default]
    Major,
    NaturalMinor,
    HarmonicMinor,
    MelodicMinor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
    MajorPentatonic,
    MinorPentatonic,
    Blues,
    Chromatic,
}

impl Scale {
    pub fn mask(self) -> [bool; 12] {
        match self {
            Scale::Major => [
                true, false, true, false, true, true, false, true, false, true, false, true,
            ],
            Scale::NaturalMinor => [
                true, false, true, true, false, true, false, true, true, false, true, false,
            ],
            Scale::HarmonicMinor => [
                true, false, true, true, false, true, false, true, false, true, true, false,
            ],
            Scale::MelodicMinor => [
                true, false, true, true, false, true, false, true, false, true, false, true,
            ],
            Scale::Dorian => [
                true, false, true, true, false, true, false, true, false, true, true, false,
            ],
            Scale::Phrygian => [
                true, true, false, true, false, true, false, true, true, false, true, false,
            ],
            Scale::Lydian => [
                true, false, true, false, true, false, true, true, false, true, false, true,
            ],
            Scale::Mixolydian => [
                true, false, true, false, true, true, false, true, false, true, true, false,
            ],
            Scale::Locrian => [
                true, true, false, true, false, true, true, false, true, false, true, false,
            ],
            Scale::MajorPentatonic => [
                true, false, true, false, true, false, false, true, false, true, false, false,
            ],
            Scale::MinorPentatonic => [
                true, false, false, true, false, true, false, true, false, false, true, false,
            ],
            Scale::Blues => [
                true, false, false, true, false, true, true, true, false, false, true, false,
            ],
            Scale::Chromatic => [true; 12],
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LfoShape {
    #[default]
    Sine,
    Triangle,
    Square,
    Saw,
    Random,
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModTarget {
    #[default]
    Transpose,
    Mix,
}

#[derive(Params)]
pub struct PivotParams {
    #[persist = "editor-state-v9"]
    pub editor_state: Arc<EguiState>,
    #[id = "bypass"]
    pub bypass: BoolParam,
    #[id = "quality"]
    pub quality: EnumParam<Quality>,
    #[id = "pitch"]
    pub pitch: FloatParam,
    #[id = "glide"]
    pub glide: FloatParam,
    #[id = "formant_shift"]
    pub formant_shift: FloatParam,
    #[id = "formant_preserve"]
    pub formant_preserve: FloatParam,
    #[id = "lfo_shape"]
    pub lfo_shape: EnumParam<LfoShape>,
    #[id = "lfo_rate"]
    pub lfo_rate: FloatParam,
    #[id = "lfo_depth"]
    pub lfo_depth: FloatParam,
    #[id = "lfo_phase"]
    pub lfo_phase: FloatParam,
    #[id = "mod_target"]
    pub mod_target: EnumParam<ModTarget>,
    #[id = "source_root"]
    pub source_root: IntParam,
    #[id = "source_mode"]
    pub source_mode: EnumParam<KeyMode>,
    #[id = "target_root"]
    pub target_root: IntParam,
    #[id = "target_mode"]
    pub target_mode: EnumParam<KeyMode>,
    #[id = "snap_amount"]
    pub snap_amount: FloatParam,
    #[id = "retune_speed"]
    pub retune_speed: FloatParam,
    #[id = "scale"]
    pub scale: EnumParam<Scale>,
    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "input_gain"]
    pub input_gain: FloatParam,
    #[id = "output_gain"]
    pub output_gain: FloatParam,
    #[id = "limiter"]
    pub limiter: BoolParam,
}

impl Default for PivotParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(460, 520),
            bypass: BoolParam::new("Bypass", false),
            quality: EnumParam::new("Quality", Quality::High),
            pitch: FloatParam::new(
                "Pitch",
                0.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 24.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_step_size(0.01)
            .with_unit(" st")
            .with_value_to_string(Arc::new(pitch_formatter))
            .with_string_to_value(Arc::new(parse_float)),
            glide: FloatParam::new(
                "Glide",
                20.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 2000.0,
                },
            )
            .with_step_size(1.0)
            .with_unit(" ms"),
            formant_shift: FloatParam::new(
                "Formant",
                0.0,
                FloatRange::Linear {
                    min: -12.0,
                    max: 12.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_step_size(1.0)
            .with_unit(" st")
            .with_value_to_string(stepped_formatter(" st"))
            .with_string_to_value(Arc::new(parse_float)),
            formant_preserve: FloatParam::new(
                "Preserve",
                100.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 100.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_step_size(1.0)
            .with_unit(" %"),
            lfo_shape: EnumParam::new("LFO Shape", LfoShape::Sine),
            lfo_rate: FloatParam::new(
                "LFO Rate",
                0.5,
                FloatRange::Linear {
                    min: 0.01,
                    max: 20.0,
                },
            )
            .with_step_size(0.01)
            .with_unit(" Hz"),
            lfo_depth: FloatParam::new(
                "LFO Depth",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 12.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_step_size(0.1)
            .with_unit(" st"),
            lfo_phase: FloatParam::new(
                "LFO Phase",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 360.0,
                },
            )
            .with_step_size(1.0)
            .with_unit(" °"),
            mod_target: EnumParam::new("LFO Target", ModTarget::Transpose),
            source_root: IntParam::new("Source Root", 0, IntRange::Linear { min: 0, max: 11 }),
            source_mode: EnumParam::new("Source Mode", KeyMode::Minor),
            target_root: IntParam::new("Target Root", 0, IntRange::Linear { min: 0, max: 11 }),
            target_mode: EnumParam::new("Target Mode", KeyMode::Minor),
            snap_amount: FloatParam::new(
                "Snap",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 100.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_step_size(1.0)
            .with_unit(" %"),
            retune_speed: FloatParam::new(
                "Retune",
                30.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 200.0,
                },
            )
            .with_step_size(1.0)
            .with_unit(" ms"),
            scale: EnumParam::new("Scale", Scale::Major),
            mix: FloatParam::new(
                "Mix",
                100.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 100.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_step_size(0.1)
            .with_unit(" %"),
            input_gain: gain_param("Input"),
            output_gain: gain_param("Output"),
            limiter: BoolParam::new("Limiter", true),
        }
    }
}

fn stepped_formatter(unit: &'static str) -> Arc<dyn Fn(f32) -> String + Send + Sync> {
    Arc::new(move |value| {
        let rounded = if value.abs() < 0.5 {
            0.0
        } else {
            value.round()
        };
        format!("{rounded:.0}{unit}")
    })
}

fn parse_float(text: &str) -> Option<f32> {
    text.split_whitespace().next()?.parse().ok()
}

fn pitch_formatter(value: f32) -> String {
    format!("{value:+.2}")
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
