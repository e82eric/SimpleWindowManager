//! Build the small in-memory command list synchronously for each opening.
use crate::commands::Action;
use crate::picker::{Outcome, PickerHost, PickerRequest, PickerSource, PickerView};
use nfm_egui::shared::{self, SharedPicker};
use nfm_egui::{Appearance, PickerEvent, Placement, egui};
use nfm_picker_sources::structured::StructuredPickerItem;
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
        let (rows, actions) = self
            .rows
            .into_iter()
            .map(|(row, action)| {
                let name = row.first().cloned().unwrap_or_default();
                (row, (name, action))
            })
            .unzip();
        let picker = shared::replace(
            host.ctx,
            source(rows)?,
            nfm_egui::PickerConfig {
                title: "Commands".into(),
                preview_visible: false,
                ..host.config()
            },
            None,
        );
        Ok(Box::new(CommandsView { picker, actions }))
    }
}
struct CommandsView {
    picker: SharedPicker,
    actions: Vec<(String, Action)>,
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
            Some(PickerEvent::Accepted(selection)) => {
                let item = selection.item.downcast_ref::<StructuredPickerItem>()?;
                let name = item.fields.get("Name")?;
                let (_, action) = self.actions.iter().find(|(n, _)| n == name)?;
                Some(Outcome::Action(*action))
            }
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
    fn malformed_rows_are_rejected_and_empty_lists_are_ready() {
        assert!(source(vec![vec!["invalid".into()]]).is_err());
        let empty = source(Vec::new()).unwrap();
        assert!(empty.is_done());
        assert_eq!(empty.snapshot().unwrap().len(), 0);
    }
}
