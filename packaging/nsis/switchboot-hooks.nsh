; Product policy and application-specific file/service/updater integrations.
Var SwitchbootUpdateMode
Var UpgradePID
Var UpgradePIDProvided

; Tauri's NSIS installer writes InstallLocation as a quoted directory string.
; Read the raw registry value again: NSIS sanitizes directory variables on
; assignment, so stripping quotes from an already-assigned $INSTDIR is too late.
!macro DiscoverPrevious
  ${If} $ExistingKind == "legacy"
    ReadRegStr $0 HKLM "$ExistingKey" "InstallLocation"
    StrCpy $1 $0 1
    StrCpy $2 $0 1 -1
    ${If} $1 == '$\"'
    ${AndIf} $2 == '$\"'
      StrCpy $0 $0 -1 1
    ${EndIf}
    StrCpy $INSTDIR $0
  ${ElseIf} $ExistingKey == ""
    ; Fresh installs always use the machine-wide product destination.
    StrCpy $INSTDIR "${APP_DIRECTORY}"
  ${EndIf}
!macroend

!macro InitializeOptions
  StrCpy $SwitchbootUpdateMode "manual"
  StrCpy $UpgradePID ""
  StrCpy $UpgradePIDProvided 0
!macroend

; Compatibility belongs to this application, never the reusable UI/runtime.
!macro ParseOption
  ${Switch} $Option
    ${Case} "/ARGS"
      StrCpy $StopOptions 1
      ${Break}
    ${Case} "/UPDATE"
    ${Case} "/UPGRADE"
      ${If} $SwitchbootUpdateMode != "manual"
      ${AndIf} $SwitchbootUpdateMode != $Option
        StrCpy $Failure "/UPDATE and /UPGRADE cannot be combined."
      ${EndIf}
      StrCpy $SwitchbootUpdateMode $Option
      StrCpy $Automatic 1
      ${Break}
    ${Case} "/R"
      StrCpy $RestartApp 1
      ${Break}
    ${Default}
      StrCpy $0 $Option 12
      ${If} $0 == "/UPGRADEPID="
        StrCpy $UpgradePID $Option "" 12
        StrCpy $UpgradePIDProvided 1
      ${EndIf}
  ${EndSwitch}
!macroend

!macro ValidateOptions
  ${If} $SwitchbootUpdateMode == "/UPGRADE"
    ; Preserve decimal parsing (including a leading + or zeros) before NSIS's
    ; integer conversion, which otherwise interprets a leading zero as octal.
    StrCpy $0 $UpgradePID 1
    ${If} $0 == "+"
      StrCpy $UpgradePID $UpgradePID "" 1
    ${EndIf}
    StrCpy $0 $UpgradePID 1
    ${DoWhile} $0 == "0"
      StrCpy $UpgradePID $UpgradePID "" 1
      StrCpy $0 $UpgradePID 1
    ${Loop}
    ; Canonical unsigned decimal comparison rejects zero, overflow and non-digits.
    IntFmt $0 "%u" $UpgradePID
    ${If} $UpgradePID == ""
    ${OrIf} $UpgradePID == "0"
    ${OrIf} $UpgradePID != $0
      StrCpy $Failure "/UPGRADE requires a valid /UPGRADEPID."
      Call Fail
    ${EndIf}
  ${ElseIf} $UpgradePIDProvided == 1
    StrCpy $Failure "/UPGRADEPID requires /UPGRADE."
    Call Fail
  ${EndIf}
  ${If} $SwitchbootUpdateMode != "manual"
    StrCpy $WorkerArguments "$SwitchbootUpdateMode"
    ${If} $SwitchbootUpdateMode == "/UPGRADE"
      StrCpy $WorkerArguments "$WorkerArguments /UPGRADEPID=$UpgradePID"
    ${EndIf}
  ${EndIf}
!macroend

!macro RemovePrevious
  Call RemovePreviousInstallation
!macroend

!macro PreflightInstall
  ${If} $ExistingKind == "legacy"
    ; Validate the destination before removing anything from the old location.
    Push $INSTDIR
    StrCpy $INSTDIR "${APP_DIRECTORY}"
    Call ValidateCurrentOwnership
    Pop $INSTDIR
  ${EndIf}
  ${If} $SwitchbootUpdateMode == "/UPGRADE"
    ${If} $ExistingKind == "legacy"
      StrCpy $Failure "Native /UPGRADE cannot target a legacy Tauri installation."
      Call Fail
    ${EndIf}
  ${EndIf}
!macroend

!macro PrepareApplication
  ${If} $SwitchbootUpdateMode == "/UPGRADE"
    native_nsis_ui::PrepareShutdown /NOUNLOAD "$INSTDIR\switchboot.exe" "Local\Switchboot.Upgrade.$UpgradePID" "$UpgradePID"
    Pop $Failure
    !insertmacro CheckResult
  ${EndIf}
!macroend

!macro InstallFiles
  ClearErrors
  SetOutPath "$INSTDIR\resources\icons\ico"
  File "${SWITCHBOOT_ICONS}\*.ico"
  ${If} ${Errors}
    StrCpy $Failure "Unable to install shortcut icons. Close applications using these files and retry Setup."
    Call Fail
  ${EndIf}
!macroend

!macro AfterInstall
  !insertmacro Progress 85 "Installing the Windows service..."
  !insertmacro SwitchbootService install ""
!macroend

!macro BeforeUninstall PREFIX
  !insertmacro SwitchbootService uninstall "${PREFIX}"
!macroend

!macro BeforeReplace
  !insertmacro SwitchbootService uninstall ""
!macroend

!macro RemoveDirectories
  RMDir "$INSTDIR\resources\icons\ico"
  RMDir "$INSTDIR\resources\icons"
  RMDir "$INSTDIR\resources"
!macroend

; Define after toolkit variables/functions are declared. The entry point inserts
; this macro after the toolkit include so ordinary NSIS owns migration sequencing.
!macro SwitchbootMigrationFunctions
Function RemovePreviousInstallation
  ${If} $OldUninstaller == ""
    StrCpy $Failure "The previous uninstaller is missing. Repair or remove the previous installation first."
    Call Fail
  ${EndIf}
  ClearErrors
  ExecWait '$OldUninstaller /S /UPDATE _?=$INSTDIR' $0
  ${If} ${Errors}
  ${OrIf} $0 != 0
    StrCpy $Failure "The previous uninstaller failed or was canceled. No new files have been installed."
    Call Fail
  ${EndIf}
  IfFileExists "$INSTDIR\switchboot.exe" removal_incomplete
  ReadRegStr $0 HKLM "$ExistingKey" "InstallLocation"
  ${If} $0 != ""
    Goto removal_incomplete
  ${EndIf}
  ${If} $Action != "uninstall"
    ; The old path is needed through removal. Only then switch extraction,
    ; registration, service installation and relaunch to the fixed destination.
    StrCpy $INSTDIR "${APP_DIRECTORY}"
  ${EndIf}
  Return
removal_incomplete:
  StrCpy $Failure "The previous installation was not fully removed. Close Switchboot and complete its removal before retrying."
  Call Fail
FunctionEnd
!macroend
