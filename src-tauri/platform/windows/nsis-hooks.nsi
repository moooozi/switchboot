!macro NSIS_HOOK_POSTINSTALL

  nsExec::Exec '"$INSTDIR\switchboot.exe" --cli /uninstall_service'
  Pop $0 ; return code
  DetailPrint "Uninstalling service returned: $0"
  nsExec::Exec '"$INSTDIR\switchboot.exe" --cli /install_service'
  Pop $0 ; return code
  DetailPrint "Installing service returned: $0"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::Exec '"$INSTDIR\switchboot.exe" --cli /uninstall_service'
  Pop $0 ; return code
  DetailPrint "Uninstalling service returned: $0"
!macroend