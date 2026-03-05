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
3. Start the bridge script from PowerShell:
   ```powershell
   .\scripts\obs-text-bridge.ps1
   ```
4. In OBS, add one source:
   - Text (GDI+) -> Read from file -> `overlay\soul-memory.txt`, or
   - Browser Source -> local file `overlay\soul-memory.html` (yellow when stale)
5. Keep the bridge script running while streaming.

## Expected Output

- Fresh: `315126`
- Stale: `[STALE] 315126` (text file) or yellow text (HTML source)

## Notes

- `config/overlay.toml` already includes working candidate chains.
- `GameManagerImp` is auto-resolved from CE-derived AOB logic.
- Bridge regression test:
  ```powershell
  .\scripts\test-obs-text-bridge.ps1
  ```
