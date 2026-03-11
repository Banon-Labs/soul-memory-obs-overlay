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
4. Open OBS Studio
5. In **Sources**, click **+**
6. Choose **Soul Memory Overlay**

If Windows asks for admin permissions, cancel and use the no-admin/manual route in this release package.

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

## Update Later

Run the newest installer over your current install. No manual uninstall required.

## Remove It

Use either:

- Windows Settings -> Apps -> Installed Apps -> Soul Memory OBS Overlay -> Uninstall
- `obs-overlay-uninstall.exe` in your OBS folder

## OBS Basics (Quick)

- Sources are per scene (add it to each scene where you want it)
- Move source order up/down to control what appears on top
- Use the eye icon to hide/show quickly

If all else fails, restart OBS and re-add the source.
