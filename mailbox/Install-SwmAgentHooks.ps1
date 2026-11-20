#Requires -Version 7
# Adds the hooks that report Claude Code sessions to the SWM mailbox (see "Agent
# hooks" in the README) to this account's Claude Code settings, pointing at the
# agent-hook.ps1 beside this script. Everything else in the settings stays, and
# running it again changes nothing, or moves the hooks to where this script is now.
#
#   pwsh -NoProfile -File Install-SwmAgentHooks.ps1            add or update
#   pwsh -NoProfile -File Install-SwmAgentHooks.ps1 -Remove    take them out
#
# The settings file as it was before the first change is kept beside it as
# settings.json.before-swm-hooks.
param(
    [switch]$Remove,
    [string]$Settings = (Join-Path $HOME '.claude\settings.json')
)

$ErrorActionPreference = 'Stop'
$script = Join-Path $PSScriptRoot 'agent-hook.ps1'
if (-not $Remove -and -not (Test-Path -LiteralPath $script)) { throw "agent-hook.ps1 is not beside this script: $script" }
$events = 'SessionStart', 'UserPromptSubmit', 'PostToolUse', 'PermissionRequest', 'Notification', 'Stop', 'SessionEnd'

$before = if (Test-Path -LiteralPath $Settings) { Get-Content -LiteralPath $Settings -Raw } else { '' }
$config = if ($before.Trim()) { $before | ConvertFrom-Json -AsHashtable } else { [ordered]@{} }
if ($config -isnot [Collections.IDictionary]) { throw "$Settings does not hold a JSON object" }
if (-not $config.Contains('hooks')) { $config['hooks'] = [ordered]@{} }

# Ours is a hook that runs a file called agent-hook.ps1, wherever that is.
function Test-Ours($hook) {
    $hook -is [Collections.IDictionary] -and
        ((@($hook['command']) + @($hook['args'])) -match '(^|[\\/ "])agent-hook\.ps1')
}

foreach ($event in $events) {
    $kept = @()
    foreach ($group in @($config['hooks'][$event])) {
        if ($group -isnot [Collections.IDictionary]) { if ($null -ne $group) { $kept += , $group }; continue }
        $others = @(@($group['hooks']) | Where-Object { $null -ne $_ -and -not (Test-Ours $_) })
        if ($others.Count -eq @($group['hooks']).Count) { $kept += , $group; continue }
        # A group that held only ours goes with it.
        if ($others.Count) { $group['hooks'] = $others; $kept += , $group }
    }
    if (-not $Remove) {
        $kept += , ([ordered]@{ hooks = @([ordered]@{
                        type    = 'command'
                        async   = $true
                        command = 'pwsh.exe'
                        args    = @('-NoProfile', '-File', $script, 'claude')
                    })
            })
    }
    if ($kept.Count) { $config['hooks'][$event] = $kept } else { $config['hooks'].Remove($event) }
}
if ($config['hooks'].Count -eq 0) { $config.Remove('hooks') }

$after = ($config | ConvertTo-Json -Depth 64) + "`n"
if ($after.Trim() -eq $before.Trim()) { 'Claude Code hooks: already as they should be'; return }
$folder = Split-Path -Parent $Settings
[IO.Directory]::CreateDirectory($folder) | Out-Null
$kept = "$Settings.before-swm-hooks"
if ($before -and -not (Test-Path -LiteralPath $kept)) { [IO.File]::WriteAllText($kept, $before) }
# Written beside the file and renamed over it, so Claude Code never reads half of it.
$temporary = "$Settings.swm-$PID.tmp"
[IO.File]::WriteAllText($temporary, $after)
Move-Item -LiteralPath $temporary -Destination $Settings -Force
if ($Remove) { "Claude Code hooks: removed from $Settings" } else { "Claude Code hooks: in $Settings, running $script" }
