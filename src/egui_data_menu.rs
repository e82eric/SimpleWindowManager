//! Read-only structured menus share the owned egui host.
use crate::picker::{Outcome, PickerHost, PickerRequest, PickerSource, PickerView};
use nfm_egui::shared::{self, SharedItem, SharedPicker};
use nfm_egui::{Appearance, PickerConfig, PickerEvent, Placement, Preview, egui};
use nfm_picker_sources::structured::StructuredPickerItem;
pub use nfm_picker_sources::structured::StructuredSourceRow as DataRow;
use nfm_picker_sources::structured::structured_source_with_previews;
use std::sync::Arc;

pub(crate) fn picker(
    ctx: &egui::Context,
    title: String,
    columns: Vec<String>,
    rows: Vec<DataRow>,
    result_rows: usize,
    preview_rows: usize,
) -> Result<SharedPicker, String> {
    let preview_visible = rows.iter().any(|row| row.preview.is_some());
    let source = structured_source_with_previews(columns, rows)?;
    let mut picker = shared::replace(
        ctx,
        source,
        PickerConfig {
            title,
            result_rows,
            preview_rows,
            preview_visible,
            ..Default::default()
        },
        None,
    );
    picker.set_preview_provider(
        ctx,
        Arc::new(
            |request: nfm_egui::PreviewRequest<SharedItem>, _: nfm_egui::Cancellation| {
                let Some(item) = request
                    .selection
                    .item
                    .downcast_ref::<StructuredPickerItem>()
                else {
                    return Ok(Preview::Empty);
                };
                Ok(item
                    .preview
                    .clone()
                    .map(Preview::Text)
                    .unwrap_or(Preview::Empty))
            },
        ),
    );
    Ok(picker)
}

/// Read-only rows built from the configuration each time the picker opens.
#[derive(Clone, Copy, Debug)]
pub struct Data(pub fn(&crate::Configuration) -> DataTable);
#[derive(Clone, Debug, Default)]
pub struct DataTable {
    pub title: String,
    pub columns: Vec<String>,
    pub rows: Vec<DataRow>,
}
impl PickerSource for Data {
    fn request(&self, configuration: &crate::Configuration) -> Box<dyn PickerRequest> {
        Box::new((self.0)(configuration))
    }
}
impl PickerRequest for DataTable {
    fn build(self: Box<Self>, host: &PickerHost<'_>) -> Result<Box<dyn PickerView>, String> {
        let table = *self;
        Ok(Box::new(DataView(picker(
            host.ctx,
            table.title,
            table.columns,
            table.rows,
            host.result_rows,
            host.preview_rows,
        )?)))
    }
}
struct DataView(SharedPicker);
impl PickerView for DataView {
    fn picker(&self) -> &SharedPicker {
        &self.0
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
        let event = self.0.show(ctx, placement, appearance, false).event;
        copy_or_close(ctx, &mut self.0, event)
    }
}

/// Read-only handling: accept or cancel closes; copy requests copy the row.
pub(crate) fn copy_or_close(
    ctx: &egui::Context,
    picker: &mut SharedPicker,
    event: Option<PickerEvent<SharedItem>>,
) -> Option<Outcome> {
    match event {
        Some(PickerEvent::Accepted(_)) | Some(PickerEvent::Cancelled) => Some(Outcome::Cancelled),
        Some(PickerEvent::CopyRequested(selection)) => {
            ctx.copy_text(selection.text);
            picker.notify_copied(ctx);
            None
        }
        Some(PickerEvent::CustomAction { .. }) | None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nfm_search_core::store::ItemsSource;

    fn snapshot(
        columns: Vec<String>,
        rows: Vec<DataRow>,
    ) -> Result<Arc<nfm_picker_sources::structured::StructuredStreamingSnapshot>, String> {
        Ok(structured_source_with_previews(columns, rows)?
            .snapshot()
            .unwrap())
    }
    #[test]
    fn structured_filter_renders_matching_detail() {
        let ctx = egui::Context::default();
        let mut picker = picker(
            &ctx,
            "Error logs".into(),
            vec!["Message".into()],
            vec![
                DataRow {
                    cells: vec!["first".into()],
                    preview: Some("wrong detail".into()),
                },
                DataRow {
                    cells: vec!["second".into()],
                    preview: Some("matching detail\ntrace".into()),
                },
            ],
            7,
            14,
        )
        .unwrap();
        picker.set_query("/:Message==second");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1600.0, 1000.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    picker.show(
                        ui.ctx(),
                        nfm_egui::Placement::new(ui.ctx().content_rect()),
                        &nfm_egui::Appearance::default(),
                        false,
                    );
                },
            );
            let painted: String = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.job.text.as_str()),
                    _ => None,
                })
                .collect();
            let visible = painted.contains("matching detail");
            output.drop_without_applying_deltas();
            if visible {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "matching preview never rendered"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(
            picker
                .selection()
                .unwrap()
                .item
                .downcast_ref::<StructuredPickerItem>()
                .unwrap()
                .fields["Message"],
            "second"
        );
    }

    #[test]
    fn duplicate_rows_keep_their_own_multiline_preview() {
        let source = snapshot(
            vec!["Message".into()],
            vec![
                DataRow {
                    cells: vec!["same".into()],
                    preview: Some("first\ntrace".into()),
                },
                DataRow {
                    cells: vec!["same".into()],
                    preview: Some("second\ntrace".into()),
                },
            ],
        )
        .unwrap();
        assert_eq!(
            source.item(0).unwrap().preview.as_deref(),
            Some("first\ntrace")
        );
        assert_eq!(
            source.item(1).unwrap().preview.as_deref(),
            Some("second\ntrace")
        );
        assert_eq!(source.item(1).unwrap().fields["Message"], "same");
    }

    #[test]
    fn invalid_schema_and_row_width_are_rejected() {
        assert!(snapshot(vec![], vec![]).is_err());
        assert!(snapshot(vec!["A".into(), "a".into()], vec![]).is_err());
        assert!(
            snapshot(
                vec!["A".into()],
                vec![DataRow {
                    cells: vec![],
                    preview: None
                }]
            )
            .is_err()
        );
    }
}
