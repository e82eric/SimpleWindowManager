//! The picker kinds SWM ships and its built-in pickers. Bind one with
//! `Action::ShowPicker(&builtin_pickers::WINDOWS)`.
pub use crate::egui_command_menu::{CommandList, Commands};
pub use crate::egui_data_menu::{Data, DataRow, DataTable};
pub use crate::egui_file_menu::{FileAction, FileCallback, Files, program_launcher};
pub use crate::egui_process_menu::Processes;
pub use crate::egui_switcher_menu::Windows;
use crate::picker::PickerConfig;
use crate::platform::{
    client_logs_table, configuration_state_table, error_logs_table, float_logs_table,
    services_table,
};

pub static WINDOWS: PickerConfig = PickerConfig {
    name: "ListWindowsMenu",
    source: &Windows,
};
pub static PROCESSES: PickerConfig = PickerConfig {
    name: "ProcessListMenu",
    source: &Processes,
};
pub static COMMANDS: PickerConfig = PickerConfig {
    name: "ListKeyBindings",
    source: &Commands,
};
pub static FILE_SYSTEM: PickerConfig = PickerConfig {
    name: "FileSystemMenu",
    source: &Files {
        options: nfm_file_system::FileSystemPickerOptions {
            roots: Vec::new(),
            max_depth: 5,
            directories_only: false,
            files_only: false,
            search_string: None,
            preview_visible: true,
            bat_theme: None,
        },
        action: FileAction::Launch { elevated: false },
    },
};
pub static SERVICES: PickerConfig = PickerConfig {
    name: "ServiceListMenu",
    source: &Data(services_table),
};
pub static FLOAT_LOGS: PickerConfig = PickerConfig {
    name: "FloatLogsMenu",
    source: &Data(float_logs_table),
};
pub static CLIENT_LOGS: PickerConfig = PickerConfig {
    name: "ClientLogsMenu",
    source: &Data(client_logs_table),
};
pub static ERROR_LOGS: PickerConfig = PickerConfig {
    name: "ErrorLogsMenu",
    source: &Data(error_logs_table),
};
pub static CONFIGURATION_STATE: PickerConfig = PickerConfig {
    name: "ConfigurationStateMenu",
    source: &Data(configuration_state_table),
};

pub(crate) fn defaults() -> Vec<&'static PickerConfig> {
    vec![
        &WINDOWS,
        &PROCESSES,
        &COMMANDS,
        &FILE_SYSTEM,
        &SERVICES,
        &FLOAT_LOGS,
        &CLIENT_LOGS,
        &ERROR_LOGS,
        &CONFIGURATION_STATE,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registering_keeps_every_picker_and_returns_the_listed_one() {
        let mut configuration = crate::Configuration::default();
        let builtins = configuration.pickers.len();
        let first = configuration.register_picker(program_launcher("Programs", &["C:/src"], true));
        let second = configuration.register_picker(program_launcher("Programs", &["C:/src"], true));
        assert_eq!(configuration.pickers.len(), builtins + 2);
        assert!(std::ptr::eq(first, configuration.pickers[builtins]));
        assert!(std::ptr::eq(second, configuration.pickers[builtins + 1]));
        assert!(format!("{first:?}").contains("elevated: true"));
        assert!(std::ptr::eq(&WINDOWS, configuration.pickers[0]));
    }
}
