#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use simple_window_manager_rs::{Configuration, configure};

fn main() {
    // The C entry point becomes DPI-aware before reading any monitor or window
    // geometry. Doing this later mixes virtualized and physical coordinates.
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SetProcessDPIAware();
    }
    let instance_mutex = match simple_window_manager_rs::platform::acquire_single_instance() {
        Ok(Some(handle)) => handle,
        Ok(None) => return,
        Err(error) => {
            simple_window_manager_rs::error_log::report_win32(
                "single-instance initialization failed",
                error,
            );
            return;
        }
    };
    let mut configuration = Configuration::default();
    configure(&mut configuration);
    let snapshot =
        match simple_window_manager_rs::snapshot::capture(configuration.float_uwp_windows) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                simple_window_manager_rs::error_log::report_win32(
                    "desktop discovery failed",
                    error,
                );
                return;
            }
        };
    let runtime = simple_window_manager_rs::runtime::Runtime::initialize(configuration, snapshot);
    if let Err(error) = simple_window_manager_rs::platform::run(runtime, instance_mutex) {
        simple_window_manager_rs::error_log::report_win32("GetMessageW failed", error);
    }
}
