//! Main-thread presentation state. The configuration is a read-only worker snapshot.
use crate::{
    commands::{Action, KeyBinding},
    config::Configuration,
    platform_error::PlatformResult,
};
use std::{
    cell::RefCell,
    sync::{
        OnceLock,
        atomic::{AtomicU32, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;
const WAKE: u32 = WM_APP + 74;
static THREAD: AtomicU32 = AtomicU32::new(0);
static CHANNEL: OnceLock<SyncSender<Command>> = OnceLock::new();
static BINDINGS: OnceLock<Vec<KeyBinding>> = OnceLock::new();
static BAR_HANDLES: OnceLock<Vec<(usize, isize)>> = OnceLock::new();
/// Repaint the bars and the open mailbox drop-down after a producer published.
/// May be called from any thread, as InvalidateRect may.
pub(crate) fn mailbox_changed() {
    for &(_, hwnd) in BAR_HANDLES.get().into_iter().flatten() {
        unsafe {
            windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd as _, std::ptr::null(), 0);
        }
    }
    crate::mailbox_popup::invalidate();
}
pub fn bar_handle(monitor: usize) -> Option<isize> {
    BAR_HANDLES
        .get()?
        .iter()
        .find(|(id, _)| *id == monitor)
        .map(|(_, hwnd)| *hwnd)
}
pub struct KeyState {
    suppressed: [bool; 256],
}
impl Default for KeyState {
    fn default() -> Self {
        Self {
            suppressed: [false; 256],
        }
    }
}
impl KeyState {
    pub fn event(&mut self, key: u32, up: bool, submit: impl FnOnce() -> bool) -> bool {
        let Some(suppressed) = self.suppressed.get_mut(key as usize) else {
            return false;
        };
        if up {
            return std::mem::replace(suppressed, false);
        }
        if submit() {
            *suppressed = true;
        }
        *suppressed
    }
}
/// Session of the visible picker, or 0 while hidden. The UI opens sessions; the
/// picker host closes them, before running what the picker accepted.
static VISIBLE_MENU: AtomicU64 = AtomicU64::new(0);
pub fn menu_visible() -> bool {
    VISIBLE_MENU.load(Ordering::Acquire) != 0
}
pub(crate) fn menu_is_visible(session: u64) -> bool {
    VISIBLE_MENU.load(Ordering::Acquire) == session
}
/// Close `session`; false if a newer picker has replaced it.
pub(crate) fn close_menu(session: u64) -> bool {
    VISIBLE_MENU
        .compare_exchange(session, 0, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}
/// Run an action from any thread: pickers and quit on the UI thread, the rest on
/// the worker.
pub fn dispatch(action: Action) -> bool {
    if is_menu(action) || matches!(action, Action::Quit) {
        post(Command::Action(action))
    } else {
        crate::worker::send(crate::worker::Command::Action(action))
    }
}
thread_local! { static UI:RefCell<Option<Ui>>=const { RefCell::new(None) }; }
pub enum Command {
    Action(Action),
    /// A picker built elsewhere, such as by a mailbox producer.
    ShowRequest(Box<dyn crate::picker::PickerRequest>),
    Focus,
    Hide,
    Quit,
}
pub fn wake() {
    unsafe {
        PostThreadMessageW(THREAD.load(Ordering::Acquire), WAKE, 0, 0);
    }
}
pub fn post(command: Command) -> bool {
    let sent = CHANNEL.get().is_some_and(|s| s.try_send(command).is_ok());
    if sent {
        wake();
    }
    sent
}
pub fn binding(key: u32, modifiers: i32) -> Option<Action> {
    BINDINGS
        .get()?
        .iter()
        .find(|b| b.key == key && b.modifiers == modifiers)
        .map(|b| b.action)
}
pub fn is_menu(action: Action) -> bool {
    matches!(action, Action::ShowPicker(_) | Action::ShowMailbox(_))
}
fn picker_bounds(configuration: &Configuration) -> Option<crate::egui_window_menu::ScreenBounds> {
    let monitor = configuration.selected_picker_monitor()?;
    Some(crate::egui_window_menu::ScreenBounds {
        left: monitor.x_offset,
        top: monitor.top,
        width: monitor.width,
        height: monitor.bottom - monitor.top,
    })
}

pub struct Ui {
    pub(crate) menu_session: u64,
    pub configuration: std::sync::Arc<Configuration>,
    egui_windows: Option<crate::egui_window_menu::WindowMenuHost>,
    bars: Vec<crate::bar::BarWindow>,
    snapshot: Option<std::sync::Arc<crate::worker::Snapshot>>,
}
impl Ui {
    pub fn execute(&mut self, action: Action) -> PlatformResult<()> {
        use Action::*;
        match action {
            ShowPicker(picker) => self.show_picker(picker),
            ShowMailbox(name) => self.show_mailbox(name),
            Quit => unsafe {
                PostQuitMessage(0);
            },
            _ => {
                crate::worker::send(crate::worker::Command::Action(action));
            }
        }
        Ok(())
    }
    fn show_picker(&mut self, picker: &crate::picker::PickerConfig) {
        let request = picker.source.request(&self.configuration);
        self.show_request(request);
    }
    fn show_request(&mut self, request: Box<dyn crate::picker::PickerRequest>) {
        self.menu_session = self.menu_session.wrapping_add(1).max(1);
        // Visible before the host can close it.
        VISIBLE_MENU.store(self.menu_session, Ordering::Release);
        if !self.send_request(request) {
            // A menu without an available source must not leave a closed picker visible.
            self.hide();
        }
    }
    fn show_mailbox(&self, name: &str) {
        let configuration = &self.configuration;
        let Some(index) = configuration
            .bar_mailboxes
            .iter()
            .position(|mailbox| mailbox.name == name)
        else {
            crate::error_log::report("show mailbox", format!("no bar mailbox named {name}"));
            return;
        };
        let monitors = &configuration.state.monitors;
        let Some(monitor) = configuration
            .selected_picker_monitor()
            .and_then(|selected| monitors.iter().position(|m| std::ptr::eq(m, selected)))
        else {
            return;
        };
        if let Some(bar) = self.bars.iter().find(|bar| bar.monitor_index == monitor) {
            crate::bar::toggle_mailbox(bar.hwnd as _, monitor, index);
        }
    }
    fn send_request(&self, request: Box<dyn crate::picker::PickerRequest>) -> bool {
        let Some(host) = self.egui_windows.as_ref() else {
            return false;
        };
        let Some(bounds) = picker_bounds(&self.configuration) else {
            return false;
        };
        match host.show_request(self.menu_session, bounds, request) {
            Ok(()) => true,
            Err(error) => {
                crate::error_log::report("picker failed", error);
                false
            }
        }
    }
    fn hide(&mut self) {
        if let Some(menu) = &self.egui_windows {
            menu.hide();
        }
        self.menu_session = self.menu_session.wrapping_add(1).max(1);
        VISIBLE_MENU.store(0, Ordering::Release);
        crate::worker::send(crate::worker::Command::MenuClosed);
    }
}
fn start_egui_window_host(
    configuration: &Configuration,
) -> Result<crate::egui_window_menu::WindowMenuHost, String> {
    use crate::egui_window_menu::WindowMenuHost;
    WindowMenuHost::start_with_preview_resolver(
        configuration.picker_result_rows,
        configuration.picker_preview_rows,
        configuration.file_preview_resolver.clone(),
        crate::picker::complete,
    )
}

pub fn initialize(
    configuration: Configuration,
    bindings: Vec<KeyBinding>,
) -> PlatformResult<Receiver<Command>> {
    let mut message = MSG::default();
    unsafe {
        PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
    }
    THREAD.store(
        unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() },
        Ordering::Release,
    );
    let (sender, receiver) = mpsc::sync_channel(256);
    let _ = CHANNEL.set(sender);
    let _ = BINDINGS.set(bindings);
    let bars = crate::bar::create_bars(&configuration.state.monitors, configuration.bar_height)?;
    let _ = BAR_HANDLES.set(
        bars.iter()
            .map(|bar| (bar.monitor_index, bar.hwnd))
            .collect(),
    );
    crate::mailbox::start_producers(&configuration.bar_mailboxes);
    // Prepare the native window and GL context before the first shortcut, like
    // other persistent UI resources. The tool window remains hidden until requested.
    let egui_windows = start_egui_window_host(&configuration)
        .map_err(|error| {
            crate::error_log::report("failed to initialize egui menu host", error);
        })
        .ok();
    UI.with(|slot| {
        let ui = Ui {
            menu_session: 0,
            configuration: std::sync::Arc::new(configuration),
            egui_windows,
            bars,
            snapshot: None,
        };
        *slot.borrow_mut() = Some(ui)
    });
    Ok(receiver)
}
pub fn pump(receiver: &Receiver<Command>) {
    UI.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(ui) = slot.as_mut() else {
            return;
        };
        if let Some(snapshot) = crate::worker::snapshot() {
            if ui
                .snapshot
                .as_ref()
                .is_none_or(|old| !std::sync::Arc::ptr_eq(old, &snapshot))
            {
                ui.configuration = snapshot.configuration.clone();
                ui.snapshot = Some(snapshot);
                crate::bar::redraw(&ui.bars);
            }
        }
        for _ in 0..16 {
            let Ok(command) = receiver.try_recv() else {
                break;
            };
            match command {
                Command::Action(action) => {
                    if let Err(error) = ui.execute(action) {
                        error.report();
                    }
                }
                Command::ShowRequest(request) => ui.show_request(request),
                Command::Focus => {
                    if menu_visible() {
                        if let Some(menu) = &ui.egui_windows {
                            menu.focus();
                        }
                    }
                }
                Command::Hide => {
                    ui.hide();
                }
                Command::Quit => unsafe {
                    PostQuitMessage(0);
                },
            }
        }
    });
}
pub fn bar_snapshot(monitor: usize) -> Option<crate::bar::BarSnapshot> {
    let snapshot = crate::worker::snapshot()?;
    let mut bar = snapshot.bars.get(monitor)?.clone()?;
    bar.segments = crate::bar::cached_segments(&snapshot.configuration.bar_segments);
    bar.mailboxes = snapshot
        .configuration
        .bar_mailboxes
        .iter()
        .map(|mailbox| (mailbox.clone(), mailbox.current()))
        .collect();
    Some(bar)
}
pub fn shutdown() {
    UI.with(|slot| {
        if let Some(ui) = slot.borrow_mut().take() {
            for bar in ui.bars {
                unsafe {
                    DestroyWindow(bar.hwnd as _);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saturated_worker_queue_preserves_key_pairs_and_ui_commands_have_their_own_queue() {
        let (worker_tx, worker_rx) = mpsc::sync_channel(1);
        let (ui_tx, ui_rx) = mpsc::sync_channel(1);
        let mut keys = KeyState::default();
        assert!(keys.event(65, false, || worker_tx.try_send(Action::SelectLeft).is_ok()));
        // Consumer is stalled. The repeat does not wait or leak to the app.
        assert!(keys.event(65, false, || worker_tx.try_send(Action::SelectLeft).is_ok()));
        assert!(keys.event(65, true, || panic!("key up must not submit work")));
        // A fresh press is passed through when its command cannot be queued.
        assert!(!keys.event(65, false, || worker_tx.try_send(Action::SelectLeft).is_ok()));
        assert!(!keys.event(65, true, || panic!("key up must not submit work")));
        assert!(keys.event(66, false, || {
            ui_tx
                .try_send(Action::ShowPicker(&crate::builtin_pickers::COMMANDS))
                .is_ok()
        }));
        assert!(matches!(
            ui_rx.try_recv().unwrap(),
            Action::ShowPicker(picker) if std::ptr::eq(picker, &crate::builtin_pickers::COMMANDS)
        ));
        assert!(matches!(worker_rx.try_recv().unwrap(), Action::SelectLeft));
        assert!(worker_rx.try_recv().is_err());
    }
}

#[cfg(test)]
mod picker_monitor_tests {
    use super::*;
    use crate::{Monitor, MonitorId, config::PickerMonitor};

    fn configuration() -> Configuration {
        let mut configuration = Configuration::default();
        configuration.state.monitors = vec![
            Monitor {
                id: 10,
                top: 29,
                bottom: 1080,
                x_offset: -1920,
                width: 1920,
                height: 1051,
                workspace: None,
                is_hidden: false,
                selected: false,
                gap_width: 0,
            },
            Monitor {
                id: 20,
                top: 50,
                bottom: 1440,
                x_offset: 0,
                width: 2560,
                height: 1390,
                workspace: None,
                is_hidden: false,
                selected: true,
                gap_width: 0,
            },
        ];
        configuration.state.selected_monitor = Some(MonitorId(1));
        configuration
    }
    fn bounds(configuration: &Configuration) -> Option<[i32; 4]> {
        picker_bounds(configuration)
            .map(|bounds| [bounds.left, bounds.top, bounds.width, bounds.height])
    }

    #[test]
    fn primary_monitor_setting_controls_default_picker_and_falls_back() {
        let mut configuration = configuration();
        configuration.primary_monitor = 1;
        assert_eq!(bounds(&configuration), Some([0, 50, 2560, 1390]));
        configuration.primary_monitor = 99;
        assert_eq!(bounds(&configuration), Some([-1920, 29, 1920, 1051]));
    }

    #[test]
    fn fixed_monitor_uses_swm_index_and_preserves_work_area() {
        let mut configuration = configuration();
        assert_eq!(bounds(&configuration), Some([-1920, 29, 1920, 1051]));
        configuration.picker_monitor = PickerMonitor::Fixed(1);
        assert_eq!(bounds(&configuration), Some([0, 50, 2560, 1390]));
        configuration.state.selected_monitor = Some(MonitorId(0));
        assert_eq!(bounds(&configuration), Some([0, 50, 2560, 1390]));
    }

    #[test]
    fn focused_monitor_follows_the_latest_configuration_snapshot() {
        let mut configuration = configuration();
        configuration.picker_monitor = PickerMonitor::Focused;
        assert_eq!(bounds(&configuration), Some([0, 50, 2560, 1390]));
        configuration.state.selected_monitor = Some(MonitorId(0));
        assert_eq!(bounds(&configuration), Some([-1920, 29, 1920, 1051]));
    }

    #[test]
    fn unavailable_targets_fall_back_to_a_visible_monitor() {
        let mut configuration = configuration();
        configuration.picker_monitor = PickerMonitor::Fixed(99);
        assert_eq!(bounds(&configuration), Some([-1920, 29, 1920, 1051]));
        configuration.state.monitors[0].is_hidden = true;
        assert_eq!(bounds(&configuration), Some([0, 50, 2560, 1390]));
        configuration.picker_monitor = PickerMonitor::Fixed(0);
        assert_eq!(bounds(&configuration), Some([0, 50, 2560, 1390]));
        configuration.picker_monitor = PickerMonitor::Focused;
        configuration.state.selected_monitor = None;
        assert_eq!(bounds(&configuration), Some([0, 50, 2560, 1390]));
        configuration.state.monitors[1].width = 0;
        assert_eq!(bounds(&configuration), None);
        configuration.state.monitors.clear();
        assert_eq!(bounds(&configuration), None);
    }
}
