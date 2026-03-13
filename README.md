# Soul Memory OBS Overlay

Soul Memory OBS Overlay is a Windows OBS plugin that shows your Dark Souls II: Scholar of the First Sin soul memory as a live source in OBS.

## One-Click Install (Windows)

1. Download the latest installer: `SoulMemoryOverlay-<version>-setup.exe`.
2. Close OBS.
3. Run the installer normally (not as Administrator).
4. Open OBS and add source: `Sources -> + -> Soul Memory Overlay`.
5. Launch Dark Souls II and load into a character.

The plugin auto-starts the helper process when the source is active.

### Installer Modes (Windows)

- The installer now has two explicit modes:
  - **Standard (recommended)** installs to `C:\ProgramData\obs-studio\plugins\soul-memory-obs-overlay`.
  - **Portable/custom OBS** installs to OBS root layout (`obs-plugins/64bit` and `data/obs-plugins/...`).
- In Portable/custom mode, the installer validates that the selected folder contains `bin\64bit\obs64.exe`.
- If OBS auto-detection fails in Portable/custom mode, use **Browse** and select the correct OBS folder manually.
- During Standard-mode upgrades, the installer removes this plugin's legacy OBS-root files from the detected OBS installation to prevent duplicate loads.

## Uninstall

Use either method:

1. Windows Settings -> Apps -> Installed Apps -> `Soul Memory OBS Overlay` -> Uninstall.
2. Or run `obs-overlay-uninstall.exe` from the mode-specific install folder:
   - Standard mode: `C:\ProgramData\obs-studio\plugins\soul-memory-obs-overlay`
   - Portable/custom mode: your OBS installation directory

Uninstall removes:

- Standard mode: `bin/64bit/overlay_plugin.dll`, `bin/64bit/overlay-helper.exe`, `data/config/overlay.toml`, `data/locale/en-US.ini`
- Portable/custom mode: `obs-plugins/64bit/overlay_plugin.dll`, `obs-plugins/64bit/overlay-helper.exe`, `data/obs-plugins/soul-memory-obs-overlay/*`

Migration behavior:

- If legacy OBS-root files from this plugin are detected during Standard-mode install/uninstall flows, they are cleaned up to avoid side-by-side duplicate plugin copies.

## Updates

- Update checks are non-blocking and never block OBS startup.
- If an update cannot be checked (offline/firewall), the plugin continues working.
- To update, run the newest installer over the existing installation.

## Troubleshooting

- **Source shows `Disconnected`**
  - Start Dark Souls II and load into a world/character.
  - Remove and re-add the source in OBS.
- **Source shows `Error: ...`**
  - Confirm DS2 process name is `DarkSoulsII.exe`.
  - Verify anti-cheat or security software is not blocking memory access.
- **Source not visible in OBS source list**
  - Restart OBS after install.
  - Confirm `overlay_plugin.dll` exists in OBS `obs-plugins/64bit`.
- **Installer cannot find OBS automatically in Portable/custom mode**
  - Click **Browse** and select the folder that contains `bin\64bit\obs64.exe`.

## CI Code Signing Setup (Maintainers)

To configure GitHub Actions signing secrets/variables without putting passwords into files:

- WSL wrapper for PowerShell setup script (recommended for WSL users):
  - `bash scripts/setup-codesign-secrets-wsl.sh -CreateTestCert -Repo "chozandrias76/soul-memory-obs-overlay"`
- Bash (Linux/WSL, existing `.pfx`):
  - `bash scripts/setup-codesign-secrets.sh --pfx "/path/to/codesign.pfx"`
- PowerShell 7 (Windows, existing `.pfx`):
  - `pwsh -NoLogo -NoProfile -ExecutionPolicy Bypass -File scripts/setup-codesign-secrets.ps1 -PfxPath "C:\path\to\codesign.pfx"`
- PowerShell 7 (Windows, generate temporary self-signed test cert and configure secrets):
  - `pwsh -NoLogo -NoProfile -ExecutionPolicy Bypass -File scripts/setup-codesign-secrets.ps1 -CreateTestCert`

Check whether required names exist:

- `bash scripts/setup-codesign-secrets-wsl.sh -Check -Repo "chozandrias76/soul-memory-obs-overlay"`
- `bash scripts/setup-codesign-secrets.sh --check`
- `pwsh -NoLogo -NoProfile -ExecutionPolicy Bypass -File scripts/setup-codesign-secrets.ps1 -Check`

Local signing harness (fast validation without running full release workflow):

- Sign a local EXE/DLL with an ephemeral self-signed cert and write `signing-report.json`:
  - `pwsh -NoLogo -NoProfile -ExecutionPolicy Bypass -File scripts/run-signing-harness.ps1 -OutputDir artifacts/signing-harness -Files "C:\path\to\overlay-helper.exe"`
- Quick smoke test using a copied `notepad.exe` sample:
  - `pwsh -NoLogo -NoProfile -ExecutionPolicy Bypass -File scripts/run-signing-harness.ps1 -OutputDir artifacts/signing-harness -UseSystemNotepadSample`

Workflow dispatch note:

- `windows-release` now defaults to **strict trusted-signing mode**. A self-signed cert will fail signing unless you explicitly set workflow input `allow_self_signed=true` (testing only).
- Microsoft Defender scan entries with `scan_status: "skipped"` now fail the run.

## License

GPL-2.0-only (`LICENSE`). OBS plugin distribution remains GPL-compatible.
