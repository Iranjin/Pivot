use std::f32::consts::{FRAC_PI_2, TAU};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use nih_plug::prelude::*;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, Vec2},
    resizable_window::ResizableWindow,
    widgets,
};
use serde::{Deserialize, Serialize};

use crate::dsp::{KeyDetector, PitchEngine};
use crate::params::{KeyMode, KeyMorphParams, LfoShape, ModTarget, Quality, SCALES, Scale};
use crate::ui::pitch_knob;

mod dsp;
mod params;
mod ui;

const BYPASS_RAMP_SECONDS: f32 = 0.02;
const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

#[derive(Clone, Copy, Serialize, Deserialize)]
struct Snapshot {
    version: u32,
    bypass: bool,
    quality: Quality,
    transpose: i32,
    fine: f32,
    glide: f32,
    formant_shift: f32,
    formant_preserve: f32,
    lfo_shape: LfoShape,
    lfo_rate: f32,
    lfo_depth: f32,
    lfo_phase: f32,
    mod_target: ModTarget,
    source_auto: bool,
    source_root: i32,
    source_mode: KeyMode,
    target_root: i32,
    target_mode: KeyMode,
    snap_amount: f32,
    retune_speed: f32,
    scale: Scale,
    mix: f32,
    input_gain: f32,
    output_gain: f32,
    limiter: bool,
}

#[derive(Default)]
struct EditorState {
    slot_a: Option<Snapshot>,
    slot_b: Option<Snapshot>,
    undo: Vec<Snapshot>,
    message: String,
}

struct KeyMorph {
    params: Arc<KeyMorphParams>,
    sample_rate: f32,
    engine: Option<PitchEngine>,
    key_detector: Option<KeyDetector>,
    detected_key: Arc<AtomicU32>,
    latency_samples: Arc<AtomicU32>,
    quality: Quality,
    pitch_semitones: f32,
    snap_semitones: f32,
    active_mix: f32,
    lfo_phase: f32,
    random_state: u32,
    random_value: f32,
}

impl Default for KeyMorph {
    fn default() -> Self {
        Self {
            params: Arc::new(KeyMorphParams::default()),
            sample_rate: 48_000.0,
            engine: None,
            key_detector: None,
            detected_key: Arc::new(AtomicU32::new(pack_key(0, KeyMode::Minor, 0.0))),
            latency_samples: Arc::new(AtomicU32::new(0)),
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
        let latency = self.latency_samples.clone();

        create_egui_editor(
            egui_state.clone(),
            EditorState::default(),
            |ctx, _state| {
                let mut style = (*ctx.style()).clone();
                style.visuals = egui::Visuals::dark();
                style.visuals.panel_fill = egui::Color32::from_rgb(15, 16, 20);
                style.visuals.window_fill = egui::Color32::from_rgb(15, 16, 20);
                style.spacing.item_spacing = egui::vec2(10.0, 8.0);
                ctx.set_style(style);
            },
            move |ctx, setter, state| {
                ResizableWindow::new("keymorph-ui")
                    .min_size(Vec2::new(700.0, 460.0))
                    .show(ctx, egui_state.as_ref(), |ui| {
                        ui.horizontal(|ui| {
                            ui.heading(egui::RichText::new("KeyMorph").size(20.0));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let mut bypassed = params.bypass.value();
                                    if ui.checkbox(&mut bypassed, "Bypass").changed() {
                                        set_param(setter, &params.bypass, bypassed);
                                    }
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "Latency: {} samples",
                                            latency.load(Ordering::Relaxed)
                                        ))
                                        .small()
                                        .color(muted()),
                                    );
                                },
                            );
                        });

                        ui.horizontal(|ui| {
                            if ui.button("Save Preset").clicked() {
                                save_preset(&params, &mut state.message);
                            }
                            if ui.button("Load Preset").clicked()
                                && let Some(snapshot) = load_preset(&mut state.message)
                            {
                                push_undo(state, &params);
                                snapshot.apply(&params, setter);
                            }
                            ui.label(&state.message);
                        });
                        ui.separator();

                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let panel_height = ui.available_height().clamp(320.0, 380.0);
                                ui.horizontal_top(|ui| {
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(150.0, panel_height),
                                        egui::Layout::top_down(egui::Align::Min),
                                        |ui| key_panel(ui, &params, setter, &detected),
                                    );
                                    ui.separator();
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(360.0, panel_height),
                                        egui::Layout::top_down(egui::Align::Center),
                                        |ui| pitch_panel(ui, &params, setter),
                                    );
                                    ui.separator();
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(160.0, panel_height),
                                        egui::Layout::top_down(egui::Align::Min),
                                        |ui| quick_panel(ui, &params, setter),
                                    );
                                });

                                ui.add_space(4.0);
                                ui.separator();
                                ui.collapsing("Advanced", |ui| {
                                    ui.columns(3, |columns| {
                                        pitch_extras(&mut columns[0], &params, setter);
                                        columns[0].separator();
                                        formant_advanced(&mut columns[0], &params, setter);
                                        modulation_section(&mut columns[1], &params, setter);
                                        output_section(&mut columns[2], &params, setter);
                                        columns[2].separator();
                                        ab_section(&mut columns[2], &params, setter, state);
                                    });
                                });
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
        self.latency_samples.store(latency, Ordering::Relaxed);
        context.set_latency_samples(latency);
        self.engine = Some(engine);
        self.key_detector = Some(KeyDetector::new(buffer_config.sample_rate));
        self.detected_key
            .store(pack_key(0, KeyMode::Minor, 0.0), Ordering::Relaxed);

        true
    }

    fn reset(&mut self) {
        if let Some(engine) = &mut self.engine {
            engine.reset();
        }
        if let Some(detector) = &mut self.key_detector {
            detector.reset();
        }

        let source_root = self.params.source_root.value().clamp(0, 11) as u8;
        let target_root = self.params.target_root.value().clamp(0, 11) as u8;
        self.pitch_semitones = self.params.transpose.value() as f32
            + cents_to_semitones(self.params.fine.value())
            + shortest_key_offset(source_root, target_root) as f32;
        self.snap_semitones = 0.0;
        self.active_mix = if self.params.bypass.value() { 0.0 } else { 1.0 };
        self.lfo_phase = 0.0;
        self.random_state = 0x1234_5678;
        self.random_value = 0.0;
        self.detected_key
            .store(pack_key(0, KeyMode::Minor, 0.0), Ordering::Relaxed);
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
        let source_auto = self.params.source_auto.value();
        let source_root_manual = self.params.source_root.value().clamp(0, 11) as u8;
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

                    if let Some(detector) = &mut self.key_detector
                        && detector.process_sample(mono)
                    {
                        let updated = detector.estimate();
                        self.detected_key.store(
                            pack_key(updated.root, updated.mode, updated.confidence),
                            Ordering::Relaxed,
                        );
                    }
                }
            }

            let (f0, mono_confidence) = self
                .key_detector
                .as_ref()
                .map(|detector| (detector.f0_hz(), detector.monophonicity()))
                .unwrap_or((0.0, 0.0));
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

            let transpose = self.params.transpose.value() as f32;
            let fine = cents_to_semitones(self.params.fine.value());
            let snap_amount = self.params.snap_amount.value() * 0.01;
            let snap_target = if snap_amount > 0.0 && mono_confidence > 0.5 && f0 > 0.0 {
                let note = 69.0 + 12.0 * (f0 / 440.0).log2();
                nearest_scale_delta(note + transpose + fine + key_offset, target_root, scale)
                    * snap_amount
            } else {
                0.0
            };
            let retune_step = 1.0 - (1.0 - retune_coefficient).powi(count as i32);
            self.snap_semitones += (snap_target - self.snap_semitones) * retune_step;

            let pitch_target = (transpose + fine + key_offset + self.snap_semitones + lfo_pitch)
                .clamp(-24.0, 24.0);
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

fn accent() -> egui::Color32 {
    egui::Color32::from_rgb(96, 206, 245)
}

fn muted() -> egui::Color32 {
    egui::Color32::from_rgb(140, 146, 160)
}

fn key_panel(
    ui: &mut egui::Ui,
    params: &KeyMorphParams,
    setter: &ParamSetter,
    detected: &AtomicU32,
) {
    ui.heading("KEY");
    ui.add_space(2.0);

    let packed = detected.load(Ordering::Relaxed);
    let detected_root = (packed & 0xF) as usize;
    let detected_minor = (packed >> 4) & 1 == 1;
    let detected_confidence = (packed >> 8) & 0x7F;

    ui.label(egui::RichText::new("Detected").small().color(muted()));
    if detected_confidence == 0 {
        ui.label(egui::RichText::new("—").strong());
    } else {
        ui.label(
            egui::RichText::new(format!(
                "{} {} ({}%)",
                NOTE_NAMES[detected_root.min(11)],
                if detected_minor { "minor" } else { "major" },
                detected_confidence
            ))
            .strong(),
        );
    }
    ui.add_space(4.0);

    let mut source_auto = params.source_auto.value();
    if ui.checkbox(&mut source_auto, "Auto").changed() {
        set_param(setter, &params.source_auto, source_auto);
    }

    ui.label(egui::RichText::new("Source").small().color(muted()));
    ui.horizontal(|ui| {
        note_combo(ui, "source-root", &params.source_root, setter);
        mode_combo(ui, "source-mode", &params.source_mode, setter);
    });

    ui.label(egui::RichText::new("Target").small().color(muted()));
    ui.horizontal(|ui| {
        note_combo(ui, "target-root", &params.target_root, setter);
        mode_combo(ui, "target-mode", &params.target_mode, setter);
    });

    let source_root = if params.source_auto.value() && detected_confidence > 50 {
        detected_root as u8
    } else {
        params.source_root.value().clamp(0, 11) as u8
    };
    let target_root = params.target_root.value().clamp(0, 11) as u8;
    let offset = shortest_key_offset(source_root, target_root);
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(format!("Key shift: {offset:+} st"))
            .color(accent())
            .strong(),
    );
}

fn pitch_panel(ui: &mut egui::Ui, params: &KeyMorphParams, setter: &ParamSetter) {
    ui.label(egui::RichText::new("PITCH").small().color(muted()));
    ui.add_space(2.0);

    let mut pitch = params.transpose.value() as f32 + cents_to_semitones(params.fine.value());
    let response = pitch_knob(ui, &mut pitch);
    if response.changed() {
        let semitones = pitch.trunc();
        let cents = (pitch - semitones) * 100.0;
        set_param(setter, &params.transpose, semitones as i32);
        set_param(setter, &params.fine, cents);
    }

    ui.add_space(2.0);
    ui.label(
        egui::RichText::new("Drag · Shift = cents · Double-click = reset")
            .small()
            .color(muted()),
    );
}

fn quick_panel(ui: &mut egui::Ui, params: &KeyMorphParams, setter: &ParamSetter) {
    ui.heading("QUICK");
    egui::Grid::new("quick-grid")
        .num_columns(1)
        .spacing([0.0, 6.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Scale Snap").small().color(muted()));
            ui.add(widgets::ParamSlider::for_param(&params.snap_amount, setter).with_width(130.0))
                .on_hover_text("Scale snap amount");

            ui.label(egui::RichText::new("Mix").small().color(muted()));
            ui.add(widgets::ParamSlider::for_param(&params.mix, setter).with_width(130.0));

            ui.label(egui::RichText::new("Glide").small().color(muted()));
            ui.add(widgets::ParamSlider::for_param(&params.glide, setter).with_width(130.0));

            ui.label(egui::RichText::new("Formant").small().color(muted()));
            ui.add(
                widgets::ParamSlider::for_param(&params.formant_preserve, setter).with_width(130.0),
            );
        });
}

fn pitch_extras(ui: &mut egui::Ui, params: &KeyMorphParams, setter: &ParamSetter) {
    ui.heading("PITCH");
    egui::Grid::new("pitch-extra-grid")
        .num_columns(2)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            ui.label("Fine");
            ui.add(widgets::ParamSlider::for_param(&params.fine, setter).with_width(130.0));
            ui.end_row();
        });
}

fn formant_advanced(ui: &mut egui::Ui, params: &KeyMorphParams, setter: &ParamSetter) {
    ui.heading("FORMANT / SCALE");
    egui::Grid::new("formant-grid")
        .num_columns(2)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            ui.label("Shift");
            ui.add(
                widgets::ParamSlider::for_param(&params.formant_shift, setter).with_width(130.0),
            );
            ui.end_row();

            ui.label("Retune");
            ui.add(widgets::ParamSlider::for_param(&params.retune_speed, setter).with_width(130.0));
            ui.end_row();

            ui.label("Scale");
            scale_combo(ui, "scale", &params.scale, setter);
            ui.end_row();
        });
}

fn modulation_section(ui: &mut egui::Ui, params: &KeyMorphParams, setter: &ParamSetter) {
    ui.heading("MODULATION");
    egui::Grid::new("modulation-grid")
        .num_columns(2)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            ui.label("Shape");
            lfo_shape_combo(ui, "lfo-shape", &params.lfo_shape, setter);
            ui.end_row();

            ui.label("Rate");
            ui.add(widgets::ParamSlider::for_param(&params.lfo_rate, setter).with_width(130.0));
            ui.end_row();

            ui.label("Depth");
            ui.add(widgets::ParamSlider::for_param(&params.lfo_depth, setter).with_width(130.0));
            ui.end_row();

            ui.label("Phase");
            ui.add(widgets::ParamSlider::for_param(&params.lfo_phase, setter).with_width(130.0));
            ui.end_row();

            ui.label("Target");
            mod_target_combo(ui, "lfo-target", &params.mod_target, setter);
            ui.end_row();
        });
}

fn output_section(ui: &mut egui::Ui, params: &KeyMorphParams, setter: &ParamSetter) {
    ui.heading("OUTPUT");
    egui::Grid::new("output-grid")
        .num_columns(2)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            ui.label("Quality");
            quality_combo(ui, "quality", &params.quality, setter);
            ui.end_row();

            ui.label("Input");
            ui.add(widgets::ParamSlider::for_param(&params.input_gain, setter).with_width(130.0));
            ui.end_row();

            ui.label("Output");
            ui.add(widgets::ParamSlider::for_param(&params.output_gain, setter).with_width(130.0));
            ui.end_row();

            ui.label("Limiter");
            let mut limiter = params.limiter.value();
            if ui.checkbox(&mut limiter, "").changed() {
                set_param(setter, &params.limiter, limiter);
            }
            ui.end_row();
        });
}

fn ab_section(
    ui: &mut egui::Ui,
    params: &KeyMorphParams,
    setter: &ParamSetter,
    state: &mut EditorState,
) {
    ui.heading("A/B");
    ui.horizontal_wrapped(|ui| {
        if ui.button("Store A").clicked() {
            state.slot_a = Some(Snapshot::capture(params));
            state.message = String::from("Stored A");
        }
        if ui.button("Recall A").clicked()
            && let Some(snapshot) = state.slot_a
        {
            push_undo(state, params);
            snapshot.apply(params, setter);
            state.message = String::from("Recalled A");
        }
        if ui.button("Store B").clicked() {
            state.slot_b = Some(Snapshot::capture(params));
            state.message = String::from("Stored B");
        }
        if ui.button("Recall B").clicked()
            && let Some(snapshot) = state.slot_b
        {
            push_undo(state, params);
            snapshot.apply(params, setter);
            state.message = String::from("Recalled B");
        }
        if ui.button("Undo").clicked()
            && let Some(snapshot) = state.undo.pop()
        {
            snapshot.apply(params, setter);
            state.message = String::from("Undo");
        }
    });
}

fn quality_combo(ui: &mut egui::Ui, id: &str, param: &EnumParam<Quality>, setter: &ParamSetter) {
    let current = param.value();
    egui::ComboBox::from_id_salt(id)
        .selected_text(format!("{current:?}"))
        .width(110.0)
        .show_ui(ui, |ui| {
            for value in [Quality::Low, Quality::Balanced, Quality::High] {
                if ui
                    .selectable_label(current == value, format!("{value:?}"))
                    .clicked()
                {
                    set_param(setter, param, value);
                }
            }
        });
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

fn lfo_shape_combo(ui: &mut egui::Ui, id: &str, param: &EnumParam<LfoShape>, setter: &ParamSetter) {
    let current = param.value();
    egui::ComboBox::from_id_salt(id)
        .selected_text(current.name())
        .width(120.0)
        .show_ui(ui, |ui| {
            for value in [
                LfoShape::Sine,
                LfoShape::Triangle,
                LfoShape::Square,
                LfoShape::Saw,
                LfoShape::Random,
            ] {
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

fn mod_target_combo(
    ui: &mut egui::Ui,
    id: &str,
    param: &EnumParam<ModTarget>,
    setter: &ParamSetter,
) {
    let current = param.value();
    egui::ComboBox::from_id_salt(id)
        .selected_text(current.name())
        .width(120.0)
        .show_ui(ui, |ui| {
            for value in [ModTarget::Transpose, ModTarget::Mix] {
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

fn cents_to_semitones(cents: f32) -> f32 {
    cents * 0.01
}

impl Snapshot {
    fn capture(params: &KeyMorphParams) -> Self {
        Self {
            version: 1,
            bypass: params.bypass.value(),
            quality: params.quality.value(),
            transpose: params.transpose.value(),
            fine: params.fine.value(),
            glide: params.glide.value(),
            formant_shift: params.formant_shift.value(),
            formant_preserve: params.formant_preserve.value(),
            lfo_shape: params.lfo_shape.value(),
            lfo_rate: params.lfo_rate.value(),
            lfo_depth: params.lfo_depth.value(),
            lfo_phase: params.lfo_phase.value(),
            mod_target: params.mod_target.value(),
            source_auto: params.source_auto.value(),
            source_root: params.source_root.value(),
            source_mode: params.source_mode.value(),
            target_root: params.target_root.value(),
            target_mode: params.target_mode.value(),
            snap_amount: params.snap_amount.value(),
            retune_speed: params.retune_speed.value(),
            scale: params.scale.value(),
            mix: params.mix.value(),
            input_gain: params.input_gain.value(),
            output_gain: params.output_gain.value(),
            limiter: params.limiter.value(),
        }
    }

    fn apply(&self, params: &KeyMorphParams, setter: &ParamSetter) {
        set_param(setter, &params.bypass, self.bypass);
        set_param(setter, &params.quality, self.quality);
        set_param(setter, &params.transpose, self.transpose);
        set_param(setter, &params.fine, self.fine);
        set_param(setter, &params.glide, self.glide);
        set_param(setter, &params.formant_shift, self.formant_shift);
        set_param(setter, &params.formant_preserve, self.formant_preserve);
        set_param(setter, &params.lfo_shape, self.lfo_shape);
        set_param(setter, &params.lfo_rate, self.lfo_rate);
        set_param(setter, &params.lfo_depth, self.lfo_depth);
        set_param(setter, &params.lfo_phase, self.lfo_phase);
        set_param(setter, &params.mod_target, self.mod_target);
        set_param(setter, &params.source_auto, self.source_auto);
        set_param(setter, &params.source_root, self.source_root);
        set_param(setter, &params.source_mode, self.source_mode);
        set_param(setter, &params.target_root, self.target_root);
        set_param(setter, &params.target_mode, self.target_mode);
        set_param(setter, &params.snap_amount, self.snap_amount);
        set_param(setter, &params.retune_speed, self.retune_speed);
        set_param(setter, &params.scale, self.scale);
        set_param(setter, &params.mix, self.mix);
        set_param(setter, &params.input_gain, self.input_gain);
        set_param(setter, &params.output_gain, self.output_gain);
        set_param(setter, &params.limiter, self.limiter);
    }
}

fn set_param<P: Param>(setter: &ParamSetter, param: &P, value: P::Plain) {
    setter.begin_set_parameter(param);
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}

fn push_undo(state: &mut EditorState, params: &KeyMorphParams) {
    if state.undo.len() >= 100 {
        state.undo.remove(0);
    }
    state.undo.push(Snapshot::capture(params));
}

fn save_preset(params: &KeyMorphParams, message: &mut String) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("KeyMorph preset", &["json"])
        .set_file_name("KeyMorph.json")
        .save_file()
    else {
        return;
    };

    let preset = Snapshot::capture(params);
    let result = serde_json::to_string_pretty(&preset)
        .map_err(|error| error.to_string())
        .and_then(|json| std::fs::write(&path, json).map_err(|error| error.to_string()));

    *message = match result {
        Ok(()) => format!("Saved {}", path.display()),
        Err(error) => format!("Save failed: {error}"),
    };
}

fn load_preset(message: &mut String) -> Option<Snapshot> {
    let path = rfd::FileDialog::new()
        .add_filter("KeyMorph preset", &["json"])
        .pick_file()?;

    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            *message = format!("Load failed: {error}");
            return None;
        }
    };

    match serde_json::from_str::<Snapshot>(&text) {
        Ok(snapshot) if snapshot.version == 1 => {
            *message = format!("Loaded {}", path.display());
            Some(snapshot)
        }
        Ok(_) => {
            *message = String::from("Unsupported preset version");
            None
        }
        Err(error) => {
            *message = format!("Load failed: {error}");
            None
        }
    }
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
    fn snapshot_round_trips_through_json() {
        let params = KeyMorphParams::default();
        let snapshot = Snapshot::capture(&params);
        let json = serde_json::to_string(&snapshot).unwrap();
        let decoded: Snapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.quality, snapshot.quality);
        assert_eq!(decoded.scale, snapshot.scale);
        assert_eq!(decoded.source_mode, snapshot.source_mode);
    }

    #[test]
    fn converts_cents_to_semitones() {
        assert!((cents_to_semitones(50.0) - 0.5).abs() < 1.0e-6);
        assert!((cents_to_semitones(-100.0) + 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn packed_key_round_trips() {
        let packed = pack_key(9, KeyMode::Minor, 0.87);
        assert_eq!(packed & 0xF, 9);
        assert_eq!((packed >> 4) & 1, 1);
        assert_eq!((packed >> 8) & 0x7F, 87);
    }
}
