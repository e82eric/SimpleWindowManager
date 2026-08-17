use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::sync::{Mutex, Once, OnceLock};
use std::time::SystemTime;

use tracing::field::{Field, Visit};
use tracing::{Id, Subscriber};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

const ERROR_LOG_CAPACITY: usize = 100;
const LOG_FILE_NAME: &str = "errors.log";
const LOG_FILE_MAX_BYTES: u64 = 1024 * 1024;
const LOG_FILE_BACKUPS: usize = 5;
const LOG_QUEUE_CAPACITY: usize = 256;

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

static ERROR_LOG: OnceLock<Mutex<VecDeque<ErrorLogEntry>>> = OnceLock::new();
static LOG_FILE: OnceLock<Option<PathBuf>> = OnceLock::new();
static LOG_SENDER: OnceLock<Option<SyncSender<ErrorLogEntry>>> = OnceLock::new();
static INITIALIZE: Once = Once::new();

thread_local! {
    static ACTIVE_FRAMES: RefCell<Vec<LoggingFrame>> = const { RefCell::new(Vec::new()) };
}

fn buffer() -> &'static Mutex<VecDeque<ErrorLogEntry>> {
    ERROR_LOG.get_or_init(|| Mutex::new(VecDeque::with_capacity(ERROR_LOG_CAPACITY)))
}

fn push(entry: ErrorLogEntry) {
    if !cfg!(test) {
        enqueue_log_file_entry(entry.clone());
    }
    // Error reporting must never block a Win32 callback or create another failure.
    let Ok(mut entries) = buffer().try_lock() else {
        return;
    };
    if entries.len() == ERROR_LOG_CAPACITY {
        entries.pop_front();
    }
    entries.push_back(entry);
}

fn configured_log_file() -> Option<PathBuf> {
    let root = std::env::var_os("LOCALAPPDATA")?;
    Some(
        PathBuf::from(root)
            .join("SimpleWindowManager")
            .join("logs")
            .join(LOG_FILE_NAME),
    )
}

pub fn log_file_path() -> Option<PathBuf> {
    LOG_FILE.get_or_init(configured_log_file).clone()
}

fn rotate_log_file(path: &Path) -> std::io::Result<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(LOG_FILE_NAME);
    for index in (1..=LOG_FILE_BACKUPS).rev() {
        let destination = path.with_file_name(format!("{file_name}.{index}"));
        let source = if index == 1 {
            path.to_path_buf()
        } else {
            path.with_file_name(format!("{file_name}.{}", index - 1))
        };
        if source.exists() {
            if destination.exists() {
                fs::remove_file(&destination)?;
            }
            fs::rename(source, destination)?;
        }
    }
    Ok(())
}

fn sanitize_log_field(value: &str) -> String {
    value.replace(['\r', '\n', '\t'], " ")
}

#[derive(Default)]
struct FieldVisitor {
    fields: Vec<(String, String)>,
}

impl FieldVisitor {
    fn insert(&mut self, field: &Field, value: String) {
        if let Some(existing) = self.fields.iter_mut().find(|item| item.0 == field.name()) {
            existing.1 = value;
        } else {
            self.fields.push((field.name().to_owned(), value));
        }
    }
}

impl Visit for FieldVisitor {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.insert(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.insert(field, value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.insert(field, value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.insert(field, value.to_owned());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.insert(field, format!("{value:?}"));
    }
}

#[derive(Default)]
struct LoggingContextLayer;

impl<S> Layer<S> for LoggingContextLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        attributes: &tracing::span::Attributes<'_>,
        id: &Id,
        context: Context<'_, S>,
    ) {
        let Some(span) = context.span(id) else {
            return;
        };
        let mut visitor = FieldVisitor::default();
        attributes.record(&mut visitor);
        span.extensions_mut().insert(LoggingFrame {
            method: attributes.metadata().name().to_owned(),
            fields: visitor.fields,
        });
    }

    fn on_record(&self, id: &Id, values: &tracing::span::Record<'_>, context: Context<'_, S>) {
        let Some(span) = context.span(id) else {
            return;
        };
        let mut extensions = span.extensions_mut();
        let Some(frame) = extensions.get_mut::<LoggingFrame>() else {
            return;
        };
        let mut visitor = FieldVisitor {
            fields: std::mem::take(&mut frame.fields),
        };
        values.record(&mut visitor);
        frame.fields = visitor.fields;
    }

    fn on_enter(&self, id: &Id, context: Context<'_, S>) {
        let Some(span) = context.span(id) else {
            return;
        };
        let frame = span
            .extensions()
            .get::<LoggingFrame>()
            .cloned()
            .unwrap_or_else(|| LoggingFrame {
                method: span.metadata().name().to_owned(),
                fields: Vec::new(),
            });
        ACTIVE_FRAMES.with_borrow_mut(|frames| frames.push(frame));
    }

    fn on_exit(&self, _id: &Id, _context: Context<'_, S>) {
        ACTIVE_FRAMES.with_borrow_mut(|frames| {
            frames.pop();
        });
    }
}

pub fn initialize() {
    INITIALIZE.call_once(|| {
        if !cfg!(test) {
            let _ = log_sender();
        }
        let subscriber = tracing_subscriber::registry().with(LoggingContextLayer);
        if let Err(error) = tracing::subscriber::set_global_default(subscriber) {
            eprintln!("unable to install SWM tracing subscriber: {error}");
        }
    });
}

fn format_log_line(entry: &ErrorLogEntry) -> String {
    let timestamp = entry
        .timestamp
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let properties = entry
        .frames
        .iter()
        .flat_map(|frame| {
            frame.fields.iter().map(move |(name, value)| {
                format!("{}.{}={}", frame.method, name, sanitize_log_field(value))
            })
        })
        .collect::<Vec<_>>()
        .join(" | ");
    let source = entry.line.map_or_else(
        || entry.file.clone(),
        |line| format!("{}:{line}", entry.file),
    );
    format!(
        "{timestamp}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        sanitize_log_field(&entry.entry_method),
        sanitize_log_field(&entry.current_method),
        sanitize_log_field(&entry.context),
        entry.code.map(|code| code.to_string()).unwrap_or_default(),
        sanitize_log_field(&entry.message),
        properties,
        sanitize_log_field(&source),
        std::process::id(),
    )
}

fn open_log_file(path: &Path) -> std::io::Result<(File, u64)> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let length = file.metadata()?.len();
    Ok((file, length))
}

fn log_sender() -> Option<&'static SyncSender<ErrorLogEntry>> {
    LOG_SENDER
        .get_or_init(|| {
            let path = log_file_path()?;
            let (sender, receiver) = sync_channel::<ErrorLogEntry>(LOG_QUEUE_CAPACITY);
            std::thread::Builder::new()
                .name("swm-log-writer".into())
                .spawn(move || {
                    let Some(parent) = path.parent() else {
                        return;
                    };
                    if fs::create_dir_all(parent).is_err() {
                        return;
                    }
                    let mut output = open_log_file(&path).ok();
                    while let Ok(entry) = receiver.recv() {
                        let line = format_log_line(&entry);
                        let line_bytes = line.len() as u64 + 1;
                        if output
                            .as_ref()
                            .is_some_and(|(_, length)| *length + line_bytes > LOG_FILE_MAX_BYTES)
                        {
                            output = None;
                            if rotate_log_file(&path).is_err() {
                                continue;
                            }
                        }
                        if output.is_none() {
                            output = open_log_file(&path).ok();
                        }
                        if let Some((file, length)) = output.as_mut()
                            && writeln!(file, "{line}").is_ok()
                        {
                            *length += line_bytes;
                        }
                    }
                })
                .ok()?;
            Some(sender)
        })
        .as_ref()
}

fn enqueue_log_file_entry(entry: ErrorLogEntry) {
    let Some(sender) = log_sender() else {
        return;
    };
    match sender.try_send(entry) {
        Ok(()) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
    }
}

fn push_error(
    operation: String,
    code: Option<u32>,
    message: String,
    caller: &'static std::panic::Location<'static>,
) {
    let frames = ACTIVE_FRAMES.with_borrow(Clone::clone);
    let entry_method = frames
        .first()
        .map(|frame| frame.method.clone())
        .unwrap_or_default();
    let current_method = frames
        .last()
        .map(|frame| frame.method.clone())
        .unwrap_or_default();
    if let Some(code) = code {
        eprintln!("{operation}: error {code}: {message}");
    } else {
        eprintln!("{operation}: {message}");
    }
    push(ErrorLogEntry {
        timestamp: SystemTime::now(),
        context: operation,
        code,
        message,
        entry_method,
        current_method,
        frames,
        file: caller.file().to_owned(),
        line: Some(caller.line()),
    });
}

#[track_caller]
pub fn report(context: impl Into<String>, message: impl Into<String>) {
    initialize();
    let caller = std::panic::Location::caller();
    let operation = context.into();
    let message = message.into();
    push_error(operation.clone(), None, message.clone(), caller);
    tracing::error!(target: "swm::error", operation = %operation, message = %message);
}

#[track_caller]
pub fn report_win32(context: impl Into<String>, code: u32) {
    initialize();
    let caller = std::panic::Location::caller();
    let operation = context.into();
    let message = std::io::Error::from_raw_os_error(code as i32).to_string();
    push_error(operation.clone(), Some(code), message.clone(), caller);
    tracing::error!(target: "swm::error", operation = %operation, code, message = %message);
}

#[track_caller]
pub fn last_win32(context: impl Into<String>) -> u32 {
    let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    report_win32(context, code);
    code
}

#[track_caller]
pub fn report_hresult(context: impl Into<String>, hresult: i32) {
    report_win32(context, hresult as u32);
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

pub fn entries() -> Vec<ErrorLogEntry> {
    buffer()
        .try_lock()
        .map(|entries| entries.iter().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn clear() {
        if let Ok(mut entries) = buffer().lock() {
            entries.clear();
        }
    }

    #[test]
    fn circular_buffer_keeps_the_newest_entries() {
        let _test = TEST_LOCK.lock().unwrap();
        clear();
        for index in 0..ERROR_LOG_CAPACITY + 3 {
            push(ErrorLogEntry {
                timestamp: SystemTime::UNIX_EPOCH,
                context: format!("context-{index}"),
                code: Some(index as u32),
                message: String::new(),
                entry_method: String::new(),
                current_method: String::new(),
                frames: Vec::new(),
                file: String::new(),
                line: None,
            });
        }
        let entries = entries();
        assert_eq!(entries.len(), ERROR_LOG_CAPACITY);
        assert_eq!(entries[0].code, Some(3));
        assert_eq!(entries.last().unwrap().code, Some(102));
    }

    #[test]
    fn report_win32_directly_captures_the_error_and_source() {
        let _test = TEST_LOCK.lock().unwrap();
        clear();
        report_win32("GetParent failed", 1400);
        let entries = entries();
        let entry = entries.last().unwrap();
        assert_eq!(entry.context, "GetParent failed");
        assert_eq!(entry.code, Some(1400));
        assert!(entry.message.contains("1400"));
        assert!(entry.file.ends_with("error_log.rs"));
        assert!(entry.line.is_some());
    }

    #[tracing::instrument(fields(hwnd = 0x1234_u64))]
    fn instrumented_failure() {
        report_win32("GetParent failed", 1400);
    }

    #[tracing::instrument(fields(event = "test"))]
    fn outer_failure() {
        instrumented_failure();
    }

    #[test]
    fn direct_capture_includes_active_tracing_context() {
        let _test = TEST_LOCK.lock().unwrap();
        clear();
        initialize();
        outer_failure();
        let entries = entries();
        let entry = entries.last().unwrap();
        assert_eq!(entry.entry_method, "outer_failure");
        assert_eq!(entry.current_method, "instrumented_failure");
        assert_eq!(entry.frames[0].fields[0], ("event".into(), "test".into()));
        assert_eq!(entry.frames[1].fields[0], ("hwnd".into(), "4660".into()));
    }
}
