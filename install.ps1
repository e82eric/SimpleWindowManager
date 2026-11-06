#Requires -RunAsAdministrator
# Copies the release build into Program Files. SWM runs elevated, so it must run
# from a folder only administrators can write to: anything that can replace the
# exe, or plant a DLL beside it, would otherwise run as admin on the next start.
param(
    # GetFolderPath asks Windows for Program Files; $env:ProgramFiles is only an
    # environment variable, which the user's own settings could point elsewhere.
    [string]$Destination = (Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'SimpleWindowManager')
)

$ErrorActionPreference = 'Stop'

# Accounts that may hold write access: Administrators, SYSTEM, TrustedInstaller.
$trusted = @(
    'S-1-5-32-544',
    'S-1-5-18',
    'S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464'
)
# CREATOR OWNER / CREATOR GROUP stand for whoever creates a child item. Only an
# account that can already write here can create one, so they add nothing.
$creatorSids = @('S-1-3-0', 'S-1-3-1')

# Rights that let an account change what is in the install folder, or the folder's
# own permissions: write/append data (create files/folders), write attributes and
# extended attributes, delete child, delete, change permissions, take ownership,
# and the generic write/all rights.
$installWriteMask = 0x2 -bor 0x4 -bor 0x10 -bor 0x40 -bor 0x100 -bor 0x10000 -bor
    0x40000 -bor 0x80000 -bor 0x40000000 -bor 0x10000000
# On a folder above the install folder, what matters is whether an account could
# move the install folder away (delete child; delete, which allows renaming) or
# give itself those rights (change permissions, take ownership, generic all).
# Creating unrelated items beside it is harmless, so C:\ granting users "create
# folders" is fine.
$ancestorWriteMask = 0x40 -bor 0x10000 -bor 0x40000 -bor 0x80000 -bor 0x10000000

function Get-UntrustedWriters {
    param(
        [System.IO.FileSystemInfo]$Item,
        [int]$Mask,
        # Inherit-only entries don't apply to the item itself, only to new
        # children: they matter inside the install folder, not above it.
        [bool]$IncludeInheritOnly
    )
    $problems = @()
    if ($Item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        $problems += "$($Item.FullName) is a junction or symbolic link"
    }
    $acl = Get-Acl -LiteralPath $Item.FullName
    $owner = $acl.GetOwner([Security.Principal.SecurityIdentifier]).Value
    if ($trusted -notcontains $owner) {
        $problems += "$($Item.FullName) is owned by $(Get-AccountName $owner)"
    }
    foreach ($rule in $acl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier])) {
        if ($rule.AccessControlType -ne 'Allow') { continue }
        $sid = $rule.IdentityReference.Value
        if ($trusted -contains $sid -or $creatorSids -contains $sid) { continue }
        $inheritOnly = $rule.PropagationFlags -band [Security.AccessControl.PropagationFlags]::InheritOnly
        if ($inheritOnly -and -not $IncludeInheritOnly) { continue }
        if (([int]$rule.FileSystemRights -band $Mask) -ne 0) {
            $problems += "$(Get-AccountName $sid) can modify $($Item.FullName) ($($rule.FileSystemRights))"
        }
    }
    $problems
}

function Get-AccountName([string]$Sid) {
    try {
        ([Security.Principal.SecurityIdentifier]$Sid).Translate([Security.Principal.NTAccount]).Value
    } catch {
        $Sid
    }
}

function Assert-AdminOnly([string]$Folder) {
    $folderItem = Get-Item -LiteralPath $Folder -Force
    $problems = @(Get-UntrustedWriters $folderItem $installWriteMask $true)
    # Anything already inside (an earlier install, its logs) keeps its own
    # permissions when overwritten, so check each item as well.
    foreach ($child in Get-ChildItem -LiteralPath $Folder -Recurse -Force) {
        $problems += Get-UntrustedWriters $child $installWriteMask $true
    }
    $parent = $folderItem.Parent
    while ($parent) {
        $mask = $ancestorWriteMask
        if (-not $parent.Parent) {
            # A drive root can't be renamed or deleted.
            $mask = $mask -band -bnot 0x10000
        }
        $problems += Get-UntrustedWriters $parent $mask $false
        $parent = $parent.Parent
    }
    if ($problems) {
        throw ("$Folder is not writable by administrators only, so SWM would not be safe to run from it:`n  " +
            ($problems -join "`n  "))
    }
}

$Destination = [IO.Path]::GetFullPath($Destination)
$release = Join-Path $PSScriptRoot 'target\release'
$binaries = @('simple-window-manager-rs.exe', 'restore_moved_windows.exe') |
    ForEach-Object { Join-Path $release $_ } |
    Where-Object { Test-Path -LiteralPath $_ }
if (-not ($binaries -match 'simple-window-manager-rs\.exe$')) {
    throw "No release build in $release; run .\build.ps1 first."
}

# Stopping it here would strand windows on hidden workspaces off-screen.
if (Get-Process simple-window-manager-rs -ErrorAction SilentlyContinue) {
    throw 'SimpleWindowManager is running; quit it (restoring windows) and run this again.'
}

# A new folder under Program Files inherits its ACL: users can read and run, not write.
New-Item -ItemType Directory -Force -Path $Destination | Out-Null
Assert-AdminOnly $Destination
foreach ($binary in $binaries) {
    Copy-Item -LiteralPath $binary -Destination $Destination -Force
}
Write-Host "Installed to $Destination"
Write-Host "Run $(Join-Path $Destination 'simple-window-manager-rs.exe'); logs are written to $(Join-Path $Destination 'logs')"
