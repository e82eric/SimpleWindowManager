//! What agents' sessions share as a kind of entry in a mailbox folder
//! (`extensions::spool`): a command that copies the one resuming a session, and
//! a note on its card of whether the agent still has it open. The hooks in
//! `mailbox\agent-hook.ps1` write both for Claude Code and Codex, as text the
//! folder keeps for the entry's kind without reading it.
use crate::extensions::spool::{self, Kind, Note};
use crate::mailbox::MailboxEntry;
use nfm_egui::egui::Color32;

/// The entry's field with the command line that opens its session again.
const RESUME: &str = "resume";
/// The entry's field naming the agent's process, as `spool::running_process`
/// reads one.
const ATTACHED: &str = "attached";
/// The color of `attached` on a card.
const ATTACHED_COLOR: Color32 = Color32::from_rgb(0xb8, 0xbb, 0x26);

/// `kind` as an agent's sessions: its drop-down's command palette lists **Copy
/// resume command**, which copies the selected session's; SWM never runs it.
/// With `show_attached`, each session's card says whether it is `attached`,
/// with the agent's process ID, or `not attached`: whether the agent that
/// reported it is still running.
pub fn kind(kind: Kind, show_attached: bool) -> Kind {
    let kind = kind.command("Copy resume command", |entry, drop_down| {
        if let Some(resume) = entry.field(RESUME) {
            drop_down.copy(resume);
        }
    });
    if show_attached {
        kind.note(|entry| Some(attached(entry, spool::running_process)))
    } else {
        kind
    }
}

/// Whether the session is attached; `running` gives the ID of the process a
/// field names, if it still runs.
fn attached(entry: &MailboxEntry, running: impl Fn(&str) -> Option<u32>) -> Note {
    match entry.field(ATTACHED).and_then(running) {
        Some(pid) => Note::colored(format!("attached {pid}"), ATTACHED_COLOR),
        None => Note::plain("not attached"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_says_whether_its_agent_still_runs() {
        let session = |attached: Option<&str>| MailboxEntry {
            id: "claude-a".into(),
            state: "done".into(),
            source: String::new(),
            title: String::new(),
            detail: String::new(),
            time: std::time::UNIX_EPOCH,
            fields: attached
                .map(|attached| (ATTACHED.to_owned(), attached.to_owned()))
                .into_iter()
                .collect(),
        };
        let running = |reference: &str| (reference == "40472:7").then_some(40472);
        assert_eq!(
            attached(&session(Some("40472:7")), running),
            Note::colored("attached 40472", ATTACHED_COLOR)
        );
        // A process that is gone, and a session that names none.
        let detached = Note::plain("not attached");
        assert_eq!(attached(&session(Some("1:7")), running), detached);
        assert_eq!(attached(&session(None), running), detached);
    }
}
