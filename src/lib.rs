use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use nih_plug::prelude::*;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, Vec2},
    resizable_window::ResizableWindow,
    widgets,
};

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

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let egui_state = self.params.editor_state.clone();

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
                    .min_size(Vec2::new(520.0, 440.0))
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
                                        .with_width(260.0),
                                );
                                ui.end_row();

                                ui.label("Fine");
                                ui.add(
                                    widgets::ParamSlider::for_param(&params.fine, setter)
                                        .with_width(260.0),
                                );
                                ui.end_row();

                                ui.label("Glide");
                                ui.add(
                                    widgets::ParamSlider::for_param(&params.glide, setter)
                                        .with_width(260.0),
                                );
                                ui.end_row();

                                ui.label("Formant");
                                ui.add(
                                    widgets::ParamSlider::for_param(&params.formant_shift, setter)
                                        .with_width(260.0),
                                );
                                ui.end_row();

                                ui.label("Preserve");
                                ui.add(
                                    widgets::ParamSlider::for_param(
                                        &params.formant_preserve,
                                        setter,
                                    )
                                    .with_width(260.0),
                                );
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
                                        .with_width(260.0),
                                );
                                ui.end_row();

                                ui.label("Input");
                                ui.add(
                                    widgets::ParamSlider::for_param(&params.input_gain, setter)
                                        .with_width(260.0),
                                );
                                ui.end_row();

                                ui.label("Output");
                                ui.add(
                                    widgets::ParamSlider::for_param(&params.output_gain, setter)
                                        .with_width(260.0),
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
