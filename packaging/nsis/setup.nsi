; Standalone native installer. No Tauri runtime or custom NSIS engine required.
Unicode true
RequestExecutionLevel admin
ManifestDPIAware true
ManifestDPIAwareness PerMonitorV2,PerMonitor
ManifestSupportedOS Win10
SetCompressor /SOLID lzma
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "WordFunc.nsh"
!include "x64.nsh"
!include "WinVer.nsh"
!include "service-hooks.nsh"
!ifndef VERSION
  !error "VERSION is required; use build.py"
!endif
!ifndef OUTPUT
  !define OUTPUT "..\..\artifacts\Switchboot_x64-nsis-setup.exe"
!endif
!define KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchboot"
!define INNO_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\dev.zidane.switchboot_is1"
Name "Switchboot"
Caption "Setup - Switchboot v${VERSION}"
OutFile "${OUTPUT}"
InstallDir "$PROGRAMFILES64\Switchboot"
Icon "..\..\crates\switchboot_core\icons\icon.ico"
UninstallIcon "..\..\crates\switchboot_core\icons\icon.ico"
!addplugindir /x86-unicode "..\nsis-poc\ui\target\i686-pc-windows-msvc\release"
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1033 "ProductName" "Switchboot"
VIAddVersionKey /LANG=1033 "FileDescription" "Switchboot Setup"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "zidane.dev"
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles
AutoCloseWindow true
ShowInstDetails show
ShowUninstDetails show
Var Failure
Var Mode
Var UpgradePID
Var RestartApp
Var ExistingKey
Var ExistingKind
Var OldUninstaller
Var Action
Var ExistingVersion

Function Fail
  DetailPrint "$Failure"
  MessageBox MB_OK|MB_ICONSTOP "$Failure" /SD IDOK
  SetErrorLevel 2
  Quit
FunctionEnd

Function Launch
  switchboot_nsis_ui::Launch "$INSTDIR"
  Pop $Failure
  ${If} $Failure != ""
    Call Fail
  ${EndIf}
FunctionEnd

Function .onInit
  SetRegView 64
  SetShellVarContext all
  ${IfNot} ${RunningX64}
  ${OrIfNot} ${AtLeastWin10}
    StrCpy $Failure "Switchboot requires 64-bit Windows 10 or later."
    Call Fail
  ${EndIf}
  ; Serialize all native setup/uninstall processes, including different versions.
  System::Call 'kernel32::CreateMutexW(p 0, i 0, w "Global\Switchboot.NativeInstaller") p .r0 ?e'
  Pop $1
  ${If} $0 == 0
  ${OrIf} $1 == 183
    StrCpy $Failure "Another Switchboot installer or uninstaller is running."
    Call Fail
  ${EndIf}
  ${GetParameters} $0
  switchboot_nsis_ui::Options "$0"
  Pop $Failure
  Pop $Mode
  Pop $UpgradePID
  Pop $RestartApp
  ${If} $Failure != ""
    Call Fail
  ${EndIf}
  ${If} $Mode != "manual"
    ; Both legacy Tauri and Iced handoffs are unattended, including /P.
    SetSilent silent
  ${EndIf}
  StrCpy $ExistingKey ""
  ClearErrors
  ReadRegStr $0 HKLM "${KEY}" "InstallLocation"
  ${IfNot} ${Errors}
    StrCpy $ExistingKey "${KEY}"
    StrCpy $INSTDIR $0
    StrCpy $ExistingKind "legacy"
    ReadRegStr $1 HKLM "${KEY}" "SwitchbootInstaller"
    ${If} $1 == "native-nsis"
      StrCpy $ExistingKind "native"
    ${EndIf}
  ${EndIf}
  ClearErrors
  ReadRegStr $0 HKLM "${INNO_KEY}" "InstallLocation"
  ${IfNot} ${Errors}
    ${If} $ExistingKey != ""
      StrCpy $Failure "Both NSIS and Inno installations are registered. Resolve the duplicate installations before continuing."
      Call Fail
    ${EndIf}
    StrCpy $ExistingKey "${INNO_KEY}"
    StrCpy $INSTDIR $0
    StrCpy $ExistingKind "inno"
  ${EndIf}
  ${If} $ExistingKey != ""
    IfFileExists "$INSTDIR\switchboot.exe" +3
      StrCpy $Failure "The registered Switchboot executable is missing. Repair or remove the existing installation first."
      Call Fail
    ReadRegStr $OldUninstaller HKLM "$ExistingKey" "UninstallString"
    ${If} $OldUninstaller == ""
      StrCpy $Failure "The registered Switchboot uninstaller is missing."
      Call Fail
    ${EndIf}
    ReadRegStr $ExistingVersion HKLM "$ExistingKey" "DisplayVersion"
    ${VersionCompare} "$ExistingVersion" "${VERSION}" $0
    ${If} $0 == 1
      StrCpy $Failure "A newer Switchboot version ($ExistingVersion) is installed. Downgrades are not supported."
      Call Fail
    ${EndIf}
    ${If} $Mode == "manual"
      IfSilent 0 +3
        StrCpy $Failure "Switchboot is already installed. Run Setup interactively to choose an action."
        Call Fail
      switchboot_nsis_ui::ValidateDirectory "$INSTDIR"
      Pop $Failure
      ${If} $Failure != ""
        Call Fail
      ${EndIf}
      switchboot_nsis_ui::Select "maintenance" "$INSTDIR"
      Pop $Action
      Pop $0
      ${If} $Action == "launch"
        Call Launch
        SetErrorLevel 0
        Quit
      ${ElseIf} $Action == "cancel"
        SetErrorLevel 1
        Quit
      ${ElseIf} $Action != "uninstall"
      ${AndIf} $Action != "reinstall"
        StrCpy $Failure "$Action"
        Call Fail
      ${EndIf}
    ${EndIf}
  ${ElseIf} $Mode != "manual"
    StrCpy $Failure "Update requires an existing registered installation."
    Call Fail
  ${Else}
    IfSilent selection_done
    switchboot_nsis_ui::Select "install" "$INSTDIR"
    Pop $Action
    Pop $INSTDIR
    ${If} $Action == "cancel"
      SetErrorLevel 1
      Quit
    ${ElseIf} $Action != "install"
      StrCpy $Failure "$Action"
      Call Fail
    ${EndIf}
  ${EndIf}
selection_done:
  switchboot_nsis_ui::ValidateDirectory "$INSTDIR"
  Pop $Failure
  ${If} $Failure != ""
    Call Fail
  ${EndIf}
  ${If} $ExistingKey == ""
    IfFileExists "$INSTDIR\switchboot.exe" 0 +3
      StrCpy $Failure "The selected directory contains an unregistered Switchboot executable. Choose an empty installation directory."
      Call Fail
    IfFileExists "$INSTDIR\uninstall.exe" 0 +3
      StrCpy $Failure "The selected directory already contains an uninstaller. Choose another directory."
      Call Fail
  ${EndIf}
FunctionEnd

; Migration/reinstall waits for the old owner to remove its files and registration.
Function RemovePrevious
  ClearErrors
  ${If} $ExistingKind == "inno"
    ExecWait '$OldUninstaller /VERYSILENT /SUPPRESSMSGBOXES /NORESTART' $0
  ${Else}
    ; _?= must be last and unquoted: run in place so ExecWait observes removal.
    ExecWait '$OldUninstaller /S /UPDATE _?=$INSTDIR' $0
  ${EndIf}
  ${If} ${Errors}
  ${OrIf} $0 != 0
    StrCpy $Failure "The previous uninstaller failed or was canceled. No new files have been installed."
    Call Fail
  ${EndIf}
  StrCpy $1 0
wait_removal:
  ClearErrors
  ReadRegStr $0 HKLM "$ExistingKey" "UninstallString"
  ${If} ${Errors}
    IfFileExists "$INSTDIR\switchboot.exe" still_removing
    Return
  ${EndIf}
still_removing:
  IntOp $1 $1 + 1
  ${If} $1 >= 150
    StrCpy $Failure "The previous installation was not fully removed. No new files have been installed."
    Call Fail
  ${EndIf}
  Sleep 100
  Goto wait_removal
FunctionEnd

Section "Switchboot"
  ${If} $ExistingKey != ""
    ${If} $ExistingKind != "legacy"
      switchboot_nsis_ui::PrepareShutdown "$INSTDIR" "$UpgradePID"
      Pop $Failure
      ${If} $Failure != ""
        Call Fail
      ${EndIf}
    ${ElseIf} $Mode == "native"
      StrCpy $Failure "Native /UPGRADE cannot target a legacy Tauri installation."
      Call Fail
    ${EndIf}
    ${If} $ExistingKind != "native"
    ${OrIf} $Action == "reinstall"
    ${OrIf} $Action == "uninstall"
      Call RemovePrevious
      ${If} $Action == "uninstall"
        SetErrorLevel 0
        Quit
      ${EndIf}
    ${Else}
      !insertmacro SwitchbootService uninstall
    ${EndIf}
  ${EndIf}
  SetOutPath "$INSTDIR"
  ClearErrors
  File "..\..\target\release\switchboot.exe"
  SetOutPath "$INSTDIR\resources\icons\ico"
  File "..\..\crates\switchboot_core\resources\icons\ico\*.ico"
  ${If} ${Errors}
    StrCpy $Failure "Unable to copy Switchboot files. Installation is incomplete; run Setup again."
    Call Fail
  ${EndIf}
  SetOutPath "$INSTDIR"
  ClearErrors
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKLM "${KEY}" "DisplayName" "Switchboot"
  WriteRegStr HKLM "${KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${KEY}" "Publisher" "zidane.dev"
  WriteRegStr HKLM "${KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${KEY}" "DisplayIcon" "$INSTDIR\switchboot.exe"
  WriteRegStr HKLM "${KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "${KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegStr HKLM "${KEY}" "SwitchbootInstaller" "native-nsis"
  WriteRegDWORD HKLM "${KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${KEY}" "NoRepair" 1
  CreateShortcut "$SMPROGRAMS\Switchboot.lnk" "$INSTDIR\switchboot.exe"
  ${If} ${Errors}
    StrCpy $Failure "Unable to register the installation. Run Setup again to repair it."
    Call Fail
  ${EndIf}
  !insertmacro NSIS_HOOK_POSTINSTALL
  ${If} $RestartApp == 1
  ${OrIf} $Action == "reinstall"
    Call Launch
  ${EndIf}
  SetErrorLevel 0
SectionEnd

Function un.onInit
  SetRegView 64
  SetShellVarContext all
  ReadRegStr $0 HKLM "${KEY}" "InstallLocation"
  ReadRegStr $1 HKLM "${KEY}" "SwitchbootInstaller"
  ${If} $0 != $INSTDIR
  ${OrIf} $1 != "native-nsis"
    MessageBox MB_OK|MB_ICONSTOP "This uninstaller no longer owns the registered Switchboot installation. No files or services have been changed." /SD IDOK
    SetErrorLevel 2
    Quit
  ${EndIf}
  System::Call 'kernel32::CreateMutexW(p 0, i 0, w "Global\Switchboot.NativeInstaller") p .r0 ?e'
  Pop $1
  ; An in-place child uninstaller is expected while Setup owns the mutex.
  ; NSIS /UPDATE is reserved for that explicitly coordinated path.
  ${GetParameters} $2
  ${GetOptions} $2 "/UPDATE" $3
  ${If} ${Errors}
    ${If} $0 == 0
    ${OrIf} $1 == 183
      MessageBox MB_OK|MB_ICONSTOP "Another Switchboot installer is running." /SD IDOK
      SetErrorLevel 2
      Quit
    ${EndIf}
  ${EndIf}
FunctionEnd

Section "Uninstall"
  switchboot_nsis_ui::PrepareShutdown "$INSTDIR" "0"
  Pop $0
  ${If} $0 != ""
    MessageBox MB_OK|MB_ICONSTOP "$0" /SD IDOK
    SetErrorLevel 2
    Quit
  ${EndIf}
  !insertmacro NSIS_HOOK_PREUNINSTALL
  ClearErrors
  Delete "$INSTDIR\switchboot.exe"
  ; Delete only installer-owned files; never recursively remove user content.
  !include "owned-icons.nsh"
  ${If} ${Errors}
    MessageBox MB_OK|MB_ICONSTOP "Some Switchboot files could not be removed. Registration has been kept; retry uninstall." /SD IDOK
    SetErrorLevel 2
    Quit
  ${EndIf}
  Delete "$SMPROGRAMS\Switchboot.lnk"
  DeleteRegKey HKLM "${KEY}"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR\resources\icons\ico"
  RMDir "$INSTDIR\resources\icons"
  RMDir "$INSTDIR\resources"
  RMDir "$INSTDIR"
  SetErrorLevel 0
SectionEnd
