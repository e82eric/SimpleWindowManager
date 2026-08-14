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

#[derive(Clone, Debug, Default)]
pub struct ProgramLauncherMenu {
    pub directories: Vec<String>,
    pub is_elevated: bool,
}

#[derive(Clone, Debug)]
pub struct CommandEntry {
    pub name: String,
    pub action: Action,
    pub modifiers: Option<i32>,
    pub key: Option<u32>,
}

pub struct Configuration {
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
    pub program_launchers: Vec<ProgramLauncherMenu>,
    pub float_window_movement: i32,
    pub drag_drop_float_modifier: i32,
    pub bar_right_padding: i32,
    pub prevent_float_processes: Vec<String>,
    pub minimize_to_hide_all: bool,
}

impl Default for Configuration {
    fn default() -> Self {
        let mut state = WindowManagerState::default();
        state.current_window_routing_mode =
            WindowRoutingMode::FilteredRoutedNonFilteredCurrentWorkspace;
        Self {
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
            program_launchers: Vec::new(),
            float_window_movement: 75,
            drag_drop_float_modifier: crate::commands::LALT,
            bar_right_padding: 10,
            prevent_float_processes: Vec::new(),
            minimize_to_hide_all: false,
        }
    }
}

impl Configuration {
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
        self.command_create("ErrorLogsMenu", Action::ShowErrorLogs);
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
            disabled_color: 0x504945,
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

    pub fn register_program_launcher_menu(&mut self, is_elevated: bool) -> usize {
        let id = self.program_launchers.len();
        self.program_launchers.push(ProgramLauncherMenu {
            directories: Vec::new(),
            is_elevated,
        });
        id
    }

    pub fn program_launcher_add_directory(&mut self, menu: usize, directory: &str) {
        self.program_launchers[menu]
            .directories
            .push(directory.into());
    }
}

pub fn configure(configuration: &mut Configuration) {
    use crate::commands::{LALT, LCTL, LSHIFT, LWIN};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    let modifiers = LALT | LWIN | LCTL;
    configuration.keybindings_register_defaults_with_modifiers(modifiers, LCTL | LALT);
    configuration.configuration_register_default_text_style("JetBrainsMonoNL NFP Regular", 16, 16);
    configuration.always_redraw = true;
    configuration.minimize_to_hide_all = true;
    configuration
        .prevent_float_processes
        .push("wezterm-gui.exe".into());
    configuration.state.current_window_routing_mode =
        WindowRoutingMode::FilteredRoutedNonFilteredCurrentWorkspace;
    let browser = configuration.workspace_register("Chrome", "\u{eb01}", true, Layout::Deck);
    for process in ["chrome.exe", "brave.exe", "firefox.exe", "msedge.exe"] {
        configuration.workspace_register_processimagename_contains_filter(browser, process);
    }
    configuration.workspace_register_title_not_contains_filter(browser, "ChromeApp");
    let terminal = configuration.workspace_register("Terminal", "\u{f120}", true, Layout::Deck);
    configuration
        .workspace_register_processimagename_contains_filter(terminal, "WindowsTerminal.exe");
    configuration.workspace_register_processimagename_contains_filter(terminal, "wezterm-gui.exe");
    let ide = configuration.workspace_register("Rider", "\u{eb0f}", true, Layout::Monocle);
    for process in ["rider64.exe", "Code.exe", "devenv.exe"] {
        configuration.workspace_register_processimagename_contains_filter(ide, process);
    }
    let arch = configuration.workspace_register("ArchVM", "\u{f303}", true, Layout::Deck);
    configuration.workspace_register_title_contains_filter(arch, "Arch");
    configuration.workspace_register_title_not_contains_filter(arch, "Arch Clone");
    let windows = configuration.workspace_register("Windows10VM", "\u{f17a}", true, Layout::Deck);
    configuration.workspace_register_title_contains_filter(windows, "Windows10VM");
    let teams = configuration.workspace_register("Teams", "\u{f075}", true, Layout::Deck);
    configuration.workspace_register_processimagename_contains_filter(teams, "Teams.exe");
    let chat = configuration.workspace_register("7", "7", false, Layout::Deck);
    configuration.workspace_register_title_contains_filter(chat, "ChromeApp_ChatGPT");
    let virtualbox = configuration.workspace_register("8", "8", false, Layout::Deck);
    configuration.workspace_register_title_contains_filter(virtualbox, "Oracle VirtualBox Manager");
    let arch_clone = configuration.workspace_register("9", "9", false, Layout::Deck);
    configuration.workspace_register_title_contains_filter(arch_clone, "Arch Clone");
    configuration.workspace_register("Desktop", "0", false, Layout::Deck);
    for (key, action) in [
        (VK_SPACE as u32, Action::ShowWindows),
        (VK_F15 as u32, Action::ShowProcesses),
        (VK_F17 as u32, Action::ShowCommands),
    ] {
        configuration.keybinding_create(KeyBinding {
            modifiers,
            key,
            action,
        });
    }
    configuration.keybinding_create(KeyBinding {
        modifiers: LALT,
        key: VK_F8 as u32,
        action: Action::ShowFloatLogs,
    });
    configuration.keybinding_create(KeyBinding {
        modifiers: LALT,
        key: VK_F9 as u32,
        action: Action::ShowClientLogs,
    });
    let not_elevated = configuration.register_program_launcher_menu(false);
    let elevated = configuration.register_program_launcher_menu(true);
    configuration.keybinding_create(KeyBinding {
        modifiers,
        key: b'P' as u32,
        action: Action::ShowPrograms(false),
    });
    configuration.keybinding_create(KeyBinding {
        modifiers: modifiers | LSHIFT,
        key: b'P' as u32,
        action: Action::ShowPrograms(true),
    });
    configuration.keybinding_create(KeyBinding {
        modifiers,
        key: VK_F16 as u32,
        action: Action::ShowFileSystem,
    });
    for index in 0..9 {
        configuration.keybinding_create(KeyBinding {
            modifiers,
            key: VK_F1 as u32 + index as u32,
            action: Action::MoveWorkspaceToMonitor(index, 1),
        });
    }
    for index in 0..9 {
        configuration.keybinding_create(KeyBinding {
            modifiers: LALT,
            key: VK_F13 as u32 + index as u32,
            action: Action::MoveWorkspaceToMonitor(index, 1),
        });
    }
    configuration.keybinding_create(KeyBinding {
        modifiers: modifiers | LSHIFT,
        key: VK_RETURN as u32,
        action: Action::MoveSecondaryFocusedToMain,
    });
    for directory in [
        "C:\\users\\e82er\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\",
        "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\",
        "C:\\users\\e82er\\AppData\\Local\\Microsoft\\WindowsApps\\",
        "C:\\users\\e82er\\utilities\\",
        "C:\\users\\e82er\\.claude\\",
    ] {
        configuration.program_launcher_add_directory(not_elevated, directory);
    }
    for directory in [
        "C:\\users\\e82er\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\",
        "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\",
        "C:\\users\\e82er\\AppData\\Local\\Microsoft\\WindowsApps\\",
        "C:\\Program Files\\sysinternals\\",
        "C:\\users\\e82er\\utilities\\",
    ] {
        configuration.program_launcher_add_directory(elevated, directory);
    }
    use BarSegmentValue::*;
    configuration.configuration_add_bar_segment(" | ", 5, false, SystemTime);
    configuration.configuration_add_bar_segment(" | ", 5, false, LocalTime);
    configuration.configuration_add_bar_segment(" | ", 10, false, LocalDate);
    configuration.configuration_add_bar_segment_with_header(" | ", "CPU:", 3, false, Cpu);
    configuration.configuration_add_bar_segment_with_header(" | ", "RAM:", 3, false, Memory);
    configuration.configuration_add_bar_segment_with_header(" | ", "\u{f028}", 3, false, Volume);
    configuration.configuration_add_bar_segment(" | ", 1, true, Internet);
    for (key, (dx, dy)) in [
        (VK_RIGHT as u32, (1, 0)),
        (VK_LEFT as u32, (-1, 0)),
        (VK_UP as u32, (0, -1)),
        (VK_DOWN as u32, (0, 1)),
    ] {
        configuration.keybinding_create(KeyBinding {
            modifiers: LWIN,
            key,
            action: Action::MoveFloat(dx, dy),
        });
    }
    let display_count = display_monitor_count();
    for index in 0..display_count {
        configuration.keybinding_create(KeyBinding {
            modifiers: LWIN,
            key: b'1' as u32 + index as u32,
            action: Action::MoveFloatToMonitor(index),
        });
    }
}

#[cfg(windows)]
fn display_monitor_count() -> usize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CMONITORS};
    unsafe { GetSystemMetrics(SM_CMONITORS).max(0) as usize }
}

#[cfg(not(windows))]
fn display_monitor_count() -> usize {
    1
}

#[cfg(test)]
mod parity_tests {
    use super::*;

    #[test]
    fn production_configuration_matches_workspace_contract() {
        let mut configuration = Configuration::default();
        configure(&mut configuration);
        let workspaces = &configuration.state.workspaces;
        assert_eq!(workspaces.len(), 10);
        assert_eq!(
            workspaces
                .iter()
                .map(|workspace| (
                    workspace.name.as_str(),
                    workspace.tag.as_str(),
                    workspace.layout
                ))
                .collect::<Vec<_>>(),
            vec![
                ("Chrome", "\u{eb01}", Layout::Deck),
                ("Terminal", "\u{f120}", Layout::Deck),
                ("Rider", "\u{eb0f}", Layout::Monocle),
                ("ArchVM", "\u{f303}", Layout::Deck),
                ("Windows10VM", "\u{f17a}", Layout::Deck),
                ("Teams", "\u{f075}", Layout::Deck),
                ("7", "7", Layout::Deck),
                ("8", "8", Layout::Deck),
                ("9", "9", Layout::Deck),
                ("Desktop", "0", Layout::Deck),
            ]
        );
        assert!(configuration.always_redraw);
        assert!(configuration.minimize_to_hide_all);
        assert!(!configuration.use_old_move_logic_configured);
        assert_eq!(configuration.prevent_float_processes, ["wezterm-gui.exe"]);
        assert_eq!(configuration.bar_segments.len(), 7);
        assert_eq!(configuration.program_launchers.len(), 2);
    }

    #[test]
    fn production_configuration_has_the_explicit_menu_bindings() {
        use crate::commands::{LALT, LCTL, LWIN};
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{VK_F15, VK_F16, VK_F17, VK_SPACE};
        let mut configuration = Configuration::default();
        configure(&mut configuration);
        let modifiers = LALT | LWIN | LCTL;
        for (key, action) in [
            (VK_SPACE as u32, Action::ShowWindows),
            (VK_F15 as u32, Action::ShowProcesses),
            (VK_F16 as u32, Action::ShowFileSystem),
            (VK_F17 as u32, Action::ShowCommands),
        ] {
            assert!(configuration.key_bindings.iter().any(|binding| {
                binding.key == key && binding.modifiers == modifiers && binding.action == action
            }));
        }
        assert_eq!(
            configuration.commands.len(),
            configuration.key_bindings.len() + 4,
            "C parity commands plus the Rust error-log menu command"
        );
        assert_eq!(
            configuration.commands[..10]
                .iter()
                .map(|command| command.name.as_str())
                .collect::<Vec<_>>(),
            [
                "quit_and_restore_windows",
                "quit",
                "select_window_down",
                "select_window_up",
                "monitor_select_next",
                "arrange_clients_in_selected_workspace",
                "select_window_right",
                "select_window_left",
                "move_focused_window_to_main",
                "minimize_focused_window",
            ]
        );
        let create = configuration
            .commands
            .iter()
            .position(|command| command.name == "toggle_create_window_in_current_workspace")
            .unwrap();
        assert_eq!(
            configuration.commands[create + 2].name,
            "toggle_non_filtered_windows_assigned_to_current_workspace"
        );
    }
}
