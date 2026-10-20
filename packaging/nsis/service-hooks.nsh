; Ask Switchboot to manage its Windows service and stop on failure.
; Setup uses an empty prefix; uninstall.exe uses "un." for its failure handler.
!macro SwitchbootService ACTION PREFIX
  !if "${ACTION}" == "install"
    nsExec::ExecToLog '"$INSTDIR\switchboot.exe" --cli /install_service'
  !else if "${ACTION}" == "uninstall"
    nsExec::ExecToLog '"$INSTDIR\switchboot.exe" --cli /uninstall_service'
  !else
    !error "SwitchbootService requires install or uninstall"
  !endif
  Pop $0
  ${If} $0 != 0
    !if "${ACTION}" == "install"
      StrCpy $Failure "Switchboot service install failed (result: $0). Close Switchboot and retry Setup to repair the installation."
    !else
      StrCpy $Failure "Switchboot service removal failed (result: $0). Close Switchboot and retry Setup to repair the installation."
    !endif
    Call ${PREFIX}Fail
  ${EndIf}
!macroend
