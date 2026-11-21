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
    # Do not let one broken transport prevent trying the others. curl.exe,
    # BITS and Windows PowerShell use different download paths on Windows.
    Write-Host "Downloading $Url"
    $curl = Get-Command "curl.exe" -ErrorAction SilentlyContinue
    $lastError = $null

    # Windows PowerShell 5.1 otherwise negotiates older TLS defaults.
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    } catch {
        Write-Warning "Could not set the .NET TLS 1.2 default: $($_.Exception.Message)"
    }

    for ($attempt = 1; $attempt -le 3; $attempt++) {
        $transports = @()
        if ($curl) { $transports += "curl" }
        if (Get-Command Start-BitsTransfer -ErrorAction SilentlyContinue) { $transports += "bits" }
        $transports += "webrequest"

        foreach ($transport in $transports) {
            if (Test-Path -LiteralPath $Path) {
                Remove-Item -LiteralPath $Path -Force
            }

            try {
                switch ($transport) {
                    "curl" {
                        Write-Host "  Attempt $attempt/3 using curl.exe"
                        & $curl.Source --fail --location --silent --show-error --retry 2 --retry-delay 2 --connect-timeout 30 --max-time 600 --output $Path $Url
                        if ($LASTEXITCODE -ne 0) {
                            throw "curl.exe exited with code $LASTEXITCODE"
                        }
                    }
                    "bits" {
                        Write-Host "  Attempt $attempt/3 using Background Intelligent Transfer Service"
                        Start-BitsTransfer -TransferType Download -Source $Url -Destination $Path -ErrorAction Stop
                    }
                    "webrequest" {
                        Write-Host "  Attempt $attempt/3 using Invoke-WebRequest (TLS 1.2)"
                        Invoke-WebRequest -Uri $Url -OutFile $Path -UseBasicParsing -TimeoutSec 600 -ErrorAction Stop
                    }
                }

                if ((Test-Path -LiteralPath $Path) -and (Get-Item -LiteralPath $Path).Length -gt 0) {
                    Write-Host "  Download completed using $transport."
                    return
                }
                throw "Download produced no file or an empty file."
            } catch {
                $lastError = $_
                Write-Warning "  $transport failed: $($_.Exception.Message)"
                if (Test-Path -LiteralPath $Path) {
                    Remove-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
                }
            }
        }

        if ($attempt -lt 3) {
            Start-Sleep -Seconds (5 * $attempt)
        }
    }

    throw "Unable to download $Url using curl.exe, BITS, or Invoke-WebRequest after 3 rounds. Last error: $lastError. If all transports report a TLS/Schannel handshake failure, check Windows date/time, proxy/antivirus HTTPS inspection, and access to aka.ms."
}

function Assert-AppxPackage([string]$Path, [string]$ExpectedSha256 = "") {
    if (-not (Test-Path -LiteralPath $Path)) {
        throw "Expected package does not exist: $Path"
    }

    $item = Get-Item -LiteralPath $Path
    if ($item.Length -lt 4) {
        throw "Downloaded package is too small to be a valid AppX/MSIX archive: $Path"
    }

    $stream = [System.IO.File]::OpenRead($Path)
    try {
        $signature = New-Object byte[] 4
        [void]$stream.Read($signature, 0, 4)
    } finally {
        $stream.Dispose()
    }

    # AppX/MSIX packages are ZIP-based OPC archives. Reject HTML/error pages
    # even when the HTTP request returned status 200.
    if ($signature[0] -ne 0x50 -or $signature[1] -ne 0x4B) {
        $preview = [System.Text.Encoding]::UTF8.GetString([System.IO.File]::ReadAllBytes($Path), 0, [Math]::Min(160, [int]$item.Length))
        throw "Downloaded file is not an AppX/MSIX ZIP package: $Path. First bytes: $preview"
    }

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [System.IO.Compression.ZipFile]::OpenRead($Path)
    try {
        if ($archive.Entries.Count -lt 1) {
            throw "Package archive is empty: $Path"
        }
    } finally {
        $archive.Dispose()
    }

    if ($ExpectedSha256) {
        $actual = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
        if ($actual -ne $ExpectedSha256) {
            throw "SHA256 mismatch for $Path. Expected $ExpectedSha256, received $actual."
        }
    }

    Write-Host "  Package validation passed: $($item.Name) ($($item.Length) bytes)."
}

function Install-Appx([string]$Path) {
    Assert-AppxPackage $Path
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
    $windowsAppRuntime = Join-Path $Work "WindowsAppRuntimeInstall-x64.exe"

    Download-File "https://aka.ms/Microsoft.VCLibs.x64.14.00.Desktop.appx" $vclibs
    # The old aka.ms UI.Xaml alias now redirects to Bing in some environments.
    # Use Microsoft's official GitHub release asset instead.
    Download-File "https://github.com/microsoft/microsoft-ui-xaml/releases/download/v2.8.6/Microsoft.UI.Xaml.2.8.x64.appx" $xaml
    Download-File "https://aka.ms/getwinget" $winget
    # Official Microsoft Windows App SDK 1.8 x64 runtime installer.
    # App Installer 1.29 declares WindowsAppRuntime.1.8 as a required framework.
    Download-File "https://download.microsoft.com/download/8eb5473d-5663-4337-a1ca-aa5030b9b194/WindowsAppRuntimeInstall-x64.exe" $windowsAppRuntime

    # Validate every package before invoking Windows deployment.
    Assert-AppxPackage $vclibs
    Assert-AppxPackage $xaml "249D2AFB41CC009494841372BD6DD2DF46F87386D535DDF8D9F32C97226D2E46"
    Assert-AppxPackage $winget

    $runtimeBytes = [System.IO.File]::ReadAllBytes($windowsAppRuntime)
    if ($runtimeBytes.Length -lt 2 -or $runtimeBytes[0] -ne 0x4D -or $runtimeBytes[1] -ne 0x5A) {
        throw "Windows App SDK runtime download is not a valid Windows executable: $windowsAppRuntime"
    }

    Install-Appx $vclibs
    Install-Appx $xaml

    Write-Host "Installing Microsoft Windows App SDK 1.8 x64 runtime..."
    $runtimeProcess = Start-Process -FilePath $windowsAppRuntime -ArgumentList "--quiet" -Wait -PassThru
    if ($runtimeProcess.ExitCode -ne 0) {
        throw "Windows App SDK 1.8 runtime installer failed with exit code $($runtimeProcess.ExitCode)."
    }

    $runtimePackage = Get-AppxPackage -Name "Microsoft.WindowsAppRuntime.1.8" -ErrorAction SilentlyContinue |
        Where-Object { $_.Version -ge [version]"8000.616.304.0" } |
        Select-Object -First 1
    if (-not $runtimePackage) {
        throw "Windows App SDK runtime installer completed but Microsoft.WindowsAppRuntime.1.8 version 8000.616.304.0 or newer is not registered. Check Windows App SDK deployment logs and Windows compatibility."
    }

    Write-Host "Windows App SDK runtime installed: $($runtimePackage.Version)"
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
