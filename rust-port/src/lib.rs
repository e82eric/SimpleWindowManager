extern crate self as simple_window_manager_rs;

pub mod bar;
pub mod border;
pub mod cloak;
pub mod commands;
pub mod config;
pub mod drop_target;
pub mod error_log;
pub mod layout;
pub mod list_windows;
pub mod model;
pub mod platform;
pub mod process;
pub mod restore;
pub mod runtime;
pub mod snapshot;
pub mod status;

pub mod user_config {
    include!(env!("SWM_CONFIG_RS_RESOLVED"));
}

pub use config::Configuration;
pub use layout::Layout;
pub use model::{
    Client, ClientData, ClientId, Monitor, MonitorId, Rect, WindowManagerState, Workspace,
    WorkspaceId,
};
