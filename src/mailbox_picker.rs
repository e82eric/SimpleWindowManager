//! A bar mailbox's entries in a picker, opened below the mailbox like a drop-down.
//! Each entry is a card: its state's dot, source and age, title, and detail.
//! Accepting one tells the producer and closes the picker; Esc or clicking
//! elsewhere closes it. Fuzzy search reads the cards' text, including the age.
//! The picker lists the entries as they were when it opened: it is open only
//! briefly, and the bar shows what changes meanwhile.
use crate::egui_window_menu::{BarAlign, BarAnchor, ScreenBounds};
use crate::mailbox::{Activation, BarMailbox, MailboxEvent, MailboxState};
use crate::picker::{Outcome, PickerHost, PickerRequest, PickerView};
use nfm_egui::shared::{self, SharedPicker};
use nfm_egui::{Appearance, PickerConfig, PickerEvent, Placement, egui};
use nfm_picker_sources::structured::{
    StructuredPickerItem, StructuredSchema, StructuredStreamingSnapshot, StructuredStreamingStore,
};
use nfm_search_core::row::RowTemplate;
use nfm_search_core::snapshot_store::SnapshotStore;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The picker's width in points, times the monitor's scale in pixels.
const WIDTH: f32 = 800.0;
/// Cards shown before the list scrolls.
const MAX_ROWS: usize = 6;

const COLUMNS: [&str; 7] = ["Id", "State", "Source", "Title", "Detail", "Time", "Dot"];
const TEMPLATE: &str = "Source:muted|Time:right:muted;Title:title;Detail:muted";

/// The mailbox whose picker is open.
static OPEN: Mutex<Option<usize>> = Mutex::new(None);

/// Open the picker for mailbox `index`, or close it if it is open: dropping
/// down from `anchor`, its region on the bar, or without one (from a key
/// binding) centered and sized like other pickers showing a preview, with
/// cards in that height.
pub(crate) fn toggle(index: usize, anchor: Option<BarAnchor>) {
    if *OPEN.lock().unwrap() == Some(index) && crate::ui::menu_visible() {
        crate::ui::post(crate::ui::Command::Hide);
        return;
    }
    let Some(mailbox) = mailbox(index) else {
        return;
    };
    crate::ui::post(crate::ui::Command::ShowRequest(Box::new(Request {
        index,
        mailbox,
        anchor,
    })));
}

fn mailbox(index: usize) -> Option<BarMailbox> {
    crate::worker::snapshot()?
        .configuration
        .bar_mailboxes
        .get(index)
        .cloned()
}

struct Request {
    index: usize,
    mailbox: BarMailbox,
    anchor: Option<BarAnchor>,
}
impl PickerRequest for Request {
    fn build(self: Box<Self>, host: &PickerHost<'_>) -> Result<Box<dyn PickerView>, String> {
        let Request {
            index,
            mailbox,
            anchor,
        } = *self;
        let state = mailbox.current();
        let (snapshot, template) = snapshot(&mailbox, &state)?;
        let source = Arc::new(SnapshotStore::new());
        source.publish(snapshot);
        source.complete();
        let rows = state.entries.len().clamp(1, MAX_ROWS);
        let result_rows = match anchor {
            // The window is sized for the entries (see `window_bounds`), which
            // limits the cards to them.
            Some(_) => MAX_ROWS,
            // The host's window with a preview (see `sized_with_preview`): as
            // many cards as fit where its result and preview rows would go.
            None => cards_in_rows(host, &template, host.result_rows + host.preview_rows),
        };
        let picker = shared::replace(
            host.ctx,
            source,
            PickerConfig {
                title: mailbox.name.clone(),
                result_rows,
                preview_visible: false,
                ..Default::default()
            },
            None,
        );
        *OPEN.lock().unwrap() = Some(index);
        mailbox.send(MailboxEvent::Opened);
        Ok(Box::new(View {
            index,
            mailbox,
            anchor,
            rows,
            template,
            picker,
        }))
    }
}

/// How many cards fit in the height of `rows` of the host's one-line rows.
fn cards_in_rows(host: &PickerHost<'_>, template: &RowTemplate, rows: usize) -> usize {
    let line = host.appearance.typography.preview_row_height(host.ctx);
    let card = nfm_egui::results_height(host.ctx, host.appearance, Some(template), 2)
        - nfm_egui::results_height(host.ctx, host.appearance, Some(template), 1);
    ((rows as f32 * line) / card).floor().max(1.0) as usize
}

/// The entries as a structured source with a card template.
fn snapshot(
    mailbox: &BarMailbox,
    state: &MailboxState,
) -> Result<(Arc<StructuredStreamingSnapshot>, RowTemplate), String> {
    // Ages are worded when shown; the column stays a time for sorts and filters.
    // Newest first until the query has text or sorts of its own.
    let schema = StructuredSchema::new(COLUMNS.map(String::from).to_vec())?
        .with_types("Time:datetime")?
        .with_default_sort("/!Descending==Time")?
        .with_formatter("Time", |seconds| {
            seconds.parse().map_or_else(
                |_| seconds.to_owned(),
                |seconds| {
                    crate::mailbox::relative_time(
                        UNIX_EPOCH + Duration::from_secs(seconds),
                        SystemTime::now(),
                    )
                },
            )
        })?;
    let template = schema
        .row_template(TEMPLATE, Some("Dot"))
        .map_err(|error| error.to_string())?;
    let mut store = StructuredStreamingStore::new(schema);
    store.set_row_template(template.clone())?;
    for entry in &state.entries {
        let seconds = entry
            .time
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let color = mailbox
            .style(&entry.state)
            .and_then(|style| style.icon.as_ref())
            .map_or(crate::palette::MUTED, |icon| icon.color);
        store.add_record(&csv::StringRecord::from(vec![
            entry.id.clone(),
            entry.state.clone(),
            entry.source.clone(),
            entry.title.clone(),
            entry.detail.clone(),
            seconds.to_string(),
            format!(
                "glyph:●@#{:02x}{:02x}{:02x}",
                color.r(),
                color.g(),
                color.b()
            ),
        ]))?;
    }
    Ok((store.snapshot(), template))
}

struct View {
    index: usize,
    mailbox: BarMailbox,
    /// The bar region it drops down from; without one, the host places it.
    anchor: Option<BarAnchor>,
    /// Cards a drop-down's window is sized for: one per entry, up to `MAX_ROWS`.
    rows: usize,
    template: RowTemplate,
    picker: SharedPicker,
}
impl PickerView for View {
    fn picker(&self) -> &SharedPicker {
        &self.picker
    }
    fn has_header(&self) -> bool {
        false
    }
    /// From a key binding: as tall as other pickers showing their preview,
    /// with cards where the preview would be.
    fn sized_with_preview(&self) -> bool {
        self.anchor.is_none()
    }
    fn position(&self) -> nfm_egui::Position {
        // A drop-down sits against the bar, so any slack in the window's
        // height falls below.
        if self.anchor.is_some() {
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
        let anchor = self.anchor?;
        let height = nfm_egui::results_height(ctx, appearance, Some(&self.template), self.rows);
        Some(crate::egui_window_menu::below_bar(
            anchor,
            BarAlign::Right,
            WIDTH,
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
        _: bool,
    ) -> Option<Outcome> {
        match self.picker.show(ctx, placement, appearance, false).event {
            Some(PickerEvent::Accepted(selection)) => {
                if let Some(item) = selection.item.downcast_ref::<StructuredPickerItem>()
                    && let Some(id) = item.fields.get("Id")
                {
                    self.mailbox.send(MailboxEvent::Activated {
                        id: id.clone(),
                        how: Activation::Enter,
                    });
                }
                Some(Outcome::Cancelled)
            }
            Some(PickerEvent::Cancelled) => Some(Outcome::Cancelled),
            Some(PickerEvent::CopyRequested(selection)) => {
                ctx.copy_text(selection.text);
                self.picker.notify_copied(ctx);
                None
            }
            Some(PickerEvent::CustomAction { .. }) | None => None,
        }
    }
}
impl Drop for View {
    fn drop(&mut self) {
        let mut open = OPEN.lock().unwrap();
        if *open == Some(self.index) {
            *open = None;
        }
        drop(open);
        self.mailbox.send(MailboxEvent::Closed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mailbox::{Icon, MailboxEntry, MailboxSender, StateStyle};
    use nfm_search_core::store::ItemsSource;

    fn mailbox() -> BarMailbox {
        fn producer(_: MailboxSender) {}
        let mut mailbox = BarMailbox::new("Agents", producer);
        mailbox.state(
            "waiting",
            StateStyle::icon(Icon::glyph("●", egui::Color32::from_rgb(0xd2, 0x0f, 0x39))),
        );
        mailbox
    }
    fn entry(id: &str, state: &str, ago: u64) -> MailboxEntry {
        MailboxEntry {
            id: id.into(),
            state: state.into(),
            source: "npicker".into(),
            title: format!("Title {id}"),
            detail: "detail".into(),
            time: SystemTime::now() - Duration::from_secs(ago),
        }
    }

    #[test]
    fn entries_become_cards_with_their_state_color_and_age() {
        let state = MailboxState {
            counts: vec![],
            entries: vec![entry("a", "waiting", 600), entry("b", "idle", 7200)],
        };
        let (snapshot, _) = snapshot(&mailbox(), &state).unwrap();
        assert_eq!(snapshot.len(), 2);
        let fields = snapshot.row_fields(0).unwrap();
        assert_eq!(fields.values[0], "a");
        assert_eq!(fields.values[5], "10 minutes ago");
        assert_eq!(fields.values[6], "glyph:●@#d20f39");
        // An undeclared state gets the muted dot.
        let muted = crate::palette::MUTED;
        assert_eq!(
            snapshot.row_fields(1).unwrap().values[6],
            format!(
                "glyph:●@#{:02x}{:02x}{:02x}",
                muted.r(),
                muted.g(),
                muted.b()
            )
        );
        // Fuzzy search reads the age as shown.
        assert!(
            snapshot
                .get_string_lossy(1, &mut Vec::new())
                .contains("2 hours ago")
        );
        assert_eq!(snapshot.item(1).unwrap().fields["Id"], "b");
    }

    #[test]
    fn the_window_is_as_tall_as_its_cards() {
        let ctx = egui::Context::default();
        ctx.run_ui(Default::default(), |_| {})
            .drop_without_applying_deltas();
        let appearance = crate::egui_window_menu::desktop_appearance();
        let (_, template) = snapshot(&mailbox(), &MailboxState::default()).unwrap();
        let one = nfm_egui::results_height(&ctx, &appearance, Some(&template), 1);
        let three = nfm_egui::results_height(&ctx, &appearance, Some(&template), 3);
        let card = (three - one) / 2.0;
        assert!(card > appearance.typography.preview_row_height(&ctx) * 2.0);
    }

    #[test]
    fn the_centered_picker_fills_the_preview_height_with_cards() {
        let ctx = egui::Context::default();
        ctx.run_ui(Default::default(), |_| {})
            .drop_without_applying_deltas();
        let appearance = crate::egui_window_menu::desktop_appearance();
        let resolver = crate::file_preview::resolver(Default::default());
        let host = PickerHost {
            ctx: &ctx,
            hwnd: 0,
            appearance: &appearance,
            result_rows: crate::egui_window_menu::RESULT_ROWS,
            preview_rows: crate::egui_window_menu::PREVIEW_ROWS,
            file_preview_resolver: &resolver,
        };
        let (_, template) = snapshot(&mailbox(), &MailboxState::default()).unwrap();
        // The result rows alone fit 2 cards; with the preview's rows, 7.
        assert_eq!(cards_in_rows(&host, &template, host.result_rows), 2);
        assert_eq!(
            cards_in_rows(&host, &template, host.result_rows + host.preview_rows),
            7
        );
    }
}
