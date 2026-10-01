//! SWM filesystem presentation using NFM's shared picker.
use nfm_egui::{PickerConfig, egui, shared::SharedPicker};
use nfm_file_system::FileSystemPickerOptions;
pub(crate) use nfm_file_system_egui::FileEvent;
use nfm_file_system_egui::FilePicker;
type SharedFiles = FilePicker<nfm_egui::shared::SharedSource>;
pub(crate) struct FileMenu(SharedFiles);
impl std::ops::Deref for FileMenu {
    type Target = SharedFiles;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for FileMenu {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl FileMenu {
    pub(crate) fn into_picker(self) -> SharedPicker {
        self.0.into_picker()
    }
    pub(crate) fn with_picker(
        ctx: &egui::Context,
        options: FileSystemPickerOptions,
        result_rows: usize,
        preview_rows: usize,
        previous: Option<SharedPicker>,
    ) -> Result<Self, String> {
        SharedFiles::with_shared_picker(
            ctx,
            options,
            PickerConfig {
                result_rows,
                preview_rows,
                ..Default::default()
            },
            previous,
        )
        .map(Self)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use nfm_egui::{Appearance, Placement};
    use std::{
        fs,
        path::Path,
        time::{Duration, Instant},
    };
    fn draw(
        ctx: &egui::Context,
        menu: &mut FileMenu,
        key: Option<(egui::Key, egui::Modifiers)>,
    ) -> Option<FileEvent> {
        let mut result = None;
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600.0, 900.0),
                )),
                events: key
                    .map(|(key, modifiers)| egui::Event::Key {
                        key,
                        modifiers,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                    })
                    .into_iter()
                    .collect(),
                ..Default::default()
            },
            |ui| {
                if let Some(event) = menu
                    .show(
                        ui.ctx(),
                        Placement::new(ui.ctx().content_rect()),
                        &Appearance::default(),
                    )
                    .unwrap()
                {
                    result = Some(event);
                }
            },
        )
        .drop_without_applying_deltas();
        result
    }
    fn select(ctx: &egui::Context, menu: &mut FileMenu, path: &Path) {
        menu.picker_mut()
            .set_query(path.file_name().unwrap().to_str().unwrap());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            draw(ctx, menu, None);
            if menu.picker().selection().is_some_and(|selection| {
                selection
                    .item
                    .downcast_ref::<String>()
                    .is_some_and(|value| Path::new(value) == path)
            }) {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn nfm_navigation_and_picker_reuse_work_with_swm_items() {
        let root = std::env::temp_dir().join(format!("swm-nfm-adapter-{}", std::process::id()));
        let child = root.join("child");
        fs::create_dir_all(&child).unwrap();
        let file = child.join("document.txt");
        fs::write(&file, "contents").unwrap();
        let ctx = egui::Context::default();
        let options = FileSystemPickerOptions {
            roots: vec![root.to_string_lossy().into_owned()],
            preview_visible: false,
            ..Default::default()
        };
        let mut menu = FileMenu::with_picker(&ctx, options.clone(), 7, 14, None).unwrap();
        select(&ctx, &mut menu, &child);
        assert_eq!(
            draw(
                &ctx,
                &mut menu,
                Some((egui::Key::Enter, egui::Modifiers::NONE))
            ),
            None
        );
        assert_eq!(Path::new(&menu.options().roots[0]), child);
        draw(&ctx, &mut menu, None);
        draw(&ctx, &mut menu, Some((egui::Key::U, egui::Modifiers::CTRL)));
        assert_eq!(Path::new(&menu.options().roots[0]), root);
        let picker = menu.into_picker();
        let mut menu = FileMenu::with_picker(&ctx, options, 7, 14, Some(picker)).unwrap();
        assert!(menu.picker().selection().is_none());
        select(&ctx, &mut menu, &file);
        assert_eq!(
            draw(
                &ctx,
                &mut menu,
                Some((egui::Key::Enter, egui::Modifiers::NONE))
            ),
            Some(FileEvent::Selected(file.to_string_lossy().into_owned()))
        );
        drop(menu);
        fs::remove_file(file).unwrap();
        fs::remove_dir(child).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
