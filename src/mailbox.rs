//! Bar mailboxes: a bar region whose content a background producer publishes, with a
//! drop-down listing its entries.
//!
//! The configuration declares a mailbox, its states and how each looks; SWM runs
//! its producer on a thread of its own. The producer publishes counts per state
//! and the entries to list, and receives what the person does in the drop-down.
//! SWM keeps nothing beyond the latest publication.
//!
//! ```ignore
//! let agents = configuration.bar_mailbox("Agents", crate::extensions::claude::producer);
//! agents.header("Agents:").state(
//!     "waiting",
//!     StateStyle::icon(Icon::glyph("●", Color32::RED).animation(Animation::Pulse {
//!         min: 0.4,
//!         period: 1.0,
//!     })),
//! );
//! ```
use crossbeam_channel::{Receiver, Sender};
use nfm_egui::egui;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// Draws a shape filling `rect` in `color`. The bar has already applied the state's
/// animation: `rect` is scaled and `color` faded. Runs on the bar thread on every
/// paint, so it must only draw.
pub type DrawShape = fn(&egui::Painter, egui::Rect, egui::Color32);

#[derive(Clone, Debug)]
pub enum Shape {
    /// Text in the bar font, such as a Nerd Font icon.
    Glyph(String),
    /// Drawn by a function, a square as tall as the bar font.
    Drawn(DrawShape),
}

/// How a state's icon moves while the state has a count. Periods are in seconds.
#[derive(Clone, Debug, PartialEq)]
pub enum Animation {
    /// Grows from `min` to `max` of its size and back.
    Scale { min: f32, max: f32, period: f32 },
    /// Fades from full opacity to `min` and back.
    Pulse { min: f32, period: f32 },
    /// Steps through glyphs in place of the icon's own.
    Frames { frames: Vec<String>, period: f32 },
}

/// An icon's look at one moment of its animation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Frame {
    pub scale: f32,
    pub opacity: f32,
    /// The glyph from `Animation::Frames` to show instead of the icon's.
    pub glyph: Option<usize>,
}
impl Frame {
    pub(crate) const STILL: Self = Self {
        scale: 1.0,
        opacity: 1.0,
        glyph: None,
    };
}
impl Animation {
    pub(crate) fn frame(&self, seconds: f64) -> Frame {
        // 0 at the start of each period, 1 halfway through, smoothly.
        let wave = |period: f32| {
            if period <= 0.0 {
                return 0.0;
            }
            let phase = (seconds / period as f64).fract() * std::f64::consts::TAU;
            (0.5 - 0.5 * phase.cos()) as f32
        };
        let still = Frame::STILL;
        match self {
            Self::Scale { min, max, period } => Frame {
                scale: min + (max - min) * wave(*period),
                ..still
            },
            Self::Pulse { min, period } => Frame {
                opacity: 1.0 - (1.0 - min) * wave(*period),
                ..still
            },
            Self::Frames { frames, period } if !frames.is_empty() && *period > 0.0 => Frame {
                glyph: Some(
                    ((seconds / *period as f64).fract() * frames.len() as f64) as usize
                        % frames.len(),
                ),
                ..still
            },
            Self::Frames { .. } => still,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Icon {
    pub shape: Shape,
    pub color: egui::Color32,
    pub animation: Option<Animation>,
}
impl Icon {
    pub fn glyph(text: impl Into<String>, color: egui::Color32) -> Self {
        Self {
            shape: Shape::Glyph(text.into()),
            color,
            animation: None,
        }
    }
    pub fn drawn(draw: DrawShape, color: egui::Color32) -> Self {
        Self {
            shape: Shape::Drawn(draw),
            color,
            animation: None,
        }
    }
    pub fn animation(mut self, animation: Animation) -> Self {
        self.animation = Some(animation);
        self
    }
}

/// How the bar shows one state. A state without an icon shows only its count.
#[derive(Clone, Debug)]
pub struct StateStyle {
    pub icon: Option<Icon>,
    pub show_count: bool,
}
impl StateStyle {
    pub fn icon(icon: Icon) -> Self {
        Self {
            icon: Some(icon),
            show_count: true,
        }
    }
}

/// What a producer publishes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MailboxState {
    /// Count per state name. States the mailbox does not declare are not shown.
    pub counts: Vec<(String, usize)>,
    /// Listed in the drop-down in this order.
    pub entries: Vec<MailboxEntry>,
}
impl MailboxState {
    pub fn count(&self, state: &str) -> usize {
        self.counts
            .iter()
            .filter(|(name, _)| name == state)
            .map(|(_, count)| count)
            .sum()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MailboxEntry {
    /// Identifies the entry in events.
    pub id: String,
    /// The state whose icon color marks the entry.
    pub state: String,
    /// A short line above the title, such as a project.
    pub source: String,
    pub title: String,
    pub detail: String,
    pub time: SystemTime,
}

/// What the person did in the drop-down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MailboxEvent {
    Opened,
    Closed,
    /// An entry was opened, by clicking it or pressing a key on it; the drop-down
    /// closes.
    Activated {
        id: String,
        how: Activation,
    },
}

/// How an entry was opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activation {
    Click,
    Enter,
}

/// Runs a mailbox: see `Configuration::bar_mailbox`.
#[derive(Clone)]
pub struct Producer(Arc<dyn Fn(MailboxSender) + Send + Sync>);
impl std::fmt::Debug for Producer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Producer")
    }
}

pub(crate) struct Shared {
    state: Mutex<Arc<MailboxState>>,
    events: (Sender<MailboxEvent>, Receiver<MailboxEvent>),
}
impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Shared")
    }
}

/// A mailbox in the bar; see the module documentation.
#[derive(Clone, Debug)]
pub struct BarMailbox {
    /// The drop-down's title.
    pub name: String,
    /// Text before the states, such as `Agents:`.
    pub header: Option<String>,
    /// Shown in this order; a state with no count is left out.
    pub states: Vec<(String, StateStyle)>,
    pub producer: Producer,
    shared: Arc<Shared>,
}
impl BarMailbox {
    pub(crate) fn new(
        name: &str,
        producer: impl Fn(MailboxSender) + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            header: None,
            states: Vec::new(),
            producer: Producer(Arc::new(producer)),
            shared: Arc::new(Shared {
                state: Mutex::new(Arc::default()),
                events: crossbeam_channel::bounded(64),
            }),
        }
    }
    pub fn header(&mut self, text: impl Into<String>) -> &mut Self {
        self.header = Some(text.into());
        self
    }
    /// Declare a state; declaring one again replaces its style and keeps its place.
    pub fn state(&mut self, name: impl Into<String>, style: StateStyle) -> &mut Self {
        let name = name.into();
        match self
            .states
            .iter_mut()
            .find(|(existing, _)| *existing == name)
        {
            Some((_, existing)) => *existing = style,
            None => self.states.push((name, style)),
        }
        self
    }
    pub(crate) fn current(&self) -> Arc<MailboxState> {
        self.shared.state.lock().unwrap().clone()
    }
    pub(crate) fn send(&self, event: MailboxEvent) {
        // A producer that stopped reading misses events rather than blocking the UI.
        let _ = self.shared.events.0.try_send(event);
    }
    pub(crate) fn style(&self, state: &str) -> Option<&StateStyle> {
        self.states
            .iter()
            .find(|(name, _)| name == state)
            .map(|(_, style)| style)
    }
    /// The declared states that have a count, in order.
    pub(crate) fn visible<'a>(
        &'a self,
        state: &MailboxState,
    ) -> Vec<(&'a str, &'a StateStyle, usize)> {
        self.states
            .iter()
            .map(|(name, style)| (name.as_str(), style, state.count(name)))
            .filter(|(_, _, count)| *count > 0)
            .collect()
    }
    /// Whether a visible state is animated, so the bar repaints quickly.
    pub(crate) fn animated(&self, state: &MailboxState) -> bool {
        self.visible(state)
            .iter()
            .any(|(_, style, _)| style.icon.as_ref().is_some_and(|i| i.animation.is_some()))
    }
    fn sender(&self) -> MailboxSender {
        MailboxSender {
            shared: self.shared.clone(),
        }
    }
}

/// The producer's side of a mailbox.
#[derive(Clone)]
pub struct MailboxSender {
    shared: Arc<Shared>,
}
impl MailboxSender {
    /// Replace what the mailbox shows. The bar repaints only when it changed.
    pub fn publish(&self, state: MailboxState) {
        {
            let mut current = self.shared.state.lock().unwrap();
            if **current == state {
                return;
            }
            *current = Arc::new(state);
        }
        crate::ui::mailbox_changed();
    }
    /// The next event, waiting up to `timeout` for one.
    pub fn wait_event(&self, timeout: Duration) -> Option<MailboxEvent> {
        self.shared.events.1.recv_timeout(timeout).ok()
    }
    /// Open `request` as a picker on the picker monitor. Returns whether SWM took
    /// the request.
    pub fn show_picker(&self, request: impl crate::picker::PickerRequest + 'static) -> bool {
        crate::ui::post(crate::ui::Command::ShowRequest(Box::new(request)))
    }
}

/// Start each mailbox's producer on its own thread.
pub(crate) fn start_producers(mailboxes: &[BarMailbox]) {
    for mailbox in mailboxes {
        let sender = mailbox.sender();
        let producer = mailbox.producer.clone();
        let name = mailbox.name.clone();
        let spawned = std::thread::Builder::new()
            .name(format!("swm-mailbox-{name}"))
            .spawn(move || {
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (producer.0)(sender)));
                let ended = match result {
                    Ok(()) => "producer returned".to_owned(),
                    Err(panic) => panic
                        .downcast_ref::<&str>()
                        .map(|s| s.to_string())
                        .or_else(|| panic.downcast_ref::<String>().cloned())
                        .unwrap_or_else(|| "producer panicked".into()),
                };
                crate::error_log::report(format!("mailbox {name} stopped"), ended);
            });
        if let Err(error) = spawned {
            crate::error_log::report(
                format!("failed to start mailbox {}", mailbox.name),
                error.to_string(),
            );
        }
    }
}

/// `just now`, `5 minutes ago`, `1 hour ago`, `2 days ago`.
pub(crate) fn relative_time(time: SystemTime, now: SystemTime) -> String {
    let seconds = now.duration_since(time).unwrap_or_default().as_secs();
    let (count, unit) = match seconds {
        0..60 => return "just now".into(),
        60..3600 => (seconds / 60, "minute"),
        3600..86400 => (seconds / 3600, "hour"),
        _ => (seconds / 86400, "day"),
    };
    format!("{count} {unit}{} ago", if count == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_grows_and_shrinks_within_its_bounds() {
        let scale = Animation::Scale {
            min: 0.45,
            max: 1.0,
            period: 1.2,
        };
        assert!((scale.frame(0.0).scale - 0.45).abs() < 1e-6);
        assert!((scale.frame(0.6).scale - 1.0).abs() < 1e-6);
        assert!((scale.frame(3.9).scale - scale.frame(0.3).scale).abs() < 1e-5);
        for step in 0..100 {
            let frame = scale.frame(step as f64 * 0.037);
            assert!((0.45..=1.0).contains(&frame.scale), "{frame:?}");
            assert_eq!(frame.opacity, 1.0);
        }
    }

    #[test]
    fn pulse_fades_and_frames_cycle() {
        let pulse = Animation::Pulse {
            min: 0.4,
            period: 1.0,
        };
        assert_eq!(pulse.frame(0.0).opacity, 1.0);
        assert!((pulse.frame(0.5).opacity - 0.4).abs() < 1e-6);
        let frames = Animation::Frames {
            frames: vec!["a".into(), "b".into(), "c".into()],
            period: 0.3,
        };
        let shown: Vec<_> = [0.05, 0.15, 0.25, 0.35]
            .iter()
            .map(|&t| frames.frame(t).glyph)
            .collect();
        assert_eq!(shown, [Some(0), Some(1), Some(2), Some(0)]);
        let stopped = Animation::Scale {
            min: 0.5,
            max: 1.0,
            period: 0.0,
        };
        assert_eq!(stopped.frame(7.0).scale, 0.5);
    }

    #[test]
    fn only_declared_states_with_counts_are_visible() {
        fn producer(_: MailboxSender) {}
        let mut mailbox = BarMailbox::new("Agents", producer);
        mailbox
            .state(
                "running",
                StateStyle::icon(Icon::glyph("*", egui::Color32::RED)),
            )
            .state(
                "waiting",
                StateStyle::icon(Icon::glyph("●", egui::Color32::RED).animation(
                    Animation::Pulse {
                        min: 0.4,
                        period: 1.0,
                    },
                )),
            );
        let state = MailboxState {
            counts: vec![
                ("running".into(), 2),
                ("waiting".into(), 0),
                ("other".into(), 4),
            ],
            entries: Vec::new(),
        };
        let visible: Vec<_> = mailbox
            .visible(&state)
            .iter()
            .map(|(name, _, count)| (*name, *count))
            .collect();
        assert_eq!(visible, [("running", 2)]);
        assert!(!mailbox.animated(&state));
        let waiting = MailboxState {
            counts: vec![("waiting".into(), 1)],
            entries: Vec::new(),
        };
        assert!(mailbox.animated(&waiting));
    }

    #[test]
    fn publishing_and_events_reach_the_other_side() {
        fn producer(_: MailboxSender) {}
        let mailbox = BarMailbox::new("Agents", producer);
        let sender = mailbox.sender();
        let state = MailboxState {
            counts: vec![("done".into(), 1)],
            entries: Vec::new(),
        };
        sender.publish(state.clone());
        assert_eq!(*mailbox.clone().current(), state);
        let opened = MailboxEvent::Activated {
            id: "a".into(),
            how: Activation::Enter,
        };
        mailbox.send(opened.clone());
        assert_eq!(sender.wait_event(Duration::from_millis(10)), Some(opened));
        assert_eq!(sender.wait_event(Duration::from_millis(1)), None);
    }

    #[test]
    fn relative_times_read_naturally() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let ago = |s| relative_time(now - Duration::from_secs(s), now);
        assert_eq!(ago(5), "just now");
        assert_eq!(ago(60), "1 minute ago");
        assert_eq!(ago(150), "2 minutes ago");
        assert_eq!(ago(7200), "2 hours ago");
        assert_eq!(ago(86400), "1 day ago");
        assert_eq!(relative_time(now + Duration::from_secs(5), now), "just now");
    }
}
