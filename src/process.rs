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

/// Whether `pid` is still the process created at `created` (a FILETIME), so a
/// recorded pid that Windows has since reused does not count.
pub fn is_running(pid: u32, created: u64) -> bool {
    use windows_sys::Win32::Foundation::{FILETIME, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return false;
    }
    let mut times = [FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    }; 4];
    let mut exit_code = 0;
    let [creation, exit, kernel, user] = &mut times;
    let running = unsafe {
        GetProcessTimes(process, creation, exit, kernel, user) != 0
            && GetExitCodeProcess(process, &mut exit_code) != 0
    } && exit_code == STILL_ACTIVE as u32
        && filetime_ticks(*creation) == created;
    if let Err(error) = unsafe { close_handle(process, "CloseHandle(process)") } {
        error.report();
    }
    running
}

/// A FILETIME as one count of 100 ns ticks.
pub(crate) fn filetime_ticks(time: windows_sys::Win32::Foundation::FILETIME) -> u64 {
    u64::from(time.dwHighDateTime) << 32 | u64::from(time.dwLowDateTime)
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

/// A handle closed, and any failure reported, when dropped.
struct Handle(windows_sys::Win32::Foundation::HANDLE, &'static str);
impl Drop for Handle {
    fn drop(&mut self) {
        if let Err(error) = unsafe { close_handle(self.0, self.1) } {
            error.report();
        }
    }
}

/// Open `path` (a file, shortcut, folder or program) as the desktop user, as
/// double-clicking it in Explorer would. This goes through `explorer.exe` rather
/// than `cmd /c start`, which would expand any `%VAR%` in the path.
pub fn start_as_explorer_user(path: &str) -> PlatformResult<()> {
    // No Windows path holds a quote; refuse one rather than let it end the argument.
    if path.contains('"') {
        return Err(PlatformError::win32(
            "start_as_explorer_user(path)",
            windows_sys::Win32::Foundation::ERROR_INVALID_NAME,
        ));
    }
    let explorer = windows_directory_file("explorer.exe")?;
    // Explorer reads its own command line: a plain quoted path, so a drive root
    // stays `"C:\"` rather than gaining the `\\` that `quote_argument` would add.
    create_as_explorer_user(format!("\"{explorer}\" \"{path}\""))
}

/// `name` in the Windows directory, as a full path, so it isn't searched for.
fn windows_directory_file(name: &str) -> PlatformResult<String> {
    use windows_sys::Win32::System::SystemInformation::GetWindowsDirectoryW;
    let mut buffer = [0u16; 260];
    let length =
        unsafe { GetWindowsDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    if length == 0 || length >= buffer.len() {
        return Err(PlatformError::last_win32("GetWindowsDirectoryW"));
    }
    Ok(format!(
        "{}\\{name}",
        String::from_utf16_lossy(&buffer[..length]).trim_end_matches('\\')
    ))
}

/// Run `program` with `args` as the desktop user rather than SWM's elevated user.
/// Returns once the process has started.
pub fn start_program_as_explorer_user(program: &str, args: &[&str]) -> PlatformResult<()> {
    let command_line = std::iter::once(program)
        .chain(args.iter().copied())
        .map(quote_argument)
        .collect::<Vec<_>>()
        .join(" ");
    create_as_explorer_user(command_line)
}

/// Quote one argument for the standard Windows command-line parser.
fn quote_argument(argument: &str) -> String {
    let mut quoted = String::from('"');
    let mut backslashes = 0;
    // Backslashes are literal unless they precede a quote, so double those runs.
    for ch in argument.chars() {
        if ch == '\\' {
            backslashes += 1;
            continue;
        }
        let escaped = if ch == '"' {
            backslashes * 2 + 1
        } else {
            backslashes
        };
        quoted.extend(std::iter::repeat_n('\\', escaped));
        quoted.push(ch);
        backslashes = 0;
    }
    quoted.extend(std::iter::repeat_n('\\', backslashes * 2));
    quoted.push('"');
    quoted
}

/// The desktop shell's token, duplicated as `kind`: `TokenPrimary` to start a
/// process as the desktop user, `TokenImpersonation` to act as them on a thread.
fn shell_token(kind: windows_sys::Win32::Security::TOKEN_TYPE) -> PlatformResult<Handle> {
    use windows_sys::Win32::Security::{
        DuplicateTokenEx, SecurityImpersonation, TOKEN_DUPLICATE, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::SystemServices::MAXIMUM_ALLOWED;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, OpenProcessToken, PROCESS_QUERY_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetShellWindow, GetWindowThreadProcessId};
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
    let process = Handle(process, "CloseHandle(shell process)");
    let mut token = std::ptr::null_mut();
    if unsafe { OpenProcessToken(process.0, TOKEN_DUPLICATE | TOKEN_QUERY, &mut token) } == 0 {
        return Err(PlatformError::last_win32("OpenProcessToken(shell)"));
    }
    let token = Handle(token, "CloseHandle(shell token)");
    let mut duplicate = std::ptr::null_mut();
    if unsafe {
        DuplicateTokenEx(
            token.0,
            MAXIMUM_ALLOWED,
            std::ptr::null(),
            SecurityImpersonation,
            kind,
            &mut duplicate,
        )
    } == 0
    {
        return Err(PlatformError::last_win32("DuplicateTokenEx(shell token)"));
    }
    Ok(Handle(duplicate, "CloseHandle(duplicated shell token)"))
}

/// Run `f` on this thread as the desktop user rather than SWM's elevated user.
/// Files it creates are checked against, and owned by, that user, so a folder the
/// user's own programs have redirected somewhere protected (a junction, or a
/// known-folder setting in HKCU) refuses the write instead of SWM making it as admin.
pub fn as_desktop_user<T>(f: impl FnOnce() -> T) -> PlatformResult<T> {
    use windows_sys::Win32::Security::{RevertToSelf, TokenImpersonation};
    use windows_sys::Win32::System::Threading::SetThreadToken;
    struct Revert;
    impl Drop for Revert {
        fn drop(&mut self) {
            if let Err(error) = check_bool(unsafe { RevertToSelf() }, "RevertToSelf") {
                error.report();
            }
        }
    }
    let token = shell_token(TokenImpersonation)?;
    check_bool(
        unsafe { SetThreadToken(std::ptr::null(), token.0) },
        "SetThreadToken(desktop user)",
    )?;
    // Reverts even if `f` panics, before `token` closes.
    let _revert = Revert;
    Ok(f())
}

/// Start `command_line` with the shell's token, so it does not inherit SWM's elevation.
fn create_as_explorer_user(command_line: String) -> PlatformResult<()> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Security::TokenPrimary;
    use windows_sys::Win32::System::Threading::{
        CreateProcessWithTokenW, PROCESS_INFORMATION, STARTF_USESHOWWINDOW, STARTUPINFOW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let primary = shell_token(TokenPrimary)?;
    let mut startup: STARTUPINFOW = unsafe { zeroed() };
    startup.cb = size_of::<STARTUPINFOW>() as u32;
    startup.wShowWindow = SW_SHOWNORMAL as u16;
    startup.dwFlags = STARTF_USESHOWWINDOW;
    let mut info: PROCESS_INFORMATION = unsafe { zeroed() };
    let mut command: Vec<u16> = command_line.encode_utf16().chain([0]).collect();
    let result = unsafe {
        CreateProcessWithTokenW(
            primary.0,
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
    if result == 0 {
        return Err(PlatformError::last_win32("CreateProcessWithTokenW"));
    }
    drop(Handle(info.hThread, "CloseHandle(created thread)"));
    drop(Handle(info.hProcess, "CloseHandle(created process)"));
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::quote_argument;

    /// Whether the calling thread is impersonating (has a token of its own).
    fn thread_has_token() -> bool {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::Security::TOKEN_QUERY;
        use windows_sys::Win32::System::Threading::{GetCurrentThread, OpenThreadToken};
        let mut token = std::ptr::null_mut();
        let opened = unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut token) };
        if opened != 0 {
            unsafe { CloseHandle(token) };
        }
        opened != 0
    }

    #[test]
    fn desktop_user_closures_impersonate_and_then_revert() {
        assert!(!thread_has_token());
        let inside = super::as_desktop_user(thread_has_token).expect("desktop shell token");
        assert!(inside, "the closure runs with the shell's token");
        assert!(!thread_has_token(), "the thread reverts afterwards");
    }

    #[test]
    fn explorer_is_named_by_its_full_path() {
        let explorer = super::windows_directory_file("explorer.exe").unwrap();
        assert!(explorer.ends_with(r"\explorer.exe") && !explorer.contains(r"\\"));
        assert!(std::path::Path::new(&explorer).is_file());
    }

    #[test]
    fn arguments_survive_the_windows_command_line_parser() {
        assert_eq!(
            quote_argument(r"C:\src\my app\app.sln"),
            r#""C:\src\my app\app.sln""#
        );
        // A trailing backslash would otherwise escape the closing quote.
        assert_eq!(quote_argument(r"C:\src\"), r#""C:\src\\""#);
        assert_eq!(quote_argument(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(quote_argument(r#"a\"b"#), r#""a\\\"b""#);
    }
}
