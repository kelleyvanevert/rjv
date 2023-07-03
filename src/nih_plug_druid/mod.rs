use crossbeam::atomic::AtomicCell;
use nih_plug::params::persist::PersistentField;
use nih_plug::prelude::Editor;
use parking_lot::RwLock;
// use nih_plug::params::persist::PersistentField;
// use nih_plug::prelude::{Editor, ParamSetter};
// use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Re-export for convenience.
pub use druid;

mod editor;
// pub mod widgets;

pub fn create_druid_editor<T>(
    druid_state: Arc<DruidState>,
    user_state: T,
    // build: B,
    // update: U,
) -> Option<Box<dyn Editor>>
where
    T: 'static + Send + Sync,
    // B: Fn(&Context, &mut T) + 'static + Send + Sync,
    // U: Fn(&Context, &ParamSetter, &mut T) + 'static + Send + Sync,
{
    Some(Box::new(editor::DruidEditor {
        druid_state,
        user_state: Arc::new(RwLock::new(user_state)),
        // build: Arc::new(build),
        // update: Arc::new(update),

        // TODO: We can't get the size of the window when baseview does its own scaling, so if the
        //       host does not set a scale factor on Windows or Linux we should just use a factor of
        //       1. That may make the GUI tiny but it also prevents it from getting cut off.
        #[cfg(target_os = "macos")]
        scaling_factor: AtomicCell::new(None),
        #[cfg(not(target_os = "macos"))]
        scaling_factor: AtomicCell::new(Some(1.0)),
    }))
}

/// State for an `nih_plug_druid` editor.
#[derive(Debug, Serialize, Deserialize)]
pub struct DruidState {
    /// The window's size in logical pixels before applying `scale_factor`.
    #[serde(with = "nih_plug::params::persist::serialize_atomic_cell")]
    size: AtomicCell<(u32, u32)>,
    /// Whether the editor's window is currently open.
    #[serde(skip)]
    open: AtomicBool,
}

impl<'a> PersistentField<'a, DruidState> for Arc<DruidState> {
    fn set(&self, new_value: DruidState) {
        self.size.store(new_value.size.load());
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&DruidState) -> R,
    {
        f(self)
    }
}

impl DruidState {
    /// Initialize the GUI's state. This value can be passed to [`create_druid_editor()`]. The window
    /// size is in logical pixels, so before it is multiplied by the DPI scaling factor.
    pub fn from_size(width: u32, height: u32) -> Arc<DruidState> {
        Arc::new(DruidState {
            size: AtomicCell::new((width, height)),
            open: AtomicBool::new(false),
        })
    }

    /// Returns a `(width, height)` pair for the current size of the GUI in logical pixels.
    pub fn size(&self) -> (u32, u32) {
        self.size.load()
    }

    /// Whether the GUI is currently visible.
    // Called `is_open()` instead of `open()` to avoid the ambiguity.
    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }
}
