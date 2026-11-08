//! Build the small in-memory command list synchronously for each opening.
use crate::commands::Action;
use crate::picker::{Outcome, PickerHost, PickerRequest, PickerSource, PickerView};
use nfm_egui::shared::{self, SharedPicker};
use nfm_egui::{Appearance, PickerEvent, Placement, egui};
use nfm_picker_sources::structured::StructuredStreamingSnapshot;
use nfm_search_core::snapshot_store::SnapshotStore;
use crate::egui_window_menu::{BarAlign, BarAnchor, ScreenBounds};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) type CommandSource = SnapshotStore<StructuredStreamingSnapshot>;

const COLUMNS: [&str; 4] = ["Name", "Type", "KeyBinding", "Description"];
/// The launcher's columns, as indexes into `COLUMNS`.
const LAUNCHER_COLUMNS: [usize; 2] = [0, 2];
/// The launcher picker's width in points.
const LAUNCHER_WIDTH: f32 = 900.0;

#[cfg(test)]
fn source(rows: Vec<Vec<String>>) -> Result<Arc<CommandSource>, String> {
    columns_source(&[0, 1, 2, 3], rows)
}

/// `rows` of `COLUMNS`, cut to the `columns` at those indexes.
fn columns_source(columns: &[usize], rows: Vec<Vec<String>>) -> Result<Arc<CommandSource>, String> {
    let rows = rows
        .into_iter()
        .map(|row| {
            if row.len() != COLUMNS.len() {
                return Err(format!(
                    "a command row has {} fields, not {}",
                    row.len(),
                    COLUMNS.len()
                ));
            }
            Ok(columns.iter().map(|&column| row[column].clone()).collect())
        })
        .collect::<Result<_, _>>()?;
    nfm_picker_sources::structured::structured_source(
        columns.iter().map(|&column| COLUMNS[column].to_owned()).collect(),
        rows,
    )
}

/// Whether the launcher is open, so its button closes it.
static LAUNCHER_OPEN: AtomicBool = AtomicBool::new(false);

/// Open the commands launcher (names and key bindings) below `anchor`, its
/// button on the bar, or close it if it is open.
pub(crate) fn toggle_launcher(anchor: BarAnchor) {
    if LAUNCHER_OPEN.load(Ordering::Acquire) && crate::ui::menu_visible() {
        crate::ui::post(crate::ui::Command::Hide);
        return;
    }
    let Some(snapshot) = crate::worker::snapshot() else {
        return;
    };
    crate::ui::post(crate::ui::Command::ShowRequest(Box::new(Launcher {
        list: CommandList::of(&snapshot.configuration),
        anchor,
    })));
}

struct Launcher {
    list: CommandList,
    anchor: BarAnchor,
}
impl PickerRequest for Launcher {
    fn build(self: Box<Self>, host: &PickerHost<'_>) -> Result<Box<dyn PickerView>, String> {
        let rows = self.list.rows.len().clamp(1, host.result_rows);
        let mut view = self.list.view(host.ctx, host.config(), &LAUNCHER_COLUMNS)?;
        view.launcher = Some((self.anchor, rows));
        LAUNCHER_OPEN.store(true, Ordering::Release);
        Ok(Box::new(view))
    }
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
        Box::new(CommandList::of(configuration))
    }
}
impl PickerRequest for CommandList {
    fn build(self: Box<Self>, host: &PickerHost<'_>) -> Result<Box<dyn PickerView>, String> {
        Ok(Box::new(self.view(host.ctx, host.config(), &[0, 1, 2, 3])?))
    }
}
impl CommandList {
    fn of(configuration: &crate::Configuration) -> Self {
        Self {
            rows: crate::platform::command_menu_rows(configuration)
                .into_iter()
                .zip(configuration.commands.iter().map(|command| command.action))
                .collect(),
        }
    }
    /// A picker showing `columns` of the rows (indexes into `COLUMNS`).
    fn view(
        self,
        ctx: &egui::Context,
        config: nfm_egui::PickerConfig,
        columns: &[usize],
    ) -> Result<CommandsView, String> {
        let (rows, actions) = self.rows.into_iter().unzip();
        let picker = shared::replace(
            ctx,
            columns_source(columns, rows)?,
            nfm_egui::PickerConfig {
                title: "Commands".into(),
                preview_visible: false,
                ..config
            },
            None,
        );
        Ok(CommandsView {
            picker,
            actions,
            launcher: None,
        })
    }
}
struct CommandsView {
    picker: SharedPicker,
    /// Each row's action, in row order.
    actions: Vec<Action>,
    /// For the launcher, the bar button it drops down from and the rows its
    /// window is sized for.
    launcher: Option<(BarAnchor, usize)>,
}
impl Drop for CommandsView {
    fn drop(&mut self) {
        if self.launcher.is_some() {
            LAUNCHER_OPEN.store(false, Ordering::Release);
        }
    }
}
impl PickerView for CommandsView {
    fn picker(&self) -> &SharedPicker {
        &self.picker
    }
    fn has_header(&self) -> bool {
        true
    }
    fn position(&self) -> nfm_egui::Position {
        // Against the bar, so any slack in the window's height falls below.
        if self.launcher.is_some() {
            nfm_egui::Position::Top
        } else {
            nfm_egui::Position::Center
        }
    }
    fn window_bounds(
        &self,
        ctx: &egui::Context,
        appearance: &Appearance,
        margin: f32,
        scale: f32,
    ) -> Option<ScreenBounds> {
        let (anchor, rows) = self.launcher?;
        // The rows and the column header above them.
        let height = nfm_egui::results_height(ctx, appearance, None, rows)
            + appearance.typography.preview_row_height(ctx);
        Some(crate::egui_window_menu::below_bar(
            anchor,
            BarAlign::Left,
            LAUNCHER_WIDTH,
            height,
            margin,
            scale,
        ))
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
        let mut view = list
            .view(&ctx, nfm_egui::PickerConfig::default(), &[0, 1, 2, 3])
            .unwrap();
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

    #[test]
    fn the_launcher_lists_only_names_and_key_bindings() {
        let row = ["Focus next", "Action", "Alt+J", "Focus the next window"];
        let source = columns_source(
            &LAUNCHER_COLUMNS,
            vec![row.map(str::to_owned).to_vec()],
        )
        .unwrap();
        let item = source.snapshot().unwrap().item(0).unwrap();
        let mut fields: Vec<_> = item.fields.iter().collect();
        fields.sort();
        assert_eq!(
            fields,
            [
                (&"KeyBinding".to_owned(), &"Alt+J".to_owned()),
                (&"Name".to_owned(), &"Focus next".to_owned()),
            ]
        );
        assert!(columns_source(&LAUNCHER_COLUMNS, vec![vec!["short".into()]]).is_err());
    }
}
