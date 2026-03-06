# Soul Memory OBS Overlay (Quick Start)

Shows Dark Souls II SOTFS Soul Memory in OBS.

## What Works Right Now

Use the helper + bridge script with OBS Text/Browser sources.

## 5-Step Setup

1. Build the Windows helper:
   ```bash
   cargo build -p overlay-helper --target x86_64-pc-windows-msvc
   ```
2. Make sure DS2 is running and a character is loaded.
3. Start the local overlay server (WSL):
   ```bash
   python3 scripts/obs-overlay-server.py --host 127.0.0.1 --port 8937
   ```
4. In OBS, add a Browser Source URL:
   - `http://127.0.0.1:8937/soul-memory.html`
5. Keep the server process running while streaming.

## Expected Output

- Fresh: `315126`
- Stale: `[STALE] 315126` in yellow

## Notes

- `config/overlay.toml` already includes working candidate chains.
- `GameManagerImp` is auto-resolved from CE-derived AOB logic.
- `scripts/obs-text-bridge.ps1` remains available as legacy file-output mode.
- Bridge regression test:
  ```powershell
  .\scripts\test-obs-text-bridge.ps1
  ```
