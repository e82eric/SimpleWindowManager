//! Persistent egui/OpenGL state for the Win32 bars and the mailbox drop-down. All
//! access is on the window thread.
use super::bar::{BarSnapshot, paint_bar};
use glow::HasContext;
use nfm_egui::{
    egui,
    renderer::{Renderer, glow},
};
use std::{cell::RefCell, collections::HashMap, ffi::CString, sync::Arc};
use windows_sys::Win32::{
    Foundation::{FreeLibrary, HMODULE, HWND},
    Graphics::{Gdi::*, OpenGL::*},
    System::LibraryLoader::{GetProcAddress, LoadLibraryW},
};

thread_local! {
    static RENDERERS: RefCell<HashMap<isize, BarRenderer>> = RefCell::new(HashMap::new());
}

pub(crate) fn attach(hwnd: HWND) -> Result<(), String> {
    let renderer = BarRenderer::new(hwnd)?;
    RENDERERS.with(|renderers| renderers.borrow_mut().insert(hwnd as isize, renderer));
    Ok(())
}

pub(crate) fn detach(hwnd: HWND) {
    let renderer = RENDERERS.with(|renderers| renderers.borrow_mut().remove(&(hwnd as isize)));
    drop(renderer);
}

pub(super) fn paint(hwnd: HWND, size: [u32; 2], snapshot: &BarSnapshot) -> Result<(), String> {
    // Do not retain a RefCell borrow across native calls that could dispatch messages.
    let mut renderer = RENDERERS.with(|renderers| renderers.borrow_mut().remove(&(hwnd as isize)));
    let result = match renderer.as_mut() {
        Some(renderer) => renderer.paint(size, snapshot, true),
        None => Ok(()), // WM_PAINT can arrive before initialization completes.
    };
    if let Some(renderer) = renderer {
        RENDERERS.with(|renderers| renderers.borrow_mut().insert(hwnd as isize, renderer));
    }
    result
}

/// Paint `hwnd` with `run`, which receives `events` as focused pointer and keyboard
/// input. Returns whether egui wants another frame right away, such as for hover.
pub(crate) fn paint_ui(
    hwnd: HWND,
    size: [u32; 2],
    font_name: &str,
    events: Vec<egui::Event>,
    run: impl FnMut(&mut egui::Ui),
) -> Result<bool, String> {
    let mut renderer = RENDERERS.with(|renderers| renderers.borrow_mut().remove(&(hwnd as isize)));
    let result = match renderer.as_mut() {
        Some(renderer) => renderer.render(size, font_name, events, true, run),
        None => Ok(false),
    };
    if let Some(renderer) = renderer {
        RENDERERS.with(|renderers| renderers.borrow_mut().insert(hwnd as isize, renderer));
    }
    result
}

fn win_error(operation: &str) -> String {
    format!("{operation}: Win32 error {}", unsafe {
        windows_sys::Win32::Foundation::GetLastError()
    })
}

/// Restore any context belonging to other UI on this thread after rendering a bar.
struct CurrentContext {
    dc: HDC,
    rc: HGLRC,
}
impl CurrentContext {
    fn capture() -> Self {
        unsafe {
            Self {
                dc: wglGetCurrentDC(),
                rc: wglGetCurrentContext(),
            }
        }
    }
}
impl Drop for CurrentContext {
    fn drop(&mut self) {
        unsafe {
            if wglGetCurrentContext() != self.rc && wglMakeCurrent(self.dc, self.rc) == 0 {
                crate::error_log::last_win32("wglMakeCurrent(bar restore) failed");
            }
        }
    }
}

struct BarRenderer {
    hwnd: HWND,
    dc: HDC,
    rc: HGLRC,
    module: HMODULE,
    gl: Option<Arc<glow::Context>>,
    renderer: Option<Renderer>,
    ctx: egui::Context,
    font_name: Option<String>,
    start: std::time::Instant,
}
impl BarRenderer {
    fn new(hwnd: HWND) -> Result<Self, String> {
        let _previous = CurrentContext::capture();
        let mut this = Self {
            hwnd,
            dc: std::ptr::null_mut(),
            rc: std::ptr::null_mut(),
            module: std::ptr::null_mut(),
            gl: None,
            renderer: None,
            ctx: egui::Context::default(),
            font_name: None,
            start: std::time::Instant::now(),
        };
        unsafe {
            this.dc = GetDC(hwnd);
            if this.dc.is_null() {
                return Err(win_error("GetDC(bar)"));
            }
            let pfd = PIXELFORMATDESCRIPTOR {
                nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
                nVersion: 1,
                dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
                iPixelType: PFD_TYPE_RGBA,
                cColorBits: 32,
                cAlphaBits: 8,
                ..Default::default()
            };
            let format = ChoosePixelFormat(this.dc, &pfd);
            if format == 0 || SetPixelFormat(this.dc, format, &pfd) == 0 {
                return Err(win_error("SetPixelFormat(bar)"));
            }
            this.rc = wglCreateContext(this.dc);
            if this.rc.is_null() || wglMakeCurrent(this.dc, this.rc) == 0 {
                return Err(win_error("wglCreateContext(bar)"));
            }
            let Some(proc) = wglGetProcAddress(c"wglCreateContextAttribsARB".as_ptr().cast())
            else {
                return Err("OpenGL 3.3 context creation is unavailable for the bar".into());
            };
            let create: unsafe extern "system" fn(HDC, HGLRC, *const i32) -> HGLRC =
                std::mem::transmute(proc);
            let modern = create(
                this.dc,
                std::ptr::null_mut(),
                [0x2091, 3, 0x2092, 3, 0x9126, 1, 0].as_ptr(),
            );
            if modern.is_null() {
                return Err(win_error("OpenGL 3.3 bar context"));
            }
            wglMakeCurrent(std::ptr::null_mut(), std::ptr::null_mut());
            wglDeleteContext(this.rc);
            this.rc = modern;
            if wglMakeCurrent(this.dc, this.rc) == 0 {
                return Err(win_error("wglMakeCurrent(bar)"));
            }
            if let Some(proc) = wglGetProcAddress(c"wglSwapIntervalEXT".as_ptr().cast()) {
                let swap: unsafe extern "system" fn(i32) -> i32 = std::mem::transmute(proc);
                swap(0);
            }
            let module_name: Vec<u16> = "opengl32.dll\0".encode_utf16().collect();
            this.module = LoadLibraryW(module_name.as_ptr());
            if this.module.is_null() {
                return Err(win_error("LoadLibraryW(bar opengl32)"));
            }
            let gl = Arc::new(glow::Context::from_loader_function(|name| {
                let name = CString::new(name).unwrap();
                let pointer = wglGetProcAddress(name.as_ptr().cast())
                    .map_or(std::ptr::null(), |p| p as *const std::ffi::c_void);
                if pointer.is_null() || [1, 2, 3, usize::MAX].contains(&(pointer as usize)) {
                    GetProcAddress(this.module, name.as_ptr().cast())
                        .map_or(std::ptr::null(), |p| p as *const std::ffi::c_void)
                } else {
                    pointer
                }
            }));
            this.renderer = Some(Renderer::new(gl.clone())?);
            this.gl = Some(gl);
        }
        Ok(this)
    }

    fn paint(
        &mut self,
        size: [u32; 2],
        snapshot: &BarSnapshot,
        present: bool,
    ) -> Result<(), String> {
        self.render_frame(
            size,
            &snapshot.font_name,
            Vec::new(),
            false,
            present,
            |ui| paint_bar(ui, size, snapshot),
        )
        .map(|_| ())
    }

    fn render(
        &mut self,
        size: [u32; 2],
        font_name: &str,
        events: Vec<egui::Event>,
        focused: bool,
        run: impl FnMut(&mut egui::Ui),
    ) -> Result<bool, String> {
        self.render_frame(size, font_name, events, focused, true, run)
    }

    fn render_frame(
        &mut self,
        size: [u32; 2],
        font_name: &str,
        events: Vec<egui::Event>,
        focused: bool,
        present: bool,
        run: impl FnMut(&mut egui::Ui),
    ) -> Result<bool, String> {
        if size.contains(&0) {
            return Ok(false);
        }
        let _previous = CurrentContext::capture();
        if unsafe { wglMakeCurrent(self.dc, self.rc) } == 0 {
            return Err(win_error("wglMakeCurrent(bar paint)"));
        }
        if self.font_name.as_deref() != Some(font_name) {
            self.ctx.set_fonts(bar_fonts(font_name));
            self.font_name = Some(font_name.to_owned());
        }
        // Existing bar height, layout constants and click hit tests are physical pixels.
        // A one-to-one coordinate system preserves them at every monitor DPI.
        let mut input = egui::RawInput {
            time: Some(self.start.elapsed().as_secs_f64()),
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0] as f32, size[1] as f32),
            )),
            focused,
            events,
            ..Default::default()
        };
        let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
        viewport.native_pixels_per_point = Some(1.0);
        viewport.focused = Some(focused);
        let mut output = self.ctx.run_ui(input, run);
        let again = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .is_some_and(|viewport| viewport.repaint_delay.is_zero());
        let gl = self.gl.as_ref().unwrap();
        unsafe {
            gl.disable(glow::SCISSOR_TEST);
            gl.clear_color(
                0x3c as f32 / 255.0,
                0x38 as f32 / 255.0,
                0x36 as f32 / 255.0,
                1.0,
            );
            gl.clear(glow::COLOR_BUFFER_BIT);
        }
        self.renderer
            .as_mut()
            .unwrap()
            .render(&self.ctx, size, &mut output);
        if present && unsafe { SwapBuffers(self.dc) } == 0 {
            return Err(win_error("SwapBuffers(bar)"));
        }
        Ok(again)
    }
}
impl Drop for BarRenderer {
    fn drop(&mut self) {
        let mut previous = CurrentContext::capture();
        // Never restore a context which we are about to delete.
        if previous.rc == self.rc {
            previous.dc = std::ptr::null_mut();
            previous.rc = std::ptr::null_mut();
        }
        unsafe {
            if !self.rc.is_null() {
                if wglMakeCurrent(self.dc, self.rc) != 0 {
                    if let Some(renderer) = &mut self.renderer {
                        renderer.destroy();
                    }
                }
                wglMakeCurrent(std::ptr::null_mut(), std::ptr::null_mut());
                wglDeleteContext(self.rc);
            }
            if !self.dc.is_null() {
                ReleaseDC(self.hwnd, self.dc);
            }
            if !self.module.is_null() {
                FreeLibrary(self.module);
            }
        }
    }
}

pub(crate) fn bar_fonts(family: &str) -> egui::FontDefinitions {
    let mut definitions = egui::FontDefinitions::default();
    let mut faces = Vec::new();
    if let Some(bytes) = windows_font(family) {
        faces.push(("configured".to_owned(), bytes));
    }
    let mut folders = Vec::new();
    if let Some(root) = std::env::var_os("LOCALAPPDATA") {
        folders.push(std::path::PathBuf::from(root).join("Microsoft/Windows/Fonts"));
    }
    if let Some(root) = std::env::var_os("WINDIR") {
        folders.push(std::path::PathBuf::from(root).join("Fonts"));
    }
    for folder in folders {
        for name in [
            "JetBrainsMonoNLNerdFontPropo-Regular.ttf",
            "JetBrainsMonoNerdFontPropo-Regular.ttf",
            "JetBrainsMonoNLNerdFontMono-Regular.ttf",
        ] {
            if let Ok(bytes) = std::fs::read(folder.join(name)) {
                faces.push((name.to_owned(), bytes));
                break;
            }
        }
    }
    let names: Vec<_> = faces.iter().map(|(name, _)| name.clone()).collect();
    for (name, bytes) in faces {
        definitions
            .font_data
            .insert(name, Arc::new(egui::FontData::from_owned(bytes)));
    }
    let fallback = definitions
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    definitions.families.insert(
        egui::FontFamily::Name("bar".into()),
        names.into_iter().chain(fallback).collect(),
    );
    definitions
}

/// Ask GDI for the configured installed face, without adding another font library.
fn windows_font(family: &str) -> Option<Vec<u8>> {
    if family.is_empty() {
        return None;
    }
    let family: Vec<u16> = family.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let dc = CreateCompatibleDC(std::ptr::null_mut());
        if dc.is_null() {
            return None;
        }
        let font = CreateFontW(
            -16,
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
            DEFAULT_PITCH as u32,
            family.as_ptr(),
        );
        if font.is_null() {
            DeleteDC(dc);
            return None;
        }
        let old = SelectObject(dc, font);
        let count = GetFontData(dc, 0, 0, std::ptr::null_mut(), 0);
        let bytes = if count != GDI_ERROR as u32 && count > 0 && count <= 32 * 1024 * 1024 {
            let mut bytes = vec![0; count as usize];
            (GetFontData(dc, 0, 0, bytes.as_mut_ptr().cast(), count) == count).then_some(bytes)
        } else {
            None
        };
        if !old.is_null() {
            SelectObject(dc, old);
        }
        DeleteObject(font);
        DeleteDC(dc);
        bytes
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use windows_sys::Win32::{System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*};

    struct TestWindow(HWND);
    impl TestWindow {
        fn new() -> Self {
            unsafe {
                let class: Vec<u16> = "SWM.EguiBarTest\0".encode_utf16().collect();
                let instance = GetModuleHandleW(std::ptr::null());
                let wc = WNDCLASSW {
                    style: CS_OWNDC,
                    lpfnWndProc: Some(DefWindowProcW),
                    hInstance: instance,
                    lpszClassName: class.as_ptr(),
                    ..Default::default()
                };
                if RegisterClassW(&wc) == 0 {
                    assert_eq!(
                        windows_sys::Win32::Foundation::GetLastError(),
                        windows_sys::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS
                    );
                }
                let hwnd = CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    class.as_ptr(),
                    class.as_ptr(),
                    WS_POPUP,
                    0,
                    0,
                    1600,
                    40,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    instance,
                    std::ptr::null(),
                );
                assert!(!hwnd.is_null());
                Self(hwnd)
            }
        }
    }
    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe {
                DestroyWindow(self.0);
            }
        }
    }

    pub(crate) fn snapshot() -> BarSnapshot {
        BarSnapshot {
            monitor_index: 0,
            selected_monitor: true,
            tags: vec![
                (0, "1".into(), true, true, false),
                (1, "\u{f120}".into(), false, true, true),
                (2, "3".into(), false, false, false),
            ],
            workspace_info: "3 windows | Tile".into(),
            focused_window: "PowerShell — SimpleWindowManager".into(),
            segments: {
                use crate::bar::{RenderedSegment, Tone};
                let segment =
                    |header: Option<(&str, bool)>, value: &str, is_icon, tone| RenderedSegment {
                        separator: Some((" | ".into(), false)),
                        header: header.map(|(text, is_icon)| (text.into(), is_icon)),
                        value: value.into(),
                        value_is_icon: is_icon,
                        tone,
                    };
                vec![
                    segment(None, "12:34", false, Tone::Normal),
                    segment(None, "2026-10-15", false, Tone::Normal),
                    segment(Some(("CPU:", false)), " 82%", false, Tone::Warning),
                    segment(Some(("RAM:", false)), "100%", false, Tone::Critical),
                    segment(Some(("\u{f028} ", true)), "  0%", false, Tone::Off),
                    segment(None, crate::status::INTERNET_OFFLINE, true, Tone::Critical),
                ]
            },
            focus_style: Default::default(),
            mailboxes: Vec::new(),
            background_color: 0,
            focus_background_color: 0,
            text_color: 0,
            info_color: 0,
            disabled_color: 0,
            focus_text_color: 0,
            extra_focus_background_color: 0,
            font_name: "JetBrainsMonoNL NFP Regular".into(),
            normal_font_size: 12,
            icon_font_size: 13,
        }
    }

    fn pixels(renderer: &BarRenderer, size: [u32; 2]) -> Vec<u8> {
        let _previous = CurrentContext::capture();
        unsafe {
            assert_ne!(wglMakeCurrent(renderer.dc, renderer.rc), 0);
            let gl = renderer.gl.as_ref().unwrap();
            let mut pixels = vec![0; (size[0] * size[1] * 4) as usize];
            gl.read_buffer(glow::BACK);
            gl.read_pixels(
                0,
                0,
                size[0] as i32,
                size[1] as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut pixels)),
            );
            assert_eq!(gl.get_error(), glow::NO_ERROR);
            pixels
        }
    }

    #[test]
    fn mailboxes_paint_left_of_the_status_and_take_clicks() {
        use crate::mailbox::{Animation, Icon, MailboxSender, MailboxState, StateStyle};
        fn producer(_: MailboxSender) {}
        fn square(painter: &egui::Painter, area: egui::Rect, color: egui::Color32) {
            painter.rect_filled(area, 0, color);
        }
        let mut configuration = crate::Configuration::default();
        configuration
            .bar_mailbox("Agents", producer)
            .header("Agents:")
            .state(
                "running",
                StateStyle::icon(Icon::drawn(square, egui::Color32::RED).animation(
                    Animation::Scale {
                        min: 0.5,
                        max: 1.0,
                        period: 1.0,
                    },
                )),
            )
            .state(
                "waiting",
                StateStyle::icon(Icon::glyph("●", egui::Color32::RED)),
            )
            .state(
                "done",
                StateStyle::icon(Icon::glyph("✓", egui::Color32::GREEN)),
            );
        let mailbox = configuration.bar_mailboxes.remove(0);
        let window = TestWindow::new();
        let mut renderer = BarRenderer::new(window.0).unwrap();
        let mut snapshot = snapshot();
        let size = [1600, 29];
        let counts = |running, waiting, done| {
            std::sync::Arc::new(MailboxState {
                counts: vec![
                    ("running".into(), running),
                    ("waiting".into(), waiting),
                    ("done".into(), done),
                ],
                entries: Vec::new(),
            })
        };
        let clickable = |monitor| {
            (0..size[0] as i32)
                .filter(|&x| crate::bar::mailbox_at_x(monitor, x).is_some())
                .collect::<Vec<_>>()
        };
        snapshot.mailboxes = vec![(mailbox.clone(), counts(2, 1, 3))];
        renderer.paint(size, &snapshot, false).unwrap();
        let region = clickable(snapshot.monitor_index);
        assert!(!region.is_empty(), "no clickable mailbox");
        assert!(
            *region.last().unwrap() < 1500,
            "the mailbox overlaps the status segments: {:?}",
            (region.first(), region.last())
        );
        // A missing glyph would render as a box rather than the state symbol.
        let font = egui::FontId::new(12.0, egui::FontFamily::Name("bar".into()));
        for glyph in ['●', '✓'] {
            assert!(
                renderer
                    .ctx
                    .fonts_mut(|fonts| fonts.has_glyph(&font, glyph)),
                "bar fonts lack {glyph:?}"
            );
        }
        snapshot.mailboxes = vec![(mailbox, counts(0, 0, 0))];
        renderer.paint(size, &snapshot, false).unwrap();
        assert!(
            clickable(snapshot.monitor_index).is_empty(),
            "an empty mailbox left a region"
        );
    }

    #[test]
    fn native_bar_rendering_preserves_contexts_and_releases_resources() {
        let first_window = TestWindow::new();
        let second_window = TestWindow::new();
        let mut first = BarRenderer::new(first_window.0).unwrap();
        assert!(unsafe { wglGetCurrentContext() }.is_null());
        unsafe {
            assert_ne!(wglMakeCurrent(first.dc, first.rc), 0);
        }
        let mut second = BarRenderer::new(second_window.0).unwrap();
        assert_eq!(unsafe { wglGetCurrentContext() }, first.rc);
        let foreground = unsafe { GetForegroundWindow() };
        let mut snapshot = snapshot();
        let size = [1600, 29];
        second.paint(size, &snapshot, false).unwrap();
        assert_eq!(unsafe { wglGetCurrentContext() }, first.rc);
        let rendered = pixels(&second, size);
        assert!(
            rendered
                .chunks_exact(4)
                .filter(|p| p[0] > 150 && p[1] > 150)
                .count()
                > 100,
            "text and icons must render"
        );
        assert!(
            rendered
                .chunks_exact(4)
                .filter(|p| p[..3] == [0x83, 0xa5, 0x98])
                .count()
                > 200,
            "workspace region must render"
        );
        assert!(
            rendered
                .chunks_exact(4)
                .enumerate()
                .filter(|(index, p)| {
                    let x = *index % size[0] as usize;
                    x > 600 && x < 1000 && p[1] > 110 && p[1] > p[0] + 10 && p[2] > p[0] + 5
                })
                .count()
                > 10,
            "focus outline must render"
        );
        if let Some(path) = std::env::var_os("SWM_BAR_PREVIEW_BMP") {
            let write = |path: &std::path::Path, rendered: &[u8]| {
                // Positive BMP height uses the same bottom-up row order as OpenGL.
                let mut bmp = Vec::new();
                bmp.extend_from_slice(b"BM");
                bmp.extend_from_slice(&(54u32 + rendered.len() as u32).to_le_bytes());
                bmp.extend_from_slice(&[0; 4]);
                bmp.extend_from_slice(&54u32.to_le_bytes());
                bmp.extend_from_slice(&40u32.to_le_bytes());
                bmp.extend_from_slice(&(size[0] as i32).to_le_bytes());
                bmp.extend_from_slice(&(size[1] as i32).to_le_bytes());
                bmp.extend_from_slice(&1u16.to_le_bytes());
                bmp.extend_from_slice(&32u16.to_le_bytes());
                bmp.extend_from_slice(&[0; 24]);
                for pixel in rendered.chunks_exact(4) {
                    bmp.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
                }
                std::fs::write(path, bmp).unwrap();
            };
            let path = std::path::PathBuf::from(path);
            write(&path, &rendered);
            // The same bar on a monitor that isn't focused, beside it.
            let mut unfocused = snapshot.clone();
            unfocused.selected_monitor = false;
            second.paint(size, &unfocused, false).unwrap();
            write(
                &path.with_extension("unfocused.bmp"),
                &pixels(&second, size),
            );
        }
        snapshot.selected_monitor = false;
        snapshot.focused_window = "Long title ".repeat(200);
        snapshot.font_name = "Consolas".into();
        second.paint([800, 29], &snapshot, true).unwrap();
        assert_eq!(unsafe { GetForegroundWindow() }, foreground);
        drop(second);
        assert_eq!(unsafe { wglGetCurrentContext() }, first.rc);
        first.paint([1600, 40], &snapshot, false).unwrap();
        drop(first);
        assert!(unsafe { wglGetCurrentContext() }.is_null());
        // A fresh window/context exercises shutdown followed by recreation.
        let third_window = TestWindow::new();
        let mut third = BarRenderer::new(third_window.0).unwrap();
        third.paint([1600, 40], &snapshot, true).unwrap();
    }

    #[test]
    fn bar_window_renderer_registry_detaches_cleanly() {
        let window = TestWindow::new();
        attach(window.0).unwrap();
        paint(window.0, [1600, 40], &snapshot()).unwrap();
        RENDERERS.with(|renderers| assert_eq!(renderers.borrow().len(), 1));
        detach(window.0);
        RENDERERS.with(|renderers| assert!(renderers.borrow().is_empty()));
        assert!(unsafe { wglGetCurrentContext() }.is_null());
    }
}
