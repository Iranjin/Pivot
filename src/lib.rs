use std::f32::consts::{FRAC_PI_2, TAU};
use std::sync::Arc;

use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui};

use crate::dsp::{PitchAnalyzer, PitchEngine};
use crate::params::{LfoShape, ModTarget, PivotParams, Quality, Scale};
use crate::ui::pitch_knob;

mod dsp;
mod params;
mod ui;

const BYPASS_RAMP_SECONDS: f32 = 0.02;
struct Pivot {
    params: Arc<PivotParams>,
    sample_rate: f32,
    engine: Option<PitchEngine>,
    analyzer: Option<PitchAnalyzer>,
    quality: Quality,
    pitch_semitones: f32,
    snap_semitones: f32,
    active_mix: f32,
    lfo_phase: f32,
    random_state: u32,
    random_value: f32,
}

impl Default for Pivot {
    fn default() -> Self {
        Self {
            params: Arc::new(PivotParams::default()),
            sample_rate: 48_000.0,
            engine: None,
            analyzer: None,
            quality: Quality::High,
            pitch_semitones: 0.0,
            snap_semitones: 0.0,
            active_mix: 1.0,
            lfo_phase: 0.0,
            random_state: 0x1234_5678,
            random_value: 0.0,
        }
    }
}

impl Plugin for Pivot {
    const NAME: &'static str = "Pivot";
    const VENDOR: &'static str = "Iranjin";
    const URL: &'static str = "https://github.com/Iranjin/Pivot";
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

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let egui_state = self.params.editor_state.clone();

        create_egui_editor(
            egui_state.clone(),
            (),
            |ctx, _state| {
                let mut style = (*ctx.style()).clone();
                style.visuals = egui::Visuals::dark();
                style.visuals.panel_fill = egui::Color32::from_rgb(15, 16, 20);
                style.visuals.window_fill = egui::Color32::from_rgb(15, 16, 20);
                style.spacing.item_spacing = egui::vec2(10.0, 8.0);
                ctx.set_style(style);
            },
            move |ctx, setter, _state| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::Frame::NONE
                        .inner_margin(egui::Margin::same(20))
                        .show(ui, |ui| {
                            ui.heading(egui::RichText::new("Pivot").size(20.0));
                            ui.add_space(6.0);
                            pitch_panel(ui, &params, setter);
                        });
                });
            },
        )
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
        self.quality = self.params.quality.value();

        let mut engine = PitchEngine::new(buffer_config.sample_rate, channels);
        engine.set_quality(self.quality);
        let latency = engine.latency_samples();
        context.set_latency_samples(latency);
        self.engine = Some(engine);
        self.analyzer = Some(PitchAnalyzer::new(buffer_config.sample_rate));

        true
    }

    fn reset(&mut self) {
        if let Some(engine) = &mut self.engine {
            engine.reset();
        }
        if let Some(analyzer) = &mut self.analyzer {
            analyzer.reset();
        }

        let source_root = self.params.source_root.value().clamp(0, 11) as u8;
        let target_root = self.params.target_root.value().clamp(0, 11) as u8;
        self.pitch_semitones =
            self.params.pitch.value() + shortest_key_offset(source_root, target_root) as f32;
        self.snap_semitones = 0.0;
        self.active_mix = if self.params.bypass.value() { 0.0 } else { 1.0 };
        self.lfo_phase = 0.0;
        self.random_state = 0x1234_5678;
        self.random_value = 0.0;
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let Some(engine) = self.engine.as_mut() else {
            return ProcessStatus::Normal;
        };

        let quality = self.params.quality.value();
        if quality != self.quality {
            engine.set_quality(quality);
            self.quality = quality;
        }

        let channels = buffer.channels();
        let total_frames = buffer.samples();
        if channels == 0 || total_frames == 0 || channels != engine.channel_count() {
            return ProcessStatus::Normal;
        }

        let lfo_shape = self.params.lfo_shape.value();
        let lfo_rate = self.params.lfo_rate.value();
        let lfo_phase_offset = self.params.lfo_phase.value() / 360.0;
        let mod_target = self.params.mod_target.value();
        let limiter = self.params.limiter.value();
        let source_root = self.params.source_root.value().clamp(0, 11) as u8;
        let target_root = self.params.target_root.value().clamp(0, 11) as u8;
        let scale = self.params.scale.value();

        let bypass_target = if self.params.bypass.value() { 0.0 } else { 1.0 };
        let bypass_step = 1.0 / (BYPASS_RAMP_SECONDS * self.sample_rate).max(1.0);
        let glide_samples = (self.params.glide.value() * 0.001 * self.sample_rate).max(0.0);
        let glide_coefficient = if glide_samples <= 1.0 {
            1.0
        } else {
            1.0 - (-1.0 / glide_samples).exp()
        };
        let retune_ms = self.params.retune_speed.value();
        let retune_coefficient = if retune_ms <= 0.0 {
            1.0
        } else {
            let samples = (retune_ms * 0.001 * self.sample_rate).max(1.0);
            1.0 - (-1.0 / samples).exp()
        };

        let mut frame_position = 0;
        while frame_position < total_frames {
            let count = (total_frames - frame_position).min(engine.chunk_frames());

            {
                let channel_slices = buffer.as_slice();
                for frame in 0..count {
                    let mut mono = 0.0;
                    for channel_slice in channel_slices.iter().take(channels) {
                        mono += channel_slice[frame_position + frame];
                    }
                    mono /= channels as f32;

                    if let Some(analyzer) = &mut self.analyzer {
                        analyzer.process_sample(mono);
                    }
                }
            }

            let (f0, mono_confidence) = self
                .analyzer
                .as_ref()
                .map(|analyzer| (analyzer.f0_hz(), analyzer.monophonicity()))
                .unwrap_or((0.0, 0.0));
            let key_offset = shortest_key_offset(source_root, target_root) as f32;

            let lfo_depth = self.params.lfo_depth.value();
            let phase = (self.lfo_phase + lfo_phase_offset).fract();
            let wave = match lfo_shape {
                LfoShape::Sine => (TAU * phase).sin(),
                LfoShape::Triangle => {
                    if phase < 0.25 {
                        4.0 * phase
                    } else if phase < 0.75 {
                        2.0 - 4.0 * phase
                    } else {
                        4.0 * phase - 4.0
                    }
                }
                LfoShape::Square => {
                    if phase < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                LfoShape::Saw => 2.0 * phase - 1.0,
                LfoShape::Random => self.random_value,
            };
            let lfo = wave * lfo_depth;
            let lfo_pitch = if mod_target == ModTarget::Transpose {
                lfo
            } else {
                0.0
            };
            let lfo_mix = if mod_target == ModTarget::Mix {
                lfo / 12.0
            } else {
                0.0
            };

            self.lfo_phase += lfo_rate * count as f32 / self.sample_rate;
            if self.lfo_phase >= 1.0 {
                self.lfo_phase -= self.lfo_phase.floor();
                self.random_state = self
                    .random_state
                    .wrapping_mul(1_664_525)
                    .wrapping_add(1_013_904_223);
                self.random_value = ((self.random_state >> 8) as f32 / 16_777_216.0) * 2.0 - 1.0;
            }

            let transpose = self.params.pitch.value();
            let snap_amount = self.params.snap_amount.value() * 0.01;
            let snap_target = if snap_amount > 0.0 && mono_confidence > 0.5 && f0 > 0.0 {
                let note = 69.0 + 12.0 * (f0 / 440.0).log2();
                nearest_scale_delta(note + transpose + key_offset, target_root, scale) * snap_amount
            } else {
                0.0
            };
            let retune_step = 1.0 - (1.0 - retune_coefficient).powi(count as i32);
            self.snap_semitones += (snap_target - self.snap_semitones) * retune_step;

            let pitch_target =
                (transpose + key_offset + self.snap_semitones + lfo_pitch).clamp(-24.0, 24.0);
            let glide_step = 1.0 - (1.0 - glide_coefficient).powi(count as i32);
            self.pitch_semitones += (pitch_target - self.pitch_semitones) * glide_step;
            let transpose_semitones = self.pitch_semitones;

            let formant_shift = self.params.formant_shift.value();
            let preserve = self.params.formant_preserve.value() * 0.01;
            let formant_ratio = (formant_shift / 12.0).exp2();
            let pitch_ratio = (transpose_semitones / 12.0).exp2();
            let compensate = (preserve > 0.001 && (pitch_ratio - 1.0).abs() > 1.0e-4)
                || formant_shift.abs() > 0.001;
            let formant_multiplier = if compensate {
                pitch_ratio.powf(preserve) / formant_ratio
            } else {
                1.0
            };

            {
                let input = engine.input_buffer_mut();
                let channel_slices = buffer.as_slice();
                for frame in 0..count {
                    let input_gain = self.params.input_gain.smoothed.next();
                    for (channel, channel_slice) in channel_slices.iter().enumerate().take(channels)
                    {
                        let sample = channel_slice[frame_position + frame] * input_gain;
                        input[frame * channels + channel] = sample;
                    }
                }
            }

            engine.process_chunk(count, transpose_semitones, formant_multiplier, compensate);

            {
                let channel_slices = buffer.as_slice();
                for frame in 0..count {
                    let mix = (self.params.mix.smoothed.next() * 0.01 + lfo_mix).clamp(0.0, 1.0);
                    let output_gain = self.params.output_gain.smoothed.next();

                    if self.active_mix < bypass_target {
                        self.active_mix = (self.active_mix + bypass_step).min(bypass_target);
                    } else {
                        self.active_mix = (self.active_mix - bypass_step).max(bypass_target);
                    }

                    let angle = mix * self.active_mix * FRAC_PI_2;
                    let wet_gain = angle.sin();
                    let dry_gain = angle.cos();

                    for (channel, channel_slice) in
                        channel_slices.iter_mut().enumerate().take(channels)
                    {
                        let index = frame * channels + channel;
                        let wet = engine.wet()[index];
                        let dry = engine.dry()[index];
                        let mut output = (wet * wet_gain + dry * dry_gain) * output_gain;
                        if limiter {
                            output = output.clamp(-0.966, 0.966);
                        }
                        channel_slice[frame_position + frame] = output;
                    }
                }
            }

            frame_position += count;
        }

        ProcessStatus::Normal
    }
}

fn muted() -> egui::Color32 {
    egui::Color32::from_rgb(140, 146, 160)
}

fn pitch_panel(ui: &mut egui::Ui, params: &PivotParams, setter: &ParamSetter) {
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new("PITCH").small().color(muted()));
        ui.add_space(2.0);

        let mut pitch = params.pitch.value();
        let response = pitch_knob(ui, &mut pitch);
        if response.changed() {
            set_param(setter, &params.pitch, pitch);
        }

        ui.add_space(2.0);
    });
}

impl ClapPlugin for Pivot {
    const CLAP_ID: &'static str = "com.iranjin.pivot";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Real-time pitch and key shifting effect");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::PitchShifter,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for Pivot {
    const VST3_CLASS_ID: [u8; 16] = *b"PivotPivotPivot0";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::PitchShift];
}

nih_export_clap!(Pivot);
nih_export_vst3!(Pivot);

fn shortest_key_offset(source: u8, target: u8) -> i32 {
    let difference = (target as i32 - source as i32).rem_euclid(12);
    if difference > 6 {
        difference - 12
    } else {
        difference
    }
}

fn nearest_scale_delta(note: f32, root: u8, scale: Scale) -> f32 {
    let mask = scale.mask();
    let relative = note - root as f32;
    let center = relative.round() as i32;
    let mut best = 0.0;
    let mut best_distance = f32::INFINITY;

    for candidate in (center - 1)..=(center + 1) {
        let degree = candidate.rem_euclid(12) as usize;
        if mask[degree] {
            let candidate_note = candidate as f32;
            let distance = (relative - candidate_note).abs();
            if distance < best_distance {
                best_distance = distance;
                best = candidate_note - relative;
            }
        }
    }

    best
}

fn set_param<P: Param>(setter: &ParamSetter, param: &P, value: P::Plain) {
    setter.begin_set_parameter(param);
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_offsets_take_shortest_path() {
        assert_eq!(shortest_key_offset(9, 0), 3);
        assert_eq!(shortest_key_offset(0, 11), -1);
        assert_eq!(shortest_key_offset(0, 0), 0);
    }

    #[test]
    fn scale_snap_targets_nearest_degree() {
        assert!((nearest_scale_delta(60.2, 0, Scale::Major) + 0.2).abs() < 1.0e-5);
        assert!((nearest_scale_delta(61.2, 0, Scale::Major) - 0.8).abs() < 1.0e-5);
    }
}
