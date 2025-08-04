!macro NSIS_HOOK_POSTINSTALL

  nsExec::Exec '"$INSTDIR\switchboot-cli.exe" /uninstall_service'
  Pop $0 ; return code
  DetailPrint "Uninstalling service returned: $0"
  nsExec::Exec '"$INSTDIR\switchboot-cli.exe" /install_service'
  Pop $0 ; return code
  DetailPrint "Installing service returned: $0"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  nsExec::Exec '"$INSTDIR\switchboot-cli.exe" /uninstall_service'
!macroend