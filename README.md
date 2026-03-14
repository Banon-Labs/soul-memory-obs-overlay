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
- Mode/path rule: OBS installation folders (`...\obs-studio` containing `bin\64bit\obs64.exe`) should use **Portable/custom** mode. Standard mode is for ProgramData plugin layout paths.
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

- Standard mode: `bin/64bit/soul-memory-obs-overlay.dll`, `bin/64bit/overlay-helper.exe`, `data/config/overlay.toml`, `data/locale/en-US.ini`
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
  - Confirm plugin files exist in the selected install target:
    - Standard mode: `C:\ProgramData\obs-studio\plugins\soul-memory-obs-overlay\bin\64bit\soul-memory-obs-overlay.dll`
    - Portable/custom mode: `<OBS folder>\obs-plugins\64bit\overlay_plugin.dll`
  - Check `%AppData%\obs-studio\logs\` (or `Help -> Log Files -> View Current Log`) for `soul-memory-obs-overlay.dll`, `overlay_plugin.dll`, `soul-memory-obs-overlay`, or `Failed to load module` lines.
- **Installer cannot find OBS automatically in Portable/custom mode**
  - Click **Browse** and select the folder that contains `bin\64bit\obs64.exe`.

## CI Code Signing Setup (Maintainers)

To configure GitHub Actions signing for release builds:

- Managed trusted signing (recommended for strict mode):
  - Configure secrets: `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_SUBSCRIPTION_ID`.
  - Configure variables: `WINDOWS_TRUSTED_SIGNING_ENDPOINT`, `WINDOWS_TRUSTED_SIGNING_ACCOUNT_NAME`, `WINDOWS_TRUSTED_SIGNING_CERT_PROFILE_NAME`.
  - The `windows-release` workflow auto-selects managed mode when all six values are present.
- PFX signing (fallback/testing path):
  - WSL wrapper for PowerShell setup script (recommended for WSL users):
    - `bash scripts/setup-codesign-secrets-wsl.sh -CreateTestCert -Repo "Banon-Labs/soul-memory-obs-overlay"`
  - Bash (Linux/WSL, existing `.pfx`):
    - `bash scripts/setup-codesign-secrets.sh --pfx "/path/to/codesign.pfx"`
  - PowerShell 7 (Windows, existing `.pfx`):
    - `pwsh -NoLogo -NoProfile -ExecutionPolicy Bypass -File scripts/setup-codesign-secrets.ps1 -PfxPath "C:\path\to\codesign.pfx"`
  - PowerShell 7 (Windows, generate temporary self-signed test cert and configure secrets):
    - `pwsh -NoLogo -NoProfile -ExecutionPolicy Bypass -File scripts/setup-codesign-secrets.ps1 -CreateTestCert`
    - When `-PfxPath` is omitted, the test cert is saved to `C:\Users\<username>\.codesign\soulmemory-test-signing.pfx`.

Check whether signing configuration names exist (managed + PFX):

- `bash scripts/setup-codesign-secrets-wsl.sh -Check -Repo "Banon-Labs/soul-memory-obs-overlay"`
- `bash scripts/setup-codesign-secrets.sh --check`
- `pwsh -NoLogo -NoProfile -ExecutionPolicy Bypass -File scripts/setup-codesign-secrets.ps1 -Check`

Local signing harness (fast validation without running full release workflow):

- Sign a local EXE/DLL with an ephemeral self-signed cert and write `signing-report.json`:
  - `pwsh -NoLogo -NoProfile -ExecutionPolicy Bypass -File scripts/run-signing-harness.ps1 -OutputDir artifacts/signing-harness -Files "C:\path\to\overlay-helper.exe"`
- Quick smoke test using a copied `notepad.exe` sample:
  - `pwsh -NoLogo -NoProfile -ExecutionPolicy Bypass -File scripts/run-signing-harness.ps1 -OutputDir artifacts/signing-harness -UseSystemNotepadSample`

Workflow dispatch note:

- `windows-release` now defaults to **strict trusted-signing mode**.
- In strict mode, self-signed certs fail unless you explicitly set workflow input `allow_self_signed=true` (testing only).
- Signing mode selection is automatic: managed trusted-signing is used when fully configured; otherwise the workflow falls back to PFX mode.
- `windows-release` accepts `runner_labels_json` for selecting a Defender-capable runner profile (example: `["self-hosted","windows","x64","defender-enabled"]`).
- Microsoft Defender scan entries with `scan_status: "skipped"` now fail the run.
- The workflow also fails on `scan_status: "unscannable-environment"` (for example, hosted runners where Defender is disabled or full-drive exclusions prevent meaningful scans). Use a Defender-enabled Windows runner without broad root-drive exclusions for release validation.

## License

GPL-2.0-only (`LICENSE`). OBS plugin distribution remains GPL-compatible.
