use crate::commands::{Action, KeyBinding};
use crate::config::Configuration;
use crate::model::{
    Client, Monitor, MonitorId, Rect, WindowManagerState, WindowRoutingMode, WorkspaceId,
};
use crate::platform_error::{PlatformError, PlatformResult, check_bool, check_hresult};
use crate::snapshot::{DesktopSnapshot, WindowSnapshot};

const MAX_WORKSPACES: usize = 10;

pub struct Runtime {
    pub configuration: Configuration,
    pub hidden_window_monitor: Monitor,
    pub key_bindings: Vec<KeyBinding>,
    pub rust_nfm: Option<rust_nfm::runtime::PickerRuntime>,
    last_menu_action: Option<Action>,
    pub bars: Vec<crate::bar::BarWindow>,
    pub drag_drop_state: DragDropState,
    pub regular_resize_client: Option<crate::ClientId>,
    pub border_window: Option<crate::border::BorderWindow>,
    pub drop_target_window: Option<crate::drop_target::DropTargetWindow>,
    pub workspace_move_foreground: Option<isize>,
    pub event_foreground_hwnd: isize,
    pub is_foreground_window_same_as_selected: bool,
}

#[derive(Default)]
pub struct DragDropState {
    pub in_progress: bool,
    pub drag_hwnd: isize,
    pub drop_target_client: Option<crate::ClientId>,
    pub drop_target_monitor: Option<MonitorId>,
}

impl Runtime {
    pub fn is_root_window(&self, hwnd: isize) -> bool {
        crate::snapshot::is_root_window_handle(hwnd, self.configuration.float_uwp_windows)
    }
    pub fn initialize(mut configuration: Configuration, snapshot: DesktopSnapshot) -> Self {
        let (screen_width, screen_height) = primary_screen_size();
        let usable_bottom = screen_height - primary_taskbar_height();
        let display_monitor_count = snapshot.monitors.len().max(1);
        configuration.state.monitors.clear();
        for index in 0..MAX_WORKSPACES {
            let mut monitor = Monitor::new(
                index as i32 + 1,
                Rect::from_xywh(
                    index as i32 * screen_width,
                    configuration.bar_height,
                    screen_width,
                    usable_bottom - configuration.bar_height,
                ),
                13,
            );
            monitor.is_hidden = index >= display_monitor_count;
            let monitor_id = configuration.state.monitor_register(monitor);
            if index < configuration.state.workspaces.len() {
                configuration.state.monitors[monitor_id.0].workspace = Some(WorkspaceId(index));
                configuration.state.workspaces[index].monitor = Some(monitor_id);
            }
        }
        configuration.state.selected_monitor = Some(MonitorId(0));
        configuration.state.monitors[0].selected = true;

        for window in snapshot.windows {
            window_manager_try_add_window(&mut configuration, window, true);
        }
        for index in 0..configuration.state.workspaces.len() {
            configuration.state.arrange_workspace(WorkspaceId(index));
        }

        let mut hidden_window_monitor = Monitor::new(
            MAX_WORKSPACES as i32 + 1,
            Rect::from_xywh(
                MAX_WORKSPACES as i32 * screen_width,
                0,
                screen_width,
                screen_height,
            ),
            13,
        );
        hidden_window_monitor.is_hidden = true;
        let key_bindings = std::mem::take(&mut configuration.key_bindings);
        Self {
            configuration,
            hidden_window_monitor,
            key_bindings,
            rust_nfm: None,
            last_menu_action: None,
            bars: Vec::new(),
            drag_drop_state: DragDropState::default(),
            regular_resize_client: None,
            border_window: None,
            drop_target_window: None,
            workspace_move_foreground: None,
            event_foreground_hwnd: 0,
            is_foreground_window_same_as_selected: false,
        }
    }

    pub fn apply_all_workspaces(&mut self) -> PlatformResult<()> {
        for workspace in 0..self.configuration.state.workspaces.len() {
            self.configuration
                .state
                .arrange_workspace(WorkspaceId(workspace));
            apply_workspace_windows(
                &mut self.configuration.state,
                WorkspaceId(workspace),
                self.configuration.always_redraw,
                self.hidden_window_monitor.x_offset,
                self.configuration.use_old_move_logic_configured,
            )?;
        }
        Ok(())
    }

    pub fn finish_startup(&mut self) {
        self.focus_selected(self.selected_workspace());
        self.rust_nfm = rust_nfm::runtime::PickerRuntime::start().ok();
    }

    pub fn remove_client_by_hwnd(&mut self, hwnd: isize) -> PlatformResult<()> {
        let Some(client_id) = self
            .configuration
            .state
            .window_manager_find_client_in_workspaces_by_hwnd(hwnd)
        else {
            return Ok(());
        };
        let workspace = self.configuration.state.clients[client_id.0]
            .as_ref()
            .unwrap()
            .workspace;
        if self.regular_resize_client == Some(client_id) {
            self.regular_resize_client = None;
        }
        self.configuration.state.workspace_remove_client(client_id);
        self.configuration.state.clients[client_id.0] = None;
        self.configuration.state.arrange_workspace(workspace);
        apply_workspace_windows(
            &mut self.configuration.state,
            workspace,
            self.configuration.always_redraw,
            self.hidden_window_monitor.x_offset,
            self.configuration.use_old_move_logic_configured,
        )?;
        self.focus_selected(self.selected_workspace());
        self.redraw_bars();
        self.border_window_update();
        Ok(())
    }

    #[tracing::instrument(skip(self), fields(hwnd = hwnd as usize))]
    pub fn refresh_window(&mut self, hwnd: isize) -> PlatformResult<()> {
        if let Some(client) = self
            .configuration
            .state
            .window_manager_find_client_in_workspaces_by_hwnd(hwnd)
        {
            let workspace = self.configuration.state.clients[client.0]
                .as_ref()
                .unwrap()
                .workspace;
            self.apply_workspace(workspace)?;
            self.focus_selected(workspace);
            self.redraw_bars();
            self.border_window_update();
            return Ok(());
        }
        if let Some(window) =
            crate::snapshot::capture_window(hwnd, false, self.configuration.float_uwp_windows)
        {
            window_manager_try_add_window(&mut self.configuration, window, false);
            if let Some(client) = self
                .configuration
                .state
                .window_manager_find_client_in_workspaces_by_hwnd(hwnd)
            {
                let workspace = self.configuration.state.clients[client.0]
                    .as_ref()
                    .unwrap()
                    .workspace;
                self.configuration.state.arrange_workspace(workspace);
                apply_workspace_windows(
                    &mut self.configuration.state,
                    workspace,
                    self.configuration.always_redraw,
                    self.hidden_window_monitor.x_offset,
                    self.configuration.use_old_move_logic_configured,
                )?;
                self.focus_selected(workspace);
                self.redraw_bars();
                self.border_window_update();
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self), fields(hwnd = hwnd as usize))]
    pub fn location_changed(&mut self, hwnd: isize) -> PlatformResult<()> {
        let client_id = self
            .configuration
            .state
            .window_manager_find_client_in_workspaces_by_hwnd(hwnd);
        let Some(client_id) = client_id else {
            if left_mouse_button_down() && !left_shift_down() {
                self.drag_drop_handle_location_change_with_mouse_down(hwnd);
            }
            return Ok(());
        };
        let minimized = is_iconic(hwnd);
        let client = self.configuration.state.clients[client_id.0]
            .as_ref()
            .unwrap();
        let workspace = client.workspace;
        let stored_minimized = client.data.is_minimized;
        let use_hide = client.data.use_minimize_to_hide;
        if stored_minimized {
            if !minimized && !use_hide {
                self.configuration
                    .state
                    .client_move_from_minimized_to_unminimized(client_id);
                self.apply_workspace(workspace)?;
                self.focus_selected(workspace);
                self.redraw_bars();
                self.border_window_update();
            }
            return Ok(());
        }
        if minimized {
            if !use_hide {
                self.configuration
                    .state
                    .client_move_from_unminimized_to_minimized(client_id);
                self.apply_workspace(workspace)?;
                self.focus_selected(workspace);
                self.redraw_bars();
                self.border_window_update();
            }
            return Ok(());
        }
        if is_zoomed(hwnd) || is_fullscreen(hwnd) {
            return Ok(());
        }
        if left_mouse_button_down() && !left_shift_down() {
            self.drag_drop_handle_location_change_with_mouse_down(hwnd);
            return Ok(());
        }
        self.configuration.state.arrange_workspace(workspace);
        apply_workspace_windows(
            &mut self.configuration.state,
            workspace,
            self.configuration.always_redraw,
            self.hidden_window_monitor.x_offset,
            self.configuration.use_old_move_logic_configured,
        )
    }

    #[tracing::instrument(skip(self), fields(hwnd = hwnd as usize))]
    pub fn foreground_changed(&mut self, hwnd: isize) {
        if let Some(preserved) = self.workspace_move_foreground {
            if hwnd != preserved && is_window(preserved) {
                nudge_set_foreground_window(preserved);
                return;
            }
            if !is_window(preserved) {
                self.workspace_move_foreground = None;
            }
        }
        self.redraw_bars();
        if hit_test_hwnd(hwnd)
            && let Some(client_id) = self
                .configuration
                .state
                .window_manager_find_client_in_workspaces_by_hwnd(hwnd)
        {
            let workspace = self.configuration.state.clients[client_id.0]
                .as_ref()
                .unwrap()
                .workspace;
            let selected_monitor_client = self
                .configuration
                .state
                .selected_monitor
                .and_then(|monitor| self.configuration.state.monitors[monitor.0].workspace)
                .and_then(|workspace| self.configuration.state.workspaces[workspace.0].selected);
            if selected_monitor_client != Some(client_id) {
                self.configuration.state.workspaces[workspace.0].selected = Some(client_id);
                if let Some(monitor) = self.configuration.state.workspaces[workspace.0].monitor
                    && !self.configuration.state.monitors[monitor.0].is_hidden
                {
                    self.monitor_select(monitor);
                }
            }
        }
        self.is_foreground_window_same_as_selected = self
            .configuration
            .state
            .selected_monitor
            .and_then(|monitor| self.configuration.state.monitors[monitor.0].workspace)
            .and_then(|workspace| self.configuration.state.workspaces[workspace.0].selected)
            .and_then(|client| self.configuration.state.clients[client.0].as_ref())
            .is_some_and(|client| {
                client.data.hwnd == hwnd || self.configuration.state.menu_visible
            });
        self.event_foreground_hwnd = hwnd;
        self.border_window_update();
        self.redraw_bars();
    }

    pub fn finish_workspace_move_focus_preservation(&mut self) {
        let foreground = self.workspace_move_foreground.take();
        if let Some(hwnd) = foreground
            && is_window(hwnd)
            && foreground_window() != hwnd
        {
            nudge_set_foreground_window(hwnd);
        }
    }

    pub fn key_bindings_try_handle(&mut self, key: u32, modifiers: i32) -> bool {
        let action = self
            .key_bindings
            .iter()
            .find(|binding| binding.key == key && binding.modifiers == modifiers)
            .map(|binding| binding.action);
        let Some(action) = action else {
            return false;
        };
        let span = tracing::info_span!(
            "key_binding",
            key,
            modifiers,
            action = ?action,
        );
        let _entered = span.enter();
        if let Err(error) = self.execute(action) {
            error.report();
        }
        true
    }

    #[tracing::instrument(skip(self))]
    pub fn execute(&mut self, action: Action) -> PlatformResult<()> {
        use Action::*;
        if matches!(
            action,
            ShowWindows
                | ShowPrograms(_)
                | ShowProcesses
                | ShowServices
                | ShowFileSystem
                | ShowCommands
                | ShowFloatLogs
                | ShowClientLogs
                | ShowErrorLogs
        ) {
            self.last_menu_action = Some(action);
        }
        match action {
            Quit => platform_exit(),
            QuitAndRestore => {
                crate::restore::restore_moved_windows_to_screen(false)?;
                platform_exit();
            }
            CloseFocused => close_focused_window(),
            KillFocused => kill_focused_window()
                .map_err(|code| PlatformError::win32("kill_focused_window", code))?,
            ToggleTaskbar => taskbar_toggle(),
            ToggleFilteredCurrent => {
                self.toggle_routing(WindowRoutingMode::FilteredCurrentWorkspace)
            }
            RedrawFocused => redraw_focused_window(),
            MinimizeFocused => {
                minimize_focused_window();
                self.focus_selected(self.selected_workspace());
            }
            ShowWindows => self.run_new_windows_menu(),
            ShowPrograms(elevated) => self.run_program_launcher_menu(elevated),
            ShowProcesses => self.run_process_menu(),
            ShowServices => self.run_services_menu(),
            ShowFileSystem => self.run_file_system_menu(),
            ShowCommands => self.run_commands_menu(),
            ShowFloatLogs => self.run_float_logs_menu(),
            ShowClientLogs => self.run_client_logs_menu(),
            ShowErrorLogs => self.run_error_logs_menu(),
            RunLastMenu => self.run_last_menu()?,
            Launch(command) => crate::process::start_app(command)
                .map_err(|code| PlatformError::win32("ShellExecuteW", code))?,
            MoveFloat(dx, dy) => self.move_float_or_resize_main(dx, dy)?,
            MoveFloatToMonitor(index) => self.move_float_to_monitor(index),
            MoveWorkspaceToMonitor(workspace, monitor) => {
                self.move_workspace_to_monitor(WorkspaceId(workspace), MonitorId(monitor))?
            }
            MoveSecondaryFocusedToMain => self.move_secondary_focused_to_main()?,
            ToggleUnfilteredCurrent => {
                self.toggle_routing(WindowRoutingMode::NotFilteredCurrentWorkspace)
            }
            ToggleUnmatchedCurrent => {
                self.toggle_routing(WindowRoutingMode::FilteredRoutedNonFilteredCurrentWorkspace)
            }
            SelectNextMonitor => self.monitor_select_next(),
            SelectWorkspace(index) => self.swap_selected_monitor_to(WorkspaceId(index))?,
            MoveToWorkspace(index) => self.move_focused_window_to_workspace(WorkspaceId(index))?,
            MoveToSelectedWorkspace => {
                let workspace = self.selected_workspace();
                self.move_focused_window_to_workspace(workspace)?
            }
            StopManaging => self.client_stop_managing()?,
            SetMonocle => self.monitor_set_layout(crate::Layout::Monocle)?,
            SetDeck => self.monitor_set_layout(crate::Layout::Deck)?,
            SetGrid => self.monitor_set_layout(crate::Layout::Grid)?,
            SetTile => self.monitor_set_layout(crate::Layout::Tile)?,
            SetTileReversed => self.monitor_set_layout(crate::Layout::TileReversed)?,
            ResetLayout => {
                let workspace = self.selected_workspace();
                self.configuration.state.workspaces[workspace.0].main_offset = 0;
                self.apply_workspace(workspace)?;
                if self.configuration.state.menu_visible {
                    if let Some(menu) = &self.rust_nfm {
                        menu.focus();
                    }
                } else {
                    self.focus_selected(workspace);
                }
            }
            SelectDown => {
                let workspace = self.selected_workspace();
                self.configuration.state.layout_select_down(workspace);
                self.apply_workspace(workspace)?;
                self.focus_selected(workspace);
            }
            SelectUp => {
                let workspace = self.selected_workspace();
                self.configuration.state.layout_select_up(workspace);
                self.apply_workspace(workspace)?;
                self.focus_selected(workspace);
            }
            SelectRight => {
                let workspace = self.selected_workspace();
                self.configuration.state.layout_select_right(workspace);
                self.focus_selected(workspace);
            }
            SelectLeft => {
                let workspace = self.selected_workspace();
                self.configuration.state.layout_select_left(workspace);
                self.focus_selected(workspace);
            }
            MoveDown | MoveRight | MoveUp | MoveLeft => self.move_selected_direction(action)?,
            MoveToMain => self.move_selected_to_main()?,
        }
        self.redraw_bars();
        self.border_window_update();
        Ok(())
    }

    fn selected_workspace(&self) -> WorkspaceId {
        self.configuration.state.monitors[self.configuration.state.selected_monitor.unwrap().0]
            .workspace
            .unwrap()
    }

    #[tracing::instrument(skip(self), fields(workspace = workspace.0))]
    fn apply_workspace(&mut self, workspace: WorkspaceId) -> PlatformResult<()> {
        self.configuration.state.arrange_workspace(workspace);
        apply_workspace_windows(
            &mut self.configuration.state,
            workspace,
            self.configuration.always_redraw,
            self.hidden_window_monitor.x_offset,
            self.configuration.use_old_move_logic_configured,
        )?;
        Ok(())
    }

    fn focus_selected(&mut self, workspace: WorkspaceId) {
        if self.configuration.state.menu_visible {
            return;
        }
        if let Some(client) = self.configuration.state.workspaces[workspace.0].selected {
            if let Some(client) = self.configuration.state.clients[client.0].as_ref() {
                if foreground_window() != client.data.hwnd {
                    nudge_set_foreground_window(client.data.hwnd);
                }
                self.is_foreground_window_same_as_selected = true;
                return;
            }
        }
        if let Some(monitor) = self.configuration.state.workspaces[workspace.0].monitor
            && let Some(bar) = self.bars.iter().find(|bar| bar.monitor_index == monitor.0)
        {
            set_foreground_window(bar.hwnd);
        }
    }

    fn toggle_routing(&mut self, mode: WindowRoutingMode) {
        self.configuration.state.current_window_routing_mode =
            if self.configuration.state.current_window_routing_mode == mode {
                WindowRoutingMode::FilteredAndRoutedToWorkspace
            } else {
                mode
            };
    }

    fn move_selected_direction(&mut self, action: Action) -> PlatformResult<()> {
        use crate::model::MoveDirection;
        let Some(client) = self
            .configuration
            .state
            .window_manager_find_client_in_workspaces_by_hwnd(foreground_window())
        else {
            return Ok(());
        };
        let workspace = self.configuration.state.clients[client.0]
            .as_ref()
            .unwrap()
            .workspace;
        let direction = match action {
            Action::MoveLeft => MoveDirection::Left,
            Action::MoveRight => MoveDirection::Right,
            Action::MoveUp => MoveDirection::Up,
            Action::MoveDown => MoveDirection::Down,
            _ => unreachable!(),
        };
        self.configuration
            .state
            .layout_move_client_direction(workspace, client, direction);
        self.apply_workspace(workspace)?;
        self.focus_selected(workspace);
        Ok(())
    }

    fn move_selected_to_main(&mut self) -> PlatformResult<()> {
        let workspace = self.selected_workspace();
        let clients = self.configuration.state.workspaces[workspace.0]
            .clients
            .clone();
        if clients.len() > 1 {
            let selected = self.configuration.state.workspaces[workspace.0]
                .selected
                .unwrap_or(clients[0]);
            self.configuration
                .state
                .layout_move_client_to_main(workspace, selected);
            self.apply_workspace(workspace)?;
            self.focus_selected(workspace);
        }
        Ok(())
    }

    fn monitor_select_next(&mut self) {
        let current = self.configuration.state.selected_monitor.unwrap().0;
        let count = self
            .configuration
            .state
            .monitors
            .iter()
            .take_while(|monitor| !monitor.is_hidden)
            .count();
        let next = (current + 1) % count;
        self.monitor_select(MonitorId(next));
    }

    fn monitor_select(&mut self, monitor: MonitorId) {
        if self.configuration.state.monitors[monitor.0].is_hidden {
            return;
        }
        for item in &mut self.configuration.state.monitors {
            item.selected = false;
        }
        self.configuration.state.monitors[monitor.0].selected = true;
        self.configuration.state.selected_monitor = Some(monitor);
        if let Some(workspace) = self.configuration.state.monitors[monitor.0].workspace {
            self.focus_selected(workspace);
        }
        self.redraw_bars();
    }

    fn monitor_set_layout(&mut self, layout: crate::Layout) -> PlatformResult<()> {
        let workspace = self.selected_workspace();
        let previous = self.configuration.state.workspaces[workspace.0].layout;
        if previous == crate::Layout::Monocle && layout != crate::Layout::Monocle {
            self.configuration
                .state
                .workspace_restore_saved_layout(workspace);
        } else if previous != crate::Layout::Monocle && layout == crate::Layout::Monocle {
            self.configuration.state.workspace_save_layout(workspace);
        }
        self.configuration.state.workspaces[workspace.0].layout = layout;
        self.apply_workspace(workspace)?;
        self.focus_selected(workspace);
        Ok(())
    }

    fn swap_selected_monitor_to(&mut self, workspace: WorkspaceId) -> PlatformResult<()> {
        if workspace.0 >= self.configuration.state.workspaces.len() {
            return Ok(());
        }
        let selected_monitor = self.configuration.state.selected_monitor.unwrap();
        let current_monitor = self.configuration.state.workspaces[workspace.0]
            .monitor
            .unwrap();
        let displaced = self.configuration.state.monitors[selected_monitor.0]
            .workspace
            .unwrap();
        self.configuration.state.last_workspace = Some(displaced);
        if self.configuration.state.menu_visible {
            self.menu_hide();
        }
        if selected_monitor == current_monitor {
            return Ok(());
        }
        self.configuration.state.monitors[selected_monitor.0].workspace = Some(workspace);
        self.configuration.state.workspaces[workspace.0].monitor = Some(selected_monitor);
        self.configuration.state.monitors[current_monitor.0].workspace = Some(displaced);
        self.configuration.state.workspaces[displaced.0].monitor = Some(current_monitor);
        self.apply_workspace(workspace)?;
        self.apply_workspace(displaced)?;
        self.focus_selected(workspace);
        Ok(())
    }

    fn move_focused_window_to_workspace(&mut self, workspace: WorkspaceId) -> PlatformResult<()> {
        if workspace.0 >= self.configuration.state.workspaces.len() {
            return Ok(());
        }
        let hwnd = foreground_window();
        let client = self
            .configuration
            .state
            .window_manager_find_client_in_workspaces_by_hwnd(hwnd);
        let Some(client) = client else {
            if let Some(window) = crate::snapshot::capture_foreground_window(hwnd) {
                let mut client = Client {
                    data: window.data,
                    is_visible: true,
                    workspace,
                };
                client.data.use_minimize_to_hide =
                    client_should_use_minimize_to_hide(&self.configuration, &client);
                if client.data.use_minimize_to_hide {
                    client.data.is_minimized = false;
                }
                self.configuration.state.client_add(workspace, client);
                self.apply_workspace(workspace)?;
            }
            self.focus_selected(self.selected_workspace());
            return Ok(());
        };
        let old = self.configuration.state.clients[client.0]
            .as_ref()
            .unwrap()
            .workspace;
        if old == workspace {
            return Ok(());
        }
        self.configuration.state.workspace_remove_client(client);
        self.configuration.state.clients[client.0]
            .as_mut()
            .unwrap()
            .workspace = workspace;
        self.configuration.state.workspaces[workspace.0]
            .clients
            .insert(0, client);
        self.configuration.state.workspaces[workspace.0].selected = Some(client);
        self.apply_workspace(old)?;
        self.apply_workspace(workspace)?;
        self.focus_selected(self.selected_workspace());
        Ok(())
    }

    fn client_stop_managing(&mut self) -> PlatformResult<()> {
        let workspace = self.selected_workspace();
        let Some(client) = self.configuration.state.workspaces[workspace.0].selected else {
            return Ok(());
        };
        let hwnd = self.configuration.state.clients[client.0]
            .as_ref()
            .unwrap()
            .data
            .hwnd;
        self.configuration.state.workspace_remove_client(client);
        self.configuration.state.clients[client.0] = None;
        self.apply_workspace(workspace)?;
        stop_managing_window(
            hwnd,
            &self.configuration.state.monitors
                [self.configuration.state.selected_monitor.unwrap().0],
        );
        Ok(())
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
                    crate::platform::consume_picker_task(task, |response| match response {
                        rust_nfm::request::PickerResponse::Selected(item) => unsafe {
                            crate::platform::menu_window_selected(
                                item.native_window,
                                std::ptr::null_mut(),
                            )
                        },
                        _ => unsafe { crate::platform::menu_on_closed() },
                    });
                    self.configuration.state.menu_visible = true;
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
                    crate::platform::consume_picker_task(task, move |response| match response {
                        rust_nfm::request::PickerResponse::Selected(value) => {
                            if let Ok(value) = std::ffi::CString::new(value) {
                                unsafe {
                                    crate::platform::menu_program_selected(
                                        value.as_ptr() as *mut _,
                                        elevated as usize as *mut std::ffi::c_void,
                                    )
                                };
                            }
                        }
                        _ => unsafe { crate::platform::menu_on_closed() },
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
            self.configuration.state.menu_visible = true;
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
                    crate::platform::consume_picker_task(task, |_| unsafe {
                        crate::platform::menu_on_closed()
                    });
                    self.configuration.state.menu_visible = true;
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
                    crate::platform::consume_picker_task(task, |response| match response {
                        rust_nfm::request::PickerResponse::Selected(value) => {
                            if let Ok(value) = std::ffi::CString::new(value) {
                                unsafe {
                                    crate::platform::menu_program_selected(
                                        value.as_ptr() as *mut _,
                                        std::ptr::null_mut(),
                                    )
                                };
                            }
                        }
                        _ => unsafe { crate::platform::menu_on_closed() },
                    });
                    self.configuration.state.menu_visible = true;
                }
                Err(error) => {
                    crate::error_log::report("failed to start file picker", format!("{error:#}"))
                }
            }
        }
    }

    fn run_last_menu(&mut self) -> PlatformResult<()> {
        if let Some(action) = self.last_menu_action {
            self.execute(action)?;
        }
        Ok(())
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

    fn move_float_or_resize_main(&mut self, dx: i32, dy: i32) -> PlatformResult<()> {
        let hwnd = foreground_window();
        if self
            .configuration
            .state
            .window_manager_find_client_in_workspaces_by_hwnd(hwnd)
            .is_some()
        {
            if dx != 0 {
                let workspace = self.selected_workspace();
                self.configuration.state.workspaces[workspace.0].main_offset +=
                    dx * self.configuration.float_window_movement;
                self.apply_workspace(workspace)?;
            }
            return Ok(());
        }
        move_window_by(
            hwnd,
            dx * self.configuration.float_window_movement,
            dy * self.configuration.float_window_movement,
        );
        Ok(())
    }

    fn move_float_to_monitor(&self, index: usize) {
        let hwnd = foreground_window();
        if self
            .configuration
            .state
            .window_manager_find_client_in_workspaces_by_hwnd(hwnd)
            .is_some()
        {
            return;
        }
        let Some(monitor) = self
            .configuration
            .state
            .monitors
            .get(index)
            .filter(|m| !m.is_hidden)
        else {
            return;
        };
        center_window_on_monitor(hwnd, monitor);
    }

    #[tracing::instrument(
        skip(self),
        fields(workspace = workspace.0, monitor = monitor.0)
    )]
    fn move_workspace_to_monitor(
        &mut self,
        workspace: WorkspaceId,
        monitor: MonitorId,
    ) -> PlatformResult<()> {
        if workspace.0 >= self.configuration.state.workspaces.len()
            || monitor.0 >= self.configuration.state.monitors.len()
            || self.configuration.state.monitors[monitor.0].is_hidden
        {
            return Ok(());
        }
        let source = self.configuration.state.workspaces[workspace.0]
            .monitor
            .unwrap();
        let displaced = self.configuration.state.monitors[monitor.0]
            .workspace
            .unwrap();
        if self.configuration.state.selected_monitor == Some(monitor) {
            self.configuration.state.last_workspace = Some(displaced);
        }
        if self.configuration.state.menu_visible {
            self.menu_hide();
        }
        let foreground = foreground_window();
        self.workspace_move_foreground = Some(foreground);
        if source != monitor {
            self.configuration.state.monitors[monitor.0].workspace = Some(workspace);
            self.configuration.state.workspaces[workspace.0].monitor = Some(monitor);
            self.configuration.state.monitors[source.0].workspace = Some(displaced);
            self.configuration.state.workspaces[displaced.0].monitor = Some(source);
            self.apply_workspace(workspace)?;
            self.apply_workspace(displaced)?;
        }
        if foreground != 0 && foreground_window() != foreground {
            nudge_set_foreground_window(foreground);
        }
        if !crate::platform::schedule_workspace_move_focus_finish() {
            self.workspace_move_foreground = None;
        }
        Ok(())
    }

    fn move_secondary_focused_to_main(&mut self) -> PlatformResult<()> {
        let Some(workspace) = self
            .configuration
            .state
            .monitors
            .get(1)
            .and_then(|monitor| monitor.workspace)
        else {
            return Ok(());
        };
        let clients = self.configuration.state.workspaces[workspace.0]
            .clients
            .clone();
        if clients.len() > 1 {
            let mut selected = self.configuration.state.workspaces[workspace.0]
                .selected
                .unwrap_or(clients[0]);
            if selected == clients[0] {
                selected = clients[1];
            }
            self.configuration
                .state
                .layout_move_client_to_main(workspace, selected);
            self.apply_workspace(workspace)?;
        }
        Ok(())
    }

    pub fn menu_program_selected(&mut self, value: &str, elevated: bool) {
        let result = if elevated {
            crate::process::start_app(value)
        } else {
            crate::process::start_as_explorer_user(value)
        };
        if let Err(error) = result {
            crate::error_log::report_win32("program launch failed", error);
        }
        self.menu_hide();
    }

    pub fn menu_window_selected(&mut self, hwnd: isize) {
        self.menu_hide();
        if let Some(client) = self
            .configuration
            .state
            .window_manager_find_client_in_workspaces_by_hwnd(hwnd)
        {
            let workspace = self.configuration.state.clients[client.0]
                .as_ref()
                .unwrap()
                .workspace;
            let minimized = self.configuration.state.clients[client.0]
                .as_ref()
                .unwrap()
                .data
                .is_minimized;
            if minimized {
                self.configuration
                    .state
                    .client_move_from_minimized_to_unminimized(client);
                self.configuration.state.workspaces[workspace.0].selected = Some(client);
                if self.selected_workspace() != workspace {
                    if let Err(error) = self.swap_selected_monitor_to(workspace) {
                        error.report();
                        return;
                    }
                }
                restore_window(hwnd);
            } else {
                if self.configuration.state.workspaces[workspace.0].layout != crate::Layout::Grid {
                    let root = self.configuration.state.workspaces[workspace.0]
                        .clients
                        .first()
                        .copied();
                    if let Some(root) = root {
                        self.configuration
                            .state
                            .swap_client_nodes_data(client, root);
                        self.configuration.state.workspaces[workspace.0].selected = Some(root);
                    }
                } else {
                    self.configuration.state.workspaces[workspace.0].selected = Some(client);
                }
                if self.selected_workspace() != workspace {
                    if let Err(error) = self.swap_selected_monitor_to(workspace) {
                        error.report();
                        return;
                    }
                }
                if let Err(error) = self.apply_workspace(workspace) {
                    error.report();
                    return;
                }
                self.focus_selected(workspace);
            }
        } else {
            show_unmanaged_window_on_selected_monitor(
                hwnd,
                &self.configuration.state.monitors
                    [self.configuration.state.selected_monitor.unwrap().0],
            );
        }
    }

    pub fn menu_hide(&mut self) {
        if let Some(menu) = &self.rust_nfm {
            menu.hide();
        }
        self.configuration.state.menu_visible = false;
        self.redraw_bars();
        self.border_window_update();
    }

    pub fn menu_closed(&mut self) {
        self.menu_hide();
        let workspace = self.selected_workspace();
        self.focus_selected(workspace);
    }

    pub fn create_bars(&mut self) -> PlatformResult<()> {
        self.bars = crate::bar::create_bars(
            &self.configuration.state.monitors,
            self.configuration.bar_height,
        )
        .map_err(|code| PlatformError::win32("create_bars", code))?;
        Ok(())
    }

    pub fn redraw_bars(&self) {
        crate::bar::redraw(&self.bars);
    }

    pub fn create_border_window(&mut self) -> PlatformResult<()> {
        let style = self
            .configuration
            .text_style
            .as_ref()
            .ok_or(PlatformError::win32(
                "create_border_window: text style is not configured",
                87,
            ))?;
        self.border_window = Some(
            crate::border::create_border_window(style.focus_color2, style.lost_focus_color)
                .map_err(|code| PlatformError::win32("create_border_window", code))?,
        );
        self.border_window_update();
        Ok(())
    }

    pub fn create_drop_target_window(&mut self) -> PlatformResult<()> {
        self.drop_target_window = Some(
            crate::drop_target::create()
                .map_err(|code| PlatformError::win32("create_drop_target_window", code))?,
        );
        Ok(())
    }

    pub fn border_window_update(&self) {
        let Some(border) = &self.border_window else {
            return;
        };
        crate::platform::set_border_lost_focus(
            !self.configuration.state.menu_visible && !self.is_foreground_window_same_as_selected,
        );
        let target = self
            .configuration
            .state
            .selected_monitor
            .and_then(|monitor| self.configuration.state.monitors[monitor.0].workspace)
            .and_then(|workspace| self.configuration.state.workspaces[workspace.0].selected)
            .and_then(|client| self.configuration.state.clients[client.0].as_ref())
            .filter(|client| client.is_visible)
            .map(|client| client.data.rect);
        crate::border::update_border(border, target);
    }

    pub fn bar_snapshot(&self, monitor_index: usize) -> Option<crate::bar::BarSnapshot> {
        let monitor = self.configuration.state.monitors.get(monitor_index)?;
        let workspace = monitor.workspace?;
        let style = self.configuration.text_style.as_ref()?;
        let tags = self
            .configuration
            .state
            .workspaces
            .iter()
            .enumerate()
            .filter(|(_, item)| item.monitor.is_some() || !item.clients.is_empty())
            .map(|(index, item)| {
                (
                    index,
                    item.tag.clone(),
                    item.monitor == Some(MonitorId(monitor_index)),
                    !item.clients.is_empty(),
                    item.is_icon,
                )
            })
            .collect();
        let mode = match self.configuration.state.current_window_routing_mode {
            WindowRoutingMode::FilteredAndRoutedToWorkspace => "1",
            WindowRoutingMode::FilteredCurrentWorkspace => "2",
            WindowRoutingMode::NotFilteredCurrentWorkspace => "3",
            WindowRoutingMode::FilteredRoutedNonFilteredCurrentWorkspace => "4",
        };
        let item = &self.configuration.state.workspaces[workspace.0];
        let foreground = self.event_foreground_hwnd;
        let focused = self
            .configuration
            .state
            .window_manager_find_client_in_workspaces_by_hwnd(foreground)
            .and_then(|id| self.configuration.state.clients[id.0].as_ref());
        let unmanaged = focused
            .is_none()
            .then(|| crate::snapshot::capture_foreground_window(foreground))
            .flatten();
        let data = focused
            .map(|client| &client.data)
            .or_else(|| unmanaged.as_ref().map(|window| &window.data));
        let process = data
            .and_then(|data| data.process_image_name.rsplit(['\\', '/']).next())
            .filter(|name| !name.is_empty())
            .unwrap_or("Unknown");
        let pid = data.map(|data| data.process_id).unwrap_or(0);
        let indicator = if focused.is_none() {
            " (F)"
        } else if self.is_foreground_window_same_as_selected {
            ""
        } else {
            " (*)"
        };
        let admin = if data.is_some_and(|data| data.is_elevated) {
            " (A)"
        } else {
            ""
        };
        Some(crate::bar::BarSnapshot {
            monitor_index,
            selected_monitor: monitor.selected,
            tags,
            workspace_info: format!(
                "[{}:{}][Mode:{}]",
                item.layout.tag(),
                item.clients.len(),
                mode
            ),
            focused_window: format!("{} ({}){}{}", process, pid, indicator, admin),
            segments: crate::bar::rendered_segments(&self.configuration.bar_segments),
            background_color: style.background_color,
            focus_background_color: style.focus_background_color,
            text_color: style.text_color,
            info_color: style.info_color,
            disabled_color: style.disabled_color,
            focus_text_color: style.focus_text_color,
            extra_focus_background_color: style.extra_focus_background_color,
            font_name: style.font_name.clone(),
            normal_font_size: style.normal_font_size,
            icon_font_size: style.icon_font_size,
        })
    }

    pub fn bar_workspace_clicked(&mut self, monitor: usize, workspace: usize) {
        if monitor >= self.configuration.state.monitors.len()
            || workspace >= self.configuration.state.workspaces.len()
        {
            return;
        }
        if self.configuration.state.monitors[monitor].is_hidden {
            return;
        }
        self.monitor_select(MonitorId(monitor));
        if let Err(error) = self.swap_selected_monitor_to(WorkspaceId(workspace)) {
            error.report();
        }
    }

    pub fn drag_drop_handle_location_change_with_mouse_down(&mut self, hwnd: isize) {
        if !hit_test_hwnd(hwnd) {
            return;
        }
        let (x, y) = match cursor_position() {
            Ok(position) => position,
            Err(error) => {
                error.report();
                return;
            }
        };
        let monitor = self
            .configuration
            .state
            .monitors
            .iter()
            .enumerate()
            .find(|(_, monitor)| {
                !monitor.is_hidden
                    && x > monitor.x_offset
                    && x < monitor.x_offset + monitor.width
                    && y > 0
                    && y < monitor.height
            })
            .map(|(index, _)| MonitorId(index));
        let Some(monitor) = monitor else {
            return;
        };
        let Some(workspace) = self.configuration.state.monitors[monitor.0].workspace else {
            return;
        };
        let mut target = self.configuration.state.workspaces[workspace.0]
            .clients
            .iter()
            .copied()
            .find(|id| {
                self.configuration.state.clients[id.0]
                    .as_ref()
                    .is_some_and(|client| {
                        client.data.hwnd != hwnd && hit_test_hwnd(client.data.hwnd)
                    })
            });
        if target.is_none()
            && let Some(selected) = self.configuration.state.workspaces[workspace.0].selected
            && let Some(client) = self.configuration.state.clients[selected.0].as_ref()
            && x > client.data.rect.left
            && x < client.data.rect.right
            && y > client.data.rect.top
            && y < client.data.rect.bottom
        {
            target = Some(selected);
        }
        if let Some(client) = self
            .configuration
            .state
            .window_manager_find_client_in_workspaces_by_hwnd(hwnd)
        {
            if target.is_some() {
                let expected = self.configuration.state.clients[client.0]
                    .as_ref()
                    .unwrap()
                    .data
                    .rect;
                match window_rect(hwnd) {
                    Ok(actual) => {
                        let pad = 10;
                        let left_changed = (actual.left - expected.left).abs() > pad;
                        let top_changed = (actual.top - expected.top).abs() > pad;
                        let right_changed = (actual.right - expected.right).abs() > pad;
                        let bottom_changed = (actual.bottom - expected.bottom).abs() > pad;
                        let moving =
                            (left_changed && right_changed) || (top_changed && bottom_changed);
                        if !moving {
                            self.regular_resize_client = Some(client);
                            return;
                        }
                    }
                    Err(error) => error.report(),
                }
            }
        } else {
            if has_float_styles(hwnd)
                || current_modifiers() != self.configuration.drag_drop_float_modifier
            {
                return;
            }
        }
        self.drag_drop_state.in_progress = target.is_some()
            || self.configuration.state.workspaces[workspace.0]
                .clients
                .is_empty();
        self.drag_drop_state.drag_hwnd = hwnd;
        self.drag_drop_state.drop_target_client = target;
        self.drag_drop_state.drop_target_monitor = Some(monitor);
        if self.drag_drop_state.in_progress {
            if let Some(overlay) = &self.drop_target_window {
                if let Some(target) = target {
                    if let Some(client) = self.configuration.state.clients[target.0].as_ref() {
                        crate::drop_target::show(overlay, client.data.rect, true, true);
                    }
                } else {
                    let item = &self.configuration.state.monitors[monitor.0];
                    let gap = item.gap_width;
                    crate::drop_target::show(
                        overlay,
                        Rect::from_xywh(
                            item.x_offset + gap,
                            item.top + gap,
                            item.width - gap * 2,
                            item.bottom - item.top - gap * 2,
                        ),
                        false,
                        self.configuration.state.menu_visible,
                    );
                }
            }
        }
    }

    pub fn drag_drop_try_handle_left_mouse_up(&mut self) -> PlatformResult<bool> {
        if !self.drag_drop_state.in_progress || self.drag_drop_state.drag_hwnd == 0 {
            if let Some(client) = self.regular_resize_client.take() {
                if let Some(workspace) = self.configuration.state.clients[client.0]
                    .as_ref()
                    .map(|client| client.workspace)
                {
                    self.apply_workspace(workspace)?;
                    return Ok(true);
                }
            }
            return Ok(false);
        }
        let hwnd = self.drag_drop_state.drag_hwnd;
        let (monitor, target) = self.drag_drop_target_at_cursor(hwnd);
        self.drag_drop_state = DragDropState::default();
        if let Some(overlay) = &self.drop_target_window {
            crate::drop_target::hide(overlay);
        }
        if target.is_some() && !self.is_root_window(hwnd) {
            if let Some(client) = self
                .configuration
                .state
                .window_manager_find_client_in_workspaces_by_hwnd(hwnd)
            {
                self.configuration.state.workspace_remove_client(client);
                self.configuration.state.clients[client.0] = None;
                if self.regular_resize_client == Some(client) {
                    self.regular_resize_client = None;
                }
            }
            return Ok(true);
        }
        let existing = self
            .configuration
            .state
            .window_manager_find_client_in_workspaces_by_hwnd(hwnd);
        if let (Some(client), Some(target)) = (existing, target) {
            let old = self.configuration.state.clients[client.0]
                .as_ref()
                .unwrap()
                .workspace;
            let destination = self.configuration.state.clients[target.0]
                .as_ref()
                .unwrap()
                .workspace;
            if old == destination {
                if self.configuration.state.workspaces[destination.0].layout
                    != crate::Layout::Monocle
                {
                    self.configuration
                        .state
                        .swap_client_nodes_data(client, target);
                }
                self.configuration.state.workspaces[destination.0].selected = Some(target);
            } else {
                self.configuration.state.workspace_remove_client(client);
                self.configuration.state.clients[client.0]
                    .as_mut()
                    .unwrap()
                    .workspace = destination;
                let position = self.configuration.state.workspaces[destination.0]
                    .clients
                    .iter()
                    .position(|id| *id == target)
                    .unwrap_or(0);
                self.configuration.state.workspaces[destination.0]
                    .clients
                    .insert(position, client);
                self.configuration.state.workspaces[destination.0].selected = Some(client);
                self.apply_workspace(old)?;
                if let Some(monitor) = self.configuration.state.workspaces[destination.0].monitor {
                    self.monitor_select(monitor);
                }
            }
            self.apply_workspace(destination)?;
            self.focus_selected(destination);
        } else if existing.is_none() {
            if let Some(target) = target {
                let destination = self.configuration.state.clients[target.0]
                    .as_ref()
                    .unwrap()
                    .workspace;
                if let Some(window) = crate::snapshot::capture_window(
                    hwnd,
                    true,
                    self.configuration.float_uwp_windows,
                ) {
                    let (floated, _) = is_float_window(&self.configuration, &window);
                    if !floated && window.data.process_id != 0 {
                        let mut data = window.data;
                        data.use_minimize_to_hide = client_should_use_minimize_to_hide(
                            &self.configuration,
                            &Client {
                                data: data.clone(),
                                is_visible: true,
                                workspace: destination,
                            },
                        );
                        if data.use_minimize_to_hide {
                            data.is_minimized = false;
                        }
                        let client = crate::ClientId(self.configuration.state.clients.len());
                        self.configuration.state.clients.push(Some(Client {
                            data,
                            is_visible: true,
                            workspace: destination,
                        }));
                        let position = self.configuration.state.workspaces[destination.0]
                            .clients
                            .iter()
                            .position(|id| *id == target)
                            .unwrap_or(0);
                        self.configuration.state.workspaces[destination.0]
                            .clients
                            .insert(position, client);
                        self.configuration.state.workspaces[destination.0].selected = Some(client);
                        if let Some(monitor) =
                            self.configuration.state.workspaces[destination.0].monitor
                        {
                            self.monitor_select(monitor);
                        }
                        self.apply_workspace(destination)?;
                        self.focus_selected(destination);
                    }
                }
            }
        } else if let (Some(client), Some(monitor)) = (existing, monitor) {
            let destination = self.configuration.state.monitors[monitor.0]
                .workspace
                .unwrap();
            let old = self.configuration.state.clients[client.0]
                .as_ref()
                .unwrap()
                .workspace;
            if old != destination
                && self.configuration.state.workspaces[destination.0]
                    .clients
                    .is_empty()
            {
                self.configuration.state.workspace_remove_client(client);
                self.configuration.state.clients[client.0]
                    .as_mut()
                    .unwrap()
                    .workspace = destination;
                self.configuration.state.workspaces[destination.0]
                    .clients
                    .insert(0, client);
                self.configuration.state.workspaces[destination.0].selected = Some(client);
                self.apply_workspace(old)?;
                self.apply_workspace(destination)?;
                self.monitor_select(monitor);
            } else {
                self.apply_workspace(old)?;
            }
        }
        Ok(true)
    }

    fn drag_drop_target_at_cursor(
        &self,
        hwnd: isize,
    ) -> (Option<MonitorId>, Option<crate::ClientId>) {
        let (x, y) = match cursor_position() {
            Ok(position) => position,
            Err(error) => {
                error.report();
                return (None, None);
            }
        };
        let monitor = self
            .configuration
            .state
            .monitors
            .iter()
            .enumerate()
            .find(|(_, monitor)| {
                !monitor.is_hidden
                    && x > monitor.x_offset
                    && x < monitor.x_offset + monitor.width
                    && y > 0
                    && y < monitor.height
            })
            .map(|(index, _)| MonitorId(index));
        let Some(monitor) = monitor else {
            return (None, None);
        };
        let Some(workspace) = self.configuration.state.monitors[monitor.0].workspace else {
            return (Some(monitor), None);
        };
        let mut target = self.configuration.state.workspaces[workspace.0]
            .clients
            .iter()
            .copied()
            .find(|id| {
                self.configuration.state.clients[id.0]
                    .as_ref()
                    .is_some_and(|client| {
                        client.data.hwnd != hwnd && hit_test_hwnd(client.data.hwnd)
                    })
            });
        if target.is_none()
            && let Some(selected) = self.configuration.state.workspaces[workspace.0].selected
            && let Some(client) = self.configuration.state.clients[selected.0].as_ref()
            && x > client.data.rect.left
            && x < client.data.rect.right
            && y > client.data.rect.top
            && y < client.data.rect.bottom
        {
            target = Some(selected);
        }
        (Some(monitor), target)
    }

    pub fn taskbar_changed(&mut self, taskbar: isize) -> PlatformResult<()> {
        let screen_height = primary_screen_size().1;
        let height = taskbar_height(taskbar);
        for monitor in &mut self.configuration.state.monitors {
            monitor.height = screen_height - height;
            monitor.bottom = screen_height - height;
        }
        self.apply_all_workspaces()?;
        if let Some(monitor) = self.configuration.state.selected_monitor
            && let Some(workspace) = self.configuration.state.monitors[monitor.0].workspace
        {
            self.focus_selected(workspace);
        }
        Ok(())
    }
}

fn is_iconic(hwnd: isize) -> bool {
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsIconic(hwnd as _) != 0 }
}

fn window_manager_try_add_window(
    configuration: &mut Configuration,
    window: WindowSnapshot,
    restore_zoomed: bool,
) {
    let (is_floated, reason) = is_float_window(configuration, &window);
    configuration
        .state
        .log_float_decision(crate::model::FloatLogEntry {
            timestamp: std::time::SystemTime::now(),
            hwnd: window.data.hwnd,
            process_id: window.data.process_id,
            process_image_name: window.data.process_image_name.clone(),
            class_name: window.data.class_name.clone(),
            title: window.data.title.clone(),
            is_floated,
            reason,
            styles: window.styles,
            ex_styles: window.ex_styles,
            window_width: window.normal_rect.right - window.normal_rect.left,
            window_height: window.normal_rect.bottom - window.normal_rect.top,
        });
    if is_floated {
        return;
    }
    let placeholder_workspace = WorkspaceId(0);
    let mut client = Client {
        data: window.data,
        is_visible: true,
        workspace: placeholder_workspace,
    };
    client.data.use_minimize_to_hide = client_should_use_minimize_to_hide(configuration, &client);
    if client.data.use_minimize_to_hide {
        client.data.is_minimized = false;
    }
    let workspace =
        window_manager_find_client_workspace_using_filters(&configuration.state, &client);
    if let Some(workspace) = workspace {
        let was_minimized = client.data.is_minimized;
        let data = client.data.clone();
        let logged_rect = match window_rect(data.hwnd) {
            Ok(rect) => rect,
            Err(error) => {
                error.report();
                window.normal_rect
            }
        };
        if restore_zoomed && is_zoomed(data.hwnd) {
            restore_window(data.hwnd);
        }
        configuration.state.client_add(workspace, client);
        configuration
            .state
            .log_client_addition(crate::model::ClientLogEntry {
                timestamp: std::time::SystemTime::now(),
                hwnd: data.hwnd,
                process_id: data.process_id,
                process_image_name: data.process_image_name,
                class_name: data.class_name,
                title: data.title,
                workspace_name: configuration.state.workspaces[workspace.0].name.clone(),
                was_minimized,
                styles: window.styles,
                ex_styles: window.ex_styles,
                window_width: logged_rect.right - logged_rect.left,
                window_height: logged_rect.bottom - logged_rect.top,
            });
    }
}

fn window_manager_find_client_workspace_using_filters(
    state: &WindowManagerState,
    client: &Client,
) -> Option<WorkspaceId> {
    let selected_workspace = state
        .selected_monitor
        .and_then(|monitor| state.monitors.get(monitor.0))
        .and_then(|monitor| monitor.workspace);
    if state.current_window_routing_mode == WindowRoutingMode::NotFilteredCurrentWorkspace {
        return selected_workspace;
    }
    for (index, workspace) in state.workspaces.iter().enumerate() {
        if workspace.filter_data.matches(client) {
            return if state.current_window_routing_mode
                == WindowRoutingMode::FilteredCurrentWorkspace
            {
                selected_workspace
            } else {
                Some(WorkspaceId(index))
            };
        }
    }
    if state.current_window_routing_mode
        == WindowRoutingMode::FilteredRoutedNonFilteredCurrentWorkspace
    {
        selected_workspace
    } else {
        None
    }
}

fn client_should_use_minimize_to_hide(configuration: &Configuration, _client: &Client) -> bool {
    configuration.minimize_to_hide_all
}

fn is_float_window(configuration: &Configuration, window: &WindowSnapshot) -> (bool, String) {
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_SIZEBOX,
        };
        if configuration
            .prevent_float_processes
            .iter()
            .any(|name| window.data.process_image_name.contains(name))
        {
            return (
                false,
                "Configuration function explicitly prevents floating".into(),
            );
        }
        if window.data.class_name.contains("ApplicationFrameWindow") {
            return (
                configuration.float_uwp_windows,
                format!(
                    "UWP window, floatUwpWindows={} (className: {})",
                    if configuration.float_uwp_windows {
                        "TRUE"
                    } else {
                        "FALSE"
                    },
                    window.data.class_name,
                ),
            );
        }
        let height = window.normal_rect.bottom - window.normal_rect.top;
        if height < configuration.non_float_window_height_minimum {
            return (
                true,
                format!(
                    "Window height ({height}) below minimum ({})",
                    configuration.non_float_window_height_minimum
                ),
            );
        }
        if window.ex_styles & WS_EX_APPWINDOW as i32 != 0 {
            return (false, "No floating criteria met - will be tiled".into());
        }
        let tool_window = window.ex_styles & WS_EX_TOOLWINDOW as i32 != 0;
        let no_size_box = window.styles & WS_SIZEBOX as i32 == 0;
        if tool_window || no_size_box {
            let mut reason = String::from("Has float styles:");
            if tool_window {
                reason.push_str(" TOOLWINDOW");
            }
            if no_size_box {
                reason.push_str(" NO_SIZEBOX");
            }
            (true, reason)
        } else {
            (false, "No floating criteria met - will be tiled".into())
        }
    }
}

fn primary_screen_size() -> (i32, i32) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) }
}

fn primary_taskbar_height() -> i32 {
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;
    let class: Vec<u16> = "Shell_TrayWnd\0".encode_utf16().collect();
    let hwnd = unsafe { FindWindowW(class.as_ptr(), std::ptr::null()) };
    if hwnd.is_null() {
        0
    } else {
        taskbar_height(hwnd as isize)
    }
}

fn foreground_window() -> isize {
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() as isize }
}

fn left_mouse_button_down() -> bool {
    unsafe {
        windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(
            windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_LBUTTON as i32,
        ) as u16
            & 0x8000
            != 0
    }
}
fn left_shift_down() -> bool {
    unsafe {
        windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(
            windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_LSHIFT as i32,
        ) as u16
            & 0x8000
            != 0
    }
}
fn cursor_position() -> PlatformResult<(i32, i32)> {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::POINT;
    let mut point: POINT = unsafe { zeroed() };
    check_bool(
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point) },
        "GetCursorPos",
    )?;
    Ok((point.x, point.y))
}
fn set_foreground_window(hwnd: isize) {
    // Windows may deny foreground activation as normal focus-stealing policy.
    // Callers already preserve their state and retry where appropriate.
    let _ = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd as _) };
}
fn is_window(hwnd: isize) -> bool {
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsWindow(hwnd as _) != 0 }
}

fn nudge_set_foreground_window(hwnd: isize) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::keybd_event;
    unsafe {
        keybd_event(0, 0, 0, 0);
        let _ = windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd as _);
    }
}

fn restore_window(hwnd: isize) {
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
            hwnd as _,
            windows_sys::Win32::UI::WindowsAndMessaging::SW_RESTORE,
        );
    }
}

fn show_unmanaged_window_on_selected_monitor(hwnd: isize, monitor: &Monitor) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, MoveWindow, SW_SHOWDEFAULT, ShowWindow,
    };
    set_foreground_window(hwnd);
    unsafe {
        ShowWindow(hwnd as _, SW_SHOWDEFAULT);
        if let Err(error) = check_bool(
            BringWindowToTop(hwnd as _),
            "BringWindowToTop(unmanaged window)",
        ) {
            error.report();
        }
    }
    let rect = match window_rect(hwnd) {
        Ok(rect) => rect,
        Err(error) => {
            error.report();
            return;
        }
    };
    if rect.left > monitor.x_offset + monitor.width || rect.left < monitor.x_offset {
        unsafe {
            if let Err(error) = check_bool(
                MoveWindow(
                    hwnd as _,
                    monitor.x_offset + monitor.gap_width * 2,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    1,
                ),
                "MoveWindow(unmanaged window)",
            ) {
                error.report();
            }
        }
    }
}

fn platform_exit() {
    unsafe {
        windows_sys::Win32::System::Threading::ExitProcess(0);
    }
}
fn close_focused_window() {
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(
            windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow(),
            windows_sys::Win32::UI::WindowsAndMessaging::WM_CLOSE,
            0,
            0,
        );
    }
}
fn kill_focused_window() -> Result<(), u32> {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };
    let mut process_id = 0;
    if unsafe { GetWindowThreadProcessId(GetForegroundWindow(), &mut process_id) } == 0 {
        return Err(crate::error_log::last_win32(
            "GetWindowThreadProcessId(focused window) failed",
        ));
    }
    let process = unsafe { OpenProcess(PROCESS_TERMINATE, 0, process_id) };
    if process.is_null() {
        let error = unsafe { GetLastError() };
        crate::error_log::report_win32("OpenProcess(PROCESS_TERMINATE) failed", error);
        return Err(error);
    }
    let result = unsafe { TerminateProcess(process, 1) };
    let terminate_error = (result == 0).then(|| unsafe { GetLastError() });
    if unsafe { CloseHandle(process) } == 0 {
        crate::error_log::last_win32("CloseHandle(terminated process) failed");
    }
    if result == 0 {
        let error = terminate_error.unwrap();
        crate::error_log::report_win32("TerminateProcess failed", error);
        Err(error)
    } else {
        Ok(())
    }
}
fn minimize_focused_window() {
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
            windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow(),
            windows_sys::Win32::UI::WindowsAndMessaging::SW_MINIMIZE,
        );
    }
}
fn redraw_focused_window() {
    if unsafe {
        windows_sys::Win32::Graphics::Gdi::RedrawWindow(
            windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow(),
            std::ptr::null(),
            std::ptr::null_mut(),
            windows_sys::Win32::Graphics::Gdi::RDW_ERASE
                | windows_sys::Win32::Graphics::Gdi::RDW_INVALIDATE,
        )
    } == 0
    {
        crate::error_log::last_win32("RedrawWindow(focused window) failed");
    }
}
fn taskbar_toggle() {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let class: Vec<u16> = "Shell_TrayWnd\0".encode_utf16().collect();
    unsafe {
        let hwnd = FindWindowW(class.as_ptr(), std::ptr::null());
        if hwnd.is_null() {
            return;
        }
        let visible = runtime_window_long(hwnd as isize, GWL_STYLE) & WS_VISIBLE as i32 != 0;
        ShowWindow(hwnd, if visible { SW_HIDE } else { SW_SHOW });
        let secondary: Vec<u16> = "Shell_SecondaryTrayWnd\0".encode_utf16().collect();
        ShowWindow(
            FindWindowW(secondary.as_ptr(), std::ptr::null()),
            if visible { SW_HIDE } else { SW_SHOW },
        );
    }
}
fn taskbar_height(hwnd: isize) -> i32 {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        if runtime_window_long(hwnd, GWL_STYLE) & WS_VISIBLE as i32 == 0 {
            return 0;
        }
        let mut rect: RECT = zeroed();
        if GetWindowRect(hwnd as _, &mut rect) == 0 {
            crate::error_log::last_win32("GetWindowRect(taskbar) failed");
            0
        } else {
            rect.bottom - rect.top
        }
    }
}
fn stop_managing_window(hwnd: isize, monitor: &Monitor) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SWP_SHOWWINDOW, SetForegroundWindow, SetWindowPos,
    };
    unsafe {
        if SetWindowPos(
            hwnd as _,
            std::ptr::null_mut(),
            monitor.x_offset,
            0,
            monitor.width,
            monitor.height,
            SWP_SHOWWINDOW,
        ) == 0
        {
            crate::error_log::last_win32("SetWindowPos(stop managing) failed");
        }
        if SetForegroundWindow(hwnd as _) == 0 {
            crate::error_log::report(
                "SetForegroundWindow(stop managing) was denied",
                "returned zero",
            );
        }
    }
}
fn move_window_by(hwnd: isize, dx: i32, dy: i32) {
    use windows_sys::Win32::UI::WindowsAndMessaging::MoveWindow;
    let rect = match window_rect(hwnd) {
        Ok(rect) => rect,
        Err(error) => {
            error.report();
            return;
        }
    };
    if let Err(error) = check_bool(
        unsafe {
            MoveWindow(
                hwnd as _,
                rect.left + dx,
                rect.top + dy,
                rect.right - rect.left,
                rect.bottom - rect.top,
                0,
            )
        },
        "MoveWindow(offset)",
    ) {
        error.report();
    }
}

fn center_window_on_monitor(hwnd: isize, monitor: &Monitor) {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowRect, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
    };
    let mut rect: RECT = unsafe { zeroed() };
    if unsafe { GetWindowRect(hwnd as _, &mut rect) } != 0 {
        let x = monitor.x_offset + (monitor.width - (rect.right - rect.left)) / 2;
        let y = (monitor.height - (rect.bottom - rect.top)) / 2;
        unsafe {
            if SetWindowPos(
                hwnd as _,
                std::ptr::null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER,
            ) == 0
            {
                crate::error_log::last_win32("SetWindowPos(center window) failed");
            }
        }
    } else {
        crate::error_log::last_win32("GetWindowRect(center window) failed");
    }
}

fn window_rect(hwnd: isize) -> PlatformResult<Rect> {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;
    let mut rect: RECT = unsafe { zeroed() };
    check_bool(
        unsafe { GetWindowRect(hwnd as _, &mut rect) },
        "GetWindowRect",
    )?;
    Ok(Rect {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    })
}

fn hit_test_hwnd(hwnd: isize) -> bool {
    let rect = match window_rect(hwnd) {
        Ok(rect) => rect,
        Err(error) => {
            error.report();
            return false;
        }
    };
    let (x, y) = match cursor_position() {
        Ok(position) => position,
        Err(error) => {
            error.report();
            return false;
        }
    };
    x > rect.left && x < rect.right && y > rect.top && y < rect.bottom
}

fn current_modifiers() -> i32 {
    use crate::commands::{LALT, LCTL, LSHIFT, LWIN, RALT, RSHIFT, RWIN};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    let down = |key| unsafe { GetAsyncKeyState(key) as u16 & 0x8000 != 0 };
    let mut value = 0;
    if down(VK_LSHIFT as i32) {
        value |= LSHIFT;
    }
    if down(VK_RSHIFT as i32) {
        value |= RSHIFT;
    }
    if down(VK_LMENU as i32) {
        value |= LALT;
    }
    if down(VK_RMENU as i32) {
        value |= RALT;
    }
    if down(VK_CONTROL as i32) {
        value |= LCTL;
    }
    if down(VK_LWIN as i32) {
        value |= LWIN;
    }
    if down(VK_RWIN as i32) {
        value |= RWIN;
    }
    value
}

fn has_float_styles(hwnd: isize) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let styles = runtime_window_long(hwnd, GWL_STYLE);
    let ex_styles = runtime_window_long(hwnd, GWL_EXSTYLE);
    if ex_styles & WS_EX_APPWINDOW as i32 != 0 {
        false
    } else {
        ex_styles & WS_EX_TOOLWINDOW as i32 != 0 || styles & WS_SIZEBOX as i32 == 0
    }
}

fn runtime_window_long(hwnd: isize, index: i32) -> i32 {
    use windows_sys::Win32::Foundation::{GetLastError, SetLastError};
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongW;
    unsafe {
        SetLastError(0);
        let value = GetWindowLongW(hwnd as _, index);
        let error = GetLastError();
        if value == 0 && error != 0 {
            crate::error_log::report_win32(format!("GetWindowLongW({index}) failed"), error);
        }
        value
    }
}

fn is_zoomed(hwnd: isize) -> bool {
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsZoomed(hwnd as _) != 0 }
}

fn is_fullscreen(hwnd: isize) -> bool {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromWindow,
    };
    let rect = match window_rect(hwnd) {
        Ok(rect) => rect,
        Err(error) => {
            error.report();
            return false;
        }
    };
    let mut info: MONITORINFO = unsafe { zeroed() };
    info.cbSize = size_of::<MONITORINFO>() as u32;
    let monitor = unsafe { MonitorFromWindow(hwnd as _, MONITOR_DEFAULTTOPRIMARY) };
    if monitor.is_null() {
        crate::error_log::report("MonitorFromWindow failed", "returned a null monitor");
        return false;
    }
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        crate::error_log::last_win32("GetMonitorInfoW(fullscreen check) failed");
        return false;
    }
    rect.left == info.rcMonitor.left
        && rect.top == info.rcMonitor.top
        && rect.right == info.rcMonitor.right
        && rect.bottom == info.rcMonitor.bottom
}

#[tracing::instrument(
    skip(state),
    fields(workspace = workspace_id.0, always_redraw, hidden_x, use_old_move_logic)
)]
fn apply_workspace_windows(
    state: &mut WindowManagerState,
    workspace_id: WorkspaceId,
    always_redraw: bool,
    hidden_x: i32,
    use_old_move_logic: bool,
) -> PlatformResult<()> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::{HWND, RECT};
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BeginDeferWindowPos, DeferWindowPos, EndDeferWindowPos, GetWindowRect, HWND_BOTTOM,
        HWND_NOTOPMOST, HWND_TOPMOST, MoveWindow, SW_RESTORE, SWP_NOMOVE, SWP_NOREDRAW, SWP_NOSIZE,
        SWP_SHOWWINDOW, SetWindowPos, ShowWindow,
    };
    let workspace = &state.workspaces[workspace_id.0];
    let workspace_hidden = workspace
        .monitor
        .is_some_and(|monitor| state.monitors[monitor.0].is_hidden);
    let mut defer = unsafe { BeginDeferWindowPos(workspace.clients.len() as i32 + 1) };
    if defer.is_null() {
        return Err(PlatformError::last_win32("BeginDeferWindowPos"));
    }
    for client_id in workspace.clients.iter().rev() {
        let client = state.clients[client_id.0].as_mut().unwrap();
        if !use_old_move_logic
            && client.data.use_minimize_to_hide
            && (workspace_hidden || !client.is_visible)
        {
            if let Err(error) = crate::cloak::set_cloak_for_window(client.data.hwnd, 1, 2) {
                error.report();
            }
            continue;
        }
        let hwnd = client.data.hwnd as HWND;
        let mut window_rect: RECT = unsafe { zeroed() };
        if let Err(error) = check_bool(
            unsafe { GetWindowRect(hwnd, &mut window_rect) },
            "GetWindowRect(arrange workspace)",
        ) {
            error.report();
            continue;
        }
        let mut frame_rect = window_rect;
        let frame_result = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS as u32,
                &mut frame_rect as *mut _ as _,
                size_of::<RECT>() as u32,
            )
        };
        if let Err(error) = check_hresult(
            frame_result,
            "DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)",
        ) {
            error.report();
            frame_rect = window_rect;
        }
        let left_border = frame_rect.left - window_rect.left;
        let right_border = window_rect.right - frame_rect.right;
        let top_border = window_rect.top - frame_rect.top;
        let bottom_border = window_rect.bottom - frame_rect.bottom;
        let mut target_left = client.data.rect.left - left_border;
        if !client.is_visible {
            target_left = hidden_x;
        }
        let target_top = client.data.rect.top - top_border;
        let target_width =
            client.data.rect.right - client.data.rect.left + left_border + right_border;
        let target_height =
            client.data.rect.bottom - client.data.rect.top + top_border + bottom_border;
        let mut cloaked = 0u32;
        let cloak_result = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_CLOAKED as u32,
                &mut cloaked as *mut _ as _,
                size_of::<u32>() as u32,
            )
        };
        if let Err(error) = check_hresult(cloak_result, "DwmGetWindowAttribute(DWMWA_CLOAKED)") {
            error.report();
            cloaked = 0;
        }
        if window_matches_target(
            Rect {
                left: window_rect.left,
                top: window_rect.top,
                right: window_rect.right,
                bottom: window_rect.bottom,
            },
            target_left,
            target_top,
            target_width,
            target_height,
            cloaked != 0,
        ) {
            continue;
        }
        if use_old_move_logic {
            unsafe {
                if let Err(error) = check_bool(
                    MoveWindow(
                        hwnd,
                        target_left,
                        target_top,
                        target_width,
                        target_height,
                        0,
                    ),
                    "MoveWindow(arrange workspace)",
                ) {
                    error.report();
                    continue;
                }
                // ShowWindow reports the window's previous visibility, not
                // success or failure, so a zero return is not an error.
                ShowWindow(hwnd, SW_RESTORE);
                if client.is_visible {
                    if let Err(error) = check_bool(
                        SetWindowPos(
                            hwnd,
                            HWND_TOPMOST,
                            0,
                            0,
                            0,
                            0,
                            SWP_SHOWWINDOW | SWP_NOSIZE | SWP_NOMOVE,
                        ),
                        "SetWindowPos(set topmost)",
                    ) {
                        error.report();
                    }
                    if let Err(error) = check_bool(
                        SetWindowPos(
                            hwnd,
                            HWND_NOTOPMOST,
                            0,
                            0,
                            0,
                            0,
                            SWP_SHOWWINDOW | SWP_NOSIZE | SWP_NOMOVE,
                        ),
                        "SetWindowPos(clear topmost)",
                    ) {
                        error.report();
                    }
                } else {
                    if let Err(error) = check_bool(
                        SetWindowPos(
                            hwnd,
                            HWND_BOTTOM,
                            0,
                            0,
                            0,
                            0,
                            SWP_SHOWWINDOW | SWP_NOSIZE | SWP_NOMOVE,
                        ),
                        "SetWindowPos(hidden window z-order)",
                    ) {
                        error.report();
                    }
                }
            }
            continue;
        }
        let flags = if always_redraw {
            SWP_SHOWWINDOW
        } else {
            SWP_NOREDRAW
        };
        defer = unsafe {
            DeferWindowPos(
                defer,
                hwnd,
                std::ptr::null_mut(),
                target_left,
                target_top,
                target_width,
                target_height,
                flags,
            )
        };
        if defer.is_null() {
            // A failed DeferWindowPos invalidates the batch handle, so it is
            // unsafe to queue additional moves or try to commit the batch.
            return Err(PlatformError::last_win32("DeferWindowPos"));
        }
        if client.data.use_minimize_to_hide && !workspace_hidden {
            if let Err(error) = crate::cloak::set_cloak_for_window(client.data.hwnd, 1, 0) {
                error.report();
            }
        }
    }
    check_bool(unsafe { EndDeferWindowPos(defer) }, "EndDeferWindowPos")?;
    Ok(())
}

#[cfg(test)]
mod parity_tests {
    use super::*;

    #[test]
    fn unchanged_visible_window_is_not_moved_again() {
        let actual = Rect::from_xywh(10, 20, 300, 400);
        assert!(window_matches_target(actual, 10, 20, 300, 400, false));
        assert!(!window_matches_target(actual, 10, 20, 300, 400, true));
        assert!(!window_matches_target(actual, 11, 20, 300, 400, false));
    }
}

fn window_matches_target(
    actual: Rect,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    cloaked: bool,
) -> bool {
    actual.left == left
        && actual.top == top
        && actual.right == left + width
        && actual.bottom == top + height
        && !cloaked
}
