//! What the screenshot's keys do, independent of drawing.
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    VK_BACK, VK_DOWN, VK_ESCAPE, VK_LEFT, VK_NUMPAD1, VK_NUMPAD9, VK_RETURN, VK_RIGHT, VK_UP,
};

/// How far an edge moves per key, in pixels, without and with Ctrl.
pub(crate) const STEP: i32 = 40;
pub(crate) const FINE_STEP: i32 = 4;
/// Windows get the digits `1` to `9`.
pub(crate) const MAX_WINDOWS: usize = 9;

/// A rectangle in monitor-relative pixels; `right` and `bottom` are exclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Region {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
impl Region {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
    pub fn center(&self) -> (i32, i32) {
        ((self.left + self.right) / 2, (self.top + self.bottom) / 2)
    }
    /// This region grown by `by` pixels on every side (shrunk when negative).
    pub fn outset(self, by: i32) -> Self {
        Self {
            left: self.left - by,
            top: self.top - by,
            right: self.right + by,
            bottom: self.bottom + by,
        }
    }
    /// The part of this region inside a `width` × `height` monitor.
    fn clip(self, width: i32, height: i32) -> Self {
        Self {
            left: self.left.max(0),
            top: self.top.max(0),
            right: self.right.min(width),
            bottom: self.bottom.min(height),
        }
    }
    fn is_empty(&self) -> bool {
        self.width() <= 0 || self.height() <= 0
    }
}

/// Which pair of edges a key moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Edges {
    TopLeft,
    BottomRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Input {
    /// A window's digit, from 0 for `1`.
    Pick(usize),
    /// Move `edges` by `dx` horizontally and `dy` vertically.
    Resize {
        edges: Edges,
        dx: i32,
        dy: i32,
    },
    Back,
    Accept,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Continue,
    Cancel,
    Capture(Region),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    /// Choosing a window by its digit.
    Pick,
    /// Trimming the rectangle.
    Adjust(Region),
}

#[derive(Clone, Debug)]
pub(crate) struct Selector {
    /// The monitor's size; the rectangle stays inside it.
    pub width: i32,
    pub height: i32,
    /// What the digits choose, `1` first: left to right, then top to bottom.
    pub windows: Vec<Region>,
    pub state: State,
}

impl Selector {
    /// Digits for `windows` (monitor-relative); those off the monitor are
    /// skipped and any past the ninth dropped. With one left, the rectangle
    /// starts as that window; with none, as the whole monitor.
    pub fn new(width: i32, height: i32, windows: &[Region]) -> Self {
        let mut windows: Vec<Region> = windows
            .iter()
            .map(|r| r.clip(width, height))
            .filter(|r| !r.is_empty())
            .collect();
        windows.sort_by_key(|r| (r.left, r.top));
        windows.dedup();
        windows.truncate(MAX_WINDOWS);
        let mut selector = Self {
            width,
            height,
            windows,
            state: State::Pick,
        };
        match selector.windows.as_slice() {
            [] => selector.state = State::Adjust(selector.monitor()),
            [only] => selector.state = State::Adjust(*only),
            _ => {}
        }
        selector
    }

    fn monitor(&self) -> Region {
        Region {
            left: 0,
            top: 0,
            right: self.width,
            bottom: self.height,
        }
    }

    /// The rectangle being trimmed, once there is one.
    #[cfg(test)]
    fn selection(&self) -> Option<Region> {
        match self.state {
            State::Pick => None,
            State::Adjust(region) => Some(region),
        }
    }

    pub fn handle(&mut self, input: Input) -> Outcome {
        match (input, self.state) {
            (Input::Cancel, _) => return Outcome::Cancel,
            // Back to the windows, when there was a choice; otherwise out.
            (Input::Back, State::Adjust(_)) if self.windows.len() > 1 => self.state = State::Pick,
            (Input::Back, _) => return Outcome::Cancel,
            (Input::Pick(index), State::Pick) => {
                if let Some(&window) = self.windows.get(index) {
                    self.state = State::Adjust(window);
                }
            }
            (Input::Pick(_), State::Adjust(_)) => {}
            (Input::Accept, State::Adjust(region)) => return Outcome::Capture(region),
            (Input::Accept, State::Pick) => {}
            // Trimming before choosing a window starts from the whole monitor.
            (Input::Resize { edges, dx, dy }, state) => {
                let region = match state {
                    State::Pick => self.monitor(),
                    State::Adjust(region) => region,
                };
                self.state = State::Adjust(self.resize(region, edges, dx, dy));
            }
        }
        Outcome::Continue
    }

    /// `r` with `edges` moved, kept inside the monitor and at least a pixel
    /// across.
    fn resize(&self, mut r: Region, edges: Edges, dx: i32, dy: i32) -> Region {
        match edges {
            Edges::TopLeft => {
                r.left = (r.left + dx).clamp(0, r.right - 1);
                r.top = (r.top + dy).clamp(0, r.bottom - 1);
            }
            Edges::BottomRight => {
                r.right = (r.right + dx).clamp(r.left + 1, self.width);
                r.bottom = (r.bottom + dy).clamp(r.top + 1, self.height);
            }
        }
        r
    }
}

/// The input for virtual key `key`. Arrows and hjkl move the top and left
/// edges, or with Shift the bottom and right ones; Ctrl makes the step fine.
pub(crate) fn translate(key: u16, ctrl: bool, shift: bool) -> Option<Input> {
    let resize = |dx: i32, dy: i32| {
        let step = if ctrl { FINE_STEP } else { STEP };
        let edges = if shift {
            Edges::BottomRight
        } else {
            Edges::TopLeft
        };
        Some(Input::Resize {
            edges,
            dx: dx * step,
            dy: dy * step,
        })
    };
    const H: u16 = b'H' as u16;
    const J: u16 = b'J' as u16;
    const K: u16 = b'K' as u16;
    const L: u16 = b'L' as u16;
    const ONE: u16 = b'1' as u16;
    const NINE: u16 = b'9' as u16;
    match key {
        VK_ESCAPE => Some(Input::Cancel),
        VK_BACK => Some(Input::Back),
        VK_RETURN => Some(Input::Accept),
        VK_LEFT | H => resize(-1, 0),
        VK_RIGHT | L => resize(1, 0),
        VK_UP | K => resize(0, -1),
        VK_DOWN | J => resize(0, 1),
        ONE..=NINE => Some(Input::Pick(usize::from(key - ONE))),
        VK_NUMPAD1..=VK_NUMPAD9 => Some(Input::Pick(usize::from(key - VK_NUMPAD1))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(left: i32, top: i32, right: i32, bottom: i32) -> Region {
        Region {
            left,
            top,
            right,
            bottom,
        }
    }

    fn resize(edges: Edges, dx: i32, dy: i32) -> Input {
        Input::Resize { edges, dx, dy }
    }

    #[test]
    fn a_digit_selects_its_whole_window() {
        let left = region(0, 40, 1700, 1900);
        let right = region(1700, 40, 3400, 1900);
        let mut s = Selector::new(3400, 1900, &[right, left]);
        assert_eq!(s.state, State::Pick);
        // Past the last window, nothing; and Enter captures nothing yet.
        s.handle(Input::Pick(2));
        assert_eq!(s.handle(Input::Accept), Outcome::Continue);
        // Digits go left to right, whatever order the windows came in.
        s.handle(Input::Pick(1));
        assert_eq!(s.selection(), Some(right));
        assert_eq!(s.handle(Input::Accept), Outcome::Capture(right));
    }

    #[test]
    fn back_returns_to_the_windows_then_cancels() {
        let windows = [region(0, 0, 100, 100), region(100, 0, 200, 100)];
        let mut s = Selector::new(200, 100, &windows);
        s.handle(Input::Pick(0));
        assert_eq!(s.handle(Input::Back), Outcome::Continue);
        assert_eq!(s.state, State::Pick);
        assert_eq!(s.handle(Input::Back), Outcome::Cancel);
    }

    #[test]
    fn windows_are_clipped_to_the_monitor_sorted_and_limited_to_nine() {
        let mut windows: Vec<Region> = (0..12)
            .rev()
            .map(|i| region(i * 100, 0, i * 100 + 100, 100))
            .collect();
        // Off the monitor, and partly off it.
        windows.push(region(-500, 0, -100, 100));
        windows.push(region(-50, 200, 50, 300));
        let s = Selector::new(2000, 1000, &windows);
        assert_eq!(s.windows.len(), MAX_WINDOWS);
        assert_eq!(s.windows[0], region(0, 0, 100, 100));
        assert_eq!(s.windows[1], region(0, 200, 50, 300));
        assert_eq!(s.windows[2], region(100, 0, 200, 100));
    }

    #[test]
    fn a_single_window_starts_selected_and_back_cancels() {
        let only = region(10, 20, 300, 400);
        let mut s = Selector::new(1000, 1000, &[only]);
        assert_eq!(s.selection(), Some(only));
        assert_eq!(s.handle(Input::Back), Outcome::Cancel);
    }

    #[test]
    fn without_windows_the_whole_monitor_is_selected() {
        let s = Selector::new(800, 600, &[]);
        assert_eq!(s.selection(), Some(region(0, 0, 800, 600)));
    }

    #[test]
    fn each_pair_of_edges_moves_on_its_own() {
        let mut s = Selector::new(1000, 1000, &[region(100, 100, 500, 500)]);
        s.handle(resize(Edges::TopLeft, 40, 0));
        s.handle(resize(Edges::TopLeft, 0, -4));
        assert_eq!(s.selection(), Some(region(140, 96, 500, 500)));
        s.handle(resize(Edges::BottomRight, -40, 0));
        s.handle(resize(Edges::BottomRight, 0, 4));
        assert_eq!(s.selection(), Some(region(140, 96, 460, 504)));
    }

    #[test]
    fn edges_stay_on_the_monitor_and_cannot_cross() {
        let mut s = Selector::new(1000, 1000, &[region(100, 100, 140, 140)]);
        s.handle(resize(Edges::TopLeft, 400, 400));
        assert_eq!(s.selection(), Some(region(139, 139, 140, 140)));
        s.handle(resize(Edges::TopLeft, -400, -400));
        assert_eq!(s.selection(), Some(region(0, 0, 140, 140)));
        s.handle(resize(Edges::BottomRight, -400, 4000));
        assert_eq!(s.selection(), Some(region(0, 0, 1, 1000)));
    }

    #[test]
    fn trimming_while_picking_starts_from_the_whole_monitor() {
        let windows = [region(0, 0, 100, 100), region(100, 0, 200, 100)];
        let mut s = Selector::new(200, 100, &windows);
        s.handle(resize(Edges::BottomRight, -40, 0));
        assert_eq!(s.selection(), Some(region(0, 0, 160, 100)));
    }

    #[test]
    fn keys_choose_edges_and_step() {
        let r = |edges, dx, dy| Some(resize(edges, dx, dy));
        use Edges::*;
        assert_eq!(translate(VK_LEFT, false, false), r(TopLeft, -STEP, 0));
        assert_eq!(translate(b'J'.into(), false, false), r(TopLeft, 0, STEP));
        assert_eq!(translate(VK_UP, true, false), r(TopLeft, 0, -FINE_STEP));
        assert_eq!(
            translate(b'L'.into(), true, false),
            r(TopLeft, FINE_STEP, 0)
        );
        assert_eq!(translate(VK_RIGHT, false, true), r(BottomRight, STEP, 0));
        assert_eq!(
            translate(b'K'.into(), false, true),
            r(BottomRight, 0, -STEP)
        );
        assert_eq!(translate(VK_DOWN, true, true), r(BottomRight, 0, FINE_STEP));
        assert_eq!(
            translate(b'H'.into(), true, true),
            r(BottomRight, -FINE_STEP, 0)
        );
        assert_eq!(translate(b'3'.into(), false, false), Some(Input::Pick(2)));
        assert_eq!(translate(VK_NUMPAD1, false, false), Some(Input::Pick(0)));
        assert_eq!(translate(b'X'.into(), false, false), None);
    }
}
