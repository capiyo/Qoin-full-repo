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
    return $null
}

function Ensure-Chocolatey {
    $command = Get-Command choco.exe -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }

    Write-Host "WinGet is unavailable. Installing Chocolatey as the Windows package-manager fallback..."
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    $bootstrap = Join-Path $Work "install-chocolatey.ps1"
    Download-File "https://community.chocolatey.org/install.ps1" $bootstrap
    $content = Get-Content -LiteralPath $bootstrap -Raw
    if ($content -notmatch "ChocolateyInstall|choco") {
        throw "Chocolatey bootstrap script failed its content validation."
    }

    $env:ChocolateyInstall = Join-Path $env:ProgramData "chocolatey"
    Set-ExecutionPolicy Bypass -Scope Process -Force
    # The bootstrap script writes status text to the success pipeline; capture it
    # so this function returns only the executable path.
    $bootstrapOutput = @(& $bootstrap 2>&1)
    $bootstrapExitCode = $LASTEXITCODE
    if ($bootstrapExitCode -and $bootstrapExitCode -ne 0) {
        $bootstrapOutput | ForEach-Object { Write-Host $_ }
        throw "Chocolatey bootstrap failed with exit code $bootstrapExitCode."
    }
    $bootstrapOutput | ForEach-Object { Write-Host $_ }

    $candidate = Join-Path $env:ProgramData "chocolatey\bin\choco.exe"
    if (-not (Test-Path -LiteralPath $candidate)) {
        $candidate = Join-Path $env:ChocolateyInstall "bin\choco.exe"
    }
    if (-not (Test-Path -LiteralPath $candidate)) {
        throw "Chocolatey bootstrap completed but choco.exe was not found under ProgramData or ChocolateyInstall."
    }
    $env:PATH = "$env:PATH;$env:ProgramData\chocolatey\bin"
    return $candidate
}

$script:wingetPath = Ensure-Winget
$script:chocoPath = $null
if (-not $script:wingetPath) {
    $script:chocoPath = Ensure-Chocolatey
    Write-Host "Using Chocolatey: $script:chocoPath"
    & $script:chocoPath --version
    if ($LASTEXITCODE -ne 0) { throw "Chocolatey could not report its version." }
} else {
    Write-Host "Using Winget: $script:wingetPath"
    & $script:wingetPath --version
    if ($LASTEXITCODE -ne 0) { throw "Winget could not report its version." }
}

$installFailures = New-Object System.Collections.Generic.List[string]

function Install-WingetPackage([string]$Id, [string]$Name, [string]$Override = "", [string]$ChocolateyId = "", [string]$ChocolateyParameters = "") {
    Write-Host ""
    Write-Host "========== Installing $Name ($Id) ==========" -ForegroundColor Cyan

    if ($script:wingetPath) {
        $arguments = @(
            "install", "--id", $Id, "--exact",
            "--accept-package-agreements", "--accept-source-agreements",
            "--silent"
        )
        if ($Override) { $arguments += @("--override", $Override) }
        & $script:wingetPath @arguments
    } else {
        if (-not $ChocolateyId) {
            $installFailures.Add("$Name (no Chocolatey package mapping)")
            Write-Warning "$Name has no Chocolatey fallback mapping."
            return
        }
        $arguments = @("install", $ChocolateyId, "--yes", "--no-progress", "--limit-output")
        if ($ChocolateyParameters) {
            $arguments += @("--package-parameters", $ChocolateyParameters)
        }
        & $script:chocoPath @arguments
    }
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

Install-WingetPackage "Rustlang.Rustup" "Rust and Cargo" "" "rustup.install"

$vsOverride = "--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --add Microsoft.VisualStudio.Component.Windows10SDK.19041 --norestart"
Install-WingetPackage "Microsoft.VisualStudio.2022.BuildTools" "Visual Studio 2022 C++ Build Tools and Windows SDK" $vsOverride "visualstudio2022buildtools" "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --add Microsoft.VisualStudio.Component.Windows10SDK.19041 --quiet --wait --norestart"

# cl.exe alone is not a usable Windows C/C++ toolchain: CMake also needs
# the Windows SDK resource compiler (rc.exe) and manifest tool (mt.exe).
$windowsSdkBin = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
$sdkTools = @()
if (Test-Path -LiteralPath $windowsSdkBin) {
    $sdkTools = @(Get-ChildItem -Path $windowsSdkBin -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending | ForEach-Object {
            $candidate = Join-Path $_.FullName "x64"
            if ((Test-Path (Join-Path $candidate "rc.exe")) -and (Test-Path (Join-Path $candidate "mt.exe"))) { $candidate }
        })
}
if ($sdkTools.Count -gt 0) {
    $env:PATH = "$($sdkTools[0]);$env:PATH"
    Write-Host "Windows SDK tools : $($sdkTools[0])" -ForegroundColor Green
} else {
    $installFailures.Add("Windows SDK x64 tools missing (rc.exe and/or mt.exe); Visual Studio Windows 10 SDK component 10.0.19041 is required")
    Write-Warning "Windows SDK x64 tools were not found. CMake/vcpkg cannot build native dependencies without rc.exe and mt.exe."
}

Install-WingetPackage "Kitware.CMake" "CMake" "" "cmake.install"
Install-WingetPackage "Ninja-build.Ninja" "Ninja" "" "ninja"
Install-WingetPackage "Python.Python.3.12" "Python 3.12" "" "python"
Install-WingetPackage "OpenJS.NodeJS.LTS" "Node.js LTS" "" "nodejs-lts"
Install-WingetPackage "jqlang.jq" "jq JSON CLI" "" "jq"

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
    if ($sdkTools.Count -gt 0) {
        Write-Host "Installing Drogon and native dependencies through vcpkg..."
        & $vcpkgExe install "drogon:x64-windows"
        if ($LASTEXITCODE -ne 0) { $installFailures.Add("vcpkg drogon:x64-windows installation") }
    } else {
        Write-Warning "Skipping vcpkg Drogon build because the Windows SDK tools are missing; rerun after the SDK installation succeeds."
    }
    [Environment]::SetEnvironmentVariable("VCPKG_ROOT", $vcpkgRoot, "User")
    $env:VCPKG_ROOT = $vcpkgRoot
} else {
    $installFailures.Add("vcpkg.exe missing after bootstrap")
}

if (-not $SkipDocker) {
    Install-WingetPackage "Docker.DockerDesktop" "Docker Desktop" "" "docker-desktop"
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
