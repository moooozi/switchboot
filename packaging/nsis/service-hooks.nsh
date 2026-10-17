; Adapted from main:src-tauri/platform/windows/nsis-hooks.nsi.
; Unlike the old hooks, failures are fatal and retain application files.
!macro SwitchbootService ACTION
  nsExec::ExecToLog '"$INSTDIR\switchboot.exe" --cli /${ACTION}_service'
  Pop $0
  ${If} $0 != 0
    StrCpy $Failure 'Switchboot service ${ACTION} failed (result: $0). Application files have been kept. Run Setup again after resolving the service error.'
    Call Fail
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro SwitchbootService install
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::ExecToLog '"$INSTDIR\switchboot.exe" --cli /uninstall_service'
  Pop $0
  ${If} $0 != 0
    MessageBox MB_OK|MB_ICONSTOP 'Switchboot service removal failed (result: $0). Application files have been kept.' /SD IDOK
    SetErrorLevel 2
    Quit
  ${EndIf}
!macroend
