#[derive(Debug)]
pub struct BorderWindow {
    pub hwnd: isize,
}

pub fn border_rect(target: crate::Rect) -> crate::Rect {
    crate::Rect {
        left: target.left - 9,
        top: target.top - 9,
        right: target.right + 9,
        bottom: target.bottom + 9,
    }
}

mod dcomp {
    use std::cell::RefCell;
    use std::time::Instant;
    use windows::Win32::Foundation::{HMODULE, HWND, POINT};
    use windows::Win32::Graphics::Direct2D::Common::*;
    use windows::Win32::Graphics::Direct2D::*;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::DirectComposition::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::*;
    use windows::core::Interface;

    thread_local! {
        static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    }

    fn logged<T>(
        context: &'static str,
        result: windows::core::Result<T>,
    ) -> crate::platform_error::PlatformResult<T> {
        result
            .map_err(|error| crate::platform_error::PlatformError::hresult(context, error.code().0))
    }

    // Slice a 33px reference border into 16px corners and 1px edge strips.
    // The curve ends at 14px, so all stretching happens on straight sections.
    const SLICES: [(u32, u32, u32, u32); 8] = [
        (0, 0, 16, 16),
        (17, 0, 16, 16),
        (0, 17, 16, 16),
        (17, 17, 16, 16),
        (16, 0, 1, 16),
        (16, 17, 1, 16),
        (0, 16, 16, 1),
        (17, 16, 16, 1),
    ];

    // x, y, horizontal scale, vertical scale; tiny windows shrink corners
    // rather than overlapping them or producing negative edge lengths.
    fn placements(width: u32, height: u32) -> [(f32, f32, f32, f32); 8] {
        let w = width as f32;
        let h = height as f32;
        let cw = 16.0_f32.min(w / 2.0);
        let ch = 16.0_f32.min(h / 2.0);
        let sx = cw / 16.0;
        let sy = ch / 16.0;
        [
            (0.0, 0.0, sx, sy),
            (w - cw, 0.0, sx, sy),
            (0.0, h - ch, sx, sy),
            (w - cw, h - ch, sx, sy),
            (cw, 0.0, w - 2.0 * cw, sy),
            (cw, h - ch, w - 2.0 * cw, sy),
            (0.0, ch, sx, h - 2.0 * ch),
            (w - cw, ch, sx, h - 2.0 * ch),
        ]
    }

    struct Piece {
        visual: IDCompositionVisual,
        scale: IDCompositionScaleTransform,
        surfaces: [IDCompositionSurface; 2],
    }

    struct State {
        _d3d: ID3D11Device,
        _d2_device: ID2D1Device,
        device: IDCompositionDesktopDevice,
        _target: IDCompositionTarget,
        _root: IDCompositionVisual,
        pieces: Vec<Piece>,
        previous: Option<(u32, u32, bool)>,
    }

    fn color_ref(value: u32) -> D2D1_COLOR_F {
        D2D1_COLOR_F {
            r: (value & 0xff) as f32 / 255.0,
            g: ((value >> 8) & 0xff) as f32 / 255.0,
            b: ((value >> 16) & 0xff) as f32 / 255.0,
            a: 1.0,
        }
    }

    unsafe fn create_piece_surface(
        device: &IDCompositionDesktopDevice,
        slice: (u32, u32, u32, u32),
        color: u32,
    ) -> crate::platform_error::PlatformResult<IDCompositionSurface> {
        let (x, y, width, height) = slice;
        let surface = logged("CreateSurface(border piece)", unsafe {
            device.CreateSurface(
                width,
                height,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_ALPHA_MODE_PREMULTIPLIED,
            )
        })?;
        let mut offset = POINT::default();
        let dc: ID2D1DeviceContext = logged("BeginDraw(border piece)", unsafe {
            surface.BeginDraw(None, &mut offset)
        })?;
        // DirectComposition supplies an already-active D2D context. Always
        // pair its BeginDraw with surface.EndDraw, including brush failures.
        let drawn = (|| {
            let brush = logged("CreateSolidColorBrush(border piece)", unsafe {
                dc.CreateSolidColorBrush(&color_ref(color), None)
            })?;
            let ox = offset.x as f32;
            let oy = offset.y as f32;
            let clip = D2D_RECT_F {
                left: ox,
                top: oy,
                right: ox + width as f32,
                bottom: oy + height as f32,
            };
            let rounded = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: ox + 7.0 - x as f32,
                    top: oy + 7.0 - y as f32,
                    right: ox + 26.0 - x as f32,
                    bottom: oy + 26.0 - y as f32,
                },
                radiusX: 7.0,
                radiusY: 7.0,
            };
            unsafe {
                dc.SetDpi(96.0, 96.0);
                dc.PushAxisAlignedClip(&clip, D2D1_ANTIALIAS_MODE_ALIASED);
                dc.Clear(Some(&D2D1_COLOR_F::default()));
                dc.DrawRoundedRectangle(&rounded, &brush, 5.0, None::<&ID2D1StrokeStyle>);
                dc.PopAxisAlignedClip();
            }
            Ok(())
        })();
        drop(dc);
        let ended = logged("EndDraw(border piece)", unsafe { surface.EndDraw() });
        drawn?;
        ended?;
        Ok(surface)
    }

    impl State {
        unsafe fn new(
            hwnd: isize,
            normal: u32,
            lost: u32,
        ) -> crate::platform_error::PlatformResult<Self> {
            let mut d3d = None;
            logged("D3D11CreateDevice(border)", unsafe {
                D3D11CreateDevice(
                    None::<&IDXGIAdapter>,
                    D3D_DRIVER_TYPE_HARDWARE,
                    HMODULE::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    None,
                    D3D11_SDK_VERSION,
                    Some(&mut d3d),
                    None,
                    None,
                )
            })?;
            let d3d = d3d.unwrap();
            let dxgi: IDXGIDevice = logged("QueryInterface(border DXGI device)", d3d.cast())?;
            let factory: ID2D1Factory1 = logged("D2D1CreateFactory(border)", unsafe {
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
            })?;
            let d2_device = logged("CreateDevice(border D2D)", unsafe {
                factory.CreateDevice(&dxgi)
            })?;
            let device: IDCompositionDesktopDevice =
                logged("DCompositionCreateDevice2(border)", unsafe {
                    DCompositionCreateDevice2(&d2_device)
                })?;
            let target = logged("CreateTargetForHwnd(border)", unsafe {
                device.CreateTargetForHwnd(HWND(hwnd as _), true)
            })?;
            let root = logged("CreateVisual(border root)", unsafe {
                device.CreateVisual()
            })?;
            let mut pieces = Vec::with_capacity(8);
            for slice in SLICES {
                let surfaces = unsafe {
                    [
                        create_piece_surface(&device, slice, normal)?,
                        create_piece_surface(&device, slice, lost)?,
                    ]
                };
                let visual = logged("CreateVisual(border piece)", unsafe {
                    device.CreateVisual()
                })?;
                let scale = logged("CreateScaleTransform(border piece)", unsafe {
                    device.CreateScaleTransform()
                })?;
                unsafe {
                    logged("SetTransform(border piece)", visual.SetTransform(&scale))?;
                    logged("SetContent(border piece)", visual.SetContent(&surfaces[0]))?;
                    logged(
                        "AddVisual(border piece)",
                        root.AddVisual(&visual, true, None::<&IDCompositionVisual>),
                    )?;
                }
                pieces.push(Piece {
                    visual: visual.into(),
                    scale,
                    surfaces,
                });
            }
            logged("SetRoot(border)", unsafe { target.SetRoot(&root) })?;
            // The first draw positions all pieces and commits their initial content.
            Ok(Self {
                _d3d: d3d,
                _d2_device: d2_device,
                device,
                _target: target,
                _root: root.into(),
                pieces,
                previous: None,
            })
        }

        unsafe fn draw(
            &mut self,
            width: u32,
            height: u32,
            lost_focus: bool,
        ) -> crate::platform_error::PlatformResult<()> {
            let current = (width, height, lost_focus);
            if width == 0 || height == 0 || self.previous == Some(current) {
                return Ok(());
            }
            let started = crate::error_log::debug_output_enabled().then(Instant::now);
            let resized = self
                .previous
                .is_none_or(|old| old.0 != width || old.1 != height);
            let recolored = self.previous.is_none_or(|old| old.2 != lost_focus);
            for (piece, (x, y, sx, sy)) in self.pieces.iter().zip(placements(width, height)) {
                unsafe {
                    if resized {
                        logged("SetOffsetX(border piece)", piece.visual.SetOffsetX2(x))?;
                        logged("SetOffsetY(border piece)", piece.visual.SetOffsetY2(y))?;
                        logged("SetScaleX(border piece)", piece.scale.SetScaleX2(sx))?;
                        logged("SetScaleY(border piece)", piece.scale.SetScaleY2(sy))?;
                    }
                    if recolored {
                        logged(
                            "SetContent(border color)",
                            piece
                                .visual
                                .SetContent(&piece.surfaces[usize::from(lost_focus)]),
                        )?;
                    }
                }
            }
            let commit_started = crate::error_log::debug_output_enabled().then(Instant::now);
            let result = unsafe { self.device.Commit() };
            let commit_us = commit_started.map(|start| start.elapsed().as_micros() as u64);
            if let Some(started) = started {
                let elapsed_us = started.elapsed().as_micros() as u64;
                crate::debug_out!(
                    operation = "border_composition_update",
                    elapsed_us,
                    commit_us,
                    width,
                    height,
                    lost_focus,
                    resized,
                    recolored
                );
            }
            logged("Commit(border pieces)", result)?;
            self.previous = Some(current);
            Ok(())
        }
    }

    pub unsafe fn initialize(
        hwnd: isize,
        normal: u32,
        lost: u32,
    ) -> crate::platform_error::PlatformResult<()> {
        let state = unsafe { State::new(hwnd, normal, lost)? };
        STATE.with(|slot| *slot.borrow_mut() = Some(state));
        Ok(())
    }

    pub unsafe fn draw(width: u32, height: u32, lost_focus: bool) {
        STATE.with(|slot| {
            if let Some(state) = slot.borrow_mut().as_mut() {
                if let Err(error) = unsafe { state.draw(width, height, lost_focus) } {
                    error.report();
                }
            }
        });
    }

    pub fn clean() {
        STATE.with(|slot| *slot.borrow_mut() = None);
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn pieces_tile_the_perimeter_without_scaling_normal_corners() {
            for (w, h) in [(1919, 706), (1919, 2110), (32, 32), (18, 21)] {
                let p = placements(w, h);
                let bounds: Vec<_> = p
                    .iter()
                    .zip(SLICES)
                    .map(|(&(x, y, sx, sy), (_, _, sw, sh))| {
                        assert!(sx >= 0.0 && sy >= 0.0);
                        (x, y, x + sx * sw as f32, y + sy * sh as f32)
                    })
                    .collect();
                assert_eq!(bounds[0].2, bounds[4].0);
                assert_eq!(bounds[4].2, bounds[1].0);
                assert_eq!(bounds[0].3, bounds[6].1);
                assert_eq!(bounds[6].3, bounds[2].1);
                assert_eq!(bounds[3].2, w as f32);
                assert_eq!(bounds[3].3, h as f32);
                if w >= 32 && h >= 32 {
                    assert_eq!((p[0].2, p[0].3), (1.0, 1.0));
                }
            }
        }
    }
}
unsafe extern "system" fn border_message_loop(
    hwnd: windows_sys::Win32::Foundation::HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use windows_sys::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let span = matches!(message, WM_SIZE | WM_WINDOWPOSCHANGING | WM_PAINT).then(|| {
        tracing::info_span!(
            "window_message",
            component = "border",
            message_name = crate::platform::window_message_name(message),
            message,
            hwnd = hwnd as usize,
            wparam,
            lparam,
        )
    });
    let _entered = span.as_ref().map(tracing::Span::enter);
    match message {
        WM_MOUSEACTIVATE => MA_NOACTIVATE as isize,
        WM_SIZE => {
            let width = lparam as u32 & 0xffff;
            let height = (lparam as u32 >> 16) & 0xffff;
            let lost = crate::platform::border_is_lost_focus();
            unsafe { dcomp::draw(width, height, lost) };
            0
        }
        WM_PAINT => {
            let mut paint: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            let dc = unsafe { BeginPaint(hwnd, &mut paint) };
            if dc.is_null() {
                crate::platform_error::PlatformError::null_output("BeginPaint(border)").report();
                return 0;
            }
            let mut rect = windows_sys::Win32::Foundation::RECT::default();
            if unsafe { GetClientRect(hwnd, &mut rect) } == 0 {
                crate::platform_error::PlatformError::last_win32("GetClientRect(border paint)")
                    .report();
                unsafe { EndPaint(hwnd, &paint) };
                return 0;
            }
            let lost = crate::platform::border_is_lost_focus();
            unsafe {
                dcomp::draw(
                    (rect.right - rect.left) as u32,
                    (rect.bottom - rect.top) as u32,
                    lost,
                )
            };
            // EndPaint is documented to always return nonzero.
            unsafe { EndPaint(hwnd, &paint) };
            0
        }
        WM_ERASEBKGND => 1,
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

pub fn create_border_window(
    normal: u32,
    lost: u32,
) -> crate::platform_error::PlatformResult<BorderWindow> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::{ERROR_CLASS_ALREADY_EXISTS, GetLastError};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let class_name: Vec<u16> = "nwm_dcomp_border_class\0".encode_utf16().collect();
    let title: Vec<u16> = "nwm_dcomp_border\0".encode_utf16().collect();
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    if instance.is_null() {
        return Err(crate::platform_error::PlatformError::last_win32(
            "GetModuleHandleW(border)",
        ));
    }
    let mut class: WNDCLASSEXW = unsafe { zeroed() };
    class.cbSize = size_of::<WNDCLASSEXW>() as u32;
    class.style = CS_HREDRAW | CS_VREDRAW;
    class.lpfnWndProc = Some(border_message_loop);
    class.hInstance = instance;
    class.hCursor = unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) };
    if class.hCursor.is_null() {
        crate::error_log::last_win32("LoadCursorW(border) failed");
    }
    class.lpszClassName = class_name.as_ptr();
    if unsafe { RegisterClassExW(&class) } == 0 {
        let error = crate::platform_error::PlatformError::last_win32("RegisterClassExW(border)");
        if !error.is_win32(ERROR_CLASS_ALREADY_EXISTS) {
            return Err(error);
        }
    }
    let hwnd = unsafe {
        CreateWindowExW(
            // DirectComposition supplies the transparent content. A layered
            // host with constant alpha adds an opaque redirected background.
            WS_EX_NOREDIRECTIONBITMAP | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
            100,
            100,
            100,
            100,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null_mut(),
        )
    };
    if hwnd.is_null() {
        return Err(crate::platform_error::PlatformError::last_win32(
            "CreateWindowExW(border)",
        ));
    }
    unsafe {
        windows_sys::Win32::Foundation::SetLastError(0);
        SetWindowLongW(hwnd, GWL_STYLE, 0);
        let error = GetLastError();
        if error != 0 {
            let error =
                crate::platform_error::PlatformError::win32("SetWindowLongW(border style)", error);
            if DestroyWindow(hwnd) == 0 {
                crate::platform_error::PlatformError::last_win32(
                    "DestroyWindow(border style cleanup)",
                )
                .report();
            }
            return Err(error);
        }
    }
    if let Err(error) = unsafe { dcomp::initialize(hwnd as isize, normal, lost) } {
        if unsafe { DestroyWindow(hwnd) } == 0 {
            crate::error_log::last_win32("DestroyWindow(border initialization cleanup) failed");
        }
        return Err(error);
    }
    Ok(BorderWindow {
        hwnd: hwnd as isize,
    })
}

pub fn update_border(window: &BorderWindow, target: Option<(isize, crate::Rect)>) {
    use windows_sys::Win32::Graphics::Gdi::InvalidateRect;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        let Some((target_hwnd, target)) = target else {
            if SetWindowPos(
                window.hwnd as _,
                HWND_BOTTOM,
                0,
                0,
                0,
                0,
                SWP_HIDEWINDOW | SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE | SWP_NOZORDER,
            ) == 0
            {
                crate::error_log::last_win32("SetWindowPos(hide border) failed");
            }
            return;
        };
        let target = border_rect(target);
        let width = target.right - target.left;
        let height = target.bottom - target.top;
        let mut current = windows_sys::Win32::Foundation::RECT::default();
        if GetWindowRect(window.hwnd as _, &mut current) == 0 {
            crate::platform_error::PlatformError::last_win32("GetWindowRect(border)").report();
            // Position and redraw decisions depend on the current bounds, so
            // using the default rectangle after failure would move incorrectly.
            return;
        }
        let visible = IsWindowVisible(window.hwnd as _) != 0;
        let flags = if current.right - current.left == width
            && current.bottom - current.top == height
            && visible
        {
            SWP_NOREDRAW
        } else {
            SWP_SHOWWINDOW
        };
        // Keep the transparent composition host behind the application: its
        // transparent pixels do not provide cross-process hit-test transparency.
        let target_topmost =
            GetWindowLongW(target_hwnd as _, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0;
        let border_topmost =
            GetWindowLongW(window.hwnd as _, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0;
        if border_topmost != target_topmost {
            if SetWindowPos(
                window.hwnd as _,
                if target_topmost {
                    HWND_TOPMOST
                } else {
                    HWND_NOTOPMOST
                },
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            ) == 0
            {
                crate::error_log::last_win32("SetWindowPos(border topmost band) failed");
                return;
            }
        }
        let z_order_changed = GetWindow(target_hwnd as _, GW_HWNDNEXT) != window.hwnd as _;
        if target.left != current.left
            || target.top != current.top
            || flags == SWP_SHOWWINDOW
            || z_order_changed
        {
            if SetWindowPos(
                window.hwnd as _,
                target_hwnd as _,
                target.left,
                target.top,
                width,
                height,
                flags | SWP_NOACTIVATE | if z_order_changed { 0 } else { SWP_NOZORDER },
            ) == 0
            {
                crate::error_log::last_win32("SetWindowPos(show border) failed");
            }
        }
        // Always request a paint so focus-state changes are observed. The
        // DirectComposition renderer skips the composition update when its size and
        // lost-focus state are unchanged.
        if InvalidateRect(window.hwnd as _, std::ptr::null(), 0) == 0 {
            crate::error_log::last_win32("InvalidateRect(border) failed");
        }
    }
}

pub fn clean() {
    dcomp::clean();
}

#[cfg(test)]
mod tests {
    #[test]
    fn uses_the_c_border_padding() {
        assert_eq!(
            super::border_rect(crate::Rect::from_xywh(100, 200, 300, 400)),
            crate::Rect {
                left: 91,
                top: 191,
                right: 409,
                bottom: 609
            }
        );
    }
}
