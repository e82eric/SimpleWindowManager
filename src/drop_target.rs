#[derive(Debug)]
pub struct DropTargetWindow {
    pub hwnd: isize,
}

static SHOULD_PAINT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

unsafe extern "system" fn window_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use std::mem::zeroed;
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let span = (message == WM_PAINT).then(|| {
        tracing::info_span!(
            "window_message",
            component = "drop_target",
            message_name = crate::platform::window_message_name(message),
            message,
            hwnd = hwnd as usize,
            wparam,
            lparam,
        )
    });
    let _entered = span.as_ref().map(tracing::Span::enter);
    if message == WM_PAINT {
        let mut paint: PAINTSTRUCT = unsafe { zeroed() };
        let dc = unsafe { BeginPaint(hwnd, &mut paint) };
        if dc.is_null() {
            crate::platform_error::PlatformError::null_output("BeginPaint(drop target)").report();
            return 0;
        }
        if SHOULD_PAINT.load(std::sync::atomic::Ordering::Relaxed) {
            let brush = unsafe { CreateSolidBrush(0x005a5a00) };
            if brush.is_null() {
                crate::error_log::report(
                    "CreateSolidBrush(drop target) failed",
                    "returned a null brush",
                );
            }
            let mut client = windows_sys::Win32::Foundation::RECT::default();
            if unsafe { GetClientRect(hwnd, &mut client) } == 0 {
                crate::platform_error::PlatformError::last_win32("GetClientRect(drop target)")
                    .report();
                unsafe { EndPaint(hwnd, &paint) };
                return 0;
            }
            if !brush.is_null() {
                if unsafe { FillRect(dc, &client, brush) } == 0 {
                    crate::error_log::report("FillRect(drop target) failed", "returned zero");
                }
                if unsafe { DeleteObject(brush) } == 0 {
                    crate::error_log::report(
                        "DeleteObject(drop target brush) failed",
                        "returned zero",
                    );
                }
            }
        }
        // EndPaint is documented to always return nonzero.
        unsafe { EndPaint(hwnd, &paint) };
        0
    } else {
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }
}

pub fn create() -> crate::platform_error::PlatformResult<DropTargetWindow> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS;
    use windows_sys::Win32::Graphics::Gdi::{BLACK_BRUSH, GetStockObject};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let name: Vec<u16> = "SimpleWindowDropTargetWindowClass\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = "SimpleWM Drop Target\0".encode_utf16().collect();
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    if instance.is_null() {
        return Err(crate::platform_error::PlatformError::last_win32(
            "GetModuleHandleW(drop target)",
        ));
    }
    let mut class: WNDCLASSEXW = unsafe { zeroed() };
    class.cbSize = size_of::<WNDCLASSEXW>() as u32;
    class.style = CS_DBLCLKS | CS_OWNDC | CS_HREDRAW | CS_VREDRAW;
    class.lpfnWndProc = Some(window_proc);
    class.hInstance = instance;
    class.hIcon = unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) };
    if class.hIcon.is_null() {
        crate::error_log::last_win32("LoadIconW(drop target) failed");
    }
    class.hIconSm = class.hIcon;
    class.hCursor = unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) };
    if class.hCursor.is_null() {
        crate::error_log::last_win32("LoadCursorW(drop target) failed");
    }
    class.hbrBackground = unsafe { GetStockObject(BLACK_BRUSH) } as _;
    if class.hbrBackground.is_null() {
        crate::error_log::report(
            "GetStockObject(BLACK_BRUSH) failed",
            "returned a null object",
        );
    }
    class.lpszClassName = name.as_ptr();
    if unsafe { RegisterClassExW(&class) } == 0 {
        let error =
            crate::platform_error::PlatformError::last_win32("RegisterClassExW(drop target)");
        if !error.is_win32(ERROR_CLASS_ALREADY_EXISTS) {
            return Err(error);
        }
    }
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_WINDOWEDGE | WS_EX_NOACTIVATE | WS_EX_LAYERED,
            name.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null_mut(),
        )
    };
    if hwnd.is_null() {
        return Err(crate::platform_error::PlatformError::last_win32(
            "CreateWindowExW(drop target)",
        ));
    }
    if unsafe { SetLayeredWindowAttributes(hwnd, 0x00ff_ffff, 127, LWA_ALPHA) } == 0 {
        let error = crate::platform_error::PlatformError::last_win32(
            "SetLayeredWindowAttributes(drop target)",
        );
        if unsafe { DestroyWindow(hwnd) } == 0 {
            crate::platform_error::PlatformError::last_win32(
                "DestroyWindow(drop target initialization cleanup)",
            )
            .report();
        }
        return Err(error);
    }
    Ok(DropTargetWindow {
        hwnd: hwnd as isize,
    })
}

pub fn show(window: &DropTargetWindow, rect: crate::Rect, dragged_hwnd: isize, should_paint: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    if let Err(error) = raise_dragged_window(dragged_hwnd) {
        error.report();
        return;
    }
    SHOULD_PAINT.store(should_paint, std::sync::atomic::Ordering::Relaxed);
    unsafe {
        if SetWindowPos(
            window.hwnd as _,
            // The stationary border may still be behind another application.
            // Anchor to the raised drag window so the preview covers the target.
            dragged_hwnd as _,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_SHOWWINDOW | SWP_NOACTIVATE,
        ) == 0
        {
            crate::error_log::last_win32("SetWindowPos(show drop target) failed");
        }
    }
}

fn raise_dragged_window(hwnd: isize) -> crate::platform_error::PlatformResult<()> {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        let previous = GetWindow(hwnd as _, GW_HWNDPREV);
        let topmost = GetWindowLongW(hwnd as _, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0;
        // Already first in its group: avoid redundant positioning and events.
        if previous.is_null()
            || (!topmost && GetWindowLongW(previous, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0)
        {
            return Ok(());
        }
        crate::platform_error::check_bool(
            SetWindowPos(
                hwnd as _,
                HWND_TOP,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOOWNERZORDER,
            ),
            "SetWindowPos(raise dragged window)",
        )
    }
}

pub fn hide(window: &DropTargetWindow) {
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
            window.hwnd as _,
            windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    struct TestWindow(isize);

    impl TestWindow {
        fn new() -> Self {
            let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    class.as_ptr(),
                    std::ptr::null(),
                    WS_POPUP,
                    0,
                    0,
                    0,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                )
            };
            assert!(!hwnd.is_null(), "CreateWindowExW(test window)");
            Self(hwnd as isize)
        }

        fn put_after(&self, other: isize) {
            assert_ne!(
                unsafe {
                    SetWindowPos(
                        self.0 as _,
                        other as _,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
                    )
                },
                0
            );
        }

        fn is_above(&self, other: &Self) -> bool {
            let mut next = unsafe { GetWindow(self.0 as _, GW_HWNDNEXT) };
            while !next.is_null() {
                if next as isize == other.0 {
                    return true;
                }
                next = unsafe { GetWindow(next, GW_HWNDNEXT) };
            }
            false
        }
    }

    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe {
                DestroyWindow(self.0 as _);
            }
        }
    }

    #[test]
    fn repeated_drag_keeps_window_and_preview_above_target_with_a_stale_border() {
        // Zero-size windows exercise Win32 stacking without visible test UI.
        let dragged = TestWindow::new();
        let target = TestWindow::new();
        let border = TestWindow::new();
        let preview = TestWindow::new();
        let overlay = DropTargetWindow { hwnd: preview.0 };
        for _ in 0..2 {
            // A layout swap can leave the border and next drag below the target.
            target.put_after(HWND_TOP as isize);
            border.put_after(target.0);
            dragged.put_after(border.0);
            assert!(target.is_above(&dragged));
            show(
                &overlay,
                crate::Rect::from_xywh(0, 0, 0, 0),
                dragged.0,
                true,
            );
            assert!(dragged.is_above(&preview));
            assert!(preview.is_above(&target));
            assert_eq!(
                unsafe { GetWindowLongW(dragged.0 as _, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST,
                0
            );

            // The subsequent stationary-border update must preserve that order.
            border.put_after(dragged.0);
            assert!(dragged.is_above(&border));
            assert!(border.is_above(&preview));
            assert!(preview.is_above(&target));
            hide(&overlay);
        }
    }
}
