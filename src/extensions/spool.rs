//! A bar mailbox producer that lists the entry files other programs write to a
//! folder: a build wrapper, an agent's hooks, anything that can write a file.
//! SWM only reads the folder; it opens no pipe or socket and runs nothing an
//! entry names.
//!
//! ```ignore
//! use crate::extensions::spool;
//! // A mailbox of its own, with a style per state:
//! spool::register(configuration, "Builds");
//! // Or the producer alone, for a mailbox declared and styled by hand:
//! configuration
//!     .bar_mailbox("Agents", spool::producer(spool::SpoolOptions::named("Agents")))
//!     .header("Agents:");
//! ```
//!
//! The folder is `%LOCALAPPDATA%\swm\mailbox\<name>`. Each entry is one file,
//! `<id>.json`, where the id is letters, digits, `.`, `_` and `-`:
//!
//! ```json
//! { "state": "running", "source": "nterm", "title": "cargo build --release",
//!   "detail": "", "time": 1731200000,
//!   "pid": 1234, "started": 133754961234567890,
//!   "log": "C:\\...\\build.log", "kind": "codex" }
//! ```
//!
//! Only `state` is required. `time` is in seconds since 1970 and defaults to when
//! the file was written. Writers replace an entry by writing a temporary file
//! beside it and renaming it over the entry, and remove their own old entries:
//! SWM never writes to the folder.
//!
//! An entry with a `pid` holds its state only while that process runs (`started`,
//! the process's creation time as a FILETIME, guards against the pid being
//! reused). Once the process is gone the entry shows as `stopped`, so a build
//! that was killed does not stay `running`. Entries of finished work leave `pid`
//! out; they are listed for `SpoolOptions::keep_for`.
//!
//! An entry with a `delay` is not listed until that many seconds after its
//! `time`, so a writer can report every command it runs and have only the long
//! ones show: it removes the entry of one that ends sooner. If the entry's
//! process ends before the entry was ever listed, it is not listed at all.
//!
//! Opening an entry shows its `log` in a picker: plain text with a row per error and
//! warning, or what the viewer of the entry's `kind` shows, such as a Claude Code
//! transcript. A kind also gives its entries badges of their own on the bar; see
//! `SpoolOptions::kind`.
//!
//! Any other text an entry holds, such as `"branch": "main"`, is kept with it
//! but means nothing here: it is for the entry's kind, which can add commands
//! for its entries to the drop-down and a note to their cards (`Kind::command`,
//! `Kind::note`).
//!
//! SWM runs elevated and the folder belongs to the desktop user, so the files
//! are read as that user (see `process::as_desktop_user`): an entry cannot have
//! SWM read what the user could not. Entries are also limited in number and size.
use crate::document_picker::{DocumentItem, DocumentPicker};
use crate::mailbox::{
    Animation, BarMailbox, DrawShape, Icon, MailboxActions, MailboxContent, MailboxEntry,
    MailboxEvent, MailboxSender, MailboxState, StateStyle,
};
use crate::markdown_document::{DocumentWriter, line_range};
use nfm_egui::egui::Color32;
use nfm_egui::styled_document::Style;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod remote;
pub mod watch;

pub use remote::Remotes;

pub const RUNNING: &str = "running";
pub const WAITING: &str = "waiting";
pub const DONE: &str = "done";
pub const FAILED: &str = "failed";
/// The state of an entry whose process is gone.
pub const STOPPED: &str = "stopped";
/// The field of a listed entry that holds its kind's name.
const KIND: &str = "kind";
/// What an entry file says that is read here; anything else is the kind's.
const KNOWN: [&str; 10] = [
    "state", "source", "title", "detail", "time", "pid", "started", "log", "kind", "delay",
];
/// The fields of its kind's an entry can have; any more are ignored.
const MAX_FIELDS: usize = 32;
/// A card with its kind's note after the source: a dot and `Note` in the
/// accent color for a colored one, `Plain` dimmed for one without a color.
const NOTE_COLUMNS: [&str; 3] = ["Mark", "Note", "Plain"];
const NOTE_TEMPLATE: &str =
    "Source:muted|Mark:icon|Note:accent|Plain:muted|Time:right:muted;Title:title;Detail:muted";

/// Entry files read from the folder; any more are ignored.
const MAX_FILES: usize = 200;
/// An entry file larger than this is ignored.
const MAX_FILE_BYTES: u64 = 64 * 1024;
/// Entries listed in the drop-down.
const MAX_ENTRIES: usize = 50;
/// How much of an entry's text is kept, in characters.
const MAX_TEXT: usize = 400;
/// How much of the end of a log is shown.
const LOG_TAIL_BYTES: u64 = 512 * 1024;
/// Rows listed for a log's errors and warnings.
const MAX_LOG_ROWS: usize = 500;
/// The longest an entry's `delay` can keep it off the list.
const MAX_DELAY_SECONDS: u64 = 60 * 60;

/// One kind of entry, such as an agent's sessions: the badges its entries have
/// on the bar and how their logs are shown. An entry says which kind it is with
/// `"kind": "<name>"`; see `SpoolOptions::kind`.
#[derive(Clone)]
pub struct Kind {
    /// What entries of this kind call it: letters, digits, `.`, `_` and `-`.
    pub name: String,
    /// The shape of its badges, so they can be told from another kind's.
    pub shape: DrawShape,
    /// The shape's color while running. Waiting and done have the colors they
    /// have for every kind.
    pub color: Color32,
    viewer: Option<Arc<ViewLog>>,
    ended: Option<Arc<Ended>>,
    commands: Vec<(String, Arc<RunCommand>)>,
    note: Option<Arc<NoteOf>>,
}
/// From a log's path and a title to the picker that shows it.
type ViewLog = dyn Fn(&Path, String) -> Result<DocumentPicker, String> + Send + Sync;
/// A drop-down command, run with the selected entry.
type RunCommand = dyn Fn(&MailboxEntry, &mut MailboxActions<'_>) + Send + Sync;
/// From a listed entry to the note on its card, if it has one.
type NoteOf = dyn Fn(&MailboxEntry) -> Option<Note> + Send + Sync;

/// A short note on an entry's card, after its source; see `Kind::note`.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub text: String,
    /// A dot in this color before the text, which is then in the accent color.
    /// Without one the text is dimmed, like the source.
    pub color: Option<Color32>,
}
impl Note {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: None,
        }
    }
    pub fn colored(text: impl Into<String>, color: Color32) -> Self {
        Self {
            text: text.into(),
            color: Some(color),
        }
    }
    /// The note as `NOTE_COLUMNS`.
    fn columns(note: Option<Self>) -> Vec<String> {
        match note {
            Some(Self {
                text,
                color: Some(color),
            }) => vec![MailboxContent::dot(color), text, String::new()],
            Some(Self { text, color: None }) => vec![String::new(), String::new(), text],
            None => Vec::new(),
        }
    }
}
/// From a log's path and when its entry entered its state to how the work has
/// ended since, if the log shows that it has.
type Ended = dyn Fn(&Path, SystemTime) -> Option<String> + Send + Sync;
impl Kind {
    pub fn new(name: impl Into<String>, shape: DrawShape, color: Color32) -> Self {
        Self {
            name: name.into(),
            shape,
            color,
            viewer: None,
            ended: None,
            commands: Vec::new(),
            note: None,
        }
    }
    /// Lists `name` in the command palette (Ctrl+Shift+/) of a drop-down that
    /// lists entries of this kind. Choosing it on one of them runs `run` with
    /// that entry, whose `fields` hold what its file says beyond what is read
    /// here; on an entry of another kind it does nothing. Kinds that give a
    /// command the same name share its place in the list.
    pub fn command(
        mut self,
        name: impl Into<String>,
        run: impl Fn(&MailboxEntry, &mut MailboxActions<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.commands.push((name.into(), Arc::new(run)));
        self
    }
    /// Puts a note on the card of each entry of this kind, after its source:
    /// what `note` returns for the entry, asked each time the drop-down opens.
    pub fn note(
        mut self,
        note: impl Fn(&MailboxEntry) -> Option<Note> + Send + Sync + 'static,
    ) -> Self {
        self.note = Some(Arc::new(note));
        self
    }
    /// Has the logs of this kind's running and waiting entries say when the work
    /// ended without its writer reporting it, such as an agent's turn that was
    /// interrupted. `ended` is given the log and when the entry entered its
    /// state, and returns how the work ended since, such as `Interrupted`: the
    /// entry is then listed as done with that as its detail. It runs as the
    /// desktop user, each time the log has been written.
    pub fn ended(
        mut self,
        ended: impl Fn(&Path, SystemTime) -> Option<String> + Send + Sync + 'static,
    ) -> Self {
        self.ended = Some(Arc::new(ended));
        self
    }
    /// Shows the log of an entry of this kind with `viewer`, rather than as text.
    /// The viewer runs as the desktop user, as every read of the folder does.
    pub fn viewer(
        mut self,
        viewer: impl Fn(&Path, String) -> Result<DocumentPicker, String> + Send + Sync + 'static,
    ) -> Self {
        self.viewer = Some(Arc::new(viewer));
        self
    }
    /// The name of `state` for this kind's entries, such as `codex-running`.
    pub fn state(&self, state: &str) -> String {
        format!("{}-{state}", self.name)
    }
}
impl std::fmt::Debug for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Kind({})", self.name)
    }
}

#[derive(Clone, Debug)]
pub struct SpoolOptions {
    /// The folder's name under `%LOCALAPPDATA%\swm\mailbox`.
    pub name: String,
    /// The folder itself, instead of the one `name` gives.
    pub directory: Option<PathBuf>,
    /// An entry without a running process is listed for this long after its time.
    pub keep_for: Duration,
    /// The kinds of entry with badges and a viewer of their own; see
    /// `SpoolOptions::kind`.
    pub kinds: Vec<Kind>,
    /// The selected row's background in a log's preview.
    pub highlight: Color32,
    /// Logs open filling the picker monitor up to the bar, rather than centered.
    pub full_pane: bool,
    /// Other machines whose folders of this name are listed too, each entry
    /// under its machine's name; see `spool::remote`.
    pub remotes: Remotes,
}
impl SpoolOptions {
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            directory: None,
            keep_for: Duration::from_secs(2 * 24 * 60 * 60),
            kinds: Vec::new(),
            highlight: Color32::from_rgb(0x3c, 0x38, 0x36),
            full_pane: false,
            remotes: Remotes::default(),
        }
    }
    /// Gives the entries of a kind, such as `claude::kind(options)`, badges and a
    /// viewer of their own. Their states are `<kind>-running`, `-waiting` and
    /// `-done`, which `register_with` declares in the kind's shape; `stopped`
    /// and `failed` stay shared. An entry of no kind, or of one not given here,
    /// has the plain states and its log is shown as text.
    pub fn kind(mut self, kind: Kind) -> Self {
        self.kinds.retain(|known| known.name != kind.name);
        self.kinds.push(kind);
        self
    }
    /// Lists the folders of this name on `remotes` too: the same list given to
    /// several mailboxes is one connection to each machine.
    pub fn remotes(mut self, remotes: &Remotes) -> Self {
        self.remotes = remotes.clone();
        self
    }
    fn kind_of(&self, entry: &Entry) -> Option<&Kind> {
        let name = entry.kind.as_deref()?;
        self.kinds.iter().find(|kind| kind.name == name)
    }
    /// The entry's state as the mailbox counts it: its kind's own, for the
    /// states a kind has.
    fn state_of(&self, entry: &Entry) -> String {
        match self.kind_of(entry) {
            Some(kind) if [RUNNING, WAITING, DONE].contains(&entry.state.as_str()) => {
                kind.state(&entry.state)
            }
            _ => entry.state.clone(),
        }
    }
    fn directory(&self) -> Option<PathBuf> {
        self.directory.clone().or_else(|| {
            // A folder name, not a path out of the mailbox folder.
            is_id(&self.name).then_some(())?;
            let local = std::env::var_os("LOCALAPPDATA")?;
            Some(
                Path::new(&local)
                    .join("swm")
                    .join("mailbox")
                    .join(&self.name),
            )
        })
    }
}

/// Adds a mailbox named `name` to the bar that lists the folder of that name,
/// with a header and a style for each state above.
pub fn register<'a>(configuration: &'a mut crate::Configuration, name: &str) -> &'a mut BarMailbox {
    register_with(configuration, SpoolOptions::named(name))
}

/// `register`, with the options given: the mailbox is named `options.name`, and
/// each of `options.kinds` has its running, waiting and done states in its own
/// shape, growing and shrinking while running and pulsing while waiting.
pub fn register_with(
    configuration: &mut crate::Configuration,
    options: SpoolOptions,
) -> &mut BarMailbox {
    const DONE_COLOR: Color32 = Color32::from_rgb(0xb8, 0xbb, 0x26);
    let name = options.name.clone();
    let kinds = options.kinds.clone();
    let content = picker(&options);
    let glyph = |text: &str, color| StateStyle::icon(Icon::glyph(text, color));
    let pulse = Animation::Pulse {
        min: 0.4,
        period: 1.0,
    };
    let scale = Animation::Scale {
        min: 0.5,
        max: 1.0,
        period: 1.2,
    };
    // Drawn rather than a glyph, so it does not depend on the bar font.
    let running = Icon::drawn(ring, crate::palette::ACCENT).animation(scale.clone());
    let waiting = Icon::glyph("●", crate::palette::CRITICAL).animation(pulse.clone());
    let mailbox = configuration
        .bar_mailbox(&name, producer(options))
        .header(format!("{name}:"))
        .state(RUNNING, StateStyle::icon(running))
        .state(WAITING, StateStyle::icon(waiting))
        .state(FAILED, glyph("✗", crate::palette::CRITICAL))
        .state(STOPPED, glyph("■", crate::palette::WARNING))
        .state(DONE, glyph("✓", DONE_COLOR))
        .picker(content);
    for kind in kinds {
        let icon = |color| Icon::drawn(kind.shape, color);
        let running = icon(kind.color).animation(scale.clone());
        let waiting = icon(crate::palette::CRITICAL).animation(pulse.clone());
        mailbox
            .state(kind.state(RUNNING), StateStyle::icon(running))
            .state(kind.state(WAITING), StateStyle::icon(waiting))
            .state(kind.state(DONE), StateStyle::icon(icon(DONE_COLOR)));
    }
    mailbox
}

/// A ring filling `area`, the running state's icon. Pass it to `Icon::drawn` to
/// use it in a style of your own.
pub fn ring(painter: &nfm_egui::egui::Painter, area: nfm_egui::egui::Rect, color: Color32) {
    let radius = area.width().min(area.height()) / 2.0;
    let width = (radius * 0.45).max(1.2);
    painter.circle_stroke(
        area.center(),
        radius - width / 2.0,
        nfm_egui::egui::Stroke::new(width, color),
    );
}

/// The producer for the folder `options` names: pass it to
/// `Configuration::bar_mailbox`, for a mailbox declared by hand.
pub fn producer(options: SpoolOptions) -> impl Fn(MailboxSender) + Send + Sync + 'static {
    move |mailbox| run(&mailbox, &options)
}

/// What the drop-down of the folder `options` names has beyond the cards: pass
/// it to `BarMailbox::picker`, for a mailbox declared by hand. It is what the
/// kinds of the entries listed give: their commands, and their notes on their
/// entries' cards.
pub fn picker(
    options: &SpoolOptions,
) -> impl Fn(&MailboxState) -> MailboxContent + Send + Sync + 'static {
    let kinds = options.kinds.clone();
    move |state| content(&kinds, state)
}

fn content(kinds: &[Kind], state: &MailboxState) -> MailboxContent {
    // Only the kinds with an entry listed have a say.
    let listed = |kind: &&Kind| {
        let mut entries = state.entries.iter();
        entries.any(|entry| entry.field(KIND) == Some(kind.name.as_str()))
    };
    let kinds: Vec<&Kind> = kinds.iter().filter(listed).collect();
    let mut content = MailboxContent::cards();

    let notes: HashMap<String, Arc<NoteOf>> = kinds
        .iter()
        .filter_map(|kind| Some((kind.name.clone(), kind.note.clone()?)))
        .collect();
    if !notes.is_empty() {
        content = content.columns(&NOTE_COLUMNS, NOTE_TEMPLATE, move |entry| {
            let note = entry.field(KIND).and_then(|kind| notes.get(kind));
            Note::columns(note.and_then(|note| note(entry)))
        });
    }

    let mut names: Vec<&String> = Vec::new();
    for (name, _) in kinds.iter().flat_map(|kind| &kind.commands) {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    for name in names {
        // Each kind's own command of this name, for its entries.
        let runs: HashMap<String, Arc<RunCommand>> = kinds
            .iter()
            .filter_map(|kind| {
                let (_, run) = kind.commands.iter().find(|(known, _)| known == name)?;
                Some((kind.name.clone(), run.clone()))
            })
            .collect();
        content = content.command(name.clone(), move |entry, drop_down| {
            if let Some(run) = entry.field(KIND).and_then(|kind| runs.get(kind)) {
                run(entry, drop_down);
            }
        });
    }
    content
}

fn run(mailbox: &MailboxSender, options: &SpoolOptions) {
    let Some(directory) = options.directory() else {
        crate::error_log::report(
            format!("mailbox folder {}", options.name),
            "no folder: the name is not a folder name, or LOCALAPPDATA is not set",
        );
        return;
    };
    let mut files = Files::default();
    let mut logs = Logs::default();
    let mut tracker = Tracker::default();
    let mut reported = false;
    loop {
        let now = SystemTime::now();
        let read = || {
            let mut entries = files.read(&directory);
            entries.extend(options.remotes.entries(&options.name));
            logs.settle(&mut entries, options);
            entries
        };
        // A failure here is the desktop shell being away, such as while Explorer
        // restarts: keep what is shown and say so once.
        match crate::process::as_desktop_user(read) {
            Ok(mut entries) => {
                reported = false;
                for entry in &mut entries {
                    entry.state = options.state_of(entry);
                }
                let shown = tracker.update(entries, now, options.keep_for, |entry| {
                    entry.process_is_running()
                });
                mailbox.publish(shown);
            }
            Err(error) if !reported => {
                reported = true;
                crate::error_log::report(
                    format!("mailbox folder {}", options.name),
                    format!("{error:?}"),
                );
            }
            Err(_) => {}
        }
        if let Some(MailboxEvent::Activated { id, .. }) = mailbox.wait_event(Duration::from_secs(1))
            && let Some((entry, _)) = tracker.shown.get(&id)
            && let Err(error) = open(mailbox, entry, options)
        {
            crate::error_log::report(format!("mailbox entry {id}"), error);
        }
    }
}

/// The ID of the process `reference` names, if it still runs. An entry's field
/// names a process as `<pid>:<started>`, its ID and its creation time as a
/// FILETIME, so that a later process given the same ID is not taken for it; the
/// writers in `mailbox\SwmMailbox.ps1` write them so. For a kind that says
/// whether its entry's work is still open somewhere.
pub fn running_process(reference: &str) -> Option<u32> {
    let (pid, started) = process_reference(reference)?;
    (crate::process::running_since(pid) == Some(started)).then_some(pid)
}

/// The process ID and creation time in `<pid>:<started>`.
fn process_reference(reference: &str) -> Option<(u32, u64)> {
    let (pid, started) = reference.trim().split_once(':')?;
    Some((
        pid.parse().ok().filter(|pid| *pid != 0)?,
        started.parse().ok()?,
    ))
}

/// Shows the entry's log, as its kind is shown or as text. An entry without a
/// log has nothing to open.
fn open(mailbox: &MailboxSender, entry: &Entry, options: &SpoolOptions) -> Result<(), String> {
    let Some(log) = &entry.log else {
        return Ok(());
    };
    let title = if entry.source.is_empty() {
        entry.title.clone()
    } else {
        format!("{} · {}", entry.source, entry.title)
    };
    // Only a viewer the configuration registered: an entry names its kind, never
    // what to run.
    let viewer = options.kind_of(entry).and_then(|kind| kind.viewer.as_ref());
    let picker = crate::process::as_desktop_user(|| match viewer {
        Some(viewer) => viewer(log, title),
        None => log_picker(log, title, options),
    })
    .map_err(|error| format!("{error:?}"))??;
    if !mailbox.show_picker(picker) {
        return Err("SWM's UI is not running".into());
    }
    Ok(())
}

/// One entry file.
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    id: String,
    state: String,
    source: String,
    title: String,
    detail: String,
    time: SystemTime,
    pid: Option<u32>,
    started: Option<u64>,
    log: Option<PathBuf>,
    /// Which of `SpoolOptions::kinds` it is, such as `codex`.
    kind: Option<String>,
    /// Not listed until this long after `time`.
    delay: Option<Duration>,
    /// Whether its process runs, for an entry of another machine, which is
    /// the one that can say (`spool::remote`).
    elsewhere: Option<bool>,
    /// The other text in the file, by name: its kind's, and nothing here.
    fields: BTreeMap<String, String>,
}
impl Entry {
    /// `text` is the file's contents and `written` when it was last written.
    fn parse(id: &str, text: &str, written: SystemTime) -> Option<Self> {
        let value: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
        let text = |key: &str| {
            let text = value[key].as_str().unwrap_or_default().trim();
            match text.char_indices().nth(MAX_TEXT) {
                Some((cut, _)) => format!("{}…", &text[..cut]),
                None => text.to_owned(),
            }
        };
        // Numbers, or strings of digits: a FILETIME does not fit every writer's numbers.
        let number = |key: &str| {
            value[key]
                .as_u64()
                .or_else(|| value[key].as_str()?.trim().parse().ok())
        };
        let state = text("state");
        if state.is_empty() || state.chars().count() > 32 {
            return None;
        }
        Some(Self {
            id: id.to_owned(),
            state,
            source: text("source"),
            title: text("title"),
            detail: text("detail"),
            time: number("time")
                .and_then(|seconds| UNIX_EPOCH.checked_add(Duration::from_secs(seconds)))
                .unwrap_or(written),
            pid: number("pid").and_then(|pid| u32::try_from(pid).ok()),
            started: number("started"),
            log: value["log"]
                .as_str()
                .filter(|log| !log.is_empty())
                .map(PathBuf::from),
            kind: value["kind"]
                .as_str()
                .filter(|kind| is_id(kind) && kind.len() <= 32)
                .map(str::to_owned),
            delay: number("delay")
                .filter(|seconds| *seconds > 0)
                .map(|seconds| Duration::from_secs(seconds.min(MAX_DELAY_SECONDS))),
            elsewhere: None,
            fields: value
                .as_object()
                .into_iter()
                .flatten()
                .filter(|(name, _)| !KNOWN.contains(&name.as_str()))
                .filter_map(|(name, value)| {
                    let value = value.as_str().filter(|value| !value.trim().is_empty())?;
                    Some((name.clone(), value.to_owned()))
                })
                .take(MAX_FIELDS)
                .collect(),
        })
    }
    /// Whether the entry names a process that is still running: here, or for
    /// an entry of another machine, as that machine said.
    fn process_is_running(&self) -> bool {
        if let Some(running) = self.elsewhere {
            return self.pid.is_some() && running;
        }
        self.pid.is_some_and(|pid| {
            let since = crate::process::running_since(pid);
            since.is_some() && self.started.is_none_or(|started| since == Some(started))
        })
    }
}

/// Whether `id` can name an entry: a file name with no path in it.
fn is_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// The folder's entries. A file is read again only when it has been written.
/// `T` is what is kept of each file: the entry, or for `watch` its text too.
struct Files<T = Entry>(HashMap<String, (SystemTime, u64, Option<T>)>);
impl<T> Default for Files<T> {
    fn default() -> Self {
        Self(HashMap::new())
    }
}
impl Files<Entry> {
    fn read(&mut self, directory: &Path) -> Vec<Entry> {
        self.read_as(directory, Entry::parse)
    }
}
impl<T: Clone> Files<T> {
    /// `parse` is given a file's ID, its text and when it was written.
    fn read_as(
        &mut self,
        directory: &Path,
        parse: impl Fn(&str, &str, SystemTime) -> Option<T>,
    ) -> Vec<T> {
        let mut read = HashMap::new();
        let files = std::fs::read_dir(directory).into_iter().flatten().flatten();
        for file in files {
            if read.len() >= MAX_FILES {
                break;
            }
            let path = file.path();
            let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            if path.extension().is_none_or(|e| e != "json") || !is_id(id) {
                continue;
            }
            // Of the entry itself, not of a link's target.
            let Ok(metadata) = file.metadata() else {
                continue;
            };
            let Ok(written) = metadata.modified() else {
                continue;
            };
            if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
                continue;
            }
            let entry = match self.0.remove(id) {
                Some((at, length, entry)) if at == written && length == metadata.len() => entry,
                _ => read_limited(&path).and_then(|text| parse(id, &text, written)),
            };
            read.insert(id.to_owned(), (written, metadata.len(), entry));
        }
        self.0 = read;
        self.0
            .values()
            .filter_map(|(_, _, entry)| entry.clone())
            .collect()
    }
}

/// The file's text, if it is no larger than an entry may be.
fn read_limited(path: &Path) -> Option<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= MAX_FILE_BYTES).then(|| String::from_utf8_lossy(&bytes).into_owned())
}

/// What the logs of running and waiting entries said of how their work ended
/// (`Kind::ended`). A log is asked again only when it has been written, or its
/// entry has entered a state since.
#[derive(Default)]
struct Logs(HashMap<String, (LogRead, Option<String>)>);
/// When a log was written, its length, and when its entry entered its state.
type LogRead = (SystemTime, u64, SystemTime);
impl Logs {
    /// Makes each entry whose log says its work has ended a done one, kept as
    /// finished work is: it no longer depends on its process.
    fn settle(&mut self, entries: &mut [Entry], options: &SpoolOptions) {
        let mut asked = HashMap::new();
        for entry in entries {
            if ![RUNNING, WAITING].contains(&entry.state.as_str()) {
                continue;
            }
            let ended = options.kind_of(entry).and_then(|kind| kind.ended.as_ref());
            let (Some(ended), Some(log)) = (ended, &entry.log) else {
                continue;
            };
            let Ok(metadata) = std::fs::metadata(log) else {
                continue;
            };
            let Ok(written) = metadata.modified() else {
                continue;
            };
            let read = (written, metadata.len(), entry.time);
            let how = match self.0.remove(&entry.id) {
                Some((before, how)) if before == read => how,
                _ => ended(log, entry.time),
            };
            asked.insert(entry.id.clone(), (read, how.clone()));
            if let Some(how) = how {
                entry.state = DONE.to_owned();
                entry.detail = how;
                entry.pid = None;
            }
        }
        self.0 = asked;
    }
}

/// What the mailbox shows, and when each entry's process was found gone.
#[derive(Default)]
struct Tracker {
    /// Entries with a `pid` seen while their process ran.
    seen_running: HashMap<String, SystemTime>,
    /// When an entry's process was first found gone.
    stopped: HashMap<String, SystemTime>,
    /// The entries listed, and whether each one's process runs.
    shown: HashMap<String, (Entry, bool)>,
}
impl Tracker {
    fn update(
        &mut self,
        entries: Vec<Entry>,
        now: SystemTime,
        keep_for: Duration,
        is_running: impl Fn(&Entry) -> bool,
    ) -> MailboxState {
        let mut listed = Vec::new();
        let mut seen_running = HashMap::new();
        let mut stopped = HashMap::new();
        self.shown.clear();
        for entry in entries {
            let running = entry.pid.is_some() && is_running(&entry);
            if let Some(delay) = entry.delay {
                // Not due yet, or its process ended before it ever was: work too
                // short to list, whose writer did not get to remove its entry.
                let due = now.duration_since(entry.time).is_ok_and(|age| age >= delay);
                let listed_before = self.seen_running.contains_key(&entry.id)
                    || self.stopped.contains_key(&entry.id);
                if !due || (entry.pid.is_some() && !running && !listed_before) {
                    continue;
                }
            }
            let (state, time) = if running {
                seen_running.insert(entry.id.clone(), entry.time);
                (entry.state.clone(), entry.time)
            } else if entry.pid.is_some() {
                // Stopped just now if it was running a moment ago; otherwise at
                // some time since the entry was written, which is all that is known.
                let at = self.stopped.get(&entry.id).copied().unwrap_or_else(|| {
                    if self.seen_running.get(&entry.id) == Some(&entry.time) {
                        now
                    } else {
                        entry.time
                    }
                });
                stopped.insert(entry.id.clone(), at);
                (STOPPED.to_owned(), at)
            } else {
                (entry.state.clone(), entry.time)
            };
            // A time ahead of this clock is recent too.
            let recent = now.duration_since(time).map_or(true, |age| age < keep_for);
            if !running && !recent {
                continue;
            }
            listed.push(MailboxEntry {
                id: entry.id.clone(),
                source: entry.source.clone(),
                title: if entry.title.is_empty() {
                    entry.id.clone()
                } else {
                    entry.title.clone()
                },
                // What the entry last said it was doing no longer holds.
                detail: if state == STOPPED {
                    "Its process ended".into()
                } else {
                    entry.detail.clone()
                },
                state,
                time,
                // With its kind, which the drop-down's content goes by.
                fields: entry
                    .fields
                    .clone()
                    .into_iter()
                    .chain(entry.kind.clone().map(|kind| (KIND.to_owned(), kind)))
                    .collect(),
            });
            self.shown.insert(entry.id.clone(), (entry, running));
        }
        self.seen_running = seen_running;
        self.stopped = stopped;
        // Newest first, so the entries cut off are the oldest.
        listed.sort_by(|a, b| b.time.cmp(&a.time).then_with(|| a.id.cmp(&b.id)));
        let mut counts: Vec<(String, usize)> = Vec::new();
        for entry in &listed {
            match counts.iter_mut().find(|(state, _)| *state == entry.state) {
                Some((_, count)) => *count += 1,
                None => counts.push((entry.state.clone(), 1)),
            }
        }
        listed.truncate(MAX_ENTRIES);
        MailboxState {
            counts,
            entries: listed,
        }
    }
}

/// The end of the log at `path` as a picker: a row per error and warning, newest
/// first, after a row for the end of the output.
fn log_picker(
    path: &Path,
    title: String,
    options: &SpoolOptions,
) -> Result<DocumentPicker, String> {
    let text = tail(path, LOG_TAIL_BYTES)
        .map_err(|error| format!("reading {}: {error}", path.display()))?;
    let mut out = DocumentWriter::default();
    let mut items = Vec::new();
    let colored = |color| Style {
        foreground: Some(color),
        ..Default::default()
    };
    for line in text.lines() {
        let line = strip_escapes(line);
        let line = line.trim_end();
        let style = match severity(line) {
            Some(Severity::Error) => colored(crate::palette::CRITICAL),
            Some(Severity::Warning) => colored(crate::palette::WARNING),
            None => Style::default(),
        };
        if style != Style::default() {
            items.push(DocumentItem {
                label: line.trim_start().to_owned(),
                range: line_range(out.line(), line.len()),
            });
        }
        out.line_of(line, style);
    }
    let last = out.line().saturating_sub(1);
    items.push(DocumentItem {
        label: "End of output".into(),
        range: line_range(last, out.last_line_len()),
    });
    items.reverse();
    items.truncate(MAX_LOG_ROWS);
    Ok(DocumentPicker {
        title,
        column: "Output".into(),
        document: out.finish(),
        items,
        highlight: options.highlight,
        full_pane: options.full_pane,
    })
}

#[derive(Debug, PartialEq)]
enum Severity {
    Error,
    Warning,
}

/// Whether `line` reports an error or a warning, as compilers word them:
/// `error: ...`, `error[E0308]: ...`, `file.cs(3,4): warning CS0168: ...`.
fn severity(line: &str) -> Option<Severity> {
    let lower = line.to_ascii_lowercase();
    let reports = |word: &str| {
        lower.match_indices(word).any(|(at, _)| {
            let before = lower[..at].chars().next_back();
            let after = lower[at + word.len()..].chars().next();
            before.is_none_or(|c| !c.is_ascii_alphanumeric())
                && matches!(after, Some(':' | '[' | ' '))
                && (after != Some(' ') || lower[at + word.len()..].contains(':'))
        })
    };
    if reports("error") {
        Some(Severity::Error)
    } else if reports("warning") {
        Some(Severity::Warning)
    } else {
        None
    }
}

/// `line` without terminal escape sequences, such as a compiler's colors.
fn strip_escapes(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.next() {
            // CSI: parameters, then one final letter.
            Some('[') => {
                for c in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&c) {
                        break;
                    }
                }
            }
            // OSC: up to BEL or ESC \.
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == '\u{7}' || (c == '\u{1b}' && chars.next_if_eq(&'\\').is_some()) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The last `length` bytes of `path`, without a partial first line.
pub(crate) fn tail(path: &Path, length: u64) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let start = file.metadata()?.len().saturating_sub(length);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(length).read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Ok(match (start > 0, text.find('\n')) {
        (true, Some(newline)) => text[newline + 1..].to_owned(),
        _ => text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: Duration = Duration::from_secs(1_700_000_000);
    const DAY: Duration = Duration::from_secs(24 * 60 * 60);

    fn entry(id: &str, json: &str) -> Entry {
        Entry::parse(id, json, UNIX_EPOCH + NOW).unwrap()
    }

    #[test]
    fn an_entry_needs_only_a_state_and_reads_numbers_either_way() {
        let written = UNIX_EPOCH + NOW;
        let minimal = Entry::parse("a", r#"{"state":"done"}"#, written).unwrap();
        assert_eq!(minimal.time, written);
        assert_eq!((minimal.pid, &minimal.log), (None, &None));

        let full = entry(
            "b",
            "\u{feff}{\"state\":\"running\",\"source\":\"nterm\",\"title\":\" build \",
              \"time\":1700000000,\"pid\":4242,\"started\":\"133754961234567890\",
              \"log\":\"C:\\\\logs\\\\b.log\",\"other\":[1]}",
        );
        assert_eq!(full.title, "build");
        assert_eq!(full.time, UNIX_EPOCH + NOW);
        assert_eq!(full.pid, Some(4242));
        assert_eq!(full.started, Some(133_754_961_234_567_890));
        assert_eq!(full.log.as_deref(), Some(Path::new(r"C:\logs\b.log")));

        let kind = |json| entry("e", json).kind;
        assert_eq!(
            kind(r#"{"state":"done","kind":"codex"}"#).as_deref(),
            Some("codex")
        );
        assert_eq!(kind(r#"{"state":"done","kind":"not a kind"}"#), None);

        for bad in [
            "",
            "[]",
            "{}",
            r#"{"state":""}"#,
            r#"{"state":7}"#,
            "{\"state\":",
        ] {
            assert_eq!(Entry::parse("c", bad, written), None, "{bad}");
        }
        let long = format!(r#"{{"state":"done","title":"{}"}}"#, "x".repeat(2000));
        assert_eq!(entry("d", &long).title.chars().count(), MAX_TEXT + 1);
    }

    #[test]
    fn a_kind_gives_its_entries_their_own_states_and_viewer() {
        let fails = |message: &'static str| move |_: &Path, _: String| Err(message.to_owned());
        let options = SpoolOptions::named("Agents")
            .kind(Kind::new("claude", ring, Color32::RED).viewer(fails("first")))
            .kind(Kind::new("codex", ring, Color32::BLUE))
            .kind(Kind::new("claude", ring, Color32::RED).viewer(fails("claude")));
        assert_eq!(options.kinds.len(), 2);
        let of = |kind: &str, state: &str| {
            entry("a", &format!(r#"{{"state":"{state}","kind":"{kind}"}}"#))
        };
        let plain = entry("a", r#"{"state":"running"}"#);
        let shown_by = |entry: &Entry| {
            let viewer = options.kind_of(entry)?.viewer.as_ref()?;
            viewer(Path::new("log"), String::new()).err()
        };
        // The kind given last, if a name is given twice.
        assert_eq!(shown_by(&of("claude", "done")).as_deref(), Some("claude"));
        // A kind without a viewer, another kind, and no kind: shown as text.
        assert_eq!(shown_by(&of("codex", "done")), None);
        assert_eq!(shown_by(&of("gemini", "done")), None);
        assert_eq!(shown_by(&plain), None);

        assert_eq!(options.state_of(&of("codex", "running")), "codex-running");
        assert_eq!(options.state_of(&of("claude", "waiting")), "claude-waiting");
        assert_eq!(options.state_of(&of("claude", "done")), "claude-done");
        // States a kind does not have, and entries of no known kind, stay plain.
        assert_eq!(options.state_of(&of("codex", "failed")), "failed");
        assert_eq!(options.state_of(&of("gemini", "running")), "running");
        assert_eq!(options.state_of(&plain), "running");

        let mut configuration = crate::Configuration::default();
        let mailbox = register_with(&mut configuration, options);
        let states: Vec<_> = mailbox
            .states
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(
            states,
            [
                "running",
                "waiting",
                "failed",
                "stopped",
                "done",
                "codex-running",
                "codex-waiting",
                "codex-done",
                "claude-running",
                "claude-waiting",
                "claude-done",
            ]
        );
    }

    #[test]
    fn a_log_that_says_its_work_ended_makes_a_running_entry_done() {
        let dir = std::env::temp_dir().join(format!("swm-spool-ended-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("session.jsonl");
        std::fs::write(&log, "working").unwrap();
        let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = asked.clone();
        let options = SpoolOptions::named("Agents")
            .kind(
                Kind::new("claude", ring, Color32::RED).ended(move |log, _| {
                    count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    (std::fs::read_to_string(log).unwrap() == "interrupted")
                        .then(|| "Interrupted".to_owned())
                }),
            )
            .kind(Kind::new("codex", ring, Color32::BLUE));
        let of = |kind: &str, state: &str| {
            let json = serde_json::json!({ "state": state, "kind": kind, "pid": 7, "log": log });
            entry(kind, &json.to_string())
        };
        let mut logs = Logs::default();
        let mut settle = |entries: &mut Vec<Entry>| {
            logs.settle(entries, &options);
            let asked = asked.load(std::sync::atomic::Ordering::Relaxed);
            (entries[0].state.clone(), entries[0].pid, asked)
        };
        let mut entries = vec![of("claude", "running"), of("codex", "running")];
        assert_eq!(settle(&mut entries), (RUNNING.to_owned(), Some(7), 1));
        // The log has not been written since: not asked again.
        assert_eq!(settle(&mut entries), (RUNNING.to_owned(), Some(7), 1));

        std::fs::write(&log, "interrupted").unwrap();
        let mut entries = vec![of("claude", "waiting"), of("codex", "running")];
        // Done, and no longer its process's: it stays listed after the agent exits.
        assert_eq!(settle(&mut entries), (DONE.to_owned(), None, 2));
        assert_eq!(entries[0].detail, "Interrupted");
        // A kind that does not ask its logs keeps what its writer said.
        assert_eq!(entries[1].state, RUNNING);
        let mut entries = vec![of("claude", "running")];
        assert_eq!(settle(&mut entries), (DONE.to_owned(), None, 2));

        // A later prompt: the entry entered its state again, so the log is asked again.
        let mut entries = vec![Entry {
            time: UNIX_EPOCH + NOW + DAY,
            ..of("claude", "running")
        }];
        assert_eq!(settle(&mut entries).2, 3);
        // Only work in progress is asked about.
        let mut entries = vec![of("claude", "done"), of("claude", "failed")];
        assert_eq!(settle(&mut entries), (DONE.to_owned(), Some(7), 3));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_field_names_a_process_by_id_and_start_time() {
        assert_eq!(
            process_reference(" 12840:134388167973335495 "),
            Some((12840, 134388167973335495))
        );
        for unusable in ["", "12840", "0:1", "x:1", "1:x", "1:-1"] {
            assert_eq!(process_reference(unusable), None, "{unusable}");
        }
        let this = std::process::id();
        let started = crate::process::running_since(this).unwrap();
        assert_eq!(running_process(&format!("{this}:{started}")), Some(this));
        // The same ID, started at another time: a later process.
        assert_eq!(running_process(&format!("{this}:{}", started + 1)), None);
    }

    #[test]
    fn what_else_an_entry_says_is_kept_for_its_kind() {
        let said = entry(
            "a",
            r#"{"state":"done","kind":"claude","time":1700000000,"count":3,
                "resume":"claude --resume a","attached":"40472:7","empty":" "}"#,
        );
        // Text only, and none of what is read here.
        let fields: Vec<_> = said.fields.iter().collect();
        assert_eq!(
            fields,
            [
                (&"attached".to_owned(), &"40472:7".to_owned()),
                (&"resume".to_owned(), &"claude --resume a".to_owned())
            ]
        );
        // Listed with its kind as well, for the drop-down's content to go by.
        let listed = Tracker::default().update(vec![said], UNIX_EPOCH + NOW, DAY, |_| false);
        let listed = &listed.entries[0];
        assert_eq!(listed.field("resume"), Some("claude --resume a"));
        assert_eq!(listed.field(KIND), Some("claude"));
        let plain = entry("b", r#"{"state":"done","time":1700000000}"#);
        let listed = Tracker::default().update(vec![plain], UNIX_EPOCH + NOW, DAY, |_| false);
        assert!(listed.entries[0].fields.is_empty());
    }

    fn listed_entry(id: &str, kind: &str) -> MailboxEntry {
        let kind = Some((KIND.to_owned(), kind.to_owned())).filter(|_| !kind.is_empty());
        MailboxEntry {
            id: id.into(),
            state: DONE.into(),
            source: String::new(),
            title: String::new(),
            detail: String::new(),
            time: UNIX_EPOCH,
            fields: kind.into_iter().collect(),
        }
    }

    #[test]
    fn the_drop_down_has_what_the_kinds_of_its_entries_give() {
        let state = |entries| MailboxState {
            counts: Vec::new(),
            entries,
        };
        let commands = |content: &MailboxContent| -> Vec<String> {
            let names = content.commands.iter().map(|command| command.name.clone());
            names.collect()
        };
        let claude = Kind::new("claude", ring, Color32::RED)
            .command("Copy resume command", |_, _| {})
            .note(|entry| Some(Note::colored(format!("open {}", entry.id), Color32::RED)));
        let codex = Kind::new("codex", ring, Color32::BLUE)
            .command("Copy resume command", |_, _| {})
            .command("Fork", |_, _| {})
            .note(|_| Some(Note::plain("closed")));
        let build = Kind::new("build", ring, Color32::GREEN).command("Rerun", |_, _| {});
        let options = SpoolOptions::named("Agents")
            .kind(claude)
            .kind(codex)
            .kind(build);
        let drop_down = picker(&options);

        // Plain cards when no entry is of a kind with anything to add.
        let plain = drop_down(&state(vec![
            listed_entry("a", ""),
            listed_entry("b", "other"),
        ]));
        assert!(plain.columns.is_none());
        assert!(commands(&plain).is_empty());

        // Only the kinds listed, and a name two of them share is listed once.
        let agents = drop_down(&state(vec![
            listed_entry("a", "claude"),
            listed_entry("b", "codex"),
            listed_entry("c", ""),
        ]));
        assert_eq!(commands(&agents), ["Copy resume command", "Fork"]);
        let columns = agents.columns.as_ref().unwrap();
        assert_eq!(columns.names, NOTE_COLUMNS);
        assert_eq!(columns.template, NOTE_TEMPLATE);
        // Each entry's note is its own kind's: a dot and text, or dimmed text.
        let note = |id: &str, kind: &str| (columns.values)(&listed_entry(id, kind));
        assert_eq!(
            note("a", "claude"),
            [
                MailboxContent::dot(Color32::RED),
                "open a".to_owned(),
                String::new()
            ]
        );
        assert_eq!(note("b", "codex"), ["", "", "closed"]);
        assert!(note("c", "").is_empty());

        // A kind with commands and no note leaves the cards as they are.
        let builds = drop_down(&state(vec![listed_entry("a", "build")]));
        assert_eq!(commands(&builds), ["Rerun"]);
        assert!(builds.columns.is_none());
    }

    #[test]
    fn ids_are_file_names_without_a_path() {
        for id in ["build-1", "31f2354f-1479-44b8", "a.b_c"] {
            assert!(is_id(id), "{id}");
        }
        for id in [
            "",
            "..",
            ".hidden",
            r"..\x",
            "a/b",
            "a b",
            "C:x",
            &"x".repeat(101),
        ] {
            assert!(!is_id(id), "{id}");
        }
        assert_eq!(SpoolOptions::named(r"..\Builds").directory(), None);
        let given = SpoolOptions {
            directory: Some(r"D:\boxes".into()),
            ..SpoolOptions::named("Builds")
        };
        assert_eq!(given.directory().as_deref(), Some(Path::new(r"D:\boxes")));
    }

    #[test]
    fn the_folder_is_read_leniently_and_a_file_again_only_when_written() {
        let dir = std::env::temp_dir().join(format!("swm-spool-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("folder.json")).unwrap();
        std::fs::write(dir.join("ok.json"), r#"{"state":"done","title":"one"}"#).unwrap();
        std::fs::write(dir.join("broken.json"), "{").unwrap();
        std::fs::write(dir.join("ok.json.tmp"), r#"{"state":"done"}"#).unwrap();
        std::fs::write(dir.join("not an id.json"), r#"{"state":"done"}"#).unwrap();
        std::fs::write(
            dir.join("huge.json"),
            format!(r#"{{"state":"done","detail":"{}"}}"#, "x".repeat(70_000)),
        )
        .unwrap();
        let mut files = Files::default();
        let titles = |files: &mut Files| {
            let mut titles: Vec<_> = files.read(&dir).into_iter().map(|e| e.title).collect();
            titles.sort();
            titles
        };
        assert_eq!(titles(&mut files), ["one"]);
        // Unchanged files come from what was read before.
        let cached = files.0["ok"].clone();
        assert_eq!(titles(&mut files), ["one"]);
        assert_eq!(files.0["ok"], cached);

        std::fs::write(dir.join("ok.json"), r#"{"state":"done","title":"two!"}"#).unwrap();
        std::fs::write(dir.join("new.json"), r#"{"state":"failed","title":"new"}"#).unwrap();
        assert_eq!(titles(&mut files), ["new", "two!"]);
        std::fs::remove_file(dir.join("ok.json")).unwrap();
        assert_eq!(titles(&mut files), ["new"]);
        assert_eq!(Files::default().read(&dir.join("missing")), []);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_delayed_entry_is_listed_only_once_it_has_run_that_long() {
        let now = UNIX_EPOCH + NOW;
        let command = entry(
            "a",
            r#"{"state":"running","time":1700000000,"pid":7,"delay":5}"#,
        );
        assert_eq!(command.delay, Some(Duration::from_secs(5)));
        let alive = |_: &Entry| true;
        let dead = |_: &Entry| false;
        let at = |seconds| now + Duration::from_secs(seconds);

        let mut tracker = Tracker::default();
        let shown = tracker.update(vec![command.clone()], at(4), DAY, alive);
        assert_eq!(states(&shown), []);
        assert_eq!(shown.counts, []);
        let shown = tracker.update(vec![command.clone()], at(5), DAY, alive);
        assert_eq!(states(&shown), [("a", RUNNING)]);
        // Killed after it was listed: stopped, like any other entry.
        let shown = tracker.update(vec![command.clone()], at(9), DAY, dead);
        assert_eq!(states(&shown), [("a", STOPPED)]);
        let shown = tracker.update(vec![command.clone()], at(10), DAY, dead);
        assert_eq!(states(&shown), [("a", STOPPED)]);

        // Its shell closed before it was ever listed: never listed.
        let mut tracker = Tracker::default();
        assert_eq!(
            states(&tracker.update(vec![command.clone()], at(2), DAY, alive)),
            []
        );
        assert_eq!(
            states(&tracker.update(vec![command.clone()], at(60), DAY, dead)),
            []
        );
        // The outcome, written without a pid, is listed whatever its delay was.
        let done = entry("a", r#"{"state":"done","time":1700000000,"delay":5}"#);
        assert_eq!(
            states(&tracker.update(vec![done], at(60), DAY, dead)),
            [("a", DONE)]
        );
    }

    fn states(state: &MailboxState) -> Vec<(&str, &str)> {
        state
            .entries
            .iter()
            .map(|e| (e.id.as_str(), e.state.as_str()))
            .collect()
    }

    #[test]
    fn an_entry_whose_process_is_gone_is_stopped_from_when_it_was_noticed() {
        let now = UNIX_EPOCH + NOW;
        let started = now - Duration::from_secs(600);
        let build = |id: &str| Entry {
            time: started,
            ..entry(id, r#"{"state":"running","title":"build","pid":7}"#)
        };
        let mut tracker = Tracker::default();
        let alive = |_: &Entry| true;
        let dead = |_: &Entry| false;

        let shown = tracker.update(vec![build("a")], now, 2 * DAY, alive);
        assert_eq!(states(&shown), [("a", RUNNING)]);
        assert_eq!(shown.entries[0].time, started);
        assert!(tracker.shown["a"].1);

        // Killed: stopped now, not when the build started.
        let later = now + Duration::from_secs(5);
        let shown = tracker.update(vec![build("a")], later, 2 * DAY, dead);
        assert_eq!(states(&shown), [("a", STOPPED)]);
        assert_eq!(shown.counts, [(STOPPED.to_owned(), 1)]);
        assert_eq!(shown.entries[0].time, later);
        assert_eq!(shown.entries[0].detail, "Its process ended");
        assert!(!tracker.shown["a"].1);
        // And it stays stopped since then.
        let shown = tracker.update(vec![build("a")], later + DAY, 2 * DAY, dead);
        assert_eq!(shown.entries[0].time, later);
        let shown = tracker.update(vec![build("a")], later + 2 * DAY, 2 * DAY, dead);
        assert_eq!(states(&shown), []);

        // Found already stopped, such as after SWM restarts: since it was written.
        let shown = Tracker::default().update(vec![build("b")], now, 2 * DAY, dead);
        assert_eq!(shown.entries[0].time, started);
    }

    #[test]
    fn finished_entries_expire_and_running_ones_do_not() {
        let now = UNIX_EPOCH + NOW;
        let old = now - 3 * DAY;
        let entries = vec![
            Entry {
                time: old,
                ..entry("old-done", r#"{"state":"done"}"#)
            },
            Entry {
                time: old,
                ..entry("long-build", r#"{"state":"running","pid":7}"#)
            },
            Entry {
                time: now - Duration::from_secs(60),
                ..entry("failed", r#"{"state":"failed","title":"cargo test"}"#)
            },
            Entry {
                time: now + Duration::from_secs(60),
                ..entry("ahead", r#"{"state":"done"}"#)
            },
        ];
        let shown = Tracker::default().update(entries, now, 2 * DAY, |_| true);
        assert_eq!(
            states(&shown),
            [("ahead", DONE), ("failed", FAILED), ("long-build", RUNNING)]
        );
        assert_eq!(shown.count(DONE), 1);
        // An entry without a title shows its id.
        assert_eq!(shown.entries[0].title, "ahead");
        assert_eq!(shown.entries[1].title, "cargo test");
    }

    #[test]
    fn only_the_newest_entries_are_listed_but_all_are_counted() {
        let now = UNIX_EPOCH + NOW;
        let entries = (0..MAX_ENTRIES as u64 + 5)
            .map(|index| Entry {
                time: now - Duration::from_secs(index),
                ..entry(&format!("e{index}"), r#"{"state":"done"}"#)
            })
            .collect();
        let shown = Tracker::default().update(entries, now, DAY, |_| false);
        assert_eq!(shown.entries.len(), MAX_ENTRIES);
        assert_eq!(shown.entries[0].id, "e0");
        assert_eq!(shown.count(DONE), MAX_ENTRIES + 5);
    }

    #[test]
    fn this_process_is_running_and_a_reused_pid_is_not() {
        let me = std::process::id();
        let since = crate::process::running_since(me).unwrap();
        let with = |started: Option<u64>| Entry {
            pid: Some(me),
            started,
            ..entry("a", r#"{"state":"running"}"#)
        };
        assert!(with(None).process_is_running());
        assert!(with(Some(since)).process_is_running());
        assert!(!with(Some(since + 1)).process_is_running());
        assert!(!entry("b", r#"{"state":"running"}"#).process_is_running());
    }

    #[test]
    fn errors_and_warnings_are_found_as_compilers_word_them() {
        for line in [
            "error: could not compile `nterm`",
            "error[E0308]: mismatched types",
            r"src\main.zig:12:5: error: expected ';'",
            "Program.cs(3,4): error CS1002: ; expected",
            "ERROR: build failed",
        ] {
            assert_eq!(severity(line), Some(Severity::Error), "{line}");
        }
        for line in [
            "warning: unused variable: `x`",
            "Program.cs(3,4): warning CS0168: unused",
        ] {
            assert_eq!(severity(line), Some(Severity::Warning), "{line}");
        }
        for line in [
            "   Compiling error-chain v0.12.4",
            "test errors_are_reported ... ok",
            "0 errors, 0 warnings",
            "running 3 tests with error handling",
        ] {
            assert_eq!(severity(line), None, "{line}");
        }
    }

    #[test]
    fn a_log_lists_its_errors_newest_first_without_escape_sequences() {
        assert_eq!(
            strip_escapes("\u{1b}[1m\u{1b}[31merror\u{1b}[0m: x \u{1b}]8;;http://a\u{7}link"),
            "error: x link"
        );
        let dir = std::env::temp_dir().join(format!("swm-spool-log-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("build.log");
        std::fs::write(
            &path,
            "   Compiling nterm\r\n\u{1b}[33mwarning\u{1b}[0m: unused import\r\n\
             error[E0308]: mismatched types\r\nerror: could not compile\r\n",
        )
        .unwrap();
        let picker = log_picker(&path, "nterm".into(), &SpoolOptions::named("Builds")).unwrap();
        let labels: Vec<_> = picker.items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "End of output",
                "error: could not compile",
                "error[E0308]: mismatched types",
                "warning: unused import",
            ]
        );
        assert_eq!(picker.items[2].range.start.line, 2);
        assert!(
            log_picker(
                &dir.join("none.log"),
                String::new(),
                &SpoolOptions::named("x")
            )
            .is_err()
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
