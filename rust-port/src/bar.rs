use nfm_egui::egui;

#[derive(Clone, Debug)]
pub struct BarSnapshot {
    pub monitor_index: usize,
    pub selected_monitor: bool,
    pub tags: Vec<(usize, String, bool, bool, bool)>,
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

unsafe extern "system" fn bar_message_loop(
    hwnd: windows_sys::Win32::Foundation::HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let span = matches!(
        message,
        WM_NCCREATE | WM_PAINT | WM_TIMER | WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_CLOSE | WM_DESTROY
    )
    .then(|| {
        tracing::info_span!(
            "window_message",
            component = "bar",
            message_name = crate::platform::window_message_name(message),
            message,
            hwnd = hwnd as usize,
            wparam,
            lparam,
        )
    });
    let _entered = span.as_ref().map(tracing::Span::enter);
    match message {
        WM_NCCREATE => {
            let create = unsafe { &*(lparam as *const CREATESTRUCTW) };
            unsafe {
                windows_sys::Win32::Foundation::SetLastError(0);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
                let error = windows_sys::Win32::Foundation::GetLastError();
                if error != 0 {
                    crate::error_log::report_win32("SetWindowLongPtrW(bar userdata) failed", error);
                    return 0;
                }
            }
            1
        }
        WM_PAINT => {
            unsafe { windows_sys::Win32::Foundation::SetLastError(0) };
            let userdata = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
            let userdata_error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            if userdata == 0 && userdata_error != 0 {
                crate::error_log::report_win32(
                    "GetWindowLongPtrW(bar userdata) failed",
                    userdata_error,
                );
            }
            let monitor = (userdata as usize).saturating_sub(1);
            let Some(snapshot) = crate::platform::bar_snapshot(monitor) else {
                return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
            };
            let mut paint: PAINTSTRUCT = unsafe { zeroed() };
            let dc = unsafe { BeginPaint(hwnd, &mut paint) };
            if dc.is_null() {
                crate::platform_error::PlatformError::null_output("BeginPaint(bar)").report();
                return 0;
            }
            let mut client: RECT = unsafe { zeroed() };
            if unsafe { GetClientRect(hwnd, &mut client) } == 0 {
                crate::platform_error::PlatformError::last_win32("GetClientRect(bar)").report();
                // A successful BeginPaint must be paired with EndPaint, even
                // when later paint setup fails.
                unsafe { EndPaint(hwnd, &paint) };
                return 0;
            }
            if let Err(error) = crate::bar_renderer::paint(
                hwnd,
                [client.right.max(0) as u32, client.bottom.max(0) as u32],
                &snapshot,
            ) {
                crate::error_log::report("egui bar paint", error);
            }
            // EndPaint is documented to always return nonzero.
            unsafe { EndPaint(hwnd, &paint) };
            0
        }
        WM_TIMER => {
            if unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) } == 0 {
                crate::error_log::last_win32("InvalidateRect(bar timer) failed");
            }
            0
        }
        WM_ERASEBKGND => 1,
        WM_MOUSEACTIVATE => MA_NOACTIVATE as isize,
        WM_SIZE => {
            unsafe {
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_NCDESTROY => {
            crate::bar_renderer::detach(hwnd);
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_LBUTTONDOWN => {
            unsafe { windows_sys::Win32::Foundation::SetLastError(0) };
            let userdata = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
            let userdata_error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            if userdata == 0 && userdata_error != 0 {
                crate::error_log::report_win32(
                    "GetWindowLongPtrW(bar userdata) failed",
                    userdata_error,
                );
            }
            let monitor = (userdata as usize).saturating_sub(1);
            let x = lparam as i16 as i32;
            if let Some(snapshot) = crate::platform::bar_snapshot(monitor)
                && let Some(workspace) = workspace_at_x(&snapshot, x)
            {
                crate::platform::bar_workspace_clicked(monitor, workspace);
            }
            0
        }
        WM_RBUTTONDOWN => {
            crate::platform::bar_commands_clicked();
            0
        }
        WM_CLOSE => {
            if unsafe { DestroyWindow(hwnd) } == 0 {
                crate::error_log::last_win32("DestroyWindow(bar) failed");
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

const BAR_MARGIN: f32 = 2.0;
const REGION_GAP: f32 = 12.0;
const REGION_PADDING: f32 = 14.0;
const TAG_WIDTH: f32 = 42.0;

fn tag_groups(snapshot: &BarSnapshot) -> Vec<(usize, usize, f32, f32)> {
    if snapshot.tags.is_empty() {
        Vec::new()
    } else {
        let width = REGION_PADDING * 2.0 + snapshot.tags.len() as f32 * TAG_WIDTH;
        vec![(0, snapshot.tags.len(), BAR_MARGIN, BAR_MARGIN + width)]
    }
}

fn workspace_at_x(snapshot: &BarSnapshot, x: i32) -> Option<usize> {
    let x = x as f32;
    for (start, end, left, right) in tag_groups(snapshot) {
        if x >= left + REGION_PADDING && x < right - REGION_PADDING {
            let index = start + ((x - left - REGION_PADDING) / TAG_WIDTH) as usize;
            if index < end {
                return Some(snapshot.tags[index].0);
            }
        }
    }
    None
}

pub(super) fn paint_bar(ui: &mut egui::Ui, size: [u32; 2], snapshot: &BarSnapshot) {
    use egui::{Color32, FontFamily, FontId, Rect, pos2};
    let canvas = ui.ctx().layer_painter(egui::LayerId::background());
    let normal_font = FontId::new(
        snapshot.normal_font_size.max(1) as f32,
        FontFamily::Name("bar".into()),
    );
    let icon_font = FontId::new(
        snapshot.icon_font_size.max(1) as f32,
        FontFamily::Name("bar".into()),
    );
    let rect = |left, top, right, bottom| Rect::from_min_max(pos2(left, top), pos2(right, bottom));
    let top = 4.0;
    let bottom = size[1] as f32 - 4.0;
    let groups = tag_groups(snapshot);
    for (start, end, left, right) in &groups {
        draw_region(
            &canvas,
            rect(*left, top, *right, bottom),
            snapshot.selected_monitor,
            false,
        );
        for index in *start..*end {
            let (_, tag, selected, has_clients, is_icon) = &snapshot.tags[index];
            let item_left = *left + REGION_PADDING + (index - *start) as f32 * TAG_WIDTH;
            let item = rect(item_left, top + 3.0, item_left + TAG_WIDTH, bottom - 3.0);
            if *selected {
                canvas.rect_filled(
                    item,
                    7,
                    if snapshot.selected_monitor {
                        Color32::from_rgb(0x28, 0x28, 0x28)
                    } else {
                        Color32::from_rgb(0x66, 0x5c, 0x54)
                    },
                );
            }
            let color = if snapshot.selected_monitor {
                if *selected {
                    Color32::from_rgb(0xeb, 0xdb, 0xb2)
                } else {
                    Color32::from_rgb(0x28, 0x28, 0x28)
                }
            } else if *selected {
                Color32::from_rgb(0xeb, 0xdb, 0xb2)
            } else {
                Color32::from_rgb(0x92, 0x83, 0x74)
            };
            draw_centered_text(
                &canvas,
                tag,
                rect(item.left(), item.top(), item.right(), item.bottom() - 6.0),
                if *is_icon { &icon_font } else { &normal_font },
                color,
            );
            if *has_clients {
                canvas.rect_filled(
                    rect(
                        item.center().x - 6.0,
                        item.bottom() - 4.0,
                        item.center().x + 6.0,
                        item.bottom() - 1.0,
                    ),
                    egui::CornerRadius::same(2),
                    color,
                );
            }
        }
    }
    let mut left = groups
        .last()
        .map(|group| group.3 + REGION_GAP)
        .unwrap_or(BAR_MARGIN);
    let status = snapshot
        .segments
        .iter()
        .rev()
        .map(|segment| {
            format!(
                "{}{}{}",
                segment
                    .separator
                    .as_ref()
                    .map(|part| part.0.as_str())
                    .unwrap_or(""),
                segment
                    .header
                    .as_ref()
                    .map(|part| part.0.as_str())
                    .unwrap_or(""),
                segment.value
            )
        })
        .collect::<String>();
    let status_width = text_width(&canvas, &normal_font, &status) + REGION_PADDING * 2.0;
    let status_left = (size[0] as f32 - BAR_MARGIN - status_width).max(left);
    let info_width = (text_width(&canvas, &normal_font, &snapshot.workspace_info)
        + REGION_PADDING * 2.0)
        .max(180.0);
    let info = rect(left, top, left + info_width, bottom);
    let region_text = if snapshot.selected_monitor {
        Color32::from_rgb(0xeb, 0xdb, 0xb2)
    } else {
        Color32::from_rgb(0xa8, 0x99, 0x84)
    };
    draw_region(&canvas, info, false, false);
    draw_centered_text(
        &canvas,
        &snapshot.workspace_info,
        info,
        &normal_font,
        region_text,
    );
    left = info.right() + REGION_GAP;
    let desired_focus = (text_width(&canvas, &normal_font, &snapshot.focused_window)
        + REGION_PADDING * 2.0)
        .max(240.0);
    let available_right = status_left - REGION_GAP;
    let focus_width = desired_focus.min((available_right - left).max(0.0));
    let centered_left = size[0] as f32 / 2.0 - focus_width / 2.0;
    let focus_left = centered_left
        .max(left)
        .min((available_right - focus_width).max(left));
    if focus_width > 0.0 {
        let focus = rect(focus_left, top, focus_left + focus_width, bottom);
        draw_region(
            &canvas,
            focus,
            false,
            snapshot.selected_monitor && !snapshot.focused_window.is_empty(),
        );
        draw_centered_text(
            &canvas,
            &snapshot.focused_window,
            focus,
            &normal_font,
            region_text,
        );
    }
    if status_left < size[0] as f32 - BAR_MARGIN {
        let region = rect(status_left, top, size[0] as f32 - BAR_MARGIN, bottom);
        draw_region(&canvas, region, false, false);
        draw_centered_text(&canvas, &status, region, &normal_font, region_text);
    }
}

fn text_width(canvas: &egui::Painter, font: &egui::FontId, text: &str) -> f32 {
    canvas
        .layout_no_wrap(text.to_owned(), font.clone(), egui::Color32::WHITE)
        .size()
        .x
}

fn draw_region(
    canvas: &egui::Painter,
    rect: egui::Rect,
    focused_monitor_background: bool,
    focus_outline: bool,
) {
    use egui::{Color32, Stroke, StrokeKind};
    canvas.add(
        egui::epaint::Shadow {
            offset: [0, 2],
            blur: 5,
            spread: 0,
            color: Color32::from_black_alpha(45),
        }
        .as_shape(rect, 8),
    );
    canvas.rect_filled(
        rect,
        8,
        if focused_monitor_background {
            Color32::from_rgb(0x83, 0xa5, 0x98)
        } else {
            Color32::from_rgb(0x28, 0x28, 0x28)
        },
    );
    if focus_outline {
        canvas.rect_stroke(
            rect.shrink(0.75),
            egui::CornerRadius::same(7),
            Stroke::new(1.5, Color32::from_rgb(0x83, 0xa5, 0x98)),
            StrokeKind::Middle,
        );
    }
}

fn draw_centered_text(
    canvas: &egui::Painter,
    text: &str,
    rect: egui::Rect,
    font: &egui::FontId,
    color: egui::Color32,
) {
    // Clip long titles horizontally so they cannot cover adjacent regions.
    // Keep the bar's vertical clip: short workspace-label regions reserve room
    // for occupancy markers but must not crop the font's ascenders/descenders.
    let clip = egui::Rect::from_min_max(
        egui::pos2(rect.left(), canvas.clip_rect().top()),
        egui::pos2(rect.right(), canvas.clip_rect().bottom()),
    );
    canvas.with_clip_rect(clip).text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        font.clone(),
        color,
    );
}

pub fn create_bars(
    monitors: &[crate::Monitor],
    bar_height: i32,
) -> crate::platform_error::PlatformResult<Vec<BarWindow>> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Controls::ShowScrollBar;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let class_name: Vec<u16> = "keys2BarWindowClass\0".encode_utf16().collect();
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    if instance.is_null() {
        return Err(crate::platform_error::PlatformError::last_win32(
            "GetModuleHandleW(bar)",
        ));
    }
    let mut class: WNDCLASSEXW = unsafe { zeroed() };
    class.cbSize = size_of::<WNDCLASSEXW>() as u32;
    class.style = CS_OWNDC;
    class.lpfnWndProc = Some(bar_message_loop);
    class.hInstance = instance;
    class.lpszClassName = class_name.as_ptr();
    class.hCursor = unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) };
    if class.hCursor.is_null() {
        crate::error_log::last_win32("LoadCursorW(bar) failed");
    }
    class.hIcon = unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) };
    if class.hIcon.is_null() {
        crate::error_log::last_win32("LoadIconW(bar) failed");
    }
    class.hIconSm = class.hIcon;
    class.hbrBackground = std::ptr::null_mut();
    unsafe {
        if RegisterClassExW(&class) == 0 {
            let error = crate::platform_error::PlatformError::last_win32("RegisterClassExW(bar)");
            if !error.is_win32(ERROR_CLASS_ALREADY_EXISTS) {
                return Err(error);
            }
        }
    }
    let title: Vec<u16> = "SimpleWM Bar\0".encode_utf16().collect();
    let mut result: Vec<BarWindow> = Vec::new();
    for (index, monitor) in monitors
        .iter()
        .enumerate()
        .filter(|(_, monitor)| !monitor.is_hidden)
    {
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_POPUP | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
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
            let error = crate::platform_error::PlatformError::last_win32("CreateWindowExW(bar)");
            for bar in &result {
                if unsafe { DestroyWindow(bar.hwnd as _) } == 0 {
                    crate::platform_error::PlatformError::last_win32(
                        "DestroyWindow(bar creation cleanup)",
                    )
                    .report();
                }
            }
            return Err(error);
        }
        if let Err(error) = crate::bar_renderer::attach(hwnd) {
            crate::error_log::report("egui bar initialization", error);
            unsafe {
                DestroyWindow(hwnd);
                for bar in &result {
                    DestroyWindow(bar.hwnd as _);
                }
            }
            return Err(crate::platform_error::PlatformError::api_code(
                "egui bar initialization",
                -1,
            ));
        }
        unsafe {
            if ShowScrollBar(hwnd, SB_BOTH, 0) == 0 {
                crate::error_log::last_win32("ShowScrollBar(bar) failed");
            }
            if SetTimer(hwnd, 0, 1500, None) == 0 {
                let error = crate::platform_error::PlatformError::last_win32("SetTimer(bar)");
                if DestroyWindow(hwnd) == 0 {
                    crate::platform_error::PlatformError::last_win32(
                        "DestroyWindow(bar timer cleanup)",
                    )
                    .report();
                }
                for bar in &result {
                    if DestroyWindow(bar.hwnd as _) == 0 {
                        crate::platform_error::PlatformError::last_win32(
                            "DestroyWindow(bar creation cleanup)",
                        )
                        .report();
                    }
                }
                return Err(error);
            }
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            if windows_sys::Win32::Graphics::Gdi::UpdateWindow(hwnd) == 0 {
                crate::error_log::last_win32("UpdateWindow(bar) failed");
            }
        }
        result.push(BarWindow {
            hwnd: hwnd as isize,
            monitor_index: index,
        });
    }
    Ok(result)
}

pub fn redraw(bars: &[BarWindow]) {
    for bar in bars {
        unsafe {
            if windows_sys::Win32::Graphics::Gdi::InvalidateRect(bar.hwnd as _, std::ptr::null(), 0)
                == 0
            {
                crate::error_log::last_win32("InvalidateRect(bar redraw) failed");
            }
        }
    }
}

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
fn local_date() -> String {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    let mut time: SYSTEMTIME = unsafe { zeroed() };
    unsafe {
        windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut time);
    }
    format!("{:04}-{:02}-{:02}", time.wYear, time.wMonth, time.wDay)
}
fn memory_percent() -> String {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut value: MEMORYSTATUSEX = unsafe { zeroed() };
    value.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    unsafe {
        if GlobalMemoryStatusEx(&mut value) == 0 {
            crate::platform_error::PlatformError::last_win32("GlobalMemoryStatusEx").report();
            return "  ?%".into();
        }
    }
    format!("{:3}%", value.dwMemoryLoad)
}

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
        crate::error_log::last_win32("GetSystemTimes failed");
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workspace_hit_targets_use_the_rendered_pixel_layout() {
        let mut snapshot = crate::bar_renderer::tests::snapshot();
        snapshot.tags[0].0 = 10;
        snapshot.tags[1].0 = 20;
        assert_eq!(workspace_at_x(&snapshot, 15), None);
        assert_eq!(workspace_at_x(&snapshot, 16), Some(10));
        assert_eq!(workspace_at_x(&snapshot, 57), Some(10));
        assert_eq!(workspace_at_x(&snapshot, 58), Some(20));
        assert_eq!(workspace_at_x(&snapshot, 142), None);
    }
}
