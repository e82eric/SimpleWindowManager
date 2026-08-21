#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use simple_window_manager_rs::Configuration;

fn main() {
    simple_window_manager_rs::error_log::initialize();
    let startup = tracing::info_span!("startup", stage = tracing::field::Empty);
    let _startup = startup.enter();
    // The C entry point becomes DPI-aware before reading any monitor or window
    // geometry. Doing this later mixes virtualized and physical coordinates.
    startup.record("stage", "dpi_setup");
    unsafe {
        if windows_sys::Win32::UI::WindowsAndMessaging::SetProcessDPIAware() == 0 {
            simple_window_manager_rs::error_log::last_win32("SetProcessDPIAware failed");
        }
    }
    startup.record("stage", "single_instance");
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
    startup.record("stage", "configuration");
    let mut configuration = Configuration::default();
    simple_window_manager_rs::user_config::configure(&mut configuration);
    startup.record("stage", "desktop_discovery");
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
    startup.record("stage", "runtime_initialization");
    let runtime = simple_window_manager_rs::runtime::Runtime::initialize(configuration, snapshot);
    startup.record("stage", "run");
    if let Err(error) = simple_window_manager_rs::platform::run(runtime, instance_mutex) {
        simple_window_manager_rs::error_log::report_win32("GetMessageW failed", error);
    }
}
