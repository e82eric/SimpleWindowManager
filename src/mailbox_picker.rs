//! A bar mailbox's entries in a picker, opened below the mailbox like a drop-down.
//! Each entry is a card: its state's dot, source and age, title, and detail.
//! Accepting one tells the producer and closes the picker; Esc or clicking
//! elsewhere closes it. Fuzzy search reads the cards' text, including the age.
//! The picker lists the entries as they were when it opened: it is open only
//! briefly, and the bar shows what changes meanwhile.
use crate::egui_window_menu::ScreenBounds;
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

/// Where the clicked mailbox is, in screen pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Anchor {
    pub right: i32,
    /// The bar's bottom edge.
    pub top: i32,
    /// The bar's left and right edges, which the picker stays within.
    pub bounds: (i32, i32),
}

/// The picker's width in points, times the monitor's scale in pixels.
const WIDTH: f32 = 800.0;
/// Cards shown before the list scrolls.
const MAX_ROWS: usize = 6;

const COLUMNS: [&str; 7] = ["Id", "State", "Source", "Title", "Detail", "Time", "Dot"];
const TEMPLATE: &str = "Source:muted|Time:right:muted;Title:title;Detail:muted";

/// The mailbox whose picker is open.
static OPEN: Mutex<Option<usize>> = Mutex::new(None);

/// Open the picker for mailbox `index` below `anchor`, or close it if it is open.
pub(crate) fn toggle(index: usize, anchor: Anchor) {
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
    anchor: Anchor,
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
        // The window is sized for the entries (see `window_bounds`), which
        // limits the cards to them.
        let picker = shared::replace(
            host.ctx,
            source,
            PickerConfig {
                title: mailbox.name.clone(),
                result_rows: MAX_ROWS,
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
            rows: state.entries.len().clamp(1, MAX_ROWS),
            template,
            picker,
        }))
    }
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

/// The window: the picker `WIDTH` wide, its right edge at the mailbox's but
/// inside the bar, its top touching the bar, and as tall as `height_points`.
/// The window's `margin` (points, transparent) reaches over the bar's edge so
/// the picker itself lines up with it.
fn window_bounds(anchor: Anchor, height_points: f32, margin: f32, scale: f32) -> ScreenBounds {
    let margin_pixels = (margin * scale).round() as i32;
    let width = (((WIDTH + 2.0 * margin) * scale).round() as i32).max(1);
    let lowest = anchor.bounds.0 - margin_pixels;
    let left = (anchor.right + margin_pixels - width)
        .clamp(lowest, (anchor.bounds.1 + margin_pixels - width).max(lowest));
    ScreenBounds {
        left,
        top: anchor.top - margin_pixels,
        width,
        height: (((height_points + 2.0 * margin) * scale).ceil() as i32).max(1),
    }
}

struct View {
    index: usize,
    mailbox: BarMailbox,
    anchor: Anchor,
    /// Cards the window is sized for: one per entry, up to `MAX_ROWS`.
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
    /// Against the bar, so any slack in the window's height falls below.
    fn position(&self) -> nfm_egui::Position {
        nfm_egui::Position::Top
    }
    fn window_bounds(
        &self,
        ctx: &egui::Context,
        appearance: &Appearance,
        margin: f32,
        scale: f32,
    ) -> Option<ScreenBounds> {
        let height = nfm_egui::results_height(ctx, appearance, Some(&self.template), self.rows);
        Some(window_bounds(self.anchor, height, margin, scale))
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
    fn the_picker_ends_at_the_mailbox_touching_the_bar_and_stays_inside_it() {
        let anchor = Anchor {
            right: 1500,
            top: 30,
            bounds: (0, 1920),
        };
        // An 8-point margin is 12 pixels at 1.5: the window reaches that far
        // over the bar and past the mailbox, so the picker inside it meets both.
        let window = window_bounds(anchor, 300.0, 8.0, 1.5);
        assert_eq!(
            [window.left, window.top, window.width, window.height],
            [288, 18, 1224, 474]
        );
        assert_eq!(window.left + window.width - 12, anchor.right);
        assert_eq!(window.top + 12, anchor.top);
        let near_left = Anchor {
            right: 200,
            ..anchor
        };
        assert_eq!(window_bounds(near_left, 300.0, 8.0, 1.0).left, -8);
        let narrow = Anchor {
            right: 300,
            top: 30,
            bounds: (0, 300),
        };
        assert_eq!(window_bounds(narrow, 300.0, 8.0, 1.0).left, -8);
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
}
