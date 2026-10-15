use crate::builtin_pickers::{COMMANDS, FILE_SYSTEM, PROCESSES, SERVICES, WINDOWS, program_launcher};
use crate::commands::{Action, KeyBinding};
use crate::config::{BarSegmentValue, PickerMonitor};
use crate::model::WindowRoutingMode;
use crate::{Configuration, Layout};

pub fn configure(configuration: &mut Configuration) {
    configuration
        .always_exclude_processes
        .push("ExampleProgram.exe".into());
    // Override these to change picker sizes; available screen space may reduce them.
    configuration.picker_result_rows = 7;
    configuration.picker_preview_rows = 14;
    // Zero-based SWM monitor index: Fixed(1) selects the second monitor.
    configuration.picker_monitor = PickerMonitor::Fixed(0);
    // Use PickerMonitor::Focused instead to follow the focused monitor.
    // Routing, tool paths, and video frame position belong to this Rust config.
    configuration.file_preview_resolver =
        crate::file_preview::resolver(crate::file_preview::MediaPreviewSettings {
            ffmpeg: "ffmpeg.exe".into(),
            ffprobe: "ffprobe.exe".into(),
            video_position: 0.25,
            ..Default::default()
        });
    use crate::commands::{LALT, LSHIFT, LWIN};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

    let modifiers = LALT;
    configuration.configuration_register_default_text_style("JetBrainsMonoNL NFP Regular", 12, 13);
    configuration.keybindings_register_defaults_with_modifiers(modifiers, 0);
    configuration.state.current_window_routing_mode =
        WindowRoutingMode::FilteredRoutedNonFilteredCurrentWorkspace;
    configuration.always_redraw = true;
    configuration.use_old_move_logic_configured = false;

    configuration
        .prevent_float_processes
        .push("wezterm-gui.exe".into());

    let browser = configuration.workspace_register("Chrome", "\u{eb01}", true, Layout::Deck);
    for process in ["chrome.exe", "brave.exe", "firefox.exe", "msedge.exe"] {
        configuration.workspace_register_processimagename_contains_filter(browser, process);
    }

    let terminal = configuration.workspace_register("Terminal", "\u{f120}", true, Layout::Deck);
    for filter in ["WindowsTerminal.exe", "wezterm-gui.exe"] {
        configuration.workspace_register_processimagename_contains_filter(terminal, filter);
    }

    let ide = configuration.workspace_register("Rider", "\u{eb0f}", true, Layout::Monocle);
    for process in ["rider64.exe", "rustrover64.exe", "Code.exe", "devenv.exe"] {
        configuration.workspace_register_processimagename_contains_filter(ide, process);
    }

    let teams = configuration.workspace_register("Teams", "\u{f075}", true, Layout::Deck);
    configuration.workspace_register_processimagename_contains_filter(teams, "Teams.exe");

    for index in 5..=9 {
        let value = index.to_string();
        configuration.workspace_register(&value, &value, false, Layout::Tile);
    }
    configuration.workspace_register("Desktop", "0", false, Layout::Deck);

    for (key, action) in [
        (VK_SPACE as u32, Action::ShowPicker(&WINDOWS)),
        (VK_F14 as u32, Action::ShowPicker(&SERVICES)),
        (VK_F15 as u32, Action::ShowPicker(&PROCESSES)),
        (VK_F17 as u32, Action::ShowPicker(&COMMANDS)),
        (VK_F16 as u32, Action::ShowPicker(&FILE_SYSTEM)),
    ] {
        configuration.keybinding_create(KeyBinding {
            modifiers,
            key,
            action,
        });
    }
    let program_directories = [
        "%USERPROFILE%\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\",
        "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\",
        "%USERPROFILE%\\AppData\\Local\\Microsoft\\WindowsApps\\",
    ];
    for (name, extra_modifiers, elevated) in [
        ("ProgramLauncherMenu_not_elevated", 0, false),
        ("ProgramLauncherMenu_elevated", LSHIFT, true),
    ] {
        let launcher =
            configuration.register_picker(program_launcher(name, &program_directories, elevated));
        configuration.keybinding_create(KeyBinding {
            modifiers: modifiers | extra_modifiers,
            key: b'P' as u32,
            action: Action::ShowPicker(launcher),
        });
    }

    use BarSegmentValue::*;
    configuration.configuration_add_bar_segment(" | ", 5, false, SystemTime);
    configuration.configuration_add_bar_segment(" | ", 5, false, LocalTime);
    configuration.configuration_add_bar_segment(" | ", 10, false, LocalDate);
    configuration.configuration_add_bar_segment_with_header(" | ", "CPU:", 3, false, Cpu);
    configuration.configuration_add_bar_segment_with_header(" | ", "RAM:", 3, false, Memory);
    configuration.configuration_add_bar_segment_with_header(" | ", "\u{f028}", 3, false, Volume);
    configuration.configuration_add_bar_segment(" | ", 1, true, Internet);

    for index in 0..9 {
        configuration.keybinding_create(KeyBinding {
            modifiers,
            key: VK_F1 as u32 + index as u32,
            action: Action::MoveWorkspaceToMonitor(index, configuration.secondary_monitor),
        });
        configuration.keybinding_create(KeyBinding {
            modifiers: 0,
            key: VK_F13 as u32 + index as u32,
            action: Action::MoveWorkspaceToMonitor(index, configuration.secondary_monitor),
        });
    }
    configuration.keybinding_create(KeyBinding {
        modifiers: modifiers | LSHIFT,
        key: VK_RETURN as u32,
        action: Action::MoveSecondaryFocusedToMain,
    });

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
    for index in 0..display_monitor_count() {
        configuration.keybinding_create(KeyBinding {
            modifiers: LWIN,
            key: b'1' as u32 + index as u32,
            action: Action::MoveFloatToMonitor(index),
        });
    }
}

fn display_monitor_count() -> usize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CMONITORS};
    unsafe { GetSystemMetrics(SM_CMONITORS).max(0) as usize }
}
