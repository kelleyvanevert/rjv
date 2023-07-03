use atomic_float::AtomicF32;
use code_editor::code_editor;
use js_sandbox::Script;
use nih_plug::prelude::*;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, epaint::Shadow, Color32, FontData, FontDefinitions},
    EguiState,
};
use std::sync::{Arc, Mutex};

mod code_editor;

// This is a shortened version of the gain example with most comments removed, check out
// https://github.com/robbert-vdh/nih-plug/blob/master/plugins/examples/gain/src/lib.rs to get
// started

/// The time it takes for the peak meter to decay by 12 dB after switching to complete silence.
const PEAK_METER_DECAY_MS: f64 = 150.0;

pub struct Rjv {
    params: Arc<RjvParams>,
    sample_rate: f32,

    time_s: f32,

    /// Needed to normalize the peak meter's response based on the sample rate.
    peak_meter_decay_weight: f32,
    /// The current data for the peak meter. This is stored as an [`Arc`] so we can share it between
    /// the GUI and the audio processing parts. If you have more state to share, then it's a good
    /// idea to put all of that in a struct behind a single `Arc`.
    ///
    /// This is stored as voltage gain.
    peak_meter: Arc<AtomicF32>,

    display: Arc<Mutex<String>>,
}

struct UIState {
    preset: i32,
    code: String,
}

#[derive(Params)]
struct RjvParams {
    /// The editor state, saved together with the parameter state so the custom scaling can be
    /// restored.
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    /// The parameter's ID is used to identify the parameter in the wrappred plugin API. As long as
    /// these IDs remain constant, you can rename and reorder these fields as you wish. The
    /// parameters are exposed to the host in the same order they were defined. In this case, this
    /// gain parameter is stored as linear gain while the values are displayed in decibels.
    #[id = "gain"]
    pub gain: FloatParam,

    #[id = "preset"]
    pub preset: IntParam,

    #[id = "code_1"]
    pub code_1: StringParam,

    #[id = "code_2"]
    pub code_2: StringParam,

    #[id = "code_3"]
    pub code_3: StringParam,

    #[id = "code_4"]
    pub code_4: StringParam,

    #[id = "code_5"]
    pub code_5: StringParam,

    #[id = "code_6"]
    pub code_6: StringParam,
}

impl RjvParams {
    fn code(&self) -> &StringParam {
        if self.preset.value() == 1 {
            &self.code_1
        } else if self.preset.value() == 2 {
            &self.code_2
        } else if self.preset.value() == 3 {
            &self.code_3
        } else if self.preset.value() == 4 {
            &self.code_4
        } else if self.preset.value() == 5 {
            &self.code_5
        } else {
            &self.code_6
        }
    }
}

impl Default for Rjv {
    fn default() -> Self {
        Self {
            params: Arc::new(RjvParams::default()),
            sample_rate: 1.0,

            time_s: 0.0,

            peak_meter_decay_weight: 1.0,
            peak_meter: Arc::new(AtomicF32::new(util::MINUS_INFINITY_DB)),

            display: Arc::new(Mutex::new("hi".to_string())),
        }
    }
}

impl Default for RjvParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(800, 600),

            // This gain is stored as linear gain. NIH-plug comes with useful conversion functions
            // to treat these kinds of parameters as if we were dealing with decibels. Storing this
            // as decibels is easier to work with, but requires a conversion for every sample.
            gain: FloatParam::new(
                "Gain",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-30.0),
                    max: util::db_to_gain(30.0),
                    // This makes the range appear as if it was linear when displaying the values as
                    // decibels
                    factor: FloatRange::gain_skew_factor(-30.0, 30.0),
                },
            )
            // Because the gain parameter is stored as linear gain instead of storing the value as
            // decibels, we need logarithmic smoothing
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            // There are many predefined formatters we can use here. If the gain was stored as
            // decibels instead of as a linear gain value, we could have also used the
            // `.with_step_size(0.1)` function to get internal rounding.
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            preset: IntParam::new("Preset", 1, IntRange::Linear { min: 1, max: 6 }),

            code_1: StringParam::new("Code 1", "fn bla() { 5 + \"hello\" }".to_string()),
            code_2: StringParam::new("Code 2", "20".to_string()),
            code_3: StringParam::new("Code 3", "30".to_string()),
            code_4: StringParam::new("Code 4", "40".to_string()),
            code_5: StringParam::new("Code 5", "50".to_string()),
            code_6: StringParam::new("Code 6", "60".to_string()),
        }
    }
}

impl Plugin for Rjv {
    const NAME: &'static str = "Rjv";
    const VENDOR: &'static str = "Kelley van Evert";
    const URL: &'static str = env!("CARGO_PKG_HOMEPAGE");
    const EMAIL: &'static str = "hello@klve.nl";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // The first audio IO layout is used as the default. The other layouts may be selected either
    // explicitly or automatically by the host or the user depending on the plugin API/backend.
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

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        // let peak_meter = self.peak_meter.clone();
        let display = self.display.clone();

        create_egui_editor(
            self.params.editor_state.clone(),
            UIState {
                preset: params.preset.value(),
                code: params.code().value(),
            },
            |egui_ctx, _| {
                let mut fonts = FontDefinitions::default();

                fonts.font_data.insert(
                    "Fira Code Regular".to_owned(),
                    FontData::from_static(include_bytes!("./fonts/FiraCode-Regular.ttf")),
                );
                fonts.font_data.insert(
                    "Fira Code Medium".to_owned(),
                    FontData::from_static(include_bytes!("./fonts/FiraCode-Medium.ttf")),
                );
                fonts.font_data.insert(
                    "Fira Code Bold".to_owned(),
                    FontData::from_static(include_bytes!("./fonts/FiraCode-Bold.ttf")),
                );

                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .insert(0, "Fira Code Medium".to_owned());

                fonts
                    .families
                    .entry(egui::FontFamily::Name("Fira Code Regular".into()))
                    .or_default()
                    .insert(0, "Fira Code Regular".to_owned());

                fonts
                    .families
                    .entry(egui::FontFamily::Name("Fira Code Medium".into()))
                    .or_default()
                    .insert(0, "Fira Code Medium".to_owned());

                fonts
                    .families
                    .entry(egui::FontFamily::Name("Fira Code Bold".into()))
                    .or_default()
                    .insert(0, "Fira Code Bold".to_owned());

                // egui::TextStyle::Name("footing".into())
                egui_ctx.set_fonts(fonts);
            },
            move |egui_ctx, _setter, state| {
                if state.preset != params.preset.value() {
                    state.code = params.code().value();
                }

                egui::CentralPanel::default()
                    .frame(egui::containers::Frame {
                        outer_margin: egui::style::Margin::same(0.),
                        inner_margin: egui::style::Margin::same(20.),
                        rounding: egui::Rounding::same(0.),
                        shadow: Shadow::big_light(),
                        fill: Color32::WHITE,
                        stroke: egui::Stroke::new(0., Color32::WHITE),
                    })
                    .show(egui_ctx, |ui| {
                        ui.heading("JS code");

                        if code_editor(ui, &mut state.code, ui.available_width()).changed() {
                            params.code().set_value(state.code.clone());
                        }

                        ui.horizontal(|ui| {
                            if ui
                                .selectable_value(&mut state.preset, 1, "Preset 1")
                                .clicked()
                            {
                                params.preset.set_value(1);
                                state.code = params.code().value();
                            }

                            if ui
                                .selectable_value(&mut state.preset, 2, "Preset 2")
                                .clicked()
                            {
                                params.preset.set_value(2);
                                state.code = params.code().value();
                            }

                            if ui
                                .selectable_value(&mut state.preset, 3, "Preset 3")
                                .clicked()
                            {
                                params.preset.set_value(3);
                                state.code = params.code().value();
                            }

                            if ui
                                .selectable_value(&mut state.preset, 4, "Preset 4")
                                .clicked()
                            {
                                params.preset.set_value(4);
                                state.code = params.code().value();
                            }

                            if ui
                                .selectable_value(&mut state.preset, 5, "Preset 5")
                                .clicked()
                            {
                                params.preset.set_value(5);
                                state.code = params.code().value();
                            }

                            if ui
                                .selectable_value(&mut state.preset, 6, "Preset 6")
                                .clicked()
                            {
                                params.preset.set_value(6);
                                state.code = params.code().value();
                            }
                        });

                        ui.add_space(12.0);
                        ui.label("Yeeaah...! Let's go and evaluate some JS code :)");
                        ui.add_space(8.0);
                        ui.label(display.as_ref().lock().unwrap().clone());
                    });
            },
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        // After `PEAK_METER_DECAY_MS` milliseconds of pure silence, the peak meter's value should
        // have dropped by 12 dB
        self.peak_meter_decay_weight = 0.25f64
            .powf((buffer_config.sample_rate as f64 * PEAK_METER_DECAY_MS / 1000.0).recip())
            as f32;

        self.sample_rate = buffer_config.sample_rate;

        // Resize buffers and perform other potentially expensive initialization operations here.
        // The `reset()` function is always called right after this function. You can remove this
        // function if you do not need it.
        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
    }

    fn process(
        &mut self,
        buffer: &mut Buffer, // 1s
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        *self.display.lock().unwrap() = format!("code <{}>", self.params.code().value());

        let js_code = format!(
            "function gain(t) {{ return {}; }}",
            self.params.code().value()
        );
        // let js_code = "function bla([t, g]) { return Math.sin(t) * g; }";
        let mut script = Script::from_string(&js_code).ok();

        for (sample_id, channel_samples) in buffer.iter_samples().enumerate() {
            let time = self.time_s + ((sample_id as f32) / self.sample_rate);

            let mut amplitude = 0.0;
            let num_samples = channel_samples.len();

            // Smoothing is optionally built into the parameters themselves
            // let gain = self.params.gain.smoothed.next();
            let gain_processed: Option<f32> =
                script.as_mut().and_then(|s| s.call("gain", &time).ok());
            // let gain_processed: f32 = script.call("gain", &time).expect("JS runs");

            for sample in channel_samples {
                *sample *= gain_processed.unwrap_or(0.0);
                amplitude += *sample;
            }

            // To save resources, a plugin can (and probably should!) only perform expensive
            // calculations that are only displayed on the GUI while the GUI is open
            if self.params.editor_state.is_open() {
                amplitude = (amplitude / num_samples as f32).abs();
                let current_peak_meter = self.peak_meter.load(std::sync::atomic::Ordering::Relaxed);
                let new_peak_meter = if amplitude > current_peak_meter {
                    amplitude
                } else {
                    current_peak_meter * self.peak_meter_decay_weight
                        + amplitude * (1.0 - self.peak_meter_decay_weight)
                };

                self.peak_meter
                    .store(new_peak_meter, std::sync::atomic::Ordering::Relaxed)
            }
        }

        self.time_s += (buffer.samples() as f32) / self.sample_rate;

        ProcessStatus::Normal
    }
}

impl ClapPlugin for Rjv {
    const CLAP_ID: &'static str = "nl.klve.rjv";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Rust, JS, VST");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for Rjv {
    const VST3_CLASS_ID: [u8; 16] = *b"rjv_klve_1234567";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}

nih_export_clap!(Rjv);
nih_export_vst3!(Rjv);
