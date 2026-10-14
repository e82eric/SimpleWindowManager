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
    MenuRequested {
        session: u64,
        name: String,
    },
    Closed {
        session: u64,
        restore_focus: bool,
    },
    Window(isize),
    Program(String, bool),
    FileSelected {
        session: u64,
        path: String,
        acceptance: crate::egui_window_menu::FileAcceptance,
    },
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
            | Action::ShowPicker(_)
            | Action::ShowCommands
            | Action::ShowFloatLogs
            | Action::ShowClientLogs
            | Action::ShowErrorLogs
            | Action::ShowConfigurationState
    )
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
    pub menu_visible: bool,
    egui_windows: Option<crate::egui_window_menu::WindowMenuHost>,
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
        self.start_menu(action);
        // A menu without an available source must not leave a closed picker visible.
        if !self.menu_visible {
            if let Some(menu) = &self.egui_windows {
                menu.hide();
            }
            crate::worker::send(crate::worker::Command::MenuClosed);
        }
    }
    fn start_menu(&mut self, action: Action) {
        use Action::*;
        self.menu_session = self.menu_session.wrapping_add(1);
        self.menu_visible = false;
        match action {
            ShowWindows => self.run_new_windows_menu(),
            ShowPrograms(elevated) => self.run_program_launcher_menu(elevated),
            ShowProcesses => self.run_process_menu(),
            ShowServices => self.run_services_menu(),
            ShowFileSystem => self.run_file_system_menu(),
            ShowPicker(id) => self.run_picker(id),
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
        if let Some(menu) = &self.egui_windows {
            menu.hide();
        }
        self.menu_session = self.menu_session.wrapping_add(1);
        self.menu_visible = false;
        self.publish_menu_visibility();
        crate::worker::send(crate::worker::Command::MenuClosed);
    }
    fn run_new_windows_menu(&mut self) {
        let Some(host) = self.egui_windows.as_ref() else {
            return;
        };
        let Some(bounds) = picker_bounds(&self.configuration) else {
            return;
        };
        match host.show(self.menu_session, bounds) {
            Ok(()) => {
                self.menu_visible = true;
            }
            Err(error) => {
                crate::error_log::report("failed to show egui window picker", error);
                self.egui_windows = None;
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
        let directories = definition.directories.clone();
        let Some(host) = self.egui_windows.as_ref() else {
            return;
        };
        let Some(bounds) = picker_bounds(&self.configuration) else {
            return;
        };
        match host.show_programs(
            self.menu_session,
            bounds,
            directories,
            elevated,
            self.configuration.file_preview_bat_theme.clone(),
        ) {
            Ok(()) => {
                self.menu_visible = true;
            }
            Err(error) => {
                crate::error_log::report("failed to show egui program picker", error);
                self.egui_windows = None;
            }
        }
    }
    fn run_process_menu(&mut self) {
        let Some(host) = self.egui_windows.as_ref() else {
            return;
        };
        let Some(bounds) = picker_bounds(&self.configuration) else {
            return;
        };
        match host.show_processes(self.menu_session, bounds) {
            Ok(()) => {
                self.menu_visible = true;
            }
            Err(error) => {
                crate::error_log::report("failed to show egui process picker", error);
                self.egui_windows = None;
            }
        }
    }
    fn run_services_menu(&mut self) {
        crate::platform::show_services_menu(self);
    }

    fn run_file_system_menu(&mut self) {
        let Some(host) = self.egui_windows.as_ref() else {
            return;
        };
        let Some(bounds) = picker_bounds(&self.configuration) else {
            return;
        };
        let options = nfm_file_system::FileSystemPickerOptions {
            bat_theme: self.configuration.file_preview_bat_theme.clone(),
            max_depth: 5,
            preview_visible: true,
            ..Default::default()
        };
        match host.show_file_system(self.menu_session, bounds, options) {
            Ok(()) => {
                self.menu_visible = true;
            }
            Err(error) => {
                crate::error_log::report("failed to show egui file picker", error);
                self.egui_windows = None;
            }
        }
    }
    fn run_picker(&mut self, id: usize) {
        let Some(host) = self.egui_windows.as_ref() else {
            return;
        };
        let Some(bounds) = picker_bounds(&self.configuration) else {
            return;
        };
        let Some(config) = self.configuration.pickers.get(id).cloned() else {
            crate::error_log::report("picker failed", "unknown picker ID");
            return;
        };
        match host.show_picker(self.menu_session, bounds, config) {
            Ok(()) => self.menu_visible = true,
            Err(error) => crate::error_log::report("picker failed", error),
        }
    }
    pub fn run_commands_menu(&mut self) {
        let Some(host) = self.egui_windows.as_ref() else {
            return;
        };
        let Some(bounds) = picker_bounds(&self.configuration) else {
            return;
        };
        match host.show_commands_with_menu_actions(
            self.menu_session,
            bounds,
            crate::platform::command_menu_rows(self),
            self.configuration
                .commands
                .iter()
                .filter(|command| is_menu(command.action))
                .map(|command| command.name.clone())
                .collect(),
        ) {
            Ok(()) => {
                self.menu_visible = true;
            }
            Err(error) => {
                crate::error_log::report("failed to show egui commands picker", error);
                self.egui_windows = None;
            }
        }
    }
    pub(crate) fn show_data_menu(
        &mut self,
        title: &str,
        columns: Vec<String>,
        rows: Vec<crate::egui_data_menu::DataRow>,
    ) {
        let Some(host) = self.egui_windows.as_ref() else {
            return;
        };
        let Some(bounds) = picker_bounds(&self.configuration) else {
            return;
        };
        match host.show_data(self.menu_session, bounds, title.into(), columns, rows) {
            Ok(()) => self.menu_visible = true,
            Err(error) => {
                crate::error_log::report("failed to show egui data picker", error);
                self.egui_windows = None;
            }
        }
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
fn start_egui_window_host(
    configuration: &Configuration,
) -> Result<crate::egui_window_menu::WindowMenuHost, String> {
    use crate::egui_window_menu::{Outcome, WindowMenuHost};
    WindowMenuHost::start_with_preview_resolver(
        configuration.picker_result_rows,
        configuration.picker_preview_rows,
        configuration.file_preview_resolver.clone(),
        |session, outcome| {
            if let Outcome::FileSelected { path, acceptance } = &outcome {
                post(Command::FileSelected {
                    session,
                    path: path.clone(),
                    acceptance: *acceptance,
                });
                return;
            }
            if let Outcome::MenuRequested(name) = outcome {
                post(Command::MenuRequested { session, name });
                return;
            }
            let selected = match outcome {
                Outcome::Selected(hwnd) => Some(Command::Window(hwnd)),
                Outcome::FileSelected { .. } => unreachable!(),
                Outcome::CommandSelected(name) => Some(Command::Named(name)),
                Outcome::MenuRequested(_) => unreachable!(),
                Outcome::Cancelled => None,
                Outcome::Failed(error) => {
                    crate::error_log::report("egui picker failed", error);
                    None
                }
            };
            post(Command::Closed {
                session,
                restore_focus: selected.is_none(),
            });
            if let Some(command) = selected {
                post(command);
            }
        },
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
            menu_visible: false,
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
            if let Command::Closed { session, .. } = &command
                && (*session != ui.menu_session || !ui.menu_visible)
            {
                continue;
            }
            match command {
                Command::MenuRequested { session, name } => {
                    if session != ui.menu_session || !ui.menu_visible {
                        continue;
                    }
                    let action = ui
                        .configuration
                        .commands
                        .iter()
                        .find(|command| command.name == name)
                        .map(|command| command.action);
                    if let Some(action) = action.filter(|action| is_menu(*action)) {
                        if let Err(error) = ui.execute(action) {
                            error.report();
                        }
                    } else {
                        ui.hide();
                    }
                }
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
                Command::FileSelected {
                    session,
                    path,
                    acceptance,
                } => {
                    if session != ui.menu_session || !ui.menu_visible {
                        continue;
                    }
                    ui.menu_closed(false);
                    use crate::egui_window_menu::FileAcceptance;
                    let command = match acceptance {
                        FileAcceptance::Open => crate::worker::Command::Program(path, false),
                        FileAcceptance::Launch { elevated } => {
                            crate::worker::Command::Program(path, elevated)
                        }
                        FileAcceptance::Callback { on_accept } => {
                            crate::worker::Command::PickerSelected {
                                on_accept,
                                selection: crate::picker::PickerSelection::File(path.into()),
                            }
                        }
                    };
                    if !crate::worker::send(command) {
                        crate::error_log::report("file selection failed", "worker queue is full");
                    }
                }
                Command::Focus => {
                    if ui.menu_visible {
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
