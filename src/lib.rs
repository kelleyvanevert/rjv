use atomic_float::AtomicF32;
use js_sandbox::Script;
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets, EguiState};
use std::sync::{Arc, Mutex};

mod nih_plug_druid;

// This is a shortened version of the gain example with most comments removed, check out
// https://github.com/robbert-vdh/nih-plug/blob/master/plugins/examples/gain/src/lib.rs to get
// started

/// The time it takes for the peak meter to decay by 12 dB after switching to complete silence.
const PEAK_METER_DECAY_MS: f64 = 150.0;

struct Rjv {
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
    code: String,
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            code: "Math.sin(t) * 20".to_string(),
        }
    }
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

    #[id = "code"]
    pub code: StringParam,
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
            editor_state: EguiState::from_size(400, 300),

            // This gain is stored as linear gain. NIH-plug comes with useful conversion functions
            // to treat these kinds of parameters as if we were dealing with decibels. Storing this
            // as decibels is easier to work with, but requires a conversion for every sample.
            gain: FloatParam::new(
                "Blabla",
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

            code: StringParam::new("Kelleyy", "12.3".to_string()),
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
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),

        aux_input_ports: &[],
        aux_output_ports: &[],

        // Individual ports and the layout as a whole can be named here. By default these names
        // are generated as needed. This layout will be called 'Stereo', while a layout with
        // only one input and output channel would be called 'Mono'.
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    // If the plugin can send or receive SysEx messages, it can define a type to wrap around those
    // messages here. The type implements the `SysExMessage` trait, which allows conversion to and
    // from plain byte buffers.
    type SysExMessage = ();
    // More advanced plugins can use this to run expensive background tasks. See the field's
    // documentation for more information. `()` means that the plugin does not have any background
    // tasks.
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let peak_meter = self.peak_meter.clone();
        let display = self.display.clone();

        create_egui_editor(
            self.params.editor_state.clone(),
            UIState::default(),
            |_, _| {},
            move |egui_ctx, setter, state| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    // NOTE: See `plugins/diopser/src/editor.rs` for an example using the generic UI widget

                    // // This is a fancy widget that can get all the information it needs to properly
                    // // display and modify the parameter from the parametr itself
                    // // It's not yet fully implemented, as the text is missing.
                    // ui.label("Some random integer");
                    // ui.add(widgets::ParamSlider::for_param(&params.some_int, setter));

                    ui.label("Java");
                    ui.add(widgets::ParamSlider::for_param(&params.gain, setter));

                    ui.heading("JS code");

                    ui.label(
                        "Also gain, but with a lame widget. Can't even render the value correctly!",
                    );
                    // This is a simple naieve version of a parameter slider that's not aware of how
                    // the parameters work
                    ui.add(
                        egui::widgets::Slider::from_get_set(-30.0..=30.0, |new_value| {
                            match new_value {
                                Some(new_value_db) => {
                                    let new_value = util::gain_to_db(new_value_db as f32);

                                    setter.begin_set_parameter(&params.gain);
                                    setter.set_parameter(&params.gain, new_value);
                                    setter.end_set_parameter(&params.gain);

                                    new_value_db
                                }
                                None => util::gain_to_db(params.gain.value()) as f64,
                            }
                        })
                        .suffix(" dB"),
                    );

                    ui.label("JavaScript code");
                    ui.label(display.as_ref().lock().unwrap().clone());
                    let resp =
                        ui.add(egui::widgets::TextEdit::multiline(&mut state.code).code_editor());

                    if resp.lost_focus() {
                        params.code.set_value(state.code.clone());
                        // setter.begin_set_parameter(&params.code);
                        // setter.set_parameter(&params.code, state.code.clone());
                        // setter.end_set_parameter(&params.code);

                        // self.code = state.code.clone(); // WHAT
                    }

                    // TODO: Add a proper custom widget instead of reusing a progress bar
                    let peak_meter =
                        util::gain_to_db(peak_meter.load(std::sync::atomic::Ordering::Relaxed));
                    let peak_meter_text = if peak_meter > util::MINUS_INFINITY_DB {
                        format!("{peak_meter:.1} dBFS")
                    } else {
                        String::from("-inf dBFS")
                    };

                    let peak_meter_normalized = (peak_meter + 60.0) / 60.0;
                    ui.allocate_space(egui::Vec2::splat(2.0));
                    ui.add(
                        egui::widgets::ProgressBar::new(peak_meter_normalized)
                            .text(peak_meter_text),
                    );
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
        *self.display.lock().unwrap() = format!("code <{}>", self.params.code.value());

        let js_code = format!(
            "function gain(t) {{ return {}; }}",
            self.params.code.value()
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

    // Don't forget to change these features
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Stereo];
}

impl Vst3Plugin for Rjv {
    const VST3_CLASS_ID: [u8; 16] = *b"rjv_klve_1234567";

    // And also don't forget to change these categories
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Dynamics];
}

nih_export_clap!(Rjv);
nih_export_vst3!(Rjv);
