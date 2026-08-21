use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformError {
    Win32 { operation: &'static str, code: u32 },
    Hresult { operation: &'static str, hr: i32 },
}

pub type PlatformResult<T> = Result<T, PlatformError>;

impl PlatformError {
    #[track_caller]
    pub fn last_win32(operation: &'static str) -> Self {
        Self::Win32 {
            operation,
            code: unsafe { windows_sys::Win32::Foundation::GetLastError() },
        }
    }

    pub const fn win32(operation: &'static str, code: u32) -> Self {
        Self::Win32 { operation, code }
    }

    pub const fn hresult(operation: &'static str, hr: i32) -> Self {
        Self::Hresult { operation, hr }
    }

    #[track_caller]
    pub fn report(self) {
        match self {
            Self::Win32 { operation, code } => crate::error_log::report_win32(operation, code),
            Self::Hresult { operation, hr } => crate::error_log::report_hresult(operation, hr),
        }
    }
}

impl fmt::Display for PlatformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Win32 { operation, code } => write!(formatter, "{operation}: Win32 error {code}"),
            Self::Hresult { operation, hr } => {
                write!(formatter, "{operation}: HRESULT {:#010x}", *hr as u32)
            }
        }
    }
}

impl std::error::Error for PlatformError {}

pub fn check_bool(result: i32, operation: &'static str) -> PlatformResult<()> {
    if result == 0 {
        Err(PlatformError::last_win32(operation))
    } else {
        Ok(())
    }
}

pub fn check_hresult(hr: i32, operation: &'static str) -> PlatformResult<()> {
    if hr < 0 {
        Err(PlatformError::hresult(operation, hr))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hresult_success_includes_nonzero_success_codes() {
        assert!(check_hresult(0, "operation").is_ok());
        assert!(check_hresult(1, "operation").is_ok());
        assert_eq!(
            check_hresult(-1, "operation"),
            Err(PlatformError::hresult("operation", -1))
        );
    }
}
