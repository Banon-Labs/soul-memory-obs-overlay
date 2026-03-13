# Soul Memory Overlay - Super Simple Setup (OBS)

If you can install a normal Windows app, you can set this up.

## What This Is

This adds a new OBS source named **Soul Memory Overlay**.
When Dark Souls II: Scholar of the First Sin is running, the source shows your Soul Memory value.

## Before You Start

- You need Windows
- You need OBS Studio installed
- You need Dark Souls II: Scholar of the First Sin
- Close OBS before installing

## Install (3 minutes)

1. Download `SoulMemoryOverlay-<version>-setup.exe`
2. Run it normally (do **not** run as Administrator)
3. Finish the installer
   - Choose **Standard (recommended)** to install to `C:\ProgramData\obs-studio\plugins\soul-memory-obs-overlay`.
   - Choose **Portable/custom OBS** only when needed, then select the OBS folder that contains `bin\64bit\obs64.exe`.
   - If you point at an OBS install folder, use **Portable/custom OBS**. Do not use Standard mode for an OBS root path.
   - In Portable/custom mode, the installer will not continue until a valid OBS folder is selected.
4. Open OBS Studio
5. In **Sources**, click **+**
6. Choose **Soul Memory Overlay**

If Windows asks for admin permissions, allow the installer to continue.

## Make It Show Data

1. Start Dark Souls II
2. Load into a character/world
3. In OBS, the source should switch from `Disconnected` to your value

## If It Says Disconnected

Try these in order:

1. Confirm the game process is `DarkSoulsII.exe`
2. Remove and re-add the source in OBS
3. Restart OBS
4. Make sure antivirus is not blocking the overlay files

## If Source Type Is Missing

If you do not see **Soul Memory Overlay** in OBS after relaunch:

1. Confirm files exist for your selected mode:
   - Standard mode:
     - `C:\ProgramData\obs-studio\plugins\soul-memory-obs-overlay\bin\64bit\soul-memory-obs-overlay.dll`
     - `C:\ProgramData\obs-studio\plugins\soul-memory-obs-overlay\bin\64bit\overlay-helper.exe`
   - Portable/custom mode:
     - `<OBS folder>\obs-plugins\64bit\overlay_plugin.dll`
     - `<OBS folder>\obs-plugins\64bit\overlay-helper.exe`
2. In OBS, open `Help -> Log Files -> View Current Log`.
3. In `%AppData%\obs-studio\logs\`, inspect the newest log for:
   - `soul-memory-obs-overlay.dll`
   - `overlay_plugin.dll`
   - `soul-memory-obs-overlay`
   - `Failed to load module`

## Update Later

Run the newest installer over your current install. No manual uninstall required.

## Remove It

Use either:

- Windows Settings -> Apps -> Installed Apps -> Soul Memory OBS Overlay -> Uninstall
- `obs-overlay-uninstall.exe` in your mode-specific install folder (ProgramData plugin folder for Standard mode, OBS folder for Portable/custom mode)

## OBS Basics (Quick)

- Sources are per scene (add it to each scene where you want it)
- Move source order up/down to control what appears on top
- Use the eye icon to hide/show quickly

If all else fails, restart OBS and re-add the source.
