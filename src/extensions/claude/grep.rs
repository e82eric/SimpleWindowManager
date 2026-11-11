//! Search every Claude Code conversation for some text, then fuzzy-find among
//! the messages that contain it. `GrepClaudeSessions` in the commands picker
//! asks for the text and opens it, once the configuration adds
//! `claude::grep_command(options)` with `Configuration::picker_command`.
//!
//! The search reads each transcript under `projects`, newest first, on a few
//! threads, and lists matches as it finds them: a row per prompt or reply
//! containing the text, ignoring ASCII case. The fuzzy query then filters those
//! rows. The preview shows the whole message with the matching line marked, and
//! Enter opens the message's conversation on top; Esc there comes back to the
//! matches, and Esc here to the search text.
use super::transcript::{self, ClaudeOptions};
use crate::config::{CommandArgument, PickerCommand};
use crate::markdown_document::{BOLD, DocumentWriter, line_range};
use crate::picker::{Outcome, PickerHost, PickerRequest, PickerView};
use nfm_egui::shared::{self, SharedItem, SharedPicker};
use nfm_egui::styled_document::{DocumentPreviewProvider, StyledDocument};
use nfm_egui::{Appearance, PickerConfig, PickerEvent, Placement, egui};
use nfm_picker_sources::structured::{
    StructuredSchema, StructuredStreamingSnapshot, StructuredStreamingStore,
};
use nfm_search_core::snapshot_store::SnapshotStore;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The command's name, as the commands picker lists it.
pub const COMMAND: &str = "GrepClaudeSessions";

const COLUMNS: [&str; 4] = ["Time", "Project", "Speaker", "Text"];
/// The longest a row's text gets, in characters.
const SNIPPET_CHARS: usize = 160;
/// Characters of the matching line shown before the match when it is cut.
const SNIPPET_LEAD: usize = 40;
/// How often found messages are published to the picker.
const PUBLISH_EVERY: Duration = Duration::from_millis(100);
/// The most transcripts read at once.
const MAX_THREADS: usize = 8;

/// The command, for `Configuration::picker_command`.
pub fn command(options: ClaudeOptions) -> PickerCommand {
    PickerCommand {
        name: COMMAND.into(),
        description: "Search Claude conversations, then fuzzy-find the matches".into(),
        argument: CommandArgument {
            name: "search text".into(),
            description: "Found in prompts and replies, ignoring ASCII case".into(),
            required: true,
        },
        run: Arc::new(move |text| request(text, options.clone())),
    }
}

/// The picker searching for `text`.
fn request(text: &str, options: ClaudeOptions) -> Result<Box<dyn PickerRequest>, String> {
    let dir = claude_dir().ok_or("no Claude folder: USERPROFILE is not set")?;
    Ok(Box::new(Grep {
        dir,
        text: text.to_owned(),
        options,
    }))
}

/// Where Claude Code keeps its files: `CLAUDE_CONFIG_DIR`, else `.claude` in
/// the user's folder.
fn claude_dir() -> Option<PathBuf> {
    match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => std::env::var_os("USERPROFILE").map(|home| Path::new(&home).join(".claude")),
    }
}

/// A working directory as its last two folders: `src/nfm` for `G:\src\nfm`.
fn project(cwd: &str) -> String {
    let parts: Vec<_> = cwd
        .split(['\\', '/'])
        .filter(|part| !part.is_empty() && !part.ends_with(':'))
        .collect();
    parts[parts.len().saturating_sub(2)..].join("/")
}

struct Grep {
    /// The Claude folder, holding `projects`.
    dir: PathBuf,
    text: String,
    options: ClaudeOptions,
}

/// A message containing the search text.
#[derive(Clone, Debug, PartialEq)]
struct Hit {
    transcript: PathBuf,
    project: String,
    you: bool,
    /// As the transcript records it, an RFC 3339 time.
    time: String,
    text: String,
}

impl PickerRequest for Grep {
    fn build(self: Box<Self>, host: &PickerHost<'_>) -> Result<Box<dyn PickerView>, String> {
        let Grep { dir, text, options } = *self;
        let schema = schema()?;
        let store = Arc::new(SnapshotStore::new_append_only());
        let mut rows = StructuredStreamingStore::new(schema);
        store.publish(rows.snapshot());
        let hits = Arc::new(Mutex::new(Vec::new()));
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut picker = shared::replace(
            host.ctx,
            store.clone(),
            PickerConfig {
                title: format!("Claude · {text}"),
                preview_visible: true,
                ..host.config()
            },
            None,
        );
        let needle = text.to_ascii_lowercase();
        let preview_hits = hits.clone();
        let you = options.you;
        let claude = options.claude;
        let highlight = options.highlight;
        let preview_needle = needle.clone();
        picker.set_preview_provider(
            host.ctx,
            Arc::new(DocumentPreviewProvider::new(
                move |selection: &nfm_egui::Selection<SharedItem>| {
                    let hit = preview_hits
                        .lock()
                        .unwrap()
                        .get(selection.index)
                        .cloned()
                        .ok_or("the match is gone")?;
                    Ok(message_document(&hit, &preview_needle, you, claude))
                },
                highlight,
            )),
        );
        let search_hits = hits.clone();
        let search_cancelled = cancelled.clone();
        std::thread::Builder::new()
            .name("swm-claude-grep".into())
            .spawn(move || {
                search(&dir, &needle, rows, &store, &search_hits, &search_cancelled);
                store.complete();
            })
            .map_err(|error| error.to_string())?;
        Ok(Box::new(GrepView {
            picker,
            hits,
            cancelled,
            options,
            opening: None,
        }))
    }
}

fn schema() -> Result<StructuredSchema, String> {
    // Ages are worded when shown; the column stays a time for sorts and filters.
    StructuredSchema::new(COLUMNS.map(String::from).to_vec())?
        .with_types("Time:datetime")?
        .with_default_sort("/!Descending==Time")?
        .with_formatter("Time", |time| {
            nfm_picker_sources::value::parse_timestamp(time)
                .and_then(|time| u64::try_from(time.timestamp()).ok())
                .map_or_else(
                    || time.to_owned(),
                    |seconds| {
                        crate::mailbox::relative_time(
                            UNIX_EPOCH + Duration::from_secs(seconds),
                            SystemTime::now(),
                        )
                    },
                )
        })
}

/// Read every transcript, newest first, adding a row to `rows` and a hit to
/// `hits` for each message containing `needle`, and publishing to `store` as
/// they come.
fn search(
    dir: &Path,
    needle: &str,
    mut rows: StructuredStreamingStore,
    store: &SnapshotStore<StructuredStreamingSnapshot>,
    hits: &Mutex<Vec<Hit>>,
    cancelled: &AtomicBool,
) {
    let files = transcripts(dir);
    let next = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism()
        .map_or(4, |count| count.get())
        .clamp(1, MAX_THREADS);
    let (sender, found) = mpsc::channel::<Vec<Hit>>();
    std::thread::scope(|scope| {
        for _ in 0..threads {
            let sender = sender.clone();
            let (files, next) = (&files, &next);
            scope.spawn(move || {
                while !cancelled.load(Ordering::Relaxed) {
                    let Some(path) = files.get(next.fetch_add(1, Ordering::Relaxed)) else {
                        break;
                    };
                    // A transcript being written may end mid-line; that line
                    // simply does not parse.
                    let Ok(bytes) = std::fs::read(path) else {
                        continue;
                    };
                    let text = String::from_utf8_lossy(&bytes);
                    let found = hits_in(path, &text, needle);
                    if !found.is_empty() && sender.send(found).is_err() {
                        break;
                    }
                }
            });
        }
        drop(sender);
        let mut published = Instant::now();
        let mut pending = false;
        loop {
            let received = found.recv_timeout(PUBLISH_EVERY);
            if cancelled.load(Ordering::Relaxed) {
                break;
            }
            match received {
                Ok(found) => {
                    let mut hits = hits.lock().unwrap();
                    for hit in found {
                        let record = csv::StringRecord::from(vec![
                            hit.time.clone(),
                            hit.project.clone(),
                            speaker(hit.you).to_owned(),
                            snippet(&hit.text, needle),
                        ]);
                        if rows.add_record(&record).is_ok() {
                            // A row's index in the source is its hit's index.
                            hits.push(hit);
                            pending = true;
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            if pending && published.elapsed() >= PUBLISH_EVERY {
                store.publish(rows.snapshot());
                published = Instant::now();
                pending = false;
            }
        }
        if pending {
            store.publish(rows.snapshot());
        }
    });
}

/// Every conversation's transcript, most recently written first.
fn transcripts(dir: &Path) -> Vec<PathBuf> {
    let Ok(projects) = std::fs::read_dir(dir.join("projects")) else {
        return Vec::new();
    };
    let mut files: Vec<(SystemTime, PathBuf)> = projects
        .flatten()
        .filter_map(|project| std::fs::read_dir(project.path()).ok())
        .flat_map(|entries| entries.flatten())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "jsonl"))
        .map(|path| {
            let modified = std::fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH);
            (modified, path)
        })
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files.into_iter().map(|(_, path)| path).collect()
}

/// The messages in a transcript's `text` that contain `needle`, which is in
/// lowercase. Only lines whose JSON has the text are parsed.
fn hits_in(path: &Path, text: &str, needle: &str) -> Vec<Hit> {
    if needle.is_empty() {
        return Vec::new();
    }
    // As the text appears inside a JSON string, quotes and backslashes escaped.
    let encoded = serde_json::to_string(needle).unwrap_or_default();
    let encoded = &encoded[1..encoded.len() - 1];
    // ASCII lowercasing keeps every byte where it was.
    let lower = text.to_ascii_lowercase();
    let mut hits = Vec::new();
    let mut searched_to = 0;
    for (at, _) in lower.match_indices(encoded) {
        if at < searched_to {
            continue;
        }
        let start = lower[..at].rfind('\n').map_or(0, |index| index + 1);
        let end = lower[at..].find('\n').map_or(lower.len(), |index| at + index);
        searched_to = end;
        if let Some(hit) = message_hit(path, &text[start..end], needle) {
            hits.push(hit);
        }
    }
    hits
}

/// The prompt or reply in a transcript line, if it contains `needle`. Like the
/// conversation picker, subagents' messages, tool calls and results, and
/// Claude's own notes are left out.
fn message_hit(path: &Path, line: &str, needle: &str) -> Option<Hit> {
    let entry: Value = serde_json::from_str(line).ok()?;
    if entry["isSidechain"] == true || entry["isMeta"] == true {
        return None;
    }
    let (you, text) = match entry["type"].as_str()? {
        "user" => (true, transcript::typed_prompt(&entry)?),
        "assistant" => (false, reply(&entry)?),
        _ => return None,
    };
    if !text.to_ascii_lowercase().contains(needle) {
        return None;
    }
    Some(Hit {
        transcript: path.to_owned(),
        project: project(entry["cwd"].as_str().unwrap_or_default()),
        you,
        time: entry["timestamp"].as_str().unwrap_or_default().to_owned(),
        text,
    })
}

/// The text Claude wrote in an `assistant` entry.
fn reply(entry: &Value) -> Option<String> {
    let text = entry["message"]["content"]
        .as_array()?
        .iter()
        .filter(|block| block["type"] == "text")
        .filter_map(|block| block["text"].as_str())
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    (!text.is_empty()).then_some(text)
}

fn speaker(you: bool) -> &'static str {
    if you { "You" } else { "Claude" }
}

/// The first line of `text` containing `needle`, cut to show the match.
fn snippet(text: &str, needle: &str) -> String {
    let (line, at) = text
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .find(needle)
                .map(|at| (line.trim_end(), at))
        })
        .unwrap_or((text.lines().next().unwrap_or_default(), 0));
    let leading = line.len() - line.trim_start().len();
    let line = line.trim_start();
    let at = line[..at.saturating_sub(leading).min(line.len())].chars().count();
    let start = at.saturating_sub(SNIPPET_LEAD);
    let chars = line.chars().count();
    let cut: String = line.chars().skip(start).take(SNIPPET_CHARS).collect();
    let before = if start > 0 { "…" } else { "" };
    let after = if start + SNIPPET_CHARS < chars { "…" } else { "" };
    format!("{before}{cut}{after}")
}

/// The whole message, with its first line containing `needle` to highlight.
fn message_document(
    hit: &Hit,
    needle: &str,
    you: egui::Color32,
    claude: egui::Color32,
) -> (Arc<StyledDocument>, Option<nfm_egui::styled_document::DocumentRange>) {
    let mut out = DocumentWriter::default();
    out.line_of(
        speaker(hit.you),
        nfm_egui::styled_document::Style {
            foreground: Some(if hit.you { you } else { claude }),
            flags: BOLD,
            ..Default::default()
        },
    );
    let mut range = None;
    for line in hit.text.lines() {
        if range.is_none() && line.to_ascii_lowercase().contains(needle) {
            range = Some(line_range(out.line(), line.len()));
        }
        out.line_of(line, Default::default());
    }
    (out.finish(), range)
}

struct GrepView {
    picker: SharedPicker,
    /// Each row's message, in row order.
    hits: Arc<Mutex<Vec<Hit>>>,
    /// Set when the picker closes, to stop the search.
    cancelled: Arc<AtomicBool>,
    options: ClaudeOptions,
    /// The conversation of the match last accepted, while it is being read.
    opening: Option<mpsc::Receiver<Result<Box<dyn PickerRequest>, String>>>,
}
impl Drop for GrepView {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}
impl PickerView for GrepView {
    fn picker(&self) -> Option<&SharedPicker> {
        Some(&self.picker)
    }
    fn has_header(&self) -> bool {
        true
    }
    fn show(
        &mut self,
        ctx: &egui::Context,
        placement: Placement,
        appearance: &Appearance,
        _: bool,
    ) -> Option<Outcome> {
        let event = self.picker.show(ctx, placement, appearance, false).event;
        // The conversation a match opened is read; it goes on top.
        if let Some(opening) = &self.opening {
            match opening.try_recv() {
                Ok(Ok(conversation)) => {
                    self.opening = None;
                    return Some(Outcome::Push(conversation));
                }
                Ok(Err(error)) => {
                    self.opening = None;
                    crate::error_log::report("Claude session", error);
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => self.opening = None,
            }
        }
        match event {
            Some(PickerEvent::Accepted(selection)) => {
                let hit = self.hits.lock().unwrap().get(selection.index).cloned()?;
                self.opening = open(ctx, hit, self.options.clone());
                None
            }
            event => crate::egui_data_menu::copy_or_close(ctx, &mut self.picker, event),
        }
    }
}

/// Read the conversation `hit` is in, to show on top of the matches. Reading
/// a long transcript takes a moment, so it happens on its own thread, which
/// wakes `ctx` when the picker is ready to receive.
fn open(
    ctx: &egui::Context,
    hit: Hit,
    options: ClaudeOptions,
) -> Option<mpsc::Receiver<Result<Box<dyn PickerRequest>, String>>> {
    let (sender, opening) = mpsc::channel();
    let ctx = ctx.clone();
    let spawned = std::thread::Builder::new()
        .name("swm-claude-open".into())
        .spawn(move || {
            let title = if hit.project.is_empty() {
                "Claude".to_owned()
            } else {
                format!("Claude · {}", hit.project)
            };
            let conversation = transcript::picker(&hit.transcript, title, &options)
                .map(|picker| Box::new(picker) as Box<dyn PickerRequest>);
            let _ = sender.send(conversation);
            ctx.request_repaint();
        });
    match spawned {
        Ok(_) => Some(opening),
        Err(error) => {
            crate::error_log::report("Claude session", error.to_string());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRANSCRIPT: &str = concat!(
        r#"{"type":"user","cwd":"G:\\src\\nfm","timestamp":"2026-11-07T10:00:00Z","message":{"content":"Where is the \"Picker\" built?"}}"#,
        "\n",
        r#"{"type":"assistant","cwd":"G:\\src\\nfm","timestamp":"2026-11-07T10:00:05Z","message":{"content":[{"type":"text","text":"Intro\nThe picker is built in show."},{"type":"tool_use","name":"Read","input":{"file_path":"picker.rs"}}]}}"#,
        "\n",
        r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"picker source"}]}}"#,
        "\n",
        r#"{"type":"assistant","isSidechain":true,"message":{"content":[{"type":"text","text":"picker in a subagent"}]}}"#,
        "\n",
        r#"{"type":"user","origin":{"kind":"task-notification"},"message":{"content":"picker task done"}}"#,
        "\n",
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"nothing here"}]}}"#,
    );

    #[test]
    fn prompts_and_replies_containing_the_text_are_hits() {
        let hits = hits_in(Path::new("a.jsonl"), TRANSCRIPT, "picker");
        let found: Vec<_> = hits.iter().map(|hit| (hit.you, hit.text.as_str())).collect();
        assert_eq!(
            found,
            [
                (true, "Where is the \"Picker\" built?"),
                (false, "Intro\nThe picker is built in show."),
            ]
        );
        assert_eq!(hits[0].project, "src/nfm");
        assert_eq!(hits[0].time, "2026-11-07T10:00:00Z");
    }

    #[test]
    fn projects_show_their_last_two_folders() {
        assert_eq!(project(r"G:\src\SimpleWindowManager"), "src/SimpleWindowManager");
        assert_eq!(project(r"G:\"), "");
        assert_eq!(project("/home/me/app"), "me/app");
    }

    #[test]
    fn quotes_in_the_search_text_match_escaped_json() {
        let hits = hits_in(Path::new("a.jsonl"), TRANSCRIPT, "\"picker\" built");
        assert_eq!(hits.len(), 1);
        assert!(hits_in(Path::new("a.jsonl"), TRANSCRIPT, "").is_empty());
        assert!(hits_in(Path::new("a.jsonl"), TRANSCRIPT, "missing").is_empty());
    }

    #[test]
    fn snippets_show_the_matching_line_around_the_match() {
        assert_eq!(snippet("Intro\n  The picker is here  ", "picker"), "The picker is here");
        let long = format!("{}picker{}", "a".repeat(100), "b".repeat(300));
        let cut = snippet(&long, "picker");
        assert!(cut.starts_with('…') && cut.ends_with('…'));
        assert_eq!(cut.chars().count(), SNIPPET_CHARS + 2);
        assert!(cut.contains("picker"));
    }

    #[test]
    fn the_preview_marks_the_first_matching_line() {
        let hit = &hits_in(Path::new("a.jsonl"), TRANSCRIPT, "picker")[1];
        let (_, range) = message_document(hit, "picker", egui::Color32::GREEN, egui::Color32::RED);
        // The speaker, "Intro", then the matching line.
        assert_eq!(range.map(|range| range.start.line), Some(2));
    }

    #[test]
    fn searching_a_folder_publishes_every_hit_newest_transcript_first() {
        let dir = std::env::temp_dir().join(format!("swm-claude-grep-{}", std::process::id()));
        let project = dir.join("projects").join("G--src-nfm");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("one.jsonl"), TRANSCRIPT).unwrap();
        std::fs::write(project.join("notes.txt"), "picker").unwrap();
        let store = SnapshotStore::new_append_only();
        let hits = Mutex::new(Vec::new());
        let rows = StructuredStreamingStore::new(schema().unwrap());
        search(&dir, "picker", rows, &store, &hits, &AtomicBool::new(false));
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(hits.lock().unwrap().len(), 2);
        use nfm_search_core::store::ItemsSource;
        let snapshot = nfm_search_core::fuzzy_search_session::SearchSnapshotProvider::snapshot(&store)
            .unwrap();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot.item(1).unwrap().fields["Speaker"], "Claude");
        assert_eq!(snapshot.item(1).unwrap().fields["Text"], "The picker is built in show.");
    }
}
