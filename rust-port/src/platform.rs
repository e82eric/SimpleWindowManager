//! The only module that should directly expose Win32 callback and handle unsafety.

#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(windows)]
use std::sync::{Mutex, OnceLock};

#[cfg(windows)]
static MENU_ITEMS: Mutex<Vec<std::ffi::CString>> = Mutex::new(Vec::new());
#[cfg(windows)]
static MENU_POINTERS: Mutex<Vec<usize>> = Mutex::new(Vec::new());
#[cfg(windows)]
use crate::runtime::Runtime;

#[cfg(windows)]
static RUNTIME: OnceLock<Mutex<Runtime>> = OnceLock::new();

#[cfg(windows)]
static BORDER_LOST_FOCUS: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
pub(crate) fn set_border_lost_focus(value: bool) {
    BORDER_LOST_FOCUS.store(value, Ordering::Relaxed);
}

#[cfg(windows)]
pub(crate) fn border_is_lost_focus() -> bool {
    BORDER_LOST_FOCUS.load(Ordering::Relaxed)
}

#[cfg(not(windows))]
pub(crate) fn set_border_lost_focus(_value: bool) {}

#[cfg(not(windows))]
pub(crate) fn border_is_lost_focus() -> bool {
    false
}

#[cfg(windows)]
static SUPPRESSED_KEY_UPS: Mutex<[bool; 256]> = Mutex::new([false; 256]);

#[cfg(windows)]
pub(crate) fn show_menu_unavailable_warning() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONWARNING, MB_OK, MessageBoxW};
    let text: Vec<u16> = "LibNfm.dll not loaded; menu unavailable.\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = "SimpleWindowManager\0".encode_utf16().collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONWARNING,
        );
    }
}

#[cfg(not(windows))]
pub(crate) fn show_menu_unavailable_warning() {}
#[cfg(windows)]
static WORKSPACE_MOVE_TIMER: Mutex<usize> = Mutex::new(0);

#[cfg(windows)]
unsafe extern "system" fn finish_workspace_move_focus_preservation(
    _hwnd: windows_sys::Win32::Foundation::HWND,
    _message: u32,
    timer_id: usize,
    _time: u32,
) {
    use windows_sys::Win32::UI::WindowsAndMessaging::KillTimer;
    unsafe { KillTimer(std::ptr::null_mut(), timer_id) };
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

#[cfg(windows)]
pub(crate) fn schedule_workspace_move_focus_finish() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};
    let mut current = WORKSPACE_MOVE_TIMER.lock().unwrap();
    if *current != 0 {
        unsafe { KillTimer(std::ptr::null_mut(), *current) };
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
    timer != 0
}

#[cfg(not(windows))]
pub(crate) fn schedule_workspace_move_focus_finish() -> bool {
    true
}

#[cfg(windows)]
pub(crate) unsafe extern "C" fn menu_window_selected(hwnd: isize, _state: *mut std::ffi::c_void) {
    if let Some(runtime) = RUNTIME.get() {
        if let Ok(mut runtime) = runtime.try_lock() {
            runtime.menu_window_selected(hwnd);
        }
    }
}

#[cfg(windows)]
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

#[cfg(windows)]
pub(crate) unsafe extern "C" fn menu_on_closed() {
    if let Some(runtime) = RUNTIME.get() {
        if let Ok(mut runtime) = runtime.try_lock() {
            runtime.menu_closed();
        }
    }
}

#[cfg(windows)]
pub(crate) unsafe extern "C" fn menu_process_selected(_pid: u32, _state: *mut std::ffi::c_void) {
    unsafe { menu_on_closed() };
}

#[cfg(windows)]
pub(crate) unsafe extern "C" fn menu_string_noop(
    _value: *mut std::ffi::c_char,
    _state: *mut std::ffi::c_void,
) {
    unsafe { menu_on_closed() };
}

#[cfg(windows)]
unsafe extern "C" fn menu_items(_state: *mut std::ffi::c_void) -> *mut *mut std::ffi::c_char {
    MENU_POINTERS.lock().unwrap().as_mut_ptr() as *mut *mut std::ffi::c_char
}

#[cfg(windows)]
unsafe extern "C" fn menu_command_selected(
    value: *mut std::ffi::c_char,
    _state: *mut std::ffi::c_void,
) {
    if value.is_null() {
        return;
    }
    let value = unsafe { std::ffi::CStr::from_ptr(value) }.to_string_lossy();
    let name = value.split_whitespace().next();
    if let (Some(name), Some(runtime)) = (name, RUNTIME.get()) {
        if let Ok(mut runtime) = runtime.try_lock() {
            let action = runtime
                .configuration
                .commands
                .iter()
                .find(|command| command.name == name)
                .map(|command| command.action);
            if let Some(action) = action {
                let _ = runtime.execute(action);
            }
            runtime.menu_hide();
        }
    }
}

#[cfg(windows)]
pub(crate) fn show_commands_menu(runtime: &mut Runtime) {
    let names: Vec<String> = runtime
        .configuration
        .commands
        .iter()
        .map(|command| command.name.clone())
        .collect();
    let width = names.iter().map(String::len).max().unwrap_or(0);
    let lines: Vec<String> = runtime
        .configuration
        .commands
        .iter()
        .zip(&names)
        .map(|(command, name)| {
            format!(
                "{name:<width$} {:<25} {:<30} {}",
                command_type(command.action),
                key_binding_text(command.modifiers, command.key),
                command_description(runtime, command.action)
            )
        })
        .collect();
    let header = format!(
        "{:<width$} {:<25} {:<30} {}",
        "Name", "Type", "KeyBinding", "Description"
    );
    show_string_items(runtime, &header, lines, menu_command_selected);
}

#[cfg(windows)]
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

#[cfg(windows)]
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

#[cfg(windows)]
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

#[cfg(windows)]
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

#[cfg(windows)]
fn key_name(key: u32) -> String {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyNameTextW, MAPVK_VK_TO_VSC, MapVirtualKeyW,
    };
    if (0x7c..=0x82).contains(&key) {
        return format!("F{}", key - 0x7c + 13);
    }
    let mut scan = unsafe { MapVirtualKeyW(key, MAPVK_VK_TO_VSC) };
    if matches!(
        key,
        0x21 | 0x22 | 0x23 | 0x24 | 0x25 | 0x26 | 0x27 | 0x28 | 0x2d | 0x2e | 0x6f | 0x90
    ) {
        scan |= 0x100;
    }
    let mut buffer = [0u16; 256];
    let length = unsafe { GetKeyNameTextW((scan << 16) as i32, buffer.as_mut_ptr(), 256) };
    String::from_utf16_lossy(&buffer[..length.max(0) as usize])
}

#[cfg(windows)]
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

#[cfg(windows)]
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

#[cfg(windows)]
fn show_array_columns(
    runtime: &mut Runtime,
    rows: Vec<Vec<String>>,
    columns: &[&str],
    display: &[i32],
) {
    let visible_columns: Vec<usize> = display.iter().map(|column| *column as usize).collect();
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

#[cfg(windows)]
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
    unsafe {
        FileTimeToSystemTime(&file, &mut utc);
        SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local);
    }
    format!(
        "{:02}:{:02}:{:02}",
        local.wHour, local.wMinute, local.wSecond
    )
}

#[cfg(windows)]
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
#[cfg(windows)]
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

#[cfg(windows)]
pub(crate) fn show_services_menu(runtime: &mut Runtime) {
    let lines = crate::process::list_services()
        .unwrap_or_else(|error| vec![format!("Unable to enumerate services: {error}")]);
    show_string_items(runtime, "State      Service", lines, menu_string_noop);
}

#[cfg(windows)]
fn show_string_items(
    runtime: &mut Runtime,
    header: &str,
    mut lines: Vec<String>,
    select: crate::menu::OnSelectString,
) {
    if lines.is_empty() {
        lines.push("No entries logged yet".into());
    }
    let mut items = MENU_ITEMS.lock().unwrap();
    *items = lines
        .into_iter()
        .filter_map(|line| std::ffi::CString::new(line).ok())
        .collect();
    let mut pointers = MENU_POINTERS.lock().unwrap();
    *pointers = items.iter().map(|item| item.as_ptr() as usize).collect();
    pointers.push(0);
    drop(pointers);
    drop(items);
    let Ok(header) = std::ffi::CString::new(header) else {
        return;
    };
    let monitor = &runtime.configuration.state.monitors[0];
    if let Some(menu) = &runtime.rust_nfm {
        menu.set_monitor_center(monitor);
        unsafe {
            (menu.show_items_list)(
                header.as_ptr() as *mut _,
                menu_items,
                select,
                menu_on_closed,
                std::ptr::null_mut(),
            );
        }
        runtime.configuration.state.menu_visible = true;
    } else if let Some(menu) = &runtime.legacy_nfm {
        menu.set_monitor_center(monitor);
        unsafe {
            (menu.show_items_list)(
                header.as_ptr() as *mut _,
                menu_items,
                select,
                menu_on_closed,
                std::ptr::null_mut(),
            );
        }
        runtime.configuration.state.menu_visible = true;
    }
}

#[cfg(windows)]
pub(crate) fn bar_snapshot(monitor: usize) -> Option<crate::bar::BarSnapshot> {
    RUNTIME.get()?.try_lock().ok()?.bar_snapshot(monitor)
}

#[cfg(windows)]
pub(crate) fn bar_workspace_clicked(monitor: usize, workspace: usize) {
    if let Some(runtime) = RUNTIME.get() {
        if let Ok(mut runtime) = runtime.try_lock() {
            runtime.bar_workspace_clicked(monitor, workspace);
        }
    }
}

#[cfg(windows)]
pub(crate) fn bar_commands_clicked() {
    if let Some(runtime) = RUNTIME.get() {
        if let Ok(mut runtime) = runtime.try_lock() {
            runtime.run_commands_menu();
        }
    }
}

#[cfg(windows)]
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

#[cfg(windows)]
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

#[cfg(windows)]
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

#[cfg(windows)]
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
    if !runtime.is_root_window(raw) {
        let _ = runtime.remove_client_by_hwnd(raw);
        return;
    }
    let result = match event {
        EVENT_OBJECT_DESTROY => runtime.remove_client_by_hwnd(raw),
        EVENT_OBJECT_HIDE => {
            if is_taskbar(hwnd)
                && let Err(error) = runtime.taskbar_changed(raw)
            {
                eprintln!("taskbar resize failed with error {error}");
            }
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                GWL_STYLE, GetWindowLongW, WS_VISIBLE,
            };
            if unsafe { GetWindowLongW(hwnd, GWL_STYLE) } & WS_VISIBLE as i32 == 0 {
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
        eprintln!("WinEvent {event:#x} failed with error {error}");
    }
}

#[cfg(windows)]
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
        return false;
    }
    matches!(
        String::from_utf16_lossy(&buffer[..length as usize]).as_str(),
        "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    )
}

#[cfg(windows)]
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
            (!hook.is_null()).then_some(hook)
        })
        .collect()
}

#[cfg(windows)]
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
                return Err(windows_sys::Win32::Foundation::GetLastError());
            }
            if result == 0 {
                return Ok(());
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

#[cfg(windows)]
pub fn acquire_single_instance() -> Result<Option<isize>, u32> {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_OK, MessageBoxW};
    let mutex_name: Vec<u16> = "SimpleWindowManagerSingleInstanceLock\0"
        .encode_utf16()
        .collect();
    let mutex = unsafe { CreateMutexW(std::ptr::null(), 1, mutex_name.as_ptr()) };
    if mutex.is_null() {
        return Err(unsafe { GetLastError() });
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        let text: Vec<u16> = "SimpleWindowManager is already running\0"
            .encode_utf16()
            .collect();
        let title: Vec<u16> = "SimpleWindowManager\0".encode_utf16().collect();
        unsafe {
            MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK);
            CloseHandle(mutex);
        }
        Ok(None)
    } else {
        Ok(Some(mutex as isize))
    }
}

#[cfg(not(windows))]
pub fn acquire_single_instance() -> Result<Option<isize>, u32> {
    Ok(Some(0))
}

#[cfg(windows)]
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
    unsafe {
        CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32);
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
        );
    }
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
        SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(handle_key_press),
            GetModuleHandleW(std::ptr::null()),
            0,
        )
    };
    if keyboard_hook.is_null() {
        return Err(unsafe { GetLastError() });
    }
    let mouse_hook = unsafe {
        SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(handle_mouse),
            GetModuleHandleW(std::ptr::null()),
            0,
        )
    };
    if mouse_hook.is_null() {
        unsafe {
            UnhookWindowsHookEx(keyboard_hook);
        }
        return Err(unsafe { GetLastError() });
    }
    let hooks = install_win_event_hooks();
    if let Some(runtime) = RUNTIME.get() {
        runtime.lock().unwrap().finish_startup();
    }
    let result = run_message_loop();
    for hook in hooks {
        unsafe {
            UnhookWinEvent(hook);
        }
    }
    unsafe {
        let timer = *WORKSPACE_MOVE_TIMER.lock().unwrap();
        if timer != 0 {
            windows_sys::Win32::UI::WindowsAndMessaging::KillTimer(std::ptr::null_mut(), timer);
        }
        UnhookWindowsHookEx(keyboard_hook);
        UnhookWindowsHookEx(mouse_hook);
        crate::border::clean();
        crate::status::shutdown();
        CoUninitialize();
        CloseHandle(instance_mutex as _);
    }
    result
}

#[cfg(not(windows))]
pub fn run(_runtime: crate::runtime::Runtime, _instance_mutex: isize) -> Result<(), u32> {
    Ok(())
}

#[cfg(not(windows))]
pub fn run_message_loop() -> Result<(), u32> {
    Ok(())
}
