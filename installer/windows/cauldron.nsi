; Cauldron for Windows: a per-user installer around the one Cauldron.exe.
; Installs to %LOCALAPPDATA%\Programs\Cauldron with no admin prompt, so the app's own updater
; (core/src/update.rs) can still swap Cauldron.exe in place. Adds a Start menu entry, an
; optional desktop shortcut and an entry in Settings > Apps for uninstalling.
; Built by .github/workflows/windows.yml: makensis /DVERSION=<v> /DSRC=<dir with Cauldron.exe>.

Unicode true
!include "MUI2.nsh"

!ifndef VERSION
  !define VERSION "dev"
!endif
!ifndef SRC
  !define SRC "..\..\out"
!endif
!define APP "Cauldron"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP}"

Name "${APP}"
OutFile "${SRC}\Cauldron-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\${APP}"
InstallDirRegKey HKCU "Software\${APP}" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\Cauldron.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Open Cauldron"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Cauldron" SecApp
  SectionIn RO
  ; A running Cauldron holds its exe; close it first.
  nsExec::Exec 'taskkill /IM Cauldron.exe /F'
  SetOutPath "$INSTDIR"
  File "${SRC}\Cauldron.exe"
  Delete "$INSTDIR\Cauldron.old.exe"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateShortCut "$SMPROGRAMS\${APP}.lnk" "$INSTDIR\Cauldron.exe"
  WriteRegStr HKCU "Software\${APP}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${APP}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "Cauldron"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\Cauldron.exe"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
SectionEnd

Section "Desktop shortcut" SecDesktop
  CreateShortCut "$DESKTOP\${APP}.lnk" "$INSTDIR\Cauldron.exe"
SectionEnd

Section "Uninstall"
  nsExec::Exec 'taskkill /IM Cauldron.exe /F'
  Delete "$INSTDIR\Cauldron.exe"
  Delete "$INSTDIR\Cauldron.old.exe"
  Delete "$INSTDIR\Cauldron.new.exe"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${APP}.lnk"
  Delete "$DESKTOP\${APP}.lnk"
  DeleteRegKey HKCU "${UNINST_KEY}"
  DeleteRegKey HKCU "Software\${APP}"
SectionEnd
