#Requires -Version 7
# Writes entries for SWM's bar mailboxes (see `extensions::spool` in the README).
# Dot-source it from your PowerShell profile:
#
#   . C:\src\simplewindowmanager\mailbox\SwmMailbox.ps1
#   Invoke-SwmMailboxCommand { .\build.ps1 }            # or its alias: mbx { ... }
#   mbx -Title 'nterm release' -Log { cargo build --release }
#   Enable-SwmMailboxPrompt        # or: report every command that takes 5 s or more
#
# An entry is one small JSON file in %LOCALAPPDATA%\swm\mailbox\<mailbox>. SWM
# only reads that folder, so everything here is a plain file write: there is no
# pipe, no service and nothing to install.

# Entries and logs older than this are removed when a new entry is started.
$script:SwmMailboxKeepDays = 7

function Get-SwmMailboxFolder {
    # The folder SWM reads for `Mailbox`. SWM_MAILBOX_ROOT moves it, for tests;
    # SWM itself reads only the default, or the folder its configuration names.
    param([Parameter(Mandatory)][string]$Mailbox)
    if ($Mailbox -notmatch '^[A-Za-z0-9_-][A-Za-z0-9._-]*$') {
        throw "'$Mailbox' is not a mailbox name: use letters, digits, '.', '_' and '-'."
    }
    $root = $env:SWM_MAILBOX_ROOT
    if ([string]::IsNullOrWhiteSpace($root)) {
        $root = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'swm\mailbox'
    }
    Join-Path $root $Mailbox
}

function ConvertTo-SwmMailboxId {
    # `Text` as an entry id: a file name of letters, digits, '.', '_' and '-'.
    param([Parameter(Mandatory)][string]$Text)
    $id = ($Text -replace '[^A-Za-z0-9._-]', '-').TrimStart('.')
    if ($id.Length -gt 100) { $id = $id.Substring(0, 100) }
    if (-not $id) { throw "'$Text' has nothing an entry id can use." }
    $id
}

function Get-SwmMailboxEntry {
    # The entry as it was last written, or nothing.
    param([Parameter(Mandatory)][string]$Mailbox, [Parameter(Mandatory)][string]$Id)
    $path = Join-Path (Get-SwmMailboxFolder $Mailbox) "$Id.json"
    try {
        [IO.File]::ReadAllText($path) | ConvertFrom-Json -AsHashtable
    } catch {
        $null
    }
}

function Set-SwmMailboxEntry {
    # Writes or replaces an entry. `ProcessId` ties the entry to a process: SWM
    # shows the entry as stopped once that process is gone, so pass it for work in
    # progress and leave it out for work that has finished.
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$Mailbox,
        [Parameter(Mandatory)][string]$Id,
        [Parameter(Mandatory)][string]$State,
        [string]$Title,
        [string]$Source,
        [string]$Detail,
        # When the entry entered this state; now, if left out.
        [DateTime]$Time = [DateTime]::UtcNow,
        [int]$ProcessId,
        # A file SWM shows when the entry is opened: as text, or as its kind is
        # shown.
        [string]$Log,
        # Which kind of entry it is, such as `codex`: a mailbox can give a kind
        # badges of its own and a viewer for its logs (`SpoolOptions::kind`).
        [string]$Kind,
        # Seconds after `Time` before SWM lists the entry, for work that is only
        # worth showing once it has run a while. Remove the entry if it ends sooner.
        [int]$Delay
    )
    $entry = [ordered]@{
        state = $State
        time  = [DateTimeOffset]::new($Time.ToUniversalTime()).ToUnixTimeSeconds()
    }
    if ($Delay -gt 0) { $entry.delay = $Delay }
    if ($Source) { $entry.source = $Source }
    if ($Title) { $entry.title = $Title }
    if ($Detail) { $entry.detail = $Detail }
    if ($ProcessId) {
        $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
        if ($process) {
            $entry.pid = $ProcessId
            # With its start time, a later process given the same id is not mistaken
            # for this one. A string: the number is too large for some JSON readers.
            try { $entry.started = [string]$process.StartTime.ToFileTimeUtc() } catch { }
        }
    }
    if ($Log) { $entry.log = $Log }
    if ($Kind) { $entry.kind = $Kind }

    $folder = Get-SwmMailboxFolder $Mailbox
    [IO.Directory]::CreateDirectory($folder) | Out-Null
    $path = Join-Path $folder "$Id.json"
    # Written beside the entry and renamed over it, so SWM never reads half a file.
    $temporary = "$path.$PID.tmp"
    [IO.File]::WriteAllText($temporary, ($entry | ConvertTo-Json -Compress), [Text.UTF8Encoding]::new($false))
    for ($attempt = 1; ; $attempt++) {
        try {
            [IO.File]::Move($temporary, $path, $true)
            break
        } catch [IO.IOException] {
            # Something has the entry open for a moment.
            if ($attempt -ge 5) {
                Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue
                throw
            }
            Start-Sleep -Milliseconds 40
        }
    }
}

function Remove-SwmMailboxEntry {
    param([Parameter(Mandatory)][string]$Mailbox, [Parameter(Mandatory)][string]$Id)
    $folder = Get-SwmMailboxFolder $Mailbox
    foreach ($extension in 'json', 'log') {
        Remove-Item -LiteralPath (Join-Path $folder "$Id.$extension") -Force -ErrorAction SilentlyContinue
    }
}

function Clear-SwmMailboxOldEntries {
    # Removes entries, logs and leftover temporary files not written for `Days`.
    param([Parameter(Mandatory)][string]$Mailbox, [int]$Days = $script:SwmMailboxKeepDays)
    $folder = Get-SwmMailboxFolder $Mailbox
    if (-not (Test-Path -LiteralPath $folder)) { return }
    $before = [DateTime]::UtcNow.AddDays(-$Days)
    Get-ChildItem -LiteralPath $folder -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Extension -in '.json', '.log', '.tmp' -and $_.LastWriteTimeUtc -lt $before } |
        Remove-Item -Force -ErrorAction SilentlyContinue
}

function Get-SwmMailboxAncestor {
    # The id of the nearest process above `ProcessId` named like `Name` (a regular
    # expression), such as the agent a hook runs under; 0 if there is none.
    param([Parameter(Mandatory)][string]$Name, [int]$ProcessId = $PID)
    $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
    for ($depth = 0; $process -and $depth -lt 24; $depth++) {
        if ($process.ProcessName -match $Name) { return $process.Id }
        $process = try { $process.Parent } catch { $null }
    }
    0
}

function Format-SwmMailboxDuration {
    param([Parameter(Mandatory)][TimeSpan]$Duration)
    if ($Duration.TotalHours -ge 1) { '{0}h {1}m' -f [int][Math]::Floor($Duration.TotalHours), $Duration.Minutes }
    elseif ($Duration.TotalMinutes -ge 1) { '{0}m {1}s' -f $Duration.Minutes, $Duration.Seconds }
    else { '{0}s' -f $Duration.Seconds }
}

function Invoke-SwmMailboxCommand {
    # Runs `Command`, such as a build, and reports it to a mailbox: running while
    # it runs, then done, or failed when it throws or ends with an exit code.
    # Ctrl+C reports it as stopped; so does SWM itself if this shell is killed,
    # since the running entry names this process.
    #
    # Without -Log the command writes straight to the terminal, keeping its colors
    # and progress display. With -Log its output also goes to a file that opening
    # the entry shows, with a row per error and warning; most tools then write
    # plainer output, as they do into any pipe.
    [CmdletBinding()]
    param(
        [Parameter(Mandatory, Position = 0)][scriptblock]$Command,
        # What the entry says ran; the command's own text, if left out.
        [string]$Title,
        # The mailbox Enable-SwmMailboxPrompt reports to, if it was called.
        [string]$Mailbox = ($script:SwmMailboxPrompt.Mailbox ?? 'Builds'),
        # The line above the title; the current folder's name, if left out.
        [string]$Source = (Split-Path -Leaf (Get-Location).Path),
        [switch]$Log
    )
    if (-not $Title) {
        $Title = (($Command.ToString() -split "`r?`n" | ForEach-Object Trim | Where-Object { $_ }) -join '; ')
        if ($Title.Length -gt 120) { $Title = $Title.Substring(0, 120) + '…' }
    }
    $id = 'run-{0}-{1}' -f $PID, [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss-fff')
    $entry = @{ Mailbox = $Mailbox; Id = $id; Title = $Title; Source = $Source }
    $logPath = $null
    if ($Log) {
        $logPath = Join-Path (Get-SwmMailboxFolder $Mailbox) "$id.log"
        $entry.Log = $logPath
    }
    # Reporting must never be why a build fails: a problem writing the entry is a
    # warning, and the command still runs.
    $report = {
        param([hashtable]$More)
        try { Set-SwmMailboxEntry @entry @More } catch { Write-Warning "SWM mailbox: $_" }
    }
    try { Clear-SwmMailboxOldEntries $Mailbox } catch { }
    $started = [DateTime]::UtcNow
    & $report @{ State = 'running'; ProcessId = $PID; Time = $started }

    $state = 'stopped'
    $outcome = 'Cancelled'
    try {
        $global:LASTEXITCODE = 0
        if ($Log) {
            & $Command 2>&1 | Tee-Object -FilePath $logPath
        } else {
            & $Command
        }
        if ($global:LASTEXITCODE) {
            $state = 'failed'
            $outcome = "Exit code $global:LASTEXITCODE"
        } else {
            $state = 'done'
            $outcome = 'Succeeded'
        }
    } catch {
        $state = 'failed'
        $outcome = "$_".Trim()
        throw
    } finally {
        # Also reached on Ctrl+C, with the state still `stopped`.
        $took = Format-SwmMailboxDuration ([DateTime]::UtcNow - $started)
        $word = if ($state -eq 'done') { 'in' } else { 'after' }
        & $report @{ State = $state; Detail = "$outcome $word $took" }
    }
}

Set-Alias -Name mbx -Value Invoke-SwmMailboxCommand

# Commands the prompt does not report: programs you sit in rather than wait for.
$script:SwmMailboxPromptIgnore = @(
    'mbx', 'Invoke-SwmMailboxCommand', 'exit', 'nvim', 'vim', 'vi', 'hx', 'nano', 'code',
    'ssh', 'wsl', 'claude', 'codex', 'pwsh', 'powershell', 'cmd', 'bash', 'less', 'more',
    'man', 'lazygit', 'htop', 'btop', 'fzf', 'python', 'node', 'psql', 'sqlite3'
)
# Kept when this file is sourced again: the functions Enable-SwmMailboxPrompt
# wrapped are only reachable from here.
if (-not (Get-Variable SwmMailboxPrompt -Scope Script -ErrorAction Ignore)) {
    $script:SwmMailboxPrompt = $null
}

function Start-SwmMailboxPromptCommand {
    # Called with each line the prompt accepts, before it runs: writes a running
    # entry that SWM lists once the command has run for the configured time.
    param([string]$Line)
    $settings = $script:SwmMailboxPrompt
    if (-not $settings) { return }
    $settings.Pending = $null
    $text = (($Line -split "`r?`n" | ForEach-Object Trim | Where-Object { $_ }) -join '; ')
    if (-not $text) { return }
    # The command's name: the first word, without a path, extension or call operator.
    $name = ($text -replace '^[&.]\s+', '' -split '\s+')[0].Trim('"', "'")
    $name = [IO.Path]::GetFileNameWithoutExtension(($name -split '[\\/]')[-1])
    if ($name -in $settings.Ignore) { return }
    if ($text.Length -gt 120) { $text = $text.Substring(0, 120) + '…' }
    $started = [DateTime]::UtcNow
    $entry = @{
        Mailbox = $settings.Mailbox
        Id      = 'cmd-{0}-{1}' -f $PID, $started.ToString('yyyyMMdd-HHmmss-fff')
        Title   = $text
        Source  = Split-Path -Leaf (Get-Location).Path
    }
    Set-SwmMailboxEntry @entry -State running -ProcessId $PID -Time $started -Delay $settings.After
    $last = Get-History -Count 1
    $settings.Pending = @{
        Entry = $entry; Started = $started; HistoryId = if ($last) { $last.Id } else { 0 }
        ExitCode = $global:LASTEXITCODE
    }
}

function Complete-SwmMailboxPromptCommand {
    # Called when the prompt returns, with the command's `$?` and `$LASTEXITCODE`:
    # removes the entry of a command that was quick, and reports the outcome of
    # one that was not.
    param([bool]$Succeeded, [int]$ExitCode)
    $settings = $script:SwmMailboxPrompt
    $pending = $settings.Pending
    if (-not $pending) { return }
    $settings.Pending = $null
    $entry = $pending.Entry
    $took = [DateTime]::UtcNow - $pending.Started
    if ($took.TotalSeconds -lt $settings.After) {
        Remove-SwmMailboxEntry $entry.Mailbox $entry.Id
        return
    }
    # $LASTEXITCODE keeps the last program's exit code, so one that has not changed
    # may be an earlier command's: a failure is then reported without a code.
    if ($ExitCode -eq $pending.ExitCode) { $ExitCode = 0 }
    # Ctrl+C: PowerShell records the pipeline as stopped, and a program it ended
    # exits with STATUS_CONTROL_C_EXIT.
    $last = Get-History -Count 1
    $ran = $last -and $last.Id -ne $pending.HistoryId
    $state, $outcome =
        if (($ran -and $last.ExecutionStatus -eq 'Stopped') -or $ExitCode -eq -1073741510) { 'stopped', 'Cancelled' }
        elseif ($Succeeded) { 'done', 'Succeeded' }
        elseif ($ExitCode) { 'failed', "Exit code $ExitCode" }
        else { 'failed', 'Failed' }
    $word = if ($state -eq 'done') { 'in' } else { 'after' }
    Set-SwmMailboxEntry @entry -State $state -Detail "$outcome $word $(Format-SwmMailboxDuration $took)"
}

function Enable-SwmMailboxPrompt {
    # Reports every command that runs for `After` seconds or more to a mailbox,
    # with nothing to type: running while it runs, then done, failed or stopped.
    # Call it once, at the end of your profile, after anything else that replaces
    # the prompt. `mbx -Log` remains for a command whose output you want to open
    # from the entry.
    [CmdletBinding()]
    param(
        [string]$Mailbox = 'Builds',
        [ValidateRange(1, 3600)][int]$After = 5,
        # Command names not to report, instead of the built-in list.
        [string[]]$Ignore = $script:SwmMailboxPromptIgnore
    )
    $previous = $script:SwmMailboxPrompt
    $script:SwmMailboxPrompt = @{
        Mailbox = $Mailbox; After = $After; Ignore = $Ignore; Pending = $null
        Prompt = $previous.Prompt; ReadLine = $previous.ReadLine
    }
    try { Clear-SwmMailboxOldEntries $Mailbox } catch { }

    # Each function is wrapped unless it is the wrapper already, so a profile that
    # is sourced again, and defines its prompt again, ends up wrapped once.
    $readLine = (Get-Command PSConsoleHostReadLine -ErrorAction Ignore).ScriptBlock
    if ($readLine -and "$readLine" -notmatch 'Start-SwmMailboxPromptCommand') {
        $script:SwmMailboxPrompt.ReadLine = $readLine
        function global:PSConsoleHostReadLine {
            # First, so the wrapped function still reads the last command's `$?`.
            $line = & $script:SwmMailboxPrompt.ReadLine
            # Reporting must never be why a command does not run.
            try { Start-SwmMailboxPromptCommand $line } catch { }
            $line
        }
    }
    $prompt = $function:prompt
    if ("$prompt" -notmatch 'Complete-SwmMailboxPromptCommand') {
        $script:SwmMailboxPrompt.Prompt = $prompt
        function global:prompt {
            $succeeded = $?
            $code = $global:LASTEXITCODE
            try { Complete-SwmMailboxPromptCommand $succeeded ([int]$code) } catch { }
            # The wrapped prompt reads `$?` too: give it back the command's.
            if (-not $succeeded) { Write-Error 'the last command failed' -ErrorAction Ignore }
            & $script:SwmMailboxPrompt.Prompt
        }
    }
}
