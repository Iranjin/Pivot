use nih_plug::prelude::*;

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Low,
    Balanced,
    High,
}

impl Quality {
    pub fn fft_size(self) -> usize {
        match self {
            Quality::Low => 1024,
            Quality::Balanced => 2048,
            Quality::High => 4096,
        }
    }
}

#[derive(Params)]
pub struct KeyMorphParams {
    #[id = "bypass"]
    pub bypass: BoolParam,
    #[id = "quality"]
    pub quality: EnumParam<Quality>,
    #[id = "transpose"]
    pub transpose: IntParam,
    #[id = "fine"]
    pub fine: FloatParam,
    #[id = "glide"]
    pub glide: FloatParam,
    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "input_gain"]
    pub input_gain: FloatParam,
    #[id = "output_gain"]
    pub output_gain: FloatParam,
}

impl Default for KeyMorphParams {
    fn default() -> Self {
        Self {
            bypass: BoolParam::new("Bypass", false),
            quality: EnumParam::new("Quality", Quality::Balanced),
            transpose: IntParam::new("Transpose", 0, IntRange::Linear { min: -24, max: 24 })
                .with_unit(" st"),
            fine: FloatParam::new(
                "Fine",
                0.0,
                FloatRange::Linear {
                    min: -100.0,
                    max: 100.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_step_size(1.0)
            .with_unit(" ct"),
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
