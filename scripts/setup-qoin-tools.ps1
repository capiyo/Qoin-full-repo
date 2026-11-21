[CmdletBinding()]
param(
    [switch]$SkipDocker
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
Set-StrictMode -Version Latest

$Work = Join-Path $env:TEMP "QoinSetup"
New-Item -ItemType Directory -Force -Path $Work | Out-Null

function Test-IsAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

if (-not (Test-IsAdministrator)) {
    Write-Host "Requesting administrator privileges..."
    $scriptPath = $PSCommandPath
    $argumentLine = '-NoProfile -ExecutionPolicy Bypass -File "' + $scriptPath + '"'
    if ($SkipDocker) { $argumentLine += " -SkipDocker" }
    $process = Start-Process -FilePath "powershell.exe" -Verb RunAs -ArgumentList $argumentLine -Wait -PassThru
    if ($process.ExitCode -ne 0) {
        throw "Elevated installer failed with exit code $($process.ExitCode)."
    }
    exit 0
}

function Download-File([string]$Url, [string]$Path) {
    Write-Host "Downloading $Url"
    Invoke-WebRequest -Uri $Url -OutFile $Path -UseBasicParsing
    if (-not (Test-Path $Path) -or (Get-Item $Path).Length -eq 0) {
        throw "Download failed or produced an empty file: $Url"
    }
}

function Install-Appx([string]$Path) {
    Write-Host "Installing AppX package: $Path"
    Add-AppxPackage -Path $Path
}

function Get-WingetPath {
    $command = Get-Command winget.exe -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }

    $candidate = Join-Path $env:LOCALAPPDATA "Microsoft\WindowsApps\winget.exe"
    if (Test-Path $candidate) { return $candidate }
    return $null
}

function Ensure-Winget {
    $existing = Get-WingetPath
    if ($existing) { return $existing }

    Write-Host "Winget is missing. Bootstrapping Microsoft's App Installer..."
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

    $vclibs = Join-Path $Work "Microsoft.VCLibs.x64.appx"
    $xaml = Join-Path $Work "Microsoft.UI.Xaml.2.8.x64.appx"
    $winget = Join-Path $Work "Microsoft.DesktopAppInstaller.msixbundle"

    Download-File "https://aka.ms/Microsoft.VCLibs.x64.14.00.Desktop.appx" $vclibs
    Download-File "https://aka.ms/Microsoft.UI.Xaml.2.8.x64.appx" $xaml
    Download-File "https://aka.ms/getwinget" $winget

    Install-Appx $vclibs
    Install-Appx $xaml
    Install-Appx $winget

    $env:PATH = "$env:PATH;$env:LOCALAPPDATA\Microsoft\WindowsApps"
    $existing = Get-WingetPath
    if (-not $existing) {
        throw "Winget bootstrap completed but winget.exe is still unavailable. Restart Windows and rerun this installer."
    }
    return $existing
}

$script:wingetPath = Ensure-Winget
Write-Host "Using Winget: $script:wingetPath"
& $script:wingetPath --version
if ($LASTEXITCODE -ne 0) { throw "Winget could not report its version." }

$installFailures = New-Object System.Collections.Generic.List[string]

function Install-WingetPackage([string]$Id, [string]$Name, [string]$Override = "") {
    Write-Host ""
    Write-Host "========== Installing $Name ($Id) ==========" -ForegroundColor Cyan

    $arguments = @(
        "install", "--id", $Id, "--exact",
        "--accept-package-agreements", "--accept-source-agreements",
        "--silent"
    )
    if ($Override) {
        $arguments += @("--override", $Override)
    }

    & $script:wingetPath @arguments
    $code = $LASTEXITCODE

    if ($code -eq 0) {
        Write-Host "$Name installed or already present." -ForegroundColor Green
    } elseif ($code -eq 3010 -or $code -eq 1641) {
        Write-Warning "$Name installed; Windows restart is required."
    } else {
        $installFailures.Add("$Name (exit code $code)")
        Write-Warning "$Name installation failed with exit code $code."
    }
}

Write-Host "============================================"
Write-Host " QOIN WINDOWS DEVELOPMENT TOOLCHAIN SETUP"
Write-Host "============================================"

Install-WingetPackage "Rustlang.Rustup" "Rust and Cargo"

$vsOverride = "--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --norestart"
Install-WingetPackage "Microsoft.VisualStudio.2022.BuildTools" "Visual Studio 2022 C++ Build Tools" $vsOverride

Install-WingetPackage "Kitware.CMake" "CMake"
Install-WingetPackage "Ninja-build.Ninja" "Ninja"
Install-WingetPackage "Python.Python.3.12" "Python 3.12"
Install-WingetPackage "OpenJS.NodeJS.LTS" "Node.js LTS"

if (-not $SkipDocker) {
    Install-WingetPackage "Docker.DockerDesktop" "Docker Desktop"
}

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")
$env:PATH = "$machinePath;$userPath;$env:LOCALAPPDATA\Microsoft\WindowsApps;$env:USERPROFILE\.cargo\bin"

$rustup = Join-Path $env:USERPROFILE ".cargo\bin\rustup.exe"
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
if (Test-Path $rustup) {
    & $rustup default stable-x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { $installFailures.Add("Rust stable MSVC toolchain configuration") }
    & $rustup component add rustfmt clippy
    if ($LASTEXITCODE -ne 0) { $installFailures.Add("Rust rustfmt/clippy components") }
}

Write-Host ""
Write-Host "========== Toolchain verification ==========" -ForegroundColor Cyan
foreach ($entry in @(
    @{ Name = "cargo"; Path = $cargo },
    @{ Name = "rustc"; Path = (Join-Path $env:USERPROFILE ".cargo\bin\rustc.exe") }
)) {
    if (Test-Path $entry.Path) {
        & $entry.Path --version
    } else {
        $installFailures.Add("$($entry.Name) executable missing after installation")
        Write-Warning "$($entry.Name) was not found at $($entry.Path)"
    }
}

foreach ($name in @("python", "cmake", "ninja", "node")) {
    $command = Get-Command $name -ErrorAction SilentlyContinue
    if ($command) {
        Write-Host "$name : $($command.Source)"
    } else {
        $installFailures.Add("$name is not available on PATH")
        Write-Warning "$name is not available on PATH in this session."
    }
}

if (-not $SkipDocker) {
    $docker = Get-Command docker -ErrorAction SilentlyContinue
    if ($docker) {
        Write-Host "docker : $($docker.Source)"
    } else {
        Write-Warning "Docker CLI is not available yet. Restart Windows, start Docker Desktop, then verify with 'docker version'."
    }
}

if ($installFailures.Count -gt 0) {
    Write-Host ""
    Write-Host "Installer completed with issues:" -ForegroundColor Yellow
    $installFailures | ForEach-Object { Write-Host " - $_" }
    throw "Qoin toolchain setup is incomplete. Resolve the listed issues, restart Windows if requested, and rerun this script."
}

Write-Host ""
Write-Host "Qoin development toolchain installation completed." -ForegroundColor Green
Write-Host "Restart Windows, open a new terminal, then run cargo --version, rustc --version, cmake --version, python --version and docker version."
Write-Host "Then run scripts\check-backend.ps1 from the repository root."
