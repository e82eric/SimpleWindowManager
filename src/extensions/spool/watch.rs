//! What `swm-mailbox watch` does on a machine whose mailbox folders another
//! machine's SWM shows (`spool::remote`): it reads every folder here as SWM
//! reads its own, and prints what is in them and what changes.
//!
//! It is started by that SWM through `ssh`, and its output is the connection.
//! It listens for nothing, reads no input, and ends when its output closes.
//!
//! Each line it prints is one message, as JSON:
//!
//! - `{"swm-mailbox":1}` first: the version of these messages.
//! - `{"entry":{"mailbox":"Agents","id":"a","text":"…","written":1700000000000,"running":true}}`
//!   for an entry file that is new or has changed: its text as it is, when it
//!   was written (milliseconds since 1970), and whether the process it names
//!   runs here. Whether it does is only known on this machine.
//! - `{"gone":{"mailbox":"Agents","id":"a"}}` for one that was removed.
//! - `{"log":{"mailbox":"Agents","id":"a","start":true,"text":"…"}}` for an
//!   entry's log: with `start` the end of it, replacing what was sent before,
//!   and without, what has been added to it since.
//! - `{"pass":true}` each time everything has been read, about once a second:
//!   what was printed up to here is how the folders are now.
use super::{Entry, Files, LOG_TAIL_BYTES, is_id};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The version of the messages. A reader shows nothing of another version.
pub const VERSION: u64 = 1;
/// Folders read; any more are ignored.
const MAX_MAILBOXES: usize = 16;

/// One line of what a watch prints.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Message {
    Hello {
        version: u64,
    },
    Entry {
        mailbox: String,
        id: String,
        text: String,
        written: u64,
        running: bool,
    },
    Gone {
        mailbox: String,
        id: String,
    },
    Log {
        mailbox: String,
        id: String,
        start: bool,
        text: String,
    },
    Pass,
}
impl Message {
    pub(super) fn line(&self) -> String {
        match self {
            Self::Hello { version } => json!({ "swm-mailbox": version }),
            Self::Entry {
                mailbox,
                id,
                text,
                written,
                running,
            } => json!({ "entry": {
                "mailbox": mailbox, "id": id, "text": text, "written": written, "running": running,
            } }),
            Self::Gone { mailbox, id } => json!({ "gone": { "mailbox": mailbox, "id": id } }),
            Self::Log {
                mailbox,
                id,
                start,
                text,
            } => json!({ "log": { "mailbox": mailbox, "id": id, "start": start, "text": text } }),
            Self::Pass => json!({ "pass": true }),
        }
        .to_string()
    }

    /// The message on `line`, if it is one whose mailbox and entry are named
    /// as a folder and a file can be.
    pub(super) fn parse(line: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(line).ok()?;
        if let Some(version) = value["swm-mailbox"].as_u64() {
            return Some(Self::Hello { version });
        }
        if value["pass"].as_bool() == Some(true) {
            return Some(Self::Pass);
        }
        let named = |of: &Value| {
            let name = |key: &str| of[key].as_str().filter(|name| is_id(name)).map(str::to_owned);
            Some((name("mailbox")?, name("id")?))
        };
        if let Some(entry) = value.get("entry") {
            let (mailbox, id) = named(entry)?;
            return Some(Self::Entry {
                mailbox,
                id,
                text: entry["text"].as_str()?.to_owned(),
                written: entry["written"].as_u64()?,
                running: entry["running"].as_bool()?,
            });
        }
        if let Some(gone) = value.get("gone") {
            let (mailbox, id) = named(gone)?;
            return Some(Self::Gone { mailbox, id });
        }
        let log = value.get("log")?;
        let (mailbox, id) = named(log)?;
        Some(Self::Log {
            mailbox,
            id,
            start: log["start"].as_bool()?,
            text: log["text"].as_str()?.to_owned(),
        })
    }
}

/// What was last said of an entry.
#[derive(PartialEq)]
struct Sent {
    text: String,
    written: u64,
    running: bool,
}

/// How much of a log has been sent.
struct LogSent {
    path: PathBuf,
    offset: u64,
}

#[derive(Default)]
struct Folder {
    files: Files<(Entry, String)>,
    sent: HashMap<String, Sent>,
    logs: HashMap<String, LogSent>,
}

/// The folders of a mailbox root, and what has been said of them so far.
#[derive(Default)]
pub(super) struct Watch(BTreeMap<String, Folder>);
impl Watch {
    /// Reads every folder in `root` and gives what to say of it: everything
    /// the first time, and from then on what has changed. `is_running` says
    /// whether the process an entry names runs.
    pub(super) fn pass(
        &mut self,
        root: &Path,
        is_running: &dyn Fn(&Entry) -> bool,
    ) -> Vec<Message> {
        let mut messages = Vec::new();
        let mut names: Vec<String> = std::fs::read_dir(root)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|folder| folder.metadata().is_ok_and(|metadata| metadata.is_dir()))
            .filter_map(|folder| folder.file_name().into_string().ok())
            .filter(|name| is_id(name))
            .collect();
        names.sort();
        names.truncate(MAX_MAILBOXES);

        let mut folders = BTreeMap::new();
        for name in names {
            let mut folder = self.0.remove(&name).unwrap_or_default();
            folder.pass(&name, &root.join(&name), is_running, &mut messages);
            folders.insert(name, folder);
        }
        // What is left is of folders that are no longer there.
        for (mailbox, folder) in std::mem::replace(&mut self.0, folders) {
            for id in folder.sent.into_keys() {
                let mailbox = mailbox.clone();
                messages.push(Message::Gone { mailbox, id });
            }
        }
        messages
    }
}
impl Folder {
    fn pass(
        &mut self,
        mailbox: &str,
        directory: &Path,
        is_running: &dyn Fn(&Entry) -> bool,
        messages: &mut Vec<Message>,
    ) {
        let read = self.files.read_as(directory, |id, text, written| {
            Some((Entry::parse(id, text, written)?, text.to_owned()))
        });
        let files = &self.files.0;
        let mut sent = HashMap::new();
        let mut logs = HashMap::new();
        for (entry, text) in read {
            let written = files.get(&entry.id).map_or(UNIX_EPOCH, |(at, ..)| *at);
            let now = Sent {
                text,
                written: milliseconds(written),
                running: entry.pid.is_some() && is_running(&entry),
            };
            if self.sent.get(&entry.id) != Some(&now) {
                messages.push(Message::Entry {
                    mailbox: mailbox.to_owned(),
                    id: entry.id.clone(),
                    text: now.text.clone(),
                    written: now.written,
                    running: now.running,
                });
            }
            sent.insert(entry.id.clone(), now);

            let Some(path) = &entry.log else { continue };
            let before = self.logs.remove(&entry.id);
            match more_of_log(path, before.as_ref()) {
                Some((now, start, text)) => {
                    messages.push(Message::Log {
                        mailbox: mailbox.to_owned(),
                        id: entry.id.clone(),
                        start,
                        text,
                    });
                    logs.insert(entry.id.clone(), now);
                }
                None => {
                    if let Some(before) = before {
                        logs.insert(entry.id.clone(), before);
                    }
                }
            }
        }
        for id in self.sent.keys().filter(|id| !sent.contains_key(*id)) {
            messages.push(Message::Gone {
                mailbox: mailbox.to_owned(),
                id: id.clone(),
            });
        }
        self.sent = sent;
        self.logs = logs;
    }
}

fn milliseconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64)
}

/// What to send of the log at `path` after `sent`: whether it starts the log
/// again, the text, and how far that reaches. The first time, and when the log
/// is another file or a shorter one, that is its end without a partial first
/// line; otherwise what has been added, if anything has.
fn more_of_log(path: &Path, sent: Option<&LogSent>) -> Option<(LogSent, bool, String)> {
    let mut file = std::fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    let (start, from) = match sent {
        Some(sent) if sent.path == path && length >= sent.offset => (false, sent.offset),
        _ => (true, length.saturating_sub(LOG_TAIL_BYTES)),
    };
    if !start && length == from {
        return None;
    }
    file.seek(SeekFrom::Start(from)).ok()?;
    let mut bytes = Vec::new();
    file.take(LOG_TAIL_BYTES).read_to_end(&mut bytes).ok()?;
    let skipped = match bytes.iter().position(|byte| *byte == b'\n') {
        Some(newline) if start && from > 0 => newline + 1,
        _ => 0,
    };
    let bytes = &bytes[skipped..];
    // A character cut in two at the end is sent with what follows it.
    let (text, used) = match std::str::from_utf8(bytes) {
        Ok(text) => (text.to_owned(), bytes.len()),
        Err(error) if error.error_len().is_none() => {
            let whole = error.valid_up_to();
            (String::from_utf8_lossy(&bytes[..whole]).into_owned(), whole)
        }
        Err(_) => (String::from_utf8_lossy(bytes).into_owned(), bytes.len()),
    };
    if !start && used == 0 {
        return None;
    }
    let sent = LogSent {
        path: path.to_owned(),
        offset: from + (skipped + used) as u64,
    };
    Some((sent, start, text))
}

/// Where the mailbox folders are on this machine: `%LOCALAPPDATA%\swm\mailbox`.
pub fn root() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(Path::new(&local).join("swm").join("mailbox"))
}

/// Prints the folders in `root` to `out`, and what changes in them from then
/// on, until `out` can no longer be written to.
pub fn run(root: &Path, out: &mut impl Write) -> std::io::Result<()> {
    let mut watch = Watch::default();
    writeln!(out, "{}", Message::Hello { version: VERSION }.line())?;
    loop {
        for message in watch.pass(root, &Entry::process_is_running) {
            writeln!(out, "{}", message.line())?;
        }
        writeln!(out, "{}", Message::Pass.line())?;
        out.flush()?;
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("swm-watch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Agents")).unwrap();
        root
    }

    #[test]
    fn a_message_is_read_as_it_was_written() {
        let named = || ("Agents".to_owned(), "claude-1".to_owned());
        let (mailbox, id) = named();
        let entry = Message::Entry {
            mailbox,
            id,
            text: "{\"state\":\"running\",\"title\":\"a \\\"b\\\"\nc\"}".into(),
            written: 1_700_000_000_123,
            running: true,
        };
        let (mailbox, id) = named();
        let gone = Message::Gone { mailbox, id };
        let (mailbox, id) = named();
        let log = Message::Log {
            mailbox,
            id,
            start: false,
            text: "line\r\nénd".into(),
        };
        let hello = Message::Hello { version: 7 };
        for message in [hello, entry, gone, log, Message::Pass] {
            let line = message.line();
            assert!(!line.contains('\n'), "{line}");
            assert_eq!(Message::parse(&line), Some(message));
        }
        // A name that could be a path somewhere else is not taken.
        let outside = r#"{"gone":{"mailbox":"..\\..\\x","id":"a"}}"#;
        assert_eq!(Message::parse(outside), None);
        let outside = r#"{"log":{"mailbox":"Agents","id":"a/b","start":true,"text":""}}"#;
        assert_eq!(Message::parse(outside), None);
        assert_eq!(Message::parse("Welcome to the machine"), None);
    }

    #[test]
    fn a_pass_says_everything_once_and_then_what_changed() {
        let root = folder("pass");
        let agents = root.join("Agents");
        std::fs::create_dir_all(root.join("not a mailbox")).unwrap();
        std::fs::write(agents.join("a.json"), r#"{"state":"running","pid":7}"#).unwrap();
        std::fs::write(agents.join("b.json"), r#"{"state":"done"}"#).unwrap();
        let running = std::cell::Cell::new(true);
        let is_running = |_: &Entry| running.get();
        let mut watch = Watch::default();
        let said = |watch: &mut Watch| {
            let mut said: Vec<String> = watch
                .pass(&root, &is_running)
                .into_iter()
                .map(|message| match message {
                    Message::Entry { id, running, .. } => format!("{id} running={running}"),
                    Message::Gone { mailbox, id } => format!("{mailbox} {id} gone"),
                    other => format!("{other:?}"),
                })
                .collect();
            said.sort();
            said
        };
        // Only an entry that names a process can be running.
        assert_eq!(said(&mut watch), ["a running=true", "b running=false"]);
        assert_eq!(said(&mut watch), [] as [&str; 0]);

        // Its process ending is a change, though its file is as it was.
        running.set(false);
        assert_eq!(said(&mut watch), ["a running=false"]);

        std::fs::write(agents.join("b.json"), r#"{"state":"failed","detail":"x"}"#).unwrap();
        std::fs::remove_file(agents.join("a.json")).unwrap();
        assert_eq!(said(&mut watch), ["Agents a gone", "b running=false"]);

        // A folder that is removed takes its entries with it.
        std::fs::remove_dir_all(&agents).unwrap();
        assert_eq!(said(&mut watch), ["Agents b gone"]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_log_is_sent_from_its_end_and_then_as_it_grows() {
        let root = folder("log");
        let log = root.join("build.log");
        let entry = serde_json::json!({ "state": "running", "log": log }).to_string();
        std::fs::write(root.join("Agents").join("a.json"), entry).unwrap();
        let mut watch = Watch::default();
        let logs = |watch: &mut Watch| -> Vec<(bool, String)> {
            let messages = watch.pass(&root, &|_| false);
            let logs = messages.into_iter().filter_map(|message| match message {
                Message::Log { start, text, .. } => Some((start, text)),
                _ => None,
            });
            logs.collect()
        };
        // No log yet: nothing to say of it.
        assert_eq!(logs(&mut watch), []);

        // More than is sent of a log: its end, from the start of a line.
        let long = "x".repeat(LOG_TAIL_BYTES as usize);
        std::fs::write(&log, format!("{long}\nsecond\nthird\n")).unwrap();
        let first = logs(&mut watch);
        assert_eq!(first, [(true, "second\nthird\n".to_owned())]);
        assert_eq!(logs(&mut watch), []);

        // What is added, holding back a character that is not all there yet.
        let mut file = std::fs::OpenOptions::new().append(true).open(&log).unwrap();
        file.write_all("fourth \u{e9}".as_bytes().split_last().unwrap().1).unwrap();
        assert_eq!(logs(&mut watch), [(false, "fourth ".to_owned())]);
        assert_eq!(logs(&mut watch), []);
        file.write_all(&"\u{e9}".as_bytes()[1..]).unwrap();
        file.write_all(b"\n").unwrap();
        assert_eq!(logs(&mut watch), [(false, "\u{e9}\n".to_owned())]);

        // Written anew and shorter: sent from its start.
        drop(file);
        std::fs::write(&log, "again\n").unwrap();
        assert_eq!(logs(&mut watch), [(true, "again\n".to_owned())]);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
