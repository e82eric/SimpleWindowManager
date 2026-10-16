//! A bar mailbox producer for Claude Code sessions, read from Claude's own files:
//! - `sessions/<pid>.json`: one per running Claude process, with its status.
//! - `history.jsonl`: every prompt, with its session.
//! - `projects/<project>/<session>.jsonl`: each session's transcript.
//!
//! Sessions that are no longer running come from their transcripts, as `/resume`
//! finds them: one modified within `DONE_FOR` counts as done when it was modified.
//!
//! It publishes counts for the states `running`, `waiting` and `done`, and an
//! entry per session in those states. Prompts and transcripts are read only from
//! their ends, and only when a session's status changes.
//!
//! These are Claude's internal formats, so reading is lenient: unreadable files and
//! unknown fields are skipped.
use super::transcript::ClaudeOptions;
use crate::mailbox::{MailboxEntry, MailboxEvent, MailboxSender, MailboxState};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const RUNNING: &str = "running";
pub const WAITING: &str = "waiting";
pub const DONE: &str = "done";

/// A finished session counts as done for this long after it finished.
pub const DONE_FOR: Duration = Duration::from_secs(2 * 24 * 60 * 60);
/// How often transcripts are scanned for sessions that are no longer running.
const SCAN_EVERY: Duration = Duration::from_secs(30);
/// The status of a session between turns, and of a new one before its first.
const IDLE: &str = "idle";
/// Claude writes its first idle status as it starts. An idle status written later
/// than this after the session started follows a finished turn.
const STARTUP: Duration = Duration::from_secs(5);
/// How much of the end of `history.jsonl` and of a transcript is read.
const TAIL_BYTES: u64 = 256 * 1024;
const MAX_ENTRIES: usize = 50;

/// `CLAUDE_CONFIG_DIR`, or `.claude` in the user's profile.
fn claude_dir() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(|home| Path::new(&home).join(".claude")))
}

/// Publishes Claude Code sessions to `mailbox` once a second, and opens an entry's
/// session in a picker. Pass it to `Configuration::bar_mailbox`, or pass
/// `producer_with` to change how sessions are shown.
pub fn producer(mailbox: MailboxSender) {
    run(mailbox, &ClaudeOptions::default());
}

/// `producer` showing sessions as `options` says.
pub fn producer_with(options: ClaudeOptions) -> impl Fn(MailboxSender) + Send + Sync + 'static {
    move |mailbox| run(mailbox, &options)
}

fn run(mailbox: MailboxSender, options: &ClaudeOptions) {
    let mut tracker = Tracker::default();
    let mut details = Details::default();
    let mut transcripts = Transcripts::default();
    loop {
        let now = SystemTime::now();
        let dir = claude_dir();
        let live = dir.as_deref().map(live_sessions).unwrap_or_default();
        tracker.update(live, transcripts.scan(dir.as_deref(), now), now);
        mailbox.publish(tracker.publish(now, |id, session| {
            dir.as_deref()
                .map(|dir| details.get(dir, id, session))
                .unwrap_or_default()
        }));
        if let Some(MailboxEvent::Activated { id, .. }) = mailbox.wait_event(Duration::from_secs(1))
            && let Some(dir) = dir.as_deref()
        {
            let cwd = tracker
                .0
                .get(&id)
                .map_or("", |session| session.cwd.as_str());
            if let Err(error) = open(&mailbox, dir, &id, cwd, options) {
                crate::error_log::report("Claude session", error);
            }
        }
    }
}

/// Shows session `id`'s conversation in a picker.
fn open(
    mailbox: &MailboxSender,
    dir: &Path,
    id: &str,
    cwd: &str,
    options: &ClaudeOptions,
) -> Result<(), String> {
    let path = transcript(dir, id).ok_or_else(|| format!("no transcript for session {id}"))?;
    let project = project(cwd);
    let title = if project.is_empty() {
        "Claude".to_owned()
    } else {
        format!("Claude · {project}")
    };
    let picker = super::transcript::picker(&path, title, options)?;
    if !mailbox.show_picker(picker) {
        return Err("SWM's UI is not running".into());
    }
    Ok(())
}

/// A running session's state, from its file.
struct Live {
    status: String,
    updated: Option<SystemTime>,
    started: Option<SystemTime>,
    cwd: String,
}

/// Remembers sessions while they matter to the bar, including after they exit.
#[derive(Default)]
struct Tracker(HashMap<String, Tracked>);
#[derive(Clone)]
struct Tracked {
    status: String,
    running: bool,
    updated: Option<SystemTime>,
    /// When the session last finished a turn, if it has finished one.
    finished: Option<SystemTime>,
    cwd: String,
}
impl Tracker {
    fn update(
        &mut self,
        live: HashMap<String, Live>,
        recorded: &HashMap<String, Recorded>,
        now: SystemTime,
    ) {
        let mut previous = std::mem::take(&mut self.0);
        for (id, session) in live {
            let before = previous.remove(&id);
            let after_a_turn = before
                .as_ref()
                .is_some_and(|before| before.status != IDLE || before.finished.is_some())
                || session
                    .updated
                    .zip(session.started)
                    .is_some_and(|(updated, started)| updated > started + STARTUP);
            let finished = (session.status == IDLE && after_a_turn).then(|| {
                session
                    .updated
                    .or_else(|| before.and_then(|before| before.finished))
                    .unwrap_or(now)
            });
            self.0.insert(
                id,
                Tracked {
                    status: session.status,
                    running: true,
                    updated: session.updated,
                    finished,
                    cwd: session.cwd,
                },
            );
        }
        // Sessions whose process has exited.
        for (id, mut session) in previous {
            if std::mem::take(&mut session.running) {
                match session.status.as_str() {
                    // Done when it went idle, if it had finished a turn.
                    IDLE => {}
                    // Left waiting for the person: nothing finished.
                    "waiting" | "blocked" => continue,
                    // Exited mid-turn: that turn is over now.
                    _ => session.finished = Some(now),
                }
            }
            self.0.insert(id, session);
        }
        // Sessions that ended before this ran, or before their transcript was
        // last scanned, finished when their transcript was last written.
        for (id, recorded) in recorded {
            self.0.entry(id.clone()).or_insert_with(|| Tracked {
                status: IDLE.into(),
                running: false,
                updated: Some(recorded.modified),
                finished: Some(recorded.modified),
                cwd: recorded.cwd.clone(),
            });
        }
        self.0
            .retain(|_, session| session.running || recent(session.finished, now));
    }

    /// Each session the bar shows, with its state and when it entered it.
    fn shown(&self, now: SystemTime) -> Vec<(&str, &Tracked, &'static str, SystemTime)> {
        let mut shown: Vec<_> = self
            .0
            .iter()
            .filter_map(|(id, session)| {
                let since = session.updated.unwrap_or(now);
                let (state, since) = match (session.running, session.status.as_str()) {
                    (true, "waiting" | "blocked") => (WAITING, since),
                    (true, IDLE) if recent(session.finished, now) => (DONE, session.finished?),
                    (true, IDLE) => return None,
                    (true, _) => (RUNNING, since),
                    (false, _) => (DONE, session.finished?),
                };
                Some((id.as_str(), session, state, since))
            })
            .collect();
        // Waiting first, then running, then finished; newest first within each.
        let rank = |state| [WAITING, RUNNING, DONE].iter().position(|s| *s == state);
        shown.sort_by_key(|&(_, _, state, since)| (rank(state), std::cmp::Reverse(since)));
        shown
    }

    fn publish(
        &self,
        now: SystemTime,
        mut describe: impl FnMut(&str, &Tracked) -> Description,
    ) -> MailboxState {
        let shown = self.shown(now);
        let count = |state| shown.iter().filter(|(_, _, s, _)| *s == state).count();
        MailboxState {
            counts: [RUNNING, WAITING, DONE]
                .iter()
                .map(|&state| (state.to_owned(), count(state)))
                .collect(),
            entries: shown
                .iter()
                .take(MAX_ENTRIES)
                .map(|&(id, session, state, since)| {
                    let description = describe(id, session);
                    MailboxEntry {
                        id: id.to_owned(),
                        state: state.to_owned(),
                        source: project(&session.cwd),
                        title: if description.prompt.is_empty() {
                            "New session".into()
                        } else {
                            description.prompt
                        },
                        detail: if description.activity.is_empty() {
                            status_text(state, &session.status)
                        } else {
                            description.activity
                        },
                        time: since,
                    }
                })
                .collect(),
        }
    }
}

fn recent(at: Option<SystemTime>, now: SystemTime) -> bool {
    // A timestamp ahead of this clock is recent too.
    at.is_some_and(|at| now.duration_since(at).map_or(true, |age| age < DONE_FOR))
}

fn status_text(state: &str, status: &str) -> String {
    match (state, status) {
        (WAITING, "blocked") => "Blocked".into(),
        (WAITING, _) => "Waiting for you".into(),
        (DONE, _) => "Finished".into(),
        (_, status) => {
            let mut chars = status.chars();
            chars.next().map_or_else(
                || "Working".into(),
                |first| first.to_uppercase().chain(chars).collect(),
            )
        }
    }
}

/// The last two parts of `cwd`: `SimpleWindowManager/rust-port`.
fn project(cwd: &str) -> String {
    let parts: Vec<_> = cwd
        .split(['\\', '/'])
        .filter(|part| !part.is_empty() && !part.ends_with(':'))
        .collect();
    parts[parts.len().saturating_sub(2)..].join("/")
}

/// What a session was asked and what it did last.
#[derive(Clone, Default)]
struct Description {
    prompt: String,
    activity: String,
}

/// Descriptions by session, re-read when the session's status changes.
#[derive(Default)]
struct Details(HashMap<String, (String, Option<SystemTime>, Description)>);
impl Details {
    fn get(&mut self, dir: &Path, id: &str, session: &Tracked) -> Description {
        if let Some((status, updated, description)) = self.0.get(id)
            && *status == session.status
            && *updated == session.updated
        {
            return description.clone();
        }
        let transcript = transcript(dir, id);
        let mut prompt = last_prompt(dir, id);
        // Older sessions' prompts have left the end of `history.jsonl`.
        if prompt.is_empty()
            && let Some(transcript) = &transcript
        {
            prompt = transcript_prompt(transcript);
        }
        let description = Description {
            prompt,
            activity: transcript
                .map(|path| last_activity(&path))
                .unwrap_or_default(),
        };
        self.0.insert(
            id.to_owned(),
            (session.status.clone(), session.updated, description.clone()),
        );
        description
    }
}

/// The last `TAIL_BYTES` of `path`, without a partial first line.
fn tail(path: &Path) -> String {
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let length = file.metadata().map_or(0, |m| m.len());
    let start = length.saturating_sub(TAIL_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut bytes = Vec::new();
    let _ = file.take(TAIL_BYTES).read_to_end(&mut bytes);
    let text = String::from_utf8_lossy(&bytes).into_owned();
    match (start > 0, text.find('\n')) {
        (true, Some(newline)) => text[newline + 1..].to_owned(),
        (true, None) => String::new(),
        (false, _) => text,
    }
}

fn last_prompt(dir: &Path, id: &str) -> String {
    tail(&dir.join("history.jsonl"))
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|entry| entry["sessionId"] == id)
        .and_then(|entry| entry["display"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn transcript(dir: &Path, id: &str) -> Option<PathBuf> {
    let name = format!("{id}.jsonl");
    std::fs::read_dir(dir.join("projects"))
        .ok()?
        .flatten()
        .map(|project| project.path().join(&name))
        .find(|path| path.is_file())
}

/// The last prompt `transcript` records.
fn transcript_prompt(transcript: &Path) -> String {
    tail(transcript)
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find_map(|entry| {
            (entry["type"] == "last-prompt")
                .then(|| entry["lastPrompt"].as_str().map(str::to_owned))
                .flatten()
        })
        .unwrap_or_default()
}

/// A session found from its transcript.
struct Recorded {
    modified: SystemTime,
    cwd: String,
}

/// Sessions whose transcripts were modified within `DONE_FOR`, rescanned every
/// `SCAN_EVERY`. A transcript is read again only when it has been modified.
#[derive(Default)]
struct Transcripts {
    scanned: Option<SystemTime>,
    sessions: HashMap<String, Recorded>,
    /// The conversation's working directory by transcript, and when it was read.
    read: HashMap<PathBuf, (SystemTime, Option<String>)>,
}
impl Transcripts {
    fn scan(&mut self, dir: Option<&Path>, now: SystemTime) -> &HashMap<String, Recorded> {
        let due = self.scanned.is_none_or(|scanned| {
            now.duration_since(scanned)
                .map_or(true, |since| since >= SCAN_EVERY)
        });
        if !due {
            return &self.sessions;
        }
        self.scanned = Some(now);
        let mut sessions = HashMap::new();
        let mut read = HashMap::new();
        let projects = dir.and_then(|dir| std::fs::read_dir(dir.join("projects")).ok());
        // Subagents' transcripts are in folders below these, so are not listed.
        let files = projects
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|project| std::fs::read_dir(project.path()).ok())
            .flatten()
            .flatten();
        for file in files {
            let path = file.path();
            if path.extension().is_none_or(|e| e != "jsonl") {
                continue;
            }
            let Some(modified) = file.metadata().ok().and_then(|m| m.modified().ok()) else {
                continue;
            };
            if !recent(Some(modified), now) {
                continue;
            }
            let cwd = match self.read.remove(&path) {
                Some((at, cwd)) if at == modified => cwd,
                _ => conversation_cwd(&path),
            };
            if let (Some(id), Some(cwd)) = (path.file_stem().and_then(|s| s.to_str()), &cwd) {
                sessions.insert(
                    id.to_owned(),
                    Recorded {
                        modified,
                        cwd: cwd.clone(),
                    },
                );
            }
            read.insert(path, (modified, cwd));
        }
        self.sessions = sessions;
        self.read = read;
        &self.sessions
    }
}

/// The working directory of the last message in `transcript`, or `None` if it
/// has no messages: a session closed before its first prompt.
fn conversation_cwd(transcript: &Path) -> Option<String> {
    let message = tail(transcript)
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|entry| {
            matches!(entry["type"].as_str(), Some("user" | "assistant"))
                && entry["isSidechain"] != true
        })
        .map(|entry| entry["cwd"].as_str().unwrap_or_default().to_owned());
    // Messages can be further back than the tail reaches in a long transcript.
    let long = || std::fs::metadata(transcript).is_ok_and(|m| m.len() > TAIL_BYTES);
    message.or_else(|| long().then(String::new))
}

/// The assistant's last text or tool call in `transcript`.
fn last_activity(transcript: &Path) -> String {
    for line in tail(transcript).lines().rev() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if entry["type"] != "assistant" || entry["isSidechain"] == true {
            continue;
        }
        let Some(blocks) = entry["message"]["content"].as_array() else {
            continue;
        };
        for block in blocks.iter().rev() {
            match block["type"].as_str() {
                Some("text") => {
                    if let Some(text) = block["text"].as_str().filter(|t| !t.trim().is_empty()) {
                        return text.to_owned();
                    }
                }
                Some("tool_use") => return tool_summary(block),
                _ => {}
            }
        }
    }
    String::new()
}

/// A tool call as its name and what it acted on: `Edit src/bar.rs`.
pub(super) fn tool_summary(block: &Value) -> String {
    let name = block["name"].as_str().unwrap_or("Tool");
    let input = &block["input"];
    let target = [
        "file_path",
        "command",
        "pattern",
        "path",
        "url",
        "description",
    ]
    .iter()
    .find_map(|key| input[*key].as_str())
    .unwrap_or_default();
    // A multi-line command shows its first line.
    let target = target.lines().next().unwrap_or_default();
    format!("{name} {target}").trim().to_owned()
}

/// Running sessions by id. A file whose process has exited, or whose pid now belongs
/// to another process, is ignored; Claude checks `procStart` the same way.
fn live_sessions(dir: &Path) -> HashMap<String, Live> {
    let mut live = HashMap::new();
    let Ok(files) = std::fs::read_dir(dir.join("sessions")) else {
        return live;
    };
    for file in files.flatten() {
        let path = file.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let Some(entry) = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        else {
            continue;
        };
        let (Some(id), Some(pid), Some(process_start)) = (
            entry["sessionId"].as_str(),
            entry["pid"].as_u64().and_then(|p| u32::try_from(p).ok()),
            entry["procStart"]
                .as_str()
                .and_then(|s| s.parse::<u64>().ok()),
        ) else {
            continue;
        };
        if !crate::process::is_running(pid, process_start) {
            continue;
        }
        live.insert(
            id.to_owned(),
            Live {
                status: entry["status"].as_str().unwrap_or("running").to_owned(),
                updated: entry["statusUpdatedAt"]
                    .as_u64()
                    .or_else(|| entry["updatedAt"].as_u64())
                    .map(from_millis),
                started: entry["startedAt"].as_u64().map(from_millis),
                cwd: entry["cwd"].as_str().unwrap_or_default().to_owned(),
            },
        );
    }
    live
}

fn from_millis(millis: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: Duration = Duration::from_secs(1_000_000);
    const DAY: Duration = Duration::from_secs(24 * 60 * 60);

    fn session(status: &str, updated: Option<SystemTime>, started: Option<SystemTime>) -> Live {
        Live {
            status: status.into(),
            updated,
            started,
            cwd: r"G:\src\SimpleWindowManager\rust-port".into(),
        }
    }
    /// Sessions started a week before `START`, each with its status and when it
    /// was written, in seconds before `now`.
    fn live(now: SystemTime, sessions: &[(&str, &str, u64)]) -> HashMap<String, Live> {
        let started = Some(UNIX_EPOCH + START - 7 * DAY);
        sessions
            .iter()
            .map(|&(id, status, ago)| {
                let updated = Some(now - Duration::from_secs(ago));
                (id.to_owned(), session(status, updated, started))
            })
            .collect()
    }
    /// (running, waiting, done)
    fn counts(tracker: &Tracker, now: SystemTime) -> (usize, usize, usize) {
        let state = tracker.publish(now, |_, _| Description::default());
        (
            state.count(RUNNING),
            state.count(WAITING),
            state.count(DONE),
        )
    }

    #[test]
    fn idle_counts_as_done_only_after_a_recent_turn() {
        let now = UNIX_EPOCH + START;
        let mut tracker = Tracker::default();
        let mut sessions = live(
            now,
            &[
                ("busy", "busy", 5),
                ("prompt", "waiting", 5),
                ("blocked", "blocked", 5),
                ("finished", "idle", 60),
                ("stale", "idle", DONE_FOR.as_secs() + 60),
            ],
        );
        // Claude writes a new session's idle status just before its start time.
        sessions.insert(
            "new".into(),
            session(IDLE, Some(now - Duration::from_secs(1)), Some(now)),
        );
        tracker.update(sessions, &HashMap::new(), now);
        assert_eq!(counts(&tracker, now), (1, 2, 1));
    }

    #[test]
    fn a_session_seen_working_is_done_when_it_goes_idle() {
        let now = UNIX_EPOCH + START;
        let one = |status: &str| HashMap::from([("id".to_owned(), session(status, None, None))]);
        let mut tracker = Tracker::default();
        tracker.update(one("idle"), &HashMap::new(), now);
        assert_eq!(counts(&tracker, now), (0, 0, 0));
        tracker.update(one("busy"), &HashMap::new(), now);
        assert_eq!(counts(&tracker, now), (1, 0, 0));
        tracker.update(one("idle"), &HashMap::new(), now);
        assert_eq!(counts(&tracker, now), (0, 0, 1));
        let later = now + Duration::from_secs(60);
        tracker.update(one("idle"), &HashMap::new(), later);
        assert_eq!(counts(&tracker, later), (0, 0, 1));
        tracker.update(one("idle"), &HashMap::new(), now + DONE_FOR);
        assert_eq!(counts(&tracker, now + DONE_FOR), (0, 0, 0));
    }

    #[test]
    fn exited_sessions_are_timed_from_when_they_finished() {
        let now = UNIX_EPOCH + START;
        let mut tracker = Tracker::default();
        tracker.update(
            live(
                now,
                &[
                    ("long turn", "busy", 15 * 60),
                    ("finished", "idle", DONE_FOR.as_secs() - 60),
                    ("prompt", "waiting", 5),
                ],
            ),
            &HashMap::new(),
            now,
        );
        // All three exit. The long turn ends now; the idle session finished a
        // minute before it would stop counting; the one waiting for the person never finished.
        tracker.update(HashMap::new(), &HashMap::new(), now);
        assert_eq!(counts(&tracker, now), (0, 0, 2));
        let minute = now + Duration::from_secs(60);
        tracker.update(HashMap::new(), &HashMap::new(), minute);
        assert_eq!(counts(&tracker, minute), (0, 0, 1));
        tracker.update(HashMap::new(), &HashMap::new(), now + DONE_FOR);
        assert_eq!(counts(&tracker, now + DONE_FOR), (0, 0, 0));
        assert!(tracker.0.is_empty());
    }

    #[test]
    fn a_new_session_that_exits_is_forgotten() {
        let now = UNIX_EPOCH + START;
        let mut tracker = Tracker::default();
        let new = HashMap::from([("new".to_owned(), session(IDLE, Some(now), Some(now)))]);
        tracker.update(new, &HashMap::new(), now);
        assert_eq!(counts(&tracker, now), (0, 0, 0));
        tracker.update(HashMap::new(), &HashMap::new(), now);
        assert!(tracker.0.is_empty());
    }

    #[test]
    fn recorded_sessions_are_done_from_when_their_transcript_was_written() {
        let now = UNIX_EPOCH + START;
        let recorded = |ago: Duration| Recorded {
            modified: now - ago,
            cwd: r"G:\src\dotfiles".into(),
        };
        let transcripts = HashMap::from([
            ("closed".to_owned(), recorded(DAY)),
            ("busy".to_owned(), recorded(Duration::ZERO)),
        ]);
        let mut tracker = Tracker::default();
        tracker.update(live(now, &[("busy", "busy", 5)]), &transcripts, now);
        assert_eq!(counts(&tracker, now), (1, 0, 1));
        assert_eq!(tracker.0["closed"].cwd, r"G:\src\dotfiles");
        let expired = now + DAY;
        tracker.update(live(now, &[("busy", "busy", 5)]), &transcripts, expired);
        assert_eq!(counts(&tracker, expired), (1, 0, 0));
    }

    #[test]
    fn transcripts_give_their_last_prompt_and_conversation_folder() {
        let dir = std::env::temp_dir().join(format!("swm-claude-scan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let conversation = dir.join("conversation.jsonl");
        std::fs::write(
            &conversation,
            [
                r#"{"type":"user","cwd":"G:\\src\\nfm","message":{"content":"hi"}}"#,
                r#"{"type":"assistant","isSidechain":true,"cwd":"G:\\other"}"#,
                r#"{"type":"last-prompt","lastPrompt":"fix the bar"}"#,
            ]
            .join("\n"),
        )
        .unwrap();
        let unused = dir.join("unused.jsonl");
        std::fs::write(&unused, r#"{"type":"mode","mode":"default"}"#).unwrap();
        assert_eq!(
            conversation_cwd(&conversation).as_deref(),
            Some(r"G:\src\nfm")
        );
        assert_eq!(transcript_prompt(&conversation), "fix the bar");
        assert_eq!(conversation_cwd(&unused), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn entries_list_waiting_then_running_then_done() {
        let now = UNIX_EPOCH + START;
        let mut tracker = Tracker::default();
        tracker.update(
            live(
                now,
                &[
                    ("busy", "busy", 5),
                    ("old done", "idle", 300),
                    ("new done", "idle", 30),
                    ("prompt", "waiting", 60),
                ],
            ),
            &HashMap::new(),
            now,
        );
        let describe = |id: &str, _: &Tracked| Description {
            prompt: format!("prompt of {id}"),
            activity: String::new(),
        };
        let state = tracker.publish(now, describe);
        let listed: Vec<_> = state
            .entries
            .iter()
            .map(|e| (e.id.as_str(), e.state.as_str()))
            .collect();
        assert_eq!(
            listed,
            [
                ("prompt", WAITING),
                ("busy", RUNNING),
                ("new done", DONE),
                ("old done", DONE),
            ]
        );
        let first = &state.entries[0];
        assert_eq!(first.source, "SimpleWindowManager/rust-port");
        assert_eq!(first.title, "prompt of prompt");
        assert_eq!(first.detail, "Waiting for you");
    }

    #[test]
    fn activity_is_the_last_main_thread_text_or_tool_call() {
        let dir = std::env::temp_dir().join(format!("swm-claude-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("transcript.jsonl");
        let lines = [
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Looking"}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"src/bar.rs"}}]}}"#,
            r#"{"type":"assistant","isSidechain":true,"message":{"content":[{"type":"text","text":"subagent"}]}}"#,
            r#"{"type":"user","message":{"content":"result"}}"#,
        ];
        std::fs::write(&path, lines.join("\n")).unwrap();
        assert_eq!(last_activity(&path), "Edit src/bar.rs");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn projects_show_their_last_two_folders() {
        assert_eq!(
            project(r"G:\src\SimpleWindowManager"),
            "src/SimpleWindowManager"
        );
        assert_eq!(project(r"G:\"), "");
        assert_eq!(project("/home/me/app"), "me/app");
    }
}
