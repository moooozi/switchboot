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
  nsExec::Exec '"$INSTDIR\switchboot.exe" --cli /uninstall_service'
  Pop $0
  File "..\target\release\switchboot.exe"
  SetOutPath "$INSTDIR\resources\icons\ico"
  File "..\crates\switchboot_core\resources\icons\ico\*.ico"
  SetOutPath "$INSTDIR"
  nsExec::Exec '"$INSTDIR\switchboot.exe" --cli /install_service'
  Pop $0
  IntCmp $0 0 +3
    MessageBox MB_ICONSTOP "Unable to install the Switchboot service (error $0)."
    Abort
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$SMPROGRAMS\Switchboot.lnk" "$INSTDIR\switchboot.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchboot" "DisplayName" "Switchboot"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchboot" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchboot" "UninstallString" '"$INSTDIR\uninstall.exe"'
SectionEnd
Section "Uninstall"
  nsExec::Exec '"$INSTDIR\switchboot.exe" --cli /uninstall_service'
  Pop $0
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
