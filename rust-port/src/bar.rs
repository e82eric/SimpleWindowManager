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
            paint_skia(dc, client.right, client.bottom, &snapshot);
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
const SELECTED_MONITOR_DOT_RADIUS: f32 = 5.0;
const SELECTED_MONITOR_DOT_GAP: f32 = 11.0;
const SELECTED_MONITOR_DOT_COUNT: usize = 4;

fn color_ref(value: u32) -> skia_safe::Color {
    skia_safe::Color::from_argb(
        255,
        (value & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        ((value >> 16) & 0xff) as u8,
    )
}

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

fn icon_tag(_index: usize, configured: &str, _is_icon: bool) -> String {
    configured.to_owned()
}

fn paint_skia(
    dc: windows_sys::Win32::Graphics::Gdi::HDC,
    width: i32,
    height: i32,
    snapshot: &BarSnapshot,
) {
    use skia_safe::{Font, FontMgr, FontStyle, Paint, Rect, surfaces};
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, SRCCOPY, StretchDIBits,
    };
    if width <= 0 || height <= 0 {
        return;
    }
    let Some(mut surface) = surfaces::raster_n32_premul((width, height)) else {
        return;
    };
    let canvas = surface.canvas();
    // canvas.clear(skia_safe::Color::from_rgb(0x1d, 0x20, 0x21));
    canvas.clear(skia_safe::Color::from_rgb(0x3c, 0x38, 0x36));

    let manager = FontMgr::default();
    let typeface = resolve_bar_typeface(&manager, &snapshot.font_name, FontStyle::normal())
        .or_else(|| manager.legacy_make_typeface(None, FontStyle::normal()));
    let Some(typeface) = typeface else {
        return;
    };
    let mut normal_font = Font::new(typeface.clone(), snapshot.normal_font_size as f32);
    normal_font.set_subpixel(true);
    let mut icon_font = Font::new(typeface, snapshot.icon_font_size as f32);
    icon_font.set_subpixel(true);

    let top = 4.0;
    let bottom = height as f32 - 4.0;
    let groups = tag_groups(snapshot);
    for (start, end, left, right) in &groups {
        let region = Rect::new(*left, top, *right, bottom);
        draw_region(canvas, region, snapshot.selected_monitor);
        for index in *start..*end {
            let (_, tag, selected, has_clients, is_icon) = &snapshot.tags[index];
            let item_left = *left + REGION_PADDING + (index - *start) as f32 * TAG_WIDTH;
            let item = Rect::new(item_left, top + 3.0, item_left + TAG_WIDTH, bottom - 3.0);
            if *selected {
                let mut selected_paint = Paint::default();
                selected_paint.set_anti_alias(true);
                selected_paint.set_color(color_ref(snapshot.focus_background_color));
                canvas.draw_round_rect(item, 7.0, 7.0, &selected_paint);
            }
            let color = if *selected {
                snapshot.focus_text_color
            } else if *has_clients {
                snapshot.text_color
            } else {
                snapshot.disabled_color
            };
            let display_tag = icon_tag(index, tag, *is_icon);
            draw_centered_text(
                canvas,
                &display_tag,
                item,
                if *is_icon { &icon_font } else { &normal_font },
                color_ref(color),
            );
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
    let status_width = text_width(&normal_font, &status) + REGION_PADDING * 2.0;
    let status_left = (width as f32 - BAR_MARGIN - status_width).max(left);

    let info_width =
        (text_width(&normal_font, &snapshot.workspace_info) + REGION_PADDING * 2.0).max(180.0);
    let info = Rect::new(left, top, left + info_width, bottom);
    draw_region(canvas, info, snapshot.selected_monitor);
    draw_centered_text(
        canvas,
        &snapshot.workspace_info,
        info,
        &normal_font,
        color_ref(snapshot.text_color),
    );
    left = info.right + REGION_GAP;

    if snapshot.selected_monitor {
        let diameter = SELECTED_MONITOR_DOT_RADIUS * 2.0;
        let indicator_width = SELECTED_MONITOR_DOT_COUNT as f32 * diameter
            + (SELECTED_MONITOR_DOT_COUNT - 1) as f32 * SELECTED_MONITOR_DOT_GAP;
        let mut dot_paint = Paint::default();
        dot_paint.set_anti_alias(true);
        dot_paint.set_color(skia_safe::Color::from_rgb(0xfa, 0xbd, 0x2f));
        for index in 0..SELECTED_MONITOR_DOT_COUNT {
            let center_x = left
                + SELECTED_MONITOR_DOT_RADIUS
                + index as f32 * (diameter + SELECTED_MONITOR_DOT_GAP);
            canvas.draw_circle(
                (center_x, (top + bottom) / 2.0),
                SELECTED_MONITOR_DOT_RADIUS,
                &dot_paint,
            );
        }
        left += indicator_width + REGION_GAP;
    }

    let desired_focus =
        (text_width(&normal_font, &snapshot.focused_window) + REGION_PADDING * 2.0).max(240.0);
    let available_right = status_left - REGION_GAP;
    let focus_width = desired_focus.min((available_right - left).max(0.0));
    let centered_left = width as f32 / 2.0 - focus_width / 2.0;
    let focus_left = centered_left
        .max(left)
        .min((available_right - focus_width).max(left));
    let focus_right = focus_left + focus_width;
    if focus_width > 0.0 {
        let focus = Rect::new(focus_left, top, focus_right, bottom);
        draw_region(canvas, focus, snapshot.selected_monitor);
        draw_centered_text(
            canvas,
            &snapshot.focused_window,
            focus,
            &normal_font,
            color_ref(snapshot.text_color),
        );
    }

    if status_left < width as f32 - BAR_MARGIN {
        let region = Rect::new(status_left, top, width as f32 - BAR_MARGIN, bottom);
        draw_region(canvas, region, snapshot.selected_monitor);
        draw_centered_text(
            canvas,
            &status,
            region,
            &normal_font,
            color_ref(snapshot.text_color),
        );
    }
    let Some(pixmap) = surface.peek_pixels() else {
        return;
    };
    let Some(bytes) = pixmap.bytes() else {
        return;
    };
    let mut bitmap = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        ..Default::default()
    };
    unsafe {
        let copied = StretchDIBits(
            dc,
            0,
            0,
            width,
            height,
            0,
            0,
            width,
            height,
            bytes.as_ptr().cast(),
            &mut bitmap,
            DIB_RGB_COLORS,
            SRCCOPY,
        );
        if copied == -1 {
            crate::error_log::last_win32("StretchDIBits(bar) failed");
        }
    }
}

fn resolve_bar_typeface(
    manager: &skia_safe::FontMgr,
    family: &str,
    style: skia_safe::FontStyle,
) -> Option<skia_safe::Typeface> {
    let supports_bar_icons = |face: &skia_safe::Typeface| {
        ['\u{f028}', '\u{ea7a}', '\u{f127}', '\u{f128}']
            .into_iter()
            .all(|character| face.unichar_to_glyph(character as i32) != 0)
    };
    let named = manager.legacy_make_typeface(family, style);
    if named.as_ref().is_some_and(supports_bar_icons) {
        return named;
    }
    if let Some(fonts) = std::env::var_os("LOCALAPPDATA").map(|root| {
        std::path::PathBuf::from(root)
            .join("Microsoft")
            .join("Windows")
            .join("Fonts")
    }) {
        for file in [
            "JetBrainsMonoNLNerdFontPropo-Regular.ttf",
            "JetBrainsMonoNerdFontPropo-Regular.ttf",
            "JetBrainsMonoNLNerdFontMono-Regular.ttf",
        ] {
            if let Ok(bytes) = std::fs::read(fonts.join(file))
                && let Some(face) = manager.new_from_data(&bytes, None)
                && supports_bar_icons(&face)
            {
                return Some(face);
            }
        }
    }
    named
}

fn text_width(font: &skia_safe::Font, text: &str) -> f32 {
    font.measure_str(text, None).0
}

fn draw_region(canvas: &skia_safe::Canvas, rect: skia_safe::Rect, _selected_monitor: bool) {
    use skia_safe::{Color, Paint};
    let mut shadow = Paint::default();
    shadow.set_anti_alias(true);
    shadow.set_color(Color::from_argb(45, 0, 0, 0));
    shadow.set_mask_filter(skia_safe::MaskFilter::blur(
        skia_safe::BlurStyle::Normal,
        2.5,
        true,
    ));
    canvas.draw_round_rect(rect.with_offset((0.0, 2.0)), 8.0, 8.0, &shadow);

    let mut panel = Paint::default();
    panel.set_anti_alias(true);
    // panel.set_color(Color::from_rgb(0x3c, 0x38, 0x36));
    panel.set_color(Color::from_rgb(0x28, 0x28, 0x28));
    canvas.draw_round_rect(rect, 8.0, 8.0, &panel);
}

fn draw_centered_text(
    canvas: &skia_safe::Canvas,
    text: &str,
    rect: skia_safe::Rect,
    font: &skia_safe::Font,
    color: skia_safe::Color,
) {
    let mut paint = skia_safe::Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(color);
    let (width, bounds) = font.measure_str(text, Some(&paint));
    let x = rect.left + (rect.width() - width) / 2.0;
    let y = rect.center_y() - (bounds.top + bounds.bottom) / 2.0;
    canvas.draw_str(text, (x, y), font, &paint);
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
            ShowWindow(hwnd, SW_SHOWDEFAULT);
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
