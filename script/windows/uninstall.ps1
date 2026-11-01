#requires -Version 5.1
# Remove this fork, including legacy OpenWarp installations. -Purge deletes user data.
[CmdletBinding(SupportsShouldProcess = $true)]
Param(
    [Switch]$Purge,
    [Switch]$LegacyOnly
)

$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'Run this script on Windows.' }
if ($LegacyOnly -and $Purge) { throw '-LegacyOnly preserves shared data. Do not combine it with -Purge.' }

$names = @('OpenWarp')
$executables = @('warp-oss')
if (-not $LegacyOnly) {
    $names += 'dWarp'
    $executables += 'dwarp'
}
if (-not $WhatIfPreference -and (Get-Process -Name $executables -ErrorAction SilentlyContinue)) {
    throw 'Close dWarp/OpenWarp and its terminal sessions before uninstalling.'
}
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$isAdmin = ([Security.Principal.WindowsPrincipal]$identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
$locations = @()
foreach ($hive in @('HKCU:', 'HKLM:')) {
    foreach ($view in @('Software', 'Software\WOW6432Node')) {
        $key = "$hive\$view\Microsoft\Windows\CurrentVersion\Uninstall\warp-terminal-oss_is1"
        if (-not (Test-Path -LiteralPath $key)) { continue }
        $entry = Get-ItemProperty -LiteralPath $key
        if ($LegacyOnly -and $entry.DisplayName -notlike 'OpenWarp*') { continue }
        if ($hive -eq 'HKLM:' -and -not $isAdmin -and -not $WhatIfPreference) {
            throw 'An all-users installation exists. Run this script as administrator using the same Windows account.'
        }
        $uninstallCommand = $entry.UninstallString
        if ($uninstallCommand -match '^"([^\"]+\.exe)"' -or $uninstallCommand -match '^([^\"]+?unins\d+\.exe)(?:\s|$)') {
            $uninstaller = $Matches[1]
        } else {
            throw "Cannot parse the registered uninstaller in $key"
        }
        if (-not (Test-Path -LiteralPath $uninstaller -PathType Leaf)) {
            throw "Registered uninstaller missing: $uninstaller"
        }
        $locations += Split-Path -Parent $uninstaller
        if ($PSCmdlet.ShouldProcess($uninstaller, 'Uninstall dWarp/OpenWarp')) {
            $process = Start-Process -FilePath $uninstaller -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -Wait -PassThru
            if ($process.ExitCode -ne 0) { throw "Uninstall failed with exit code $($process.ExitCode)" }
        }
    }
}

# Inno Setup removes its tracked files, shortcuts, context menu, and PATH entry.
# Remove legacy integration remnants left by an upgrade and runtime URI handlers.
foreach ($hive in @('HKCU:', 'HKLM:')) {
    if ($hive -eq 'HKLM:' -and -not $isAdmin) { continue }
    foreach ($name in $names) {
        foreach ($shell in @('Directory\shell', 'Directory\Background\shell')) {
            foreach ($suffix in @('', 'Tab', 'Window')) {
                $key = "$hive\Software\Classes\$shell\$name$suffix"
                if ((Test-Path -LiteralPath $key) -and $PSCmdlet.ShouldProcess($key, 'Remove terminal context menu')) {
                    Remove-Item -LiteralPath $key -Recurse -Force
                }
            }
        }
        $key = "$hive\Software\Warp.dev\$name"
        if ((Test-Path -LiteralPath $key) -and $PSCmdlet.ShouldProcess($key, 'Remove installation registry entry')) {
            Remove-Item -LiteralPath $key -Recurse -Force
        }
    }
    $schemes = @('openwarp')
    if (-not $LegacyOnly) { $schemes += 'dwarp' }
    foreach ($scheme in $schemes) {
        $key = "$hive\Software\Classes\$scheme"
        if (-not (Test-Path -LiteralPath $key)) { continue }
        $commandKey = "$key\shell\open\command"
        if (-not (Test-Path -LiteralPath $commandKey)) { continue }
        $command = (Get-Item -LiteralPath $commandKey).GetValue('')
        $owned = @($executables | Where-Object { $command -match ('[\\/]' + [Regex]::Escape($_) + '\.exe"') }).Count -gt 0
        if ($owned -and $PSCmdlet.ShouldProcess($key, 'Remove terminal URL handler')) {
            Remove-Item -LiteralPath $key -Recurse -Force
        }
    }
}

# Remove only known install bin directories if an older uninstaller left PATH behind.
$pathScopes = @('User')
if ($isAdmin) { $pathScopes += 'Machine' }
$binDirectories = @($locations | ForEach-Object { (Join-Path $_ 'bin').TrimEnd('\') })
foreach ($scope in $pathScopes) {
    $oldPath = [Environment]::GetEnvironmentVariable('Path', $scope)
    if (-not $oldPath) { continue }
    $entries = @($oldPath.Split(';') | Where-Object {
        $binDirectories -notcontains $_.Trim('"').TrimEnd('\')
    })
    $newPath = $entries -join ';'
    if ($newPath -ne $oldPath -and $PSCmdlet.ShouldProcess("$scope PATH", 'Remove terminal install bin directories')) {
        [Environment]::SetEnvironmentVariable('Path', $newPath, $scope)
    }
}

foreach ($location in ($locations | Select-Object -Unique)) {
    # The uninstaller may leave its own executable until its process exits.
    # Do not recursively delete install roots that can contain user-created files.
    foreach ($file in @('warp-oss.exe', 'bin\warp-oss.cmd', 'bin\dwarp.cmd')) {
        $path = Join-Path $location $file
        if ((Test-Path -LiteralPath $path) -and $PSCmdlet.ShouldProcess($path, 'Remove legacy launcher')) {
            Remove-Item -LiteralPath $path -Force
        }
    }
}

$shortcutDirectories = @(
    [Environment]::GetFolderPath('DesktopDirectory'),
    [Environment]::GetFolderPath('Programs')
)
if ($isAdmin) {
    $shortcutDirectories += [Environment]::GetFolderPath('CommonDesktopDirectory')
    $shortcutDirectories += [Environment]::GetFolderPath('CommonPrograms')
}
foreach ($directory in $shortcutDirectories) {
    if (-not $directory) { continue }
    foreach ($name in $names) {
        $path = Join-Path $directory "$name.lnk"
        if ((Test-Path -LiteralPath $path) -and $PSCmdlet.ShouldProcess($path, 'Remove terminal shortcut')) {
            Remove-Item -LiteralPath $path -Force
        }
    }
}
# keyring maps SSH accounts to <account>.openwarp.ssh in Credential Manager.
# Enumerate target names only. Credential blobs are never read or printed.
if ($Purge) {
    if (-not ('DWarpCredentialCleanup' -as [Type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class DWarpCredentialCleanup {
    [StructLayout(LayoutKind.Sequential)]
    private struct Header {
        public uint Flags;
        public uint Type;
        public IntPtr TargetName;
    }
    [DllImport("advapi32.dll", EntryPoint = "CredEnumerateW", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool Enumerate(string filter, uint flags, out uint count, out IntPtr credentials);
    [DllImport("advapi32.dll", EntryPoint = "CredDeleteW", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool Delete(string target, uint type, uint flags);
    [DllImport("advapi32.dll")]
    private static extern void CredFree(IntPtr buffer);
    public static string[] Targets() {
        uint count;
        IntPtr entries;
        if (!Enumerate(null, 0, out count, out entries)) {
            int error = Marshal.GetLastWin32Error();
            if (error == 1168) return new string[0];
            throw new Win32Exception(error);
        }
        try {
            var names = new List<string>();
            for (int i = 0; i < count; ++i) {
                IntPtr entry = Marshal.ReadIntPtr(entries, i * IntPtr.Size);
                var header = (Header)Marshal.PtrToStructure(entry, typeof(Header));
                string name = Marshal.PtrToStringUni(header.TargetName);
                if (header.Type == 1 && name != null && name.EndsWith(".openwarp.ssh", StringComparison.Ordinal)) names.Add(name);
            }
            return names.ToArray();
        } finally { CredFree(entries); }
    }
    public static void Remove(string target) {
        if (!target.EndsWith(".openwarp.ssh", StringComparison.Ordinal)) throw new ArgumentException("Unexpected credential service");
        if (!Delete(target, 1, 0)) {
            int error = Marshal.GetLastWin32Error();
            if (error != 1168) throw new Win32Exception(error);
        }
    }
}
'@
    }
    foreach ($target in [DWarpCredentialCleanup]::Targets()) {
        if ($PSCmdlet.ShouldProcess($target, 'Delete SSH credential')) {
            [DWarpCredentialCleanup]::Remove($target)
        }
    }
}

if ($Purge) {
    foreach ($path in @(
        (Join-Path $env:APPDATA 'openwarp\OpenWarp'),
        (Join-Path $env:LOCALAPPDATA 'openwarp\OpenWarp'),
        (Join-Path $env:USERPROFILE '.openwarp'),
        (Join-Path $env:USERPROFILE '.warp-oss\remote-server')
    )) {
        if ((Test-Path -LiteralPath $path) -and $PSCmdlet.ShouldProcess($path, 'Permanently delete settings, history, caches, and local credentials')) {
            Remove-Item -LiteralPath $path -Recurse -Force
        }
    }
}
if ($WhatIfPreference) {
    Write-Host 'Dry run complete.'
} elseif ($Purge) {
    Write-Host 'dWarp/OpenWarp uninstalled and current-user data deleted.'
} else {
    Write-Host 'dWarp/OpenWarp uninstalled. User data preserved. Use -Purge to delete it.'
}
