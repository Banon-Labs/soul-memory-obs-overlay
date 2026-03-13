!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"
!include "nsDialogs.nsh"

!ifndef PROJECT_ROOT
!define PROJECT_ROOT "."
!endif

!define APP_NAME "Soul Memory OBS Overlay"
!define COMPANY_NAME "soul-memory-obs-overlay"
!define STANDARD_DLL_NAME "soul-memory-obs-overlay.dll"
!define PORTABLE_DLL_NAME "overlay_plugin.dll"
!define HELPER_EXE "overlay-helper.exe"
!define PRODUCT_VERSION "0.1.3"
!define OBS_UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\OBS Studio_is1"
!define INSTALL_MODE_STANDARD "standard"
!define INSTALL_MODE_PORTABLE "portable"

Var ObsPathDetected
Var DetectedObsDir
Var ProgramDataDir
Var InstallMode
Var ModePageDialog
Var ModeStandardRadio
Var ModePortableRadio
Var DirectoryPageDialog
Var DirectoryPathInput
Var DirectoryBrowseButton
Var DirectoryGuideLabel
Var DirectoryStatusLabel
Var DirectoryStatusUpdateGuard

!define STATUS_COLOR_INVALID 0x0000FF
!define STATUS_COLOR_VALID 0x00AA00

Name "${APP_NAME} ${PRODUCT_VERSION}"
OutFile "SoulMemoryOverlay-${PRODUCT_VERSION}-setup.exe"
InstallDir "$PROGRAMFILES64\obs-studio"
RequestExecutionLevel admin

Page custom InstallModePageCreate InstallModePageLeave
Page custom DirectoryPageCreate DirectoryPageLeave
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Function .onInit
  StrCpy $InstallMode "${INSTALL_MODE_STANDARD}"
  StrCpy $ObsPathDetected "0"
  StrCpy $DetectedObsDir ""
  ExpandEnvStrings $ProgramDataDir "%ProgramData%"
  StrCmp $ProgramDataDir "" 0 +2
    StrCpy $ProgramDataDir "C:\ProgramData"
  Call DetectObsInstallDir
FunctionEnd

Function InstallModePageCreate
  nsDialogs::Create 1018
  Pop $ModePageDialog
  StrCmp $ModePageDialog "error" 0 +2
    Abort

  ${NSD_CreateLabel} 0 0 100% 18u "Choose how to install Soul Memory OBS Overlay."
  Pop $0

  ${NSD_CreateRadioButton} 0 22u 100% 12u "Standard (recommended) - install to ProgramData plugin layout"
  Pop $ModeStandardRadio
  ${NSD_CreateLabel} 10u 36u 100% 20u "Use for regular OBS installs. Files go to C:\\ProgramData\\obs-studio\\plugins\\soul-memory-obs-overlay."
  Pop $0

  ${NSD_CreateRadioButton} 0 62u 100% 12u "Portable/custom OBS - install to OBS root layout"
  Pop $ModePortableRadio
  ${NSD_CreateLabel} 10u 76u 100% 28u "Use for OBS portable/custom folders. You must select the OBS folder that contains bin\\64bit\\obs64.exe."
  Pop $0

  StrCmp $InstallMode "${INSTALL_MODE_PORTABLE}" 0 +3
    ${NSD_Check} $ModePortableRadio
    Goto done
  ${NSD_Check} $ModeStandardRadio
done:
  nsDialogs::Show
FunctionEnd

Function InstallModePageLeave
  ${NSD_GetState} $ModePortableRadio $0
  StrCmp $0 1 0 +3
    StrCpy $InstallMode "${INSTALL_MODE_PORTABLE}"
    Return
  StrCpy $InstallMode "${INSTALL_MODE_STANDARD}"
FunctionEnd

Function NormalizePortableInstallDir
  IfFileExists "$INSTDIR\bin\64bit\obs64.exe" valid 0
  IfFileExists "$INSTDIR\obs64.exe" 0 invalid
  ${GetParent} "$INSTDIR" $1
  ${GetParent} "$1" $2
  IfFileExists "$2\bin\64bit\obs64.exe" 0 invalid
  StrCpy $INSTDIR "$2"
  Goto valid

invalid:
  StrCpy $0 "0"
  Return

valid:
  StrCpy $0 "1"
FunctionEnd

Function DetectObsRootLikePath
  IfFileExists "$INSTDIR\bin\64bit\obs64.exe" found 0
  IfFileExists "$INSTDIR\obs64.exe" 0 not_found
  ${GetParent} "$INSTDIR" $1
  ${GetParent} "$1" $2
  IfFileExists "$2\bin\64bit\obs64.exe" 0 not_found

found:
  StrCpy $0 "1"
  Return

not_found:
  StrCpy $0 "0"
FunctionEnd

Function UpdateDirectoryStatus
  StrCmp $DirectoryStatusUpdateGuard "1" already_updating
  StrCpy $DirectoryStatusUpdateGuard "1"

  ${NSD_GetText} $DirectoryPathInput $INSTDIR
  StrCmp $InstallMode "${INSTALL_MODE_PORTABLE}" portable_status standard_status

standard_status:
  Call DetectObsRootLikePath
  StrCmp $0 "1" standard_obs_root standard_path_check

standard_obs_root:
  ${NSD_SetText} $DirectoryStatusLabel "This looks like an OBS folder. Use Portable/custom mode for OBS install directories."
  SetCtlColors $DirectoryStatusLabel ${STATUS_COLOR_INVALID} transparent
  Goto done

standard_path_check:
  StrLen $0 $INSTDIR
  IntCmp $0 3 standard_invalid standard_invalid standard_valid

standard_invalid:
  ${NSD_SetText} $DirectoryStatusLabel "Select a plugin folder path to continue."
  SetCtlColors $DirectoryStatusLabel ${STATUS_COLOR_INVALID} transparent
  Goto done

standard_valid:
  ${NSD_SetText} $DirectoryStatusLabel "Standard mode target looks valid (plugin files go to this folder's bin\\64bit and data subfolders)."
  SetCtlColors $DirectoryStatusLabel ${STATUS_COLOR_VALID} transparent
  Goto done

portable_status:
  Call NormalizePortableInstallDir
  StrCmp $0 "1" portable_valid portable_invalid

portable_valid:
  ${NSD_SetText} $DirectoryStatusLabel "Valid OBS folder detected."
  SetCtlColors $DirectoryStatusLabel ${STATUS_COLOR_VALID} transparent
  ${NSD_GetText} $DirectoryPathInput $1
  StrCmp $1 $INSTDIR done 0
  ${NSD_SetText} $DirectoryPathInput $INSTDIR
  Goto done

portable_invalid:
  ${NSD_SetText} $DirectoryStatusLabel "Select the OBS folder that contains bin\\64bit\\obs64.exe."
  SetCtlColors $DirectoryStatusLabel ${STATUS_COLOR_INVALID} transparent

done:
  StrCpy $DirectoryStatusUpdateGuard "0"
  Return

already_updating:
FunctionEnd

Function OnDirectoryPathChange
  Pop $0
  Call UpdateDirectoryStatus
FunctionEnd

Function OnDirectoryBrowse
  Pop $0
  ${NSD_GetText} $DirectoryPathInput $0
  nsDialogs::SelectFolderDialog "Select installation folder" $0
  Pop $1
  StrCmp $1 "error" done 0
  ${NSD_SetText} $DirectoryPathInput $1

done:
  Call UpdateDirectoryStatus
FunctionEnd

Function DirectoryPageCreate
  Call DirectoryPagePre
  StrCpy $DirectoryStatusUpdateGuard "0"

  nsDialogs::Create 1018
  Pop $DirectoryPageDialog
  StrCmp $DirectoryPageDialog "error" 0 +2
    Abort

  ${NSD_CreateLabel} 0 0 100% 14u "Choose installation folder."
  Pop $0

  ${NSD_CreateLabel} 0 18u 100% 24u ""
  Pop $DirectoryGuideLabel

  ${NSD_CreateDirRequest} 0 46u 78% 12u "$INSTDIR"
  Pop $DirectoryPathInput
  ${NSD_OnChange} $DirectoryPathInput OnDirectoryPathChange

  ${NSD_CreateBrowseButton} 80% 46u 20% 12u "Browse..."
  Pop $DirectoryBrowseButton
  ${NSD_OnClick} $DirectoryBrowseButton OnDirectoryBrowse

  ${NSD_CreateLabel} 0 62u 100% 24u ""
  Pop $DirectoryStatusLabel

  StrCmp $InstallMode "${INSTALL_MODE_PORTABLE}" 0 standard_guide
  ${NSD_SetText} $DirectoryGuideLabel "Portable/custom mode: select the OBS folder that contains bin\\64bit\\obs64.exe."
  Goto guide_done

standard_guide:
  ${NSD_SetText} $DirectoryGuideLabel "Standard mode installs to ProgramData plugin layout by default."

guide_done:
  Call UpdateDirectoryStatus
  nsDialogs::Show
FunctionEnd

Function DirectoryPageLeave
  ${NSD_GetText} $DirectoryPathInput $INSTDIR
  StrCmp $InstallMode "${INSTALL_MODE_PORTABLE}" portable_leave standard_leave

standard_leave:
  Call DetectObsRootLikePath
  StrCmp $0 "1" standard_obs_root_leave standard_leave_path_check

standard_obs_root_leave:
  ${NSD_SetText} $DirectoryStatusLabel "That path is an OBS folder. Switch to Portable/custom mode for OBS install directories."
  SetCtlColors $DirectoryStatusLabel ${STATUS_COLOR_INVALID} transparent
  Abort

standard_leave_path_check:
  StrLen $0 $INSTDIR
  IntCmp $0 3 standard_invalid standard_invalid directory_valid

standard_invalid:
  ${NSD_SetText} $DirectoryStatusLabel "Select a plugin folder path to continue."
  SetCtlColors $DirectoryStatusLabel ${STATUS_COLOR_INVALID} transparent
  Abort

portable_leave:
  Call NormalizePortableInstallDir
  StrCmp $0 "1" directory_valid portable_invalid

portable_invalid:
  ${NSD_SetText} $DirectoryStatusLabel "Portable/custom mode requires an OBS folder containing bin\\64bit\\obs64.exe."
  SetCtlColors $DirectoryStatusLabel ${STATUS_COLOR_INVALID} transparent
  Abort

directory_valid:
FunctionEnd

Function DirectoryPagePre
  StrCmp $InstallMode "${INSTALL_MODE_PORTABLE}" portable_mode standard_mode

standard_mode:
  StrCpy $INSTDIR "$ProgramDataDir\obs-studio\plugins\soul-memory-obs-overlay"
  Return

portable_mode:
  StrCmp $ObsPathDetected "1" detected not_detected

not_detected:
  StrCpy $INSTDIR "$PROGRAMFILES64\obs-studio"
  Goto portable_text

detected:
  StrCpy $INSTDIR $DetectedObsDir

portable_text:
FunctionEnd

Function DetectObsInstallDir
  StrCpy $ObsPathDetected "0"
  StrCpy $DetectedObsDir ""

  SetRegView 64
  ReadRegStr $0 HKLM "${OBS_UNINSTALL_KEY}" "InstallLocation"
  StrCmp $0 "" +2 0
    Goto FoundInstallDir
  ReadRegStr $0 HKLM "${OBS_UNINSTALL_KEY}" "Inno Setup: App Path"
  StrCmp $0 "" +2 0
    Goto FoundInstallDir
  ReadRegStr $0 HKCU "${OBS_UNINSTALL_KEY}" "InstallLocation"
  StrCmp $0 "" +2 0
    Goto FoundInstallDir
  ReadRegStr $0 HKCU "${OBS_UNINSTALL_KEY}" "Inno Setup: App Path"
  StrCmp $0 "" +2 0
    Goto FoundInstallDir

  SetRegView 32
  ReadRegStr $0 HKLM "${OBS_UNINSTALL_KEY}" "InstallLocation"
  StrCmp $0 "" +2 0
    Goto FoundInstallDir
  ReadRegStr $0 HKLM "${OBS_UNINSTALL_KEY}" "Inno Setup: App Path"
  StrCmp $0 "" +2 0
    Goto FoundInstallDir
  ReadRegStr $0 HKCU "${OBS_UNINSTALL_KEY}" "InstallLocation"
  StrCmp $0 "" +2 0
    Goto FoundInstallDir
  ReadRegStr $0 HKCU "${OBS_UNINSTALL_KEY}" "Inno Setup: App Path"
  StrCmp $0 "" +2 0
    Goto FoundInstallDir

  IfFileExists "D:\Obs\bin\64bit\obs64.exe" 0 +3
    StrCpy $INSTDIR "D:\Obs"
    StrCpy $DetectedObsDir "D:\Obs"
    StrCpy $ObsPathDetected "1"
    Return
  IfFileExists "D:\obs-studio\bin\64bit\obs64.exe" 0 +3
    StrCpy $INSTDIR "D:\obs-studio"
    StrCpy $DetectedObsDir "D:\obs-studio"
    StrCpy $ObsPathDetected "1"
    Return
  IfFileExists "$PROGRAMFILES64\obs-studio\bin\64bit\obs64.exe" 0 +3
    StrCpy $INSTDIR "$PROGRAMFILES64\obs-studio"
    StrCpy $DetectedObsDir "$PROGRAMFILES64\obs-studio"
    StrCpy $ObsPathDetected "1"
    Return
  IfFileExists "$PROGRAMFILES\obs-studio\bin\64bit\obs64.exe" 0 +3
    StrCpy $INSTDIR "$PROGRAMFILES\obs-studio"
    StrCpy $DetectedObsDir "$PROGRAMFILES\obs-studio"
    StrCpy $ObsPathDetected "1"
    Return
  Return

FoundInstallDir:
  IfFileExists "$0\bin\64bit\obs64.exe" 0 +4
    StrCpy $INSTDIR "$0"
    StrCpy $DetectedObsDir "$0"
    StrCpy $ObsPathDetected "1"
    Return
  IfFileExists "$0\obs64.exe" 0 +7
    ${GetParent} "$0" $1
    ${GetParent} "$1" $2
    IfFileExists "$2\bin\64bit\obs64.exe" 0 +3
      StrCpy $INSTDIR "$2"
      StrCpy $DetectedObsDir "$2"
      StrCpy $ObsPathDetected "1"
    Return
  Return
FunctionEnd

Function CleanupLegacyObsRootInstall
  StrCmp $ObsPathDetected "1" 0 cleanup_done
  StrCmp $DetectedObsDir "" cleanup_done 0
  StrCmp $DetectedObsDir $INSTDIR cleanup_done 0

  Delete "$DetectedObsDir\obs-plugins\64bit\overlay_plugin.dll"
  Delete "$DetectedObsDir\obs-plugins\64bit\overlay-helper.exe"
  Delete "$DetectedObsDir\data\obs-plugins\soul-memory-obs-overlay\config\overlay.toml"
  Delete "$DetectedObsDir\data\obs-plugins\soul-memory-obs-overlay\locale\en-US.ini"
  RMDir "$DetectedObsDir\data\obs-plugins\soul-memory-obs-overlay\config"
  RMDir "$DetectedObsDir\data\obs-plugins\soul-memory-obs-overlay\locale"
  RMDir "$DetectedObsDir\data\obs-plugins\soul-memory-obs-overlay"
  Delete "$DetectedObsDir\obs-overlay-uninstall.exe"

cleanup_done:
FunctionEnd

Function .onVerifyInstDir
  StrCmp $InstallMode "${INSTALL_MODE_PORTABLE}" portable_verify standard_verify

standard_verify:
  Call DetectObsRootLikePath
  StrCmp $0 "1" invalid_standard 0
  StrLen $0 $INSTDIR
  IntCmp $0 3 invalid_standard invalid_standard standard_continue

standard_continue:
  Goto valid

portable_verify:
  IfFileExists "$INSTDIR\bin\64bit\obs64.exe" valid 0
  IfFileExists "$INSTDIR\obs64.exe" 0 invalid
  ${GetParent} "$INSTDIR" $0
  ${GetParent} "$0" $1
  IfFileExists "$1\bin\64bit\obs64.exe" 0 invalid
  StrCpy $INSTDIR "$1"
  Goto valid

invalid_standard:
  Abort

invalid:
  Abort
valid:
FunctionEnd

Section "Install"
  StrCmp $InstallMode "${INSTALL_MODE_PORTABLE}" install_portable install_standard

install_standard:
  Call CleanupLegacyObsRootInstall

  SetOutPath "$INSTDIR\bin\64bit"
  Delete "$INSTDIR\bin\64bit\${PORTABLE_DLL_NAME}"
  File /oname=${STANDARD_DLL_NAME} "${PROJECT_ROOT}\installer\staging\${PORTABLE_DLL_NAME}"
  File "${PROJECT_ROOT}\installer\staging\${HELPER_EXE}"

  SetOutPath "$INSTDIR\data\config"
  File "${PROJECT_ROOT}\installer\staging\overlay.toml"

  SetOutPath "$INSTDIR\data\locale"
  File "${PROJECT_ROOT}\installer\staging\en-US.ini"

  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\obs-overlay-uninstall.exe"
  Goto install_done

install_portable:
  SetOutPath "$INSTDIR\obs-plugins\64bit"
  File "${PROJECT_ROOT}\installer\staging\${PORTABLE_DLL_NAME}"
  File "${PROJECT_ROOT}\installer\staging\${HELPER_EXE}"

  SetOutPath "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\config"
  File "${PROJECT_ROOT}\installer\staging\overlay.toml"

  SetOutPath "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\locale"
  File "${PROJECT_ROOT}\installer\staging\en-US.ini"

  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\obs-overlay-uninstall.exe"

install_done:
SectionEnd

Function un.DetectObsInstallDir
  StrCpy $ObsPathDetected "0"
  StrCpy $DetectedObsDir ""

  SetRegView 64
  ReadRegStr $0 HKLM "${OBS_UNINSTALL_KEY}" "InstallLocation"
  StrCmp $0 "" +2 0
    Goto un.FoundInstallDir
  ReadRegStr $0 HKLM "${OBS_UNINSTALL_KEY}" "Inno Setup: App Path"
  StrCmp $0 "" +2 0
    Goto un.FoundInstallDir
  ReadRegStr $0 HKCU "${OBS_UNINSTALL_KEY}" "InstallLocation"
  StrCmp $0 "" +2 0
    Goto un.FoundInstallDir
  ReadRegStr $0 HKCU "${OBS_UNINSTALL_KEY}" "Inno Setup: App Path"
  StrCmp $0 "" +2 0
    Goto un.FoundInstallDir

  SetRegView 32
  ReadRegStr $0 HKLM "${OBS_UNINSTALL_KEY}" "InstallLocation"
  StrCmp $0 "" +2 0
    Goto un.FoundInstallDir
  ReadRegStr $0 HKLM "${OBS_UNINSTALL_KEY}" "Inno Setup: App Path"
  StrCmp $0 "" +2 0
    Goto un.FoundInstallDir
  ReadRegStr $0 HKCU "${OBS_UNINSTALL_KEY}" "InstallLocation"
  StrCmp $0 "" +2 0
    Goto un.FoundInstallDir
  ReadRegStr $0 HKCU "${OBS_UNINSTALL_KEY}" "Inno Setup: App Path"
  StrCmp $0 "" +2 0
    Goto un.FoundInstallDir

  IfFileExists "D:\Obs\bin\64bit\obs64.exe" 0 +3
    StrCpy $DetectedObsDir "D:\Obs"
    StrCpy $ObsPathDetected "1"
    Return
  IfFileExists "D:\obs-studio\bin\64bit\obs64.exe" 0 +3
    StrCpy $DetectedObsDir "D:\obs-studio"
    StrCpy $ObsPathDetected "1"
    Return
  IfFileExists "$PROGRAMFILES64\obs-studio\bin\64bit\obs64.exe" 0 +3
    StrCpy $DetectedObsDir "$PROGRAMFILES64\obs-studio"
    StrCpy $ObsPathDetected "1"
    Return
  IfFileExists "$PROGRAMFILES\obs-studio\bin\64bit\obs64.exe" 0 +3
    StrCpy $DetectedObsDir "$PROGRAMFILES\obs-studio"
    StrCpy $ObsPathDetected "1"
    Return
  Return

un.FoundInstallDir:
  IfFileExists "$0\bin\64bit\obs64.exe" 0 +4
    StrCpy $DetectedObsDir "$0"
    StrCpy $ObsPathDetected "1"
    Return
  IfFileExists "$0\obs64.exe" 0 +7
    ${GetParent} "$0" $1
    ${GetParent} "$1" $2
    IfFileExists "$2\bin\64bit\obs64.exe" 0 +3
      StrCpy $DetectedObsDir "$2"
      StrCpy $ObsPathDetected "1"
    Return
  Return
FunctionEnd

Function un.CleanupLegacyObsRootInstall
  StrCmp $ObsPathDetected "1" 0 un.cleanup_done
  StrCmp $DetectedObsDir "" un.cleanup_done 0
  StrCmp $DetectedObsDir $INSTDIR un.cleanup_done 0

  Delete "$DetectedObsDir\obs-plugins\64bit\overlay_plugin.dll"
  Delete "$DetectedObsDir\obs-plugins\64bit\overlay-helper.exe"
  Delete "$DetectedObsDir\data\obs-plugins\soul-memory-obs-overlay\config\overlay.toml"
  Delete "$DetectedObsDir\data\obs-plugins\soul-memory-obs-overlay\locale\en-US.ini"
  RMDir "$DetectedObsDir\data\obs-plugins\soul-memory-obs-overlay\config"
  RMDir "$DetectedObsDir\data\obs-plugins\soul-memory-obs-overlay\locale"
  RMDir "$DetectedObsDir\data\obs-plugins\soul-memory-obs-overlay"
  Delete "$DetectedObsDir\obs-overlay-uninstall.exe"

un.cleanup_done:
FunctionEnd

Section "Uninstall"
  IfFileExists "$INSTDIR\bin\64bit\${STANDARD_DLL_NAME}" uninstall_standard uninstall_standard_legacy_check

uninstall_standard_legacy_check:
  IfFileExists "$INSTDIR\bin\64bit\${PORTABLE_DLL_NAME}" uninstall_standard uninstall_portable_check

uninstall_standard:
  StrCpy $R9 $INSTDIR

  Call un.DetectObsInstallDir
  StrCpy $INSTDIR $R9
  Call un.CleanupLegacyObsRootInstall

  Delete "$INSTDIR\bin\64bit\${STANDARD_DLL_NAME}"
  Delete "$INSTDIR\bin\64bit\${PORTABLE_DLL_NAME}"
  Delete "$INSTDIR\bin\64bit\${HELPER_EXE}"
  Delete "$INSTDIR\data\config\overlay.toml"
  Delete "$INSTDIR\data\locale\en-US.ini"
  Delete "$INSTDIR\obs-overlay-uninstall.exe"
  RMDir "$INSTDIR\bin\64bit"
  RMDir "$INSTDIR\bin"
  RMDir "$INSTDIR\data\config"
  RMDir "$INSTDIR\data\locale"
  RMDir "$INSTDIR\data"
  RMDir "$INSTDIR"
  Goto uninstall_done

uninstall_portable_check:
  IfFileExists "$INSTDIR\obs-plugins\64bit\${PORTABLE_DLL_NAME}" uninstall_portable uninstall_done

uninstall_portable:
  Delete "$INSTDIR\obs-plugins\64bit\${PORTABLE_DLL_NAME}"
  Delete "$INSTDIR\obs-plugins\64bit\${HELPER_EXE}"
  Delete "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\config\overlay.toml"
  Delete "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\locale\en-US.ini"
  RMDir "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\config"
  RMDir "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\locale"
  RMDir "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay"
  Delete "$INSTDIR\obs-overlay-uninstall.exe"

uninstall_done:
SectionEnd
