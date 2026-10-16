//! The drop-down a bar mailbox opens below itself, listing what its producer last
//! published. It takes focus so that clicking elsewhere or Esc closes it; clicking
//! an entry tells the producer and closes it. Runs on the bar's thread.
use crate::mailbox::{Activation, BarMailbox, MailboxEvent, MailboxState};
use nfm_egui::egui;
use std::cell::RefCell;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::SystemTime;
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// Where the clicked mailbox is, in screen pixels.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Anchor {
    pub right: i32,
    /// The bar's bottom edge.
    pub top: i32,
    /// The bar's left and right edges, which the drop-down stays within.
    pub bounds: (i32, i32),
}

const WIDTH: f32 = 480.0;
/// Below the bar.
const OFFSET: i32 = 6;
const MAX_ROWS: usize = 6;
const PADDING: f32 = 16.0;
const ROW_PADDING: f32 = 10.0;
/// Room left of an entry's text for its state's dot.
const DOT_COLUMN: f32 = 22.0;
/// Refreshes relative times while open.
const CLOCK_TIMER: usize = 1;

use crate::bar::one_line as line;
use crate::palette::{BACKGROUND, BORDER, HOVER, MUTED, TEXT, TITLE};

static WINDOW: AtomicIsize = AtomicIsize::new(0);

struct Open {
    mailbox: usize,
    monitor: usize,
    events: Vec<egui::Event>,
    tracking_mouse: bool,
    /// The selected entry's id; the first entry when none is selected yet.
    selected: Option<String>,
}
thread_local! {
    static OPEN: RefCell<Option<Open>> = const { RefCell::new(None) };
}

/// Open the drop-down for `mailbox`, or close it if it is already open there.
pub(crate) fn toggle(mailbox: usize, monitor: usize, anchor: Anchor) {
    let same = OPEN.with(|open| {
        open.borrow()
            .as_ref()
            .is_some_and(|open| open.mailbox == mailbox && open.monitor == monitor)
    });
    close();
    if !same {
        show(mailbox, monitor, anchor);
    }
}

/// Repaint the drop-down if it is open; callable from any thread.
pub(crate) fn invalidate() {
    let hwnd = WINDOW.load(Ordering::Acquire);
    if hwnd != 0 && unsafe { IsWindowVisible(hwnd as HWND) } != 0 {
        unsafe { InvalidateRect(hwnd as HWND, std::ptr::null(), 0) };
    }
}

fn mailbox(index: usize) -> Option<BarMailbox> {
    crate::worker::snapshot()?
        .configuration
        .bar_mailboxes
        .get(index)
        .cloned()
}

/// The bar font's name and size on `monitor`.
fn font(monitor: usize) -> (String, f32) {
    crate::ui::bar_snapshot(monitor).map_or((String::new(), 16.0), |bar| {
        (bar.font_name, bar.normal_font_size.max(8) as f32)
    })
}

fn show(index: usize, monitor: usize, anchor: Anchor) {
    let Some(mailbox) = mailbox(index) else {
        return;
    };
    let Some(hwnd) = window() else {
        return;
    };
    OPEN.with(|open| {
        *open.borrow_mut() = Some(Open {
            mailbox: index,
            monitor,
            events: Vec::new(),
            tracking_mouse: false,
            selected: None,
        })
    });
    let (_, size) = font(monitor);
    let [width, height] = window_size(&Metrics::new(size), &mailbox.current());
    let left = (anchor.right - width).clamp(
        anchor.bounds.0,
        (anchor.bounds.1 - width).max(anchor.bounds.0),
    );
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            left,
            anchor.top + OFFSET,
            width,
            height,
            SWP_SHOWWINDOW,
        );
        // The bar click is input to this process, so it may take the foreground.
        SetForegroundWindow(hwnd);
        windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(hwnd);
        SetTimer(hwnd, CLOCK_TIMER, 30_000, None);
        InvalidateRect(hwnd, std::ptr::null(), 0);
    }
    mailbox.send(MailboxEvent::Opened);
}

fn close() {
    // Taken before hiding: hiding deactivates the window, which closes it again.
    let Some(open) = OPEN.with(|open| open.borrow_mut().take()) else {
        return;
    };
    let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
    unsafe {
        KillTimer(hwnd, CLOCK_TIMER);
        ShowWindow(hwnd, SW_HIDE);
    }
    if let Some(mailbox) = mailbox(open.mailbox) {
        mailbox.send(MailboxEvent::Closed);
    }
}

/// The drop-down window, created on first use and then reused.
fn window() -> Option<HWND> {
    let existing = WINDOW.load(Ordering::Acquire);
    if existing != 0 {
        return Some(existing as HWND);
    }
    use std::mem::{size_of, zeroed};
    let class_name: Vec<u16> = "SWMMailboxPopup\0".encode_utf16().collect();
    let instance =
        unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()) };
    let mut class: WNDCLASSEXW = unsafe { zeroed() };
    class.cbSize = size_of::<WNDCLASSEXW>() as u32;
    class.style = CS_OWNDC;
    class.lpfnWndProc = Some(window_proc);
    class.hInstance = instance;
    class.lpszClassName = class_name.as_ptr();
    class.hCursor = unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) };
    if unsafe { RegisterClassExW(&class) } == 0
        && unsafe { windows_sys::Win32::Foundation::GetLastError() }
            != windows_sys::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS
    {
        crate::error_log::last_win32("RegisterClassExW(mailbox popup) failed");
        return None;
    }
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            class_name.as_ptr(),
            class_name.as_ptr(),
            WS_POPUP,
            0,
            0,
            WIDTH as i32,
            100,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        crate::error_log::last_win32("CreateWindowExW(mailbox popup) failed");
        return None;
    }
    if let Err(error) = crate::bar_renderer::attach(hwnd) {
        crate::error_log::report("mailbox popup initialization", error);
        unsafe { DestroyWindow(hwnd) };
        return None;
    }
    // Rounded corners on Windows 11; ignored elsewhere.
    let round = windows_sys::Win32::Graphics::Dwm::DWMWCP_ROUND;
    unsafe {
        windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
            hwnd,
            windows_sys::Win32::Graphics::Dwm::DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            (&round as *const i32).cast(),
            size_of::<i32>() as u32,
        );
    }
    WINDOW.store(hwnd as isize, Ordering::Release);
    Some(hwnd)
}

fn push_event(event: egui::Event) {
    OPEN.with(|open| {
        if let Some(open) = open.borrow_mut().as_mut() {
            open.events.push(event);
        }
    });
    invalidate();
}

fn point(lparam: isize) -> egui::Pos2 {
    egui::pos2(
        (lparam & 0xffff) as i16 as f32,
        ((lparam >> 16) & 0xffff) as i16 as f32,
    )
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match message {
        WM_PAINT => {
            let mut paint: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            unsafe { BeginPaint(hwnd, &mut paint) };
            let action = paint_open(hwnd);
            unsafe { EndPaint(hwnd, &paint) };
            match action {
                Some(PopupAction::Close) => close(),
                Some(PopupAction::Activated(id, how)) => {
                    send(MailboxEvent::Activated { id, how });
                    close();
                }
                None => {}
            }
            0
        }
        WM_MOUSEMOVE => {
            let start_tracking = OPEN.with(|open| {
                open.borrow_mut()
                    .as_mut()
                    .is_some_and(|open| !std::mem::replace(&mut open.tracking_mouse, true))
            });
            if start_tracking {
                use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
                    TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
                };
                let mut track = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                unsafe { TrackMouseEvent(&mut track) };
            }
            push_event(egui::Event::PointerMoved(point(lparam)));
            0
        }
        windows_sys::Win32::UI::Controls::WM_MOUSELEAVE => {
            OPEN.with(|open| {
                if let Some(open) = open.borrow_mut().as_mut() {
                    open.tracking_mouse = false;
                }
            });
            push_event(egui::Event::PointerGone);
            0
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP => {
            push_event(egui::Event::PointerButton {
                pos: point(lparam),
                button: egui::PointerButton::Primary,
                pressed: message == WM_LBUTTONDOWN,
                modifiers: egui::Modifiers::default(),
            });
            0
        }
        WM_MOUSEWHEEL => {
            let notches = ((wparam >> 16) as u16 as i16) as f32 / WHEEL_DELTA as f32;
            push_event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, notches),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::default(),
            });
            0
        }
        WM_KEYDOWN
            if wparam as u16 == windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE =>
        {
            close();
            0
        }
        WM_KEYDOWN => {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::{VK_DOWN, VK_RETURN, VK_UP};
            let key = match wparam as u16 {
                VK_DOWN => Some(egui::Key::ArrowDown),
                VK_UP => Some(egui::Key::ArrowUp),
                VK_RETURN => Some(egui::Key::Enter),
                key if key == b'J' as u16 => Some(egui::Key::J),
                key if key == b'K' as u16 => Some(egui::Key::K),
                _ => None,
            };
            match key {
                Some(key) => {
                    push_event(egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        // Bit 30: the key was already down, so this is a repeat.
                        repeat: lparam & (1 << 30) != 0,
                        modifiers: egui::Modifiers::default(),
                    });
                    0
                }
                None => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
            }
        }
        WM_ACTIVATE if wparam & 0xffff == WA_INACTIVE as usize => {
            close();
            0
        }
        WM_TIMER => {
            unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
            0
        }
        WM_ERASEBKGND => 1,
        WM_NCDESTROY => {
            crate::bar_renderer::detach(hwnd);
            WINDOW.store(0, Ordering::Release);
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn send(event: MailboxEvent) {
    let index = OPEN.with(|open| open.borrow().as_ref().map(|open| open.mailbox));
    if let Some(mailbox) = index.and_then(mailbox) {
        mailbox.send(event);
    }
}

/// Paints the open drop-down, first resizing it to what it now lists.
fn paint_open(hwnd: HWND) -> Option<PopupAction> {
    let (index, monitor, events, mut selected) = OPEN.with(|open| {
        open.borrow_mut().as_mut().map(|open| {
            (
                open.mailbox,
                open.monitor,
                std::mem::take(&mut open.events),
                open.selected.take(),
            )
        })
    })?;
    let mailbox = mailbox(index)?;
    let state = mailbox.current();
    let (font_name, font_size) = font(monitor);
    let metrics = Metrics::new(font_size);
    let size = window_size(&metrics, &state);
    let mut client: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(hwnd, &mut client) };
    if client.bottom != size[1] {
        unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                size[0],
                size[1],
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
    }
    let mut action = None;
    let now = SystemTime::now();
    match crate::bar_renderer::paint_ui(
        hwnd,
        [size[0] as u32, size[1] as u32],
        &font_name,
        events,
        |ui| action = popup_ui(ui, &mailbox, &state, &metrics, now, &mut selected),
    ) {
        Ok(true) => unsafe {
            InvalidateRect(hwnd, std::ptr::null(), 0);
        },
        Ok(false) => {}
        Err(error) => crate::error_log::report("mailbox popup paint", error),
    }
    OPEN.with(|open| {
        if let Some(open) = open.borrow_mut().as_mut() {
            open.selected = selected;
        }
    });
    action
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PopupAction {
    Close,
    Activated(String, Activation),
}

/// Sizes derived from the bar font.
pub(crate) struct Metrics {
    normal: egui::FontId,
    small: egui::FontId,
    header: f32,
    row: f32,
}
impl Metrics {
    pub(crate) fn new(size: f32) -> Self {
        let family = egui::FontFamily::Name("bar".into());
        let small = (size * 0.8).round();
        Self {
            normal: egui::FontId::new(size, family.clone()),
            small: egui::FontId::new(small, family),
            header: (size * 2.6).round(),
            row: (ROW_PADDING * 2.0 + (small * 2.0 + size) * 1.35).round(),
        }
    }
}

/// The window's size: a row per entry up to `MAX_ROWS`, which then scroll.
fn window_size(metrics: &Metrics, state: &MailboxState) -> [i32; 2] {
    let rows = state.entries.len().clamp(1, MAX_ROWS);
    [
        WIDTH as i32,
        (metrics.header + 1.0 + rows as f32 * metrics.row) as i32,
    ]
}

/// A text button; returns whether it was clicked.
fn button(ui: &mut egui::Ui, id: &str, rect: egui::Rect, text: &str, font: &egui::FontId) -> bool {
    let response = ui.interact(rect, ui.id().with(id), egui::Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 6, HOVER);
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        font.clone(),
        if response.hovered() { TITLE } else { TEXT },
    );
    response.clicked()
}

/// Draws the drop-down filling the window; returns what the person did. Down or j
/// and Up or k move `selected`, stopping at either end, and Enter clicks it;
/// moving the mouse selects the entry under it.
pub(crate) fn popup_ui(
    ui: &mut egui::Ui,
    mailbox: &BarMailbox,
    state: &MailboxState,
    metrics: &Metrics,
    now: SystemTime,
    selected: &mut Option<String>,
) -> Option<PopupAction> {
    let full = ui.max_rect();
    let painter = ui.painter().clone();
    painter.rect_filled(full, 0, BACKGROUND);
    painter.rect_stroke(
        full.shrink(0.5),
        0,
        egui::Stroke::new(1.0, BORDER),
        egui::StrokeKind::Inside,
    );
    let mut action = None;

    let header = egui::Rect::from_min_size(full.min, egui::vec2(full.width(), metrics.header));
    let title = painter.layout_no_wrap(mailbox.name.clone(), metrics.normal.clone(), TITLE);
    painter.galley(
        egui::pos2(
            full.left() + PADDING,
            header.center().y - title.size().y / 2.0,
        ),
        title,
        TITLE,
    );
    let close_size = metrics.header - 16.0;
    let close = egui::Rect::from_center_size(
        egui::pos2(
            full.right() - PADDING / 2.0 - close_size / 2.0,
            header.center().y,
        ),
        egui::vec2(close_size, close_size),
    );
    if button(ui, "close", close, "✕", &metrics.normal) {
        action = Some(PopupAction::Close);
    }
    painter.hline(
        full.x_range(),
        header.bottom(),
        egui::Stroke::new(1.0, BORDER),
    );

    let list = egui::Rect::from_min_max(egui::pos2(full.left(), header.bottom() + 1.0), full.max);
    if state.entries.is_empty() {
        painter.text(
            list.center(),
            egui::Align2::CENTER_CENTER,
            "Nothing here",
            metrics.small.clone(),
            MUTED,
        );
        return action;
    }
    let (down, up, enter, pointer_moved) = ui.input(|input| {
        let presses =
            |keys: [egui::Key; 2]| -> usize { keys.map(|key| input.num_presses(key)).iter().sum() };
        (
            presses([egui::Key::ArrowDown, egui::Key::J]),
            presses([egui::Key::ArrowUp, egui::Key::K]),
            input.key_pressed(egui::Key::Enter),
            input.pointer.delta() != egui::Vec2::ZERO,
        )
    });
    let current = selected
        .as_deref()
        .and_then(|id| state.entries.iter().position(|entry| entry.id == id))
        .unwrap_or(0);
    let mut index = (current + down)
        .saturating_sub(up)
        .min(state.entries.len() - 1);
    let mut hovered = None;
    let mut list_ui = ui.new_child(egui::UiBuilder::new().max_rect(list));
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(&mut list_ui, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            for (row_index, entry) in state.entries.iter().enumerate() {
                let (row, response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), metrics.row),
                    egui::Sense::click(),
                );
                let painter = ui.painter_at(row);
                if response.hovered() {
                    hovered = Some(row_index);
                }
                if row_index == index {
                    painter.rect_filled(row.shrink2(egui::vec2(6.0, 2.0)), 6, HOVER);
                    if down + up > 0 {
                        response.scroll_to_me(None);
                    }
                }
                if response.clicked() {
                    action = Some(PopupAction::Activated(entry.id.clone(), Activation::Click));
                }
                let small_line = metrics.small.size * 1.35;
                let normal_line = metrics.normal.size * 1.35;
                let top = row.top() + ROW_PADDING;
                let left = row.left() + PADDING + DOT_COLUMN;
                let right = row.right() - PADDING;
                let color = mailbox
                    .style(&entry.state)
                    .and_then(|style| style.icon.as_ref())
                    .map_or(MUTED, |icon| icon.color);
                painter.circle_filled(
                    egui::pos2(
                        row.left() + PADDING + 5.0,
                        top + small_line + normal_line / 2.0,
                    ),
                    5.0,
                    color,
                );
                // Relative time is at the top line's right end.
                let time = line(
                    &painter,
                    &crate::mailbox::relative_time(entry.time, now),
                    &metrics.small,
                    MUTED,
                    f32::INFINITY,
                );
                let time_width = time.size().x;
                painter.galley(egui::pos2(right - time_width, top), time, MUTED);
                let source = line(
                    &painter,
                    &entry.source,
                    &metrics.small,
                    MUTED,
                    right - time_width - 12.0 - left,
                );
                painter.galley(egui::pos2(left, top), source, MUTED);
                let title = line(&painter, &entry.title, &metrics.normal, TITLE, right - left);
                painter.galley(egui::pos2(left, top + small_line), title, TITLE);
                let detail = line(&painter, &entry.detail, &metrics.small, MUTED, right - left);
                painter.galley(
                    egui::pos2(left, top + small_line + normal_line),
                    detail,
                    MUTED,
                );
                painter.hline(
                    (row.left() + PADDING)..=(row.right() - PADDING),
                    row.bottom() - 0.5,
                    egui::Stroke::new(1.0, HOVER),
                );
            }
        });
    // Rows are drawn by now, so the next frame shows the hovered one selected.
    if pointer_moved && let Some(row_index) = hovered.filter(|&row_index| row_index != index) {
        index = row_index;
        ui.ctx().request_repaint();
    }
    let id = &state.entries[index].id;
    if enter {
        action = Some(PopupAction::Activated(id.clone(), Activation::Enter));
    }
    *selected = Some(id.clone());
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mailbox::{Icon, MailboxEntry, MailboxSender, StateStyle};
    use std::time::Duration;

    fn mailbox() -> BarMailbox {
        fn producer(_: MailboxSender) {}
        let mut configuration = crate::Configuration::default();
        configuration.bar_mailbox("Agents", producer).state(
            "done",
            StateStyle::icon(Icon::glyph("✓", egui::Color32::GREEN)),
        );
        configuration.bar_mailboxes.remove(0)
    }

    fn state(now: SystemTime) -> MailboxState {
        let entry = |id: &str| MailboxEntry {
            id: id.into(),
            state: "done".into(),
            source: "SimpleWindowManager/rust-port".into(),
            title: "Take a look at the sizing of screenshots in these new guides too".into(),
            detail: "Edit src/bar.rs".into(),
            time: now - Duration::from_secs(120),
        };
        MailboxState {
            counts: vec![("done".into(), 2)],
            entries: vec![entry("first"), entry("second")],
        }
    }

    /// Runs frames of the drop-down with `events`, one frame per batch.
    fn run(batches: Vec<Vec<egui::Event>>) -> Vec<Option<PopupAction>> {
        run_selecting(batches)
            .into_iter()
            .map(|(action, _)| action)
            .collect()
    }

    /// Like `run`, with the selected entry after each frame.
    fn run_selecting(batches: Vec<Vec<egui::Event>>) -> Vec<(Option<PopupAction>, String)> {
        let mut selected = None;
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::bar_renderer::bar_fonts(""));
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let (mailbox, state) = (mailbox(), state(now));
        let metrics = Metrics::new(16.0);
        let size = window_size(&metrics, &state);
        batches
            .into_iter()
            .map(|events| {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(size[0] as f32, size[1] as f32),
                    )),
                    events,
                    ..Default::default()
                };
                let mut action = None;
                let output = ctx.run_ui(input, |ui| {
                    action = popup_ui(ui, &mailbox, &state, &metrics, now, &mut selected)
                });
                output.drop_without_applying_deltas();
                (action, selected.clone().unwrap_or_default())
            })
            .collect()
    }

    fn keys(keys: &[egui::Key]) -> Vec<Vec<egui::Event>> {
        keys.iter()
            .map(|&key| {
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::default(),
                }]
            })
            .collect()
    }

    #[test]
    fn keys_move_the_selection_and_stop_at_either_end() {
        use egui::Key::*;
        let selected: Vec<_> = run_selecting(keys(&[J, ArrowDown, K, ArrowDown, ArrowUp, K]))
            .into_iter()
            .map(|(_, selected)| selected)
            .collect();
        assert_eq!(
            selected,
            ["second", "second", "first", "second", "first", "first"]
        );
    }

    #[test]
    fn enter_clicks_the_selected_entry() {
        let actions = run(keys(&[egui::Key::Enter]));
        assert_eq!(
            actions[0],
            Some(PopupAction::Activated("first".into(), Activation::Enter))
        );
        let actions = run(keys(&[egui::Key::J, egui::Key::Enter]));
        assert_eq!(
            actions[1],
            Some(PopupAction::Activated("second".into(), Activation::Enter))
        );
    }

    #[test]
    fn moving_the_mouse_selects_the_entry_under_it() {
        let metrics = Metrics::new(16.0);
        let row = |index: f32| egui::pos2(200.0, metrics.header + 1.0 + metrics.row * index);
        let frames = run_selecting(vec![
            vec![egui::Event::PointerMoved(row(0.5))],
            vec![egui::Event::PointerMoved(row(1.5))],
        ]);
        assert_eq!(frames[1].1, "second");
    }

    fn click(at: egui::Pos2) -> Vec<Vec<egui::Event>> {
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        vec![
            vec![egui::Event::PointerMoved(at)],
            vec![button(true)],
            vec![button(false)],
        ]
    }

    #[test]
    fn the_window_fits_its_entries_and_scrolls_beyond_the_maximum() {
        let metrics = Metrics::new(16.0);
        let now = SystemTime::now();
        let [_, two] = window_size(&metrics, &state(now));
        assert_eq!(two, (metrics.header + 1.0 + 2.0 * metrics.row) as i32);
        let mut many = state(now);
        many.entries = std::iter::repeat_n(many.entries[0].clone(), 40).collect();
        let [_, capped] = window_size(&metrics, &many);
        assert_eq!(
            capped,
            (metrics.header + 1.0 + MAX_ROWS as f32 * metrics.row) as i32
        );
    }

    #[test]
    fn clicking_an_entry_reports_it() {
        let metrics = Metrics::new(16.0);
        let second_row = egui::pos2(200.0, metrics.header + 1.0 + metrics.row * 1.5);
        let actions = run(click(second_row));
        assert_eq!(
            actions.last().unwrap(),
            &Some(PopupAction::Activated("second".into(), Activation::Click))
        );
    }

    #[test]
    fn the_header_closes() {
        let metrics = Metrics::new(16.0);
        let middle = metrics.header / 2.0;
        let close = run(click(egui::pos2(WIDTH - PADDING / 2.0 - 4.0, middle)));
        assert_eq!(close.last().unwrap(), &Some(PopupAction::Close));
    }
}
