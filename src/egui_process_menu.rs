//! SWM process presentation using NFM's shared picker.
use crate::picker::{Outcome, PickerHost, PickerRequest, PickerSource, PickerView};
use nfm_egui::{Appearance, PickerConfig, Placement, egui, shared::SharedPicker};
use nfm_process_egui::ProcessPicker;
pub(crate) struct ProcessMenu(ProcessPicker<nfm_egui::shared::SharedSource>);
impl ProcessMenu {
    pub(crate) fn new(ctx: &egui::Context, result_rows: usize, preview_rows: usize) -> Self {
        Self(ProcessPicker::with_shared_picker(
            ctx,
            PickerConfig {
                result_rows,
                preview_rows,
                preview_visible: false,
                ..Default::default()
            },
            None,
        ))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Processes;
impl PickerSource for Processes {
    fn request(&self, _: &crate::Configuration) -> Box<dyn PickerRequest> {
        Box::new(Processes)
    }
}
impl PickerRequest for Processes {
    fn build(self: Box<Self>, host: &PickerHost<'_>) -> Result<Box<dyn PickerView>, String> {
        Ok(Box::new(ProcessMenu::new(
            host.ctx,
            host.result_rows,
            host.preview_rows,
        )))
    }
}
impl PickerView for ProcessMenu {
    fn picker(&self) -> &SharedPicker {
        self.0.picker()
    }
    fn has_header(&self) -> bool {
        true
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
            .then_some(Outcome::Cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    fn select_self(ctx: &egui::Context, menu: &mut ProcessMenu) {
        let pid = std::process::id();
        menu.0.picker_mut().set_query(format!("/:PID=={pid}"));
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            ctx.run_ui(egui::RawInput::default(), |ui| {
                menu.0.show(
                    ui.ctx(),
                    Placement::new(ui.ctx().content_rect()),
                    &Appearance::default(),
                );
            })
            .drop_without_applying_deltas();
            if menu.0.picker().selection().is_some_and(|s| {
                s.item
                    .downcast_ref::<nfm_win32::ProcessPickerItem>()
                    .is_some_and(|item| item.pid == pid)
            }) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "process adapter did not select the test process"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn nfm_process_picker_maps_process_items() {
        let ctx = egui::Context::default();
        let mut menu = ProcessMenu::new(&ctx, 7, 14);
        select_self(&ctx, &mut menu);
    }
}
