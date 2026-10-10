Unicode true
!include "MUI2.nsh"
!ifndef VERSION
  !error "VERSION must be defined"
!endif
Name "Switchboot"
OutFile "..\artifacts\Switchboot_x64-setup.exe"
InstallDir "$PROGRAMFILES64\Switchboot"
RequestExecutionLevel admin
!define MUI_ICON "..\crates\switchboot_core\icons\icon.ico"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
Function .onInit
  SetRegView 64
  SetShellVarContext all
FunctionEnd
Function un.onInit
  SetRegView 64
  SetShellVarContext all
FunctionEnd
Section "Switchboot"
  SetOutPath "$INSTDIR"
  IfFileExists "$INSTDIR\switchboot.exe" 0 install_files
  nsExec::Exec '"$INSTDIR\switchboot.exe" --cli /uninstall_service'
  Pop $0
  StrCmp $0 "0" install_files
    MessageBox MB_ICONSTOP "Unable to stop and remove the existing Switchboot service (error $0). Installation has stopped before replacing application files." /SD IDOK
    SetErrorLevel 1
    Abort
  install_files:
  File "..\target\release\switchboot.exe"
  SetOutPath "$INSTDIR\resources\icons\ico"
  File "..\crates\switchboot_core\resources\icons\ico\*.ico"
  SetOutPath "$INSTDIR"
  nsExec::Exec '"$INSTDIR\switchboot.exe" --cli /install_service'
  Pop $0
  StrCmp $0 "0" service_installed
    MessageBox MB_ICONSTOP "Unable to install the Switchboot service (error $0)." /SD IDOK
    SetErrorLevel 1
    Abort
  service_installed:
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$SMPROGRAMS\Switchboot.lnk" "$INSTDIR\switchboot.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchboot" "DisplayName" "Switchboot"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchboot" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchboot" "UninstallString" '"$INSTDIR\uninstall.exe"'
SectionEnd
Section "Uninstall"
  nsExec::Exec '"$INSTDIR\switchboot.exe" --cli /uninstall_service'
  Pop $0
  StrCmp $0 "0" service_removed
    MessageBox MB_ICONSTOP "Unable to stop and remove the Switchboot service (error $0). Application files have been kept." /SD IDOK
    SetErrorLevel 1
    Abort
  service_removed:
  Delete "$SMPROGRAMS\Switchboot.lnk"
  Delete "$INSTDIR\resources\icons\ico\*.ico"
  RMDir "$INSTDIR\resources\icons\ico"
  RMDir "$INSTDIR\resources\icons"
  RMDir "$INSTDIR\resources"
  Delete "$INSTDIR\switchboot.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchboot"
SectionEnd
