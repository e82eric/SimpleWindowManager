#![allow(unsafe_op_in_unsafe_fn)]

use crate::{ClientData, Monitor, Rect};

#[derive(Debug, Default)]
pub struct DesktopSnapshot {
    pub monitors: Vec<Monitor>,
    pub windows: Vec<WindowSnapshot>,
}

#[derive(Debug, Default)]
pub struct WindowSnapshot {
    pub data: ClientData,
    pub styles: i32,
    pub ex_styles: i32,
    pub normal_rect: Rect,
}

unsafe fn process_is_elevated(process: windows_sys::Win32::Foundation::HANDLE) -> bool {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Security::{
        GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
    };
    use windows_sys::Win32::System::Threading::OpenProcessToken;
    let mut token = std::ptr::null_mut();
    if process.is_null() {
        return false;
    }
    if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
        crate::error_log::last_win32("OpenProcessToken failed");
        return false;
    }
    let mut elevation: TOKEN_ELEVATION = zeroed();
    let mut returned = 0;
    let information_result = GetTokenInformation(
        token,
        TokenElevation,
        &mut elevation as *mut _ as _,
        size_of::<TOKEN_ELEVATION>() as u32,
        &mut returned,
    );
    if information_result == 0 {
        crate::error_log::last_win32("GetTokenInformation(TokenElevation) failed");
    }
    if CloseHandle(token) == 0 {
        crate::error_log::last_win32("CloseHandle(process token) failed");
    }
    information_result != 0 && elevation.TokenIsElevated != 0
}

unsafe fn window_long(hwnd: windows_sys::Win32::Foundation::HWND, index: i32) -> i32 {
    match unsafe { window_long_checked(hwnd, index) } {
        Ok(value) => value,
        Err(error) => {
            crate::error_log::report_win32(format!("GetWindowLongW({index}) failed"), error);
            0
        }
    }
}

unsafe fn window_long_checked(
    hwnd: windows_sys::Win32::Foundation::HWND,
    index: i32,
) -> Result<i32, u32> {
    use windows_sys::Win32::Foundation::{GetLastError, SetLastError};
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongW;
    SetLastError(0);
    let value = GetWindowLongW(hwnd, index);
    let error = GetLastError();
    if value == 0 && error != 0 {
        Err(error)
    } else {
        Ok(value)
    }
}

unsafe fn window_text_length(hwnd: windows_sys::Win32::Foundation::HWND) -> usize {
    use windows_sys::Win32::Foundation::{GetLastError, SetLastError};
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextLengthW;
    SetLastError(0);
    let length = GetWindowTextLengthW(hwnd);
    let error = GetLastError();
    if length == 0 && error != 0 && error != 1400 {
        crate::error_log::report_win32("GetWindowTextLengthW failed", error);
    }
    length.max(0) as usize
}

unsafe fn window_text(hwnd: windows_sys::Win32::Foundation::HWND, buffer: &mut [u16]) -> i32 {
    use windows_sys::Win32::Foundation::{GetLastError, SetLastError};
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    SetLastError(0);
    let length = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
    let error = GetLastError();
    if length == 0 && error != 0 && error != 1400 {
        crate::error_log::report_win32("GetWindowTextW failed", error);
    }
    length
}

unsafe fn parent_window(
    hwnd: windows_sys::Win32::Foundation::HWND,
) -> windows_sys::Win32::Foundation::HWND {
    use windows_sys::Win32::Foundation::{GetLastError, SetLastError};
    use windows_sys::Win32::UI::WindowsAndMessaging::GetParent;
    SetLastError(0);
    let parent = GetParent(hwnd);
    let error = GetLastError();
    if parent.is_null() && error != 0 {
        crate::error_log::report_win32("GetParent failed", error);
    }
    parent
}

unsafe fn root_owner(
    hwnd: windows_sys::Win32::Foundation::HWND,
) -> windows_sys::Win32::Foundation::HWND {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GA_ROOTOWNER, GetAncestor};
    let ancestor = GetAncestor(hwnd, GA_ROOTOWNER);
    if ancestor.is_null() {
        crate::error_log::report("GetAncestor(GA_ROOTOWNER) failed", "returned a null window");
    }
    ancestor
}

unsafe fn last_active_popup(
    hwnd: windows_sys::Win32::Foundation::HWND,
) -> windows_sys::Win32::Foundation::HWND {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetLastActivePopup;
    let popup = GetLastActivePopup(hwnd);
    if popup.is_null() {
        crate::error_log::report("GetLastActivePopup failed", "returned a null window");
    }
    popup
}

pub fn is_root_window_handle(hwnd: isize, float_uwp_windows: bool) -> bool {
    use windows_sys::Win32::Foundation::{HANDLE, HWND};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe fn text(hwnd: HWND) -> String {
        let mut buffer = [0u16; 256];
        let length = GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        if length == 0 {
            crate::error_log::last_win32("GetClassNameW failed");
        }
        String::from_utf16_lossy(&buffer[..length.max(0) as usize])
    }
    unsafe extern "system" fn property(
        _hwnd: HWND,
        name: *const u16,
        value: HANDLE,
        state: usize,
    ) -> i32 {
        if name as usize > 0xffff {
            let mut length = 0;
            while unsafe { *name.add(length) } != 0 {
                length += 1;
            }
            if String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(name, length) })
                == "ApplicationViewCloakType"
            {
                unsafe { *(state as *mut bool) = value.is_null() };
                return 0;
            }
        }
        1
    }
    unsafe {
        let hwnd = hwnd as HWND;
        if hwnd.is_null() || hwnd == GetDesktopWindow() || IsWindow(hwnd) == 0 {
            return false;
        }
        if text(hwnd).contains("ApplicationFrameWindow") && !float_uwp_windows {
            let mut appropriate = false;
            EnumPropsExW(hwnd, Some(property), &mut appropriate as *mut bool as isize);
            return appropriate;
        }
        let styles = window_long(hwnd, GWL_STYLE);
        let ex_styles = window_long(hwnd, GWL_EXSTYLE);
        if ex_styles & WS_EX_NOACTIVATE as i32 != 0
            || styles & WS_CHILD as i32 != 0
            || !parent_window(hwnd).is_null()
            || !GetWindow(hwnd, GW_OWNER).is_null()
        {
            return false;
        }
        let mut walk = root_owner(hwnd);
        loop {
            let candidate = last_active_popup(walk);
            if candidate == walk {
                break;
            }
            if IsWindowVisible(candidate) != 0 {
                break;
            }
            walk = candidate;
        }
        walk == hwnd
    }
}

fn capture_window_data(hwnd: isize, require_visible: bool) -> Option<WindowSnapshot> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ, QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let hwnd_value = hwnd as windows_sys::Win32::Foundation::HWND;
    if hwnd_value.is_null() || unsafe { IsWindow(hwnd_value) } == 0 {
        return None;
    }
    let styles = match unsafe { window_long_checked(hwnd_value, GWL_STYLE) } {
        Ok(styles) => styles,
        Err(1400) => return None,
        Err(error) => {
            crate::error_log::report_win32("GetWindowLongW(-16) failed", error);
            return None;
        }
    };
    if require_visible && styles & WS_VISIBLE as i32 == 0 {
        return None;
    }
    let wide = |class_name: bool| unsafe {
        let length = if class_name {
            255
        } else {
            window_text_length(hwnd_value)
        };
        let mut buffer = vec![0u16; length + 1];
        let written = if class_name {
            GetClassNameW(hwnd_value, buffer.as_mut_ptr(), buffer.len() as i32)
        } else {
            window_text(hwnd_value, &mut buffer)
        };
        if class_name && written == 0 {
            let error = windows_sys::Win32::Foundation::GetLastError();
            if error != 1400 {
                crate::error_log::report_win32("GetClassNameW failed", error);
            }
        }
        String::from_utf16_lossy(&buffer[..written.max(0) as usize])
    };
    let class_name = wide(true);
    if unsafe { IsWindow(hwnd_value) } == 0 {
        return None;
    }
    let title = wide(false);
    let mut process_id = 0;
    if unsafe { GetWindowThreadProcessId(hwnd_value, &mut process_id) } == 0 {
        let error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        if error != 1400 {
            crate::error_log::report_win32("GetWindowThreadProcessId failed", error);
        }
        return None;
    }
    let process =
        unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, process_id) };
    let mut process_image_name = String::new();
    if process.is_null() {
        crate::error_log::last_win32("OpenProcess(window) failed");
    }
    let is_elevated = unsafe { process_is_elevated(process) };
    if !process.is_null() {
        let mut buffer = vec![0u16; 1024];
        let mut length = buffer.len() as u32;
        if unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) } != 0
        {
            process_image_name = String::from_utf16_lossy(&buffer[..length as usize]);
        } else {
            crate::error_log::last_win32("QueryFullProcessImageNameW failed");
        }
        if unsafe { CloseHandle(process) } == 0 {
            crate::error_log::last_win32("CloseHandle(window process) failed");
        }
    }
    let mut placement: WINDOWPLACEMENT = unsafe { zeroed() };
    placement.length = size_of::<WINDOWPLACEMENT>() as u32;
    if unsafe { GetWindowPlacement(hwnd_value, &mut placement) } == 0 {
        let error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        if error != 1400 {
            crate::error_log::report_win32("GetWindowPlacement failed", error);
        }
        return None;
    }
    let ex_styles = match unsafe { window_long_checked(hwnd_value, GWL_EXSTYLE) } {
        Ok(ex_styles) => ex_styles,
        Err(1400) => return None,
        Err(error) => {
            crate::error_log::report_win32("GetWindowLongW(-20) failed", error);
            return None;
        }
    };
    Some(WindowSnapshot {
        data: ClientData {
            hwnd,
            process_id,
            process_image_name,
            class_name,
            title,
            is_elevated,
            is_minimized: unsafe { IsIconic(hwnd_value) } != 0,
            ..Default::default()
        },
        styles,
        ex_styles,
        normal_rect: Rect {
            left: placement.rcNormalPosition.left,
            top: placement.rcNormalPosition.top,
            right: placement.rcNormalPosition.right,
            bottom: placement.rcNormalPosition.bottom,
        },
    })
}

#[tracing::instrument(fields(hwnd = hwnd as usize))]
pub fn capture_window(
    hwnd: isize,
    require_visible: bool,
    float_uwp_windows: bool,
) -> Option<WindowSnapshot> {
    is_root_window_handle(hwnd, float_uwp_windows)
        .then(|| capture_window_data(hwnd, require_visible))
        .flatten()
}

pub fn capture_foreground_window(hwnd: isize) -> Option<WindowSnapshot> {
    capture_window_data(hwnd, false)
}

pub fn is_list_window_handle(hwnd: isize) -> bool {
    use std::mem::size_of;
    use windows_sys::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        let value = hwnd as windows_sys::Win32::Foundation::HWND;
        if value.is_null()
            || value == GetDesktopWindow()
            || value == GetShellWindow()
            || IsWindowVisible(value) == 0
        {
            return false;
        }
        let mut class = [0u16; 256];
        let class_length = GetClassNameW(value, class.as_mut_ptr(), class.len() as i32);
        if class_length == 0 {
            crate::error_log::last_win32("GetClassNameW failed");
        }
        if String::from_utf16_lossy(&class[..class_length.max(0) as usize])
            .contains("ApplicationFrameWindow")
        {
            return is_root_window_handle(hwnd, false);
        }
        let styles = window_long(value, GWL_STYLE);
        let ex_styles = window_long(value, GWL_EXSTYLE);
        if ex_styles & WS_EX_TOOLWINDOW as i32 != 0
            || ex_styles & WS_EX_NOACTIVATE as i32 != 0
            || styles & WS_CHILD as i32 != 0
            || !parent_window(value).is_null()
        {
            return false;
        }
        let mut cloaked = 0u32;
        let cloak_result = DwmGetWindowAttribute(
            value,
            DWMWA_CLOAKED as u32,
            &mut cloaked as *mut _ as _,
            size_of::<u32>() as u32,
        );
        if cloak_result < 0 {
            crate::error_log::report_hresult(
                "DwmGetWindowAttribute(DWMWA_CLOAKED) failed",
                cloak_result,
            );
        } else if cloaked != 0 {
            return false;
        }
        let mut walk = root_owner(value);
        loop {
            let candidate = last_active_popup(walk);
            if candidate == walk {
                break;
            }
            if IsWindowVisible(candidate) != 0 {
                break;
            }
            walk = candidate;
        }
        walk == value
    }
}

#[tracing::instrument]
pub fn capture_list_windows() -> Result<Vec<WindowSnapshot>, u32> {
    use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::EnumWindows;
    unsafe extern "system" fn callback(hwnd: HWND, state: LPARAM) -> i32 {
        if is_list_window_handle(hwnd as isize) {
            if let Some(window) = capture_window_data(hwnd as isize, true) {
                if !window.data.class_name.contains("Progman")
                    && !window.data.title.contains("ApplicationFrameWindow")
                {
                    (*(state as *mut Vec<WindowSnapshot>)).push(window);
                }
            }
        }
        1
    }
    let mut windows = Vec::new();
    if unsafe { EnumWindows(Some(callback), &mut windows as *mut _ as LPARAM) } == 0 {
        let error = unsafe { GetLastError() };
        crate::error_log::report_win32("EnumWindows(list windows) failed", error);
        Err(error)
    } else {
        Ok(windows)
    }
}

#[tracing::instrument]
pub fn capture(float_uwp_windows: bool) -> Result<DesktopSnapshot, u32> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HWND, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ, QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumPropsExW, EnumWindows, GW_OWNER, GWL_EXSTYLE, GWL_STYLE, GetClassNameW,
        GetDesktopWindow, GetWindow, GetWindowPlacement, GetWindowThreadProcessId, IsIconic,
        IsWindowVisible, WINDOWPLACEMENT, WS_CHILD, WS_EX_NOACTIVATE, WS_VISIBLE,
    };

    unsafe extern "system" fn monitor_callback(
        monitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> i32 {
        let monitors = &mut *(data as *mut Vec<Monitor>);
        let mut info: MONITORINFO = zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) != 0 {
            let rect = Rect {
                left: info.rcWork.left,
                top: info.rcWork.top,
                right: info.rcWork.right,
                bottom: info.rcWork.bottom,
            };
            monitors.push(Monitor::new(monitors.len() as i32, rect, 10));
        } else {
            crate::error_log::last_win32("GetMonitorInfoW failed");
        }
        1
    }

    unsafe fn wide_text(hwnd: HWND, class_name: bool) -> String {
        let length = if class_name {
            255
        } else {
            window_text_length(hwnd)
        };
        let mut buffer = vec![0u16; length + 1];
        let written = if class_name {
            GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32)
        } else {
            window_text(hwnd, &mut buffer)
        };
        if class_name && written == 0 {
            crate::error_log::last_win32("GetClassNameW failed");
        }
        String::from_utf16_lossy(&buffer[..written.max(0) as usize])
    }

    unsafe fn is_alt_tab_window(hwnd: HWND) -> bool {
        let mut walk = root_owner(hwnd);
        loop {
            let candidate = last_active_popup(walk);
            if candidate == walk {
                break;
            }
            if IsWindowVisible(candidate) != 0 {
                break;
            }
            walk = candidate;
        }
        walk == hwnd
    }

    unsafe fn is_root_window(
        hwnd: HWND,
        styles: i32,
        ex_styles: i32,
        float_uwp_windows: bool,
    ) -> bool {
        if hwnd == GetDesktopWindow() {
            return false;
        }
        if wide_text(hwnd, true).contains("ApplicationFrameWindow") && !float_uwp_windows {
            unsafe extern "system" fn property(
                _hwnd: HWND,
                name: *const u16,
                value: windows_sys::Win32::Foundation::HANDLE,
                state: usize,
            ) -> i32 {
                if name as usize > 0xffff {
                    let mut length = 0;
                    while *name.add(length) != 0 {
                        length += 1;
                    }
                    if String::from_utf16_lossy(std::slice::from_raw_parts(name, length))
                        == "ApplicationViewCloakType"
                    {
                        *(state as *mut bool) = value.is_null();
                        return 0;
                    }
                }
                1
            }
            let mut appropriate = false;
            EnumPropsExW(hwnd, Some(property), &mut appropriate as *mut bool as isize);
            return appropriate;
        }
        if ex_styles & WS_EX_NOACTIVATE as i32 != 0
            || styles & WS_CHILD as i32 != 0
            || !parent_window(hwnd).is_null()
            || !GetWindow(hwnd, GW_OWNER).is_null()
        {
            return false;
        }
        is_alt_tab_window(hwnd)
    }

    unsafe extern "system" fn window_callback(hwnd: HWND, data: LPARAM) -> i32 {
        let context = &mut *(data as *mut (Vec<WindowSnapshot>, bool));
        let styles = window_long(hwnd, GWL_STYLE);
        let ex_styles = window_long(hwnd, GWL_EXSTYLE);
        if !is_root_window(hwnd, styles, ex_styles, context.1) {
            return 1;
        }
        if styles & WS_VISIBLE as i32 == 0 {
            return 1;
        }
        let class_name = wide_text(hwnd, true);
        let title = wide_text(hwnd, false);
        let mut process_id = 0;
        if GetWindowThreadProcessId(hwnd, &mut process_id) == 0 {
            crate::error_log::last_win32("GetWindowThreadProcessId failed");
        }
        let process = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, process_id);
        let mut process_image_name = String::new();
        let is_elevated = process_is_elevated(process);
        if process.is_null() {
            crate::error_log::last_win32("OpenProcess(window) failed");
        }
        if !process.is_null() {
            let mut buffer = vec![0u16; 1024];
            let mut length = buffer.len() as u32;
            if QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) != 0 {
                process_image_name = String::from_utf16_lossy(&buffer[..length as usize]);
            } else {
                crate::error_log::last_win32("QueryFullProcessImageNameW failed");
            }
            if CloseHandle(process) == 0 {
                crate::error_log::last_win32("CloseHandle(window process) failed");
            }
        }
        let mut placement: WINDOWPLACEMENT = zeroed();
        placement.length = size_of::<WINDOWPLACEMENT>() as u32;
        if GetWindowPlacement(hwnd, &mut placement) == 0 {
            crate::error_log::last_win32("GetWindowPlacement failed");
        }
        context.0.push(WindowSnapshot {
            data: ClientData {
                hwnd: hwnd as isize,
                process_id,
                process_image_name,
                class_name,
                title,
                is_elevated,
                is_minimized: IsIconic(hwnd) != 0,
                ..Default::default()
            },
            styles,
            ex_styles,
            normal_rect: Rect {
                left: placement.rcNormalPosition.left,
                top: placement.rcNormalPosition.top,
                right: placement.rcNormalPosition.right,
                bottom: placement.rcNormalPosition.bottom,
            },
        });
        1
    }

    let mut snapshot = DesktopSnapshot::default();
    unsafe {
        if EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(monitor_callback),
            &mut snapshot.monitors as *mut _ as LPARAM,
        ) == 0
        {
            let error = GetLastError();
            crate::error_log::report_win32("EnumDisplayMonitors failed", error);
            return Err(error);
        }
        let mut window_context = (Vec::new(), float_uwp_windows);
        if EnumWindows(
            Some(window_callback),
            &mut window_context as *mut _ as LPARAM,
        ) == 0
        {
            let error = GetLastError();
            crate::error_log::report_win32("EnumWindows(desktop capture) failed", error);
            return Err(error);
        }
        snapshot.windows = window_context.0;
    }
    Ok(snapshot)
}
