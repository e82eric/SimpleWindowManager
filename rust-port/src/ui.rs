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
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;
const WAKE: u32 = WM_APP + 74;
static THREAD: AtomicU32 = AtomicU32::new(0);
static CHANNEL: OnceLock<SyncSender<Command>> = OnceLock::new();
static BINDINGS: OnceLock<Vec<KeyBinding>> = OnceLock::new();
static BAR_HANDLES: OnceLock<Vec<(usize, isize)>> = OnceLock::new();
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
static MENU_VISIBLE: AtomicBool = AtomicBool::new(false);
pub fn menu_visible() -> bool {
    MENU_VISIBLE.load(Ordering::Acquire)
}
thread_local! { static UI:RefCell<Option<Ui>>=const { RefCell::new(None) }; }
pub enum Command {
    Action(Action),
    Named(String),
    Closed { session: u64, restore_focus: bool },
    Window(isize),
    Program(String, bool),
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
    matches!(
        action,
        Action::ShowWindows
            | Action::ShowPrograms(_)
            | Action::ShowProcesses
            | Action::ShowServices
            | Action::ShowFileSystem
            | Action::ShowCommands
            | Action::ShowFloatLogs
            | Action::ShowClientLogs
            | Action::ShowErrorLogs
            | Action::ShowConfigurationState
    )
}
pub fn queue_action(action: Action) -> bool {
    if is_menu(action) || action == Action::Quit {
        post(Command::Action(action))
    } else {
        crate::worker::send(crate::worker::Command::Action(action))
    }
}
pub struct Ui {
    pub(crate) menu_session: u64,
    pub configuration: std::sync::Arc<Configuration>,
    pub rust_nfm: Option<rust_nfm::runtime::PickerRuntime>,
    pub menu_visible: bool,
    bars: Vec<crate::bar::BarWindow>,
    snapshot: Option<std::sync::Arc<crate::worker::Snapshot>>,
}
impl Ui {
    pub fn execute(&mut self, action: Action) -> PlatformResult<()> {
        use Action::*;
        if is_menu(action) {
            self.request_menu(action);
            self.publish_menu_visibility();
            return Ok(());
        }
        match action {
            Quit => unsafe {
                PostQuitMessage(0);
            },
            _ => {
                crate::worker::send(crate::worker::Command::Action(action));
            }
        }
        Ok(())
    }
    fn publish_menu_visibility(&self) {
        MENU_VISIBLE.store(self.menu_visible, Ordering::Release);
    }
    fn request_menu(&mut self, action: Action) {
        if self.menu_visible {
            if let Some(menu) = &self.rust_nfm {
                menu.hide();
            }
            self.menu_visible = false;
        }
        self.start_menu(action);
    }
    fn start_menu(&mut self, action: Action) {
        use Action::*;
        self.menu_session = self.menu_session.wrapping_add(1);
        match action {
            ShowWindows => self.run_new_windows_menu(),
            ShowPrograms(elevated) => self.run_program_launcher_menu(elevated),
            ShowProcesses => self.run_process_menu(),
            ShowServices => self.run_services_menu(),
            ShowFileSystem => self.run_file_system_menu(),
            ShowCommands => self.run_commands_menu(),
            ShowFloatLogs => self.run_float_logs_menu(),
            ShowClientLogs => self.run_client_logs_menu(),
            ShowErrorLogs => self.run_error_logs_menu(),
            ShowConfigurationState => self.run_configuration_state_menu(),
            _ => unreachable!("start_menu only accepts menu actions"),
        }
    }
    fn menu_closed(&mut self, restore_focus: bool) {
        self.menu_visible = false;
        if restore_focus {
            crate::worker::send(crate::worker::Command::MenuClosed);
        }
        self.publish_menu_visibility();
    }
    fn hide(&mut self) {
        self.menu_session = self.menu_session.wrapping_add(1);
        if let Some(menu) = &self.rust_nfm {
            menu.hide();
        }
        self.menu_visible = false;
        self.publish_menu_visibility();
        crate::worker::send(crate::worker::Command::MenuClosed);
    }
    fn run_new_windows_menu(&mut self) {
        let monitor = &self.configuration.state.monitors[0];
        if let Some(menu) = &self.rust_nfm {
            menu.set_preferred_center(
                monitor.x_offset + monitor.width / 2,
                (monitor.top + monitor.bottom) / 2,
            );
            match menu.show_windows() {
                Ok(task) => {
                    crate::platform::consume_picker_task(task, self.menu_session, |response| {
                        match response {
                            rust_nfm::request::PickerResponse::Selected(item) => {
                                Some(Command::Window(item.native_window))
                            }
                            _ => None,
                        }
                    });
                    self.menu_visible = true;
                }
                Err(error) => {
                    crate::error_log::report("failed to start window picker", format!("{error:#}"))
                }
            }
        }
    }

    fn run_program_launcher_menu(&mut self, elevated: bool) {
        let Some(definition) = self
            .configuration
            .program_launchers
            .iter()
            .find(|menu| menu.is_elevated == elevated)
        else {
            return;
        };
        let monitor = &self.configuration.state.monitors[0];
        if let Some(menu) = &self.rust_nfm {
            menu.set_preferred_center(
                monitor.x_offset + monitor.width / 2,
                (monitor.top + monitor.bottom) / 2,
            );
            match menu.show_programs(definition.directories.clone()) {
                Ok(task) => {
                    crate::platform::consume_picker_task(task, self.menu_session, move |response| {
                        match response {
                            rust_nfm::request::PickerResponse::Selected(value) => {
                                Some(Command::Program(value, elevated))
                            }
                            _ => None,
                        }
                    })
                }
                Err(error) => {
                    crate::error_log::report(
                        "failed to start program picker",
                        format!("{error:#}"),
                    );
                    return;
                }
            }
            self.menu_visible = true;
        }
    }

    fn run_process_menu(&mut self) {
        let monitor = &self.configuration.state.monitors[0];
        if let Some(menu) = &self.rust_nfm {
            menu.set_preferred_center(
                monitor.x_offset + monitor.width / 2,
                (monitor.top + monitor.bottom) / 2,
            );
            match menu.show_processes() {
                Ok(task) => {
                    crate::platform::consume_picker_task(task, self.menu_session, |_| None);
                    self.menu_visible = true;
                }
                Err(error) => {
                    crate::error_log::report("failed to start process picker", format!("{error:#}"))
                }
            }
            return;
        }
    }

    fn run_services_menu(&mut self) {
        crate::platform::show_services_menu(self);
    }

    fn run_file_system_menu(&mut self) {
        let monitor = &self.configuration.state.monitors[0];
        if let Some(menu) = &self.rust_nfm {
            menu.set_preferred_center(
                monitor.x_offset + monitor.width / 2,
                (monitor.top + monitor.bottom) / 2,
            );
            let options = rust_nfm::runtime::FileSystemPickerOptions {
                max_depth: 5,
                preview_visible: true,
                ..Default::default()
            };
            match menu.show_file_system(options) {
                Ok(task) => {
                    crate::platform::consume_picker_task(task, self.menu_session, |response| {
                        match response {
                            rust_nfm::request::PickerResponse::Selected(value) => {
                                Some(Command::Program(value, false))
                            }
                            _ => None,
                        }
                    });
                    self.menu_visible = true;
                }
                Err(error) => {
                    crate::error_log::report("failed to start file picker", format!("{error:#}"))
                }
            }
        }
    }

    pub fn run_commands_menu(&mut self) {
        crate::platform::show_commands_menu(self);
    }
    fn run_float_logs_menu(&mut self) {
        crate::platform::show_float_logs_menu(self);
    }
    fn run_client_logs_menu(&mut self) {
        crate::platform::show_client_logs_menu(self);
    }

    fn run_error_logs_menu(&mut self) {
        crate::platform::show_error_logs_menu(self);
    }

    fn run_configuration_state_menu(&mut self) {
        crate::platform::show_configuration_state_menu(self);
    }
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
    let rust_nfm = rust_nfm::runtime::PickerRuntime::start().ok();
    UI.with(|slot| {
        *slot.borrow_mut() = Some(Ui {
            menu_session: 0,
            configuration: std::sync::Arc::new(configuration),
            rust_nfm,
            menu_visible: false,
            bars,
            snapshot: None,
        })
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
            if let Command::Closed { session, .. } = &command
                && (*session != ui.menu_session || !ui.menu_visible)
            {
                continue;
            }
            match command {
                Command::Action(action) => {
                    if let Err(error) = ui.execute(action) {
                        error.report();
                    }
                }
                Command::Named(name) => {
                    let action = ui
                        .configuration
                        .commands
                        .iter()
                        .find(|c| c.name == name)
                        .map(|c| c.action);
                    if let Some(action) = action {
                        if let Err(error) = ui.execute(action) {
                            error.report();
                        }
                    }
                }
                Command::Closed { restore_focus, .. } => {
                    ui.menu_closed(restore_focus);
                }
                Command::Window(hwnd) => {
                    crate::worker::send(crate::worker::Command::Window(hwnd));
                }
                Command::Program(value, elevated) => {
                    crate::worker::send(crate::worker::Command::Program(value, elevated));
                }
                Command::Focus => {
                    if ui.menu_visible {
                        if let Some(menu) = &ui.rust_nfm {
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
    bar.segments = crate::bar::rendered_segments(&snapshot.configuration.bar_segments);
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
            drop(ui.rust_nfm);
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
        assert!(keys.event(66, false, || ui_tx.try_send(Action::ShowCommands).is_ok()));
        assert_eq!(ui_rx.try_recv().unwrap(), Action::ShowCommands);
        assert_eq!(worker_rx.try_recv().unwrap(), Action::SelectLeft);
        assert!(worker_rx.try_recv().is_err());
    }
}
