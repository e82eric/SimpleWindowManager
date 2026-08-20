//! The only module that should directly expose Win32 callback and handle unsafety.

use std::backtrace::Backtrace;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::runtime::Runtime;

pub(crate) fn consume_picker_task<T, F>(task: rust_nfm::runtime::PickerTask<T>, completed: F)
where
    T: Send + 'static,
    F: FnOnce(rust_nfm::request::PickerResponse<T>) + Send + 'static,
{
    let parent = tracing::Span::current();
    std::thread::spawn(move || {
        let _entered = parent.enter();
        let response = task
            .recv()
            .unwrap_or_else(|error| rust_nfm::request::PickerResponse::Error(error.to_string()));
        if let rust_nfm::request::PickerResponse::Error(error) = &response {
            crate::error_log::report("picker returned an error", error.to_string());
        }
        completed(response);
    });
}

static RUNTIME: OnceLock<Mutex<Runtime>> = OnceLock::new();

static BORDER_LOST_FOCUS: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_border_lost_focus(value: bool) {
    BORDER_LOST_FOCUS.store(value, Ordering::Relaxed);
}

pub(crate) fn border_is_lost_focus() -> bool {
    BORDER_LOST_FOCUS.load(Ordering::Relaxed)
}

static SUPPRESSED_KEY_UPS: Mutex<[bool; 256]> = Mutex::new([false; 256]);

static WORKSPACE_MOVE_TIMER: Mutex<usize> = Mutex::new(0);

unsafe extern "system" fn finish_workspace_move_focus_preservation(
    _hwnd: windows_sys::Win32::Foundation::HWND,
    _message: u32,
    timer_id: usize,
    _time: u32,
) {
    let span = tracing::info_span!(
        "finish_workspace_move_focus_preservation",
        timer_id = timer_id as u64
    );
    let _entered = span.enter();
    use windows_sys::Win32::UI::WindowsAndMessaging::KillTimer;
    if unsafe { KillTimer(std::ptr::null_mut(), timer_id) } == 0 {
        crate::error_log::last_win32("KillTimer(workspace focus callback) failed");
    }
    let mut current = WORKSPACE_MOVE_TIMER.lock().unwrap();
    if *current == timer_id {
        *current = 0;
    }
    drop(current);
    if let Some(runtime) = RUNTIME.get()
        && let Ok(mut runtime) = runtime.try_lock()
    {
        runtime.finish_workspace_move_focus_preservation();
    }
}

pub(crate) fn schedule_workspace_move_focus_finish() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};
    let mut current = WORKSPACE_MOVE_TIMER.lock().unwrap();
    if *current != 0 {
        if unsafe { KillTimer(std::ptr::null_mut(), *current) } == 0 {
            crate::error_log::last_win32("KillTimer(replace workspace focus timer) failed");
        }
        *current = 0;
    }
    let timer = unsafe {
        SetTimer(
            std::ptr::null_mut(),
            0,
            250,
            Some(finish_workspace_move_focus_preservation),
        )
    };
    *current = timer;
    if timer == 0 {
        crate::error_log::last_win32("SetTimer(workspace focus) failed");
    }
    timer != 0
}

pub(crate) unsafe extern "C" fn menu_window_selected(hwnd: isize, _state: *mut std::ffi::c_void) {
    if let Some(runtime) = RUNTIME.get() {
        if let Ok(mut runtime) = runtime.try_lock() {
            runtime.menu_window_selected(hwnd);
        }
    }
}

pub(crate) unsafe extern "C" fn menu_program_selected(
    value: *mut std::ffi::c_char,
    state: *mut std::ffi::c_void,
) {
    if value.is_null() {
        return;
    }
    let value = unsafe { std::ffi::CStr::from_ptr(value) }.to_string_lossy();
    if let Some(runtime) = RUNTIME.get() {
        if let Ok(mut runtime) = runtime.try_lock() {
            runtime.menu_program_selected(&value, state as usize != 0);
        }
    }
}

pub(crate) unsafe extern "C" fn menu_on_closed() {
    if let Some(runtime) = RUNTIME.get() {
        if let Ok(mut runtime) = runtime.try_lock() {
            runtime.menu_closed();
        }
    }
}

pub(crate) unsafe extern "C" fn menu_string_noop(
    _value: *mut std::ffi::c_char,
    _state: *mut std::ffi::c_void,
) {
    unsafe { menu_on_closed() };
}

fn command_selected(name: &str) {
    if let Some(runtime) = RUNTIME.get() {
        if let Ok(mut runtime) = runtime.try_lock() {
            let action = runtime
                .configuration
                .commands
                .iter()
                .find(|command| command.name == name)
                .map(|command| command.action);
            // Close the commands picker before executing the selection. Menu-opening
            // actions otherwise create their picker and then immediately hide it.
            runtime.menu_hide();
            if let Some(action) = action {
                if let Err(error) = runtime.execute(action) {
                    crate::error_log::report_win32(format!("command {action:?} failed"), error);
                }
            }
        }
    }
}

pub(crate) fn show_commands_menu(runtime: &mut Runtime) {
    let columns = ["Name", "Type", "KeyBinding", "Description"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let rows = runtime
        .configuration
        .commands
        .iter()
        .map(|command| {
            vec![
                command.name.clone(),
                command_type(command.action).to_owned(),
                key_binding_text(command.modifiers, command.key),
                command_description(runtime, command.action),
            ]
        })
        .collect();
    let monitor = &runtime.configuration.state.monitors[0];
    let Some(menu) = &runtime.rust_nfm else {
        return;
    };
    menu.set_preferred_center(
        monitor.x_offset + monitor.width / 2,
        (monitor.top + monitor.bottom) / 2,
    );
    match menu.show_structured(columns, rows, None) {
        Ok(task) => consume_picker_task(task, |response| match response {
            rust_nfm::request::PickerResponse::Selected(item) => {
                if let Some(name) = item.fields.get("Name") {
                    command_selected(name);
                } else {
                    unsafe { menu_on_closed() };
                }
            }
            _ => unsafe { menu_on_closed() },
        }),
        Err(error) => {
            crate::error_log::report("failed to start commands picker", format!("{error:#}"));
            return;
        }
    }
    runtime.configuration.state.menu_visible = true;
}

pub(crate) fn binding_name(binding: &crate::commands::KeyBinding) -> String {
    use crate::commands::Action::*;
    let suffix = |i: usize| {
        if binding.key >= 0x7c {
            format!("F{}", i + 13)
        } else if i == 9 {
            "0".into()
        } else {
            (i + 1).to_string()
        }
    };
    match binding.action {
        QuitAndRestore => "quit_and_restore_windows".into(),
        Quit => "quit".into(),
        SelectDown => "select_window_down".into(),
        SelectUp => "select_window_up".into(),
        SelectRight => "select_window_right".into(),
        SelectLeft => "select_window_left".into(),
        SelectNextMonitor => "monitor_select_next".into(),
        ResetLayout => "arrange_clients_in_selected_workspace".into(),
        MoveToMain => "move_focused_window_to_main".into(),
        MinimizeFocused => "minimize_focused_window".into(),
        SelectWorkspace(i) => format!("swap_selected_monitor_to[{}]", suffix(i)),
        MoveToWorkspace(i) => format!("move_focused_window_to_workspace[{}]", suffix(i)),
        MoveToSelectedWorkspace => {
            if binding.key >= 0x7c {
                "move_focused_window_to_selected_monitor_workspace[F22]".into()
            } else {
                "move_focused_window_to_selected_monitor_workspace".into()
            }
        }
        MoveDown => "move_focused_client_down".into(),
        MoveUp => "move_focused_client_up".into(),
        MoveRight => "move_focused_client_right".into(),
        MoveLeft => "move_focused_client_left".into(),
        CloseFocused => "close_focused_window".into(),
        KillFocused => "kill_focused_window".into(),
        ToggleTaskbar => "taskbar_toggle".into(),
        ToggleFilteredCurrent => "toggle_create_window_in_current_workspace".into(),
        ToggleUnfilteredCurrent => "toggle_ignore_workspace_filters".into(),
        ToggleUnmatchedCurrent => {
            "toggle_non_filtered_windows_assigned_to_current_workspace".into()
        }
        StopManaging => "client_stop_managing".into(),
        SetMonocle => "swap_selected_monitor_to_monacle_layout".into(),
        SetDeck => "swap_selected_monitor_to_deck_layout".into(),
        SetGrid => "swap_selected_monitor_to_grid_layout".into(),
        SetTile => "swap_selected_monitor_to_tile_layout".into(),
        SetTileReversed => "swap_selected_monitor_to_tile_layout_reversed".into(),
        RedrawFocused => "redraw_focused_window".into(),
        ShowWindows => "ListWindowsMenu".into(),
        ShowProcesses => "ProcessListMenu".into(),
        ShowServices => "ServiceListMenu".into(),
        ShowFileSystem => "FileSystemMenu".into(),
        ShowCommands => "ListKeyBindings".into(),
        ShowFloatLogs => "FloatLogsMenu".into(),
        ShowClientLogs => "ClientLogsMenu".into(),
        ShowErrorLogs => "ErrorLogsMenu".into(),
        RunLastMenu => "LastMenu".into(),
        ShowPrograms(true) => "ProgramLauncherMenu_elevated".into(),
        ShowPrograms(false) => "ProgramLauncherMenu_not_elevated".into(),
        Launch(_) => "Shell".into(),
        MoveFloat(1, 0) => "move_focused_window_right".into(),
        MoveFloat(-1, 0) => "move_focused_window_left".into(),
        MoveFloat(0, -1) => "move_focused_window_up".into(),
        MoveFloat(_, _) => "move_focused_window_down".into(),
        MoveFloatToMonitor(_) => "move_focused_window_to_monitor".into(),
        MoveWorkspaceToMonitor(i, _) => format!(
            "move_workspace_to_secondary_monitor_without_focus[{}]",
            i + 1
        ),
        MoveSecondaryFocusedToMain => "move_secondary_monitor_focused_window_to_main".into(),
    }
}

fn command_type(action: crate::commands::Action) -> &'static str {
    use crate::commands::Action::*;
    match action {
        SelectWorkspace(_) | MoveToWorkspace(_) | MoveWorkspaceToMonitor(_, _) => {
            "WorkspaceFunction"
        }
        MoveFloatToMonitor(_) => "MonitorFunction",
        ShowPrograms(_) => "ProgramLauncherFunction",
        Launch(_) => "Shell",
        _ => "Function",
    }
}

fn command_description(runtime: &Runtime, action: crate::commands::Action) -> String {
    use crate::commands::Action::*;
    match action {
        SelectWorkspace(i) | MoveToWorkspace(i) | MoveWorkspaceToMonitor(i, _) => runtime
            .configuration
            .state
            .workspaces
            .get(i)
            .map(|workspace| workspace.name.clone())
            .unwrap_or_default(),
        MoveFloatToMonitor(i) => (i + 1).to_string(),
        ShowPrograms(elevated) => format!(
            "ProgramLauncher({})",
            if elevated { "elevated" } else { "not elevated" }
        ),
        Launch(command) => command.into(),
        _ => String::new(),
    }
}

fn key_binding_text(modifiers: Option<i32>, key: Option<u32>) -> String {
    use crate::commands::{LALT, LCTL, LSHIFT, LWIN};
    let (Some(modifiers), Some(key)) = (modifiers, key) else {
        return String::new();
    };
    let mut parts = Vec::new();
    if modifiers & LALT != 0 {
        parts.push("ALT".into());
    }
    if modifiers & LCTL != 0 {
        parts.push("CTL".into());
    }
    if modifiers & LWIN != 0 {
        parts.push("WIN".into());
    }
    if modifiers & LSHIFT != 0 {
        parts.push("SHIFT".into());
    }
    parts.push(key_name(key));
    parts.join("+")
}

fn key_name(key: u32) -> String {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyNameTextW, MAPVK_VK_TO_VSC, MapVirtualKeyW,
    };
    const VK_F1_VALUE: u32 = 0x70;
    const VK_F24_VALUE: u32 = 0x87;
    if (VK_F1_VALUE..=VK_F24_VALUE).contains(&key) {
        return format!("F{}", key - VK_F1_VALUE + 1);
    }
    let mut scan = unsafe { MapVirtualKeyW(key, MAPVK_VK_TO_VSC) };
    if scan == 0 {
        return format!("VK_{key:02X}");
    }
    if matches!(
        key,
        0x21 | 0x22 | 0x23 | 0x24 | 0x25 | 0x26 | 0x27 | 0x28 | 0x2d | 0x2e | 0x6f | 0x90
    ) {
        scan |= 0x100;
    }
    let mut buffer = [0u16; 256];
    unsafe { windows_sys::Win32::Foundation::SetLastError(0) };
    let length = unsafe { GetKeyNameTextW((scan << 16) as i32, buffer.as_mut_ptr(), 256) };
    if length == 0 {
        let error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        if error == 0 {
            crate::error_log::report("GetKeyNameTextW failed", "returned zero");
        } else {
            crate::error_log::report_win32("GetKeyNameTextW failed", error);
        }
        return format!("VK_{key:02X}");
    }
    String::from_utf16_lossy(&buffer[..length.max(0) as usize])
}

#[cfg(test)]
mod key_name_tests {
    #[test]
    fn formats_all_function_keys_without_scan_code_lookup() {
        assert_eq!(super::key_name(0x70), "F1");
        assert_eq!(super::key_name(0x7c), "F13");
        assert_eq!(super::key_name(0x83), "F20");
        assert_eq!(super::key_name(0x84), "F21");
        assert_eq!(super::key_name(0x87), "F24");
    }
}

pub(crate) fn show_float_logs_menu(_runtime: &mut Runtime) {
    let rows: Vec<Vec<String>> = _runtime
        .configuration
        .state
        .float_log_buffer
        .iter()
        .map(|entry| {
            vec![
                local_log_time(entry.timestamp),
                if entry.is_floated { "FLOAT" } else { "TILE" }.into(),
                entry.process_image_name.clone(),
                entry.class_name.clone(),
                entry.title.clone(),
                format_window_styles(entry.styles),
                format_extended_styles(entry.ex_styles),
                entry.reason.clone(),
                format!("0x{:p}", entry.hwnd as *const ()),
                entry.process_id.to_string(),
                entry.window_width.to_string(),
                entry.window_height.to_string(),
            ]
        })
        .collect();
    let rows = if rows.is_empty() {
        vec![vec![
            "".into(),
            "".into(),
            "No float decisions logged yet".into(),
            "".into(),
            "".into(),
            "".into(),
            "".into(),
            "".into(),
            "".into(),
            "".into(),
            "".into(),
            "".into(),
        ]]
    } else {
        rows
    };
    show_array_columns(
        _runtime,
        rows,
        &[
            "Time",
            "Action",
            "Process",
            "Class",
            "Title",
            "Styles",
            "ExStyles",
            "Reason",
            "HWND",
            "ProcessId",
            "Width",
            "Height",
        ],
        &[0, 1, 2, 3, 4, 7],
    );
}

pub(crate) fn show_client_logs_menu(_runtime: &mut Runtime) {
    let rows: Vec<Vec<String>> = _runtime
        .configuration
        .state
        .client_log_buffer
        .iter()
        .map(|entry| {
            vec![
                local_log_time(entry.timestamp),
                if entry.was_minimized { "MIN" } else { "NORM" }.into(),
                "TILE".into(),
                entry.process_image_name.clone(),
                entry.class_name.clone(),
                entry.workspace_name.clone(),
                entry.title.clone(),
                format!("0x{:p}", entry.hwnd as *const ()),
                entry.process_id.to_string(),
                if entry.was_minimized { "TRUE" } else { "FALSE" }.into(),
                "FALSE".into(),
                format_window_styles(entry.styles),
                format_extended_styles(entry.ex_styles),
                entry.window_width.to_string(),
                entry.window_height.to_string(),
            ]
        })
        .collect();
    let rows = if rows.is_empty() {
        let mut row = vec![String::new(); 15];
        row[0] = "No clients logged yet".into();
        vec![row]
    } else {
        rows
    };
    show_array_columns(
        _runtime,
        rows,
        &[
            "Time",
            "State",
            "Layout",
            "Process",
            "Class",
            "Workspace",
            "Title",
            "HWND",
            "ProcessId",
            "WasMinimized",
            "IsFloated",
            "Styles",
            "ExStyles",
            "Width",
            "Height",
        ],
        &[0, 1, 2, 3, 4, 5, 6],
    );
}

pub(crate) fn show_error_logs_menu(runtime: &mut Runtime) {
    let rows: Vec<Vec<String>> = crate::error_log::entries()
        .into_iter()
        .rev()
        .map(|entry| {
            let properties = entry
                .frames
                .iter()
                .flat_map(|frame| {
                    frame
                        .fields
                        .iter()
                        .map(move |(name, value)| format!("{}.{}={value}", frame.method, name))
                })
                .collect::<Vec<_>>()
                .join(" | ");
            let source = entry
                .line
                .map_or(entry.file.clone(), |line| format!("{}:{line}", entry.file));
            vec![
                local_log_time(entry.timestamp),
                entry.entry_method,
                entry.current_method,
                entry.context,
                entry.code.map(|code| code.to_string()).unwrap_or_default(),
                entry.message,
                properties,
                source,
            ]
        })
        .collect();
    let rows = if rows.is_empty() {
        let log_path = crate::error_log::log_file_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "File logging unavailable: LOCALAPPDATA is not set".into());
        vec![vec![
            String::new(),
            String::new(),
            String::new(),
            "No runtime errors logged".into(),
            String::new(),
            String::new(),
            String::new(),
            log_path,
        ]]
    } else {
        rows
    };
    show_array_columns(
        runtime,
        rows,
        &[
            "Time",
            "Entry",
            "Current",
            "Context",
            "Code",
            "Message",
            "Properties",
            "Source",
        ],
        &[0, 1, 2, 3, 4, 5, 6, 7],
    );
}

fn show_array_columns(
    runtime: &mut Runtime,
    rows: Vec<Vec<String>>,
    columns: &[&str],
    display: &[i32],
) {
    let visible_columns: Vec<usize> = display.iter().map(|column| *column as usize).collect();
    if let Some(menu) = &runtime.rust_nfm {
        let monitor = &runtime.configuration.state.monitors[0];
        menu.set_preferred_center(
            monitor.x_offset + monitor.width / 2,
            (monitor.top + monitor.bottom) / 2,
        );
        let picker_columns = visible_columns
            .iter()
            .filter_map(|index| columns.get(*index))
            .map(|column| (*column).to_owned())
            .collect::<Vec<_>>();
        let picker_rows = rows
            .iter()
            .map(|row| {
                visible_columns
                    .iter()
                    .map(|index| row.get(*index).cloned().unwrap_or_default())
                    .collect::<Vec<_>>()
            })
            .collect();
        match menu.show_structured(picker_columns, picker_rows, None) {
            Ok(task) => consume_picker_task(task, |_| unsafe { menu_on_closed() }),
            Err(error) => {
                crate::error_log::report("failed to start structured picker", format!("{error:#}"));
                return;
            }
        }
        runtime.configuration.state.menu_visible = true;
        return;
    }
    let lines = rows
        .into_iter()
        .map(|row| {
            visible_columns
                .iter()
                .filter_map(|index| {
                    let name = columns.get(*index)?;
                    let value = row.get(*index)?;
                    Some(format!("{name}: {}", value.replace(['\r', '\n'], " ")))
                })
                .collect::<Vec<_>>()
                .join(" | ")
        })
        .collect();
    let header = visible_columns
        .iter()
        .filter_map(|index| columns.get(*index).copied())
        .collect::<Vec<_>>()
        .join(" | ");
    show_string_items(runtime, &header, lines, menu_string_noop);
}

fn local_log_time(value: std::time::SystemTime) -> String {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows_sys::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};
    let ticks = value
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        / 100
        + 116_444_736_000_000_000;
    let file = FILETIME {
        dwLowDateTime: ticks as u32,
        dwHighDateTime: (ticks >> 32) as u32,
    };
    let mut utc: SYSTEMTIME = unsafe { zeroed() };
    let mut local: SYSTEMTIME = unsafe { zeroed() };
    if unsafe { FileTimeToSystemTime(&file, &mut utc) } == 0 {
        crate::error_log::last_win32("FileTimeToSystemTime(log timestamp) failed");
    }
    if unsafe { SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) } == 0 {
        crate::error_log::last_win32("SystemTimeToTzSpecificLocalTime(log timestamp) failed");
    }
    format!(
        "{:02}:{:02}:{:02}",
        local.wHour, local.wMinute, local.wSecond
    )
}

fn format_window_styles(styles: i32) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let mut names = Vec::new();
    if styles & (WS_POPUP | WS_CHILD) as i32 == 0 {
        names.push("WS_OVERLAPPED");
    }
    for (flag, name) in [
        (WS_POPUP, "WS_POPUP"),
        (WS_CHILD, "WS_CHILD"),
        (WS_MINIMIZE, "WS_MINIMIZE"),
        (WS_VISIBLE, "WS_VISIBLE"),
        (WS_DISABLED, "WS_DISABLED"),
        (WS_CLIPSIBLINGS, "WS_CLIPSIBLINGS"),
        (WS_CLIPCHILDREN, "WS_CLIPCHILDREN"),
        (WS_MAXIMIZE, "WS_MAXIMIZE"),
        (WS_CAPTION, "WS_CAPTION"),
        (WS_BORDER, "WS_BORDER"),
        (WS_DLGFRAME, "WS_DLGFRAME"),
        (WS_VSCROLL, "WS_VSCROLL"),
        (WS_HSCROLL, "WS_HSCROLL"),
        (WS_SYSMENU, "WS_SYSMENU"),
        (WS_THICKFRAME, "WS_THICKFRAME"),
        (WS_MINIMIZEBOX, "WS_MINIMIZEBOX"),
        (WS_MAXIMIZEBOX, "WS_MAXIMIZEBOX"),
        (WS_TABSTOP, "WS_TABSTOP"),
        (WS_GROUP, "WS_GROUP"),
    ] {
        if styles & flag as i32 != 0 {
            names.push(name);
        }
    }
    if names.is_empty() {
        "NONE".into()
    } else {
        names.join("\n    ")
    }
}
fn format_extended_styles(styles: i32) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let mut names = Vec::new();
    for (flag, name) in [
        (WS_EX_DLGMODALFRAME, "WS_EX_DLGMODALFRAME"),
        (WS_EX_NOPARENTNOTIFY, "WS_EX_NOPARENTNOTIFY"),
        (WS_EX_TOPMOST, "WS_EX_TOPMOST"),
        (WS_EX_ACCEPTFILES, "WS_EX_ACCEPTFILES"),
        (WS_EX_TRANSPARENT, "WS_EX_TRANSPARENT"),
        (WS_EX_MDICHILD, "WS_EX_MDICHILD"),
        (WS_EX_TOOLWINDOW, "WS_EX_TOOLWINDOW"),
        (WS_EX_WINDOWEDGE, "WS_EX_WINDOWEDGE"),
        (WS_EX_CLIENTEDGE, "WS_EX_CLIENTEDGE"),
        (WS_EX_CONTEXTHELP, "WS_EX_CONTEXTHELP"),
        (WS_EX_RIGHT, "WS_EX_RIGHT"),
        (WS_EX_RTLREADING, "WS_EX_RTLREADING"),
        (WS_EX_LEFTSCROLLBAR, "WS_EX_LEFTSCROLLBAR"),
        (WS_EX_CONTROLPARENT, "WS_EX_CONTROLPARENT"),
        (WS_EX_STATICEDGE, "WS_EX_STATICEDGE"),
        (WS_EX_APPWINDOW, "WS_EX_APPWINDOW"),
        (WS_EX_LAYERED, "WS_EX_LAYERED"),
        (WS_EX_NOINHERITLAYOUT, "WS_EX_NOINHERITLAYOUT"),
        (WS_EX_NOREDIRECTIONBITMAP, "WS_EX_NOREDIRECTIONBITMAP"),
        (WS_EX_LAYOUTRTL, "WS_EX_LAYOUTRTL"),
        (WS_EX_COMPOSITED, "WS_EX_COMPOSITED"),
        (WS_EX_NOACTIVATE, "WS_EX_NOACTIVATE"),
    ] {
        if styles & flag as i32 != 0 {
            names.push(name);
        }
    }
    if names.is_empty() {
        "NONE".into()
    } else {
        names.join("\n    ")
    }
}

pub(crate) fn show_services_menu(runtime: &mut Runtime) {
    let lines = crate::process::list_services().unwrap_or_else(|error| {
        crate::error_log::report_win32("unable to enumerate services", error);
        vec![format!("Unable to enumerate services: {error}")]
    });
    show_string_items(runtime, "State      Service", lines, menu_string_noop);
}

fn show_string_items(
    runtime: &mut Runtime,
    _header: &str,
    mut lines: Vec<String>,
    select: unsafe extern "C" fn(*mut std::ffi::c_char, *mut std::ffi::c_void),
) {
    if lines.is_empty() {
        lines.push("No entries logged yet".into());
    }
    let rust_values = lines;
    let monitor = &runtime.configuration.state.monitors[0];
    if let Some(menu) = &runtime.rust_nfm {
        menu.set_preferred_center(
            monitor.x_offset + monitor.width / 2,
            (monitor.top + monitor.bottom) / 2,
        );
        match menu.show_items(rust_values, None) {
            Ok(task) => consume_picker_task(task, move |response| match response {
                rust_nfm::request::PickerResponse::Selected(value) => {
                    if let Ok(value) = std::ffi::CString::new(value) {
                        unsafe { select(value.as_ptr() as *mut _, std::ptr::null_mut()) };
                    }
                }
                _ => unsafe { menu_on_closed() },
            }),
            Err(error) => {
                crate::error_log::report("failed to start item picker", format!("{error:#}"));
                return;
            }
        }
        runtime.configuration.state.menu_visible = true;
    }
}

pub(crate) fn bar_snapshot(monitor: usize) -> Option<crate::bar::BarSnapshot> {
    RUNTIME.get()?.try_lock().ok()?.bar_snapshot(monitor)
}

pub(crate) fn bar_workspace_clicked(monitor: usize, workspace: usize) {
    if let Some(runtime) = RUNTIME.get() {
        if let Ok(mut runtime) = runtime.try_lock() {
            runtime.bar_workspace_clicked(monitor, workspace);
        }
    }
}

pub(crate) fn bar_commands_clicked() {
    if let Some(runtime) = RUNTIME.get() {
        if let Ok(mut runtime) = runtime.try_lock() {
            runtime.run_commands_menu();
        }
    }
}

fn get_modifiers_pressed() -> i32 {
    use crate::commands::{LALT, LCTL, LSHIFT, LWIN, RALT, RSHIFT, RWIN};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    let down = |key| unsafe { GetAsyncKeyState(key) as u16 & 0x8000 != 0 };
    let mut result = 0;
    if down(VK_LSHIFT as i32) {
        result |= LSHIFT;
    }
    if down(VK_RSHIFT as i32) {
        result |= RSHIFT;
    }
    if down(VK_LMENU as i32) {
        result |= LALT;
    }
    if down(VK_RMENU as i32) {
        result |= RALT;
    }
    if down(VK_CONTROL as i32) {
        result |= LCTL;
    }
    if down(VK_LWIN as i32) {
        result |= LWIN;
    }
    if down(VK_RWIN as i32) {
        result |= RWIN;
    }
    result
}

unsafe extern "system" fn handle_key_press(code: i32, wparam: usize, lparam: isize) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };
    if code == 0 {
        let event = wparam as u32;
        let key = unsafe { (*(lparam as *const KBDLLHOOKSTRUCT)).vkCode };
        if event == WM_KEYUP || event == WM_SYSKEYUP {
            if key < 256 {
                let mut suppressed = SUPPRESSED_KEY_UPS.lock().unwrap();
                if suppressed[key as usize] {
                    suppressed[key as usize] = false;
                    return 1;
                }
            }
        } else if event == WM_KEYDOWN || event == WM_SYSKEYDOWN {
            if let Some(runtime) = RUNTIME.get() {
                if let Ok(mut runtime) = runtime.try_lock() {
                    if runtime.key_bindings_try_handle(key, get_modifiers_pressed()) {
                        if key < 256 {
                            SUPPRESSED_KEY_UPS.lock().unwrap()[key as usize] = true;
                        }
                        return 1;
                    }
                }
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

unsafe extern "system" fn handle_mouse(code: i32, wparam: usize, lparam: isize) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{CallNextHookEx, WM_LBUTTONUP};
    if code >= 0 && wparam as u32 == WM_LBUTTONUP {
        if let Some(runtime) = RUNTIME.get() {
            if let Ok(mut runtime) = runtime.try_lock() {
                let _ = runtime.drag_drop_try_handle_left_mouse_up();
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

unsafe extern "system" fn handle_windows_event(
    _hook: windows_sys::Win32::UI::Accessibility::HWINEVENTHOOK,
    event: u32,
    hwnd: windows_sys::Win32::Foundation::HWND,
    id_object: i32,
    id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{CHILDID_SELF, OBJID_WINDOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE, EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_SHOW,
        EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZESTART,
    };
    if hwnd.is_null() || id_child != CHILDID_SELF as i32 || id_object != OBJID_WINDOW {
        return;
    }
    let Some(runtime) = RUNTIME.get() else {
        return;
    };
    let Ok(mut runtime) = runtime.try_lock() else {
        return;
    };
    let raw = hwnd as isize;

    // if !runtime.is_root_window(raw) {
    //     debug_out(&format!("removing {raw:#X}"));
    //     let _ = runtime.remove_client_by_hwnd(raw);
    //     return;
    // }
    let result = match event {
        EVENT_OBJECT_DESTROY => {
            if let Err(error) = runtime.remove_client_by_hwnd(raw) {
                report_win_event_error(
                    event,
                    raw,
                    id_object,
                    id_child,
                    "client removal failed",
                    error,
                );
            }
            Ok(())
        }
        EVENT_OBJECT_HIDE => {
            if is_taskbar(hwnd)
                && let Err(error) = runtime.taskbar_changed(raw)
            {
                report_win_event_error(
                    event,
                    raw,
                    id_object,
                    id_child,
                    "taskbar resize failed",
                    error,
                );
            }
            use windows_sys::Win32::UI::WindowsAndMessaging::{GWL_STYLE, WS_VISIBLE};
            if platform_window_long(hwnd, GWL_STYLE) & WS_VISIBLE as i32 == 0 {
                runtime.remove_client_by_hwnd(raw)
            } else {
                Ok(())
            }
        }
        EVENT_OBJECT_SHOW | EVENT_OBJECT_UNCLOAKED if is_taskbar(hwnd) => {
            runtime.taskbar_changed(raw)
        }
        EVENT_OBJECT_SHOW | EVENT_OBJECT_UNCLOAKED => runtime.refresh_window(raw),
        EVENT_SYSTEM_MINIMIZESTART | EVENT_OBJECT_LOCATIONCHANGE => runtime.location_changed(raw),
        EVENT_SYSTEM_FOREGROUND => {
            runtime.foreground_changed(raw);
            Ok(())
        }
        _ => Ok(()),
    };
    if let Err(error) = result {
        report_win_event_error(event, raw, id_object, id_child, "WinEvent failed", error);
    }
}

fn report_win_event_error(
    event: u32,
    hwnd: isize,
    object: i32,
    child: i32,
    operation: &str,
    error: u32,
) {
    let span = tracing::error_span!(
        "handle_windows_event",
        event,
        hwnd = hwnd as usize,
        object,
        child
    );
    let _entered = span.enter();
    crate::error_log::report_win32(operation, error);
}

fn platform_window_long(hwnd: windows_sys::Win32::Foundation::HWND, index: i32) -> i32 {
    use windows_sys::Win32::Foundation::{GetLastError, SetLastError};
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongW;
    unsafe {
        SetLastError(0);
        let value = GetWindowLongW(hwnd, index);
        let error = GetLastError();
        if value == 0 && error != 0 {
            crate::error_log::report_win32(format!("GetWindowLongW({index}) failed"), error);
        }
        value
    }
}

fn is_taskbar(hwnd: windows_sys::Win32::Foundation::HWND) -> bool {
    let mut buffer = [0u16; 256];
    let length = unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::GetClassNameW(
            hwnd,
            buffer.as_mut_ptr(),
            buffer.len() as i32,
        )
    };
    if length <= 0 {
        crate::error_log::last_win32("GetClassNameW(taskbar) failed");
        return false;
    }
    matches!(
        String::from_utf16_lossy(&buffer[..length as usize]).as_str(),
        "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    )
}

fn install_win_event_hooks() -> Vec<windows_sys::Win32::UI::Accessibility::HWINEVENTHOOK> {
    use windows_sys::Win32::UI::Accessibility::SetWinEventHook;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE, EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_SHOW,
        EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZESTART,
        WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
    };
    let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
    let ranges = [
        (EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE),
        (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZESTART),
        (EVENT_OBJECT_DESTROY, EVENT_OBJECT_SHOW),
        (EVENT_OBJECT_HIDE, EVENT_OBJECT_HIDE),
        (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
        (EVENT_OBJECT_UNCLOAKED, EVENT_OBJECT_UNCLOAKED),
    ];
    ranges
        .into_iter()
        .filter_map(|(min, max)| {
            let hook = unsafe {
                SetWinEventHook(
                    min,
                    max,
                    std::ptr::null_mut(),
                    Some(handle_windows_event),
                    0,
                    0,
                    flags,
                )
            };
            if hook.is_null() {
                crate::error_log::last_win32(format!("SetWinEventHook({min:#x}-{max:#x}) failed"));
            }
            (!hook.is_null()).then_some(hook)
        })
        .collect()
}

#[tracing::instrument]
pub fn run_message_loop() -> Result<(), u32> {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, TranslateMessage,
    };
    unsafe {
        let mut message: MSG = zeroed();
        loop {
            let result = GetMessageW(&mut message, 0 as HWND, 0, 0);
            if result == -1 {
                let error = windows_sys::Win32::Foundation::GetLastError();
                crate::error_log::report_win32("GetMessageW failed", error);
                return Err(error);
            }
            if result == 0 {
                return Ok(());
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

#[tracing::instrument]
pub fn acquire_single_instance() -> Result<Option<isize>, u32> {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_OK, MessageBoxW};
    let mutex_name: Vec<u16> = "SimpleWindowManagerSingleInstanceLock\0"
        .encode_utf16()
        .collect();
    let mutex = unsafe { CreateMutexW(std::ptr::null(), 1, mutex_name.as_ptr()) };
    if mutex.is_null() {
        let error = unsafe { GetLastError() };
        crate::error_log::report_win32("CreateMutexW failed", error);
        return Err(error);
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        let text: Vec<u16> = "SimpleWindowManager is already running\0"
            .encode_utf16()
            .collect();
        let title: Vec<u16> = "SimpleWindowManager\0".encode_utf16().collect();
        if unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK) } == 0 {
            crate::error_log::last_win32("MessageBoxW(already running) failed");
        }
        if unsafe { CloseHandle(mutex) } == 0 {
            crate::error_log::last_win32("CloseHandle(single-instance mutex) failed");
        }
        Ok(None)
    } else {
        Ok(Some(mutex as isize))
    }
}

#[tracing::instrument(skip(runtime), fields(instance_mutex = instance_mutex as usize))]
pub fn run(runtime: Runtime, instance_mutex: isize) -> Result<(), u32> {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
    use windows_sys::Win32::System::Com::{
        COINIT_MULTITHREADED, CoInitializeEx, CoInitializeSecurity, CoUninitialize, EOAC_NONE,
        RPC_C_AUTHN_LEVEL_DEFAULT, RPC_C_IMP_LEVEL_IMPERSONATE,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Accessibility::UnhookWinEvent;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL,
    };
    let com_result = unsafe { CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32) };
    crate::error_log::check_hresult("CoInitializeEx failed", com_result);
    let security_result = unsafe {
        CoInitializeSecurity(
            std::ptr::null_mut(),
            -1,
            std::ptr::null(),
            std::ptr::null(),
            RPC_C_AUTHN_LEVEL_DEFAULT,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            std::ptr::null(),
            EOAC_NONE as u32,
            std::ptr::null(),
        )
    };
    crate::error_log::check_hresult("CoInitializeSecurity failed", security_result);
    crate::status::initialize();
    RUNTIME
        .set(Mutex::new(runtime))
        .map_err(|_| unsafe { GetLastError() })?;
    if let Some(runtime) = RUNTIME.get() {
        let mut runtime = runtime.lock().unwrap();
        runtime.create_bars()?;
        runtime.apply_all_workspaces()?;
        runtime.create_border_window()?;
        runtime.create_drop_target_window()?;
    }
    let keyboard_hook = unsafe {
        let module = GetModuleHandleW(std::ptr::null());
        if module.is_null() {
            crate::error_log::last_win32("GetModuleHandleW(keyboard hook) failed");
        }
        SetWindowsHookExW(WH_KEYBOARD_LL, Some(handle_key_press), module, 0)
    };
    if keyboard_hook.is_null() {
        let error = unsafe { GetLastError() };
        crate::error_log::report_win32("SetWindowsHookExW(keyboard) failed", error);
        return Err(error);
    }
    let mouse_hook = unsafe {
        let module = GetModuleHandleW(std::ptr::null());
        if module.is_null() {
            crate::error_log::last_win32("GetModuleHandleW(mouse hook) failed");
        }
        SetWindowsHookExW(WH_MOUSE_LL, Some(handle_mouse), module, 0)
    };
    if mouse_hook.is_null() {
        let error = unsafe { GetLastError() };
        if unsafe { UnhookWindowsHookEx(keyboard_hook) } == 0 {
            crate::error_log::last_win32("UnhookWindowsHookEx(keyboard cleanup) failed");
        }
        crate::error_log::report_win32("SetWindowsHookExW(mouse) failed", error);
        return Err(error);
    }
    let hooks = install_win_event_hooks();
    if let Some(runtime) = RUNTIME.get() {
        runtime.lock().unwrap().finish_startup();
    }
    let result = run_message_loop();
    for hook in hooks {
        if unsafe { UnhookWinEvent(hook) } == 0 {
            crate::error_log::last_win32("UnhookWinEvent failed");
        }
    }
    unsafe {
        let timer = *WORKSPACE_MOVE_TIMER.lock().unwrap();
        if timer != 0 {
            if windows_sys::Win32::UI::WindowsAndMessaging::KillTimer(std::ptr::null_mut(), timer)
                == 0
            {
                crate::error_log::last_win32("KillTimer(workspace focus shutdown) failed");
            }
        }
        if UnhookWindowsHookEx(keyboard_hook) == 0 {
            crate::error_log::last_win32("UnhookWindowsHookEx(keyboard) failed");
        }
        if UnhookWindowsHookEx(mouse_hook) == 0 {
            crate::error_log::last_win32("UnhookWindowsHookEx(mouse) failed");
        }
        crate::border::clean();
        crate::status::shutdown();
        CoUninitialize();
        if CloseHandle(instance_mutex as _) == 0 {
            crate::error_log::last_win32("CloseHandle(single-instance mutex) failed");
        }
    }
    result
}
