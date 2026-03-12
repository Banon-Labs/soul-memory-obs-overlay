!include "MUI2.nsh"

!define APP_NAME "Soul Memory OBS Overlay"
!define COMPANY_NAME "soul-memory-obs-overlay"
!define DLL_NAME "overlay_plugin.dll"
!define HELPER_EXE "overlay-helper.exe"
!define PRODUCT_VERSION "0.1.0"

Name "${APP_NAME} ${PRODUCT_VERSION}"
OutFile "SoulMemoryOverlay-${PRODUCT_VERSION}-setup.exe"
InstallDir "$PROGRAMFILES64\obs-studio"
RequestExecutionLevel admin

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetOutPath "$INSTDIR\obs-plugins\64bit"
  File "target\x86_64-pc-windows-msvc\release\overlay_plugin.dll"
  File "target\x86_64-pc-windows-msvc\release\overlay-helper.exe"

  SetOutPath "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\config"
  File "crates\overlay-plugin\data\config\overlay.toml"

  SetOutPath "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\locale"
  File "crates\overlay-plugin\data\locale\en-US.ini"

  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\obs-overlay-uninstall.exe"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\obs-plugins\64bit\overlay_plugin.dll"
  Delete "$INSTDIR\obs-plugins\64bit\overlay-helper.exe"
  Delete "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\config\overlay.toml"
  Delete "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\locale\en-US.ini"
  RMDir "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\config"
  RMDir "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay\locale"
  RMDir "$INSTDIR\data\obs-plugins\soul-memory-obs-overlay"
  Delete "$INSTDIR\obs-overlay-uninstall.exe"
SectionEnd
