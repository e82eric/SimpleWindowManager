param(
    [switch]$Dev,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CargoArgs
)

$ErrorActionPreference = 'Stop'
$portDirectory = $PSScriptRoot
$selectedConfig = $env:SWM_CONFIG_RS

if ([string]::IsNullOrWhiteSpace($selectedConfig)) {
    $localSettings = Join-Path $portDirectory 'build.local.ps1'
    if (Test-Path -LiteralPath $localSettings) {
        $SwmConfigRs = $null
        . $localSettings
        $selectedConfig = $SwmConfigRs
    }
}

if ([string]::IsNullOrWhiteSpace($selectedConfig)) {
    $selectedConfig = Join-Path $portDirectory 'default_user_config.rs'
} elseif (-not [System.IO.Path]::IsPathRooted($selectedConfig)) {
    $selectedConfig = Join-Path $portDirectory $selectedConfig
}

$selectedConfig = (Resolve-Path -LiteralPath $selectedConfig).Path
$env:SWM_CONFIG_RS = $selectedConfig
$selectedContents = Get-Content -LiteralPath $selectedConfig -Raw
$effectiveContents = $selectedContents
$delegation = 'simple_window_manager_rs::config::configure(configuration);'

if ($selectedContents.Contains($delegation)) {
    $repositoryConfig = Join-Path $portDirectory 'default_user_config.rs'
    $effectiveContents = (Get-Content -LiteralPath $repositoryConfig -Raw).TrimEnd()
}

Write-Host "SWM configuration: $selectedConfig"
if ($selectedContents.Contains($delegation)) {
    Write-Host 'SWM configuration delegates to the repository configuration'
}
Write-Host '----- effective SWM configuration begin -----'
$lines = $effectiveContents -split "`r?`n"
for ($index = 0; $index -lt $lines.Count; $index++) {
    Write-Host ('{0,4} | {1}' -f ($index + 1), $lines[$index])
}
Write-Host '----- effective SWM configuration end -------'

$arguments = @('build', '--manifest-path', (Join-Path $portDirectory 'Cargo.toml'))
if (-not $Dev) {
    $arguments += '--release'
}
if ($CargoArgs) {
    $arguments += $CargoArgs
}

& cargo @arguments
exit $LASTEXITCODE
