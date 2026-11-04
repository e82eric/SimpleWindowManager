use crate::platform_error::PlatformResult;
use crate::snapshot::WindowSnapshot;

fn process_name(window: &WindowSnapshot) -> &str {
    window
        .data
        .process_image_name
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or("")
}

pub fn list_windows_run(max_items: usize) -> PlatformResult<Vec<String>> {
    let windows = crate::snapshot::capture_list_windows()?;
    Ok(format_windows(&windows, max_items))
}

fn format_windows(windows: &[WindowSnapshot], max_items: usize) -> Vec<String> {
    let max_process_name_len = windows
        .iter()
        .map(|window| process_name(window).len())
        .max()
        .unwrap_or(0);
    let mut result = Vec::with_capacity(windows.len().min(max_items) + 1);
    result.push(format!(
        "{:<8} {:<8} {:<width$} {}\n",
        "HWND",
        "PID",
        "Name",
        "Title",
        width = max_process_name_len
    ));
    for window in windows.iter().take(max_items) {
        result.push(format!(
            "{:08x} {:08} {:<width$} {}\n",
            window.data.hwnd as usize,
            window.data.process_id,
            process_name(window),
            window.data.title,
            width = max_process_name_len
        ));
    }
    result
}

pub fn parse_selected_hwnd(selection: &str) -> Option<isize> {
    let field = selection.split_ascii_whitespace().next()?;
    usize::from_str_radix(field, 16)
        .ok()
        .map(|value| value as isize)
}

pub fn focus_selected_window(selection: &str) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SW_RESTORE, SetForegroundWindow, ShowWindow,
    };
    let Some(hwnd) = parse_selected_hwnd(selection) else {
        return false;
    };
    unsafe {
        ShowWindow(hwnd as _, SW_RESTORE);
        let focused = SetForegroundWindow(hwnd as _) != 0;
        if !focused {
            crate::error_log::report(
                "SetForegroundWindow(selected window) was denied",
                "returned zero",
            );
        }
        focused
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClientData, Rect};

    #[test]
    fn formats_the_same_fixed_columns_as_list_windows_c() {
        let windows = vec![WindowSnapshot {
            data: ClientData {
                hwnd: 0x12ab,
                process_id: 42,
                process_image_name: "app.exe".into(),
                title: "Editor".into(),
                ..Default::default()
            },
            styles: 0,
            ex_styles: 0,
            normal_rect: Rect::default(),
        }];
        let rows = format_windows(&windows, 100);
        assert_eq!(rows[0], "HWND     PID      Name    Title\n");
        assert_eq!(rows[1], "000012ab 00000042 app.exe Editor\n");
        assert_eq!(parse_selected_hwnd(&rows[1]), Some(0x12ab));
    }
}
