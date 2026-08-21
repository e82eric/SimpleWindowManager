use std::backtrace::Backtrace;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use serde_json::Value;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::{Layer, SubscriberExt};

const ERROR_LOG_CAPACITY: usize = 100;
const LOG_FILE_PREFIX: &str = "errors.log";
const LOG_FILE_COUNT: usize = 5;

#[derive(Clone, Debug, Default)]
pub struct LoggingFrame {
    pub method: String,
    pub fields: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct ErrorLogEntry {
    pub timestamp: SystemTime,
    pub context: String,
    pub code: Option<u32>,
    pub message: String,
    pub entry_method: String,
    pub current_method: String,
    pub frames: Vec<LoggingFrame>,
    pub file: String,
    pub line: Option<u32>,
}

static LOG_DIRECTORY: OnceLock<Option<PathBuf>> = OnceLock::new();
static LOG_WRITER: OnceLock<tracing_appender::non_blocking::NonBlocking> = OnceLock::new();
static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

fn configured_log_directory() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
            .join("SimpleWindowManager")
            .join("logs"),
    )
}

pub fn log_file_path() -> Option<PathBuf> {
    log_directory().map(|directory| directory.join(LOG_FILE_PREFIX))
}

fn log_directory() -> Option<PathBuf> {
    LOG_DIRECTORY.get_or_init(configured_log_directory).clone()
}

fn install_file_subscriber(directory: &Path) -> Result<(), String> {
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix(LOG_FILE_PREFIX)
        .max_log_files(LOG_FILE_COUNT)
        .build(directory)
        .map_err(|error| error.to_string())?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let _ = LOG_WRITER.set(writer.clone());
    let _ = LOG_GUARD.set(guard);
    let layer = tracing_subscriber::fmt::layer()
        .json()
        .with_ansi(false)
        .without_time()
        .with_current_span(true)
        .with_span_list(true)
        .with_writer(writer)
        .with_filter(filter_fn(|metadata| {
            metadata.is_span() || metadata.target() == "swm::error"
        }));
    tracing::subscriber::set_global_default(tracing_subscriber::registry().with(layer))
        .map_err(|error| error.to_string())
}

pub fn initialize() {
    let result = if cfg!(test) {
        let layer = tracing_subscriber::fmt::layer()
            .json()
            .with_writer(std::io::sink)
            .with_filter(filter_fn(|metadata| {
                metadata.is_span() || metadata.target() == "swm::error"
            }));
        tracing::subscriber::set_global_default(tracing_subscriber::registry().with(layer))
            .map_err(|error| error.to_string())
    } else if let Some(directory) = log_directory() {
        install_file_subscriber(&directory)
    } else {
        Err("LOCALAPPDATA is not set".into())
    };
    if let Err(error) = result {
        eprintln!("unable to install SWM tracing subscriber: {error}");
    }
}

fn timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[track_caller]
pub fn report(context: impl Into<String>, message: impl Into<String>) {
    let caller = std::panic::Location::caller();
    let operation = context.into();
    let error_message = message.into();
    let backtrace = Backtrace::force_capture();
    eprintln!("{operation}: {error_message}");
    tracing::error!(
        target: "swm::error",
        operation = %operation,
        error_message = %error_message,
        backtrace = %backtrace,
        timestamp_ms = timestamp_millis(),
        caller_file = caller.file(),
        caller_line = caller.line(),
    );
}

#[track_caller]
pub fn report_win32(context: impl Into<String>, code: u32) {
    let caller = std::panic::Location::caller();
    let operation = context.into();
    let error_message = std::io::Error::from_raw_os_error(code as i32).to_string();
    let backtrace = Backtrace::force_capture();
    eprintln!("{operation}: error {code}: {error_message}");
    tracing::error!(
        target: "swm::error",
        operation = %operation,
        code,
        error_message = %error_message,
        backtrace = %backtrace,
        timestamp_ms = timestamp_millis(),
        caller_file = caller.file(),
        caller_line = caller.line(),
    );
}

#[track_caller]
pub fn last_win32(context: impl Into<String>) -> u32 {
    let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    report_win32(context, code);
    code
}

#[track_caller]
pub fn report_hresult(context: impl Into<String>, hresult: i32) {
    let caller = std::panic::Location::caller();
    let operation = context.into();
    let code = hresult as u32;
    let error_message =
        windows::core::Error::from_hresult(windows::core::HRESULT(hresult)).to_string();
    let backtrace = Backtrace::force_capture();
    eprintln!("{operation}: HRESULT {code:#010x}: {error_message}");
    tracing::error!(
        target: "swm::error",
        operation = %operation,
        error_kind = "hresult",
        code,
        error_message = %error_message,
        backtrace = %backtrace,
        timestamp_ms = timestamp_millis(),
        caller_file = caller.file(),
        caller_line = caller.line(),
    );
}

#[track_caller]
pub fn check_hresult(context: impl Into<String>, hresult: i32) -> bool {
    if hresult < 0 {
        report_hresult(context, hresult);
        false
    } else {
        true
    }
}

fn display_json_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

fn parse_frame(value: &Value) -> Option<LoggingFrame> {
    let object = value.as_object()?;
    Some(LoggingFrame {
        method: object.get("name")?.as_str()?.to_owned(),
        fields: object
            .iter()
            .filter(|(name, _)| name.as_str() != "name")
            .map(|(name, value)| (name.clone(), display_json_value(value)))
            .collect(),
    })
}

fn parse_entry(line: &str) -> Option<ErrorLogEntry> {
    let value: Value = serde_json::from_str(line).ok()?;
    if value.get("target")?.as_str()? != "swm::error" {
        return None;
    }
    let fields = value.get("fields")?.as_object()?;
    let frames: Vec<LoggingFrame> = value
        .get("spans")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parse_frame)
        .collect();
    let timestamp = fields
        .get("timestamp_ms")?
        .as_u64()
        .map(|milliseconds| SystemTime::UNIX_EPOCH + Duration::from_millis(milliseconds))?;
    Some(ErrorLogEntry {
        timestamp,
        context: fields.get("operation").map(display_json_value)?,
        code: fields
            .get("code")
            .and_then(Value::as_u64)
            .map(|code| code as u32),
        message: fields.get("error_message").map(display_json_value)?,
        entry_method: frames
            .first()
            .map(|frame| frame.method.clone())
            .unwrap_or_default(),
        current_method: frames
            .last()
            .map(|frame| frame.method.clone())
            .unwrap_or_default(),
        frames,
        file: fields
            .get("caller_file")
            .map(display_json_value)
            .unwrap_or_default(),
        line: fields
            .get("caller_line")
            .and_then(Value::as_u64)
            .map(|line| line as u32),
    })
}

fn rolling_log_files(directory: &Path) -> Vec<PathBuf> {
    let Ok(files) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = files
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(LOG_FILE_PREFIX))
        })
        .collect();
    files.sort();
    files
}

pub fn entries() -> Vec<ErrorLogEntry> {
    if let Some(writer) = LOG_WRITER.get() {
        let _ = writer.clone().flush();
    }
    let Some(directory) = log_directory() else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for path in rolling_log_files(&directory) {
        let Ok(contents) = fs::read_to_string(path) else {
            continue;
        };
        entries.extend(contents.lines().filter_map(parse_entry));
    }
    if entries.len() > ERROR_LOG_CAPACITY {
        entries.drain(..entries.len() - ERROR_LOG_CAPACITY);
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct TestWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for TestWriter {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn parses_structured_error_and_span_context() {
        let entry = parse_entry(
            r#"{"level":"ERROR","fields":{"operation":"GetParent failed","code":1400,"error_message":"Invalid window handle.","timestamp_ms":1000,"caller_file":"src/platform.rs","caller_line":42},"target":"swm::error","spans":[{"event":"test","name":"outer_failure"},{"hwnd":4660,"name":"instrumented_failure"}]}"#,
        )
        .unwrap();
        assert_eq!(
            entry.timestamp,
            SystemTime::UNIX_EPOCH + Duration::from_secs(1)
        );
        assert_eq!(entry.context, "GetParent failed");
        assert_eq!(entry.code, Some(1400));
        assert_eq!(entry.entry_method, "outer_failure");
        assert_eq!(entry.current_method, "instrumented_failure");
        assert_eq!(entry.frames[0].fields[0], ("event".into(), "test".into()));
        assert_eq!(entry.file, "src/platform.rs");
        assert_eq!(entry.line, Some(42));
    }

    #[test]
    fn ignores_non_error_targets_and_partial_lines() {
        assert!(parse_entry(r#"{"target":"another-target"}"#).is_none());
        assert!(parse_entry("{\"target\":").is_none());
    }

    #[test]
    fn parses_the_json_schema_emitted_by_tracing_subscriber() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let layer = tracing_subscriber::fmt::layer()
            .json()
            .without_time()
            .with_current_span(true)
            .with_span_list(true)
            .with_writer({
                let output = output.clone();
                move || TestWriter(output.clone())
            });
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            let outer = tracing::info_span!("outer", event = "test");
            let _outer = outer.enter();
            let inner = tracing::info_span!("inner", hwnd = 4660_u64);
            let _inner = inner.enter();
            tracing::error!(
                target: "swm::error",
                operation = "GetParent failed",
                code = 1400_u32,
                error_message = "Invalid window handle.",
                timestamp_ms = 1000_u64,
                caller_file = "src/platform.rs",
                caller_line = 42_u32,
            );
        });
        let json = String::from_utf8(output.lock().unwrap().clone()).unwrap();
        let entry = parse_entry(json.lines().next().unwrap()).unwrap();
        assert_eq!(entry.entry_method, "outer");
        assert_eq!(entry.current_method, "inner");
        assert_eq!(entry.frames[0].fields[0], ("event".into(), "test".into()));
        assert_eq!(entry.frames[1].fields[0], ("hwnd".into(), "4660".into()));
    }
}
