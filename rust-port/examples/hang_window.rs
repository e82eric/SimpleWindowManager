//! Run with: cargo run --manifest-path rust-port/Cargo.toml --example hang_window
//! Deliberately blocks this process's UI thread; does not suspend another app.
#![windows_subsystem = "windows"]

use windows_sys::Win32::{
    Foundation::*, Graphics::Gdi::*, System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::*,
};

const BUTTON: usize = 100;
const HANG_TIMER: usize = 1;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, w: usize, l: isize) -> isize {
    unsafe {
        match message {
            WM_COMMAND if w & 0xffff == BUTTON && w >> 16 == BN_CLICKED as usize => {
                // Defer the sleep so the button's mouse-up finishes first.
                // Disable it to prevent repeated clicks queuing more freezes.
                windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                    GetDlgItem(hwnd, BUTTON as i32),
                    0,
                );
                SetWindowTextW(hwnd, wide("Hang test — starting in 1 second").as_ptr());
                if SetTimer(hwnd, HANG_TIMER, 1000, None) == 0 {
                    windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                        GetDlgItem(hwnd, BUTTON as i32),
                        1,
                    );
                    SetWindowTextW(hwnd, wide("Hang test — timer failed").as_ptr());
                }
                0
            }
            WM_TIMER if w == HANG_TIMER => {
                KillTimer(hwnd, HANG_TIMER);
                SetWindowTextW(
                    hwnd,
                    wide("Hang test — UI thread blocked for 5 seconds").as_ptr(),
                );
                // Intentionally no message pumping here. This is the test.
                std::thread::sleep(std::time::Duration::from_secs(30));
                SetWindowTextW(
                    hwnd,
                    wide("Hang test — recovered, ready to repeat").as_ptr(),
                );
                windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                    GetDlgItem(hwnd, BUTTON as i32),
                    1,
                );
                InvalidateRect(hwnd, std::ptr::null(), 1);
                0
            }
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut paint);
                if !dc.is_null() {
                    let mut rect = RECT {
                        left: 20,
                        top: 20,
                        right: 500,
                        bottom: 130,
                    };
                    SetBkMode(dc, TRANSPARENT as i32);
                    DrawTextW(dc, wide("Click the button, then test SWM shortcuts and input in another app.\n\nAfter a 1-second delay, this window stops processing messages for 5 seconds and recovers automatically.").as_ptr(), -1, &mut rect, DT_LEFT | DT_WORDBREAK);
                }
                EndPaint(hwnd, &paint);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, w, l),
        }
    }
}

fn run() -> Result<(), std::io::Error> {
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        if instance.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let name = wide("SwmHangTestWindow");
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: name.as_ptr(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let hwnd = CreateWindowExW(
            0,
            name.as_ptr(),
            wide("Hang test — ready").as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            280,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let button = CreateWindowExW(
            0,
            wide("BUTTON").as_ptr(),
            wide("Hang for 5 seconds").as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON as u32,
            20,
            145,
            220,
            40,
            hwnd,
            BUTTON as HMENU,
            instance,
            std::ptr::null(),
        );
        if button.is_null() {
            let error = std::io::Error::last_os_error();
            DestroyWindow(hwnd);
            return Err(error);
        }
        ShowWindow(hwnd, SW_SHOW);
        let mut message = MSG::default();
        loop {
            match GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) {
                -1 => return Err(std::io::Error::last_os_error()),
                0 => return Ok(()),
                _ => {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        }
    }
}

fn main() {
    if let Err(error) = run() {
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                wide(&error.to_string()).as_ptr(),
                wide("Hang test failed").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}
