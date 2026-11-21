[CmdletBinding()]
param(
    [switch]$SkipDrogon
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Push-Location $RepoRoot
try {
    function Require-Command([string]$Name) {
        if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
            throw "Required command '$Name' was not found on PATH."
        }
    }
    function Invoke-Check([string]$Label, [string]$Command, [string[]]$Arguments) {
        Write-Host "`n== $Label ==" -ForegroundColor Cyan
        & $Command @Arguments
        if ($LASTEXITCODE -ne 0) { throw "$Label failed with exit code $LASTEXITCODE." }
    }

    Require-Command "cargo"
    Require-Command "python"
    Invoke-Check "Rust formatting" "cargo" @("fmt", "--all", "--", "--check")
    Invoke-Check "Rust workspace tests" "cargo" @("test", "--workspace", "--locked")
    Invoke-Check "Rust clippy" "cargo" @("clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings")
    Invoke-Check "Python backend reference tests" "python" @("-m", "unittest", "discover", "-s", "backend/tests", "-v")

    if (-not $SkipDrogon) {
        Require-Command "cmake"
        $DrogonConfigureArgs = @("-S", "backend/cpp-drogon", "-B", "build/drogon", "-DCMAKE_BUILD_TYPE=Release")
        if ($env:VCPKG_ROOT) {
            $VcpkgToolchain = Join-Path $env:VCPKG_ROOT "scripts/buildsystems/vcpkg.cmake"
            if (Test-Path $VcpkgToolchain) {
                $DrogonConfigureArgs += "-DCMAKE_TOOLCHAIN_FILE=$VcpkgToolchain"
                $DrogonConfigureArgs += "-DVCPKG_TARGET_TRIPLET=x64-windows"
            }
        }
        Invoke-Check "Drogon configure" "cmake" $DrogonConfigureArgs
        Invoke-Check "Drogon build" "cmake" @("--build", "build/drogon", "--config", "Release", "--parallel")
    }

    Write-Host "`nAll selected Qoin backend checks passed." -ForegroundColor Green
}
finally {
    Pop-Location
}
