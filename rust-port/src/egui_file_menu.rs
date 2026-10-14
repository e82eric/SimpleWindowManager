//! Tests for NFM filesystem picker integration with SWM.
#[cfg(test)]
mod tests {
    use nfm_egui::{PickerConfig, egui, shared::SharedSource};
    use nfm_file_system::FileSystemPickerOptions;
    use nfm_file_system_egui::{FileEvent, FilePicker};
    type FileMenu = FilePicker<SharedSource>;
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
        let mut menu = FileMenu::with_shared_picker(
            &ctx,
            options.clone(),
            PickerConfig {
                result_rows: 7,
                preview_rows: 14,
                ..Default::default()
            },
            None,
        )
        .unwrap();
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
        let mut menu = FileMenu::with_shared_picker(
            &ctx,
            options.clone(),
            PickerConfig {
                result_rows: 7,
                preview_rows: 14,
                ..Default::default()
            },
            Some(picker),
        )
        .unwrap();
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
        let solution = child.join("solution.SLN");
        let outside = root.join("outside.sln");
        fs::write(&solution, "solution").unwrap();
        fs::write(&outside, "outside").unwrap();
        let picker = menu.into_picker();
        let mut menu = FileMenu::with_shared_picker(
            &ctx,
            FileSystemPickerOptions {
                roots: vec![child.to_string_lossy().into_owned()],
                files_only: true,
                search_string: Some(".sln$ ".into()),
                ..Default::default()
            },
            PickerConfig {
                result_rows: 7,
                preview_rows: 14,
                ..Default::default()
            },
            Some(picker),
        )
        .unwrap();
        assert!(menu.picker().selection().is_none());
        menu.picker_mut().set_presentation(7, 14, false);
        select(&ctx, &mut menu, &solution);
        assert_eq!(
            draw(
                &ctx,
                &mut menu,
                Some((egui::Key::Enter, egui::Modifiers::NONE))
            ),
            Some(FileEvent::Selected(solution.to_string_lossy().into_owned()))
        );
        let picker = menu.into_picker();
        let mut menu = FileMenu::with_shared_picker(
            &ctx,
            options,
            PickerConfig {
                result_rows: 7,
                preview_rows: 14,
                ..Default::default()
            },
            Some(picker),
        )
        .unwrap();
        assert!(menu.picker().selection().is_none());
        select(&ctx, &mut menu, &child);
        draw(
            &ctx,
            &mut menu,
            Some((egui::Key::Enter, egui::Modifiers::NONE)),
        );
        assert_eq!(Path::new(&menu.options().roots[0]), child);
        draw(&ctx, &mut menu, None);
        draw(&ctx, &mut menu, Some((egui::Key::U, egui::Modifiers::CTRL)));
        assert_eq!(Path::new(&menu.options().roots[0]), root);
        drop(menu);
        fs::remove_file(solution).unwrap();
        fs::remove_file(outside).unwrap();
        fs::remove_file(file).unwrap();
        fs::remove_dir(child).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
