//! Window switcher: NFM's window picker; accepting focuses the window.
use crate::picker::{Outcome, PickerHost, PickerRequest, PickerSource, PickerView, RuntimeCall};
use nfm_egui::shared::{SharedPicker, SharedSource};
use nfm_egui::{Appearance, Placement, egui};
use nfm_window_egui::{WindowEvent, WindowPicker};

#[derive(Clone, Copy, Debug)]
pub struct Windows;
impl PickerSource for Windows {
    fn request(&self, _: &crate::Configuration) -> Box<dyn PickerRequest> {
        Box::new(Windows)
    }
}
impl PickerRequest for Windows {
    fn build(self: Box<Self>, host: &PickerHost<'_>) -> Result<Box<dyn PickerView>, String> {
        let config = nfm_egui::PickerConfig {
            preview_visible: true,
            ..host.config()
        };
        Ok(Box::new(WindowsView(WindowPicker::with_shared_picker(
            host.ctx, host.hwnd, config, None,
        )?)))
    }
}

struct WindowsView(WindowPicker<SharedSource>);
impl PickerView for WindowsView {
    fn picker(&self) -> &SharedPicker {
        self.0.picker()
    }
    fn has_header(&self) -> bool {
        false
    }
    fn show(
        &mut self,
        ctx: &egui::Context,
        placement: Placement,
        appearance: &Appearance,
        _: bool,
    ) -> Option<Outcome> {
        self.0
            .show(ctx, placement, appearance)
            .map(|event| match event {
                WindowEvent::Selected(hwnd) => Outcome::Runtime(RuntimeCall::new(
                    format!("focus window {hwnd:#x}"),
                    move |runtime| runtime.menu_window_selected(hwnd),
                )),
                WindowEvent::Cancelled => Outcome::Cancelled,
            })
    }
}
