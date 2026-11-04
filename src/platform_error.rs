use std::{backtrace::Backtrace, fmt};

#[derive(Debug)]
enum PlatformErrorKind {
    Win32 {
        operation: &'static str,
        code: u32,
    },
    Hresult {
        operation: &'static str,
        hr: i32,
    },
    NullOutput {
        operation: &'static str,
    },
    ApiCode {
        operation: &'static str,
        code: isize,
    },
}

#[derive(Debug)]
pub struct PlatformError {
    kind: PlatformErrorKind,
    backtrace: Backtrace,
}

pub type PlatformResult<T> = Result<T, PlatformError>;

impl PlatformError {
    #[track_caller]
    pub fn last_win32(operation: &'static str) -> Self {
        let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        Self {
            kind: PlatformErrorKind::Win32 { operation, code },
            backtrace: Backtrace::force_capture(),
        }
    }

    pub fn win32(operation: &'static str, code: u32) -> Self {
        Self {
            kind: PlatformErrorKind::Win32 { operation, code },
            backtrace: Backtrace::force_capture(),
        }
    }

    pub fn hresult(operation: &'static str, hr: i32) -> Self {
        Self {
            kind: PlatformErrorKind::Hresult { operation, hr },
            backtrace: Backtrace::force_capture(),
        }
    }

    pub fn null_output(operation: &'static str) -> Self {
        Self {
            kind: PlatformErrorKind::NullOutput { operation },
            backtrace: Backtrace::force_capture(),
        }
    }

    pub fn api_code(operation: &'static str, code: isize) -> Self {
        Self {
            kind: PlatformErrorKind::ApiCode { operation, code },
            backtrace: Backtrace::force_capture(),
        }
    }

    pub fn is_win32(&self, expected_code: u32) -> bool {
        matches!(
            self.kind,
            PlatformErrorKind::Win32 { code, .. } if code == expected_code
        )
    }

    #[track_caller]
    pub fn report(self) {
        match self.kind {
            PlatformErrorKind::Win32 { operation, code } => {
                crate::error_log::report_win32_with_backtrace(operation, code, &self.backtrace)
            }
            PlatformErrorKind::Hresult { operation, hr } => {
                crate::error_log::report_hresult_with_backtrace(operation, hr, &self.backtrace)
            }
            PlatformErrorKind::NullOutput { operation } => crate::error_log::report_with_backtrace(
                operation,
                "API returned a null output after reporting success",
                &self.backtrace,
            ),
            PlatformErrorKind::ApiCode { operation, code } => {
                crate::error_log::report_with_backtrace(
                    operation,
                    format!("API returned failure code {code}"),
                    &self.backtrace,
                )
            }
        }
    }
}

impl fmt::Display for PlatformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            PlatformErrorKind::Win32 { operation, code } => {
                write!(formatter, "{operation}: Win32 error {code}")
            }
            PlatformErrorKind::Hresult { operation, hr } => {
                write!(formatter, "{operation}: HRESULT {:#010x}", hr as u32)
            }
            PlatformErrorKind::NullOutput { operation } => {
                write!(
                    formatter,
                    "{operation}: API returned a null output after success"
                )
            }
            PlatformErrorKind::ApiCode { operation, code } => {
                write!(formatter, "{operation}: API failure code {code}")
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
        assert!(check_hresult(-1, "operation").is_err());
    }
}
