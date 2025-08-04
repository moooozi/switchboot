; NSIS Script for Switchboot Portable Launcher
!system 'mkdir "..\target\release\bundle\portable_win" >nul 2>&1'
Outfile "..\target\release\bundle\portable_win\Switchboot-Portable.exe"
!ifndef SUFFIX
  !define SUFFIX "DEV"
!endif
InstallDir "$TEMP\SwitchbootPortable-${SUFFIX}"
RequestExecutionLevel user
SilentInstall silent
SetOverwrite off
SetCompressor /SOLID /FINAL zlib
Icon "../icons/icon.ico"

Section "MainSection" SEC01
  SetOutPath "$INSTDIR"
  File "..\target\release\switchboot.exe"
  File "..\target\release\switchboot-cli.exe"

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
