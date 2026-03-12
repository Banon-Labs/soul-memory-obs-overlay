#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ps1_script="$script_dir/setup-codesign-secrets.ps1"

if ! command -v pwsh.exe >/dev/null 2>&1; then
  echo "pwsh.exe not found. Install PowerShell 7 on Windows and ensure WSL interop PATH is enabled." >&2
  exit 1
fi

if ! command -v wslpath >/dev/null 2>&1; then
  echo "wslpath is required when running from WSL." >&2
  exit 1
fi

if [[ ! -f "$ps1_script" ]]; then
  echo "Missing script: $ps1_script" >&2
  exit 1
fi

ps1_windows_path="$(wslpath -w "$ps1_script")"

exec pwsh.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "$ps1_windows_path" "$@"
