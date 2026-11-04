//! A picker over parts of one styled document: each row names a part, and the
//! preview shows the whole document centered on the selected part, highlighted.
//! Enter opens copy mode on the preview with its cursor on the selected part, and
//! the preview has quick select too. A mailbox producer opens one with
//! `MailboxSender::show_picker`.
use crate::picker::{Outcome, PickerHost, PickerRequest, PickerView};
use nfm_egui::shared::{self, SharedItem, SharedPicker};
use nfm_egui::styled_document::{DocumentPreviewProvider, DocumentRange, StyledDocument};
use nfm_egui::{Appearance, PickerConfig, Placement, egui};
use nfm_picker_sources::structured::{StructuredSourceRow, structured_source_with_previews};
use std::sync::Arc;

pub struct DocumentPicker {
    pub title: String,
    /// The heading of the list's one column.
    pub column: String,
    pub document: Arc<StyledDocument>,
    /// Listed in order; the first is selected when the picker opens.
    pub items: Vec<DocumentItem>,
    /// The background of the selected part in the preview.
    pub highlight: egui::Color32,
    /// Opens filling its monitor up to the bar, rather than centered.
    pub full_pane: bool,
}
pub struct DocumentItem {
    pub label: String,
    pub range: DocumentRange,
}

impl PickerRequest for DocumentPicker {
    fn build(self: Box<Self>, host: &PickerHost<'_>) -> Result<Box<dyn PickerView>, String> {
        Ok(Box::new(self.view(
            host.ctx,
            host.result_rows,
            host.preview_rows,
        )?))
    }
}
impl DocumentPicker {
    fn view(
        self,
        ctx: &egui::Context,
        result_rows: usize,
        preview_rows: usize,
    ) -> Result<DocumentView, String> {
        let DocumentPicker {
            title,
            column,
            document,
            items,
            highlight,
            full_pane,
        } = self;
        let rows = items.iter().map(|item| StructuredSourceRow {
            cells: vec![item.label.clone()],
            preview: None,
        });
        let source = structured_source_with_previews(vec![column], rows)?;
        let mut picker = shared::replace(
            ctx,
            source,
            PickerConfig {
                title,
                result_rows,
                preview_rows,
                preview_visible: true,
                ..Default::default()
            },
            None,
        );
        // A selection's index is its row's position in the source.
        let ranges: Vec<_> = items.iter().map(|item| item.range).collect();
        picker.set_preview_provider(
            ctx,
            Arc::new(DocumentPreviewProvider::new(
                move |selection: &nfm_egui::Selection<SharedItem>| {
                    Ok((document.clone(), ranges.get(selection.index).copied()))
                },
                highlight,
            )),
        );
        Ok(DocumentView(picker, full_pane))
    }
}

/// The picker, and whether it opens full-pane.
struct DocumentView(SharedPicker, bool);
impl PickerView for DocumentView {
    fn picker(&self) -> &SharedPicker {
        &self.0
    }
    fn has_header(&self) -> bool {
        true
    }
    fn position(&self) -> nfm_egui::Position {
        if self.1 {
            nfm_egui::Position::FullPane
        } else {
            nfm_egui::Position::Center
        }
    }
    fn show(
        &mut self,
        ctx: &egui::Context,
        placement: Placement,
        appearance: &Appearance,
        _: bool,
    ) -> Option<Outcome> {
        // Enter on the list opens copy mode on the preview, its cursor on the
        // selected part. In copy mode Enter is the preview's own, such as to submit
        // a search; Esc or Ctrl+W go back to the list.
        if !self.0.preview_focused()
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
        {
            self.0.focus_preview(ctx);
        }
        let event = self.0.show(ctx, placement, appearance, false).event;
        crate::egui_data_menu::copy_or_close(ctx, &mut self.0, event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown_document::{DocumentWriter, line_range};

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn enter_opens_copy_mode_on_the_preview_and_is_then_the_previews_own() {
        let mut writer = DocumentWriter::default();
        let mut items = Vec::new();
        for part in ["first", "second"] {
            let line = writer.line();
            writer.line_of(part, Default::default());
            items.push(DocumentItem {
                label: part.into(),
                range: line_range(line, part.len()),
            });
        }
        let picker = DocumentPicker {
            title: "Parts".into(),
            column: "Part".into(),
            document: writer.finish(),
            items,
            highlight: egui::Color32::DARK_GRAY,
            full_pane: false,
        };
        let ctx = egui::Context::default();
        let mut view = picker.view(&ctx, 5, 5).unwrap();
        assert_eq!(view.position(), nfm_egui::Position::Center);
        // The preview loads on a worker; until it has, Enter has nothing to focus.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            assert!(frame(&ctx, &mut view, vec![key(egui::Key::Enter)]).is_none());
            if view.0.preview_focused() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "copy mode never opened"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // In copy mode Enter is the preview's: the picker stays open, still focused.
        assert!(frame(&ctx, &mut view, vec![key(egui::Key::Enter)]).is_none());
        assert!(view.0.preview_focused());
    }

    #[test]
    fn a_full_pane_document_picker_opens_full_pane() {
        let mut writer = DocumentWriter::default();
        writer.line_of("only", Default::default());
        let picker = DocumentPicker {
            title: "Parts".into(),
            column: "Part".into(),
            document: writer.finish(),
            items: Vec::new(),
            highlight: egui::Color32::DARK_GRAY,
            full_pane: true,
        };
        let view = picker.view(&egui::Context::default(), 5, 5).unwrap();
        assert_eq!(view.position(), nfm_egui::Position::FullPane);
    }

    fn frame(
        ctx: &egui::Context,
        view: &mut DocumentView,
        events: Vec<egui::Event>,
    ) -> Option<Outcome> {
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let input = egui::RawInput {
            screen_rect: Some(bounds),
            events,
            ..Default::default()
        };
        let mut outcome = None;
        ctx.run_ui(input, |ui| {
            outcome = view.show(
                ui.ctx(),
                Placement::new(bounds),
                &Appearance::default(),
                true,
            );
        })
        .drop_without_applying_deltas();
        outcome
    }
}
