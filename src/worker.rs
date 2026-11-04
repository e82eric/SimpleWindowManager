//! Commands cross this boundary; Runtime is accessed only by the worker.
use crate::{
    commands::Action,
    platform_error::{PlatformError, PlatformResult},
    runtime::Runtime,
};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    mpsc::{self, Receiver, SyncSender},
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub const WAKE: u32 = WM_APP + 73;
pub static GESTURE: AtomicU64 = AtomicU64::new(0);
static DIRTY: AtomicBool = AtomicBool::new(true);
static CANCEL_DRAG: AtomicBool = AtomicBool::new(false);
static CHANNEL: OnceLock<SyncSender<QueuedCommand>> = OnceLock::new();
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub struct QueuedCommand {
    sequence: u64,
    queued: std::time::Instant,
    command: Command,
}
static THREAD: AtomicU32 = AtomicU32::new(0);
static SNAPSHOT: Mutex<Option<Arc<Snapshot>>> = Mutex::new(None);
pub struct Snapshot {
    pub configuration: Arc<crate::config::Configuration>,
    pub bars: Vec<Option<crate::bar::BarSnapshot>>,
}
#[derive(Debug)]
pub enum Command {
    Action(Action),
    Workspace(usize, usize),
    Release { i: i32, j: i32, gesture: u64 },
    MenuClosed,
    Runtime(crate::picker::RuntimeCall),
}
pub fn send(command: Command) -> bool {
    let queued = QueuedCommand {
        sequence: SEQUENCE.fetch_add(1, Ordering::Relaxed),
        queued: std::time::Instant::now(),
        command,
    };
    if !CHANNEL.get().is_some_and(|s| s.try_send(queued).is_ok()) {
        return false;
    }
    wake();
    true
}
pub fn wake() {
    unsafe {
        PostThreadMessageW(THREAD.load(Ordering::Acquire), WAKE, 0, 0);
    }
}
pub fn release(x: i32, y: i32) {
    if !send(Command::Release {
        i: x,
        j: y,
        gesture: GESTURE.load(Ordering::Acquire),
    }) {
        CANCEL_DRAG.store(true, Ordering::Release);
    }
}
pub fn dirty() {
    DIRTY.store(true, Ordering::Release);
}
pub fn publish(runtime: &Runtime) {
    let snapshot = Arc::new(Snapshot {
        configuration: Arc::new(runtime.configuration.clone()),
        bars: (0..runtime.configuration.state.monitors.len())
            .map(|i| runtime.bar_snapshot(i))
            .collect(),
    });
    // All window calls and allocation finish before acquiring this small lock.
    let old = SNAPSHOT.lock().unwrap().replace(snapshot);
    drop(old);
    crate::ui::wake();
}
pub fn snapshot() -> Option<Arc<Snapshot>> {
    thread_local! {static CACHED:std::cell::RefCell<Option<Arc<Snapshot>>>=const {std::cell::RefCell::new(None)};}
    CACHED.with(|cached| {
        if let Ok(snapshot) = SNAPSHOT.try_lock() {
            *cached.borrow_mut() = snapshot.clone();
        }
        cached.borrow().clone()
    })
}
pub fn publish_if_dirty(runtime: &Runtime) {
    if DIRTY.swap(false, Ordering::AcqRel) {
        publish(runtime);
    }
}
pub fn drain(receiver: &Receiver<QueuedCommand>, runtime: &mut Runtime) {
    if CANCEL_DRAG.swap(false, Ordering::AcqRel) {
        runtime.cancel_input_drag();
    }
    for _ in 0..16 {
        let Ok(QueuedCommand {
            sequence,
            queued,
            command,
        }) = receiver.try_recv()
        else {
            break;
        };
        let span = tracing::info_span!("worker_command",sequence,queue_delay_us=queued.elapsed().as_micros() as u64,command=?command);
        let _entered = span.enter();
        let result = match command {
            Command::Action(action) => runtime.execute(action),
            Command::Workspace(m, w) => {
                runtime.bar_workspace_clicked(m, w);
                Ok(())
            }
            Command::Release { i, j, gesture } => runtime.input_release(i, j, gesture),
            Command::MenuClosed => {
                runtime.menu_closed();
                Ok(())
            }
            Command::Runtime(call) => {
                call.run(runtime);
                Ok(())
            }
        };
        if let Err(error) = result {
            error.report();
        }
        dirty();
    }
    publish_if_dirty(runtime);
}
pub struct Worker {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    pub fn start(runtime: Runtime) -> PlatformResult<Self> {
        let (sender, receiver) = mpsc::sync_channel(256);
        CHANNEL
            .set(sender)
            .map_err(|_| PlatformError::win32("worker already started", 183))?;
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let thread = std::thread::Builder::new()
            .name("swm-window-manager".into())
            .spawn(move || {
                let mut message = MSG::default();
                unsafe {
                    PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
                }
                THREAD.store(
                    unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() },
                    Ordering::Release,
                );
                if let Err(error) = crate::platform::run_worker(runtime, receiver, &worker_stop) {
                    error.report();
                    crate::ui::post(crate::ui::Command::Quit);
                }
            })
            .map_err(|e| {
                PlatformError::win32("spawn window manager", e.raw_os_error().unwrap_or(8) as u32)
            })?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        unsafe {
            PostThreadMessageW(THREAD.load(Ordering::Acquire), WAKE, 0, 0);
        }
        if let Some(thread) = self.thread.take() {
            // An application may still be blocking the worker. Do not freeze
            // the UI indefinitely while exiting the process.
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
            while !thread.is_finished() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            if thread.is_finished() {
                if thread.join().is_err() {
                    crate::error_log::report("window manager worker", "thread panicked");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn painting_reuses_last_snapshot_if_a_publication_is_in_progress() {
        let view = Arc::new(Snapshot {
            configuration: Arc::new(crate::config::Configuration::default()),
            bars: Vec::new(),
        });
        *SNAPSHOT.lock().unwrap() = Some(view.clone());
        assert!(Arc::ptr_eq(&snapshot().unwrap(), &view));
        let locked = SNAPSHOT.lock().unwrap();
        // This must neither wait for the producer nor return a blank view.
        assert!(Arc::ptr_eq(&snapshot().unwrap(), &view));
        drop(locked);
    }
}
