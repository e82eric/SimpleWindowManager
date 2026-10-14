//! Register named pickers with source-specific options and typed acceptance.
use std::{io, path::PathBuf};

#[derive(Clone, Debug)]
pub enum PickerSource {
    /// NFM filesystem browsing with an optional editable initial query.
    Files(nfm_file_system::FileSystemPickerOptions),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickerSelection {
    File(PathBuf),
}

/// Runs on SWM's worker after the picker closes. Spawn separate work for lengthy
/// operations; returned errors are reported through SWM's error log.
pub type PickerCallback = fn(PickerSelection) -> io::Result<()>;

#[derive(Clone, Debug)]
pub struct PickerConfig {
    pub name: String,
    pub source: PickerSource,
    pub on_accept: PickerCallback,
}

impl PickerConfig {
    pub(crate) fn validate(&self) -> Result<(), String> {
        let PickerSource::Files(options) = &self.source;
        if options.roots.is_empty()
            || options
                .roots
                .iter()
                .any(|root| !std::path::Path::new(root).is_dir())
        {
            return Err("file picker roots must contain existing directories".into());
        }
        Ok(())
    }
}

pub(crate) fn accept(on_accept: PickerCallback, selection: PickerSelection) -> io::Result<()> {
    match &selection {
        PickerSelection::File(path) => {
            if !path.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "selected file no longer exists",
                ));
            }
        }
    }
    on_accept(selection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    #[test]
    fn registered_picker_delivers_typed_selection_and_propagates_callback_errors() {
        static SELECTED: Mutex<Option<PickerSelection>> = Mutex::new(None);
        fn selected(selection: PickerSelection) -> io::Result<()> {
            *SELECTED.lock().unwrap() = Some(selection);
            Ok(())
        }
        let root =
            std::env::temp_dir().join(format!("swm-registered-picker-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("solution with spaces.SLN");
        std::fs::write(&path, "solution").unwrap();
        let config = PickerConfig {
            name: "Solutions".into(),
            source: PickerSource::Files(nfm_file_system::FileSystemPickerOptions {
                roots: vec![root.to_string_lossy().into_owned()],
                files_only: true,
                search_string: Some(".sln$ ".into()),
                ..Default::default()
            }),
            on_accept: selected,
        };
        assert!(config.validate().is_ok());
        let mut configuration = crate::Configuration::default();
        let id = configuration.register_picker(config);
        assert!(crate::ui::is_menu(crate::commands::Action::ShowPicker(id)));
        accept(
            configuration.pickers[id].on_accept,
            PickerSelection::File(path.clone()),
        )
        .unwrap();
        assert_eq!(
            *SELECTED.lock().unwrap(),
            Some(PickerSelection::File(path.clone()))
        );
        assert!(
            accept(
                configuration.pickers[id].on_accept,
                PickerSelection::File(root.clone())
            )
            .is_err()
        );
        let other = root.join("other.txt");
        std::fs::write(&other, "other").unwrap();
        accept(
            configuration.pickers[id].on_accept,
            PickerSelection::File(other.clone()),
        )
        .unwrap();
        fn fail(_: PickerSelection) -> io::Result<()> {
            Err(io::Error::from_raw_os_error(5))
        }
        configuration.pickers[id].on_accept = fail;
        assert_eq!(
            accept(
                configuration.pickers[id].on_accept,
                PickerSelection::File(path.clone())
            )
            .unwrap_err()
            .raw_os_error(),
            Some(5)
        );
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(other).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
