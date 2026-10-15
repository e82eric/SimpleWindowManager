//! The picker framework. A picker is a `&'static PickerConfig`, opened with
//! `Action::ShowPicker(picker)`; the host, UI and worker know only these traits,
//! never the picker kinds, which live in their own modules (see `builtin_pickers`).
use crate::commands::Action;
use nfm_egui::{Appearance, Placement, egui, shared::SharedPicker};
use std::fmt;

/// A picker definition. Pickers live for the whole process: built-ins are statics
/// and `Configuration::register_picker` leaks the ones a configuration builds.
#[derive(Debug)]
pub struct PickerConfig {
    /// Shown in the commands picker and logs.
    pub name: &'static str,
    pub source: &'static dyn PickerSource,
}
impl PickerConfig {
    /// Leaks `source`; register the result with `Configuration::register_picker`.
    pub fn new(name: &'static str, source: impl PickerSource + 'static) -> Self {
        Self {
            name,
            source: Box::leak(Box::new(source)),
        }
    }
}

/// What a configured picker lists. Runs on the UI thread each time it opens and
/// captures whatever it needs from the configuration.
pub trait PickerSource: fmt::Debug + Send + Sync {
    fn request(&self, configuration: &crate::Configuration) -> Box<dyn PickerRequest>;
}

/// Captured data, sent to the host thread to build the control.
pub trait PickerRequest: Send {
    fn build(self: Box<Self>, host: &PickerHost<'_>) -> Result<Box<dyn PickerView>, String>;
}

/// A picker open in the host.
pub trait PickerView {
    fn picker(&self) -> &SharedPicker;
    /// Whether the source shows a column header row (structured sources do).
    /// NFM only reports it once results arrive, so the window size needs it upfront.
    fn has_header(&self) -> bool;
    /// Draw one frame. `awaiting_handoff` is set after this picker returned a
    /// `ShowPicker` action, until the next picker replaces it.
    fn show(
        &mut self,
        ctx: &egui::Context,
        placement: Placement,
        appearance: &Appearance,
        awaiting_handoff: bool,
    ) -> Option<Outcome>;
}

/// Host settings available while building a picker.
pub struct PickerHost<'a> {
    pub ctx: &'a egui::Context,
    pub hwnd: isize,
    pub appearance: &'a Appearance,
    pub result_rows: usize,
    pub preview_rows: usize,
    pub file_preview_resolver: &'a crate::file_preview::FilePreviewResolver,
}
impl PickerHost<'_> {
    pub fn config(&self) -> nfm_egui::PickerConfig {
        nfm_egui::PickerConfig {
            result_rows: self.result_rows,
            preview_rows: self.preview_rows,
            ..Default::default()
        }
    }
}

/// What a picker accepted; `complete` carries it out.
pub enum Outcome {
    /// Close and restore focus.
    Cancelled,
    /// Close, report the error and restore focus.
    Failed(String),
    /// Run one of SWM's commands, as a keybinding would. A `ShowPicker` action
    /// replaces this picker without closing the window.
    Action(Action),
    /// Run picker-specific code with access to the window-manager runtime, after the
    /// picker closes. For results no `Action` describes (a chosen window, a file path).
    Runtime(RuntimeCall),
}

/// Carry out a picker's outcome on the host thread. Closing comes first so the
/// worker sees no visible picker while it runs what was accepted; only picker and
/// quit actions involve the UI thread.
pub(crate) fn complete(session: u64, outcome: Outcome) {
    if let Outcome::Action(action) = outcome
        && crate::ui::is_menu(action)
    {
        // The host keeps its window for the picker this action opens.
        if crate::ui::menu_is_visible(session) {
            crate::ui::dispatch(action);
        }
        return;
    }
    if !crate::ui::close_menu(session) {
        return; // A newer picker replaced this one.
    }
    let sent = match outcome {
        Outcome::Cancelled => crate::worker::send(crate::worker::Command::MenuClosed),
        Outcome::Failed(error) => {
            crate::error_log::report("picker failed", error);
            crate::worker::send(crate::worker::Command::MenuClosed)
        }
        Outcome::Action(action) => crate::ui::dispatch(action),
        Outcome::Runtime(call) => crate::worker::send(crate::worker::Command::Runtime(call)),
    };
    if !sent {
        crate::error_log::report("picker failed", "command queue is full");
    }
}

/// A call into the window-manager runtime, made on its thread with the runtime
/// locked: window management waits until it returns, so start a thread for
/// anything that may block. The label is for logs and tests.
pub struct RuntimeCall {
    label: String,
    run: Box<dyn FnOnce(&mut crate::runtime::Runtime) + Send>,
}
impl RuntimeCall {
    pub fn new(
        label: impl Into<String>,
        run: impl FnOnce(&mut crate::runtime::Runtime) + Send + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            run: Box::new(run),
        }
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub(crate) fn run(self, runtime: &mut crate::runtime::Runtime) {
        (self.run)(runtime)
    }
}
impl fmt::Debug for RuntimeCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RuntimeCall").field(&self.label).finish()
    }
}
