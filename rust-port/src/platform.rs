//! The only module that should directly expose Win32 callback and handle unsafety.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::platform_error::{PlatformError, PlatformResult, check_hresult};
use crate::runtime::Runtime;

static RUNTIME: OnceLock<Mutex<Runtime>> = OnceLock::new();

static BORDER_LOST_FOCUS: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_border_lost_focus(value: bool) {
    BORDER_LOST_FOCUS.store(value, Ordering::Relaxed);
}

pub(crate) fn border_is_lost_focus() -> bool {
    BORDER_LOST_FOCUS.load(Ordering::Relaxed)
}

pub(crate) fn window_message_name(message: u32) -> &'static str {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    match message {
        WM_NCCREATE => "WM_NCCREATE",
        WM_CREATE => "WM_CREATE",
        WM_SIZE => "WM_SIZE",
        WM_WINDOWPOSCHANGING => "WM_WINDOWPOSCHANGING",
        WM_PAINT => "WM_PAINT",
        WM_ERASEBKGND => "WM_ERASEBKGND",
        WM_TIMER => "WM_TIMER",
        WM_LBUTTONDOWN => "WM_LBUTTONDOWN",
        WM_LBUTTONUP => "WM_LBUTTONUP",
        WM_RBUTTONDOWN => "WM_RBUTTONDOWN",
        WM_CLOSE => "WM_CLOSE",
        WM_DESTROY => "WM_DESTROY",
        _ => "UNKNOWN_WINDOW_MESSAGE",
    }
}

static WORKSPACE_MOVE_TIMER: Mutex<usize> = Mutex::new(0);

unsafe extern "system" fn refresh_ui_state(
    _hwnd: windows_sys::Win32::Foundation::HWND,
    _message: u32,
    _timer_id: usize,
    _time: u32,
) {
    if let Some(runtime) = RUNTIME.get()
        && let Ok(mut runtime) = runtime.try_lock()
    {
        runtime.refresh_ui_state();
    }
}

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

pub(crate) fn command_menu_rows(configuration: &crate::Configuration) -> Vec<Vec<String>> {
    let rows = configuration
        .commands
        .iter()
        .map(|command| {
            vec![
                command.name.clone(),
                command_type(command.action).to_owned(),
                key_binding_text(command.modifiers, command.key),
                command_description(configuration, command.action),
            ]
        })
        .collect();
    rows
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
        ToggleDebugOutput => "ToggleDebugOutput".into(),
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
        ShowPicker(picker) => picker.name.into(),
        ShowMailbox(_) => "show_bar_mailbox".into(),
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
        Launch(_) => "Shell",
        _ => "Function",
    }
}

fn command_description(
    configuration: &crate::Configuration,
    action: crate::commands::Action,
) -> String {
    use crate::commands::Action::*;
    match action {
        SelectWorkspace(i) | MoveToWorkspace(i) | MoveWorkspaceToMonitor(i, _) => configuration
            .state
            .workspaces
            .get(i)
            .map(|workspace| workspace.name.clone())
            .unwrap_or_default(),
        MoveFloatToMonitor(i) => (i + 1).to_string(),
        Launch(command) => command.into(),
        ShowPicker(picker) => picker.name.into(),
        ToggleDebugOutput => format!(
            "OutputDebugString logging is {}",
            if crate::error_log::debug_output_enabled() {
                "enabled"
            } else {
                "disabled"
            }
        ),
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

pub(crate) fn float_logs_table(
    configuration: &crate::Configuration,
) -> crate::egui_data_menu::DataTable {
    let rows: Vec<crate::egui_data_menu::DataRow> = configuration
        .state
        .float_log_buffer
        .iter()
        .map(|entry| crate::egui_data_menu::DataRow {
            cells: vec![
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
            ],
            preview: None,
        })
        .collect();
    let rows = if rows.is_empty() {
        vec![crate::egui_data_menu::DataRow {
            cells: vec![
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
            ],
            preview: None,
        }]
    } else {
        rows
    };
    array_columns_table(
        rows,
        "Float logs",
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
    )
}

pub(crate) fn client_logs_table(
    configuration: &crate::Configuration,
) -> crate::egui_data_menu::DataTable {
    let rows: Vec<crate::egui_data_menu::DataRow> = configuration
        .state
        .client_log_buffer
        .iter()
        .map(|entry| crate::egui_data_menu::DataRow {
            cells: vec![
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
            ],
            preview: None,
        })
        .collect();
    let rows = if rows.is_empty() {
        let mut cells = vec![String::new(); 15];
        cells[0] = "No clients logged yet".into();
        vec![crate::egui_data_menu::DataRow {
            cells,
            preview: None,
        }]
    } else {
        rows
    };
    array_columns_table(
        rows,
        "Client logs",
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
    )
}

pub(crate) fn error_logs_table(
    _configuration: &crate::Configuration,
) -> crate::egui_data_menu::DataTable {
    let mut rows: Vec<crate::egui_data_menu::DataRow> = crate::error_log::entries()
        .into_iter()
        .rev()
        .map(|entry| {
            let preview = format_error_preview(&entry);
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
            crate::egui_data_menu::DataRow {
                cells: vec![
                    local_log_time(entry.timestamp),
                    entry.entry_method,
                    entry.current_method,
                    entry.context,
                    entry.code.map(|code| code.to_string()).unwrap_or_default(),
                    entry.message,
                    properties,
                    source,
                ],
                preview: Some(preview),
            }
        })
        .collect();
    if rows.is_empty() {
        let log_path = crate::error_log::log_file_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "File logging unavailable: LOCALAPPDATA is not set".into());
        rows.push(crate::egui_data_menu::DataRow {
            cells: vec![
                String::new(),
                String::new(),
                String::new(),
                "No runtime errors logged".into(),
                String::new(),
                String::new(),
                String::new(),
                log_path.clone(),
            ],
            preview: Some(format!("No runtime errors logged\n\nLog file: {log_path}")),
        });
    }
    array_columns_table(
        rows,
        "Error logs",
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
    )
}

pub(crate) fn configuration_state_table(
    configuration: &crate::Configuration,
) -> crate::egui_data_menu::DataTable {
    let state = &configuration.state;
    let mut rows = Vec::new();
    let mut add = |category: &str, property: &str, value: String, details: Option<String>| {
        let preview = details.unwrap_or_else(|| {
            format!("Category: {category}\nProperty: {property}\nValue: {value}")
        });
        rows.push(crate::egui_data_menu::DataRow {
            cells: vec![category.into(), property.into(), value],
            preview: Some(preview),
        });
    };

    macro_rules! scalar {
        ($category:literal, $property:ident, $value:expr) => {
            add(
                $category,
                stringify!($property),
                format!("{}", $value),
                None,
            )
        };
    }

    scalar!("Configuration", always_redraw, configuration.always_redraw);
    scalar!(
        "Configuration",
        use_old_move_logic_configured,
        configuration.use_old_move_logic_configured
    );
    scalar!(
        "Configuration",
        non_float_window_height_minimum,
        configuration.non_float_window_height_minimum
    );
    scalar!(
        "Configuration",
        float_uwp_windows,
        configuration.float_uwp_windows
    );
    scalar!("Configuration", bar_height, configuration.bar_height);
    scalar!(
        "Configuration",
        primary_monitor,
        configuration.primary_monitor
    );
    scalar!(
        "Configuration",
        secondary_monitor,
        configuration.secondary_monitor
    );
    scalar!(
        "Configuration",
        picker_monitor,
        format!("{:?}", configuration.picker_monitor)
    );
    scalar!(
        "Configuration",
        float_window_movement,
        configuration.float_window_movement
    );
    scalar!(
        "Configuration",
        drag_drop_float_modifier,
        configuration.drag_drop_float_modifier
    );
    scalar!(
        "Configuration",
        bar_right_padding,
        configuration.bar_right_padding
    );
    scalar!(
        "Configuration",
        cloak_to_hide_all,
        configuration.cloak_to_hide_all
    );
    add(
        "Configuration",
        "text_style",
        if configuration.text_style.is_some() {
            "configured".into()
        } else {
            "none".into()
        },
        Some(format!("{:#?}", configuration.text_style)),
    );

    let mut collection = |property: &str, count: usize, details: String| {
        add(
            "Configuration collection",
            property,
            format!("{count} entries"),
            Some(details),
        );
    };
    collection(
        "key_bindings",
        configuration.key_bindings.len(),
        format!("{:#?}", configuration.key_bindings),
    );
    collection(
        "commands",
        configuration.commands.len(),
        format!("{:#?}", configuration.commands),
    );
    collection(
        "bar_segments",
        configuration.bar_segments.len(),
        format!("{:#?}", configuration.bar_segments),
    );
    collection(
        "pickers",
        configuration.pickers.len(),
        format!("{:#?}", configuration.pickers),
    );
    collection(
        "prevent_float_processes",
        configuration.prevent_float_processes.len(),
        format!("{:#?}", configuration.prevent_float_processes),
    );
    drop(collection);

    add(
        "State",
        "selected_monitor",
        format!("{:?}", state.selected_monitor),
        None,
    );
    add(
        "State",
        "last_workspace",
        format!("{:?}", state.last_workspace),
        None,
    );
    scalar!("UI", menu_visible, crate::ui::menu_visible());
    scalar!(
        "Logging",
        debug_output_enabled,
        crate::error_log::debug_output_enabled()
    );
    add(
        "State",
        "current_window_routing_mode",
        format!("{:?}", state.current_window_routing_mode),
        None,
    );

    let active_clients = state.clients.iter().flatten().count();
    add(
        "State collection",
        "clients",
        format!("{active_clients} active / {} slots", state.clients.len()),
        Some(format!("{:#?}", state.clients)),
    );
    add(
        "State collection",
        "workspaces",
        format!("{} entries", state.workspaces.len()),
        Some(format!("{:#?}", state.workspaces)),
    );
    add(
        "State collection",
        "monitors",
        format!("{} entries", state.monitors.len()),
        Some(format!("{:#?}", state.monitors)),
    );
    add(
        "State collection",
        "float_log_buffer",
        format!("{} entries", state.float_log_buffer.len()),
        Some(format!("{:#?}", state.float_log_buffer)),
    );
    add(
        "State collection",
        "client_log_buffer",
        format!("{} entries", state.client_log_buffer.len()),
        Some(format!("{:#?}", state.client_log_buffer)),
    );

    array_columns_table(
        rows,
        "Configuration state",
        &["Category", "Property", "Value"],
        &[0, 1, 2],
    )
}

fn format_error_preview(entry: &crate::error_log::ErrorLogEntry) -> String {
    use std::fmt::Write;
    let source = entry.line.map_or_else(
        || entry.file.clone(),
        |line| format!("{}:{line}", entry.file),
    );
    let code = entry
        .code
        .map_or_else(|| "N/A".into(), |code| code.to_string());
    let mut preview = format!(
        "Time: {}\nEntry: {}\nCurrent: {}\nOperation: {}\nCode: {}\nMessage: {}\nSource: {}\n",
        local_log_time(entry.timestamp),
        entry.entry_method,
        entry.current_method,
        entry.context,
        code,
        entry.message,
        source,
    );
    preview.push_str("\nContext:\n");
    if entry.frames.is_empty() {
        preview.push_str("  None\n");
    } else {
        for frame in &entry.frames {
            let _ = writeln!(preview, "  {}", frame.method);
            for (name, value) in &frame.fields {
                let _ = writeln!(preview, "    {name}: {value}");
            }
        }
    }
    preview.push_str("\nStack trace:\n");
    if entry.backtrace.is_empty() {
        preview.push_str("  Not captured\n");
    } else {
        preview.push_str(&entry.backtrace);
        if !entry.backtrace.ends_with('\n') {
            preview.push('\n');
        }
    }
    preview
}

fn array_columns_table(
    rows: Vec<crate::egui_data_menu::DataRow>,
    title: &str,
    columns: &[&str],
    display: &[i32],
) -> crate::egui_data_menu::DataTable {
    let columns = display
        .iter()
        .filter_map(|index| columns.get(*index as usize))
        .map(|column| (*column).to_owned())
        .collect();
    let rows = rows
        .into_iter()
        .map(|row| crate::egui_data_menu::DataRow {
            cells: display
                .iter()
                .map(|index| row.cells.get(*index as usize).cloned().unwrap_or_default())
                .collect(),
            preview: row.preview,
        })
        .collect();
    crate::egui_data_menu::DataTable {
        title: title.into(),
        columns,
        rows,
    }
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
        PlatformError::last_win32("FileTimeToSystemTime(log timestamp)").report();
        return "--:--:--".into();
    }
    if unsafe { SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) } == 0 {
        PlatformError::last_win32("SystemTimeToTzSpecificLocalTime(log timestamp)").report();
        return "--:--:--".into();
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

pub(crate) fn services_table(
    _configuration: &crate::Configuration,
) -> crate::egui_data_menu::DataTable {
    let mut lines = crate::process::list_services().unwrap_or_else(|error| {
        let message = format!("Unable to enumerate services: {error}");
        error.report();
        vec![message]
    });
    if lines.is_empty() {
        lines.push("No services found".into());
    }
    let rows = lines
        .into_iter()
        .map(|line| {
            let (state, service) = line
                .split_once(' ')
                .filter(|(state, _)| matches!(*state, "RUNNING" | "STOPPED"))
                .unwrap_or(("", line.as_str()));
            crate::egui_data_menu::DataRow {
                cells: vec![state.into(), service.trim_start().into()],
                preview: None,
            }
        })
        .collect();
    crate::egui_data_menu::DataTable {
        title: "Services".into(),
        columns: vec!["State".into(), "Service".into()],
        rows,
    }
}

pub(crate) fn bar_snapshot(monitor: usize) -> Option<crate::bar::BarSnapshot> {
    crate::ui::bar_snapshot(monitor)
}
pub(crate) fn bar_workspace_clicked(monitor: usize, workspace: usize) {
    crate::worker::send(crate::worker::Command::Workspace(monitor, workspace));
}
pub(crate) fn bar_commands_clicked() {
    crate::ui::post(crate::ui::Command::Action(
        crate::commands::Action::ShowPicker(&crate::builtin_pickers::COMMANDS),
    ));
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
    let span = tracing::info_span!(
        "windows_event",
        event_name = win_event_name(event),
        event,
        hwnd = hwnd as usize,
        object = id_object,
        child = id_child,
    );
    let _entered = span.enter();
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
    let result = (|| -> PlatformResult<()> {
        match event {
            EVENT_OBJECT_DESTROY => {
                if let Err(error) = runtime.remove_client_by_hwnd(raw) {
                    report_win_event_error(event, raw, id_object, id_child, error);
                }
                Ok(())
            }
            EVENT_OBJECT_HIDE => {
                if is_taskbar(hwnd)?
                    && let Err(error) = runtime.taskbar_changed(raw)
                {
                    report_win_event_error(event, raw, id_object, id_child, error);
                }
                use windows_sys::Win32::Foundation::ERROR_INVALID_WINDOW_HANDLE;
                use windows_sys::Win32::UI::WindowsAndMessaging::{GWL_STYLE, WS_VISIBLE};
                match platform_window_long(hwnd, GWL_STYLE) {
                    Ok(styles) if styles & WS_VISIBLE as i32 == 0 => {
                        runtime.remove_client_by_hwnd(raw)
                    }
                    Ok(_) => Ok(()),
                    Err(error) if error.is_win32(ERROR_INVALID_WINDOW_HANDLE) => {
                        // A hide event can outlive its HWND. An invalid handle is
                        // already effectively hidden, so remove any tracked client.
                        runtime.remove_client_by_hwnd(raw)
                    }
                    Err(error) => Err(error),
                }
            }
            EVENT_OBJECT_SHOW | EVENT_OBJECT_UNCLOAKED if is_taskbar(hwnd)? => {
                runtime.taskbar_changed(raw)
            }
            EVENT_OBJECT_SHOW | EVENT_OBJECT_UNCLOAKED => runtime.refresh_window(raw),
            EVENT_SYSTEM_MINIMIZESTART | EVENT_OBJECT_LOCATIONCHANGE => {
                runtime.location_changed(raw)
            }
            EVENT_SYSTEM_FOREGROUND => {
                runtime.foreground_changed(raw);
                Ok(())
            }
            _ => Ok(()),
        }
    })();
    if let Err(error) = result {
        report_win_event_error(event, raw, id_object, id_child, error);
    }
}

fn win_event_name(event: u32) -> &'static str {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE, EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_SHOW,
        EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZESTART,
    };
    match event {
        EVENT_OBJECT_DESTROY => "EVENT_OBJECT_DESTROY",
        EVENT_OBJECT_HIDE => "EVENT_OBJECT_HIDE",
        EVENT_OBJECT_LOCATIONCHANGE => "EVENT_OBJECT_LOCATIONCHANGE",
        EVENT_OBJECT_SHOW => "EVENT_OBJECT_SHOW",
        EVENT_OBJECT_UNCLOAKED => "EVENT_OBJECT_UNCLOAKED",
        EVENT_SYSTEM_FOREGROUND => "EVENT_SYSTEM_FOREGROUND",
        EVENT_SYSTEM_MINIMIZESTART => "EVENT_SYSTEM_MINIMIZESTART",
        _ => "UNKNOWN_WIN_EVENT",
    }
}

fn report_win_event_error(event: u32, hwnd: isize, object: i32, child: i32, error: PlatformError) {
    let span = tracing::error_span!(
        "handle_windows_event",
        event,
        hwnd = hwnd as usize,
        object,
        child
    );
    let _entered = span.enter();
    error.report();
}

fn platform_window_long(
    hwnd: windows_sys::Win32::Foundation::HWND,
    index: i32,
) -> PlatformResult<i32> {
    use windows_sys::Win32::Foundation::{GetLastError, SetLastError};
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongW;
    unsafe {
        SetLastError(0);
        let value = GetWindowLongW(hwnd, index);
        let error = GetLastError();
        if value == 0 && error != 0 {
            Err(PlatformError::win32("GetWindowLongW(window event)", error))
        } else {
            Ok(value)
        }
    }
}

fn is_taskbar(hwnd: windows_sys::Win32::Foundation::HWND) -> PlatformResult<bool> {
    use windows_sys::Win32::Foundation::ERROR_INVALID_WINDOW_HANDLE;
    match crate::snapshot::window_class_name(hwnd as isize, "GetClassNameW(taskbar)") {
        Ok(Some(class_name)) => Ok(matches!(
            class_name.as_str(),
            "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
        )),
        Ok(None) => Ok(false),
        Err(error) if error.is_win32(ERROR_INVALID_WINDOW_HANDLE) => {
            // Out-of-context WinEvent delivery can race window destruction.
            tracing::info!(
                target: "swm::window",
                hwnd = hwnd as usize,
                "window disappeared before taskbar classification"
            );
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

fn install_win_event_hooks()
-> PlatformResult<Vec<windows_sys::Win32::UI::Accessibility::HWINEVENTHOOK>> {
    use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent};
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
    let mut hooks = Vec::with_capacity(ranges.len());
    for (min, max) in ranges {
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
            let error = PlatformError::last_win32("SetWinEventHook");
            for installed in hooks {
                if unsafe { UnhookWinEvent(installed) } == 0 {
                    PlatformError::last_win32("UnhookWinEvent(hook installation cleanup)").report();
                }
            }
            return Err(error);
        }
        hooks.push(hook);
    }
    Ok(hooks)
}

#[tracing::instrument]
fn run_message_loop(
    receiver: &std::sync::mpsc::Receiver<crate::ui::Command>,
) -> PlatformResult<()> {
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
                return Err(PlatformError::win32("GetMessageW", error));
            }
            if result == 0 {
                return Ok(());
            }
            crate::ui::pump(receiver);
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

pub(crate) fn run_worker(
    runtime: Runtime,
    receiver: std::sync::mpsc::Receiver<crate::worker::QueuedCommand>,
    stop: &AtomicBool,
) -> PlatformResult<()> {
    use windows_sys::Win32::System::Com::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    check_hresult(
        unsafe { CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32) },
        "CoInitializeEx(worker)",
    )?;
    // The guard runs on this thread on both startup failures and normal exit.
    struct Cleanup {
        hooks: Vec<windows_sys::Win32::UI::Accessibility::HWINEVENTHOOK>,
        timer: usize,
    }
    impl Drop for Cleanup {
        fn drop(&mut self) {
            unsafe {
                for hook in self.hooks.drain(..) {
                    windows_sys::Win32::UI::Accessibility::UnhookWinEvent(hook);
                }
                if self.timer != 0 {
                    KillTimer(std::ptr::null_mut(), self.timer);
                }
                let timer = *WORKSPACE_MOVE_TIMER.lock().unwrap();
                if timer != 0 {
                    KillTimer(std::ptr::null_mut(), timer);
                }
                if let Some(runtime) = RUNTIME.get() {
                    let mut runtime = runtime
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if let Some(window) = runtime.border_window.take() {
                        DestroyWindow(window.hwnd as _);
                    }
                    if let Some(window) = runtime.drop_target_window.take() {
                        DestroyWindow(window.hwnd as _);
                    }
                }
                crate::border::clean();
                CoUninitialize();
            }
        }
    }
    let mut cleanup = Cleanup {
        hooks: Vec::new(),
        timer: 0,
    };
    RUNTIME
        .set(Mutex::new(runtime))
        .map_err(|_| PlatformError::win32("initialize worker runtime", 183))?;
    {
        let mut runtime = RUNTIME.get().unwrap().lock().unwrap();
        runtime.apply_all_workspaces()?;
        runtime.create_border_window()?;
        runtime.create_drop_target_window()?;
        runtime.finish_startup();
        crate::worker::publish(&runtime);
    }
    cleanup.hooks = install_win_event_hooks()?;
    cleanup.timer = unsafe { SetTimer(std::ptr::null_mut(), 0, 1500, Some(refresh_ui_state)) };
    let mut message = MSG::default();
    while !stop.load(Ordering::Acquire) {
        // Polling also drains commands whose wake arrived during a nested loop.
        let wait = unsafe {
            MsgWaitForMultipleObjectsEx(0, std::ptr::null(), 50, QS_ALLINPUT, MWMO_INPUTAVAILABLE)
        };
        if wait == windows_sys::Win32::Foundation::WAIT_FAILED {
            return Err(PlatformError::last_win32("worker message wait"));
        }
        for _ in 0..64 {
            if unsafe { PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) } == 0 {
                break;
            }
            if message.message == WM_QUIT {
                return Ok(());
            }
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        let mut runtime = RUNTIME.get().unwrap().lock().unwrap();
        crate::worker::drain(&receiver, &mut runtime);
    }
    Ok(())
}

#[tracing::instrument(skip(runtime), fields(instance_mutex=instance_mutex as usize))]
pub fn run(runtime: Runtime, instance_mutex: isize) -> PlatformResult<()> {
    use windows_sys::Win32::System::Com::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    check_hresult(
        unsafe { CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32) },
        "CoInitializeEx(UI)",
    )?;
    struct Cleanup {
        timer: usize,
        mutex: isize,
    }
    impl Drop for Cleanup {
        fn drop(&mut self) {
            unsafe {
                if self.timer != 0 {
                    KillTimer(std::ptr::null_mut(), self.timer);
                }
                crate::ui::shutdown();
                crate::status::shutdown();
                CoUninitialize();
                windows_sys::Win32::Foundation::CloseHandle(self.mutex as _);
            }
        }
    }
    let mut cleanup = Cleanup {
        timer: 0,
        mutex: instance_mutex,
    };
    check_hresult(
        unsafe {
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
        },
        "CoInitializeSecurity",
    )?;
    crate::status::initialize();
    crate::worker::publish(&runtime);
    let receiver =
        crate::ui::initialize(runtime.configuration.clone(), runtime.key_bindings.clone())?;
    // Thread messages can be consumed by modal loops; a UI timer supplies an
    // independent wake and keeps status painting independent of worker health.
    cleanup.timer = unsafe { SetTimer(std::ptr::null_mut(), 0, 250, None) };
    let worker = crate::worker::Worker::start(runtime)?;
    let input = crate::input::InputHooks::start()?;
    let result = run_message_loop(&receiver);
    drop(input);
    drop(worker);
    result
}

#[tracing::instrument]
pub fn acquire_single_instance() -> PlatformResult<Option<isize>> {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_OK, MessageBoxW};
    let mutex_name: Vec<u16> = "SimpleWindowManagerSingleInstanceLock\0"
        .encode_utf16()
        .collect();
    let mutex = unsafe { CreateMutexW(std::ptr::null(), 1, mutex_name.as_ptr()) };
    let create_error = unsafe { GetLastError() };
    if mutex.is_null() {
        return Err(PlatformError::win32("CreateMutexW", create_error));
    }
    if create_error == ERROR_ALREADY_EXISTS {
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
