#Requires -Version 7
# Sets another Windows machine up so that this machine's SWM can list its
# mailboxes (see "Mailboxes of other machines" in the README). Run it here, for
# a machine `ssh` reaches without asking anything:
#
#   .\mailbox\Deploy-SwmMailbox.ps1 -Address me@host           copy the files
#   .\mailbox\Deploy-SwmMailbox.ps1 -Address me@host -Hooks    and add Claude Code's hooks there
#   .\mailbox\Deploy-SwmMailbox.ps1 -Address me@host -Remove   take everything out again
#
# It copies swm-mailbox.exe and the hook scripts to %LOCALAPPDATA%\swm\bin on
# that machine, and nothing else. Run it again after building to update them:
# a watch that is running keeps the program it started with until SWM connects
# again. The machine needs PowerShell 7 for the hooks, and nothing for the rest.
param(
    [Parameter(Mandatory)][string]$Address,
    # Also add the hooks to that account's Claude Code settings.
    [switch]$Hooks,
    # Take the hooks, the files and the copies of that machine's logs out again.
    [switch]$Remove,
    # The swm-mailbox.exe to copy, when it is not where build.ps1 leaves it.
    [string]$Program
)

$ErrorActionPreference = 'Stop'
if ($Address -match '^-|\s' -or -not $Address) { throw "Address must be one word, as ssh takes it: '$Address'" }
$ssh = @('-T', '-o', 'BatchMode=yes', '--', $Address)

# Runs a line in that machine's shell for ssh, which is cmd.exe unless it was
# changed, and gives back what it printed.
function Invoke-There([string]$Line, [switch]$MayFail) {
    # With no input of its own: it must not wait for any, nor take this script's.
    $said = $null | & ssh @ssh $Line 2>&1 | ForEach-Object { "$_" } | Where-Object { $_ -notmatch '^\*\* ' }
    if ($LASTEXITCODE -ne 0 -and -not $MayFail) { throw "on ${Address}: $Line`n$($said -join "`n")" }
    $said
}

$local = (Invoke-There 'echo %LOCALAPPDATA%' | Select-Object -Last 1).Trim()
if ($local -notmatch '^[A-Za-z]:\\' -or $local -match '["%]') { throw "$Address did not say where its LOCALAPPDATA is: '$local'" }
$bin = "$local\swm\bin"

if ($Remove) {
    Invoke-There "pwsh -NoProfile -File `"$bin\Install-SwmAgentHooks.ps1`" -Remove" -MayFail
    Invoke-There "rmdir /s /q `"$bin`"" -MayFail | Out-Null
    "Removed $bin on $Address. Its mailbox entries are left in $local\swm\mailbox."
    return
}

if (-not $Program) {
    $target = $env:CARGO_TARGET_DIR ?? (Join-Path (Split-Path -Parent $PSScriptRoot) 'target')
    $Program = Join-Path $target 'release\swm-mailbox.exe'
}
if (-not (Test-Path -LiteralPath $Program)) { throw "No swm-mailbox.exe at ${Program}: run build.ps1 first, or give -Program" }
$files = @($Program) + @('agent-hook.ps1', 'SwmMailbox.ps1', 'Install-SwmAgentHooks.ps1' | ForEach-Object { Join-Path $PSScriptRoot $_ })

Invoke-There "if not exist `"$bin`" mkdir `"$bin`"" | Out-Null
# scp takes forward slashes in a Windows path, after a slash, and the path
# as it is: spaces in it need no quoting.
$there = '/' + $bin.Replace('\', '/')
foreach ($file in $files) {
    $name = Split-Path -Leaf $file
    $null | & scp -q -o BatchMode=yes -- $file "${Address}:$there/$name.new"
    if ($LASTEXITCODE -ne 0) { throw "copying $name to $Address failed" }
    # A program that is running can be renamed but not replaced: the one there
    # is moved aside, and removed the next time if it was still running now.
    $swap = "cd /d `"$bin`" && (del /q `"$name.old`" 2>nul & if exist `"$name`" ren `"$name`" `"$name.old`") && ren `"$name.new`" `"$name`" && (del /q `"$name.old`" 2>nul & echo ok)"
    Invoke-There $swap | Out-Null
}
$version = (Invoke-There "`"$bin\swm-mailbox.exe`" version" | Select-Object -Last 1).Trim()
"Copied to $bin on ${Address}: swm-mailbox (version $version) and the hook scripts."

$install = "pwsh -NoProfile -File `"$bin\Install-SwmAgentHooks.ps1`""
if ($Hooks) {
    Invoke-There $install
} else {
    "Claude Code's hooks there are untouched. To add them: run this again with -Hooks, or on ${Address}: $install"
}
"For builds, that machine's PowerShell profile needs:  . `"$bin\SwmMailbox.ps1`"; Enable-SwmMailboxPrompt"
