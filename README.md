# Soul Memory OBS Overlay

Native OBS source plugin + helper process for showing Dark Souls II SOTFS soul memory in OBS.

## Overview

- `overlay-helper` reads process memory from `DarkSoulsII.exe` and publishes newline-delimited JSON over local named pipe IPC.
- `overlay-plugin` is a native OBS source (`obs-wrapper`) that reads IPC messages and renders text inside OBS.
- Shared protocol and config models live in `overlay-proto`.

## Build

### Build from Windows PowerShell

Run from repository root:

```powershell
cargo.exe build -p overlay-plugin --target x86_64-pc-windows-msvc --release
cargo.exe build -p overlay-helper --target x86_64-pc-windows-msvc --release
```

### Build from WSL (using Windows toolchain)

```bash
WIN_PATH="$(wslpath -w "$PWD")"
powershell.exe -NoProfile -Command "Set-Location '$WIN_PATH'; cargo.exe build -p overlay-plugin --target x86_64-pc-windows-msvc --release"
powershell.exe -NoProfile -Command "Set-Location '$WIN_PATH'; cargo.exe build -p overlay-helper --target x86_64-pc-windows-msvc --release"
```

## Install

1. Build release artifacts (commands above).
2. Copy plugin DLL:
   - from `target\x86_64-pc-windows-msvc\release\overlay_plugin.dll`
   - to `%ProgramFiles%\obs-studio\obs-plugins\64bit\overlay_plugin.dll`
3. Put helper + config in a stable folder (example `C:\SoulMemoryOverlay\`):
   - `overlay-helper.exe`
   - `config\overlay.toml`
4. Edit `config\overlay.toml` for your machine if needed:
   - `process.exe` should remain `DarkSoulsII.exe`
   - `ipc.pipe_name` must match between helper and OBS source (default `SoulMemoryOverlay`)
5. Start `overlay-helper.exe` before streaming.

## Run

1. Launch DS2 and load into a character/world.
2. Start `overlay-helper.exe`.
3. Open OBS and add source: `Sources -> + -> Soul Memory Overlay`.
4. In source properties:
   - `Label Prefix` (default `Soul Memory: `)
   - `Pipe Name` (default `SoulMemoryOverlay`)
   - `Reconnect Interval (ms)`
5. Confirm the source text updates with current soul memory.

## Troubleshoot

- **Source shows `Disconnected`**
  - Verify helper is running.
  - Verify `Pipe Name` in OBS matches `ipc.pipe_name` in `overlay.toml`.
- **Source shows `Error: ...`**
  - Check helper console logs for memory read details.
  - Make sure DS2 is running and character is fully loaded.
- **Windows MSVC build fails with `link.exe not found` in WSL**
  - Run build using Windows PowerShell (`cargo.exe ...`) instead of Linux cargo.
- **Plugin not visible in OBS source list**
  - Re-check plugin DLL path under OBS install and restart OBS.

## Development Verification

```bash
cargo test --workspace
cargo build -p overlay-plugin --release
cargo check -p overlay-plugin --target x86_64-pc-windows-msvc
```

## License

This project is GPL-2.0-only (`LICENSE`).

OBS plugins and `obs-wrapper` usage require GPL-compatible distribution; this repository keeps the plugin/helper stack under GPL-2.0-only to remain license-compatible.
