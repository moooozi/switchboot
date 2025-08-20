; NSIS Script for Switchboot Portable Launcher
!system 'mkdir "..\..\target\release\bundle\portable_win" >nul 2>&1'
!ifndef PRODUCTNAME
  !include "metadata.nsh"
!endif
Outfile "..\..\target\release\bundle\portable_win\Switchboot_${PRODUCT_VERSION}_x64-portable.exe"
InstallDir "$TEMP\${IDENTIFIER}-${PRODUCT_VERSION}"
RequestExecutionLevel user
SilentInstall silent
SetOverwrite off
SetCompressor /SOLID /FINAL zlib
Icon "..\..\icons\icon.ico"

; Add version info using NSIS VersionInfo
VIProductVersion "${VI_PRODUCT_VERSION}"
VIAddVersionKey /LANG=0x0409 "CompanyName" "${PUBLISHER}"
VIAddVersionKey /LANG=0x0409 "FileDescription" "${PRODUCTNAME}"
VIAddVersionKey /LANG=0x0409 "FileVersion" "${FILE_VERSION}"
VIAddVersionKey /LANG=0x0409 "ProductName" "${PRODUCTNAME}"
VIAddVersionKey /LANG=0x0409 "ProductVersion" "${PRODUCT_VERSION}"
VIAddVersionKey /LANG=0x0409 "LegalCopyright" "${COPYRIGHT}"

Section "MainSection" SEC01
  SetOutPath "$INSTDIR"
  File "..\..\target\release\switchboot.exe"
  File "..\..\target\release\switchboot-cli.exe"

  ; Run CLI as admin (hidden)
  ExecShell "runas" "$INSTDIR\switchboot-cli.exe" "/pipe_server" SW_HIDE

  ; Run GUI as user and wait for it to close
  ExecWait '"$INSTDIR\switchboot.exe" --portable'  

  ; Attempt to delete files and folder
  Sleep 500
  Delete "$INSTDIR\switchboot.exe"
  Delete "$INSTDIR\switchboot-cli.exe"
  RMDir "$INSTDIR"

SectionEnd
