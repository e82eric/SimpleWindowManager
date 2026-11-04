//! Low-level hooks live on a thread that owns no windows and does no UI work.
use crate::platform_error::{PlatformError, PlatformResult};
use std::{cell::RefCell, sync::atomic::Ordering, sync::mpsc, thread::JoinHandle};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const MODIFIER_KEYS: [u16; 8] = [
    VK_LSHIFT,
    VK_RSHIFT,
    VK_LMENU,
    VK_RMENU,
    VK_LCONTROL,
    VK_RCONTROL,
    VK_LWIN,
    VK_RWIN,
];

#[derive(Default)]
struct Modifiers {
    down: [bool; 8],
}
impl Modifiers {
    fn update(&mut self, key: u32, down: bool) {
        if let Some(index) = MODIFIER_KEYS.iter().position(|&vk| u32::from(vk) == key) {
            self.down[index] = down;
        }
    }

    fn mask(&self) -> i32 {
        use crate::commands::{LALT, LCTL, LSHIFT, LWIN, RALT, RSHIFT, RWIN};
        let bits = [LSHIFT, RSHIFT, LALT, RALT, LCTL, LCTL, LWIN, RWIN];
        self.down
            .iter()
            .zip(bits)
            .fold(0, |mask, (&down, bit)| mask | if down { bit } else { 0 })
    }

    fn initial() -> Self {
        // Seed keys already held at startup, outside the hook callback. All
        // subsequent changes come from hook events, including both Ctrl keys.
        Self {
            down: MODIFIER_KEYS
                .map(|key| unsafe { GetAsyncKeyState(i32::from(key)) as u16 & 0x8000 != 0 }),
        }
    }
}
#[derive(Default)]
struct InputState {
    modifiers: Modifiers,
    keys: crate::ui::KeyState,
}
thread_local! {
    static INPUT: RefCell<InputState> = RefCell::new(InputState::default());
}

pub(crate) struct InputHooks {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

struct Hooks {
    keyboard: HHOOK,
    mouse: HHOOK,
}
impl Drop for Hooks {
    fn drop(&mut self) {
        for hook in [self.mouse, self.keyboard] {
            if !hook.is_null() && unsafe { UnhookWindowsHookEx(hook) } == 0 {
                PlatformError::last_win32("UnhookWindowsHookEx(input)").report();
            }
        }
    }
}

fn install() -> PlatformResult<Hooks> {
    let module =
        unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()) };
    if module.is_null() {
        return Err(PlatformError::last_win32("GetModuleHandleW(input)"));
    }
    let mut hooks = Hooks {
        keyboard: std::ptr::null_mut(),
        mouse: std::ptr::null_mut(),
    };
    hooks.keyboard =
        unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(handle_key_press), module, 0) };
    if hooks.keyboard.is_null() {
        return Err(PlatformError::last_win32("keyboard hook"));
    }
    hooks.mouse = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(handle_mouse), module, 0) };
    if hooks.mouse.is_null() {
        return Err(PlatformError::last_win32("mouse hook"));
    }
    Ok(hooks)
}

impl InputHooks {
    pub(crate) fn start() -> PlatformResult<Self> {
        let (ready, receiver) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("swm-input".into())
            .spawn(move || {
                let mut message = MSG::default();
                unsafe {
                    PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
                }
                INPUT.with(|state| state.borrow_mut().modifiers = Modifiers::initial());
                let _hooks = match install() {
                    Ok(hooks) => hooks,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                let thread_id =
                    unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
                if ready.send(Ok(thread_id)).is_err() {
                    return;
                }
                loop {
                    // Retrieving messages dispatches the low-level hook calls.
                    // This thread has no windows or application work to dispatch.
                    let result = unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) };
                    if result == 0 {
                        break;
                    }
                    if result == -1 {
                        PlatformError::last_win32("GetMessageW(input)").report();
                        crate::ui::post(crate::ui::Command::Quit);
                        break;
                    }
                }
            })
            .map_err(|error| {
                PlatformError::win32(
                    "spawn input thread",
                    error.raw_os_error().unwrap_or(8) as u32,
                )
            })?;
        match receiver.recv() {
            Ok(Ok(thread_id)) => Ok(Self {
                thread_id,
                thread: Some(thread),
            }),
            result => {
                let _ = thread.join();
                Err(match result {
                    Ok(Err(error)) => error,
                    _ => PlatformError::win32("input thread startup failed", 1067),
                })
            }
        }
    }
}

impl Drop for InputHooks {
    fn drop(&mut self) {
        let Some(thread) = self.thread.take() else {
            return;
        };
        if !thread.is_finished()
            && unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) } == 0
        {
            PlatformError::last_win32("PostThreadMessageW(stop input)").report();
            // Do not block shutdown if the quit message could not be delivered.
            return;
        }
        if thread.join().is_err() {
            crate::error_log::report("input thread", "thread panicked");
        }
    }
}

unsafe extern "system" fn handle_key_press(code: i32, wparam: usize, lparam: isize) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };
    if code == 0 {
        let event = wparam as u32;
        let key = unsafe { (*(lparam as *const KBDLLHOOKSTRUCT)).vkCode };
        if matches!(event, WM_KEYUP | WM_SYSKEYUP | WM_KEYDOWN | WM_SYSKEYDOWN) {
            let up = matches!(event, WM_KEYUP | WM_SYSKEYUP);
            let swallowed = INPUT.with(|state| {
                let mut state = state.borrow_mut();
                state.modifiers.update(key, !up);
                let modifiers = state.modifiers.mask();
                state.keys.event(key, up, || {
                    crate::ui::binding(key, modifiers).is_some_and(crate::commands::dispatch_action)
                })
            });
            if swallowed {
                return 1;
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

unsafe extern "system" fn handle_mouse(code: i32, wparam: usize, lparam: isize) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{CallNextHookEx, WM_LBUTTONUP};
    if code == 0 {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MSLLHOOKSTRUCT, WM_LBUTTONDOWN};
        if wparam as u32 == WM_LBUTTONDOWN {
            crate::worker::GESTURE.fetch_add(1, Ordering::AcqRel);
        }
        if wparam as u32 == WM_LBUTTONUP {
            let point = unsafe { (*(lparam as *const MSLLHOOKSTRUCT)).pt };
            crate::worker::release(point.x, point.y);
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn releasing_one_ctrl_preserves_the_other() {
        let mut modifiers = Modifiers::default();
        modifiers.update(VK_LCONTROL.into(), true);
        modifiers.update(VK_RCONTROL.into(), true);
        modifiers.update(VK_LCONTROL.into(), false);
        assert_eq!(modifiers.mask(), crate::commands::LCTL);
        modifiers.update(VK_RCONTROL.into(), false);
        assert_eq!(modifiers.mask(), 0);
    }

    #[test]
    fn modifier_sides_repeats_and_unrelated_keys() {
        use crate::commands::{LALT, LWIN, RSHIFT};
        let mut modifiers = Modifiers::default();
        modifiers.update(VK_LMENU.into(), true);
        modifiers.update(VK_RSHIFT.into(), true);
        modifiers.update(VK_LWIN.into(), true);
        modifiers.update(VK_RSHIFT.into(), true);
        modifiers.update(65, true);
        modifiers.update(65, false);
        assert_eq!(modifiers.mask(), LALT | RSHIFT | LWIN);
        modifiers.update(VK_RSHIFT.into(), false);
        assert_eq!(modifiers.mask(), LALT | LWIN);
    }
}
