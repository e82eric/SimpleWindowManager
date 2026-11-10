#Requires -Version 7
# A hook for Claude Code and Codex that reports each session to an SWM bar
# mailbox: running after a prompt, waiting when the agent asks for permission,
# done when its turn ends. See "Agent hooks" in the README for the settings.
#
#   pwsh -NoProfile -File agent-hook.ps1 claude     (the event's JSON on stdin)
#   pwsh -NoProfile -File agent-hook.ps1 codex
#
# It prints nothing and always exits 0: what a hook prints can be read by the
# agent as an instruction or context, and a failing hook can hold up a turn.
# Plain parameters, with no [Parameter()] attributes: those would make this an
# advanced script, which refuses the stdin PowerShell pipes to it.
param(
    # Which agent calls it, `claude` or `codex`: names the entries and finds the
    # agent's process.
    [string]$Agent = 'claude',
    # The event as an argument rather than on stdin, as Codex's older `notify`
    # setting passes it.
    [string]$Payload,
    [string]$Mailbox = 'Agents'
)

$ErrorActionPreference = 'Stop'
# Taken first: dot-sourcing another script below replaces $input.
$piped = @($input) -join "`n"
try {
    . (Join-Path $PSScriptRoot 'SwmMailbox.ps1')
    if ($Agent -notin 'claude', 'codex') { throw "unknown agent '$Agent'" }

    if (-not $Payload) {
        # PowerShell has already read stdin, as $input, decoding it with the
        # console's input encoding. The agents write UTF-8, so undo that decoding.
        $Payload = $piped
        $console = [Console]::InputEncoding
        if ($console.CodePage -ne 65001) {
            $Payload = [Text.Encoding]::UTF8.GetString($console.GetBytes($Payload))
        }
    }
    $hook = $Payload | ConvertFrom-Json -AsHashtable
    # `notify` names its one event `type`, and the session `thread-id`.
    $name = $hook.hook_event_name ?? $hook.type
    $session = $hook.session_id ?? $hook['thread-id']
    if (-not $name -or -not $session) { exit 0 }
    $id = ConvertTo-SwmMailboxId "$Agent-$session"
    $existing = Get-SwmMailboxEntry $Mailbox $id

    # A line of text for the bar: the first line with anything on it, cut short.
    function Brief([string]$Text, [int]$Length = 300) {
        $line = ($Text -split "`r?`n" | ForEach-Object Trim | Where-Object { $_ } | Select-Object -First 1)
        if (-not $line) { return '' }
        if ($line.Length -gt $Length) { $line.Substring(0, $Length) + '…' } else { $line }
    }

    $state = $null
    $title = $existing.title
    $detail = ''
    switch ($name) {
        'UserPromptSubmit' {
            $state = 'running'
            $title = Brief $hook.prompt
        }
        # A tool ran: the session is working, and a permission it waited for was given.
        { $_ -in 'PreToolUse', 'PostToolUse' } {
            if ($existing.state -eq 'running') { exit 0 }
            $state = 'running'
        }
        'PermissionRequest' {
            $state = 'waiting'
            $detail = Brief "$($hook.tool_name) $($hook.tool_input.description ?? $hook.tool_input.command ?? $hook.tool_input.file_path)"
        }
        'Notification' {
            # The other kinds, such as the prompt sitting idle, are not waiting on anyone.
            if ($hook.notification_type -notin 'permission_prompt', 'elicitation_dialog', 'elicitation_url_dialog', 'agent_needs_input') { exit 0 }
            $state = 'waiting'
            $detail = Brief $hook.message
        }
        { $_ -in 'Stop', 'agent-turn-complete' } {
            $state = 'done'
            $detail = Brief ($hook.last_assistant_message ?? $hook['last-assistant-message'])
            if (-not $title -and $hook['input-messages']) { $title = Brief $hook['input-messages'][-1] }
        }
        'Interrupt' {
            $state = 'done'
            $detail = 'Interrupted'
        }
        'SessionEnd' {
            # A finished turn stays listed; a session closed mid-turn has nothing to show.
            if ($existing -and $existing.state -ne 'done') { Remove-SwmMailboxEntry $Mailbox $id }
            exit 0
        }
        default { exit 0 }
    }

    # The kind is what a mailbox gives each agent its own badges and viewer by.
    $entry = @{ Mailbox = $Mailbox; Id = $id; State = $state; Title = $title ?? 'New session'; Kind = $Agent }
    if ($detail) { $entry.Detail = $detail }
    # Which agent, then the last two folders of where it runs.
    $entry.Source = $Agent
    if ($hook.cwd) {
        $parts = @($hook.cwd -split '[\\/]' | Where-Object { $_ -and $_ -notmatch ':$' })
        if ($parts) { $entry.Source = "$Agent · " + (($parts | Select-Object -Last 2) -join '/') }
    }
    $names = if ($Agent -eq 'claude') { '^(claude|node)$' } else { '^codex' }
    # While a turn runs the entry belongs to the agent's process, so a session that
    # is killed shows as stopped. A finished turn stays after the agent exits.
    $agentId = if ($state -ne 'done') { Get-SwmMailboxAncestor $names }
    if ($agentId) { $entry.ProcessId = $agentId }
    # The session's transcript, which SWM shows as the entry's kind is shown.
    # Codex's is found by the session's id when the event does not give it,
    # once: later events take it from the entry.
    $log = $hook.transcript_path ?? $existing.log
    if (-not $log -and $Agent -eq 'codex') {
        $sessions = Join-Path ($env:CODEX_HOME ?? (Join-Path $HOME '.codex')) 'sessions'
        $log = Get-ChildItem -LiteralPath $sessions -Recurse -File -Filter "rollout-*$session.jsonl" -ErrorAction Ignore |
            Sort-Object LastWriteTimeUtc | Select-Object -Last 1 -ExpandProperty FullName
    }
    if ($log) { $entry.Log = $log }
    Set-SwmMailboxEntry @entry
} catch {
    # Recorded for whoever is setting the hook up; never shown to the agent.
    try {
        $folder = Get-SwmMailboxFolder $Mailbox
        [IO.Directory]::CreateDirectory($folder) | Out-Null
        $log = Join-Path $folder 'agent-hook.errors.txt'
        [IO.File]::AppendAllText($log, "$([DateTime]::Now.ToString('s')) $Agent $_`n")
    } catch { }
}
exit 0
