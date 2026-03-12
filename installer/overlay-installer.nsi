!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"
!include "nsDialogs.nsh"

!ifndef PROJECT_ROOT
!define PROJECT_ROOT "."
!endif

!define APP_NAME "Soul Memory OBS Overlay"
!define COMPANY_NAME "soul-memory-obs-overlay"
!define DLL_NAME "overlay_plugin.dll"
!define HELPER_EXE "overlay-helper.exe"
!define PRODUCT_VERSION "0.1.1"
!define OBS_UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\OBS Studio_is1"
!define INSTALL_MODE_STANDARD "standard"
!define INSTALL_MODE_PORTABLE "portable"

Var ObsPathDetected
Var DetectedObsDir
Var InstallMode
Var PortableDetectNoticeShown
Var ModePageDialog
Var ModeStandardRadio
Var ModePortableRadio

Name "${APP_NAME} ${PRODUCT_VERSION}"
OutFile "SoulMemoryOverlay-${PRODUCT_VERSION}-setup.exe"
InstallDir "$COMMONAPPDATA\obs-studio\plugins\soul-memory-obs-overlay"
RequestExecutionLevel admin

!define MUI_DIRECTORYPAGE_TEXT_TOP "Choose installation folder."
!define MUI_DIRECTORYPAGE_TEXT_DESTINATION "Installation folder"

Page custom InstallModePageCreate InstallModePageLeave
!define MUI_PAGE_CUSTOMFUNCTION_PRE DirectoryPagePre
!insertmacro MUI_PAGE_DIRECTORY
!undef MUI_PAGE_CUSTOMFUNCTION_PRE
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Function .onInit
  StrCpy $InstallMode "${INSTALL_MODE_STANDARD}"
  StrCpy $ObsPathDetected "0"
  StrCpy $DetectedObsDir ""
  StrCpy $PortableDetectNoticeShown "0"
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

Function DirectoryPagePre
  StrCmp $InstallMode "${INSTALL_MODE_PORTABLE}" portable_mode standard_mode

standard_mode:
  StrCpy $INSTDIR "$COMMONAPPDATA\obs-studio\plugins\soul-memory-obs-overlay"
  DirText "Choose plugin installation folder." "Standard mode installs to ProgramData plugin layout for non-portable OBS." "Browse..."
  Return

portable_mode:
  StrCmp $ObsPathDetected "1" detected not_detected

not_detected:
  StrCpy $INSTDIR "$PROGRAMFILES64\obs-studio"
  StrCmp $PortableDetectNoticeShown "1" portable_text 0
  MessageBox MB_ICONINFORMATION "OBS installation could not be auto-detected. In Portable/custom mode, click Browse and select the OBS folder that contains bin\\64bit\\obs64.exe."
  StrCpy $PortableDetectNoticeShown "1"
  Goto portable_text

detected:
  StrCpy $INSTDIR $DetectedObsDir
  StrCpy $PortableDetectNoticeShown "1"

portable_text:
  DirText "Choose OBS installation folder." "Portable/custom mode installs to OBS root layout. Select the folder containing bin\\64bit\\obs64.exe." "Browse..."
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
  MessageBox MB_ICONEXCLAMATION "Standard mode requires a plugin folder path (for example C:\\ProgramData\\obs-studio\\plugins\\soul-memory-obs-overlay)."
  Abort

invalid:
  MessageBox MB_ICONEXCLAMATION "Portable/custom mode requires the OBS installation folder. It must contain bin\\64bit\\obs64.exe (for example C:\\Program Files\\obs-studio)."
  Abort
valid:
FunctionEnd

Section "Install"
  StrCmp $InstallMode "${INSTALL_MODE_PORTABLE}" install_portable install_standard

install_standard:
  Call CleanupLegacyObsRootInstall

  SetOutPath "$INSTDIR\bin\64bit"
  File "${PROJECT_ROOT}\installer\staging\overlay_plugin.dll"
  File "${PROJECT_ROOT}\installer\staging\overlay-helper.exe"

  SetOutPath "$INSTDIR\data\config"
  File "${PROJECT_ROOT}\installer\staging\overlay.toml"

  SetOutPath "$INSTDIR\data\locale"
  File "${PROJECT_ROOT}\installer\staging\en-US.ini"

  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\obs-overlay-uninstall.exe"
  Goto install_done

install_portable:
  SetOutPath "$INSTDIR\obs-plugins\64bit"
  File "${PROJECT_ROOT}\installer\staging\overlay_plugin.dll"
  File "${PROJECT_ROOT}\installer\staging\overlay-helper.exe"

  SetOutPath "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\config"
  File "${PROJECT_ROOT}\installer\staging\overlay.toml"

  SetOutPath "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\locale"
  File "${PROJECT_ROOT}\installer\staging\en-US.ini"

  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\obs-overlay-uninstall.exe"

install_done:
SectionEnd

Section "Uninstall"
  IfFileExists "$INSTDIR\bin\64bit\overlay_plugin.dll" uninstall_standard uninstall_portable_check

uninstall_standard:
  StrCpy $R9 $INSTDIR

  Call DetectObsInstallDir
  StrCpy $INSTDIR $R9
  Call CleanupLegacyObsRootInstall

  Delete "$INSTDIR\bin\64bit\overlay_plugin.dll"
  Delete "$INSTDIR\bin\64bit\overlay-helper.exe"
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
  IfFileExists "$INSTDIR\obs-plugins\64bit\overlay_plugin.dll" uninstall_portable uninstall_done

uninstall_portable:
  Delete "$INSTDIR\obs-plugins\64bit\overlay_plugin.dll"
  Delete "$INSTDIR\obs-plugins\64bit\overlay-helper.exe"
  Delete "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\config\overlay.toml"
  Delete "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\locale\en-US.ini"
  RMDir "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\config"
  RMDir "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\locale"
  RMDir "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay"
  Delete "$INSTDIR\obs-overlay-uninstall.exe"

uninstall_done:
SectionEnd
