use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use nih_plug::prelude::*;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, Vec2},
    resizable_window::ResizableWindow,
    widgets,
};

use crate::dsp::{KeyDetector, PitchShifter};
use crate::params::{KeyMode, KeyMorphParams, Quality, SCALES, Scale};

mod dsp;
mod params;

const BYPASS_RAMP_SECONDS: f32 = 0.02;
const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

struct KeyMorph {
    params: Arc<KeyMorphParams>,
    sample_rate: f32,
    shifter: Option<PitchShifter>,
    key_detector: Option<KeyDetector>,
    detected_key: Arc<AtomicU32>,
    quality: Quality,
    pitch_semitones: f32,
    snap_semitones: f32,
    active_mix: f32,
}

impl Default for KeyMorph {
    fn default() -> Self {
        Self {
            params: Arc::new(KeyMorphParams::default()),
            sample_rate: 48_000.0,
            shifter: None,
            key_detector: None,
            detected_key: Arc::new(AtomicU32::new(pack_key(0, KeyMode::Minor, 0.0))),
            quality: Quality::Balanced,
            pitch_semitones: 0.0,
            snap_semitones: 0.0,
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

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let egui_state = self.params.editor_state.clone();
        let detected = self.detected_key.clone();

        create_egui_editor(
            egui_state.clone(),
            (),
            |ctx, _state| {
                let mut style = (*ctx.style()).clone();
                style.visuals = egui::Visuals::dark();
                style.visuals.panel_fill = egui::Color32::from_rgb(18, 18, 22);
                style.visuals.window_fill = egui::Color32::from_rgb(18, 18, 22);
                style.spacing.item_spacing = egui::vec2(10.0, 8.0);
                ctx.set_style(style);
            },
            move |ctx, setter, _state| {
                ResizableWindow::new("keymorph-ui")
                    .min_size(Vec2::new(560.0, 620.0))
                    .show(ctx, egui_state.as_ref(), |ui| {
                        ui.heading(egui::RichText::new("KeyMorph").size(22.0));
                        ui.label("Real-time pitch and key shifting");
                        ui.add_space(4.0);
                        ui.separator();

                        let mut bypassed = params.bypass.value();
                        if ui.checkbox(&mut bypassed, "Bypass").changed() {
                            setter.begin_set_parameter(&params.bypass);
                            setter.set_parameter(&params.bypass, bypassed);
                            setter.end_set_parameter(&params.bypass);
                        }

                        ui.add_space(4.0);

                        egui::Grid::new("keymorph-params")
                            .num_columns(2)
                            .spacing([12.0, 8.0])
                            .show(ui, |ui| {
                                ui.label("Transpose");
                                ui.add(
                                    widgets::ParamSlider::for_param(&params.transpose, setter)
                                        .with_width(240.0),
                                );
                                ui.end_row();

                                ui.label("Fine");
                                ui.add(
                                    widgets::ParamSlider::for_param(&params.fine, setter)
                                        .with_width(240.0),
                                );
                                ui.end_row();

                                ui.label("Glide");
                                ui.add(
                                    widgets::ParamSlider::for_param(&params.glide, setter)
                                        .with_width(240.0),
                                );
                                ui.end_row();

                                ui.label("Formant");
                                ui.add(
                                    widgets::ParamSlider::for_param(&params.formant_shift, setter)
                                        .with_width(240.0),
                                );
                                ui.end_row();

                                ui.label("Preserve");
                                ui.add(
                                    widgets::ParamSlider::for_param(
                                        &params.formant_preserve,
                                        setter,
                                    )
                                    .with_width(240.0),
                                );
                                ui.end_row();

                                ui.label("Detected");
                                let packed = detected.load(Ordering::Relaxed);
                                let detected_root = (packed & 0xF) as usize;
                                let detected_minor = (packed >> 4) & 1 == 1;
                                let detected_confidence = (packed >> 8) & 0x7F;
                                let detected_text = if detected_confidence == 0 {
                                    String::from("Detected: —")
                                } else {
                                    format!(
                                        "{} {} ({}%)",
                                        NOTE_NAMES[detected_root.min(11)],
                                        if detected_minor { "minor" } else { "major" },
                                        detected_confidence
                                    )
                                };
                                ui.label(detected_text);
                                ui.end_row();

                                ui.label("Source Auto");
                                let mut source_auto = params.source_auto.value();
                                if ui.checkbox(&mut source_auto, "").changed() {
                                    setter.begin_set_parameter(&params.source_auto);
                                    setter.set_parameter(&params.source_auto, source_auto);
                                    setter.end_set_parameter(&params.source_auto);
                                }
                                ui.end_row();

                                ui.label("Source Key");
                                ui.horizontal(|ui| {
                                    note_combo(ui, "source-root", &params.source_root, setter);
                                    mode_combo(ui, "source-mode", &params.source_mode, setter);
                                });
                                ui.end_row();

                                ui.label("Target Key");
                                ui.horizontal(|ui| {
                                    note_combo(ui, "target-root", &params.target_root, setter);
                                    mode_combo(ui, "target-mode", &params.target_mode, setter);
                                });
                                ui.end_row();

                                ui.label("Snap");
                                ui.add(
                                    widgets::ParamSlider::for_param(&params.snap_amount, setter)
                                        .with_width(240.0),
                                );
                                ui.end_row();

                                ui.label("Retune");
                                ui.add(
                                    widgets::ParamSlider::for_param(&params.retune_speed, setter)
                                        .with_width(240.0),
                                );
                                ui.end_row();

                                ui.label("Scale");
                                scale_combo(ui, "scale", &params.scale, setter);
                                ui.end_row();

                                ui.label("Quality");
                                let current = params.quality.value();
                                egui::ComboBox::from_id_salt("quality")
                                    .selected_text(format!("{current:?}"))
                                    .show_ui(ui, |ui| {
                                        for value in
                                            [Quality::Low, Quality::Balanced, Quality::High]
                                        {
                                            if ui
                                                .selectable_label(
                                                    current == value,
                                                    format!("{value:?}"),
                                                )
                                                .clicked()
                                            {
                                                setter.begin_set_parameter(&params.quality);
                                                setter.set_parameter(&params.quality, value);
                                                setter.end_set_parameter(&params.quality);
                                            }
                                        }
                                    });
                                ui.end_row();

                                ui.label("Mix");
                                ui.add(
                                    widgets::ParamSlider::for_param(&params.mix, setter)
                                        .with_width(240.0),
                                );
                                ui.end_row();

                                ui.label("Input");
                                ui.add(
                                    widgets::ParamSlider::for_param(&params.input_gain, setter)
                                        .with_width(240.0),
                                );
                                ui.end_row();

                                ui.label("Output");
                                ui.add(
                                    widgets::ParamSlider::for_param(&params.output_gain, setter)
                                        .with_width(240.0),
                                );
                                ui.end_row();
                            });

                        ui.add_space(4.0);
                        ui.separator();
                        ui.label(format!(
                            "Latency: {} samples",
                            params.quality.value().fft_size()
                        ));
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

        let mut shifter = PitchShifter::new(channels, buffer_config.sample_rate);
        self.quality = self.params.quality.value();
        shifter.set_quality(self.quality);
        context.set_latency_samples(shifter.latency_samples());
        self.shifter = Some(shifter);
        self.key_detector = Some(KeyDetector::new(buffer_config.sample_rate));
        self.detected_key
            .store(pack_key(0, KeyMode::Minor, 0.0), Ordering::Relaxed);

        true
    }

    fn reset(&mut self) {
        if let Some(shifter) = &mut self.shifter {
            shifter.reset();
        }
        if let Some(detector) = &mut self.key_detector {
            detector.reset();
        }

        let source_root = self.params.source_root.value().clamp(0, 11) as u8;
        let target_root = self.params.target_root.value().clamp(0, 11) as u8;
        self.pitch_semitones = self.params.transpose.value() as f32
            + self.params.fine.value()
            + shortest_key_offset(source_root, target_root) as f32;
        self.snap_semitones = 0.0;
        self.active_mix = if self.params.bypass.value() { 0.0 } else { 1.0 };
        self.detected_key
            .store(pack_key(0, KeyMode::Minor, 0.0), Ordering::Relaxed);
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

        let source_auto = self.params.source_auto.value();
        let source_root_manual = self.params.source_root.value().clamp(0, 11) as u8;
        let target_root = self.params.target_root.value().clamp(0, 11) as u8;
        let scale = self.params.scale.value();
        let retune_ms = self.params.retune_speed.value();
        let retune_coefficient = if retune_ms <= 0.0 {
            1.0
        } else {
            let samples = (retune_ms * 0.001 * self.sample_rate).max(1.0);
            1.0 - (-1.0 / samples).exp()
        };

        let estimate = self
            .key_detector
            .as_ref()
            .map(|detector| detector.estimate())
            .unwrap_or_default();
        let source_root = if source_auto && estimate.confidence > 0.5 {
            estimate.root
        } else {
            source_root_manual
        };
        let key_offset = shortest_key_offset(source_root, target_root) as f32;

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
            let mono = {
                let mut sum = 0.0;
                for sample in channel_samples.iter_mut() {
                    sum += *sample;
                }
                sum / channel_samples.len() as f32
            };
            if let Some(detector) = &mut self.key_detector
                && detector.process_sample(mono)
            {
                let updated = detector.estimate();
                self.detected_key.store(
                    pack_key(updated.root, updated.mode, updated.confidence),
                    Ordering::Relaxed,
                );
            }

            let snap_amount = self.params.snap_amount.smoothed.next() * 0.01;
            let fine = self.params.fine.smoothed.next();
            let (f0, mono_confidence) = shifter.analysis();
            let snap_target = if snap_amount > 0.0 && mono_confidence > 0.5 && f0 > 0.0 {
                let note = 69.0 + 12.0 * (f0 / 440.0).log2();
                nearest_scale_delta(note + transpose + fine + key_offset, target_root, scale)
                    * snap_amount
            } else {
                0.0
            };
            self.snap_semitones += (snap_target - self.snap_semitones) * retune_coefficient;

            let pitch_target = transpose + fine + key_offset + self.snap_semitones;
            self.pitch_semitones += (pitch_target - self.pitch_semitones) * glide_coefficient;
            let ratio = (self.pitch_semitones / 12.0).exp2();

            let formant_shift = self.params.formant_shift.smoothed.next();
            let formant_preserve = self.params.formant_preserve.smoothed.next() * 0.01;
            let formant_ratio = (formant_shift / 12.0).exp2();
            let envelope_warp = formant_ratio * ratio.powf(-formant_preserve);

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
                    let (wet, dry) = shifter.process_sample(channel, input, ratio, envelope_warp);
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
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Fx];
}

nih_export_clap!(KeyMorph);
nih_export_vst3!(KeyMorph);

fn note_combo(ui: &mut egui::Ui, id: &str, param: &IntParam, setter: &ParamSetter) {
    let current = param.value().clamp(0, 11) as usize;
    egui::ComboBox::from_id_salt(id)
        .selected_text(NOTE_NAMES[current])
        .width(56.0)
        .show_ui(ui, |ui| {
            for (index, name) in NOTE_NAMES.iter().enumerate() {
                if ui.selectable_label(current == index, *name).clicked() {
                    setter.begin_set_parameter(param);
                    setter.set_parameter(param, index as i32);
                    setter.end_set_parameter(param);
                }
            }
        });
}

fn mode_combo(ui: &mut egui::Ui, id: &str, param: &EnumParam<KeyMode>, setter: &ParamSetter) {
    let current = param.value();
    egui::ComboBox::from_id_salt(id)
        .selected_text(current.name())
        .width(72.0)
        .show_ui(ui, |ui| {
            for value in [KeyMode::Major, KeyMode::Minor] {
                if ui
                    .selectable_label(current == value, value.name())
                    .clicked()
                {
                    setter.begin_set_parameter(param);
                    setter.set_parameter(param, value);
                    setter.end_set_parameter(param);
                }
            }
        });
}

fn scale_combo(ui: &mut egui::Ui, id: &str, param: &EnumParam<Scale>, setter: &ParamSetter) {
    let current = param.value();
    egui::ComboBox::from_id_salt(id)
        .selected_text(current.name())
        .width(150.0)
        .show_ui(ui, |ui| {
            for value in SCALES {
                if ui
                    .selectable_label(current == value, value.name())
                    .clicked()
                {
                    setter.begin_set_parameter(param);
                    setter.set_parameter(param, value);
                    setter.end_set_parameter(param);
                }
            }
        });
}

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

fn pack_key(root: u8, mode: KeyMode, confidence: f32) -> u32 {
    let confidence = (confidence * 100.0).round().clamp(0.0, 100.0) as u32;
    (root as u32 & 0xF) | (((mode == KeyMode::Minor) as u32) << 4) | (confidence << 8)
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

    #[test]
    fn packed_key_round_trips() {
        let packed = pack_key(9, KeyMode::Minor, 0.87);
        assert_eq!(packed & 0xF, 9);
        assert_eq!((packed >> 4) & 1, 1);
        assert_eq!((packed >> 8) & 0x7F, 87);
    }
}
