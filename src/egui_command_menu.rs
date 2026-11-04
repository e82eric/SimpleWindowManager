//! Build the small in-memory command list synchronously for each opening.
use crate::commands::Action;
use crate::picker::{Outcome, PickerHost, PickerRequest, PickerSource, PickerView};
use nfm_egui::shared::{self, SharedPicker};
use nfm_egui::{Appearance, PickerEvent, Placement, egui};
use nfm_picker_sources::structured::StructuredStreamingSnapshot;
use nfm_search_core::snapshot_store::SnapshotStore;
use std::sync::Arc;

pub(crate) type CommandSource = SnapshotStore<StructuredStreamingSnapshot>;

pub(crate) fn source(rows: Vec<Vec<String>>) -> Result<Arc<CommandSource>, String> {
    nfm_picker_sources::structured::structured_source(
        ["Name", "Type", "KeyBinding", "Description"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        rows,
    )
}

/// The registered commands; accepting one runs its action.
#[derive(Clone, Copy, Debug)]
pub struct Commands;
/// Rows of Name, Type, KeyBinding and Description, each with the action it runs.
pub struct CommandList {
    pub rows: Vec<(Vec<String>, Action)>,
}
impl PickerSource for Commands {
    fn request(&self, configuration: &crate::Configuration) -> Box<dyn PickerRequest> {
        Box::new(CommandList {
            rows: crate::platform::command_menu_rows(configuration)
                .into_iter()
                .zip(configuration.commands.iter().map(|command| command.action))
                .collect(),
        })
    }
}
impl PickerRequest for CommandList {
    fn build(self: Box<Self>, host: &PickerHost<'_>) -> Result<Box<dyn PickerView>, String> {
        Ok(Box::new(self.view(host.ctx, host.config())?))
    }
}
impl CommandList {
    fn view(
        self,
        ctx: &egui::Context,
        config: nfm_egui::PickerConfig,
    ) -> Result<CommandsView, String> {
        let (rows, actions) = self.rows.into_iter().unzip();
        let picker = shared::replace(
            ctx,
            source(rows)?,
            nfm_egui::PickerConfig {
                title: "Commands".into(),
                preview_visible: false,
                ..config
            },
            None,
        );
        Ok(CommandsView { picker, actions })
    }
}
struct CommandsView {
    picker: SharedPicker,
    /// Each row's action, in row order.
    actions: Vec<Action>,
}
impl PickerView for CommandsView {
    fn picker(&self) -> &SharedPicker {
        &self.picker
    }
    fn has_header(&self) -> bool {
        true
    }
    fn show(
        &mut self,
        ctx: &egui::Context,
        placement: Placement,
        appearance: &Appearance,
        awaiting_handoff: bool,
    ) -> Option<Outcome> {
        match self
            .picker
            .show(ctx, placement, appearance, awaiting_handoff)
            .event
        {
            // The selection's index is its row's position in the source, so commands
            // that share a name still run their own action.
            Some(PickerEvent::Accepted(selection)) => self
                .actions
                .get(selection.index)
                .map(|&action| Outcome::Action(action)),
            event => crate::egui_data_menu::copy_or_close(ctx, &mut self.picker, event),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nfm_search_core::store::ItemsSource;
    #[test]
    fn sources_are_ready_immediately_and_old_snapshots_remain_unchanged() {
        let rows = vec![vec![
            "first".into(),
            "Function".into(),
            "Ctrl+A".into(),
            "Original description".into(),
        ]];
        let first = source(rows.clone()).unwrap();
        assert!(first.is_done());
        let mut changed = rows;
        changed[0][0] = "second".into();
        changed[0][3] = "Updated description".into();
        let second = source(changed).unwrap();
        assert!(second.is_done());
        assert_eq!(
            first.snapshot().unwrap().item(0).unwrap().fields["Name"],
            "first"
        );
        assert_eq!(
            second.snapshot().unwrap().item(0).unwrap().fields["Description"],
            "Updated description"
        );
    }
    #[test]
    fn commands_that_share_a_name_run_their_own_action() {
        let row = |binding: &str| {
            vec![
                "move_workspace".into(),
                "Function".into(),
                binding.into(),
                String::new(),
            ]
        };
        let list = CommandList {
            rows: vec![
                (row("Win+F1"), Action::MoveWorkspaceToMonitor(0, 1)),
                (row("Alt+F13"), Action::MoveWorkspaceToMonitor(0, 0)),
            ],
        };
        let ctx = egui::Context::default();
        let mut view = list.view(&ctx, nfm_egui::PickerConfig::default()).unwrap();
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let mut frame = |events: Vec<egui::Event>| {
            let mut outcome = None;
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(bounds),
                    events,
                    ..Default::default()
                },
                |ui| {
                    outcome = view.show(
                        ui.ctx(),
                        Placement::new(bounds),
                        &Appearance::default(),
                        false,
                    )
                },
            )
            .drop_without_applying_deltas();
            outcome
        };
        frame(vec![egui::Event::Text("F13".into())]);
        // Search runs on a worker; Enter accepts once the matching row is selected.
        let enter = egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let outcome = loop {
            std::thread::sleep(std::time::Duration::from_millis(20));
            if let Some(outcome) = frame(vec![enter.clone()]) {
                break outcome;
            }
            assert!(std::time::Instant::now() < deadline, "nothing was accepted");
        };
        assert!(
            matches!(
                outcome,
                Outcome::Action(Action::MoveWorkspaceToMonitor(0, 0))
            ),
            "the first command with that name ran instead"
        );
    }

    #[test]
    fn malformed_rows_are_rejected_and_empty_lists_are_ready() {
        assert!(source(vec![vec!["invalid".into()]]).is_err());
        let empty = source(Vec::new()).unwrap();
        assert!(empty.is_done());
        assert_eq!(empty.snapshot().unwrap().len(), 0);
    }
}
