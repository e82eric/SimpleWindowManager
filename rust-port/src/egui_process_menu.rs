//! SWM process presentation using NFM's shared picker.
use nfm_egui::{Appearance, PickerConfig, Placement, egui, shared::SharedPicker};
use nfm_process_egui::ProcessPicker;
pub(crate) struct ProcessMenu(ProcessPicker<nfm_egui::shared::SharedSource>);
impl ProcessMenu {
    pub(crate) fn with_picker(
        ctx: &egui::Context,
        result_rows: usize,
        preview_rows: usize,
        previous: Option<SharedPicker>,
    ) -> Self {
        Self(ProcessPicker::with_shared_picker(
            ctx,
            PickerConfig {
                result_rows,
                preview_rows,
                preview_visible: false,
                ..Default::default()
            },
            previous,
        ))
    }
    pub(crate) fn into_picker(self) -> SharedPicker {
        self.0.into_picker()
    }
    pub(crate) fn show(
        &mut self,
        ctx: &egui::Context,
        placement: Placement,
        appearance: &Appearance,
    ) -> bool {
        self.0.show(ctx, placement, appearance)
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
                menu.show(
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
    fn nfm_process_picker_reuses_swm_control_and_maps_process_items() {
        let ctx = egui::Context::default();
        let mut menu = ProcessMenu::with_picker(&ctx, 7, 14, None);
        select_self(&ctx, &mut menu);
        let picker = menu.into_picker();
        let mut reopened = ProcessMenu::with_picker(&ctx, 7, 14, Some(picker));
        assert!(reopened.0.picker().selection().is_none());
        assert_eq!(reopened.0.picker().query(), "");
        select_self(&ctx, &mut reopened);
    }
}
