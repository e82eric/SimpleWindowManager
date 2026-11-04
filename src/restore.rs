pub fn restore_moved_windows_to_screen(
    no_restore: bool,
) -> crate::platform_error::PlatformResult<()> {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::{HWND, RECT};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let wide = |value: &str| value.encode_utf16().chain([0]).collect::<Vec<_>>();
    unsafe {
        let primary = FindWindowW(wide("Shell_TrayWnd").as_ptr(), std::ptr::null());
        if !primary.is_null() && IsWindowVisible(primary) == 0 {
            println!("Unhiding primary taskbar");
            if !no_restore {
                ShowWindow(primary, SW_SHOW);
            }
        }
        let secondary_class = wide("Shell_SecondaryTrayWnd");
        let mut secondary: HWND = FindWindowW(secondary_class.as_ptr(), std::ptr::null());
        while !secondary.is_null() {
            if IsWindowVisible(secondary) == 0 {
                println!("Unhiding secondary taskbar");
                if !no_restore {
                    ShowWindow(secondary, SW_SHOW);
                }
            }
            secondary = FindWindowExW(
                std::ptr::null_mut(),
                secondary,
                secondary_class.as_ptr(),
                std::ptr::null(),
            );
        }
        let screen_right = GetSystemMetrics(SM_CXMAXTRACK);
        for window in crate::snapshot::capture_list_windows()? {
            let hwnd = window.data.hwnd as HWND;
            let mut rect: RECT = zeroed();
            if let Err(error) = crate::platform_error::check_bool(
                GetWindowRect(hwnd, &mut rect),
                "GetWindowRect(restore moved window)",
            ) {
                error.report();
                continue;
            }
            if rect.right > screen_right
                && !window.data.title.contains("Progman")
                && !window.data.title.contains("ApplicationFrameWindow")
            {
                println!(
                    "Restoring window, title: {} right: {}",
                    window.data.title, rect.right
                );
                if !no_restore {
                    if let Err(error) = crate::platform_error::check_bool(
                        SetWindowPos(
                            hwnd,
                            std::ptr::null_mut(),
                            25,
                            25,
                            0,
                            0,
                            SWP_NOSIZE | SWP_NOZORDER,
                        ),
                        "SetWindowPos(restore moved window)",
                    ) {
                        error.report();
                    }
                }
            }
        }
    }
    Ok(())
}
