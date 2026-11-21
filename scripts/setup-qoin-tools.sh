#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PS_SCRIPT="${SCRIPT_DIR}/setup-qoin-tools.ps1"

if [[ ! -f "${PS_SCRIPT}" ]]; then
  echo "Missing Windows installer: ${PS_SCRIPT}" >&2
  exit 1
fi

if ! command -v powershell.exe >/dev/null 2>&1; then
  echo "Run this script from Git Bash on Windows; powershell.exe was not found." >&2
  exit 1
fi

if ! command -v cygpath >/dev/null 2>&1; then
  echo "cygpath was not found. Run this script from Git Bash for Windows." >&2
  exit 1
fi

echo "Launching the Qoin Windows development toolchain installer..."
PS_SCRIPT_WIN="$(cygpath -w "${PS_SCRIPT}")"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "${PS_SCRIPT_WIN}" "$@"
