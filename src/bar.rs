use crate::palette;
use nfm_egui::egui;

#[derive(Clone, Debug)]
pub struct BarSnapshot {
    pub monitor_index: usize,
    pub selected_monitor: bool,
    pub tags: Vec<(usize, String, bool, bool, bool)>,
    pub workspace_info: String,
    pub focused_window: String,
    pub segments: Vec<RenderedSegment>,
    pub focus_style: crate::config::BarFocusStyle,
    /// Each configured mailbox with what its producer last published.
    pub mailboxes: Vec<(
        crate::mailbox::BarMailbox,
        std::sync::Arc<crate::mailbox::MailboxState>,
    )>,
    pub background_color: u32,
    pub focus_background_color: u32,
    pub text_color: u32,
    pub info_color: u32,
    pub disabled_color: u32,
    pub focus_text_color: u32,
    pub extra_focus_background_color: u32,
    pub font_name: String,
    pub normal_font_size: i32,
    pub icon_font_size: i32,
}

#[derive(Clone, Debug)]
pub struct RenderedSegment {
    pub separator: Option<(String, bool)>,
    pub header: Option<(String, bool)>,
    pub value: String,
    pub value_is_icon: bool,
    pub tone: Tone,
}

/// How a segment's value reads, which sets its color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Normal,
    Warning,
    Critical,
    /// Muted, such as volume at zero.
    Off,
}

/// CPU and memory use at or above these read as a warning or critical.
const WARNING_PERCENT: u32 = 75;
const CRITICAL_PERCENT: u32 = 90;

fn tone(value: crate::config::BarSegmentValue, text: &str) -> Tone {
    use crate::config::BarSegmentValue::*;
    let percent = || text.trim().trim_end_matches('%').trim().parse::<u32>().ok();
    match value {
        Cpu | Memory => match percent() {
            Some(percent) if percent >= CRITICAL_PERCENT => Tone::Critical,
            Some(percent) if percent >= WARNING_PERCENT => Tone::Warning,
            _ => Tone::Normal,
        },
        Volume if percent() == Some(0) => Tone::Off,
        Internet if text == crate::status::INTERNET_OFFLINE => Tone::Critical,
        Internet if text == crate::status::INTERNET_UNKNOWN => Tone::Warning,
        _ => Tone::Normal,
    }
}

#[derive(Debug)]
pub struct BarWindow {
    pub hwnd: isize,
    pub monitor_index: usize,
}

unsafe extern "system" fn bar_message_loop(
    hwnd: windows_sys::Win32::Foundation::HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
    use windows_sys::Win32::UI::Controls::WM_MOUSELEAVE;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let span = matches!(
        message,
        WM_NCCREATE | WM_PAINT | WM_TIMER | WM_LBUTTONDOWN | WM_CLOSE | WM_DESTROY
    )
    .then(|| {
        tracing::info_span!(
            "window_message",
            component = "bar",
            message_name = crate::platform::window_message_name(message),
            message,
            hwnd = hwnd as usize,
            wparam,
            lparam,
        )
    });
    let _entered = span.as_ref().map(tracing::Span::enter);
    match message {
        WM_NCCREATE => {
            let create = unsafe { &*(lparam as *const CREATESTRUCTW) };
            unsafe {
                windows_sys::Win32::Foundation::SetLastError(0);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
                let error = windows_sys::Win32::Foundation::GetLastError();
                if error != 0 {
                    crate::error_log::report_win32("SetWindowLongPtrW(bar userdata) failed", error);
                    return 0;
                }
            }
            1
        }
        WM_PAINT => {
            let Some(snapshot) = crate::platform::bar_snapshot(bar_monitor(hwnd)) else {
                return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
            };
            let mut paint: PAINTSTRUCT = unsafe { zeroed() };
            let dc = unsafe { BeginPaint(hwnd, &mut paint) };
            if dc.is_null() {
                crate::platform_error::PlatformError::null_output("BeginPaint(bar)").report();
                return 0;
            }
            let mut client: RECT = unsafe { zeroed() };
            if unsafe { GetClientRect(hwnd, &mut client) } == 0 {
                crate::platform_error::PlatformError::last_win32("GetClientRect(bar)").report();
                // A successful BeginPaint must be paired with EndPaint, even
                // when later paint setup fails.
                unsafe { EndPaint(hwnd, &paint) };
                return 0;
            }
            let actions = paint_window(
                hwnd,
                [client.right.max(0) as u32, client.bottom.max(0) as u32],
                &snapshot,
            );
            // Repaint quickly only while a mailbox shows an animated state.
            if snapshot
                .mailboxes
                .iter()
                .any(|(mailbox, state)| mailbox.animated(state))
            {
                unsafe { SetTimer(hwnd, ANIMATION_TIMER, ANIMATION_INTERVAL_MS, None) };
            } else {
                unsafe { KillTimer(hwnd, ANIMATION_TIMER) };
            }
            // EndPaint is documented to always return nonzero.
            unsafe { EndPaint(hwnd, &paint) };
            // Clicks open pickers and switch workspaces, which create windows and
            // send messages: only once painting is over.
            run_actions(hwnd, actions);
            0
        }
        WM_TIMER => {
            if unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) } == 0 {
                crate::error_log::last_win32("InvalidateRect(bar timer) failed");
            }
            0
        }
        WM_ERASEBKGND => 1,
        WM_MOUSEACTIVATE => MA_NOACTIVATE as isize,
        WM_SIZE => {
            unsafe {
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_NCDESTROY => {
            detach(hwnd);
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP => {
            pointer_input(hwnd, message, wparam, lparam);
            0
        }
        WM_MOUSELEAVE => {
            pointer_left(hwnd);
            0
        }
        WM_SETCURSOR if (lparam & 0xffff) as u32 == HTCLIENT => {
            set_cursor(with_bar(hwnd, |bar| bar.cursor).unwrap_or_default());
            1
        }
        WM_CLOSE => {
            if unsafe { DestroyWindow(hwnd) } == 0 {
                crate::error_log::last_win32("DestroyWindow(bar) failed");
            }
            0
        }
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

/// The monitor index `hwnd`'s bar was created for.
fn bar_monitor(hwnd: windows_sys::Win32::Foundation::HWND) -> usize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GWLP_USERDATA, GetWindowLongPtrW};
    unsafe { windows_sys::Win32::Foundation::SetLastError(0) };
    let userdata = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
    let userdata_error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    if userdata == 0 && userdata_error != 0 {
        crate::error_log::report_win32("GetWindowLongPtrW(bar userdata) failed", userdata_error);
    }
    (userdata as usize).saturating_sub(1)
}

/// What a click on the bar asks for. A frame only records these; they run after
/// painting, since they create windows and send messages.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum BarAction {
    /// Open or close the commands launcher below its button, whose edges these
    /// are in bar pixels.
    ToggleLauncher { left: f32, right: f32 },
    /// Open or close mailbox `index`'s picker below its region's edges, or at the
    /// bar's right end while the mailbox shows nothing.
    ToggleMailbox {
        index: usize,
        span: Option<(f32, f32)>,
    },
    /// Switch this monitor to the workspace with this id.
    Workspace(usize),
    /// Close any open picker: a press on the bar that opens none moves on from it.
    /// It comes before the press's own action.
    Dismiss,
}

/// A bar window's own state, from `attach` until its `WM_NCDESTROY`.
pub(crate) struct BarState {
    /// Taken out while it paints, so no borrow of `BARS` is held across the
    /// native calls painting makes.
    renderer: Option<crate::bar_renderer::BarRenderer>,
    /// Mouse input for the next frame.
    events: Vec<egui::Event>,
    /// A mailbox for the next frame to open, asked for by a key binding.
    open_mailbox: Option<usize>,
    /// Whether the bar has asked for `WM_MOUSELEAVE`, which it does as the cursor
    /// arrives.
    tracking_leave: bool,
    /// The cursor the last frame asked for.
    cursor: egui::CursorIcon,
}

thread_local! {
    /// Each bar's state, by its window handle. Bars and their messages all run on
    /// the UI thread.
    static BARS: std::cell::RefCell<std::collections::HashMap<isize, BarState>> =
        Default::default();
}

/// Runs `f` on `hwnd`'s state; `None` before `attach` or after `detach`. `f` must
/// not make calls that can dispatch window messages.
fn with_bar<R>(
    hwnd: windows_sys::Win32::Foundation::HWND,
    f: impl FnOnce(&mut BarState) -> R,
) -> Option<R> {
    BARS.with(|bars| bars.borrow_mut().get_mut(&(hwnd as isize)).map(f))
}

/// Gives the new bar `hwnd` its state and renderer.
pub(crate) fn attach(hwnd: windows_sys::Win32::Foundation::HWND) -> Result<(), String> {
    let renderer = crate::bar_renderer::BarRenderer::new(hwnd)?;
    let state = BarState {
        renderer: Some(renderer),
        events: Vec::new(),
        open_mailbox: None,
        tracking_leave: false,
        cursor: egui::CursorIcon::Default,
    };
    BARS.with(|bars| bars.borrow_mut().insert(hwnd as isize, state));
    Ok(())
}

/// Drops `hwnd`'s state and renderer, as the window is destroyed.
pub(crate) fn detach(hwnd: windows_sys::Win32::Foundation::HWND) {
    // Dropped after the borrow ends: releasing the renderer makes native calls.
    let state = BARS.with(|bars| bars.borrow_mut().remove(&(hwnd as isize)));
    drop(state);
}

/// Runs a frame of `hwnd` for `snapshot`, with the input since the last one, and
/// returns what its clicks asked for.
fn paint_window(
    hwnd: windows_sys::Win32::Foundation::HWND,
    size: [u32; 2],
    snapshot: &BarSnapshot,
) -> Vec<BarAction> {
    let Some((Some(mut renderer), events, open_mailbox)) = with_bar(hwnd, |bar| {
        (
            bar.renderer.take(),
            std::mem::take(&mut bar.events),
            bar.open_mailbox.take(),
        )
    }) else {
        // WM_PAINT can arrive before `attach`, or while a paint is under way.
        return Vec::new();
    };
    let frame = renderer.paint(size, snapshot, events, open_mailbox, true);
    // If the window was destroyed while painting, the renderer drops here.
    let over_bar = with_bar(hwnd, |bar| {
        bar.renderer = Some(renderer);
        if let Ok(Some(frame)) = &frame {
            bar.cursor = frame.cursor;
        }
        bar.tracking_leave
    });
    match frame {
        Ok(Some(frame)) => {
            // WM_SETCURSOR came before the move this frame saw; show its answer now.
            if over_bar == Some(true) {
                set_cursor(frame.cursor);
            }
            if frame.repaint {
                invalidate(hwnd);
            }
            frame.actions
        }
        // A frame that did not run still answers a key binding's mailbox.
        Ok(None) => open_mailbox
            .map(|index| BarAction::ToggleMailbox { index, span: None })
            .into_iter()
            .collect(),
        Err(error) => {
            crate::error_log::report("egui bar paint", error);
            Vec::new()
        }
    }
}

/// Runs a frame's `actions` for the bar `hwnd`.
fn run_actions(hwnd: windows_sys::Win32::Foundation::HWND, actions: Vec<BarAction>) {
    for action in actions {
        match action {
            BarAction::ToggleLauncher { left, right } => {
                if let Some(bar) = window_rect(hwnd) {
                    crate::egui_command_menu::toggle_launcher(anchor(
                        &bar,
                        bar.left + left as i32,
                        bar.left + right as i32,
                    ));
                }
            }
            BarAction::ToggleMailbox { index, span } => {
                if let Some(bar) = window_rect(hwnd) {
                    let (left, right) = span.map_or((bar.right, bar.right), |(left, right)| {
                        (bar.left + left as i32, bar.left + right as i32)
                    });
                    crate::mailbox_picker::toggle(index, anchor(&bar, left, right));
                }
            }
            BarAction::Workspace(workspace) => {
                crate::platform::bar_workspace_clicked(bar_monitor(hwnd), workspace)
            }
            BarAction::Dismiss => {
                if crate::ui::menu_visible() {
                    crate::ui::post(crate::ui::Command::Hide);
                }
            }
        }
    }
}

/// Queues a mouse move, or a left or right press or release, for `hwnd`'s next
/// frame. `wparam` holds the buttons down after the message.
fn pointer_input(
    hwnd: windows_sys::Win32::Foundation::HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
) {
    use windows_sys::Win32::System::SystemServices::{MK_LBUTTON, MK_RBUTTON};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_RBUTTONDOWN,
    };
    let pos = pixel(lparam as i16 as i32, (lparam >> 16) as i16 as i32);
    let pressed = matches!(message, WM_LBUTTONDOWN | WM_RBUTTONDOWN);
    let Some(arriving) = with_bar(hwnd, |bar| {
        bar.events.push(egui::Event::PointerMoved(pos));
        if message != WM_MOUSEMOVE {
            bar.events.push(egui::Event::PointerButton {
                pos,
                button: if matches!(message, WM_LBUTTONDOWN | WM_LBUTTONUP) {
                    egui::PointerButton::Primary
                } else {
                    egui::PointerButton::Secondary
                },
                pressed,
                modifiers: egui::Modifiers::default(),
            });
        }
        !std::mem::replace(&mut bar.tracking_leave, true)
    }) else {
        return;
    };
    if arriving {
        let mut track = TRACKMOUSEEVENT {
            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: hwnd,
            dwHoverTime: 0,
        };
        if unsafe { TrackMouseEvent(&mut track) } == 0 {
            crate::error_log::last_win32("TrackMouseEvent(bar) failed");
        }
    }
    if message == WM_MOUSEMOVE {
        invalidate(hwnd);
    } else {
        // Capture keeps the release with the bar, so egui never sees a button
        // stuck down, even when the press opened a picker under the cursor.
        // It lasts until the last button held comes up.
        if pressed {
            unsafe { SetCapture(hwnd) };
        } else if wparam as u32 & (MK_LBUTTON | MK_RBUTTON) == 0 {
            unsafe { ReleaseCapture() };
        }
        // A click acts now rather than whenever the bar next paints.
        repaint_now(hwnd);
    }
}

/// The egui position of the bar's pixel (`x`, `y`) from a window message: its
/// center, since the pixel covers `x` to `x + 1`. egui rects include their right
/// edge, so this keeps each pixel in exactly one target. Bar pixels are egui
/// points, one to one; see `bar_renderer::bar_input`.
fn pixel(x: i32, y: i32) -> egui::Pos2 {
    egui::pos2(x as f32 + 0.5, y as f32 + 0.5)
}

/// Tells `hwnd`'s next frame that the cursor left it.
fn pointer_left(hwnd: windows_sys::Win32::Foundation::HWND) {
    let left = with_bar(hwnd, |bar| {
        bar.tracking_leave = false;
        bar.events.push(egui::Event::PointerGone);
    });
    if left.is_some() {
        invalidate(hwnd);
    }
}

/// Opens or closes the picker of mailbox `index` below the bar `hwnd`: under the
/// mailbox's region, or at the bar's right end while the mailbox shows nothing.
pub(crate) fn toggle_mailbox(hwnd: windows_sys::Win32::Foundation::HWND, index: usize) {
    // The frame knows where the mailbox is, so it opens the picker.
    if with_bar(hwnd, |bar| bar.open_mailbox = Some(index)).is_none() {
        return;
    }
    repaint_now(hwnd);
    // A bar that did not paint, such as a hidden one, still opens the picker.
    if let Some(Some(index)) = with_bar(hwnd, |bar| bar.open_mailbox.take()) {
        run_actions(hwnd, vec![BarAction::ToggleMailbox { index, span: None }]);
    }
}

/// Asks for a repaint of `hwnd` whenever it next paints.
fn invalidate(hwnd: windows_sys::Win32::Foundation::HWND) {
    if unsafe { windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 0) } == 0
    {
        crate::error_log::last_win32("InvalidateRect(bar) failed");
    }
}

/// Repaints `hwnd`, and runs what that frame asks for, before returning.
fn repaint_now(hwnd: windows_sys::Win32::Foundation::HWND) {
    invalidate(hwnd);
    // Sends WM_PAINT directly; a hidden window has nothing to paint.
    unsafe { windows_sys::Win32::Graphics::Gdi::UpdateWindow(hwnd) };
}

/// Shows `cursor`, as a frame asked for it, over the bar.
fn set_cursor(cursor: egui::CursorIcon) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IDC_ARROW, IDC_HAND, LoadCursorW, SetCursor,
    };
    let shape = match cursor {
        egui::CursorIcon::PointingHand => IDC_HAND,
        _ => IDC_ARROW,
    };
    unsafe { SetCursor(LoadCursorW(std::ptr::null_mut(), shape)) };
}

/// Whether the primary button went down on `response` this frame. The bar acts on
/// the press, as menus open, rather than on the release.
fn pressed(response: &egui::Response) -> bool {
    response.is_pointer_button_down_on()
        && response.ctx.input(|input| input.pointer.primary_pressed())
}

/// Makes the bar's full height above and below `region` clickable, so the screen's
/// edge works too, with a hand cursor over it.
fn interact(
    ui: &egui::Ui,
    region: egui::Rect,
    id: impl std::hash::Hash + std::fmt::Debug,
) -> egui::Response {
    let column = egui::Rect::from_x_y_ranges(region.x_range(), ui.max_rect().y_range());
    ui.interact(column, egui::Id::new(("bar", id)), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Lights a hovered region from `draw_region`'s plain background.
fn draw_region_hover(canvas: &egui::Painter, rect: egui::Rect) {
    canvas.rect_filled(rect, 8, palette::BAR_HOVER);
}

/// The bar's regular repaint timer is id 0; this one runs only while animating.
const ANIMATION_TIMER: usize = 1;
const ANIMATION_INTERVAL_MS: u32 = 60;
/// Between a state's icon and its count, and between states.
const ICON_GAP: f32 = 6.0;
const STATE_GAP: f32 = 14.0;

fn window_rect(
    hwnd: windows_sys::Win32::Foundation::HWND,
) -> Option<windows_sys::Win32::Foundation::RECT> {
    let mut bar = unsafe { std::mem::zeroed() };
    if unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut bar) } == 0 {
        crate::error_log::last_win32("GetWindowRect(bar) failed");
        return None;
    }
    Some(bar)
}

/// A region from `left` to `right` on `bar`, in screen pixels.
fn anchor(
    bar: &windows_sys::Win32::Foundation::RECT,
    left: i32,
    right: i32,
) -> crate::egui_window_menu::BarAnchor {
    crate::egui_window_menu::BarAnchor {
        left,
        right,
        top: bar.bottom,
        bounds: (bar.left, bar.right),
    }
}

/// Paints mailbox `mailbox_index` at the right of `space`, lit while hovered, and
/// returns its interaction; `None` when no declared state has a count or there is
/// no room.
fn paint_mailbox(
    ui: &egui::Ui,
    mailbox: &crate::mailbox::BarMailbox,
    state: &crate::mailbox::MailboxState,
    mailbox_index: usize,
    font: &egui::FontId,
    shade: Shade,
    space: egui::Rect,
) -> Option<egui::Response> {
    use crate::mailbox::Shape;
    let canvas = &ui.ctx().layer_painter(egui::LayerId::background());
    let seconds = ui.input(|input| input.time);
    let visible = mailbox.visible(state);
    if visible.is_empty() {
        return None;
    }
    // Laid out uncolored: whether the mailbox is hovered, which sets its colors,
    // is known only once these widths place it.
    let galley = |text: &str| {
        canvas.layout_no_wrap(text.to_owned(), font.clone(), egui::Color32::PLACEHOLDER)
    };
    let header = mailbox.header.as_deref().map(galley);
    let parts: Vec<_> = visible
        .iter()
        .map(|(_, style, count)| {
            let icon = style.icon.as_ref();
            let icon_width = icon.map_or(0.0, |icon| match &icon.shape {
                // Wide enough for every frame, so the count does not move.
                Shape::Glyph(glyph) => std::iter::once(glyph)
                    .chain(match &icon.animation {
                        Some(crate::mailbox::Animation::Frames { frames, .. }) => frames.iter(),
                        _ => [].iter(),
                    })
                    .map(|text| galley(text).size().x)
                    .fold(0.0, f32::max),
                Shape::Drawn(_) => font.size,
            });
            let count = style.show_count.then(|| galley(&count.to_string()));
            (icon, icon_width, count)
        })
        .collect();
    let mut content = header
        .as_ref()
        .map_or(0.0, |header| header.size().x + ICON_GAP);
    for (index, (_, icon_width, count)) in parts.iter().enumerate() {
        if index > 0 {
            content += STATE_GAP;
        }
        content += icon_width;
        if let Some(count) = count {
            content += count.size().x + if *icon_width > 0.0 { ICON_GAP } else { 0.0 };
        }
    }
    let width = content + REGION_PADDING * 2.0;
    let region_left = (space.right() - width).max(space.left());
    if space.right() <= region_left {
        return None;
    }
    let region = egui::Rect::from_min_max(
        egui::pos2(region_left, space.top()),
        egui::pos2(space.right(), space.bottom()),
    );
    let response = interact(ui, region, ("mailbox", mailbox_index));
    let hovered = response.hovered();
    // Hover lights a dimmed mailbox up to full brightness.
    let shade = if hovered { Shade::Focused } else { shade };
    let text_color = shade.content();
    draw_region(canvas, region, false, false);
    if hovered {
        draw_region_hover(canvas, region);
    }
    let painter = canvas.with_clip_rect(region);
    let middle = region.center().y;
    let mut x = region.left() + REGION_PADDING;
    let text = |painter: &egui::Painter,
                x: &mut f32,
                galley: std::sync::Arc<egui::Galley>,
                color: egui::Color32| {
        let width = galley.size().x;
        painter.galley(
            egui::pos2(*x, middle - galley.size().y / 2.0),
            galley,
            color,
        );
        *x += width;
    };
    if let Some(header) = header {
        text(&painter, &mut x, header, shade.context());
        x += ICON_GAP;
    }
    for (index, (icon, icon_width, count)) in parts.into_iter().enumerate() {
        if index > 0 {
            x += STATE_GAP;
        }
        if let Some(icon) = icon {
            let frame = icon
                .animation
                .as_ref()
                .map_or(crate::mailbox::Frame::STILL, |a| a.frame(seconds));
            let color = shade.mark(icon.color).gamma_multiply(frame.opacity);
            let center = egui::pos2(x + icon_width / 2.0, middle);
            match &icon.shape {
                Shape::Glyph(glyph) => {
                    let glyph = match (&icon.animation, frame.glyph) {
                        (Some(crate::mailbox::Animation::Frames { frames, .. }), Some(i)) => {
                            &frames[i]
                        }
                        _ => glyph,
                    };
                    painter.text(
                        center,
                        egui::Align2::CENTER_CENTER,
                        glyph,
                        egui::FontId::new(font.size * frame.scale, font.family.clone()),
                        color,
                    );
                }
                Shape::Drawn(draw) => {
                    let size = font.size * frame.scale;
                    let area = egui::Rect::from_center_size(center, egui::vec2(size, size));
                    draw_shape(*draw, &painter, area, color);
                }
            }
            x += icon_width;
            if count.is_some() {
                x += ICON_GAP;
            }
        }
        if let Some(count) = count {
            let color = icon.map_or(text_color, |icon| shade.mark(icon.color));
            text(&painter, &mut x, count, color);
        }
    }
    Some(response)
}

/// Runs a configured shape; one that panics is reported once and then not drawn.
fn draw_shape(
    draw: crate::mailbox::DrawShape,
    painter: &egui::Painter,
    area: egui::Rect,
    color: egui::Color32,
) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static FAILED: AtomicBool = AtomicBool::new(false);
    if FAILED.load(Ordering::Relaxed) {
        return;
    }
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| draw(painter, area, color)));
    if result.is_err() && !FAILED.swap(true, Ordering::Relaxed) {
        crate::error_log::report(
            "bar mailbox",
            "a drawn shape panicked; it is no longer drawn",
        );
    }
}

const BAR_MARGIN: f32 = 2.0;
const REGION_GAP: f32 = 12.0;
const REGION_PADDING: f32 = 14.0;
const TAG_WIDTH: f32 = 42.0;
/// The commands launcher's button at the bar's left end: its left and right
/// edges, in bar pixels, and its Nerd Font glyph (`nf-fa-bars`).
const LAUNCHER: (f32, f32) = (BAR_MARGIN, BAR_MARGIN + 44.0);
const LAUNCHER_GLYPH: &str = "\u{f0c9}";

fn tag_groups(snapshot: &BarSnapshot) -> Vec<(usize, usize, f32, f32)> {
    if snapshot.tags.is_empty() {
        Vec::new()
    } else {
        let left = LAUNCHER.1 + REGION_GAP;
        let width = REGION_PADDING * 2.0 + snapshot.tags.len() as f32 * TAG_WIDTH;
        vec![(0, snapshot.tags.len(), left, left + width)]
    }
}

/// Paints the bar for `snapshot`, lighting what the pointer is over, and returns
/// what this frame's clicks, and `open_mailbox` from a key binding, ask for.
pub(super) fn paint_bar(
    ui: &mut egui::Ui,
    size: [u32; 2],
    snapshot: &BarSnapshot,
    open_mailbox: Option<usize>,
) -> Vec<BarAction> {
    let mut actions = Vec::new();
    use egui::{FontFamily, FontId, Rect, pos2};
    let canvas = ui.ctx().layer_painter(egui::LayerId::background());
    let normal_font = FontId::new(
        snapshot.normal_font_size.max(1) as f32,
        FontFamily::Name("bar".into()),
    );
    let icon_font = FontId::new(
        snapshot.icon_font_size.max(1) as f32,
        FontFamily::Name("bar".into()),
    );
    let rect = |left, top, right, bottom| Rect::from_min_max(pos2(left, top), pos2(right, bottom));
    let top = 4.0;
    let bottom = size[1] as f32 - 4.0;
    let shade = Shade::of(snapshot);
    let launcher = rect(LAUNCHER.0, top, LAUNCHER.1, bottom);
    let response = interact(ui, launcher, "launcher");
    if pressed(&response) {
        actions.push(BarAction::ToggleLauncher {
            left: launcher.left(),
            right: launcher.right(),
        });
    }
    draw_region(&canvas, launcher, false, false);
    if response.hovered() {
        draw_region_hover(&canvas, launcher);
    }
    draw_centered_text(
        &canvas,
        LAUNCHER_GLYPH,
        launcher,
        &icon_font,
        if response.hovered() {
            palette::TEXT
        } else {
            shade.content()
        },
    );
    let groups = tag_groups(snapshot);
    for (start, end, left, right) in &groups {
        draw_region(
            &canvas,
            rect(*left, top, *right, bottom),
            snapshot.selected_monitor,
            false,
        );
        for index in *start..*end {
            let (workspace, tag, selected, has_clients, is_icon) = &snapshot.tags[index];
            let item_left = *left + REGION_PADDING + (index - *start) as f32 * TAG_WIDTH;
            let item = rect(item_left, top + 3.0, item_left + TAG_WIDTH, bottom - 3.0);
            let response = interact(ui, item, ("workspace", *workspace));
            if pressed(&response) {
                actions.push(BarAction::Workspace(*workspace));
            }
            let tag_hovered = response.hovered();
            let fill = match (*selected, snapshot.selected_monitor, tag_hovered) {
                (true, true, false) => Some(palette::BACKGROUND),
                (true, true, true) => Some(palette::BAR_HOVER),
                (true, false, false) => Some(palette::INACTIVE),
                (true, false, true) => Some(palette::INACTIVE_HOVER),
                // A lighter accent keeps the tag's dark text readable.
                (false, true, true) => Some(palette::ACCENT_HOVER),
                (false, false, true) => Some(palette::BAR_HOVER),
                (false, _, false) => None,
            };
            if let Some(fill) = fill {
                canvas.rect_filled(item, 7, fill);
            }
            let color = match (shade, *selected) {
                // Hover lights a tag on another monitor to full brightness.
                (Shade::Unfocused | Shade::Dimmed, true) if tag_hovered => palette::TEXT,
                (Shade::Unfocused | Shade::Dimmed, false) if tag_hovered => palette::MUTED,
                (Shade::Dimmed, true) => palette::MUTED,
                (_, true) => palette::TEXT,
                // On the focused monitor's accent background.
                (Shade::Focused, false) => palette::BACKGROUND,
                (Shade::Unfocused, false) => palette::FAINT,
                (Shade::Dimmed, false) => palette::INACTIVE,
            };
            draw_centered_text(
                &canvas,
                tag,
                rect(item.left(), item.top(), item.right(), item.bottom() - 6.0),
                if *is_icon { &icon_font } else { &normal_font },
                color,
            );
            if *has_clients {
                canvas.rect_filled(
                    rect(
                        item.center().x - 6.0,
                        item.bottom() - 4.0,
                        item.center().x + 6.0,
                        item.bottom() - 1.0,
                    ),
                    egui::CornerRadius::same(2),
                    color,
                );
            }
        }
    }
    let mut left = groups.last().map_or(LAUNCHER.1, |group| group.3) + REGION_GAP;
    let status = status_pieces(&canvas, &snapshot.segments, &normal_font, &icon_font, shade);
    let status_width = pieces_width(&status) + REGION_PADDING * 2.0;
    let status_left = (size[0] as f32 - BAR_MARGIN - status_width).max(left);
    let info_width = (text_width(&canvas, &normal_font, &snapshot.workspace_info)
        + REGION_PADDING * 2.0)
        .max(180.0);
    let info = rect(left, top, left + info_width, bottom);
    let region_text = shade.content();
    draw_region(&canvas, info, false, false);
    draw_centered_text(
        &canvas,
        &snapshot.workspace_info,
        info,
        &normal_font,
        region_text,
    );
    left = info.right() + REGION_GAP;
    let mut available_right = status_left - REGION_GAP;
    // The first mailbox added is leftmost, so place them from the right.
    for (index, (mailbox, state)) in snapshot.mailboxes.iter().enumerate().rev() {
        let response = paint_mailbox(
            ui,
            mailbox,
            state,
            index,
            &normal_font,
            shade,
            rect(left, top, available_right, bottom),
        );
        let span = response
            .as_ref()
            .map(|response| (response.rect.left(), response.rect.right()));
        if open_mailbox == Some(index) || response.as_ref().is_some_and(pressed) {
            actions.push(BarAction::ToggleMailbox { index, span });
        }
        if let Some((left, _)) = span {
            available_right = left - REGION_GAP;
        }
    }
    // A key binding can name a mailbox that does not exist.
    if let Some(index) = open_mailbox.filter(|&index| index >= snapshot.mailboxes.len()) {
        actions.push(BarAction::ToggleMailbox { index, span: None });
    }
    // Pressing the bar anywhere else, with either button, closes a picker. The
    // launcher and mailboxes open or close their own instead.
    let bar_pressed =
        ui.input(|input| input.pointer.primary_pressed() || input.pointer.secondary_pressed());
    let toggles = actions.iter().any(|action| {
        matches!(
            action,
            BarAction::ToggleLauncher { .. } | BarAction::ToggleMailbox { .. }
        )
    });
    if bar_pressed && !toggles {
        actions.insert(0, BarAction::Dismiss);
    }
    let desired_focus = (text_width(&canvas, &normal_font, &snapshot.focused_window)
        + REGION_PADDING * 2.0)
        .max(240.0);
    let focus_width = desired_focus.min((available_right - left).max(0.0));
    let centered_left = size[0] as f32 / 2.0 - focus_width / 2.0;
    let focus_left = centered_left
        .max(left)
        .min((available_right - focus_width).max(left));
    if focus_width > 0.0 {
        let focus = rect(focus_left, top, focus_left + focus_width, bottom);
        draw_region(
            &canvas,
            focus,
            false,
            snapshot.selected_monitor && !snapshot.focused_window.is_empty(),
        );
        draw_centered_text(
            &canvas,
            &snapshot.focused_window,
            focus,
            &normal_font,
            region_text,
        );
    }
    if status_left < size[0] as f32 - BAR_MARGIN {
        let region = rect(status_left, top, size[0] as f32 - BAR_MARGIN, bottom);
        draw_region(&canvas, region, false, false);
        draw_pieces(
            &canvas.with_clip_rect(region),
            &status,
            region.left() + REGION_PADDING,
            region,
        );
    }
    actions
}

/// How brightly a bar draws, by whether its monitor is focused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shade {
    Focused,
    Unfocused,
    /// Unfocused, with `BarFocusStyle::dim_unfocused`.
    Dimmed,
}
impl Shade {
    fn of(snapshot: &BarSnapshot) -> Self {
        match (
            snapshot.selected_monitor,
            snapshot.focus_style.dim_unfocused,
        ) {
            (true, _) => Self::Focused,
            (false, false) => Self::Unfocused,
            (false, true) => Self::Dimmed,
        }
    }
    /// Headers, processes and other context.
    fn context(self) -> egui::Color32 {
        match self {
            Self::Focused => palette::MUTED,
            Self::Unfocused => palette::FAINT,
            Self::Dimmed => palette::INACTIVE,
        }
    }
    /// Values, titles and other content.
    fn content(self) -> egui::Color32 {
        match self {
            Self::Focused => palette::TEXT,
            Self::Unfocused => palette::MUTED,
            Self::Dimmed => palette::FAINT,
        }
    }
    /// A color that carries meaning, such as a warning or a mailbox icon.
    fn mark(self, color: egui::Color32) -> egui::Color32 {
        if self == Self::Dimmed {
            color.gamma_multiply(0.55)
        } else {
            color
        }
    }
}

/// What a status or focus region draws, left to right.
enum Piece {
    Text(std::sync::Arc<egui::Galley>),
    /// Space, in points.
    Gap(f32),
    /// A thin vertical line between items.
    Divider,
}
/// Space on each side of a divider, and between items without one.
const DIVIDER_GAP: f32 = 9.0;
const ITEM_GAP: f32 = 8.0;
/// Space between a segment's header and its value.
const HEADER_GAP: f32 = 5.0;

fn pieces_width(pieces: &[Piece]) -> f32 {
    pieces
        .iter()
        .map(|piece| match piece {
            Piece::Text(galley) => galley.size().x,
            Piece::Gap(gap) => *gap,
            Piece::Divider => DIVIDER_GAP * 2.0 + 1.0,
        })
        .sum()
}

fn draw_pieces(painter: &egui::Painter, pieces: &[Piece], mut x: f32, region: egui::Rect) {
    let middle = region.center().y;
    for piece in pieces {
        match piece {
            Piece::Text(galley) => {
                let size = galley.size();
                painter.galley(
                    egui::pos2(x, middle - size.y / 2.0),
                    galley.clone(),
                    palette::TEXT,
                );
                x += size.x;
            }
            Piece::Gap(gap) => x += gap,
            Piece::Divider => {
                x += DIVIDER_GAP;
                let inset = region.height() * 0.28;
                painter.line_segment(
                    [
                        egui::pos2(x + 0.5, region.top() + inset),
                        egui::pos2(x + 0.5, region.bottom() - inset),
                    ],
                    egui::Stroke::new(1.0, palette::BORDER),
                );
                x += 1.0 + DIVIDER_GAP;
            }
        }
    }
}

/// The status segments, the first configured rightmost: headers muted, values
/// colored by their tone, and a `|` separator drawn as a divider.
fn status_pieces(
    canvas: &egui::Painter,
    segments: &[RenderedSegment],
    normal: &egui::FontId,
    icon: &egui::FontId,
    shade: Shade,
) -> Vec<Piece> {
    let (header_color, value_color) = (shade.context(), shade.content());
    let text = |text: &str, is_icon: bool, color| {
        let font = if is_icon { icon } else { normal };
        Piece::Text(canvas.layout_no_wrap(text.to_owned(), font.clone(), color))
    };
    let mut pieces = Vec::new();
    for (index, segment) in segments.iter().rev().enumerate() {
        match &segment.separator {
            Some((separator, _)) if separator.trim() == "|" => {
                if index > 0 {
                    pieces.push(Piece::Divider);
                }
            }
            Some((separator, is_icon)) if !separator.trim().is_empty() => {
                pieces.push(text(separator, *is_icon, shade.mark(palette::FAINT)));
            }
            _ => {
                if index > 0 {
                    pieces.push(Piece::Gap(ITEM_GAP));
                }
            }
        }
        if let Some((header, is_icon)) = &segment.header {
            pieces.push(text(header.trim_end(), *is_icon, header_color));
            pieces.push(Piece::Gap(HEADER_GAP));
        }
        let color = match segment.tone {
            Tone::Normal => value_color,
            Tone::Warning => shade.mark(palette::WARNING),
            Tone::Critical => shade.mark(palette::CRITICAL),
            Tone::Off => shade.mark(palette::FAINT),
        };
        // The value starts right after its header and keeps the width of its padded
        // text, so `7%` and `100%` sit the same distance from the header and the
        // segments beside it don't move as it changes.
        let font = if segment.value_is_icon { icon } else { normal };
        let padded = canvas
            .layout_no_wrap(segment.value.clone(), font.clone(), color)
            .size()
            .x;
        let value = canvas.layout_no_wrap(segment.value.trim().to_owned(), font.clone(), color);
        let rest = padded - value.size().x;
        pieces.push(Piece::Text(value));
        if rest > 0.5 {
            pieces.push(Piece::Gap(rest));
        }
    }
    pieces
}

fn text_width(canvas: &egui::Painter, font: &egui::FontId, text: &str) -> f32 {
    canvas
        .layout_no_wrap(text.to_owned(), font.clone(), egui::Color32::WHITE)
        .size()
        .x
}

fn draw_region(
    canvas: &egui::Painter,
    rect: egui::Rect,
    focused_monitor_background: bool,
    focus_outline: bool,
) {
    use egui::{Color32, Stroke, StrokeKind};
    canvas.add(
        egui::epaint::Shadow {
            offset: [0, 2],
            blur: 5,
            spread: 0,
            color: Color32::from_black_alpha(45),
        }
        .as_shape(rect, 8),
    );
    canvas.rect_filled(
        rect,
        8,
        if focused_monitor_background {
            palette::ACCENT
        } else {
            palette::BACKGROUND
        },
    );
    if focus_outline {
        canvas.rect_stroke(
            rect.shrink(0.75),
            egui::CornerRadius::same(7),
            Stroke::new(1.5, palette::ACCENT),
            StrokeKind::Middle,
        );
    }
}

fn draw_centered_text(
    canvas: &egui::Painter,
    text: &str,
    rect: egui::Rect,
    font: &egui::FontId,
    color: egui::Color32,
) {
    // Clip long titles horizontally so they cannot cover adjacent regions.
    // Keep the bar's vertical clip: short workspace-label regions reserve room
    // for occupancy markers but must not crop the font's ascenders/descenders.
    let clip = egui::Rect::from_min_max(
        egui::pos2(rect.left(), canvas.clip_rect().top()),
        egui::pos2(rect.right(), canvas.clip_rect().bottom()),
    );
    canvas.with_clip_rect(clip).text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        font.clone(),
        color,
    );
}

pub fn create_bars(
    monitors: &[crate::Monitor],
    bar_height: i32,
) -> crate::platform_error::PlatformResult<Vec<BarWindow>> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Controls::ShowScrollBar;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let class_name: Vec<u16> = "keys2BarWindowClass\0".encode_utf16().collect();
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    if instance.is_null() {
        return Err(crate::platform_error::PlatformError::last_win32(
            "GetModuleHandleW(bar)",
        ));
    }
    let mut class: WNDCLASSEXW = unsafe { zeroed() };
    class.cbSize = size_of::<WNDCLASSEXW>() as u32;
    class.style = CS_OWNDC;
    class.lpfnWndProc = Some(bar_message_loop);
    class.hInstance = instance;
    class.lpszClassName = class_name.as_ptr();
    class.hCursor = unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) };
    if class.hCursor.is_null() {
        crate::error_log::last_win32("LoadCursorW(bar) failed");
    }
    class.hIcon = unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) };
    if class.hIcon.is_null() {
        crate::error_log::last_win32("LoadIconW(bar) failed");
    }
    class.hIconSm = class.hIcon;
    class.hbrBackground = std::ptr::null_mut();
    unsafe {
        if RegisterClassExW(&class) == 0 {
            let error = crate::platform_error::PlatformError::last_win32("RegisterClassExW(bar)");
            if !error.is_win32(ERROR_CLASS_ALREADY_EXISTS) {
                return Err(error);
            }
        }
    }
    let title: Vec<u16> = "SimpleWM Bar\0".encode_utf16().collect();
    let mut result: Vec<BarWindow> = Vec::new();
    for (index, monitor) in monitors
        .iter()
        .enumerate()
        .filter(|(_, monitor)| !monitor.is_hidden)
    {
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_POPUP | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
                monitor.x_offset,
                0,
                monitor.width,
                bar_height,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                (index + 1) as *mut _,
            )
        };
        if hwnd.is_null() {
            let error = crate::platform_error::PlatformError::last_win32("CreateWindowExW(bar)");
            for bar in &result {
                if unsafe { DestroyWindow(bar.hwnd as _) } == 0 {
                    crate::platform_error::PlatformError::last_win32(
                        "DestroyWindow(bar creation cleanup)",
                    )
                    .report();
                }
            }
            return Err(error);
        }
        if let Err(error) = attach(hwnd) {
            crate::error_log::report("egui bar initialization", error);
            unsafe {
                DestroyWindow(hwnd);
                for bar in &result {
                    DestroyWindow(bar.hwnd as _);
                }
            }
            return Err(crate::platform_error::PlatformError::api_code(
                "egui bar initialization",
                -1,
            ));
        }
        unsafe {
            if ShowScrollBar(hwnd, SB_BOTH, 0) == 0 {
                crate::error_log::last_win32("ShowScrollBar(bar) failed");
            }
            if SetTimer(hwnd, 0, 1500, None) == 0 {
                let error = crate::platform_error::PlatformError::last_win32("SetTimer(bar)");
                if DestroyWindow(hwnd) == 0 {
                    crate::platform_error::PlatformError::last_win32(
                        "DestroyWindow(bar timer cleanup)",
                    )
                    .report();
                }
                for bar in &result {
                    if DestroyWindow(bar.hwnd as _) == 0 {
                        crate::platform_error::PlatformError::last_win32(
                            "DestroyWindow(bar creation cleanup)",
                        )
                        .report();
                    }
                }
                return Err(error);
            }
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            if windows_sys::Win32::Graphics::Gdi::UpdateWindow(hwnd) == 0 {
                crate::error_log::last_win32("UpdateWindow(bar) failed");
            }
        }
        result.push(BarWindow {
            hwnd: hwnd as isize,
            monitor_index: index,
        });
    }
    Ok(result)
}

pub fn redraw(bars: &[BarWindow]) {
    for bar in bars {
        unsafe {
            if windows_sys::Win32::Graphics::Gdi::InvalidateRect(bar.hwnd as _, std::ptr::null(), 0)
                == 0
            {
                crate::error_log::last_win32("InvalidateRect(bar redraw) failed");
            }
        }
    }
}

pub fn status_text(segments: &[crate::config::BarSegmentConfiguration]) -> String {
    segments
        .iter()
        .map(|segment| {
            let value = segment_value(segment.value);
            format!(
                "{}{}{:>width$}",
                segment.separator_text.as_deref().unwrap_or(""),
                segment.header_text.as_deref().unwrap_or(""),
                value,
                width = segment.variable_text_fixed_width
            )
        })
        .collect()
}

/// `rendered_segments`, reused for up to a second. The Claude animation repaints
/// every bar many times a second; the values themselves change far more slowly.
/// Every bar shows the same segments, and the regular 1500 ms repaint always
/// refreshes them.
pub fn cached_segments(
    segments: &[crate::config::BarSegmentConfiguration],
) -> Vec<RenderedSegment> {
    use std::time::{Duration, Instant};
    static CACHE: std::sync::Mutex<Option<(Instant, Vec<RenderedSegment>)>> =
        std::sync::Mutex::new(None);
    let mut cache = CACHE.lock().unwrap();
    match &*cache {
        Some((at, rendered)) if at.elapsed() < Duration::from_secs(1) => rendered.clone(),
        _ => {
            let rendered = rendered_segments(segments);
            *cache = Some((Instant::now(), rendered.clone()));
            rendered
        }
    }
}

pub fn rendered_segments(
    segments: &[crate::config::BarSegmentConfiguration],
) -> Vec<RenderedSegment> {
    segments
        .iter()
        .map(|segment| {
            let value = segment_value(segment.value);
            RenderedSegment {
                separator: segment
                    .separator_text
                    .clone()
                    .map(|text| (text, segment.separator_is_icon)),
                header: segment
                    .header_text
                    .clone()
                    .map(|text| (text, segment.header_is_icon)),
                tone: tone(segment.value, &value),
                value: format!(
                    "{:>width$}",
                    value,
                    width = segment.variable_text_fixed_width
                ),
                value_is_icon: segment.variable_is_icon,
            }
        })
        .collect()
}

fn segment_value(value: crate::config::BarSegmentValue) -> String {
    use crate::config::BarSegmentValue::*;
    match value {
        SystemTime => system_time(false),
        LocalTime => system_time(true),
        LocalDate => local_date(),
        Memory => memory_percent(),
        Cpu => cpu_percent(),
        Volume => crate::status::volume_percent(),
        Internet => crate::status::internet_icon(),
    }
}

fn system_time(local: bool) -> String {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    let mut time: SYSTEMTIME = unsafe { zeroed() };
    unsafe {
        if local {
            windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut time);
        } else {
            windows_sys::Win32::System::SystemInformation::GetSystemTime(&mut time);
        }
    }
    format!("{:02}:{:02}", time.wHour, time.wMinute)
}
fn local_date() -> String {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    let mut time: SYSTEMTIME = unsafe { zeroed() };
    unsafe {
        windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut time);
    }
    format!("{:04}-{:02}-{:02}", time.wYear, time.wMonth, time.wDay)
}
fn memory_percent() -> String {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut value: MEMORYSTATUSEX = unsafe { zeroed() };
    value.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    unsafe {
        if GlobalMemoryStatusEx(&mut value) == 0 {
            crate::platform_error::PlatformError::last_win32("GlobalMemoryStatusEx").report();
            return "  ?%".into();
        }
    }
    format!("{:3}%", value.dwMemoryLoad)
}

fn cpu_percent() -> String {
    use std::mem::zeroed;
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::GetSystemTimes;
    static STATE: Mutex<(u64, u64, u64, i32, Option<std::time::Instant>)> =
        Mutex::new((0, 0, 0, 0, None));
    let mut state = STATE.lock().unwrap();
    if state
        .4
        .is_some_and(|last| last.elapsed() < std::time::Duration::from_secs(1))
    {
        return format!("{:3}%", state.3);
    }
    state.4 = Some(std::time::Instant::now());
    let mut idle: FILETIME = unsafe { zeroed() };
    let mut kernel: FILETIME = unsafe { zeroed() };
    let mut user: FILETIME = unsafe { zeroed() };
    if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
        crate::error_log::last_win32("GetSystemTimes failed");
        return "  0%".into();
    }
    let value = crate::process::filetime_ticks;
    let (idle, kernel, user) = (value(idle), value(kernel), value(user));
    let total = kernel.saturating_sub(state.1) + user.saturating_sub(state.2);
    let result = if state.1 != 0 && total != 0 {
        ((total.saturating_sub(idle.saturating_sub(state.0))) * 100 / total) as i32
    } else {
        state.3
    };
    if (0..=100).contains(&result) {
        state.3 = result;
    }
    state.0 = idle;
    state.1 = kernel;
    state.2 = user;
    format!("{:3}%", state.3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unfocused_bars_dim_a_step_further_when_set_to() {
        let mut snapshot = crate::bar_renderer::tests::snapshot();
        assert_eq!(Shade::of(&snapshot), Shade::Focused);
        snapshot.selected_monitor = false;
        assert_eq!(Shade::of(&snapshot), Shade::Dimmed);
        snapshot.focus_style.dim_unfocused = false;
        assert_eq!(Shade::of(&snapshot), Shade::Unfocused);
        assert_eq!(Shade::Dimmed.content(), Shade::Unfocused.context());
        assert_eq!(Shade::Unfocused.mark(palette::CRITICAL), palette::CRITICAL);
        assert_ne!(Shade::Dimmed.mark(palette::CRITICAL), palette::CRITICAL);
    }

    #[test]
    fn values_read_as_warning_critical_or_off() {
        use crate::config::BarSegmentValue::*;
        assert_eq!(tone(Cpu, " 74%"), Tone::Normal);
        assert_eq!(tone(Cpu, " 75%"), Tone::Warning);
        assert_eq!(tone(Memory, " 90%"), Tone::Critical);
        assert_eq!(tone(Memory, "  ?%"), Tone::Normal);
        assert_eq!(tone(Volume, "  0%"), Tone::Off);
        assert_eq!(tone(Volume, " 30%"), Tone::Normal);
        assert_eq!(
            tone(Internet, crate::status::INTERNET_OFFLINE),
            Tone::Critical
        );
        assert_eq!(
            tone(Internet, crate::status::INTERNET_UNKNOWN),
            Tone::Warning
        );
        assert_eq!(
            tone(Internet, crate::status::INTERNET_CONNECTED),
            Tone::Normal
        );
        assert_eq!(tone(LocalTime, "99"), Tone::Normal);
    }

    /// Runs `paint` with a painter whose fonts include the bar font.
    fn with_painter(mut paint: impl FnMut(&egui::Painter, &egui::FontId)) {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::bar_renderer::bar_fonts(""));
        let font = egui::FontId::new(12.0, egui::FontFamily::Name("bar".into()));
        ctx.run_ui(egui::RawInput::default(), |ui| paint(ui.painter(), &font))
            .drop_without_applying_deltas();
    }

    #[test]
    fn bar_separators_become_dividers_between_segments_only() {
        let segment = |separator: &str| RenderedSegment {
            separator: Some((separator.into(), false)),
            header: Some(("CPU:".into(), false)),
            value: " 5%".into(),
            value_is_icon: false,
            tone: Tone::Normal,
        };
        with_painter(|painter, font| {
            let kinds = |segments: &[RenderedSegment]| {
                status_pieces(painter, segments, font, font, Shade::Focused)
                    .iter()
                    .map(|piece| match piece {
                        Piece::Text(galley) => galley.text().to_owned(),
                        Piece::Gap(_) => "gap".into(),
                        Piece::Divider => "divider".into(),
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                kinds(&[segment(" | "), segment(" | ")]),
                [
                    "CPU:", "gap", "5%", "gap", "divider", "CPU:", "gap", "5%", "gap"
                ]
            );
            assert_eq!(
                kinds(&[segment(""), segment(" - ")]),
                [
                    " - ", "CPU:", "gap", "5%", "gap", "gap", "CPU:", "gap", "5%", "gap"
                ]
            );
        });
    }

    #[test]
    fn values_start_the_same_distance_from_their_header_and_keep_their_width() {
        let segment = |value: &str| RenderedSegment {
            separator: None,
            header: Some(("{f028} ".into(), true)),
            value: value.into(),
            value_is_icon: false,
            tone: Tone::Normal,
        };
        with_painter(|painter, font| {
            // The header's width plus the gap, and the whole segment's width.
            let measure = |value: &str| {
                let pieces = status_pieces(painter, &[segment(value)], font, font, Shade::Focused);
                let before_value = pieces_width(&pieces[..2]);
                (before_value, pieces_width(&pieces))
            };
            let (seven, hundred) = (measure("  7%"), measure("100%"));
            assert_eq!(seven.0, hundred.0);
            assert!((seven.1 - hundred.1).abs() < 0.5, "{seven:?} {hundred:?}");
        });
    }

    /// A bar run headlessly in egui, for its input.
    struct Headless {
        ctx: egui::Context,
        snapshot: BarSnapshot,
        time: f64,
    }
    impl Headless {
        const SIZE: [u32; 2] = [1600, 40];

        /// The test snapshot, with workspaces 10 and 20 first, after a first frame:
        /// egui finds what the pointer is over among the last frame's widgets.
        fn new() -> Self {
            let ctx = crate::bar_renderer::bar_context();
            ctx.set_fonts(crate::bar_renderer::bar_fonts(""));
            let mut snapshot = crate::bar_renderer::tests::snapshot();
            snapshot.tags[0].0 = 10;
            snapshot.tags[1].0 = 20;
            let mut bar = Self {
                ctx,
                snapshot,
                time: 0.0,
            };
            bar.frame(Vec::new());
            bar
        }

        /// Runs a frame with `events`: what it asked for and its cursor.
        fn frame(&mut self, events: Vec<egui::Event>) -> (Vec<BarAction>, egui::CursorIcon) {
            self.time += 0.1;
            let input = crate::bar_renderer::bar_input(Self::SIZE, self.time, events);
            let mut actions = Vec::new();
            let output = self.ctx.run_ui(input, |ui| {
                actions = paint_bar(ui, Self::SIZE, &self.snapshot, None)
            });
            let cursor = output.platform_output.cursor_icon;
            output.drop_without_applying_deltas();
            (actions, cursor)
        }

        /// Moves the pointer to (`x`, `y`) and left-presses there: what that asked
        /// for.
        fn press(&mut self, x: i32, y: i32) -> Vec<BarAction> {
            self.press_with(egui::PointerButton::Primary, x, y)
        }

        /// Moves the pointer to (`x`, `y`) and presses `button` there.
        fn press_with(&mut self, button: egui::PointerButton, x: i32, y: i32) -> Vec<BarAction> {
            let pos = pixel(x, y);
            self.frame(vec![egui::Event::PointerMoved(pos)]);
            let (actions, _) = self.frame(vec![egui::Event::PointerButton {
                pos,
                button,
                pressed: true,
                modifiers: Default::default(),
            }]);
            // Released, so the next press starts fresh.
            self.frame(vec![egui::Event::PointerButton {
                pos,
                button,
                pressed: false,
                modifiers: Default::default(),
            }]);
            actions
        }
    }

    #[test]
    fn workspaces_take_clicks_across_their_columns() {
        let mut bar = Headless::new();
        let workspace = |id| vec![BarAction::Dismiss, BarAction::Workspace(id)];
        let elsewhere = vec![BarAction::Dismiss];
        // The tags follow the launcher's button and a gap: 44 + 12 pixels, then
        // the region's padding.
        assert_eq!(bar.press(71, 20), elsewhere);
        assert_eq!(bar.press(72, 20), workspace(10));
        assert_eq!(bar.press(113, 20), workspace(10));
        assert_eq!(bar.press(114, 20), workspace(20));
        assert_eq!(bar.press(198, 20), elsewhere);
        // The bar's full height counts, up to the screen's edge.
        assert_eq!(bar.press(90, 0), workspace(10));
        assert_eq!(bar.press(90, 39), workspace(10));
    }

    #[test]
    fn the_launcher_button_takes_clicks() {
        let mut bar = Headless::new();
        let launcher = vec![BarAction::ToggleLauncher {
            left: LAUNCHER.0,
            right: LAUNCHER.1,
        }];
        // Its own button opens or closes the launcher, rather than dismissing it.
        assert_eq!(bar.press(1, 20), [BarAction::Dismiss]);
        assert_eq!(bar.press(2, 20), launcher);
        assert_eq!(bar.press(45, 0), launcher);
        assert_eq!(bar.press(46, 20), [BarAction::Dismiss]);
    }

    #[test]
    fn right_presses_only_dismiss() {
        let mut bar = Headless::new();
        let right = egui::PointerButton::Secondary;
        for x in [20, 90, 300] {
            assert_eq!(bar.press_with(right, x, 20), [BarAction::Dismiss]);
        }
    }

    #[test]
    fn what_takes_clicks_shows_a_hand_and_releases_do_nothing() {
        let mut bar = Headless::new();
        let moved = |x| vec![egui::Event::PointerMoved(egui::pos2(x, 20.0))];
        assert_eq!(bar.frame(moved(20.0)).1, egui::CursorIcon::PointingHand);
        assert_eq!(bar.frame(moved(90.0)).1, egui::CursorIcon::PointingHand);
        // The workspace info takes no clicks.
        assert_eq!(bar.frame(moved(300.0)).1, egui::CursorIcon::Default);
        let (actions, _) = bar.frame(vec![egui::Event::PointerButton {
            pos: egui::pos2(20.0, 20.0),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }]);
        assert_eq!(actions, []);
    }

    #[test]
    fn a_bars_state_lives_from_attach_until_detach() {
        use windows_sys::Win32::Graphics::OpenGL::wglGetCurrentContext;
        let window = crate::bar_renderer::tests::TestWindow::new();
        let count = || BARS.with(|bars| bars.borrow().len());
        let before = count();
        attach(window.0).unwrap();
        with_bar(window.0, |bar| {
            bar.events
                .push(egui::Event::PointerMoved(egui::pos2(20.0, 20.0)))
        })
        .unwrap();
        let actions = paint_window(
            window.0,
            [1600, 40],
            &crate::bar_renderer::tests::snapshot(),
        );
        assert_eq!(actions, []);
        assert_eq!(count(), before + 1);
        with_bar(window.0, |bar| {
            assert!(bar.renderer.is_some(), "painting put the renderer back");
            assert!(bar.events.is_empty(), "the frame took the input");
        })
        .unwrap();
        detach(window.0);
        assert_eq!(count(), before);
        assert!(with_bar(window.0, |_| ()).is_none());
        assert!(unsafe { wglGetCurrentContext() }.is_null());
    }
}
