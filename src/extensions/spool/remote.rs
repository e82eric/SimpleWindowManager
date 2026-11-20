//! The mailbox folders of other machines, listed in the mailboxes of the same
//! names here. For each machine SWM keeps an `ssh` running that runs
//! `swm-mailbox watch` there (`spool::watch`) and reads what it prints.
//!
//! ```ignore
//! let remotes = spool::Remotes::new().machine("vm", "me@192.168.56.10");
//! spool::register_with(configuration, SpoolOptions::named("Agents").remotes(&remotes));
//! spool::register_with(configuration, SpoolOptions::named("Builds").remotes(&remotes));
//! ```
//!
//! SWM only connects out, and only to the machines the configuration names. The
//! `ssh` runs as the desktop user, asks nothing and forwards nothing: the
//! machine must take a key or an agent, and its host key must already be known.
//!
//! What a machine says is text to show and nothing else. An entry's process is
//! never looked for here: the machine says whether it runs. Its log is never
//! opened where the entry says: what the machine sends of it is kept in a file
//! of SWM's, `%LOCALAPPDATA%\swm\remote\<machine>\<mailbox>\<id>.log`.
use super::watch::{Message, VERSION};
use super::{Entry, LOG_TAIL_BYTES, is_id};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, UNIX_EPOCH};

/// Where `swm-mailbox` is on a machine that was not given another place.
/// Its `cmd.exe` reads the variable, as the shell `ssh` runs a command with.
const PROGRAM: &str = r"%LOCALAPPDATA%\swm\bin\swm-mailbox.exe";
/// How long after a connection ends the next is tried: this, then twice as
/// long each time, up to `LONGEST_WAIT`.
const FIRST_WAIT: Duration = Duration::from_secs(5);
const LONGEST_WAIT: Duration = Duration::from_secs(60);
/// A line longer than this is not a message, and ends the connection: a log's
/// text is at most `LOG_TAIL_BYTES`, which JSON can write six times as long.
const MAX_LINE_BYTES: u64 = 8 * LOG_TAIL_BYTES;
/// Lines that are not messages before the first one, such as a banner.
const MAX_LINES_BEFORE: usize = 50;
/// How many of the last lines that were not messages a problem's report has.
const SAID_LINES: usize = 3;
/// Entries kept of one machine's mailbox; any more are ignored.
const MAX_ENTRIES: usize = 200;
/// A copy of a log that has grown past this is cut to its end again.
const MAX_LOG_BYTES: u64 = 4 * LOG_TAIL_BYTES;

/// The machines whose mailbox folders are listed too: see `SpoolOptions::remotes`.
#[derive(Clone, Default)]
pub struct Remotes(Vec<Arc<Machine>>);
impl std::fmt::Debug for Remotes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names = self.0.iter().map(|machine| &machine.name);
        f.debug_list().entries(names).finish()
    }
}
impl Remotes {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a machine. `name` is what its entries are listed under, and
    /// `address` what `ssh` is given to find it: `user@host`, or a `Host` of the
    /// ssh configuration. `swm-mailbox.exe` is looked for there in
    /// `%LOCALAPPDATA%\swm\bin`, where `mailbox\Deploy-SwmMailbox.ps1` puts it.
    pub fn machine(self, name: &str, address: &str) -> Self {
        self.machine_with(name, address, PROGRAM)
    }

    /// `machine`, with `swm-mailbox.exe` at `program` on that machine.
    pub fn machine_with(mut self, name: &str, address: &str, program: &str) -> Self {
        self.0.retain(|machine| machine.name != name);
        self.0.push(Arc::new(Machine {
            name: name.to_owned(),
            address: address.to_owned(),
            program: program.to_owned(),
            started: Once::new(),
            shown: Mutex::new(Shown::default()),
        }));
        self
    }

    /// The entries the machines have in their folders called `mailbox`, as
    /// entries of this machine's mailbox. Connects to each machine the first
    /// time it is asked.
    pub(super) fn entries(&self, mailbox: &str) -> Vec<Entry> {
        let mut entries = Vec::new();
        for machine in &self.0 {
            machine.start();
            let shown = machine.shown.lock().unwrap();
            let logs = log_folder(&machine.name, mailbox);
            let listed = shown.mailboxes.get(mailbox).into_iter().flatten();
            entries.extend(listed.filter_map(|(id, said)| {
                let log = logs.as_ref().map(|logs| logs.join(format!("{id}.log")));
                said.entry(&machine.name, id, log, shown.lost.as_deref())
            }));
        }
        entries
    }
}

/// One machine, and what it last said.
struct Machine {
    name: String,
    address: String,
    program: String,
    started: Once,
    shown: Mutex<Shown>,
}

/// What a machine's folders held when it last said, by mailbox and entry.
#[derive(Default)]
struct Shown {
    mailboxes: Mailboxes,
    /// Why the last connection ended, until the machine is heard from again.
    problem: Option<String>,
    /// When the connection was lost, as the time of day, if it is.
    lost: Option<String>,
}
type Mailboxes = HashMap<String, HashMap<String, Said>>;

/// What a machine said of one entry.
#[derive(Clone, Debug, PartialEq)]
struct Said {
    text: String,
    written: u64,
    running: bool,
}
impl Said {
    /// The entry as one of this machine's mailbox: named with its machine, its
    /// process what the machine said of it, and its log the copy at `log`, if
    /// there is one. `lost` is since when the machine has not been heard from.
    fn entry(
        &self,
        machine: &str,
        id: &str,
        log: Option<PathBuf>,
        lost: Option<&str>,
    ) -> Option<Entry> {
        let written = UNIX_EPOCH + Duration::from_millis(self.written);
        let mut entry = Entry::parse(&format!("{machine}:{id}"), &self.text, written)?;
        entry.source = match entry.source.as_str() {
            "" => machine.to_owned(),
            source => format!("{machine} · {source}"),
        };
        // Only whether it names a process is the entry's to say.
        entry.started = None;
        entry.elsewhere = Some(self.running);
        entry.log = log.filter(|log| log.is_file());
        if let Some(lost) = lost
            && entry.pid.is_some()
            && self.running
        {
            entry.detail = format!("{machine} not connected since {lost}");
        }
        Some(entry)
    }
}

impl Machine {
    /// Starts the thread that keeps a connection to the machine, once.
    fn start(self: &Arc<Self>) {
        self.started.call_once(|| {
            let machine = self.clone();
            let name = format!("mailboxes of {}", self.name);
            let spawned = std::thread::Builder::new()
                .name(name.clone())
                .spawn(move || machine.keep_connected());
            if let Err(error) = spawned {
                crate::error_log::report(name, error.to_string());
            }
        });
    }

    fn keep_connected(&self) {
        let context = format!("mailboxes of {}", self.name);
        let mut wait = FIRST_WAIT;
        // Said once for each time the machine goes from heard to not heard.
        let mut reported = false;
        loop {
            let (heard, problem) = self.connect();
            if heard {
                wait = FIRST_WAIT;
                reported = false;
                self.shown.lock().unwrap().lost = Some(time_of_day());
            }
            self.shown.lock().unwrap().problem = Some(problem.clone());
            if !reported {
                reported = true;
                crate::error_log::report(&context, problem);
            }
            std::thread::sleep(wait);
            wait = (wait * 2).min(LONGEST_WAIT);
        }
    }

    /// What `ssh` is given: it asks nothing, as nobody would see the question,
    /// forwards nothing, and ends by itself when the machine stops answering.
    fn ssh_arguments(&self) -> Result<Vec<String>, String> {
        let address = self.address.as_str();
        if address.is_empty() || address.starts_with('-') || address.contains(char::is_whitespace) {
            return Err(format!(
                "address {address:?} must be one word, as ssh takes it"
            ));
        }
        if self.program.contains('"') {
            return Err(format!(
                "program {:?} must have no quote in it",
                self.program
            ));
        }
        let mut arguments = vec!["-T".to_owned()];
        for option in [
            "BatchMode=yes",
            "StrictHostKeyChecking=yes",
            "ForwardAgent=no",
            "ForwardX11=no",
            "ClearAllForwardings=yes",
            "ServerAliveInterval=15",
            "ServerAliveCountMax=3",
        ] {
            arguments.extend(["-o".to_owned(), option.to_owned()]);
        }
        // `--`: what follows is the machine, whatever it looks like.
        arguments.extend(["--".to_owned(), address.to_owned()]);
        arguments.push(format!("\"{}\" watch", self.program));
        Ok(arguments)
    }

    /// One connection, from its start to its end: whether the machine was
    /// heard from, and why it ended.
    fn connect(&self) -> (bool, String) {
        let arguments = match self.ssh_arguments() {
            Ok(arguments) => arguments,
            Err(problem) => return (false, problem),
        };
        let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
        let ssh = match crate::process::run_piped_as_desktop_user("ssh", &arguments) {
            Ok(ssh) => ssh,
            Err(problem) => return (false, format!("starting ssh: {problem}")),
        };
        let mut heard = Heard::default();
        let mut lines = BufReader::new(&ssh.output);
        let mut heard_from = false;
        let problem = loop {
            let line = match read_line(&mut lines) {
                Ok(Some(line)) => line,
                Ok(None) => break "the connection ended".to_owned(),
                Err(problem) => break problem,
            };
            let logs = |mailbox: &str| log_folder(&self.name, mailbox);
            match heard.take(&line, &logs) {
                Ok(true) => {
                    // What it holds is how the machine's folders are now.
                    heard_from = true;
                    let mut shown = self.shown.lock().unwrap();
                    shown.lost = None;
                    shown.problem = None;
                    if shown.mailboxes != heard.mailboxes {
                        shown.mailboxes = heard.mailboxes.clone();
                    }
                }
                Ok(false) => {}
                Err(problem) => break problem,
            }
        };
        let problem = match heard.said.is_empty() {
            true => problem,
            false => format!("{problem}; it said: {}", heard.said.join(" | ")),
        };
        (heard_from, problem)
    }
}

/// The next line, without its end; `None` at the end of the output.
fn read_line(mut lines: impl BufRead) -> Result<Option<String>, String> {
    let mut line = Vec::new();
    let read = lines
        .by_ref()
        .take(MAX_LINE_BYTES + 1)
        .read_until(b'\n', &mut line);
    match read {
        Ok(0) => Ok(None),
        Ok(_) if line.len() as u64 > MAX_LINE_BYTES => {
            Err("a line too long to be a message".into())
        }
        Ok(_) => Ok(Some(String::from_utf8_lossy(&line).trim_end().to_owned())),
        Err(error) => Err(format!("reading: {error}")),
    }
}

/// What one connection has said so far.
#[derive(Default)]
struct Heard {
    /// Whether it has said which version its messages are, and it is ours.
    greeted: bool,
    mailboxes: Mailboxes,
    /// The last lines that were not messages: what `ssh` or the machine said.
    said: Vec<String>,
    /// Lines before the first message.
    before: usize,
    /// Whether everything the folders held has been said once.
    passed: bool,
}
impl Heard {
    /// Takes a line. `Ok(true)` when what has been taken up to here is how the
    /// machine's folders are; `Err` when the connection is of no use. `logs`
    /// gives the folder a mailbox's logs are kept in.
    fn take(&mut self, line: &str, logs: &dyn Fn(&str) -> Option<PathBuf>) -> Result<bool, String> {
        let Some(message) = Message::parse(line) else {
            if !self.greeted {
                self.before += 1;
                if self.before > MAX_LINES_BEFORE {
                    return Err("it is not swm-mailbox that answers".into());
                }
            }
            if !line.trim().is_empty() {
                self.said.push(line.chars().take(200).collect());
                if self.said.len() > SAID_LINES {
                    self.said.remove(0);
                }
            }
            return Ok(false);
        };
        match message {
            Message::Hello { version } if version == VERSION => self.greeted = true,
            Message::Hello { version } => {
                let which = if version < VERSION {
                    "an older"
                } else {
                    "a newer"
                };
                return Err(format!(
                    "it has {which} swm-mailbox (version {version}, this SWM reads {VERSION}): \
                     run mailbox\\Deploy-SwmMailbox.ps1 for it, or update this SWM"
                ));
            }
            _ if !self.greeted => return Err("it did not say its version first".into()),
            Message::Entry {
                mailbox,
                id,
                text,
                written,
                running,
            } => {
                let entries = self.mailboxes.entry(mailbox).or_default();
                if entries.len() < MAX_ENTRIES || entries.contains_key(&id) {
                    let said = Said {
                        text,
                        written,
                        running,
                    };
                    entries.insert(id, said);
                }
            }
            Message::Gone { mailbox, id } => {
                if let Some(entries) = self.mailboxes.get_mut(&mailbox) {
                    entries.remove(&id);
                }
                if let Some(folder) = logs(&mailbox) {
                    as_user(|| drop(std::fs::remove_file(folder.join(format!("{id}.log")))));
                }
            }
            Message::Log {
                mailbox,
                id,
                start,
                text,
            } => {
                let listed = |entries: &HashMap<String, Said>| entries.contains_key(&id);
                if self.mailboxes.get(&mailbox).is_some_and(listed)
                    && let Some(folder) = logs(&mailbox)
                {
                    // A log that cannot be kept is one that cannot be opened.
                    let _ = keep_log(&folder.join(format!("{id}.log")), start, &text);
                }
            }
            Message::Pass => {
                if !self.passed {
                    self.passed = true;
                    self.forget_other_logs(logs);
                }
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Removes the logs kept from an earlier connection of entries this one
    /// did not name: they went while the machine was not heard from.
    fn forget_other_logs(&self, logs: &dyn Fn(&str) -> Option<PathBuf>) {
        // Every folder of the machine's, also of a mailbox it no longer has.
        let Some(machine) = logs("x").and_then(|folder| Some(folder.parent()?.to_owned())) else {
            return;
        };
        as_user(|| forget_logs(&machine, &self.mailboxes));
    }
}

/// Removes the logs in the folders of `machine` of entries not in `mailboxes`.
fn forget_logs(machine: &Path, mailboxes: &Mailboxes) {
    {
        for folder in std::fs::read_dir(machine).into_iter().flatten().flatten() {
            let mailbox = folder.file_name();
            let entries = mailbox.to_str().and_then(|mailbox| mailboxes.get(mailbox));
            for log in std::fs::read_dir(folder.path())
                .into_iter()
                .flatten()
                .flatten()
            {
                let path = log.path();
                let id = path.file_stem().and_then(|id| id.to_str());
                let listed = id.is_some_and(|id| entries.is_some_and(|e| e.contains_key(id)));
                if !listed && path.extension().is_some_and(|e| e == "log") {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }
}

/// Where the logs a machine sends of a mailbox's entries are kept.
fn log_folder(machine: &str, mailbox: &str) -> Option<PathBuf> {
    (is_id(machine) && is_id(mailbox)).then_some(())?;
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(
        Path::new(&local)
            .join("swm")
            .join("remote")
            .join(machine)
            .join(mailbox),
    )
}

/// Writes `text` as the log at `path`, or adds it to the end. The file is the
/// desktop user's, written as them, as their folder is.
fn keep_log(path: &Path, start: bool, text: &str) -> Result<(), String> {
    let write = || -> std::io::Result<()> {
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let mut options = std::fs::OpenOptions::new();
        if start {
            options.write(true).create(true).truncate(true);
        } else {
            options.append(true).create(true);
        }
        let mut file = options.open(path)?;
        file.write_all(text.as_bytes())?;
        // Kept to a size: only the end of a log is shown.
        if file.metadata()?.len() > MAX_LOG_BYTES {
            drop(file);
            let end = super::tail(path, LOG_TAIL_BYTES)?;
            std::fs::write(path, end)?;
        }
        Ok(())
    };
    as_user(write)
        .ok_or("the desktop user could not be acted as")?
        .map_err(|error| error.to_string())
}

/// Runs `f` as the desktop user when SWM runs elevated, so that what it writes
/// and removes is theirs. `None` if it could not be run as them.
fn as_user<T>(f: impl FnOnce() -> T) -> Option<T> {
    match crate::process::is_elevated() {
        true => crate::process::as_desktop_user(f).ok(),
        false => Some(f()),
    }
}

/// Connects to a machine as SWM does and prints what its mailboxes list,
/// and again each time that changes, until `out` can no longer be written to:
/// `swm-mailbox show`, to see why a machine lists what it does or nothing.
pub fn show(
    name: &str,
    address: &str,
    program: Option<&str>,
    out: &mut impl Write,
) -> std::io::Result<()> {
    let remotes = Remotes::new().machine_with(name, address, program.unwrap_or(PROGRAM));
    let machine = remotes.0[0].clone();
    let mut printed = String::new();
    loop {
        machine.start();
        let (mut names, problem, lost) = {
            let shown = machine.shown.lock().unwrap();
            let names: Vec<String> = shown.mailboxes.keys().cloned().collect();
            (names, shown.problem.clone(), shown.lost.clone())
        };
        names.sort();
        let mut text = match (&problem, &lost) {
            (Some(problem), _) => format!(
                "{name}: not connected: {problem}
"
            ),
            (None, Some(lost)) => format!(
                "{name}: not connected since {lost}
"
            ),
            (None, None) if names.is_empty() => format!(
                "{name}: nothing heard yet
"
            ),
            (None, None) => format!(
                "{name}: connected
"
            ),
        };
        for mailbox in names {
            let options = super::SpoolOptions::named(&mailbox);
            let entries = remotes.entries(&mailbox);
            let logs: HashMap<String, bool> = entries
                .iter()
                .map(|entry| (entry.id.clone(), entry.log.is_some()))
                .collect();
            let state = super::Tracker::default().update(
                entries,
                std::time::SystemTime::now(),
                options.keep_for,
                Entry::process_is_running,
            );
            text += &format!(
                "{mailbox}: {} listed
",
                state.entries.len()
            );
            for entry in state.entries {
                let log = if logs.get(&entry.id) == Some(&true) {
                    "  [log]"
                } else {
                    ""
                };
                text += &format!(
                    "  {:<8} {}  {}  {}{log}
",
                    entry.state, entry.source, entry.title, entry.detail
                );
            }
        }
        if text != printed {
            writeln!(out, "{text}")?;
            out.flush()?;
            printed = text;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// The time of day here, as `14:02`.
fn time_of_day() -> String {
    let mut time: windows_sys::Win32::Foundation::SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe { windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut time) };
    format!("{:02}:{:02}", time.wHour, time.wMinute)
}

#[cfg(test)]
mod tests {
    use super::super::watch::Watch;
    use super::super::{Files, SpoolOptions, Tracker};
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("swm-remote-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn machine(address: &str, program: &str) -> Arc<Machine> {
        Remotes::new().machine_with("vm", address, program).0[0].clone()
    }

    #[test]
    fn the_ssh_asks_nothing_forwards_nothing_and_runs_only_the_watch() {
        let arguments = machine("me@host", PROGRAM).ssh_arguments().unwrap();
        let options: Vec<&str> = arguments
            .iter()
            .zip(arguments.iter().skip(1))
            .filter(|(flag, _)| *flag == "-o")
            .map(|(_, option)| option.as_str())
            .collect();
        for option in [
            "BatchMode=yes",
            "StrictHostKeyChecking=yes",
            "ClearAllForwardings=yes",
        ] {
            assert!(options.contains(&option), "{option}");
        }
        let end: Vec<&str> = arguments.iter().rev().take(3).map(String::as_str).collect();
        let watch = r#""%LOCALAPPDATA%\swm\bin\swm-mailbox.exe" watch"#;
        assert_eq!(end, [watch, "me@host", "--"]);

        // Nothing ssh would read as an option or as two words, and no program
        // that ends its own quoting.
        for address in ["", "-oProxyCommand=calc", "me@host calc"] {
            assert!(
                machine(address, PROGRAM).ssh_arguments().is_err(),
                "{address}"
            );
        }
        assert!(
            machine("me@host", "a\" & calc & \"")
                .ssh_arguments()
                .is_err()
        );
    }

    #[test]
    fn a_connection_is_of_use_once_it_says_our_version() {
        let no_logs = |_: &str| None;
        let entry = Message::Entry {
            mailbox: "Agents".into(),
            id: "a".into(),
            text: r#"{"state":"done"}"#.into(),
            written: 1,
            running: false,
        };
        let hello = |version| Message::Hello { version }.line();

        let mut heard = Heard::default();
        assert_eq!(heard.take("Welcome to the machine", &no_logs), Ok(false));
        assert_eq!(heard.take(&hello(VERSION), &no_logs), Ok(false));
        assert_eq!(heard.take(&entry.line(), &no_logs), Ok(false));
        assert_eq!(heard.take(&Message::Pass.line(), &no_logs), Ok(true));
        assert_eq!(heard.mailboxes["Agents"].len(), 1);
        assert_eq!(heard.said, ["Welcome to the machine"]);

        // Another version shows nothing, and says which side to update.
        let older = Heard::default()
            .take(&hello(VERSION - 1), &no_logs)
            .unwrap_err();
        assert!(
            older.contains("an older") && older.contains("Deploy-SwmMailbox"),
            "{older}"
        );
        let newer = Heard::default()
            .take(&hello(VERSION + 1), &no_logs)
            .unwrap_err();
        assert!(newer.contains("a newer"), "{newer}");
        // Nor does one that never says.
        assert!(Heard::default().take(&entry.line(), &no_logs).is_err());
        let mut other = Heard::default();
        let lines = (0..=MAX_LINES_BEFORE).map(|_| other.take("something else", &no_logs));
        assert!(lines.last().unwrap().is_err());
        assert_eq!(other.said.len(), SAID_LINES);
    }

    #[test]
    fn an_entry_of_a_machine_is_named_with_it_and_trusts_only_its_text() {
        let said = |text: &str, running| Said {
            text: text.to_owned(),
            written: 1_700_000_000_000,
            running,
        };
        let root = folder("entry");
        let copy = root.join("a.log");
        std::fs::write(&copy, "copied").unwrap();

        // The process and the log the entry names are of the other machine.
        let text = r#"{"state":"running","source":"claude · src/x","pid":4,"started":"7",
                       "log":"C:\\Windows\\win.ini","time":1700000000}"#;
        let entry = said(text, true)
            .entry("vm", "a", Some(copy.clone()), None)
            .unwrap();
        assert_eq!(entry.id, "vm:a");
        assert_eq!(entry.source, "vm · claude · src/x");
        assert_eq!(entry.log.as_deref(), Some(copy.as_path()));
        assert_eq!((entry.started, entry.elsewhere), (None, Some(true)));
        assert!(entry.process_is_running());
        // Its process is gone when the machine says so, whatever runs here.
        let own = format!(r#"{{"state":"running","pid":{}}}"#, std::process::id());
        let stopped = said(&own, false).entry("vm", "a", None, None).unwrap();
        assert!(!stopped.process_is_running());
        // No copy of its log: nothing to open.
        let missing = Some(root.join("none.log"));
        assert_eq!(
            said(text, true)
                .entry("vm", "a", missing, None)
                .unwrap()
                .log,
            None
        );

        // Not heard from: what was running says since when, and nothing else does.
        let lost = said(text, true)
            .entry("vm", "a", None, Some("14:02"))
            .unwrap();
        assert_eq!(lost.detail, "vm not connected since 14:02");
        assert!(lost.process_is_running());
        let done = r#"{"state":"done","detail":"All tests pass"}"#;
        let done = said(done, false)
            .entry("vm", "b", None, Some("14:02"))
            .unwrap();
        assert_eq!(
            (done.source.as_str(), done.detail.as_str()),
            ("vm", "All tests pass")
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn logs_are_kept_as_sent_and_removed_with_their_entries() {
        let root = folder("logs");
        let logs = |mailbox: &str| Some(root.join("vm").join(mailbox));
        let log = |id: &str, start, text: &str| {
            Message::Log {
                mailbox: "Agents".into(),
                id: id.into(),
                start,
                text: text.into(),
            }
            .line()
        };
        let entry = |id: &str| {
            Message::Entry {
                mailbox: "Agents".into(),
                id: id.into(),
                text: r#"{"state":"done"}"#.into(),
                written: 1,
                running: false,
            }
            .line()
        };
        // Left by an earlier connection, of an entry that has gone since.
        let agents = root.join("vm").join("Agents");
        std::fs::create_dir_all(&agents).unwrap();
        std::fs::write(agents.join("old.log"), "old").unwrap();

        let mut heard = Heard::default();
        let hello = Message::Hello { version: VERSION }.line();
        for line in [
            hello,
            entry("a"),
            log("a", true, "one\n"),
            log("a", false, "two\n"),
        ] {
            heard.take(&line, &logs).unwrap();
        }
        // Of an entry that was never named: not kept.
        heard.take(&log("stray", true, "x"), &logs).unwrap();
        assert!(agents.join("old.log").exists());
        heard.take(&Message::Pass.line(), &logs).unwrap();
        let kept = |id: &str| std::fs::read_to_string(agents.join(format!("{id}.log"))).ok();
        assert_eq!(kept("a").as_deref(), Some("one\ntwo\n"));
        assert_eq!((kept("stray"), kept("old")), (None, None));

        heard.take(&log("a", true, "again\n"), &logs).unwrap();
        assert_eq!(kept("a").as_deref(), Some("again\n"));
        let gone = Message::Gone {
            mailbox: "Agents".into(),
            id: "a".into(),
        };
        heard.take(&gone.line(), &logs).unwrap();
        assert_eq!(kept("a"), None);
        assert!(heard.mailboxes["Agents"].is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A folder read here, and the same folder read by a watch and heard
    /// through its messages, list the same entries.
    #[test]
    fn a_folder_lists_the_same_read_here_or_heard_from_a_watch() {
        let root = folder("same");
        let agents = root.join("Agents");
        std::fs::create_dir_all(&agents).unwrap();
        let write = |id: &str, json: &str| {
            std::fs::write(agents.join(format!("{id}.json")), json).unwrap();
        };
        write(
            "running",
            r#"{"state":"running","title":"a","time":1700000000,"pid":11,"kind":"claude","resume":"claude --resume 1"}"#,
        );
        write(
            "stopped",
            r#"{"state":"running","title":"b","time":1700000050,"pid":12,"source":"nterm"}"#,
        );
        write(
            "done",
            r#"{"state":"done","title":"c","time":1700000100,"detail":"ok"}"#,
        );
        write("old", r#"{"state":"done","title":"d","time":1000000000}"#);
        write(
            "early",
            r#"{"state":"running","title":"e","time":1700000199,"pid":11,"delay":60}"#,
        );
        write("broken", "{");
        let is_running = |entry: &Entry| entry.pid == Some(11);
        let now = UNIX_EPOCH + Duration::from_secs(1_700_000_200);
        let options = SpoolOptions::named("Agents");
        let list = |entries: Vec<Entry>, running: &dyn Fn(&Entry) -> bool| {
            Tracker::default().update(entries, now, options.keep_for, running)
        };

        let here = list(Files::default().read(&agents), &is_running);

        let no_logs = |_: &str| None;
        let mut heard = Heard::default();
        heard
            .take(&Message::Hello { version: VERSION }.line(), &no_logs)
            .unwrap();
        for message in Watch::default().pass(&root, &is_running) {
            heard.take(&message.line(), &no_logs).unwrap();
        }
        assert_eq!(heard.take(&Message::Pass.line(), &no_logs), Ok(true));
        let entries = heard.mailboxes["Agents"].iter();
        let entries = entries.filter_map(|(id, said)| said.entry("vm", id, None, None));
        let mut there = list(entries.collect(), &Entry::process_is_running);

        assert_eq!(here.entries.len(), 3);
        // The same, but for the machine's name on each.
        for entry in &mut there.entries {
            entry.id = entry.id.strip_prefix("vm:").unwrap().to_owned();
            entry.source = match entry.source.as_str() {
                "vm" => String::new(),
                source => source.strip_prefix("vm · ").unwrap().to_owned(),
            };
        }
        assert_eq!(there, here);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
