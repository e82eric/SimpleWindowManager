//! Exercises the actual OpenGL host without starting a second window manager.
use simple_window_manager_rs::builtin_pickers::{
    CommandList, DataRow, DataTable, FileAction, Files, Processes, Windows,
};
use simple_window_manager_rs::commands::Action;
use simple_window_manager_rs::egui_window_menu::{ScreenBounds, WindowMenuHost};
use simple_window_manager_rs::picker::{Outcome, PickerConfig, PickerRequest};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;
fn files(
    options: nfm_file_system::FileSystemPickerOptions,
    action: FileAction,
) -> Box<dyn PickerRequest> {
    Box::new(Files { options, action })
}
fn leak(name: &str) -> &'static str {
    Box::leak(name.to_owned().into_boxed_str())
}
/// Rows named in `menu_actions` open a picker; others launch their own name, so
/// accepting a row identifies it.
fn commands(rows: Vec<Vec<String>>, menu_actions: Vec<String>) -> Box<dyn PickerRequest> {
    Box::new(CommandList {
        rows: rows
            .into_iter()
            .map(|row| {
                let name = leak(&row[0]);
                let action = if menu_actions.iter().any(|menu| menu == name) {
                    Action::ShowPicker(Box::leak(Box::new(PickerConfig::new(name, Windows))))
                } else {
                    Action::Launch(name)
                };
                (row, action)
            })
            .collect(),
    })
}
fn data(title: String, columns: Vec<String>, rows: Vec<DataRow>) -> Box<dyn PickerRequest> {
    Box::new(DataTable {
        title,
        columns,
        rows,
    })
}
/// The launched path and elevation of a file picker's accept call.
fn launched(outcome: Outcome) -> Option<(std::path::PathBuf, bool)> {
    let Outcome::Runtime(call) = outcome else {
        return None;
    };
    let path = call.label().strip_prefix("launch ")?;
    Some(match path.strip_suffix(" elevated") {
        Some(path) => (path.into(), true),
        None => (path.into(), false),
    })
}
static WATCHED_WINDOW: AtomicUsize = AtomicUsize::new(0);
static HIDE_EVENTS: AtomicUsize = AtomicUsize::new(0);
unsafe extern "system" fn visibility_event(
    _hook: windows_sys::Win32::UI::Accessibility::HWINEVENTHOOK,
    _event: u32,
    hwnd: windows_sys::Win32::Foundation::HWND,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    if hwnd as usize == WATCHED_WINDOW.load(Ordering::Relaxed)
        && object == OBJID_WINDOW
        && child == CHILDID_SELF as i32
    {
        HIDE_EVENTS.fetch_add(1, Ordering::Relaxed);
    }
}
fn pump_events() {
    unsafe {
        let mut message = MSG::default();
        while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

#[track_caller]
fn wait_until(mut condition: impl FnMut() -> bool) {
    let caller = std::panic::Location::caller();
    let deadline = Instant::now() + Duration::from_secs(15);
    while !condition() {
        pump_events();
        assert!(
            Instant::now() < deadline,
            "native menu did not become ready at {caller}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn key(hwnd: windows_sys::Win32::Foundation::HWND, key: u32) {
    unsafe {
        assert_ne!(PostMessageW(hwnd, WM_KEYDOWN, key as usize, 1), 0);
        assert_ne!(
            PostMessageW(hwnd, WM_KEYUP, key as usize, (1_u32 << 31) as isize | 1),
            0
        );
    }
}
fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(true)
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .init();
    let item = nfm_win32::list_windows::list_windows()
        .unwrap()
        .into_iter()
        .next()
        .expect("smoke test requires at least one application window");
    let previous_foreground = unsafe { GetForegroundWindow() };
    let (send, receive) = mpsc::channel();
    let host = WindowMenuHost::start(move |session, outcome| {
        let _ = send.send((session, outcome));
    })
    .unwrap();
    let startup_title: Vec<u16> = "SimpleWindowManager window switcher\0"
        .encode_utf16()
        .collect();
    let startup_hwnd = unsafe { FindWindowW(std::ptr::null(), startup_title.as_ptr()) };
    assert!(
        !startup_hwnd.is_null(),
        "startup must finish creating the popup"
    );
    assert_eq!(
        unsafe { IsWindowVisible(startup_hwnd) },
        0,
        "startup popup must stay hidden"
    );
    assert_eq!(
        unsafe { GetForegroundWindow() },
        previous_foreground,
        "startup popup must not take focus"
    );
    let bounds = ScreenBounds {
        left: 0,
        top: 0,
        width: unsafe { GetSystemMetrics(SM_CXSCREEN) },
        height: unsafe { GetSystemMetrics(SM_CYSCREEN) },
    };
    host.show_request(1, bounds, Box::new(Windows)).unwrap();
    let title: Vec<u16> = "SimpleWindowManager window switcher\0"
        .encode_utf16()
        .collect();
    let mut hwnd = std::ptr::null_mut();
    wait_until(|| {
        hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
        !hwnd.is_null() && unsafe { IsWindowVisible(hwnd) != 0 && GetForegroundWindow() == hwnd }
    });
    std::thread::sleep(Duration::from_millis(400));
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
    assert_ne!(
        style & WS_EX_TOOLWINDOW,
        0,
        "popup must remain a tool window after showing"
    );
    assert_eq!(
        style & WS_EX_APPWINDOW,
        0,
        "popup must not be managed as an application"
    );
    let mut outer = windows_sys::Win32::Foundation::RECT::default();
    let mut client = windows_sys::Win32::Foundation::RECT::default();
    unsafe {
        assert_ne!(GetWindowRect(hwnd, &mut outer), 0);
        assert_ne!(GetClientRect(hwnd, &mut client), 0);
        let mut origin = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
        assert_ne!(
            windows_sys::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut origin),
            0
        );
        assert_eq!(
            (origin.x, origin.y),
            (outer.left, outer.top),
            "no non-client top edge"
        );
        assert_eq!(
            (client.right, client.bottom),
            (outer.right - outer.left, outer.bottom - outer.top),
            "client must cover the entire popup"
        );
    }
    let query = format!("{:08X}", item.hwnd as usize);
    let native_style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32;
    assert_eq!(
        native_style & (WS_CAPTION | WS_THICKFRAME),
        0,
        "shown popup must have no native title bar or resize frame"
    );
    assert_eq!(
        style & (WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE),
        0,
        "shown popup must have no native edge decorations"
    );
    for ch in query.encode_utf16() {
        unsafe {
            assert_ne!(PostMessageW(hwnd, WM_KEYDOWN, 0xE7, 1), 0);
            assert_ne!(PostMessageW(hwnd, WM_CHAR, ch as usize, 1), 0);
            assert_ne!(
                PostMessageW(hwnd, WM_KEYUP, 0xE7, (1_u32 << 31) as isize | 1),
                0
            );
        }
    }
    std::thread::sleep(Duration::from_millis(400));
    key(hwnd, 0x0D);
    let (session, outcome) = receive.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(session, 1);
    match outcome {
        Outcome::Runtime(call) => {
            assert_eq!(call.label(), format!("focus window {:#x}", item.hwnd))
        }
        Outcome::Action(_) => panic!("unexpected action"),
        Outcome::Failed(error) => panic!("menu failed: {error}"),
        Outcome::Cancelled => panic!("menu cancelled instead of accepting"),
    }
    wait_until(|| unsafe { IsWindowVisible(hwnd) == 0 });
    host.show_request(2, bounds, Box::new(Windows)).unwrap();
    wait_until(|| unsafe { IsWindowVisible(hwnd) != 0 && GetForegroundWindow() == hwnd });
    std::thread::sleep(Duration::from_millis(300));
    key(hwnd, 0x1B);
    let (session, outcome) = receive.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(session, 2);
    assert!(matches!(outcome, Outcome::Cancelled));
    wait_until(|| unsafe { IsWindowVisible(hwnd) == 0 });
    host.show_request(3, bounds, Box::new(Windows)).unwrap();
    wait_until(|| unsafe { IsWindowVisible(hwnd) != 0 && GetForegroundWindow() == hwnd });
    host.hide();
    wait_until(|| unsafe { IsWindowVisible(hwnd) == 0 });
    assert!(
        receive.try_recv().is_err(),
        "superseded menu must not restore focus"
    );
    host.show_request(4, bounds, Box::new(Windows)).unwrap();
    wait_until(|| unsafe { IsWindowVisible(hwnd) != 0 && GetForegroundWindow() == hwnd });
    if std::env::args().any(|arg| arg == "--pause") {
        std::thread::sleep(Duration::from_secs(10));
    }
    unsafe {
        assert_ne!(PostMessageW(hwnd, WM_CLOSE, 0, 0), 0);
    }
    let (session, outcome) = receive.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(session, 4);
    assert!(matches!(outcome, Outcome::Cancelled));
    wait_until(|| unsafe { IsWindowVisible(hwnd) == 0 && IsWindow(hwnd) != 0 });
    host.show_request(5, bounds, Box::new(Windows)).unwrap();
    wait_until(|| unsafe { IsWindowVisible(hwnd) != 0 && GetForegroundWindow() == hwnd });
    host.hide();
    wait_until(|| unsafe { IsWindowVisible(hwnd) == 0 });
    let root = std::env::temp_dir().join(format!("swm-native-files-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("unique-egui-preview.txt");
    std::fs::write(&file, "native filesystem preview\nsecond line\n").unwrap();
    host.show_request(
        6,
        bounds,
        files(
            nfm_file_system::FileSystemPickerOptions {
                roots: vec![root.to_string_lossy().into_owned()],
                max_depth: 5,
                preview_visible: true,
                ..Default::default()
            },
            FileAction::Launch { elevated: false },
        ),
    )
    .unwrap();
    wait_until(|| unsafe { IsWindowVisible(hwnd) != 0 && GetForegroundWindow() == hwnd });
    std::thread::sleep(Duration::from_millis(400));
    for ch in "unique-egui-preview.txt".encode_utf16() {
        unsafe {
            assert_ne!(PostMessageW(hwnd, WM_KEYDOWN, 0xE7, 1), 0);
            assert_ne!(PostMessageW(hwnd, WM_CHAR, ch as usize, 1), 0);
            assert_ne!(
                PostMessageW(hwnd, WM_KEYUP, 0xE7, (1_u32 << 31) as isize | 1),
                0
            );
        }
    }
    std::thread::sleep(Duration::from_millis(400));
    key(hwnd, 0x0D);
    let (session, outcome) = receive.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(session, 6);
    assert_eq!(launched(outcome), Some((file.clone(), false)));
    wait_until(|| unsafe { IsWindowVisible(hwnd) == 0 });
    std::fs::remove_file(file).unwrap();
    let program = root.join("unique-egui-program.cmd");
    std::fs::write(&program, "@echo test fixture\r\n").unwrap();
    for (session, elevated) in [(7, false), (8, true)] {
        host.show_request(
            session,
            bounds,
            files(
                nfm_file_system::FileSystemPickerOptions {
                    roots: vec![root.to_string_lossy().into_owned()],
                    max_depth: 5,
                    files_only: true,
                    preview_visible: false,
                    bat_theme: Some("gruvbox-dark".into()),
                    ..Default::default()
                },
                FileAction::Launch { elevated },
            ),
        )
        .unwrap();
        wait_until(|| unsafe { IsWindowVisible(hwnd) != 0 && GetForegroundWindow() == hwnd });
        std::thread::sleep(Duration::from_millis(400));
        for ch in "unique-egui-program.cmd".encode_utf16() {
            unsafe {
                assert_ne!(PostMessageW(hwnd, WM_KEYDOWN, 0xE7, 1), 0);
                assert_ne!(PostMessageW(hwnd, WM_CHAR, ch as usize, 1), 0);
                assert_ne!(
                    PostMessageW(hwnd, WM_KEYUP, 0xE7, (1_u32 << 31) as isize | 1),
                    0
                );
            }
        }
        std::thread::sleep(Duration::from_millis(400));
        key(hwnd, 0x0D);
        let (received_session, outcome) = receive.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(received_session, session);
        assert_eq!(launched(outcome), Some((program.clone(), elevated)));
        wait_until(|| unsafe { IsWindowVisible(hwnd) == 0 });
    }
    std::fs::remove_file(program).unwrap();
    std::fs::remove_dir(root).unwrap();
    host.show_request(9, bounds, Box::new(Processes)).unwrap();
    wait_until(|| unsafe { IsWindowVisible(hwnd) != 0 && GetForegroundWindow() == hwnd });
    std::thread::sleep(Duration::from_millis(500));
    for ch in "/".encode_utf16() {
        unsafe {
            assert_ne!(PostMessageW(hwnd, WM_KEYDOWN, 0xE7, 1), 0);
            assert_ne!(PostMessageW(hwnd, WM_CHAR, ch as usize, 1), 0);
            assert_ne!(
                PostMessageW(hwnd, WM_KEYUP, 0xE7, (1_u32 << 31) as isize | 1),
                0
            );
        }
    }
    for _ in 0..2 {
        std::thread::sleep(Duration::from_millis(150));
        key(hwnd, 0x28); // Select Sort, then Descending.
        key(hwnd, 0x0D);
        std::thread::sleep(Duration::from_millis(150));
        assert!(
            receive.try_recv().is_err(),
            "completion Enter must not close the picker"
        );
        assert_ne!(unsafe { IsWindowVisible(hwnd) }, 0);
    }
    for ch in "CPU ".encode_utf16() {
        unsafe {
            assert_ne!(PostMessageW(hwnd, WM_KEYDOWN, 0xE7, 1), 0);
            assert_ne!(PostMessageW(hwnd, WM_CHAR, ch as usize, 1), 0);
            assert_ne!(
                PostMessageW(hwnd, WM_KEYUP, 0xE7, (1_u32 << 31) as isize | 1),
                0
            );
        }
    }
    std::thread::sleep(Duration::from_millis(200));
    key(hwnd, 0x1B);
    let (session, outcome) = receive.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(session, 9);
    assert!(matches!(outcome, Outcome::Cancelled));
    wait_until(|| unsafe { IsWindowVisible(hwnd) == 0 });
    host.show_request(
        10,
        bounds,
        commands(
            vec![
                vec![
                    "fixture_first".into(),
                    "Action".into(),
                    "Ctrl+A".into(),
                    "First fixture".into(),
                ],
                vec![
                    "fixture_selected".into(),
                    "Action".into(),
                    "Ctrl+B".into(),
                    "Selected fixture".into(),
                ],
            ],
            Vec::new(),
        ),
    )
    .unwrap();
    wait_until(|| unsafe { IsWindowVisible(hwnd) != 0 && GetForegroundWindow() == hwnd });
    std::thread::sleep(Duration::from_millis(300));
    for ch in "fixture_selected".encode_utf16() {
        unsafe {
            assert_ne!(PostMessageW(hwnd, WM_KEYDOWN, 0xE7, 1), 0);
            assert_ne!(PostMessageW(hwnd, WM_CHAR, ch as usize, 1), 0);
            assert_ne!(
                PostMessageW(hwnd, WM_KEYUP, 0xE7, (1_u32 << 31) as isize | 1),
                0
            );
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    std::thread::sleep(Duration::from_millis(400));
    key(hwnd, 0x0D);
    let (session, outcome) = receive.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(session, 10);
    match outcome {
        Outcome::Action(Action::Launch(name)) => assert_eq!(name, "fixture_selected"),
        Outcome::Cancelled => panic!("commands cancelled"),
        Outcome::Failed(error) => panic!("commands failed: {error}"),
        _ => panic!("unexpected command outcome"),
    };
    wait_until(|| unsafe { IsWindowVisible(hwnd) == 0 });
    host.show_request(
        11,
        bounds,
        commands(
            vec![
                vec![
                    "fixture_first".into(),
                    "Action".into(),
                    "Ctrl+A".into(),
                    "First fixture".into(),
                ],
                vec![
                    "fixture_selected".into(),
                    "Action".into(),
                    "Ctrl+B".into(),
                    "Selected fixture".into(),
                ],
            ],
            Vec::new(),
        ),
    )
    .unwrap();
    wait_until(|| unsafe { IsWindowVisible(hwnd) != 0 && GetForegroundWindow() == hwnd });
    std::thread::sleep(Duration::from_millis(200));
    key(hwnd, 0x1B);
    let (session, outcome) = receive.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(session, 11);
    assert!(matches!(outcome, Outcome::Cancelled));
    wait_until(|| unsafe { IsWindowVisible(hwnd) == 0 });
    host.show_request(
        12,
        bounds,
        commands(
            vec![vec![
                "fixture_updated".into(),
                "Action".into(),
                "Ctrl+C".into(),
                "Updated fixture".into(),
            ]],
            Vec::new(),
        ),
    )
    .unwrap();
    wait_until(|| unsafe { IsWindowVisible(hwnd) != 0 && GetForegroundWindow() == hwnd });
    std::thread::sleep(Duration::from_millis(300));
    key(hwnd, 0x0D);
    let (session, outcome) = receive.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(session, 12);
    assert!(matches!(outcome, Outcome::Action(Action::Launch(name)) if name == "fixture_updated"));
    wait_until(|| unsafe { IsWindowVisible(hwnd) == 0 });
    for (index, title) in [
        "Services",
        "Float logs",
        "Client logs",
        "Error logs",
        "Configuration state",
    ]
    .into_iter()
    .enumerate()
    {
        let session = 20 + index as u64;
        host.show_request(
            session,
            bounds,
            data(
                title.into(),
                vec!["Property".into(), "Value".into()],
                vec![DataRow {
                    cells: vec!["fixture".into(), title.into()],
                    preview: (index >= 3).then(|| format!("{title}\nMultiline detail preview")),
                }],
            ),
        )
        .unwrap();
        wait_until(|| unsafe { IsWindowVisible(hwnd) != 0 && GetForegroundWindow() == hwnd });
        std::thread::sleep(Duration::from_millis(200));
        key(hwnd, if index % 2 == 0 { 0x0D } else { 0x1B });
        let (closed_session, outcome) = receive.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(closed_session, session);
        assert!(matches!(outcome, Outcome::Cancelled));
        wait_until(|| unsafe { IsWindowVisible(hwnd) == 0 });
    }
    // Observe native hide events, including transitions too brief for polling.
    WATCHED_WINDOW.store(hwnd as usize, Ordering::Relaxed);
    let hook = unsafe {
        windows_sys::Win32::UI::Accessibility::SetWinEventHook(
            EVENT_OBJECT_HIDE,
            EVENT_OBJECT_HIDE,
            std::ptr::null_mut(),
            Some(visibility_event),
            windows_sys::Win32::System::Threading::GetCurrentProcessId(),
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };
    assert!(!hook.is_null());
    let handoff_root = std::env::temp_dir().join(format!("swm-handoff-{}", std::process::id()));
    std::fs::create_dir_all(&handoff_root).unwrap();
    for index in 0..5 {
        let session = 40 + index * 2;
        host.show_request(
            session,
            bounds,
            commands(
                vec![vec![
                    "next_menu".into(),
                    "Menu".into(),
                    "".into(),
                    "Handoff fixture".into(),
                ]],
                vec!["next_menu".into()],
            ),
        )
        .unwrap();
        wait_until(|| unsafe { IsWindowVisible(hwnd) != 0 && GetForegroundWindow() == hwnd });
        std::thread::sleep(Duration::from_millis(200));
        pump_events();
        let before = HIDE_EVENTS.load(Ordering::Relaxed);
        key(hwnd, 0x0D);
        let (requested_session, outcome) = receive.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(requested_session, session);
        assert!(
            matches!(outcome, Outcome::Action(Action::ShowPicker(picker)) if picker.name == "next_menu")
        );
        // Deliberately delay the main-thread handoff. No hide or focus loss is allowed.
        std::thread::sleep(Duration::from_millis(150));
        pump_events();
        assert_eq!(HIDE_EVENTS.load(Ordering::Relaxed), before);
        assert_ne!(unsafe { IsWindowVisible(hwnd) }, 0);
        assert_eq!(unsafe { GetForegroundWindow() }, hwnd);
        match index {
            0 => host
                .show_request(
                    session + 1,
                    bounds,
                    data(
                        "Details".into(),
                        vec!["Message".into()],
                        vec![DataRow {
                            cells: vec!["new row".into()],
                            preview: Some("replacement preview".into()),
                        }],
                    ),
                )
                .unwrap(),
            1 => host
                .show_request(session + 1, bounds, Box::new(Windows))
                .unwrap(),
            2 => host
                .show_request(session + 1, bounds, Box::new(Processes))
                .unwrap(),
            3 => host
                .show_request(
                    session + 1,
                    bounds,
                    files(
                        nfm_file_system::FileSystemPickerOptions {
                            roots: vec![handoff_root.to_string_lossy().into_owned()],
                            ..Default::default()
                        },
                        FileAction::Launch { elevated: false },
                    ),
                )
                .unwrap(),
            _ => host
                .show_request(
                    session + 1,
                    bounds,
                    files(
                        nfm_file_system::FileSystemPickerOptions {
                            roots: vec![handoff_root.to_string_lossy().into_owned()],
                            max_depth: 5,
                            files_only: true,
                            preview_visible: false,
                            bat_theme: Some("gruvbox-dark".into()),
                            ..Default::default()
                        },
                        FileAction::Launch { elevated: false },
                    ),
                )
                .unwrap(),
        }
        std::thread::sleep(Duration::from_millis(250));
        pump_events();
        assert_eq!(
            HIDE_EVENTS.load(Ordering::Relaxed),
            before,
            "handoff hid the native popup"
        );
        assert_eq!(unsafe { GetForegroundWindow() }, hwnd);
        assert!(
            receive.try_recv().is_err(),
            "the original Enter reached the replacement"
        );
        key(hwnd, 0x1B);
        let (closed_session, outcome) = receive.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(closed_session, session + 1);
        assert!(matches!(outcome, Outcome::Cancelled));
        wait_until(|| unsafe { IsWindowVisible(hwnd) == 0 });
        pump_events();
        wait_until(|| HIDE_EVENTS.load(Ordering::Relaxed) > before);
    }
    unsafe {
        windows_sys::Win32::UI::Accessibility::UnhookWinEvent(hook);
    }
    std::fs::remove_dir(handoff_root).unwrap();
    drop(host);
    wait_until(|| unsafe { IsWindow(hwnd) == 0 });
    println!(
        "OpenGL host smoke passed: all menu types, visible command handoffs without native hide events, stale Enter suppression, slash completion navigation, typing, cancellation, reopen and shutdown"
    );
}
