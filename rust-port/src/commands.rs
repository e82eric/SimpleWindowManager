#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Quit,
    QuitAndRestore,
    SelectDown,
    SelectUp,
    SelectLeft,
    SelectRight,
    MoveDown,
    MoveUp,
    MoveLeft,
    MoveRight,
    MoveToMain,
    SelectNextMonitor,
    ResetLayout,
    SelectWorkspace(usize),
    MoveToWorkspace(usize),
    MoveToSelectedWorkspace,
    CloseFocused,
    KillFocused,
    ToggleTaskbar,
    ToggleDebugOutput,
    ToggleFilteredCurrent,
    ToggleUnfilteredCurrent,
    ToggleUnmatchedCurrent,
    StopManaging,
    SetMonocle,
    SetDeck,
    SetGrid,
    SetTile,
    SetTileReversed,
    RedrawFocused,
    MinimizeFocused,
    ShowWindows,
    ShowPrograms(bool),
    ShowProcesses,
    ShowServices,
    ShowFileSystem,
    ShowCommands,
    ShowFloatLogs,
    ShowClientLogs,
    ShowErrorLogs,
    ShowConfigurationState,
    Launch(&'static str),
    MoveFloat(i32, i32),
    MoveFloatToMonitor(usize),
    MoveWorkspaceToMonitor(usize, usize),
    MoveSecondaryFocusedToMain,
}

#[derive(Clone, Copy, Debug)]
pub struct KeyBinding {
    pub modifiers: i32,
    pub key: u32,
    pub action: Action,
}

pub const LSHIFT: i32 = 0x1;
pub const RSHIFT: i32 = 0x2;
pub const LALT: i32 = 0x4;
pub const RALT: i32 = 0x8;
pub const LWIN: i32 = 0x10;
pub const RWIN: i32 = 0x20;
pub const LCTL: i32 = 0x80;

pub fn keybindings_register_defaults_with_modifiers(
    modifiers: i32,
    f_modifiers: i32,
) -> Vec<KeyBinding> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    let b = |modifiers, key, action| KeyBinding {
        modifiers,
        key,
        action,
    };
    let mut bindings = vec![
        b(modifiers | LSHIFT, VK_F10 as u32, Action::QuitAndRestore),
        b(modifiers | LSHIFT, VK_F9 as u32, Action::Quit),
        b(modifiers, b'J' as u32, Action::SelectDown),
        b(modifiers, b'K' as u32, Action::SelectUp),
        b(modifiers, VK_OEM_COMMA as u32, Action::SelectNextMonitor),
        b(modifiers, b'N' as u32, Action::ResetLayout),
        b(modifiers, b'L' as u32, Action::SelectRight),
        b(modifiers, b'H' as u32, Action::SelectLeft),
        b(modifiers, VK_RETURN as u32, Action::MoveToMain),
        b(modifiers | LSHIFT, b'O' as u32, Action::MinimizeFocused),
    ];
    for index in 0..10 {
        let digit = if index == 9 { b'0' } else { b'1' + index as u8 } as u32;
        bindings.push(b(modifiers, digit, Action::SelectWorkspace(index)));
    }
    for index in 0..10 {
        bindings.push(b(
            f_modifiers,
            VK_F13 as u32 + index as u32,
            Action::SelectWorkspace(index),
        ));
    }
    bindings.extend([
        b(modifiers | LSHIFT, b'J' as u32, Action::MoveDown),
        b(modifiers | LSHIFT, b'K' as u32, Action::MoveUp),
        b(modifiers | LSHIFT, b'L' as u32, Action::MoveRight),
        b(modifiers | LSHIFT, b'H' as u32, Action::MoveLeft),
    ]);
    for index in 0..9 {
        bindings.push(b(
            modifiers | LSHIFT,
            b'1' as u32 + index as u32,
            Action::MoveToWorkspace(index),
        ));
    }
    bindings.push(b(
        modifiers | LSHIFT,
        b'0' as u32,
        Action::MoveToSelectedWorkspace,
    ));
    for index in 0..9 {
        bindings.push(b(
            f_modifiers | LSHIFT,
            VK_F13 as u32 + index as u32,
            Action::MoveToWorkspace(index),
        ));
    }
    bindings.push(b(
        f_modifiers | LSHIFT,
        VK_F22 as u32,
        Action::MoveToSelectedWorkspace,
    ));
    bindings.extend([
        b(modifiers, b'C' as u32, Action::CloseFocused),
        b(modifiers | LSHIFT, b'C' as u32, Action::KillFocused),
        b(modifiers, b'V' as u32, Action::ToggleTaskbar),
        b(modifiers, b'B' as u32, Action::ToggleFilteredCurrent),
        b(modifiers, b'Z' as u32, Action::ToggleUnfilteredCurrent),
        b(modifiers, b'B' as u32, Action::ToggleUnmatchedCurrent),
        b(modifiers, b'X' as u32, Action::StopManaging),
        b(modifiers, b'M' as u32, Action::SetMonocle),
        b(modifiers, b'Y' as u32, Action::SetDeck),
        b(modifiers, b'U' as u32, Action::SetGrid),
        b(modifiers, b'I' as u32, Action::RedrawFocused),
    ]);
    bindings
}
