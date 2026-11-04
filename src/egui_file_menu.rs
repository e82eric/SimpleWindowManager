//! File pickers: NFM filesystem browsing with a configurable accept action.
use crate::picker::{
    Outcome, PickerConfig, PickerHost, PickerRequest, PickerSource, PickerView, RuntimeCall,
};
use nfm_egui::shared::{SharedItem, SharedPicker, SharedSource};
use nfm_egui::{Appearance, Placement, egui};
use nfm_file_system::FileSystemPickerOptions;
use nfm_file_system_egui::{FileEvent, FilePicker};
use std::{io, path::PathBuf, sync::Arc};

/// Runs on its own thread after the picker closes and focus is restored; returned
/// errors are reported through SWM's error log.
pub type FileCallback = fn(PathBuf) -> io::Result<()>;

/// What happens to an accepted file.
#[derive(Clone, Copy, Debug)]
pub enum FileAction {
    /// Start the file as the desktop user, or elevated.
    Launch {
        elevated: bool,
    },
    Callback(FileCallback),
}
impl FileAction {
    fn call(self, path: String) -> RuntimeCall {
        match self {
            Self::Launch { elevated } => RuntimeCall::new(
                format!("launch {path}{}", if elevated { " elevated" } else { "" }),
                move |runtime| runtime.menu_program_selected(&path, elevated),
            ),
            Self::Callback(on_accept) => {
                RuntimeCall::new(format!("file callback {path}"), move |runtime| {
                    runtime.menu_closed();
                    std::thread::spawn(move || {
                        if let Err(error) = accept(on_accept, path.into()) {
                            crate::error_log::report("picker callback failed", error.to_string());
                        }
                    });
                })
            }
        }
    }
}

/// NFM filesystem browsing. A missing `bat_theme` uses `file_preview_bat_theme`;
/// empty roots list the drives.
#[derive(Clone, Debug)]
pub struct Files {
    pub options: FileSystemPickerOptions,
    pub action: FileAction,
}
impl Files {
    fn resolved(&self, configuration: &crate::Configuration) -> Files {
        let mut files = self.clone();
        if files.options.bat_theme.is_none() {
            files.options.bat_theme = configuration.file_preview_bat_theme.clone();
        }
        files
    }
}
impl PickerSource for Files {
    fn request(&self, configuration: &crate::Configuration) -> Box<dyn PickerRequest> {
        Box::new(self.resolved(configuration))
    }
}
impl PickerRequest for Files {
    fn build(self: Box<Self>, host: &PickerHost<'_>) -> Result<Box<dyn PickerView>, String> {
        let mut menu = FilePicker::with_shared_picker(host.ctx, self.options, host.config(), None)?;
        let resolve = host.file_preview_resolver.clone();
        menu.picker_mut().set_preview_provider(
            host.ctx,
            Arc::new(crate::file_preview::ResolvedPreviewProvider::new(
                move |selection: &nfm_egui::Selection<SharedItem>, cancel| {
                    let Some(path) = selection.item.downcast_ref::<String>() else {
                        return Ok(None);
                    };
                    resolve(std::path::Path::new(path), cancel)
                },
            )),
        );
        Ok(Box::new(FilesView {
            menu,
            action: self.action,
        }))
    }
}
struct FilesView {
    menu: FilePicker<SharedSource>,
    action: FileAction,
}
impl PickerView for FilesView {
    fn picker(&self) -> &SharedPicker {
        self.menu.picker()
    }
    fn has_header(&self) -> bool {
        false
    }
    fn show(
        &mut self,
        ctx: &egui::Context,
        placement: Placement,
        appearance: &Appearance,
        _: bool,
    ) -> Option<Outcome> {
        match self.menu.show(ctx, placement, appearance) {
            Ok(Some(FileEvent::Selected(path))) => Some(Outcome::Runtime(self.action.call(path))),
            Ok(Some(FileEvent::Cancelled)) => Some(Outcome::Cancelled),
            Ok(None) => None,
            Err(error) => Some(Outcome::Failed(error)),
        }
    }
}

/// Program launcher over the given directories: files only, no preview.
pub fn program_launcher(name: &'static str, directories: &[&str], elevated: bool) -> PickerConfig {
    PickerConfig::new(
        name,
        Files {
            options: FileSystemPickerOptions {
                roots: directories
                    .iter()
                    .map(|&directory| directory.into())
                    .collect(),
                max_depth: 5,
                files_only: true,
                preview_visible: false,
                ..Default::default()
            },
            action: FileAction::Launch { elevated },
        },
    )
}

fn accept(on_accept: FileCallback, path: PathBuf) -> io::Result<()> {
    if !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "selected file no longer exists",
        ));
    }
    on_accept(path)
}

#[cfg(test)]
mod accept_tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn requests_inherit_the_configured_bat_theme() {
        let mut configuration = crate::Configuration::default();
        configuration.file_preview_bat_theme = Some("gruvbox-dark".into());
        let files = Files {
            options: Default::default(),
            action: FileAction::Launch { elevated: false },
        };
        assert_eq!(
            files.resolved(&configuration).options.bat_theme.as_deref(),
            Some("gruvbox-dark")
        );
    }

    #[test]
    fn callback_receives_the_path_and_propagates_errors() {
        static SELECTED: Mutex<Option<PathBuf>> = Mutex::new(None);
        fn selected(path: PathBuf) -> io::Result<()> {
            *SELECTED.lock().unwrap() = Some(path);
            Ok(())
        }
        fn fail(_: PathBuf) -> io::Result<()> {
            Err(io::Error::from_raw_os_error(5))
        }
        let root =
            std::env::temp_dir().join(format!("swm-registered-picker-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("solution with spaces.SLN");
        std::fs::write(&path, "solution").unwrap();
        accept(selected, path.clone()).unwrap();
        assert_eq!(*SELECTED.lock().unwrap(), Some(path.clone()));
        assert!(accept(selected, root.clone()).is_err());
        assert_eq!(
            accept(fail, path.clone()).unwrap_err().raw_os_error(),
            Some(5)
        );
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}

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
    fn nfm_navigation_and_selection_work_with_swm_items() {
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
        drop(menu);
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
        drop(menu);
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
            None,
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
        drop(menu);
        let mut menu = FileMenu::with_shared_picker(
            &ctx,
            options,
            PickerConfig {
                result_rows: 7,
                preview_rows: 14,
                ..Default::default()
            },
            None,
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
