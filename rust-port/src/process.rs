use crate::platform_error::{PlatformError, PlatformResult, check_bool};

unsafe fn close_handle(
    handle: windows_sys::Win32::Foundation::HANDLE,
    operation: &'static str,
) -> PlatformResult<()> {
    if handle.is_null() {
        Ok(())
    } else {
        check_bool(
            unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) },
            operation,
        )
    }
}

unsafe fn close_service_handle(
    handle: windows_sys::Win32::System::Services::SC_HANDLE,
) -> PlatformResult<()> {
    check_bool(
        unsafe { windows_sys::Win32::System::Services::CloseServiceHandle(handle) },
        "CloseServiceHandle",
    )
}

pub fn start_app(command: &str) -> PlatformResult<()> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let operation: Vec<u16> = "open\0".encode_utf16().collect();
    let command: Vec<u16> = command.encode_utf16().chain([0]).collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            command.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as isize;
    if result <= 32 {
        Err(PlatformError::api_code("ShellExecuteW", result))
    } else {
        Ok(())
    }
}

pub fn start_as_explorer_user(command: &str) -> PlatformResult<()> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Security::{
        DuplicateTokenEx, SecurityImpersonation, TOKEN_DUPLICATE, TOKEN_QUERY, TokenPrimary,
    };
    use windows_sys::Win32::System::SystemServices::MAXIMUM_ALLOWED;
    use windows_sys::Win32::System::Threading::{
        CreateProcessWithTokenW, OpenProcess, OpenProcessToken, PROCESS_INFORMATION,
        PROCESS_QUERY_INFORMATION, STARTF_USESHOWWINDOW, STARTUPINFOW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetShellWindow, GetWindowThreadProcessId, SW_SHOWNORMAL,
    };
    let shell = unsafe { GetShellWindow() };
    if shell.is_null() {
        return Err(PlatformError::null_output("GetShellWindow"));
    }
    let mut pid = 0;
    if unsafe { GetWindowThreadProcessId(shell, &mut pid) } == 0 {
        return Err(PlatformError::last_win32("GetWindowThreadProcessId(shell)"));
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION, 0, pid) };
    if process.is_null() {
        return Err(PlatformError::last_win32("OpenProcess(shell)"));
    }
    let mut token = std::ptr::null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_DUPLICATE | TOKEN_QUERY, &mut token) } == 0 {
        let error = PlatformError::last_win32("OpenProcessToken(shell)");
        if let Err(close_error) = unsafe { close_handle(process, "CloseHandle(shell process)") } {
            close_error.report();
        }
        return Err(error);
    }
    let mut primary = std::ptr::null_mut();
    if unsafe {
        DuplicateTokenEx(
            token,
            MAXIMUM_ALLOWED,
            std::ptr::null(),
            SecurityImpersonation,
            TokenPrimary,
            &mut primary,
        )
    } == 0
    {
        let error = PlatformError::last_win32("DuplicateTokenEx(shell token)");
        unsafe {
            if let Err(close_error) = close_handle(token, "CloseHandle(shell token)") {
                close_error.report();
            }
            if let Err(close_error) = close_handle(process, "CloseHandle(shell process)") {
                close_error.report();
            }
        }
        return Err(error);
    }
    let mut startup: STARTUPINFOW = unsafe { zeroed() };
    startup.cb = size_of::<STARTUPINFOW>() as u32;
    startup.wShowWindow = SW_SHOWNORMAL as u16;
    startup.dwFlags = STARTF_USESHOWWINDOW;
    let mut info: PROCESS_INFORMATION = unsafe { zeroed() };
    let mut command: Vec<u16> = format!("cmd.exe /c start \"\" \"{command}\"")
        .encode_utf16()
        .chain([0])
        .collect();
    let result = unsafe {
        CreateProcessWithTokenW(
            primary,
            1,
            std::ptr::null(),
            command.as_mut_ptr(),
            0x08000000,
            std::ptr::null(),
            std::ptr::null(),
            &startup,
            &mut info,
        )
    };
    let create_error = (result == 0).then(|| PlatformError::last_win32("CreateProcessWithTokenW"));
    let mut close_error = None;
    unsafe {
        for result in [
            close_handle(info.hProcess, "CloseHandle(created process)"),
            close_handle(info.hThread, "CloseHandle(created thread)"),
            close_handle(primary, "CloseHandle(primary token)"),
            close_handle(token, "CloseHandle(shell token)"),
            close_handle(process, "CloseHandle(shell process)"),
        ] {
            if let Err(error) = result {
                if close_error.is_none() {
                    close_error = Some(error);
                } else {
                    error.report();
                }
            }
        }
    }
    if let Some(error) = create_error {
        if let Some(close_error) = close_error {
            close_error.report();
        }
        Err(error)
    } else if let Some(error) = close_error {
        Err(error)
    } else {
        Ok(())
    }
}

pub fn list_services() -> PlatformResult<Vec<String>> {
    use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, GetLastError};
    use windows_sys::Win32::System::Services::*;
    unsafe {
        let manager = OpenSCManagerW(
            std::ptr::null(),
            std::ptr::null(),
            SC_MANAGER_ENUMERATE_SERVICE,
        );
        if manager.is_null() {
            return Err(PlatformError::last_win32("OpenSCManagerW"));
        }
        let mut needed = 0;
        let mut count = 0;
        let mut resume = 0;
        let size_result = EnumServicesStatusExW(
            manager,
            SC_ENUM_PROCESS_INFO,
            SERVICE_WIN32,
            SERVICE_STATE_ALL,
            std::ptr::null_mut(),
            0,
            &mut needed,
            &mut count,
            &mut resume,
            std::ptr::null(),
        );
        if size_result == 0 {
            let error = GetLastError();
            if error != ERROR_MORE_DATA {
                let error = PlatformError::win32("EnumServicesStatusExW(size query)", error);
                if let Err(close_error) = close_service_handle(manager) {
                    close_error.report();
                }
                return Err(error);
            }
        }
        let mut bytes = vec![0u8; needed as usize];
        let ok = EnumServicesStatusExW(
            manager,
            SC_ENUM_PROCESS_INFO,
            SERVICE_WIN32,
            SERVICE_STATE_ALL,
            bytes.as_mut_ptr(),
            bytes.len() as u32,
            &mut needed,
            &mut count,
            &mut resume,
            std::ptr::null(),
        );
        if ok == 0 {
            let error = GetLastError();
            let error = PlatformError::win32("EnumServicesStatusExW", error);
            if let Err(close_error) = close_service_handle(manager) {
                close_error.report();
            }
            return Err(error);
        }
        let entries = std::slice::from_raw_parts(
            bytes.as_ptr() as *const ENUM_SERVICE_STATUS_PROCESSW,
            count as usize,
        );
        let wide = |pointer: *const u16| {
            if pointer.is_null() {
                return String::new();
            }
            let mut length = 0;
            while *pointer.add(length) != 0 {
                length += 1;
            }
            String::from_utf16_lossy(std::slice::from_raw_parts(pointer, length))
        };
        let result = entries
            .iter()
            .map(|entry| {
                format!(
                    "{:<10} {}",
                    if entry.ServiceStatusProcess.dwCurrentState == SERVICE_RUNNING {
                        "RUNNING"
                    } else {
                        "STOPPED"
                    },
                    wide(entry.lpDisplayName)
                )
            })
            .collect();
        close_service_handle(manager)?;
        Ok(result)
    }
}
