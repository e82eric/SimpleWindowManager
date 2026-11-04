//! Keyboard-driven region screenshots. The monitor is captured first and shown
//! frozen, with a digit on each visible window of the workspace. Typing one
//! makes that window the rectangle; arrows or hjkl then trim it (see
//! `selector::translate`), and Enter copies it to the clipboard. With one
//! window the rectangle starts as it, and with none as the whole monitor.
//!
//! Runs on the UI thread. Drawing is plain GDI on a back buffer.
use std::cell::{Cell, RefCell};
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

mod selector;
use selector::{Edges, Outcome, Region, Selector, State};

/// How long the copied rectangle flashes before the overlay closes, like a
/// yank highlight.
const YANK_FLASH_MS: u32 = 200;
const YANK_TIMER: usize = 1;
/// How bright the monitor stays around the rectangle, out of 255.
const DIMMED: u32 = 110;

const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}
/// Gruvbox: dark background, orange for windows and the rectangle, yellow for
/// the corner being moved and the flash once copied.
const DARK: u32 = rgb(0x1d, 0x20, 0x21);
const LIGHT: u32 = rgb(0xeb, 0xdb, 0xb2);
const ORANGE: u32 = rgb(0xfe, 0x80, 0x19);
const YELLOW: u32 = rgb(0xfa, 0xbd, 0x2f);
/// How strongly the copied rectangle is washed with `YELLOW`, out of 255.
const YANKED_TINT: u32 = 90;
/// How strongly each window is washed with `ORANGE` while picking, out of
/// 255: enough to show what its digit covers, while its content stays readable.
const WINDOW_TINT: u32 = 64;
/// How strongly the rectangle is washed with `ORANGE` while trimming, out of
/// 255: fainter, so it still looks like what will be copied.
const SELECTION_TINT: u32 = 32;
/// How wide the rectangle's border is.
const EDGE: i32 = 4;
/// The side of the square on the corner the keys move now.
const MARKER: i32 = 24;

impl From<Region> for RECT {
    fn from(r: Region) -> Self {
        RECT {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
}

/// A 32-bit top-down DIB selected into its own memory DC.
struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u8,
    width: i32,
    height: i32,
}
impl Surface {
    fn new(width: i32, height: i32) -> Option<Self> {
        let mut info: BITMAPINFO = unsafe { std::mem::zeroed() };
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width;
        info.bmiHeader.biHeight = -height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut bits = std::ptr::null_mut();
        let bitmap = unsafe {
            CreateDIBSection(
                std::ptr::null_mut(),
                &info,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            )
        };
        if bitmap.is_null() || bits.is_null() {
            crate::error_log::last_win32("CreateDIBSection(screenshot) failed");
            return None;
        }
        let dc = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
        if dc.is_null() {
            crate::error_log::last_win32("CreateCompatibleDC(screenshot) failed");
            unsafe { DeleteObject(bitmap) };
            return None;
        }
        let old = unsafe { SelectObject(dc, bitmap) };
        Some(Self {
            dc,
            bitmap,
            old,
            bits: bits.cast(),
            width,
            height,
        })
    }
    fn len(&self) -> usize {
        (self.width * self.height * 4) as usize
    }
    /// The pixels as blue, green, red, unused; GDI drawing must be flushed
    /// first (`GdiFlush`).
    fn pixels(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.bits, self.len()) }
    }
    fn pixels_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.bits, self.len()) }
    }
    /// Blends `color` over `r`, `alpha` out of 255, like a yank highlight.
    fn tint(&mut self, r: Region, color: u32, alpha: u32) {
        // Earlier GDI drawing must land in the bits before they are touched.
        unsafe { GdiFlush() };
        let (left, right) = (r.left.max(0) as usize, r.right.min(self.width) as usize);
        let (top, bottom) = (r.top.max(0) as usize, r.bottom.min(self.height) as usize);
        // DIB pixels are blue, green, red, unused; COLORREF is 0x00bbggrr.
        let overlay = [(color >> 16) & 0xff, (color >> 8) & 0xff, color & 0xff];
        let stride = self.width as usize * 4;
        let pixels = self.pixels_mut();
        for y in top..bottom {
            for pixel in pixels[y * stride + left * 4..y * stride + right * 4].chunks_exact_mut(4) {
                for (channel, &over) in pixel[..3].iter_mut().zip(&overlay) {
                    *channel = ((u32::from(*channel) * (255 - alpha) + over * alpha) / 255) as u8;
                }
            }
        }
    }
    /// A copy of `self` at `DIMMED` brightness, for around the rectangle.
    fn dimmed(&self) -> Option<Self> {
        let mut dimmed = Self::new(self.width, self.height)?;
        unsafe {
            BitBlt(
                dimmed.dc,
                0,
                0,
                self.width,
                self.height,
                self.dc,
                0,
                0,
                SRCCOPY,
            );
            GdiFlush();
        }
        for channel in dimmed.pixels_mut() {
            *channel = (u32::from(*channel) * DIMMED / 255) as u8;
        }
        Some(dimmed)
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            DeleteDC(self.dc);
            DeleteObject(self.bitmap);
        }
    }
}

struct Session {
    selector: Selector,
    /// The frozen monitor, as captured, and darkened for around the rectangle.
    original: Surface,
    dimmed: Surface,
    back: Surface,
    /// For the windows' digits, and the rectangle's size.
    hint_font: HFONT,
    label_font: HFONT,
    /// The rectangle just copied, flashing until the overlay closes; keys are
    /// ignored meanwhile.
    yanked: Option<Region>,
}
impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.hint_font);
            DeleteObject(self.label_font);
        }
    }
}

thread_local! {
    static WINDOW: Cell<HWND> = const { Cell::new(std::ptr::null_mut()) };
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
}

/// Monitor bounds containing `point`, in virtual-screen pixels.
fn monitor_rect(point: POINT) -> Option<RECT> {
    let monitor = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST) };
    let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    (unsafe { GetMonitorInfoW(monitor, &mut info) } != 0).then_some(info.rcMonitor)
}

/// The foreground window, unless it is one of SWM's own.
pub(crate) fn foreground_window() -> Option<isize> {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.is_null() {
        return None;
    }
    let mut process = 0;
    unsafe { GetWindowThreadProcessId(foreground, &mut process) };
    let ours = process == unsafe { windows_sys::Win32::System::Threading::GetCurrentProcessId() };
    (!ours).then_some(foreground as isize)
}

/// The visible bounds of `hwnd`, without its invisible resize borders, in
/// virtual-screen pixels; None if it is hidden, cloaked or minimized.
fn window_bounds(hwnd: isize) -> Option<RECT> {
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
    };
    let hwnd = hwnd as HWND;
    if unsafe { IsWindowVisible(hwnd) } == 0 || unsafe { IsIconic(hwnd) } != 0 {
        return None;
    }
    // Cloaked windows, such as those behind the top of a deck, still count as
    // visible to `IsWindowVisible`.
    let mut cloaked: u32 = 0;
    let read = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED as u32,
            (&mut cloaked as *mut u32).cast(),
            std::mem::size_of::<u32>() as u32,
        )
    };
    if read == 0 && cloaked != 0 {
        return None;
    }
    let mut rect: RECT = unsafe { std::mem::zeroed() };
    let found = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS as u32,
            (&mut rect as *mut RECT).cast(),
            std::mem::size_of::<RECT>() as u32,
        )
    } == 0
        || unsafe { GetWindowRect(hwnd, &mut rect) } != 0;
    (found && rect.right > rect.left && rect.bottom > rect.top).then_some(rect)
}

/// Start a screenshot on the monitor containing `point`, with a digit on each
/// of `windows` that is showing. Does nothing if one is already open.
pub(crate) fn start(point: (i32, i32), windows: &[isize], hint_font_size: i32) {
    if SESSION.with(|session| session.borrow().is_some()) {
        return;
    }
    let Some(bounds) = monitor_rect(POINT {
        x: point.0,
        y: point.1,
    }) else {
        crate::error_log::last_win32("GetMonitorInfoW(screenshot) failed");
        return;
    };
    let width = bounds.right - bounds.left;
    let height = bounds.bottom - bounds.top;
    if width <= 0 || height <= 0 {
        return;
    }
    // Capture before anything of ours appears, so menus and tooltips survive.
    let Some(original) = Surface::new(width, height) else {
        return;
    };
    // Let the compositor present one more frame so a window hidden just before
    // this, such as the commands picker that ran it, is gone from the capture.
    unsafe { windows_sys::Win32::Graphics::Dwm::DwmFlush() };
    let screen = unsafe { GetDC(std::ptr::null_mut()) };
    let copied = unsafe {
        BitBlt(
            original.dc,
            0,
            0,
            width,
            height,
            screen,
            bounds.left,
            bounds.top,
            SRCCOPY | CAPTUREBLT,
        )
    };
    unsafe { ReleaseDC(std::ptr::null_mut(), screen) };
    if copied == 0 {
        crate::error_log::last_win32("BitBlt(screenshot capture) failed");
        return;
    }
    let (Some(dimmed), Some(back), Some(hwnd)) =
        (original.dimmed(), Surface::new(width, height), window())
    else {
        return;
    };
    let windows: Vec<Region> = windows
        .iter()
        .filter_map(|&hwnd| window_bounds(hwnd))
        .map(|w| Region {
            left: w.left - bounds.left,
            top: w.top - bounds.top,
            right: w.right - bounds.left,
            bottom: w.bottom - bounds.top,
        })
        .collect();
    let session = Session {
        selector: Selector::new(width, height, &windows),
        original,
        dimmed,
        back,
        hint_font: font(hint_font_size),
        label_font: font((hint_font_size / 5).max(14)),
        yanked: None,
    };
    SESSION.with(|slot| *slot.borrow_mut() = Some(session));
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            bounds.left,
            bounds.top,
            width,
            height,
            SWP_SHOWWINDOW,
        );
        // A zero-scan-code key event makes this the process with the last input,
        // which permits taking the foreground from a hotkey.
        keybd_event(0, 0, 0, 0);
        SetForegroundWindow(hwnd);
        SetFocus(hwnd);
        InvalidateRect(hwnd, std::ptr::null(), 0);
    }
}

/// Bold Segoe UI, `height` pixels tall.
fn font(height: i32) -> HFONT {
    let face = wide("Segoe UI\0");
    unsafe {
        CreateFontW(
            -height,
            0,
            0,
            0,
            FW_BOLD as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET.into(),
            OUT_DEFAULT_PRECIS.into(),
            CLIP_DEFAULT_PRECIS.into(),
            CLEARTYPE_QUALITY.into(),
            0,
            face.as_ptr(),
        )
    }
}

fn close() {
    // Taken before hiding: hiding deactivates the window, which closes again.
    let Some(session) = SESSION.with(|slot| slot.borrow_mut().take()) else {
        return;
    };
    let hwnd = WINDOW.get();
    unsafe {
        KillTimer(hwnd, YANK_TIMER);
        ShowWindow(hwnd, SW_HIDE);
    }
    drop(session);
}

fn window() -> Option<HWND> {
    let existing = WINDOW.get();
    if !existing.is_null() {
        return Some(existing);
    }
    use std::mem::{size_of, zeroed};
    let class_name = wide("SWMScreenshot\0");
    let instance =
        unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()) };
    let mut class: WNDCLASSEXW = unsafe { zeroed() };
    class.cbSize = size_of::<WNDCLASSEXW>() as u32;
    class.lpfnWndProc = Some(window_proc);
    class.hInstance = instance;
    class.lpszClassName = class_name.as_ptr();
    class.hCursor = unsafe { LoadCursorW(std::ptr::null_mut(), IDC_CROSS) };
    if unsafe { RegisterClassExW(&class) } == 0
        && unsafe { windows_sys::Win32::Foundation::GetLastError() }
            != windows_sys::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS
    {
        crate::error_log::last_win32("RegisterClassExW(screenshot) failed");
        return None;
    }
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            class_name.as_ptr(),
            class_name.as_ptr(),
            WS_POPUP,
            0,
            0,
            100,
            100,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        crate::error_log::last_win32("CreateWindowExW(screenshot) failed");
        return None;
    }
    WINDOW.set(hwnd);
    Some(hwnd)
}

fn key_down(key: u16) -> bool {
    unsafe { GetKeyState(i32::from(key)) as u16 & 0x8000 != 0 }
}

fn on_key(hwnd: HWND, key: u16) {
    let Some(input) = selector::translate(key, key_down(VK_CONTROL), key_down(VK_SHIFT)) else {
        return;
    };
    let outcome = SESSION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let session = slot.as_mut().filter(|session| session.yanked.is_none())?;
        let outcome = session.selector.handle(input);
        if let Outcome::Capture(region) = outcome {
            copy_to_clipboard(hwnd, &session.original, region);
            session.yanked = Some(region);
        }
        Some(outcome)
    });
    match outcome {
        None => {}
        Some(Outcome::Cancel) => close(),
        Some(Outcome::Continue) => unsafe {
            InvalidateRect(hwnd, std::ptr::null(), 0);
        },
        // Flash what was copied, then close.
        Some(Outcome::Capture(_)) => unsafe {
            InvalidateRect(hwnd, std::ptr::null(), 0);
            if SetTimer(hwnd, YANK_TIMER, YANK_FLASH_MS, None) == 0 {
                close();
            }
        },
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match message {
        WM_PAINT => {
            let mut paint: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            let dc = unsafe { BeginPaint(hwnd, &mut paint) };
            SESSION.with(|slot| {
                if let Some(session) = slot.borrow_mut().as_mut() {
                    render(session);
                    let back = &session.back;
                    unsafe { BitBlt(dc, 0, 0, back.width, back.height, back.dc, 0, 0, SRCCOPY) };
                }
            });
            unsafe { EndPaint(hwnd, &paint) };
            0
        }
        // Shift changes which corner is highlighted; its auto-repeat doesn't.
        WM_KEYDOWN | WM_KEYUP
            if wparam as u16 == VK_SHIFT && (message == WM_KEYUP || lparam & (1 << 30) == 0) =>
        {
            unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
            0
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            on_key(hwnd, wparam as u16);
            0
        }
        WM_TIMER if wparam == YANK_TIMER => {
            close();
            0
        }
        // Swallow Alt releases left over from the hotkey so they don't open a
        // system menu.
        WM_SYSKEYUP | WM_SYSCHAR | WM_CHAR => 0,
        WM_SYSCOMMAND if (wparam & 0xfff0) as u32 == SC_KEYMENU => 0,
        WM_ACTIVATE if wparam & 0xffff == WA_INACTIVE as usize => {
            close();
            0
        }
        WM_ERASEBKGND => 1,
        WM_NCDESTROY => {
            WINDOW.set(std::ptr::null_mut());
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

// ---------------------------------------------------------------------------
// Drawing.
// ---------------------------------------------------------------------------

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

fn fill(dc: HDC, r: Region, color: u32) {
    unsafe {
        let brush = CreateSolidBrush(color);
        FillRect(dc, &r.into(), brush);
        DeleteObject(brush);
    }
}

/// A `thickness`-pixel line just inside `r`.
fn frame(dc: HDC, r: Region, color: u32, thickness: i32) {
    unsafe {
        let brush = CreateSolidBrush(color);
        for inset in 0..thickness {
            FrameRect(dc, &r.outset(-inset).into(), brush);
        }
        DeleteObject(brush);
    }
}

fn text_size(dc: HDC, text: &[u16]) -> (i32, i32) {
    let mut size = windows_sys::Win32::Foundation::SIZE { cx: 0, cy: 0 };
    unsafe { GetTextExtentPoint32W(dc, text.as_ptr(), text.len() as i32, &mut size) };
    (size.cx, size.cy)
}

/// `text` centered in `r`.
fn centered_text(dc: HDC, r: Region, text: &mut [u16], color: u32) {
    unsafe {
        SetTextColor(dc, color);
        DrawTextW(
            dc,
            text.as_mut_ptr(),
            text.len() as i32,
            &mut r.into(),
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }
}

/// Half the width and height of `text`'s badge in the selected font: padded,
/// and at least square.
fn badge_half_size(dc: HDC, text: &[u16]) -> (i32, i32) {
    let (width, height) = text_size(dc, text);
    let half_height = height * 3 / 5;
    (((width + height / 2) / 2).max(half_height), half_height)
}

/// `text` on a dark badge outlined in `color`, centered on (`x`, `y`).
fn badge(dc: HDC, x: i32, y: i32, text: &str, color: u32, border: i32) {
    let mut text = wide(text);
    let (half_width, half_height) = badge_half_size(dc, &text);
    let r = Region {
        left: x - half_width,
        top: y - half_height,
        right: x + half_width,
        bottom: y + half_height,
    };
    fill(dc, r, DARK);
    frame(dc, r, color, border);
    centered_text(dc, r, &mut text, color);
}

fn render(session: &mut Session) {
    let s = &session.selector;
    let dc = session.back.dc;
    let blit = |source: &Surface, r: Region| unsafe {
        BitBlt(
            dc,
            r.left,
            r.top,
            r.width(),
            r.height(),
            source.dc,
            r.left,
            r.top,
            SRCCOPY,
        );
    };
    let monitor = Region {
        left: 0,
        top: 0,
        right: s.width,
        bottom: s.height,
    };
    unsafe { SetBkMode(dc, TRANSPARENT as _) };
    match (session.yanked, s.state) {
        // Just copied: the rectangle, flashing, and nothing else.
        (Some(r), _) => {
            blit(&session.dimmed, monitor);
            blit(&session.original, r);
            session.back.tint(r, YELLOW, YANKED_TINT);
            frame(dc, r.outset(6), YELLOW, 6);
        }
        // Each window washed orange and outlined, like a drop target, with its
        // digit in the middle.
        (None, State::Pick) => {
            blit(&session.original, monitor);
            for &r in &s.windows {
                session.back.tint(r, ORANGE, WINDOW_TINT);
            }
            for &r in &s.windows {
                frame(dc, r, DARK, 6);
                frame(dc, r, ORANGE, 2);
            }
            unsafe { SelectObject(dc, session.hint_font) };
            for (index, r) in s.windows.iter().enumerate() {
                let (x, y) = r.center();
                badge(dc, x, y, &(index + 1).to_string(), ORANGE, 4);
            }
        }
        // The rectangle as it will be copied, the rest dimmed, with a border
        // just outside it so it hides none of what is copied.
        (None, State::Adjust(r)) => {
            blit(&session.dimmed, monitor);
            blit(&session.original, r);
            session.back.tint(r, ORANGE, SELECTION_TINT);
            selection_border(dc, r, active_edges());
            size_label(session, r);
        }
    }
}

/// The border just outside `r`, edged dark so it shows over light content,
/// with a yellow square on the corner the keys move now (`active`): top-left,
/// or bottom-right while Shift is held.
fn selection_border(dc: HDC, r: Region, active: Edges) {
    frame(dc, r.outset(EDGE + 1), DARK, 1);
    frame(dc, r.outset(EDGE), ORANGE, EDGE);
    // Centered on the border's corner. A square's pixels run from half its
    // size before its point to just short of half after, so this rounds
    // toward `r` on the top-left and away on the bottom-right.
    let (x, y) = match active {
        Edges::TopLeft => (r.left - EDGE / 2, r.top - EDGE / 2),
        Edges::BottomRight => (r.right + (EDGE + 1) / 2, r.bottom + (EDGE + 1) / 2),
    };
    let square = Region {
        left: x - MARKER / 2,
        top: y - MARKER / 2,
        right: x + MARKER / 2,
        bottom: y + MARKER / 2,
    };
    fill(dc, square, YELLOW);
    frame(dc, square, DARK, 2);
}

/// The edges the keys move now: the bottom and right ones while Shift is held.
fn active_edges() -> Edges {
    if key_down(VK_SHIFT) {
        Edges::BottomRight
    } else {
        Edges::TopLeft
    }
}

/// The rectangle's size, under its bottom-right corner, or inside it there
/// when that is off the monitor.
fn size_label(session: &Session, r: Region) {
    let s = &session.selector;
    let dc = session.back.dc;
    unsafe { SelectObject(dc, session.label_font) };
    let text = format!("{} \u{d7} {}", r.width(), r.height());
    let (half_width, half_height) = badge_half_size(dc, &wide(&text));
    // Clear of the square on the bottom-right corner.
    const GAP: i32 = MARKER / 2 + 6;
    let x = (r.right - half_width)
        .min(s.width - half_width)
        .max(half_width);
    let below = r.bottom + GAP + half_height;
    let y = if below + half_height <= s.height {
        below
    } else {
        r.bottom - GAP - half_height
    };
    badge(dc, x, y, &text, LIGHT, 1);
}

/// Puts `region` of `image` on the clipboard as a 24-bit bottom-up DIB.
fn copy_to_clipboard(owner: HWND, image: &Surface, region: Region) {
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock,
    };
    const CF_DIB: u32 = 8;
    let (width, height) = (region.width() as usize, region.height() as usize);
    let stride = (width * 3 + 3) & !3;
    let header = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width as i32,
        biHeight: height as i32,
        biPlanes: 1,
        biBitCount: 24,
        biCompression: BI_RGB,
        biSizeImage: (stride * height) as u32,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    };
    let mut dib: Vec<u8> = unsafe {
        std::slice::from_raw_parts(
            (&header as *const BITMAPINFOHEADER).cast::<u8>(),
            std::mem::size_of::<BITMAPINFOHEADER>(),
        )
    }
    .to_vec();
    let header_size = dib.len();
    dib.resize(header_size + stride * height, 0);
    let source = image.pixels();
    let source_stride = image.width as usize * 4;
    let (left, bottom) = (region.left as usize, region.bottom as usize);
    // Bottom-up: the last source row comes first; the fourth byte is dropped.
    for (y, row) in dib[header_size..].chunks_exact_mut(stride).enumerate() {
        let start = (bottom - 1 - y) * source_stride + left * 4;
        let source_row = &source[start..start + width * 4];
        for (out, pixel) in row.chunks_exact_mut(3).zip(source_row.chunks_exact(4)) {
            out.copy_from_slice(&pixel[..3]);
        }
    }
    unsafe {
        let data = GlobalAlloc(GMEM_MOVEABLE, dib.len());
        if data.is_null() {
            crate::error_log::last_win32("GlobalAlloc(screenshot) failed");
            return;
        }
        let buffer = GlobalLock(data) as *mut u8;
        if buffer.is_null() {
            GlobalFree(data);
            return;
        }
        std::ptr::copy_nonoverlapping(dib.as_ptr(), buffer, dib.len());
        GlobalUnlock(data);
        if OpenClipboard(owner) == 0 {
            crate::error_log::last_win32("OpenClipboard(screenshot) failed");
            GlobalFree(data);
            return;
        }
        if EmptyClipboard() == 0 || SetClipboardData(CF_DIB, data).is_null() {
            crate::error_log::last_win32("SetClipboardData(screenshot) failed");
            GlobalFree(data);
        }
        CloseClipboard();
    }
}
