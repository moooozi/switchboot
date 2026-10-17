Unicode true
RequestExecutionLevel user
ManifestDPIAware true
ManifestDPIAwareness PerMonitorV2,PerMonitor
ManifestSupportedOS Win10
SetCompressor /SOLID lzma
Name "Switchboot NSIS UI proof of concept"
Caption "Setup - Switchboot (NSIS preview)"
!ifndef POC_OUTPUT
  !define POC_OUTPUT "..\..\artifacts\Switchboot_NSIS-preview.exe"
!endif
OutFile "${POC_OUTPUT}"
!ifdef NATIVE_SVG
  Icon "..\..\crates\switchboot_core\icons\icon.ico"
!else
  Icon "..\..\artifacts\nsis-preview.ico"
!endif
!addplugindir /x86-unicode "ui\target\i686-pc-windows-msvc\release"
!include "LogicLib.nsh"

# No application files or registration: even /S and /D cannot target an install.
SilentInstall silent
AutoCloseWindow true
ShowInstDetails nevershow
Var PreviewPath

Function .onInit
  InitPluginsDir
  !ifdef POC_MAINTENANCE
    ; Visual-only fixture: actions return exit codes, never launch/remove/install.
    switchboot_nsis_ui::Select "maintenance-preview" "$PROGRAMFILES64\Switchboot"
    Pop $0
    Pop $PreviewPath
    ${If} $0 == "launch"
      SetErrorLevel 10
    ${ElseIf} $0 == "reinstall"
      SetErrorLevel 11
    ${ElseIf} $0 == "uninstall"
      SetErrorLevel 12
    ${Else}
      SetErrorLevel 1
    ${EndIf}
    Quit
  !endif

  ClearErrors
  switchboot_nsis_ui::Choose "$PROGRAMFILES64\Switchboot" "@103"
  IfErrors plugin_load_failed
  Pop $0
  Pop $PreviewPath
  ${If} $0 == "cancel"
    SetErrorLevel 1
    Quit
  ${ElseIf} $0 != "install"
    MessageBox MB_OK|MB_ICONSTOP "Unable to create the preview UI.$\r$\n$0"
    SetErrorLevel 2
    Quit
  ${EndIf}
  Return
plugin_load_failed:
  MessageBox MB_OK|MB_ICONSTOP "Unable to load the preview UI plug-in. Rebuild using build.py to check its DLL dependencies."
  SetErrorLevel 2
  Quit
FunctionEnd

Section "Harmless temporary fixture"
  ClearErrors
  SetOutPath "$PLUGINSDIR"
  File "fixture.txt"
  ${If} ${Errors}
    switchboot_nsis_ui::Complete "error" "@103"
    SetErrorLevel 2
    Quit
  ${EndIf}
  switchboot_nsis_ui::Complete "success" "@103"
SectionEnd
