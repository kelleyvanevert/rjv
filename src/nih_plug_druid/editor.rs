// On Windows platform, don't show a console when opening the app.
#![windows_subsystem = "windows"]

use crossbeam::atomic::AtomicCell;
use druid::widget::prelude::*;
use druid::widget::{Flex, Label, TextBox};
use druid::{
    AppLauncher, Application, Data, ExtEventSink, Lens, UnitPoint, WidgetExt, WindowDesc,
    WindowHandle,
};
use nih_plug::prelude::{Editor, GuiContext, ParentWindowHandle};
use parking_lot::RwLock;
use std::marker::PhantomData;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::nih_plug_druid::DruidState;

const VERTICAL_WIDGET_SPACING: f64 = 20.0;
const TEXT_BOX_WIDTH: f64 = 200.0;

#[derive(Clone, Data, Lens)]
struct HelloState {
    name: String,
}

pub(crate) struct DruidEditor<T> {
    pub(crate) druid_state: Arc<DruidState>,
    /// The plugin's state. This is kept in between editor openenings.
    pub(crate) user_state: Arc<RwLock<T>>,

    // /// The user's build function. Applied once at the start of the application.
    // pub(crate) build: Arc<dyn Fn(&Context, &mut T) + 'static + Send + Sync>,
    // /// The user's update function.
    // pub(crate) update: Arc<dyn Fn(&Context, &ParamSetter, &mut T) + 'static + Send + Sync>,
    //
    /// The scaling factor reported by the host, if any. On macOS this will never be set and we
    /// should use the system scaling factor instead.
    pub(crate) scaling_factor: AtomicCell<Option<f32>>,
}

fn build_root_widget() -> impl Widget<HelloState> {
    // a label that will determine its text based on the current app data.
    let label = Label::new(|data: &HelloState, _env: &Env| {
        if data.name.is_empty() {
            "Hello anybody!?".to_string()
        } else {
            format!("Hello {}!", data.name)
        }
    })
    .with_text_size(32.0);

    // a textbox that modifies `name`.
    let textbox = TextBox::new()
        .with_placeholder("Who are we greeting?")
        .with_text_size(18.0)
        .fix_width(TEXT_BOX_WIDTH)
        .lens(HelloState::name);

    // arrange the two widgets vertically, with some padding
    Flex::column()
        .with_child(label)
        .with_spacer(VERTICAL_WIDGET_SPACING)
        .with_child(textbox)
        .align_vertical(UnitPoint::CENTER)
}

impl<T> Editor for DruidEditor<T>
where
    T: 'static + Send + Sync,
{
    fn spawn(
        &self,
        parent: ParentWindowHandle,
        context: Arc<dyn GuiContext>,
    ) -> Box<dyn std::any::Any + Send> {
        //         let app = Application::new().unwrap();
        //         let mut builder = WindowBuilder::new(app.clone());
        // builder.set_handler(Box::<AppState>::default());
        //     builder.set_title("Text editing example");

        // describe the main window
        let window = WindowDesc::new(build_root_widget())
            .title("Hello World!")
            .window_size((400.0, 400.0));

        // create the initial app state
        let initial_state: HelloState = HelloState {
            name: "World".into(),
        };

        // start the application. Here we pass in the application state.
        let launcher = AppLauncher::with_window(window).log_to_console();

        // let handler = launcher.get_external_handle();

        launcher
            .launch(initial_state)
            .expect("Failed to launch application");

        Box::new(DruidEditorHandle {
            druid_state: self.druid_state.clone(),
            // window_handle: window.,
        })

        // let build = self.build.clone();
        // let update = self.update.clone();
        // let state = self.user_state.clone();

        // let (unscaled_width, unscaled_height) = self.druid_state.size();
        // let scaling_factor = self.scaling_factor.load();
        // let window = EguiWindow::open_parented(
        //     &parent,
        //     WindowOpenOptions {
        //         title: String::from("egui window"),
        //         // Baseview should be doing the DPI scaling for us
        //         size: Size::new(unscaled_width as f64, unscaled_height as f64),
        //         // NOTE: For some reason passing 1.0 here causes the UI to be scaled on macOS but
        //         //       not the mouse events.
        //         scale: scaling_factor
        //             .map(|factor| WindowScalePolicy::ScaleFactor(factor as f64))
        //             .unwrap_or(WindowScalePolicy::SystemScaleFactor),

        //         #[cfg(feature = "opengl")]
        //         gl_config: Some(GlConfig {
        //             version: (3, 2),
        //             red_bits: 8,
        //             blue_bits: 8,
        //             green_bits: 8,
        //             alpha_bits: 8,
        //             depth_bits: 24,
        //             stencil_bits: 8,
        //             samples: None,
        //             srgb: true,
        //             double_buffer: true,
        //             vsync: true,
        //             ..Default::default()
        //         }),
        //     },
        //     state,
        //     move |egui_ctx, _queue, state| build(egui_ctx, &mut state.write()),
        //     move |egui_ctx, _queue, state| {
        //         let setter = ParamSetter::new(context.as_ref());

        //         // For now, just always redraw. Most plugin GUIs have meters, and those almost always
        //         // need a redraw. Later we can try to be a bit more sophisticated about this. Without
        //         // this we would also have a blank GUI when it gets first opened because most DAWs open
        //         // their GUI while the window is still unmapped.
        //         egui_ctx.request_repaint();
        //         (update)(egui_ctx, &setter, &mut state.write());
        //     },
        // );

        // self.druid_state.open.store(true, Ordering::Release);
        // Box::new(EguiEditorHandle {
        //     druid_state: self.druid_state.clone(),
        //     window,
        // })
    }

    fn size(&self) -> (u32, u32) {
        self.druid_state.size()
    }

    fn set_scale_factor(&self, factor: f32) -> bool {
        // If the editor is currently open then the host must not change the current HiDPI scale as
        // we don't have a way to handle that. Ableton Live does this.
        if self.druid_state.is_open() {
            return false;
        }

        self.scaling_factor.store(Some(factor));
        true
    }

    fn param_value_changed(&self, _id: &str, _normalized_value: f32) {
        // As mentioned above, for now we'll always force a redraw to allow meter widgets to work
        // correctly. In the future we can use an `Arc<AtomicBool>` and only force a redraw when
        // that boolean is set.
    }

    fn param_modulation_changed(&self, _id: &str, _modulation_offset: f32) {}

    fn param_values_changed(&self) {
        // Same
    }
}

struct DruidEditorHandle {
    druid_state: Arc<DruidState>,
    // window_handle: WindowHandle,
}

/// The window handle enum stored within 'WindowHandle' contains raw pointers. Is there a way around
/// having this requirement?
unsafe impl Send for DruidEditorHandle {}

impl Drop for DruidEditorHandle {
    fn drop(&mut self) {
        self.druid_state.open.store(false, Ordering::Release);

        // XXX: This should automatically happen when the handle gets dropped, but apparently not
        // self.window_handle.close()
        Application::global().quit()
    }
}
