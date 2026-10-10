extern crate self as simple_window_manager_rs;

pub mod bar;
pub mod border;
pub mod cloak;
pub mod commands;
pub mod config;
pub mod drop_target;
mod egui_command_menu;
mod egui_data_menu;
mod egui_file_menu;
pub mod file_preview;
mod egui_native;
mod egui_process_menu;
pub mod egui_window_menu;
pub mod error_log;
mod input;
pub mod layout;
pub mod list_windows;
pub mod model;
pub mod platform;
pub mod platform_error;
pub mod process;
pub mod restore;
pub mod runtime;
pub mod snapshot;
pub mod status;
pub mod ui;
pub mod worker;

pub mod user_config {
    include!(env!("SWM_CONFIG_RS_RESOLVED"));
}

pub use config::Configuration;
pub use layout::Layout;
pub use model::{
    Client, ClientData, ClientId, Monitor, MonitorId, Rect, WindowManagerState, Workspace,
    WorkspaceId,
};
