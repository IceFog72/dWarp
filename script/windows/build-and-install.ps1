#requires -Version 5.1
# Build the OSS channel with the existing Windows bundler and install its output.
[CmdletBinding()]
Param(
    [ValidateSet('x64', 'arm64')]
    [String]$Arch = 'x64',
    [ValidateNotNullOrEmpty()]
    [String]$ReleaseTag = 'v0.0.0-local',
    [Switch]$BuildOnly
)

$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') {
    throw 'Run this script on Windows with the MSVC build tools installed.'
}

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location -LiteralPath $repoRoot
try {
    $requiredTools = @('cargo', 'rustup', 'git', 'cmake', 'protoc', 'node', 'yarn')
    $missingTools = @($requiredTools | Where-Object {
        -not (Get-Command $_ -CommandType Application -ErrorAction SilentlyContinue)
    })
    if ($missingTools.Count -gt 0) {
        throw "Missing build tools: $($missingTools -join ', '). See docs/windows-build.md, then reopen your terminal."
    }

    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
        if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
            throw 'Install Visual Studio Build Tools with Desktop development with C++ and a Windows SDK. See docs/windows-build.md.'
        }
        $msvcComponent = if ($Arch -eq 'arm64') { 'Microsoft.VisualStudio.Component.VC.Tools.ARM64' } else { 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64' }
        $msvc = & $vswhere -latest -products '*' -requires $msvcComponent -property installationPath
        if ($LASTEXITCODE -ne 0 -or -not $msvc) {
            throw 'The Visual Studio C++ tools are missing. See docs/windows-build.md.'
        }
    }

    if (-not (Get-Command ISCC.exe -CommandType Application -ErrorAction SilentlyContinue)) {
        $programDirectories = @(${env:ProgramFiles(x86)}, $env:ProgramFiles) | Where-Object { $_ }
        $innoCompiler = $programDirectories | ForEach-Object {
            Get-ChildItem -LiteralPath $_ -Directory -Filter 'Inno Setup *' -ErrorAction SilentlyContinue
        } | ForEach-Object { Join-Path $_.FullName 'ISCC.exe' } |
            Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
        if (-not $innoCompiler) {
            throw 'Install Inno Setup or add ISCC.exe to PATH. See docs/windows-build.md.'
        }
        $env:PATH = "$(Split-Path -Parent $innoCompiler);$env:PATH"
    }

    if ($Arch -eq 'arm64') {
        $target = 'aarch64-pc-windows-msvc'
        $installerName = 'OpenWarpSetup-arm64.exe'
    } else {
        $target = 'x86_64-pc-windows-msvc'
        $installerName = 'OpenWarpSetup.exe'
    }
    & rustup target add $target
    if ($LASTEXITCODE -ne 0) { throw "Could not install Rust target $target" }

    if (-not (Get-Command cargo-about -CommandType Application -ErrorAction SilentlyContinue)) {
        & cargo install --locked cargo-about --version 0.8.4
        if ($LASTEXITCODE -ne 0) { throw 'Could not install cargo-about for bundled license generation.' }
    }

    # A separate PowerShell process contains the bundler's exit statements and
    # environment changes. Always call the script directly to preserve arguments.
    $shell = (Get-Process -Id $PID).Path
    & $shell -NoLogo -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'bundle.ps1') `
        -Channel oss -Arch $Arch -ReleaseTag $ReleaseTag
    if ($LASTEXITCODE -ne 0) { throw "Windows build failed with exit code $LASTEXITCODE" }

    $installer = Join-Path $PSScriptRoot "Output\$installerName"
    if (-not (Test-Path -LiteralPath $installer -PathType Leaf)) {
        throw "The build did not produce the installer: $installer"
    }
    Write-Host "Built installer: $installer"
    if (-not $BuildOnly) {
        # Keep the wizard interactive so running sessions are not closed silently.
        $setup = Start-Process -FilePath $installer -ArgumentList @('/CURRENTUSER', '/NORESTART') -Wait -PassThru
        if ($setup.ExitCode -eq 3010) {
            Write-Warning 'Installation completed. Windows reports that a restart is required.'
        } elseif ($setup.ExitCode -ne 0) {
            throw "Installation was cancelled or failed with exit code $($setup.ExitCode)"
        } else {
            Write-Host 'OpenWarp installation completed.'
        }
    }
} finally {
    Pop-Location
}
