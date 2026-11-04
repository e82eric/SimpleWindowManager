//! Win32-only egui host. No global hooks, raw-input registration, synthetic keys,
//! or attachment to another thread's input queue.
use crate::egui_window_menu::WindowMenuApp;
use glow::HasContext;
use nfm_egui::{
    egui,
    renderer::{Renderer, glow},
};
use std::{
    ffi::CString,
    sync::Arc,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*, OpenGL::*},
    System::{DataExchange::*, LibraryLoader::*, Memory::*},
    UI::{
        HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext},
        Input::{Ime::*, KeyboardAndMouse::*},
        WindowsAndMessaging::*,
    },
};
use windows_sys::Win32::{System::Threading::INFINITE, UI::Controls::MARGINS};

const WAKE: u32 = WM_APP + 109;
const CLASS: &str = "SWM.NativeEguiPicker";
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn win_error(operation: &str) -> String {
    format!("{operation}: Win32 error {}", unsafe { GetLastError() })
}

#[derive(Default)]
struct Input {
    events: Vec<egui::Event>,
    close_requested: bool,
    deadline: Option<Instant>,
    surrogate: Option<u16>,
    composing: bool,
    modifiers: egui::Modifiers,
    pointer: egui::Pos2,
}
impl Input {
    fn set_modifiers(&mut self, modifiers: egui::Modifiers) {
        self.modifiers = modifiers;
        self.events.push(egui::Event::ModifiersChanged(modifiers));
    }
    fn wake(&mut self, delay: Duration) {
        let Some(deadline) = Instant::now().checked_add(delay) else {
            return;
        };
        self.deadline = Some(self.deadline.map_or(deadline, |old| old.min(deadline)));
    }
    fn text(&mut self, unit: u16) {
        if (0xd800..=0xdbff).contains(&unit) {
            self.surrogate = Some(unit);
            return;
        }
        let scalar = if (0xdc00..=0xdfff).contains(&unit) {
            self.surrogate
                .take()
                .map(|high| 0x10000 + ((high as u32 - 0xd800) << 10) + unit as u32 - 0xdc00)
        } else {
            self.surrogate = None;
            Some(unit as u32)
        };
        if let Some(ch) = scalar
            .and_then(char::from_u32)
            .filter(|ch| !ch.is_control())
        {
            self.events.push(egui::Event::Text(ch.to_string()));
        }
    }
}

fn modifiers() -> egui::Modifiers {
    let down = |key: u16| unsafe { GetKeyState(key as i32) as u16 & 0x8000 != 0 };
    let ctrl = down(VK_CONTROL);
    egui::Modifiers {
        alt: down(VK_MENU),
        ctrl,
        shift: down(VK_SHIFT),
        mac_cmd: false,
        command: ctrl,
    }
}
fn key(vk: u32, shift: bool) -> Option<egui::Key> {
    use egui::Key::*;
    Some(match vk {
        0x08 => Backspace,
        0x09 => Tab,
        0x0d => Enter,
        0x1b => Escape,
        0x20 => Space,
        0x21 => PageUp,
        0x22 => PageDown,
        0x23 => End,
        0x24 => Home,
        0x25 => ArrowLeft,
        0x26 => ArrowUp,
        0x27 => ArrowRight,
        0x28 => ArrowDown,
        0x2d => Insert,
        0x2e => Delete,
        0x30 => Num0,
        0x31 => Num1,
        0x32 => Num2,
        0x33 => Num3,
        0x34 => Num4,
        0x35 => Num5,
        0x36 => Num6,
        0x37 => Num7,
        0x38 => Num8,
        0x39 => Num9,
        0x41 => A,
        0x42 => B,
        0x43 => C,
        0x44 => D,
        0x45 => E,
        0x46 => F,
        0x47 => G,
        0x48 => H,
        0x49 => I,
        0x4a => J,
        0x4b => K,
        0x4c => L,
        0x4d => M,
        0x4e => N,
        0x4f => O,
        0x50 => P,
        0x51 => Q,
        0x52 => R,
        0x53 => S,
        0x54 => T,
        0x55 => U,
        0x56 => V,
        0x57 => W,
        0x58 => X,
        0x59 => Y,
        0x5a => Z,
        0x70 => F1,
        0x71 => F2,
        0x72 => F3,
        0x73 => F4,
        0x74 => F5,
        0x75 => F6,
        0x76 => F7,
        0x77 => F8,
        0x78 => F9,
        0x79 => F10,
        0x7a => F11,
        0x7b => F12,
        0xba => Semicolon,
        0xbb => Equals,
        0xbc => Comma,
        0xbd => Minus,
        0xbe => Period,
        0xbf if shift => Questionmark,
        0xbf => Slash,
        0xc0 => Backtick,
        0xdb => OpenBracket,
        0xdc => Backslash,
        0xdd => CloseBracket,
        0xde => Quote,
        _ => return None,
    })
}

unsafe fn clipboard_read(hwnd: HWND) -> Option<String> {
    unsafe {
        if OpenClipboard(hwnd) == 0 {
            return None;
        }
        let data = GetClipboardData(13);
        let mut result = None;
        if !data.is_null() {
            let ptr = GlobalLock(data) as *const u16;
            if !ptr.is_null() {
                let max = GlobalSize(data) / 2;
                let units = std::slice::from_raw_parts(ptr, max);
                let end = units.iter().position(|&x| x == 0).unwrap_or(max);
                result = Some(String::from_utf16_lossy(&units[..end]));
                GlobalUnlock(data);
            }
        }
        CloseClipboard();
        result
    }
}
unsafe fn clipboard_write(hwnd: HWND, text: &str) {
    unsafe {
        if OpenClipboard(hwnd) == 0 {
            return;
        }
        let units = wide(text);
        let data = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2);
        if !data.is_null() {
            let ptr = GlobalLock(data) as *mut u16;
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(units.as_ptr(), ptr, units.len());
                GlobalUnlock(data);
                if EmptyClipboard() == 0 || SetClipboardData(13, data).is_null() {
                    GlobalFree(data);
                }
            } else {
                GlobalFree(data);
            }
        }
        CloseClipboard();
    }
}
unsafe fn composition(hwnd: HWND, kind: u32) -> String {
    unsafe {
        let context = ImmGetContext(hwnd);
        if context.is_null() {
            return String::new();
        }
        let count = ImmGetCompositionStringW(context, kind, std::ptr::null_mut(), 0);
        let mut units = vec![0u16; count.max(0) as usize / 2];
        if !units.is_empty() {
            ImmGetCompositionStringW(context, kind, units.as_mut_ptr().cast(), count as u32);
        }
        ImmReleaseContext(hwnd, context);
        String::from_utf16_lossy(&units)
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    unsafe {
        if msg == WM_NCCREATE {
            let create = &*(lparam as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Input;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        // Never call reentrant native APIs while retaining this mutable reference.
        match msg {
            WAKE => {
                (&mut *ptr).wake(Duration::from_millis(wparam as u64));
                return 0;
            }
            WM_ERASEBKGND => return 1,
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                BeginPaint(hwnd, &mut ps);
                EndPaint(hwnd, &ps);
                (&mut *ptr).wake(Duration::ZERO);
                return 0;
            }
            WM_CLOSE => {
                (&mut *ptr).close_requested = true;
                (&mut *ptr).wake(Duration::ZERO);
                return 0;
            }
            WM_SIZE | WM_DPICHANGED => {
                (&mut *ptr).wake(Duration::ZERO);
            }
            WM_SETFOCUS | WM_KILLFOCUS => {
                let input = &mut *ptr;
                let mods = if msg == WM_SETFOCUS {
                    modifiers()
                } else {
                    egui::Modifiers::NONE
                };
                input.surrogate = None;
                input
                    .events
                    .push(egui::Event::WindowFocused(msg == WM_SETFOCUS));
                input.set_modifiers(mods);
                input.wake(Duration::ZERO);
            }
            WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP => {
                let mods = modifiers();
                let pressed = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
                let repeat = pressed && lparam as u32 & (1 << 30) != 0;
                let paste = pressed
                    && ((mods.ctrl && wparam == b'V' as usize)
                        || (mods.shift && wparam == VK_INSERT as usize));
                let pasted = if paste { clipboard_read(hwnd) } else { None };
                let input = &mut *ptr;
                input.set_modifiers(mods);
                if pressed && mods.alt && wparam == VK_F4 as usize {
                    input.close_requested = true;
                    input.wake(Duration::ZERO);
                    return 0;
                }
                if let Some(text) = pasted {
                    input.events.push(egui::Event::Paste(text));
                } else if pressed && mods.ctrl && !mods.shift && wparam == b'X' as usize {
                    input.events.push(egui::Event::Cut);
                } else if let Some(logical_key) = key(wparam as u32, mods.shift) {
                    input.events.push(egui::Event::Key {
                        key: logical_key,
                        physical_key: key(wparam as u32, false),
                        pressed,
                        repeat,
                        modifiers: mods,
                    });
                }
                input.wake(Duration::ZERO);
                // Suppress the popup's native Alt menu behavior; global SWM hook remains independent.
                return 0;
            }
            WM_CHAR | WM_SYSCHAR => {
                let input = &mut *ptr;
                if !input.composing {
                    input.text(wparam as u16);
                }
                input.wake(Duration::ZERO);
                return 0;
            }
            WM_IME_STARTCOMPOSITION => {
                (&mut *ptr).composing = true;
                return 0;
            }
            WM_IME_COMPOSITION => {
                if lparam as u32 & GCS_RESULTSTR != 0 {
                    let text = composition(hwnd, GCS_RESULTSTR);
                    (&mut *ptr)
                        .events
                        .push(egui::Event::Ime(egui::ImeEvent::Commit(text)));
                } else if lparam as u32 & GCS_COMPSTR != 0 {
                    let text = composition(hwnd, GCS_COMPSTR);
                    (&mut *ptr)
                        .events
                        .push(egui::Event::Ime(egui::ImeEvent::Preedit {
                            text,
                            active_range_chars: None,
                        }));
                }
                (&mut *ptr).wake(Duration::ZERO);
                return 0;
            }
            WM_IME_ENDCOMPOSITION => {
                (&mut *ptr).composing = false;
                (&mut *ptr)
                    .events
                    .push(egui::Event::Ime(egui::ImeEvent::Preedit {
                        text: String::new(),
                        active_range_chars: None,
                    }));
                (&mut *ptr).wake(Duration::ZERO);
                return 0;
            }
            WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP
            | WM_MBUTTONDOWN | WM_MBUTTONUP => {
                let scale = windows_sys::Win32::UI::HiDpi::GetDpiForWindow(hwnd) as f32 / 96.0;
                let pos = egui::pos2(
                    lparam as u16 as i16 as f32 / scale,
                    (lparam >> 16) as u16 as i16 as f32 / scale,
                );
                let mods = modifiers();
                if matches!(msg, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN) {
                    SetCapture(hwnd);
                }
                if matches!(msg, WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP) {
                    ReleaseCapture();
                }
                let input = &mut *ptr;
                input.pointer = pos;
                input.events.push(egui::Event::PointerMoved(pos));
                if msg != WM_MOUSEMOVE {
                    let button = match msg {
                        WM_LBUTTONDOWN | WM_LBUTTONUP => egui::PointerButton::Primary,
                        WM_RBUTTONDOWN | WM_RBUTTONUP => egui::PointerButton::Secondary,
                        _ => egui::PointerButton::Middle,
                    };
                    input.events.push(egui::Event::PointerButton {
                        pos,
                        button,
                        pressed: matches!(msg, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN),
                        modifiers: mods,
                    });
                }
                input.wake(Duration::ZERO);
                return 0;
            }
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                let delta = (wparam >> 16) as u16 as i16 as f32 / 120.0 * 3.0;
                let input = &mut *ptr;
                input.events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    phase: egui::TouchPhase::Move,
                    delta: if msg == WM_MOUSEWHEEL {
                        egui::vec2(0.0, delta)
                    } else {
                        egui::vec2(delta, 0.0)
                    },
                    modifiers: modifiers(),
                });
                input.wake(Duration::ZERO);
                return 0;
            }
            _ => {}
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

struct NativeWindow {
    hwnd: HWND,
    dc: HDC,
    rc: HGLRC,
    module: HMODULE,
    input: Box<Input>,
}
impl NativeWindow {
    fn new() -> Result<Self, String> {
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class = wide(CLASS);
            let wc = WNDCLASSW {
                style: CS_OWNDC,
                lpfnWndProc: Some(wndproc),
                hInstance: instance,
                hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                lpszClassName: class.as_ptr(),
                ..Default::default()
            };
            if RegisterClassW(&wc) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
                return Err(win_error("RegisterClassW"));
            }
            let mut window = Self {
                hwnd: std::ptr::null_mut(),
                dc: std::ptr::null_mut(),
                rc: std::ptr::null_mut(),
                module: std::ptr::null_mut(),
                input: Box::default(),
            };
            window.hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                class.as_ptr(),
                wide("SimpleWindowManager window switcher").as_ptr(),
                WS_POPUP | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
                0,
                0,
                1600,
                312,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                (&mut *window.input as *mut Input).cast(),
            );
            if window.hwnd.is_null() {
                return Err(win_error("CreateWindowExW"));
            }
            window.dc = GetDC(window.hwnd);
            if window.dc.is_null() {
                return Err(win_error("GetDC"));
            }
            let pfd = PIXELFORMATDESCRIPTOR {
                nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
                nVersion: 1,
                dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
                iPixelType: PFD_TYPE_RGBA,
                cColorBits: 32,
                cAlphaBits: 8,
                cDepthBits: 24,
                cStencilBits: 8,
                ..Default::default()
            };
            let format = ChoosePixelFormat(window.dc, &pfd);
            if format == 0 || SetPixelFormat(window.dc, format, &pfd) == 0 {
                return Err(win_error("SetPixelFormat"));
            }
            window.rc = wglCreateContext(window.dc);
            if window.rc.is_null() || wglMakeCurrent(window.dc, window.rc) == 0 {
                return Err(win_error("wglCreateContext"));
            }
            if let Some(proc) = wglGetProcAddress(c"wglCreateContextAttribsARB".as_ptr().cast()) {
                let create: unsafe extern "system" fn(HDC, HGLRC, *const i32) -> HGLRC =
                    std::mem::transmute(proc);
                // OpenGL 3.3 core context, independent of any other application's context.
                let modern = create(
                    window.dc,
                    std::ptr::null_mut(),
                    [0x2091, 3, 0x2092, 3, 0x9126, 1, 0].as_ptr(),
                );
                if modern.is_null() {
                    return Err(win_error("OpenGL 3.3 context"));
                }
                wglMakeCurrent(std::ptr::null_mut(), std::ptr::null_mut());
                wglDeleteContext(window.rc);
                window.rc = modern;
                if wglMakeCurrent(window.dc, window.rc) == 0 {
                    return Err(win_error("wglMakeCurrent"));
                }
            } else {
                return Err("OpenGL 3.3 context creation is unavailable".into());
            }
            if let Some(proc) = wglGetProcAddress(c"wglSwapIntervalEXT".as_ptr().cast()) {
                let swap: unsafe extern "system" fn(i32) -> i32 = std::mem::transmute(proc);
                swap(0);
            }
            window.module = LoadLibraryW(wide("opengl32.dll").as_ptr());
            if window.module.is_null() {
                return Err(win_error("LoadLibraryW(opengl32)"));
            }
            let margins = MARGINS {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            };
            if DwmExtendFrameIntoClientArea(window.hwnd, &margins) < 0 {
                return Err("DWM transparency unavailable".into());
            }
            Ok(window)
        }
    }
    fn gl(&self) -> Arc<glow::Context> {
        Arc::new(unsafe {
            glow::Context::from_loader_function(|name| {
                let name = CString::new(name).unwrap();
                let pointer = wglGetProcAddress(name.as_ptr().cast())
                    .map_or(std::ptr::null(), |p| p as *const std::ffi::c_void);
                if pointer.is_null() || [1, 2, 3, usize::MAX].contains(&(pointer as usize)) {
                    GetProcAddress(self.module, name.as_ptr().cast())
                        .map_or(std::ptr::null(), |p| p as *const std::ffi::c_void)
                } else {
                    pointer
                }
            })
        })
    }
}
impl Drop for NativeWindow {
    fn drop(&mut self) {
        unsafe {
            wglMakeCurrent(std::ptr::null_mut(), std::ptr::null_mut());
            if !self.rc.is_null() {
                wglDeleteContext(self.rc);
            }
            if !self.dc.is_null() {
                ReleaseDC(self.hwnd, self.dc);
            }
            if !self.hwnd.is_null() {
                DestroyWindow(self.hwnd);
            }
            if !self.module.is_null() {
                FreeLibrary(self.module);
            }
        }
    }
}

pub(crate) fn run(
    create: impl FnOnce(isize, &egui::Context) -> WindowMenuApp,
    ready: impl FnOnce(),
) -> Result<(), String> {
    // DPI policy belongs to the popup thread, never the WM's other windows.
    unsafe {
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let mut window = NativeWindow::new()?;
    let gl = window.gl();
    let mut renderer = Renderer::new(gl.clone())?;
    let ctx = egui::Context::default();
    let hwnd_id = window.hwnd as usize;
    ctx.set_request_repaint_callback(move |info| unsafe {
        if info.delay != Duration::MAX {
            PostMessageW(
                hwnd_id as HWND,
                WAKE,
                info.delay.as_millis().min(u32::MAX as u128 - 1) as usize,
                0,
            );
        }
    });
    let mut app = create(window.hwnd as isize, &ctx);
    ready();
    let start = Instant::now();
    window.input.wake(Duration::ZERO);
    loop {
        unsafe {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if message.message == WM_QUIT {
                    app.logic(&ctx, true);
                    renderer.destroy();
                    return Ok(());
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        let due = window
            .input
            .deadline
            .is_some_and(|deadline| deadline <= Instant::now());
        if due {
            window.input.deadline = None;
            let close = std::mem::take(&mut window.input.close_requested);
            app.logic(&ctx, close);
            if app.quitting() {
                break;
            }
            let mut rect = RECT::default();
            unsafe {
                GetClientRect(window.hwnd, &mut rect);
            }
            let size = [rect.right.max(1) as u32, rect.bottom.max(1) as u32];
            let scale = unsafe { windows_sys::Win32::UI::HiDpi::GetDpiForWindow(window.hwnd) }
                as f32
                / 96.0;
            let focused = unsafe { GetForegroundWindow() == window.hwnd };
            let mut input = egui::RawInput {
                time: Some(start.elapsed().as_secs_f64()),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(size[0] as f32 / scale, size[1] as f32 / scale),
                )),
                events: std::mem::take(&mut window.input.events),
                focused,
                ..Default::default()
            };
            let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
            viewport.native_pixels_per_point = Some(scale);
            viewport.focused = Some(focused);
            let mut output = ctx.run_ui(input, |ui| app.ui(ui));
            // Always consume texture deltas, including the hidden startup frame.
            unsafe {
                gl.disable(glow::SCISSOR_TEST);
                gl.clear_color(0.0, 0.0, 0.0, 0.0);
                gl.clear(glow::COLOR_BUFFER_BIT);
            }
            renderer.render(&ctx, size, &mut output);
            unsafe {
                // Keep the last complete frame visible while the main thread
                // resolves a menu command. The accepting frame has closed rows.
                if !app.awaiting_handoff() {
                    SwapBuffers(window.dc);
                }
            }
            for command in &output.platform_output.commands {
                if let egui::OutputCommand::CopyText(text) = command {
                    unsafe {
                        clipboard_write(window.hwnd, text);
                    }
                }
            }
            for viewport in output.viewport_output.values() {
                for command in &viewport.commands {
                    unsafe {
                        match command {
                            egui::ViewportCommand::Visible(true) => {
                                ShowWindow(window.hwnd, SW_SHOW);
                            }
                            egui::ViewportCommand::Visible(false) => {
                                ShowWindow(window.hwnd, SW_HIDE);
                            }
                            egui::ViewportCommand::Focus => {
                                SetForegroundWindow(window.hwnd);
                                SetFocus(window.hwnd);
                            }
                            _ => {}
                        }
                    }
                }
            }
            if let Some(ime) = &output.platform_output.ime {
                unsafe {
                    let context = ImmGetContext(window.hwnd);
                    if !context.is_null() {
                        let form = COMPOSITIONFORM {
                            dwStyle: CFS_POINT,
                            ptCurrentPos: POINT {
                                x: (ime.cursor_rect.left() * scale) as i32,
                                y: (ime.cursor_rect.bottom() * scale) as i32,
                            },
                            ..Default::default()
                        };
                        ImmSetCompositionWindow(context, &form);
                        ImmReleaseContext(window.hwnd, context);
                    }
                }
            }
        }
        let timeout = window.input.deadline.map_or(INFINITE, |deadline| {
            deadline
                .saturating_duration_since(Instant::now())
                .as_millis()
                .saturating_add(1)
                .min(u32::MAX as u128 - 1) as u32
        });
        unsafe {
            if MsgWaitForMultipleObjectsEx(
                0,
                std::ptr::null(),
                timeout,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            ) == WAIT_FAILED
            {
                renderer.destroy();
                return Err(win_error("MsgWaitForMultipleObjectsEx"));
            }
        }
    }
    renderer.destroy();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shift_release_and_focus_handoff_do_not_leak_modifiers() {
        let ctx = egui::Context::default();
        let mut input = Input::default();
        input.set_modifiers(egui::Modifiers::ALT | egui::Modifiers::SHIFT);
        ctx.run_ui(
            egui::RawInput {
                events: std::mem::take(&mut input.events),
                ..Default::default()
            },
            |_| {},
        )
        .drop_without_applying_deltas();
        assert!(ctx.input(|state| state.modifiers.shift));
        // Releasing the opening chord updates state even with no egui key event.
        input.set_modifiers(egui::Modifiers::NONE);
        ctx.run_ui(
            egui::RawInput {
                events: std::mem::take(&mut input.events),
                ..Default::default()
            },
            |_| {},
        )
        .drop_without_applying_deltas();
        assert_eq!(ctx.input(|state| state.modifiers), egui::Modifiers::NONE);
        input.set_modifiers(egui::Modifiers::SHIFT);
        input.events.push(egui::Event::WindowFocused(false));
        input.set_modifiers(egui::Modifiers::NONE);
        input.events.push(egui::Event::WindowFocused(true));
        input.set_modifiers(egui::Modifiers::ALT);
        ctx.run_ui(
            egui::RawInput {
                events: std::mem::take(&mut input.events),
                ..Default::default()
            },
            |_| {},
        )
        .drop_without_applying_deltas();
        assert_eq!(ctx.input(|state| state.modifiers), egui::Modifiers::ALT);
    }
    #[test]
    fn utf16_text_and_control_characters() {
        let mut input = Input::default();
        for unit in "a😀\r\u{16}".encode_utf16() {
            input.text(unit);
        }
        assert_eq!(input.events.len(), 2);
        assert!(matches!(&input.events[1], egui::Event::Text(text) if text == "😀"));
    }
    #[test]
    fn shifted_help_key_and_physical_slash() {
        assert_eq!(key(VK_OEM_2 as u32, true), Some(egui::Key::Questionmark));
        assert_eq!(key(VK_OEM_2 as u32, false), Some(egui::Key::Slash));
    }
    #[test]
    fn immediate_wakeup_precedes_delayed_repaint() {
        let mut input = Input::default();
        input.wake(Duration::from_secs(10));
        input.wake(Duration::ZERO);
        assert!(input.deadline.unwrap() <= Instant::now());
        let immediate = input.deadline;
        input.wake(Duration::from_secs(20));
        assert_eq!(input.deadline, immediate);
    }
}
