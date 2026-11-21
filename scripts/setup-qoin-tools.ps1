[CmdletBinding()]
param(
    [switch]$SkipDocker,
    [string]$LogPath
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
    if (-not $LogPath) {
        $LogPath = Join-Path $Work "setup-qoin-tools-elevated.log"
    }
    if (Test-Path $LogPath) { Remove-Item -LiteralPath $LogPath -Force }

    $scriptPath = $PSCommandPath
    $argumentList = @(
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File", "`"$scriptPath`"",
        "-LogPath", "`"$LogPath`""
    )
    if ($SkipDocker) { $argumentList += "-SkipDocker" }

    try {
        $process = Start-Process -FilePath (Join-Path $PSHOME "powershell.exe") -Verb RunAs -ArgumentList $argumentList -Wait -PassThru -ErrorAction Stop
    } catch {
        throw "Could not start the elevated installer. Windows reported: $($_.Exception.Message)"
    }

    if ($process.ExitCode -ne 0) {
        if (Test-Path $LogPath) {
            Write-Host "`n========== ELEVATED INSTALLER LOG ==========" -ForegroundColor Yellow
            Get-Content -LiteralPath $LogPath
        } else {
            Write-Warning "The elevated process did not create its diagnostic log."
        }
        throw "Elevated installer failed with exit code $($process.ExitCode). See the log above."
    }
    if (Test-Path $LogPath) {
        Write-Host "`n========== ELEVATED INSTALLER OUTPUT =========="
        Get-Content -LiteralPath $LogPath
    }
    exit 0
}

if ($LogPath) {
    Start-Transcript -LiteralPath $LogPath -Force | Out-Null
}

function Download-File([string]$Url, [string]$Path) {
    # Windows PowerShell 5.1 can fail TLS handshakes against aka.ms on older
    # Windows builds. Prefer the Windows curl client (Schannel), then retry
    # with .NET using TLS 1.2. A transient failure must not abort bootstrap.
    Write-Host "Downloading $Url"
    $curl = Get-Command "curl.exe" -ErrorAction SilentlyContinue
    $lastError = $null

    for ($attempt = 1; $attempt -le 4; $attempt++) {
        if (Test-Path $Path) { Remove-Item -LiteralPath $Path -Force }
        try {
            if ($curl) {
                Write-Host "  Attempt $attempt/4 using curl.exe"
                & $curl.Source --fail --location --silent --show-error --retry 2 --retry-all-errors --connect-timeout 30 --max-time 600 --output $Path $Url
                if ($LASTEXITCODE -ne 0) {
                    throw "curl.exe exited with code $LASTEXITCODE"
                }
            } else {
                Write-Host "  Attempt $attempt/4 using Invoke-WebRequest"
                [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
                Invoke-WebRequest -Uri $Url -OutFile $Path -UseBasicParsing -TimeoutSec 600
            }

            if ((Test-Path $Path) -and (Get-Item -LiteralPath $Path).Length -gt 0) {
                return
            }
            throw "Download produced no file or an empty file."
        } catch {
            $lastError = $_
            Write-Warning "Download attempt $attempt failed: $($_.Exception.Message)"
            Start-Sleep -Seconds ([Math]::Min(5 * $attempt, 20))
        }
    }

    throw "Unable to download $Url after 4 attempts. Last error: $lastError"
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
Install-WingetPackage "jqlang.jq" "jq JSON CLI"

# vcpkg builds Drogon and its native dependencies for the Windows MSVC toolchain.
$vcpkgRoot = Join-Path $env:LOCALAPPDATA "vcpkg"
if (-not (Test-Path (Join-Path $vcpkgRoot "vcpkg.exe"))) {
    if (Test-Path $vcpkgRoot) {
        throw "A vcpkg directory exists but vcpkg.exe is missing: $vcpkgRoot. Remove or repair that directory before rerunning."
    }
    Write-Host "Cloning Microsoft's vcpkg package manager..."
    & git clone https://github.com/microsoft/vcpkg.git $vcpkgRoot
    if ($LASTEXITCODE -ne 0) { $installFailures.Add("vcpkg clone") }
}
$vcpkgBootstrap = Join-Path $vcpkgRoot "bootstrap-vcpkg.bat"
if (Test-Path $vcpkgBootstrap) {
    & $vcpkgBootstrap -disableMetrics
    if ($LASTEXITCODE -ne 0) { $installFailures.Add("vcpkg bootstrap") }
}
$vcpkgExe = Join-Path $vcpkgRoot "vcpkg.exe"
if (Test-Path $vcpkgExe) {
    Write-Host "Installing Drogon and native dependencies through vcpkg..."
    & $vcpkgExe install "drogon:x64-windows"
    if ($LASTEXITCODE -ne 0) { $installFailures.Add("vcpkg drogon:x64-windows installation") }
    [Environment]::SetEnvironmentVariable("VCPKG_ROOT", $vcpkgRoot, "User")
    $env:VCPKG_ROOT = $vcpkgRoot
} else {
    $installFailures.Add("vcpkg.exe missing after bootstrap")
}

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

if ($LogPath) {
    Stop-Transcript | Out-Null
}
