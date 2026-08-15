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
    if message == WM_PAINT {
        let mut paint: PAINTSTRUCT = unsafe { zeroed() };
        let dc = unsafe { BeginPaint(hwnd, &mut paint) };
        if SHOULD_PAINT.load(std::sync::atomic::Ordering::Relaxed) {
            let brush = unsafe { CreateSolidBrush(0x005a5a00) };
            let mut client = windows_sys::Win32::Foundation::RECT::default();
            unsafe {
                GetClientRect(hwnd, &mut client);
                FillRect(dc, &client, brush);
                DeleteObject(brush);
            }
        }
        unsafe { EndPaint(hwnd, &paint) };
        0
    } else {
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }
}

pub fn create() -> Result<DropTargetWindow, u32> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Graphics::Gdi::{BLACK_BRUSH, GetStockObject};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let name: Vec<u16> = "SimpleWindowDropTargetWindowClass\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = "SimpleWM Drop Target\0".encode_utf16().collect();
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    let mut class: WNDCLASSEXW = unsafe { zeroed() };
    class.cbSize = size_of::<WNDCLASSEXW>() as u32;
    class.style = CS_DBLCLKS | CS_OWNDC | CS_HREDRAW | CS_VREDRAW;
    class.lpfnWndProc = Some(window_proc);
    class.hInstance = instance;
    class.hIcon = unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) };
    class.hIconSm = class.hIcon;
    class.hCursor = unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) };
    class.hbrBackground = unsafe { GetStockObject(BLACK_BRUSH) } as _;
    class.lpszClassName = name.as_ptr();
    unsafe { RegisterClassExW(&class) };
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_PALETTEWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
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
        return Err(unsafe { GetLastError() });
    }
    unsafe { SetLayeredWindowAttributes(hwnd, 0x00ff_ffff, 127, LWA_ALPHA) };
    Ok(DropTargetWindow {
        hwnd: hwnd as isize,
    })
}

pub fn show(window: &DropTargetWindow, rect: crate::Rect, topmost: bool, should_paint: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    SHOULD_PAINT.store(should_paint, std::sync::atomic::Ordering::Relaxed);
    unsafe {
        SetWindowPos(
            window.hwnd as _,
            if topmost { HWND_TOPMOST } else { HWND_TOP },
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_SHOWWINDOW,
        );
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
