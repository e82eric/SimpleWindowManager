use crate::commands::{Action, KeyBinding};
use crate::model::WindowRoutingMode;
use crate::{Layout, WindowManagerState, WorkspaceId};

#[derive(Clone, Debug)]
pub struct TextStyle {
    pub font_name: String,
    pub normal_font_size: i32,
    pub icon_font_size: i32,
    pub text_color: u32,
    pub background_color: u32,
    pub disabled_color: u32,
    pub focus_background_color: u32,
    pub focus_text_color: u32,
    pub focus_color2: u32,
    pub lost_focus_color: u32,
    pub extra_focus_background_color: u32,
    pub info_color: u32,
    pub border_width: i32,
}

#[derive(Clone, Copy, Debug)]
pub enum BarSegmentValue {
    SystemTime,
    LocalTime,
    LocalDate,
    Cpu,
    Memory,
    Volume,
    Internet,
}

#[derive(Clone, Debug)]
pub struct BarSegmentConfiguration {
    pub separator_text: Option<String>,
    pub separator_is_icon: bool,
    pub header_text: Option<String>,
    pub header_is_icon: bool,
    pub variable_text_fixed_width: usize,
    pub variable_is_icon: bool,
    pub value: BarSegmentValue,
}

#[derive(Clone, Debug)]
pub struct CommandEntry {
    pub name: String,
    pub action: Action,
    pub modifiers: Option<i32>,
    pub key: Option<u32>,
}

/// Monitor used to center all SWM pickers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PickerMonitor {
    /// Follow Configuration.primary_monitor, with the first visible monitor as fallback.
    Primary,
    /// Zero-based index in SWM's monitor list, rather than a Windows display number.
    Fixed(usize),
    /// Follow SWM's focused monitor each time a picker opens.
    Focused,
}

impl Default for PickerMonitor {
    fn default() -> Self {
        Self::Primary
    }
}

#[derive(Clone)]
pub struct Configuration {
    /// Zero-based SWM monitor index used as the primary monitor. Not a Windows display number.
    pub primary_monitor: usize,
    /// Zero-based SWM monitor index targeted by secondary-monitor actions.
    pub secondary_monitor: usize,
    pub state: WindowManagerState,
    pub always_redraw: bool,
    /// Mirrors whether the C `useOldMoveLogicFunc` function pointer is configured.
    /// The C mover currently tests the pointer twice instead of invoking it, so a
    /// configured callback selects the old mover for every client.
    pub use_old_move_logic_configured: bool,
    pub non_float_window_height_minimum: i32,
    pub float_uwp_windows: bool,
    pub bar_height: i32,
    pub key_bindings: Vec<KeyBinding>,
    pub commands: Vec<CommandEntry>,
    pub text_style: Option<TextStyle>,
    pub bar_segments: Vec<BarSegmentConfiguration>,
    /// Every registered picker, for listing; bindings refer to pickers directly.
    pub pickers: Vec<&'static crate::picker::PickerConfig>,
    pub float_window_movement: i32,
    pub drag_drop_float_modifier: i32,
    pub bar_right_padding: i32,
    pub prevent_float_processes: Vec<String>,
    /// Executable basenames excluded from workspace management in every routing mode.
    /// Matches are exact and ASCII case-insensitive, including full process paths.
    pub always_exclude_processes: Vec<String>,
    pub cloak_to_hide_all: bool,
    /// Maximum visible picker results (clamped to 1..=100).
    pub picker_result_rows: usize,
    /// Target for all pickers. Unavailable/hidden targets fall back to the first visible monitor.
    pub picker_monitor: PickerMonitor,
    /// Maximum visible preview rows (clamped to 1..=256).
    pub picker_preview_rows: usize,
    /// Bat syntax theme for file previews; None uses bat's own settings.
    pub file_preview_bat_theme: Option<String>,
    /// Host-owned routing for filesystem/program previews. Runs on the NFM worker.
    pub file_preview_resolver: crate::file_preview::FilePreviewResolver,
}

impl Default for Configuration {
    fn default() -> Self {
        let mut state = WindowManagerState::default();
        state.current_window_routing_mode =
            WindowRoutingMode::FilteredRoutedNonFilteredCurrentWorkspace;
        Self {
            primary_monitor: 0,
            secondary_monitor: 1,
            state,
            always_redraw: false,
            use_old_move_logic_configured: false,
            non_float_window_height_minimum: 500,
            float_uwp_windows: false,
            bar_height: 29,
            key_bindings: Vec::new(),
            commands: Vec::new(),
            text_style: None,
            bar_segments: Vec::new(),
            pickers: crate::builtin_pickers::defaults(),
            float_window_movement: 75,
            drag_drop_float_modifier: crate::commands::LALT,
            bar_right_padding: 10,
            prevent_float_processes: Vec::new(),
            always_exclude_processes: Vec::new(),
            cloak_to_hide_all: false,
            picker_result_rows: 7,
            picker_monitor: PickerMonitor::default(),
            picker_preview_rows: 14,
            file_preview_bat_theme: Some("gruvbox-dark".into()),
            file_preview_resolver: crate::file_preview::resolver(Default::default()),
        }
    }
}

impl Configuration {
    /// Register a picker for the life of the process; open the returned picker with
    /// `Action::ShowPicker`.
    pub fn register_picker(
        &mut self,
        picker: crate::picker::PickerConfig,
    ) -> &'static crate::picker::PickerConfig {
        let picker = Box::leak(Box::new(picker));
        self.pickers.push(picker);
        picker
    }
    pub(crate) fn selected_picker_monitor(&self) -> Option<&crate::Monitor> {
        let monitors = &self.state.monitors;
        let index = match self.picker_monitor {
            PickerMonitor::Primary => Some(self.primary_monitor),
            PickerMonitor::Fixed(index) => Some(index),
            PickerMonitor::Focused => self.state.selected_monitor.map(|monitor| monitor.0),
        };
        let usable = |monitor: &&crate::Monitor| {
            !monitor.is_hidden && monitor.width > 0 && monitor.bottom > monitor.top
        };
        index
            .and_then(|index| monitors.get(index))
            .filter(usable)
            .or_else(|| monitors.iter().find(usable))
    }

    pub fn workspace_register(
        &mut self,
        name: &str,
        tag: &str,
        is_icon: bool,
        layout: Layout,
    ) -> WorkspaceId {
        self.state.workspace_register(name, tag, is_icon, layout)
    }
    pub fn workspace_register_processimagename_contains_filter(
        &mut self,
        workspace: WorkspaceId,
        filter: &str,
    ) {
        self.state.workspaces[workspace.0]
            .filter_data
            .process_image_names
            .push(filter.into());
    }
    pub fn workspace_register_classname_contains_filter(
        &mut self,
        workspace: WorkspaceId,
        filter: &str,
    ) {
        self.state.workspaces[workspace.0]
            .filter_data
            .class_names
            .push(filter.into());
    }
    pub fn workspace_register_title_contains_filter(
        &mut self,
        workspace: WorkspaceId,
        filter: &str,
    ) {
        self.state.workspaces[workspace.0]
            .filter_data
            .titles
            .push(filter.into());
    }
    pub fn workspace_register_title_not_contains_filter(
        &mut self,
        workspace: WorkspaceId,
        filter: &str,
    ) {
        self.state.workspaces[workspace.0]
            .filter_data
            .not_titles
            .push(filter.into());
    }

    pub fn keybindings_register_defaults_with_modifiers(
        &mut self,
        modifiers: i32,
        f_modifiers: i32,
    ) {
        for binding in
            crate::commands::keybindings_register_defaults_with_modifiers(modifiers, f_modifiers)
        {
            self.keybinding_create(binding);
        }
        self.command_create("swap_selected_monitor_to_tile_layout", Action::SetTile);
        self.command_create(
            "swap_selected_monitor_to_tile_layout_reversed",
            Action::SetTileReversed,
        );
        self.command_create(
            "ErrorLogsMenu",
            Action::ShowPicker(&crate::builtin_pickers::ERROR_LOGS),
        );
        self.command_create(
            "ConfigurationStateMenu",
            Action::ShowPicker(&crate::builtin_pickers::CONFIGURATION_STATE),
        );
        self.command_create("ToggleDebugOutput", Action::ToggleDebugOutput);
    }

    pub fn keybinding_create(&mut self, binding: KeyBinding) {
        self.commands.push(CommandEntry {
            name: crate::platform::binding_name(&binding),
            action: binding.action,
            modifiers: Some(binding.modifiers),
            key: Some(binding.key),
        });
        if let Some(existing) = self
            .key_bindings
            .iter_mut()
            .find(|item| item.key == binding.key && item.modifiers == binding.modifiers)
        {
            *existing = binding;
        } else {
            self.key_bindings.push(binding);
        }
    }

    pub fn command_create(&mut self, name: &str, action: Action) {
        self.commands.push(CommandEntry {
            name: name.into(),
            action,
            modifiers: None,
            key: None,
        });
    }

    pub fn configuration_register_default_text_style(
        &mut self,
        font_name: &str,
        normal_font_size: i32,
        icon_font_size: i32,
    ) {
        self.text_style = Some(TextStyle {
            font_name: font_name.into(),
            normal_font_size,
            icon_font_size,
            text_color: 0xb2dbeb,
            background_color: 0x282828,
            disabled_color: 0x748392,
            focus_background_color: 0x504945,
            focus_text_color: 0x1d24cc,
            focus_color2: 0x2fbdFA,
            lost_focus_color: 0x7cc08e,
            extra_focus_background_color: 0x1980fe,
            info_color: 0x98a583,
            border_width: 10,
        });
    }

    pub fn configuration_add_bar_segment(
        &mut self,
        separator: &str,
        width: usize,
        is_icon: bool,
        value: BarSegmentValue,
    ) {
        self.bar_segments.push(BarSegmentConfiguration {
            separator_text: Some(separator.into()),
            separator_is_icon: false,
            header_text: None,
            header_is_icon: false,
            variable_text_fixed_width: width,
            variable_is_icon: is_icon,
            value,
        });
    }

    pub fn configuration_add_bar_segment_with_header(
        &mut self,
        separator: &str,
        header: &str,
        width: usize,
        is_icon: bool,
        value: BarSegmentValue,
    ) {
        self.bar_segments.push(BarSegmentConfiguration {
            separator_text: Some(separator.into()),
            separator_is_icon: false,
            header_text: Some(header.into()),
            header_is_icon: false,
            variable_text_fixed_width: width,
            variable_is_icon: is_icon,
            value,
        });
    }
}
