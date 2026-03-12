# ProgramData Migration Evaluation (Windows Installer)

This document evaluates whether the Windows installer should move from the current OBS-root layout to the ProgramData plugin layout.

## Current Project Behavior

- Installer writes plugin/runtime files to OBS root layout:
  - `installer/overlay-installer.nsi:117` -> `$INSTDIR\obs-plugins\64bit`
  - `installer/overlay-installer.nsi:121` -> `$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\config`
  - `installer/overlay-installer.nsi:124` -> `$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\locale`
- Uninstall currently removes those OBS-root paths and `obs-overlay-uninstall.exe` from `$INSTDIR`:
  - `installer/overlay-installer.nsi:128`
  - `installer/overlay-installer.nsi:132`
  - `installer/overlay-installer.nsi:139`
- Plugin runtime helper/config lookup currently relies on `current_exe()` plus relative candidates and APPDATA override:
  - `crates/overlay-plugin/src/lib.rs:265`
  - `crates/overlay-plugin/src/lib.rs:270`
  - `crates/overlay-plugin/src/lib.rs:300`
  - `crates/overlay-plugin/src/lib.rs:306`
- User docs currently state update behavior as rerunning installer over existing install:
  - `README.md:40`

## External Evidence

- OBS Plugins Guide (official):
  - Recommends `C:\ProgramData\obs-studio\plugins` on Windows.
  - Marks `C:\Program Files\obs-studio\obs-plugins\64bit` as legacy and states it will stop working in a future OBS version.
  - Notes custom install/portable setups need targeting the custom location's `data/plugins` folder.
  - Notes `OBS_PLUGINS_PATH` and `OBS_PLUGINS_DATA_PATH` can define custom plugin directories.
  - Source: `https://obsproject.com/kb/plugins-guide`
- OBS maintainer forum guidance (R1CH): ProgramData plugin loading is recommended, but ProgramData is not checked when OBS runs in portable mode.
  - Source: `https://obsproject.com/forum/threads/is-it-still-possible-to-store-plugin-files-in-a-separate-folder.178714/`
- OBS upstream Windows defaults still include legacy module patterns (`../../obs-plugins/64bit`, `../../data/obs-plugins/%module%`), indicating compatibility paths still exist in current releases.
  - Source: `https://raw.githubusercontent.com/obsproject/obs-studio/master/libobs/obs-windows.c`
- Maintained plugin installer example (StreamFX) uses mode-aware installation:
  - System mode defaults to ProgramData plugin root.
  - User mode configures `OBS_PLUGINS_PATH` / `OBS_PLUGINS_DATA_PATH` env vars.
  - Portable mode writes to portable OBS tree instead of ProgramData.
  - Source: `https://raw.githubusercontent.com/Vhonowslend/StreamFX-Public/root/templates/windows/installer.iss.in`

## Risk Assessment

### High Risk (if migrated immediately)

1. **Runtime path mismatch risk**
   - Current helper/config candidate paths are tied to existing relative assumptions (`current_exe()` + repo-specific tree names).
   - A direct installer-only switch to ProgramData can break helper/config resolution unless runtime lookup is expanded first.

2. **Portable mode regression risk**
   - ProgramData is not checked in OBS portable mode.
   - ProgramData-only installs would fail for portable OBS users.

3. **Duplicate-load/upgrade risk**
   - If legacy and ProgramData installs coexist during upgrades, OBS may load unexpected/duplicate versions unless migration cleanup is explicit.

### Medium Risk

1. **Uninstall path divergence**
   - Existing uninstall logic assumes OBS-root paths.
   - ProgramData migration requires uninstall path updates and backward cleanup behavior.

2. **Installer privilege model changes**
   - ProgramData/system installs typically require elevation.
   - Current docs include a no-admin/manual route; migration must preserve a workable user/portable path.

## Decision

Do **not** switch to ProgramData-only in one step.

Adopt a **staged migration**:

1. First make runtime path resolution dual-layout compatible (legacy + ProgramData + APPDATA override precedence).
2. Then add installer install modes with explicit non-portable (ProgramData) and portable/custom behavior.
3. Then migrate default non-portable installs to ProgramData, with upgrade cleanup.

## Proposed Migration Plan

### Phase 1: Runtime Compatibility (no default installer destination change)

- Extend helper/config lookup to include ProgramData plugin layout candidates.
- Keep existing legacy candidates to avoid breaking current installs.
- Keep APPDATA override precedence deterministic.
- Add diagnostics/logging to make selected helper/config paths visible.

### Phase 2: Installer Mode Support

- Introduce installer mode split:
  - **Standard install**: ProgramData plugin layout.
  - **Portable/custom install**: OBS-root compatible layout for portable mode.
- Gate behavior on explicit mode choice and/or portable detection signal.
- Update uninstall to clean both known legacy and new destination paths when upgrading.

### Phase 3: Default Migration + Cleanup

- Switch default non-portable destination to ProgramData.
- Perform upgrade migration cleanup from legacy OBS-root locations.
- Keep portable path support documented and test-covered.

## Validation Matrix (must pass before default switch)

1. Standard OBS + legacy-only install: plugin/helper/config still resolve.
2. Standard OBS + ProgramData-only install: plugin/helper/config resolve.
3. Standard OBS + both layouts present: no duplicate-load behavior.
4. APPDATA override present: override path still takes precedence.
5. Upgrade legacy -> ProgramData: no orphaned active plugin binaries.
6. Uninstall after migrated install: ProgramData files removed; expected user config retention behavior documented.
7. OBS portable mode + ProgramData-only install: expected non-load behavior documented.
8. OBS portable mode + portable layout install: plugin loads correctly.

## Follow-up Work Items

Implementation should be tracked in linked beads issues for:

1. Runtime dual-layout path resolution and logging.
2. Installer mode split and migration-safe upgrade/uninstall behavior.
3. Portable-mode verification harness/checklist coverage.
