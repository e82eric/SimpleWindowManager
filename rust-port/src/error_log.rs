use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

const ERROR_LOG_CAPACITY: usize = 100;

#[derive(Clone, Debug)]
pub struct ErrorLogEntry {
    pub timestamp: SystemTime,
    pub context: String,
    pub code: Option<u32>,
    pub message: String,
}

static ERROR_LOG: OnceLock<Mutex<VecDeque<ErrorLogEntry>>> = OnceLock::new();

fn buffer() -> &'static Mutex<VecDeque<ErrorLogEntry>> {
    ERROR_LOG.get_or_init(|| Mutex::new(VecDeque::with_capacity(ERROR_LOG_CAPACITY)))
}

fn push(entry: ErrorLogEntry) {
    // Error reporting must never block a Win32 callback or create another failure.
    let Ok(mut entries) = buffer().try_lock() else {
        return;
    };
    if entries.len() == ERROR_LOG_CAPACITY {
        entries.pop_front();
    }
    entries.push_back(entry);
}

pub fn report(context: impl Into<String>, message: impl Into<String>) {
    let context = context.into();
    let message = message.into();
    eprintln!("{context}: {message}");
    push(ErrorLogEntry {
        timestamp: SystemTime::now(),
        context,
        code: None,
        message,
    });
}

pub fn report_win32(context: impl Into<String>, code: u32) {
    let context = context.into();
    let message = std::io::Error::from_raw_os_error(code as i32).to_string();
    eprintln!("{context}: error {code}: {message}");
    push(ErrorLogEntry {
        timestamp: SystemTime::now(),
        context,
        code: Some(code),
        message,
    });
}

pub fn entries() -> Vec<ErrorLogEntry> {
    buffer()
        .try_lock()
        .map(|entries| entries.iter().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circular_buffer_keeps_the_newest_entries() {
        if let Ok(mut entries) = buffer().lock() {
            entries.clear();
        }
        for index in 0..ERROR_LOG_CAPACITY + 3 {
            push(ErrorLogEntry {
                timestamp: SystemTime::UNIX_EPOCH,
                context: format!("context-{index}"),
                code: Some(index as u32),
                message: String::new(),
            });
        }
        let entries = entries();
        assert_eq!(entries.len(), ERROR_LOG_CAPACITY);
        assert_eq!(entries[0].code, Some(3));
        assert_eq!(entries.last().unwrap().code, Some(102));
    }
}
