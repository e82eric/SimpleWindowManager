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
    use windows::Win32::Foundation::{HMODULE, HWND};
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

    struct State {
        hwnd: isize,
        _d3d: ID3D11Device,
        _dxgi: IDXGIDevice,
        swap_chain: IDXGISwapChain1,
        d2_factory: ID2D1Factory2,
        _d2_device: ID2D1Device1,
        dc: ID2D1DeviceContext,
        dcomp_device: IDCompositionDevice,
        target: IDCompositionTarget,
        visual: IDCompositionVisual,
        surface: Option<IDXGISurface2>,
        bitmap: Option<ID2D1Bitmap1>,
        normal_brush: ID2D1SolidColorBrush,
        lost_focus_brush: ID2D1SolidColorBrush,
        old_width: u32,
        old_height: u32,
        old_lost_focus: bool,
    }

    fn color_ref(value: u32) -> D2D1_COLOR_F {
        D2D1_COLOR_F {
            r: (value & 0xff) as f32 / 255.0,
            g: ((value >> 8) & 0xff) as f32 / 255.0,
            b: ((value >> 16) & 0xff) as f32 / 255.0,
            a: 1.0,
        }
    }

    impl State {
        unsafe fn new(
            hwnd: isize,
            normal: u32,
            lost: u32,
        ) -> crate::platform_error::PlatformResult<Self> {
            let mut d3d = None;
            unsafe {
                logged(
                    "D3D11CreateDevice failed",
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
                    ),
                )?;
            }
            let d3d = d3d.unwrap();
            let dxgi: IDXGIDevice = logged(
                "ID3D11Device::QueryInterface(IDXGIDevice) failed",
                d3d.cast(),
            )?;
            let factory: IDXGIFactory2 = logged("CreateDXGIFactory2 failed", unsafe {
                CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))
            })?;
            let mut description: DXGI_SWAP_CHAIN_DESC1 = unsafe { std::mem::zeroed() };
            description.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
            description.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
            description.SwapEffect = DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL;
            description.BufferCount = 2;
            description.SampleDesc.Count = 1;
            description.AlphaMode = DXGI_ALPHA_MODE_PREMULTIPLIED;
            let mut initial_rect = windows_sys::Win32::Foundation::RECT::default();
            if unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect(
                    hwnd as _,
                    &mut initial_rect,
                )
            } == 0
            {
                return Err(crate::platform_error::PlatformError::last_win32(
                    "GetClientRect(border initialization)",
                ));
            }
            description.Width = (initial_rect.right - initial_rect.left).max(1) as u32;
            description.Height = (initial_rect.bottom - initial_rect.top).max(1) as u32;
            let swap_chain = logged(
                "IDXGIFactory2::CreateSwapChainForComposition failed",
                unsafe {
                    factory.CreateSwapChainForComposition(&dxgi, &description, None::<&IDXGIOutput>)
                },
            )?;
            let d2_factory: ID2D1Factory2 = logged("D2D1CreateFactory failed", unsafe {
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
            })?;
            let d2_device = logged("ID2D1Factory2::CreateDevice failed", unsafe {
                d2_factory.CreateDevice(&dxgi)
            })?;
            let dc = logged("ID2D1Device1::CreateDeviceContext failed", unsafe {
                d2_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)
            })?;
            let normal_color = color_ref(normal);
            let lost_color = color_ref(lost);
            let normal_brush = logged(
                "ID2D1DeviceContext::CreateSolidColorBrush(normal) failed",
                unsafe { dc.CreateSolidColorBrush(&normal_color, None) },
            )?;
            let lost_focus_brush = logged(
                "ID2D1DeviceContext::CreateSolidColorBrush(lost focus) failed",
                unsafe { dc.CreateSolidColorBrush(&lost_color, None) },
            )?;
            let dcomp_device: IDCompositionDevice =
                logged("DCompositionCreateDevice failed", unsafe {
                    DCompositionCreateDevice(&dxgi)
                })?;
            let target = logged("IDCompositionDevice::CreateTargetForHwnd failed", unsafe {
                dcomp_device.CreateTargetForHwnd(HWND(hwnd as _), true)
            })?;
            let visual = logged("IDCompositionDevice::CreateVisual failed", unsafe {
                dcomp_device.CreateVisual()
            })?;
            let mut state = Self {
                hwnd,
                _d3d: d3d,
                _dxgi: dxgi,
                swap_chain,
                d2_factory,
                _d2_device: d2_device,
                dc: dc.into(),
                dcomp_device,
                target,
                visual,
                surface: None,
                bitmap: None,
                normal_brush,
                lost_focus_brush,
                old_width: 0,
                old_height: 0,
                old_lost_focus: false,
            };
            unsafe { state.resize_swap_chain()? };
            Ok(state)
        }

        unsafe fn client_size(&self) -> crate::platform_error::PlatformResult<(u32, u32)> {
            let mut rect = windows_sys::Win32::Foundation::RECT::default();
            if unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect(
                    self.hwnd as _,
                    &mut rect,
                )
            } == 0
            {
                return Err(crate::platform_error::PlatformError::last_win32(
                    "GetClientRect(border)",
                ));
            }
            Ok((
                (rect.right - rect.left).max(0) as u32,
                (rect.bottom - rect.top).max(0) as u32,
            ))
        }

        unsafe fn resize_swap_chain(&mut self) -> crate::platform_error::PlatformResult<()> {
            unsafe { self.dc.SetTarget(None::<&ID2D1Image>) };
            self.bitmap = None;
            self.surface = None;
            let (width, height) = unsafe { self.client_size()? };
            if width == 0 || height == 0 {
                return Ok(());
            }
            logged("IDXGISwapChain1::ResizeBuffers failed", unsafe {
                self.swap_chain.ResizeBuffers(
                    2,
                    width,
                    height,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )
            })?;
            let surface: IDXGISurface2 = logged("IDXGISwapChain1::GetBuffer failed", unsafe {
                self.swap_chain.GetBuffer(0)
            })?;
            let mut properties: D2D1_BITMAP_PROPERTIES1 = unsafe { std::mem::zeroed() };
            properties.pixelFormat.alphaMode = D2D1_ALPHA_MODE_PREMULTIPLIED;
            properties.pixelFormat.format = DXGI_FORMAT_B8G8R8A8_UNORM;
            properties.bitmapOptions = D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW;
            let bitmap = logged(
                "ID2D1DeviceContext::CreateBitmapFromDxgiSurface failed",
                unsafe {
                    self.dc
                        .CreateBitmapFromDxgiSurface(&surface, Some(&properties))
                },
            )?;
            self.surface = Some(surface);
            self.bitmap = Some(bitmap);
            Ok(())
        }

        unsafe fn draw(
            &mut self,
            width: u32,
            height: u32,
            lost_focus: bool,
        ) -> crate::platform_error::PlatformResult<()> {
            if width == 0
                || height == 0
                || (self.old_width == width
                    && self.old_height == height
                    && self.old_lost_focus == lost_focus)
            {
                return Ok(());
            }
            unsafe { self.resize_swap_chain()? };
            let Some(bitmap) = &self.bitmap else {
                return Ok(());
            };
            unsafe {
                self.dc.SetTarget(bitmap);
                self.dc.BeginDraw();
            }
            let rounded = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: 7.0,
                    top: 7.0,
                    right: width as f32 - 7.0,
                    bottom: height as f32 - 7.0,
                },
                radiusX: 7.0,
                radiusY: 7.0,
            };
            let stroke_properties = D2D1_STROKE_STYLE_PROPERTIES1 {
                startCap: D2D1_CAP_STYLE_FLAT,
                endCap: D2D1_CAP_STYLE_FLAT,
                dashCap: D2D1_CAP_STYLE_FLAT,
                lineJoin: D2D1_LINE_JOIN_ROUND,
                miterLimit: 20.0,
                dashStyle: D2D1_DASH_STYLE_SOLID,
                dashOffset: 0.0,
                transformType: D2D1_STROKE_TRANSFORM_TYPE_NORMAL,
            };
            let stroke = logged("ID2D1Factory2::CreateStrokeStyle failed", unsafe {
                self.d2_factory.CreateStrokeStyle(&stroke_properties, None)
            })?;
            let brush = if lost_focus {
                &self.lost_focus_brush
            } else {
                &self.normal_brush
            };
            unsafe {
                self.dc.DrawRoundedRectangle(&rounded, brush, 5.0, &stroke);
                logged(
                    "ID2D1DeviceContext::EndDraw failed",
                    self.dc.EndDraw(None, None),
                )?;
                logged(
                    "IDXGISwapChain1::Present failed",
                    self.swap_chain.Present(1, DXGI_PRESENT(0)).ok(),
                )?;
                logged(
                    "IDCompositionVisual::SetContent failed",
                    self.visual.SetContent(&self.swap_chain),
                )?;
                logged(
                    "IDCompositionTarget::SetRoot failed",
                    self.target.SetRoot(&self.visual),
                )?;
                logged(
                    "IDCompositionDevice::Commit failed",
                    self.dcomp_device.Commit(),
                )?;
            }
            self.old_width = width;
            self.old_height = height;
            self.old_lost_focus = lost_focus;
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
        WM_SIZE => {
            let width = lparam as u32 & 0xffff;
            let height = (lparam as u32 >> 16) & 0xffff;
            let lost = crate::platform::border_is_lost_focus();
            unsafe { dcomp::draw(width, height, lost) };
            0
        }
        WM_WINDOWPOSCHANGING => {
            let position = lparam as *mut WINDOWPOS;
            if !position.is_null() {
                unsafe { (*position).hwndInsertAfter = HWND_BOTTOM };
            }
            1
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
            WS_EX_NOREDIRECTIONBITMAP | WS_EX_NOACTIVATE,
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

pub fn update_border(window: &BorderWindow, target: Option<crate::Rect>) {
    use windows_sys::Win32::Graphics::Gdi::InvalidateRect;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        let Some(target) = target else {
            if SetWindowPos(
                window.hwnd as _,
                HWND_BOTTOM,
                0,
                0,
                0,
                0,
                SWP_HIDEWINDOW | SWP_NOSIZE,
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
        if target.left != current.left || target.top != current.top || flags == SWP_SHOWWINDOW {
            if SetWindowPos(
                window.hwnd as _,
                HWND_BOTTOM,
                target.left,
                target.top,
                width,
                height,
                flags,
            ) == 0
            {
                crate::error_log::last_win32("SetWindowPos(show border) failed");
            }
        }
        // Always request a paint so focus-state changes are observed. The
        // DirectComposition renderer skips the GPU work when its size and
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
