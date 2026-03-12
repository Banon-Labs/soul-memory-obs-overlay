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

## License

GPL-2.0-only (`LICENSE`). OBS plugin distribution remains GPL-compatible.
