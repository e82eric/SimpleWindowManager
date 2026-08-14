#[derive(Clone, Debug)]
pub struct BarSnapshot {
    pub monitor_index: usize,
    pub selected_monitor: bool,
    pub tags: Vec<(String, bool, bool, bool)>,
    pub workspace_info: String,
    pub focused_window: String,
    pub segments: Vec<RenderedSegment>,
    pub background_color: u32,
    pub focus_background_color: u32,
    pub text_color: u32,
    pub info_color: u32,
    pub disabled_color: u32,
    pub focus_text_color: u32,
    pub extra_focus_background_color: u32,
    pub font_name: String,
    pub normal_font_size: i32,
    pub icon_font_size: i32,
}

#[derive(Clone, Debug)]
pub struct RenderedSegment {
    pub separator: Option<(String, bool)>,
    pub header: Option<(String, bool)>,
    pub value: String,
    pub value_is_icon: bool,
}

#[derive(Debug)]
pub struct BarWindow {
    pub hwnd: isize,
    pub monitor_index: usize,
}

#[cfg(windows)]
unsafe extern "system" fn bar_message_loop(
    hwnd: windows_sys::Win32::Foundation::HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{
        BeginPaint, CreateSolidBrush, DT_CENTER, DT_LEFT, DT_SINGLELINE, DT_VCENTER, DeleteObject,
        EndPaint, FillRect, InvalidateRect, PAINTSTRUCT, SelectObject, SetBkMode, SetTextColor,
        TRANSPARENT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    match message {
        WM_NCCREATE => {
            let create = unsafe { &*(lparam as *const CREATESTRUCTW) };
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
            }
            1
        }
        WM_PAINT => {
            let monitor =
                unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as usize }.saturating_sub(1);
            let Some(snapshot) = crate::platform::bar_snapshot(monitor) else {
                return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
            };
            let mut paint: PAINTSTRUCT = unsafe { zeroed() };
            let dc = unsafe { BeginPaint(hwnd, &mut paint) };
            let brush = unsafe { CreateSolidBrush(snapshot.background_color) };
            let mut client: RECT = unsafe { zeroed() };
            unsafe {
                GetClientRect(hwnd, &mut client);
                FillRect(dc, &client, brush);
                SetBkMode(dc, TRANSPARENT as i32);
            }
            let normal_font = create_font(&snapshot.font_name, snapshot.normal_font_size);
            let icon_font = create_font(&snapshot.font_name, snapshot.icon_font_size);
            for (index, (tag, selected, has_clients, is_icon)) in snapshot.tags.iter().enumerate() {
                let mut rect = RECT {
                    left: index as i32 * 30,
                    top: 0,
                    right: index as i32 * 30 + 30,
                    bottom: client.bottom,
                };
                if *selected {
                    let selected_brush =
                        unsafe { CreateSolidBrush(snapshot.focus_background_color) };
                    unsafe {
                        FillRect(dc, &rect, selected_brush);
                        DeleteObject(selected_brush);
                    }
                }
                unsafe {
                    SetTextColor(
                        dc,
                        if *selected {
                            snapshot.focus_text_color
                        } else if *has_clients {
                            snapshot.text_color
                        } else {
                            snapshot.disabled_color
                        },
                    );
                    SelectObject(dc, if *is_icon { icon_font } else { normal_font });
                }
                draw_text(dc, tag, &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                if *selected {
                    let indicator =
                        unsafe { CreateSolidBrush(snapshot.extra_focus_background_color) };
                    let indicator_rect = RECT {
                        left: rect.left,
                        top: rect.bottom - 3,
                        right: rect.right,
                        bottom: rect.bottom,
                    };
                    unsafe {
                        FillRect(dc, &indicator_rect, indicator);
                        DeleteObject(indicator);
                    }
                }
            }
            let mut description_rect = RECT {
                left: snapshot.tags.len() as i32 * 30,
                top: 0,
                right: status_left(dc, &snapshot.segments, client.right, normal_font, icon_font),
                bottom: client.bottom,
            };
            if snapshot.selected_monitor {
                let focus = unsafe { CreateSolidBrush(snapshot.focus_background_color) };
                unsafe {
                    FillRect(dc, &description_rect, focus);
                    DeleteObject(focus);
                }
            }
            unsafe {
                SetTextColor(dc, snapshot.text_color);
                SelectObject(dc, normal_font);
            }
            draw_text(
                dc,
                &snapshot.workspace_info,
                &mut description_rect,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );
            let mut focused_rect = client;
            draw_text(
                dc,
                &snapshot.focused_window,
                &mut focused_rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
            let mut right = client.right;
            for (index, segment) in snapshot.segments.iter().enumerate() {
                let padding = if index == 0 { 10 } else { 0 };
                right -= padding;
                right = draw_status_part(
                    dc,
                    &segment.value,
                    segment.value_is_icon,
                    right,
                    client.bottom,
                    snapshot.text_color,
                    normal_font,
                    icon_font,
                );
                if let Some((header, icon)) = &segment.header {
                    right = draw_status_part(
                        dc,
                        header,
                        *icon,
                        right,
                        client.bottom,
                        snapshot.info_color,
                        normal_font,
                        icon_font,
                    );
                }
                if let Some((separator, icon)) = &segment.separator {
                    right = draw_status_part(
                        dc,
                        separator,
                        *icon,
                        right,
                        client.bottom,
                        snapshot.info_color,
                        normal_font,
                        icon_font,
                    );
                }
            }
            unsafe {
                DeleteObject(brush);
                DeleteObject(normal_font);
                DeleteObject(icon_font);
                EndPaint(hwnd, &paint);
            }
            0
        }
        WM_TIMER => {
            unsafe {
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_ERASEBKGND => 1,
        WM_LBUTTONDOWN => {
            let monitor =
                unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as usize }.saturating_sub(1);
            let x = lparam as i16 as i32;
            if let Some(snapshot) = crate::platform::bar_snapshot(monitor)
                && x >= 0
                && x < snapshot.tags.len() as i32 * 30
            {
                crate::platform::bar_workspace_clicked(monitor, (x / 30) as usize);
            }
            0
        }
        WM_RBUTTONDOWN => {
            crate::platform::bar_commands_clicked();
            0
        }
        WM_CLOSE => {
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

#[cfg(windows)]
fn status_left(
    dc: windows_sys::Win32::Graphics::Gdi::HDC,
    segments: &[RenderedSegment],
    mut right: i32,
    normal_font: windows_sys::Win32::Graphics::Gdi::HFONT,
    icon_font: windows_sys::Win32::Graphics::Gdi::HFONT,
) -> i32 {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{DT_CALCRECT, DrawTextW, SelectObject};
    let width = |text: &str, icon: bool| {
        let wide: Vec<u16> = text.encode_utf16().collect();
        let mut measured = RECT::default();
        unsafe {
            SelectObject(dc, if icon { icon_font } else { normal_font });
            DrawTextW(
                dc,
                wide.as_ptr(),
                wide.len() as i32,
                &mut measured,
                DT_CALCRECT,
            );
        }
        measured.right - measured.left
    };
    for (index, segment) in segments.iter().enumerate() {
        if index == 0 {
            right -= 10;
        }
        right -= width(&segment.value, segment.value_is_icon);
        if let Some((header, icon)) = &segment.header {
            right -= width(header, *icon);
        }
        if let Some((separator, icon)) = &segment.separator {
            right -= width(separator, *icon);
        }
    }
    right
}

#[cfg(windows)]
fn draw_status_part(
    dc: windows_sys::Win32::Graphics::Gdi::HDC,
    text: &str,
    icon: bool,
    right: i32,
    bottom: i32,
    color: u32,
    normal_font: windows_sys::Win32::Graphics::Gdi::HFONT,
    icon_font: windows_sys::Win32::Graphics::Gdi::HFONT,
) -> i32 {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{
        DT_CALCRECT, DT_LEFT, DT_SINGLELINE, DT_VCENTER, DrawTextW, SelectObject, SetTextColor,
    };
    let wide: Vec<u16> = text.encode_utf16().collect();
    let font = if icon { icon_font } else { normal_font };
    unsafe {
        SelectObject(dc, font);
    }
    let mut measured = RECT::default();
    unsafe {
        DrawTextW(
            dc,
            wide.as_ptr(),
            wide.len() as i32,
            &mut measured,
            DT_CALCRECT,
        );
    }
    let left = right - (measured.right - measured.left);
    let mut rect = RECT {
        left,
        top: 0,
        right,
        bottom,
    };
    unsafe {
        SetTextColor(dc, color);
        DrawTextW(
            dc,
            wide.as_ptr(),
            wide.len() as i32,
            &mut rect,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
        );
    }
    left
}

#[cfg(windows)]
fn create_font(name: &str, size: i32) -> windows_sys::Win32::Graphics::Gdi::HFONT {
    use windows_sys::Win32::Graphics::Gdi::*;
    let name: Vec<u16> = name.encode_utf16().chain([0]).collect();
    let screen = unsafe { GetDC(std::ptr::null_mut()) };
    let dpi = unsafe { GetDeviceCaps(screen, LOGPIXELSX as i32) };
    unsafe { ReleaseDC(std::ptr::null_mut(), screen) };
    let size = size * dpi / 96;
    unsafe {
        CreateFontW(
            -size,
            0,
            0,
            0,
            FW_NORMAL as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            DEFAULT_QUALITY as u32,
            (DEFAULT_PITCH | FF_DONTCARE) as u32,
            name.as_ptr(),
        )
    }
}

#[cfg(windows)]
fn draw_text(
    dc: windows_sys::Win32::Graphics::Gdi::HDC,
    text: &str,
    rect: &mut windows_sys::Win32::Foundation::RECT,
    flags: u32,
) {
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        windows_sys::Win32::Graphics::Gdi::DrawTextW(
            dc,
            wide.as_ptr(),
            wide.len() as i32,
            rect,
            flags,
        );
    }
}

#[cfg(windows)]
pub fn create_bars(monitors: &[crate::Monitor], bar_height: i32) -> Result<Vec<BarWindow>, u32> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Graphics::Gdi::COLOR_WINDOW;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Controls::ShowScrollBar;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let class_name: Vec<u16> = "keys2BarWindowClass\0".encode_utf16().collect();
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    let mut class: WNDCLASSEXW = unsafe { zeroed() };
    class.cbSize = size_of::<WNDCLASSEXW>() as u32;
    class.lpfnWndProc = Some(bar_message_loop);
    class.hInstance = instance;
    class.lpszClassName = class_name.as_ptr();
    class.hCursor = unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) };
    class.hIcon = unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) };
    class.hIconSm = class.hIcon;
    class.hbrBackground = (COLOR_WINDOW as usize + 1) as _;
    unsafe {
        RegisterClassExW(&class);
    }
    let title: Vec<u16> = "SimpleWM Bar\0".encode_utf16().collect();
    let mut result = Vec::new();
    for (index, monitor) in monitors
        .iter()
        .enumerate()
        .filter(|(_, monitor)| !monitor.is_hidden)
    {
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_CONTROLPARENT | WS_EX_COMPOSITED,
                class_name.as_ptr(),
                title.as_ptr(),
                !((WS_CAPTION
                    | WS_THICKFRAME
                    | WS_MINIMIZE
                    | WS_MAXIMIZE
                    | WS_SYSMENU
                    | WS_DISABLED
                    | WS_BORDER
                    | WS_DLGFRAME
                    | WS_SIZEBOX) as u32),
                monitor.x_offset,
                0,
                monitor.width,
                bar_height,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                (index + 1) as *mut _,
            )
        };
        if hwnd.is_null() {
            return Err(unsafe { GetLastError() });
        }
        unsafe {
            ShowScrollBar(hwnd, SB_BOTH, 0);
            SetTimer(hwnd, 0, 1500, None);
            ShowWindow(hwnd, SW_SHOWDEFAULT);
            windows_sys::Win32::Graphics::Gdi::UpdateWindow(hwnd);
        }
        result.push(BarWindow {
            hwnd: hwnd as isize,
            monitor_index: index,
        });
    }
    Ok(result)
}

#[cfg(not(windows))]
pub fn create_bars(_monitors: &[crate::Monitor], _bar_height: i32) -> Result<Vec<BarWindow>, u32> {
    Ok(Vec::new())
}

#[cfg(windows)]
pub fn redraw(bars: &[BarWindow]) {
    for bar in bars {
        unsafe {
            windows_sys::Win32::Graphics::Gdi::InvalidateRect(bar.hwnd as _, std::ptr::null(), 0);
        }
    }
}

#[cfg(not(windows))]
pub fn redraw(_bars: &[BarWindow]) {}

pub fn status_text(segments: &[crate::config::BarSegmentConfiguration]) -> String {
    segments
        .iter()
        .map(|segment| {
            let value = segment_value(segment.value);
            format!(
                "{}{}{:>width$}",
                segment.separator_text.as_deref().unwrap_or(""),
                segment.header_text.as_deref().unwrap_or(""),
                value,
                width = segment.variable_text_fixed_width
            )
        })
        .collect()
}

pub fn rendered_segments(
    segments: &[crate::config::BarSegmentConfiguration],
) -> Vec<RenderedSegment> {
    segments
        .iter()
        .map(|segment| RenderedSegment {
            separator: segment
                .separator_text
                .clone()
                .map(|text| (text, segment.separator_is_icon)),
            header: segment
                .header_text
                .clone()
                .map(|text| (text, segment.header_is_icon)),
            value: format!(
                "{:>width$}",
                segment_value(segment.value),
                width = segment.variable_text_fixed_width
            ),
            value_is_icon: segment.variable_is_icon,
        })
        .collect()
}

fn segment_value(value: crate::config::BarSegmentValue) -> String {
    use crate::config::BarSegmentValue::*;
    match value {
        SystemTime => system_time(false),
        LocalTime => system_time(true),
        LocalDate => local_date(),
        Memory => memory_percent(),
        Cpu => cpu_percent(),
        Volume => crate::status::volume_percent(),
        Internet => crate::status::internet_icon(),
    }
}

#[cfg(windows)]
fn system_time(local: bool) -> String {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    let mut time: SYSTEMTIME = unsafe { zeroed() };
    unsafe {
        if local {
            windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut time);
        } else {
            windows_sys::Win32::System::SystemInformation::GetSystemTime(&mut time);
        }
    }
    format!("{:02}:{:02}", time.wHour, time.wMinute)
}
#[cfg(not(windows))]
fn system_time(_local: bool) -> String {
    String::new()
}
#[cfg(windows)]
fn local_date() -> String {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    let mut time: SYSTEMTIME = unsafe { zeroed() };
    unsafe {
        windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut time);
    }
    format!("{:04}-{:02}-{:02}", time.wYear, time.wMonth, time.wDay)
}
#[cfg(not(windows))]
fn local_date() -> String {
    String::new()
}
#[cfg(windows)]
fn memory_percent() -> String {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut value: MEMORYSTATUSEX = unsafe { zeroed() };
    value.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    unsafe {
        GlobalMemoryStatusEx(&mut value);
    }
    format!("{:3}%", value.dwMemoryLoad)
}

#[cfg(windows)]
fn cpu_percent() -> String {
    use std::mem::zeroed;
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::GetSystemTimes;
    static STATE: Mutex<(u64, u64, u64, i32, Option<std::time::Instant>)> =
        Mutex::new((0, 0, 0, 0, None));
    let mut state = STATE.lock().unwrap();
    if state
        .4
        .is_some_and(|last| last.elapsed() < std::time::Duration::from_secs(1))
    {
        return format!("{:3}%", state.3);
    }
    state.4 = Some(std::time::Instant::now());
    let mut idle: FILETIME = unsafe { zeroed() };
    let mut kernel: FILETIME = unsafe { zeroed() };
    let mut user: FILETIME = unsafe { zeroed() };
    if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
        return "  0%".into();
    }
    let value = |time: FILETIME| (time.dwHighDateTime as u64) << 32 | time.dwLowDateTime as u64;
    let (idle, kernel, user) = (value(idle), value(kernel), value(user));
    let total = kernel.saturating_sub(state.1) + user.saturating_sub(state.2);
    let result = if state.1 != 0 && total != 0 {
        ((total.saturating_sub(idle.saturating_sub(state.0))) * 100 / total) as i32
    } else {
        state.3
    };
    if (0..=100).contains(&result) {
        state.3 = result;
    }
    state.0 = idle;
    state.1 = kernel;
    state.2 = user;
    format!("{:3}%", state.3)
}
#[cfg(not(windows))]
fn cpu_percent() -> String {
    String::new()
}
#[cfg(not(windows))]
fn memory_percent() -> String {
    String::new()
}
