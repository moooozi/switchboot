{ Shared setup arguments, registration lookup and registered-uninstaller execution. }
#ifndef InstallationRegistryRoot
  #define InstallationRegistryRoot HKLM64
#endif
#ifndef InstallationNativeKey
  #define InstallationNativeKey "Software\Microsoft\Windows\CurrentVersion\Uninstall\dev.zidane.switchboot_is1"
#endif
#ifndef InstallationLegacyKey
  #define InstallationLegacyKey "Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchboot"
#endif

const
  NativeUninstallKey = '{#InstallationNativeKey}';
  LegacyUninstallKey = '{#InstallationLegacyKey}';
  ButtonClickMessage = $00F5;

function HasSwitch(const Value: String): Boolean;
var
  Index: Integer;
begin
  Result := False;
  for Index := 1 to ParamCount do begin
    { Everything after /ARGS belongs to the old app, not Setup. }
    if CompareText(ParamStr(Index), '/ARGS') = 0 then Exit;
    if CompareText(ParamStr(Index), Value) = 0 then begin
      Result := True;
      Exit;
    end;
  end;
end;

function UpgradePID: Integer;
var
  Index: Integer;
  Value: String;
begin
  Result := 0;
  for Index := 1 to ParamCount do begin
    Value := ParamStr(Index);
    if CompareText(Value, '/ARGS') = 0 then Exit;
    if CompareText(Copy(Value, 1, 12), '/UPGRADEPID=') = 0 then begin
      Result := StrToIntDef(Copy(Value, 13, MaxInt), 0);
      Exit;
    end;
  end;
end;

function InstallationExists(const Key: String): Boolean;
begin
  Result := RegKeyExists({#InstallationRegistryRoot}, Key);
end;

function ReadInstallationDirectory(const Key: String; var Directory: String): Boolean;
begin
  Result := RegQueryStringValue({#InstallationRegistryRoot}, Key, 'InstallLocation', Directory);
  Directory := RemoveQuotes(Trim(Directory));
end;

function LegacyDirectory: String;
begin
  if not ReadInstallationDirectory(LegacyUninstallKey, Result) or not DirExists(Result) then
    Result := '';
end;

procedure ValidateUpgradeArguments;
begin
  if not HasSwitch('/UPGRADE') then Exit;
  if HasSwitch('/UPDATE') then
    RaiseException('/UPGRADE and legacy /UPDATE cannot be combined.');
  if UpgradePID <= 0 then
    RaiseException('/UPGRADE requires a valid /UPGRADEPID.');
  if InstallationExists(LegacyUninstallKey) then
    RaiseException('/UPGRADE requires a native Inno installation; use the legacy migration path first.');
end;

function ReadRegisteredUninstaller(const Key: String; var Uninstaller: String): String;
begin
  Result := '';
  if not RegQueryStringValue({#InstallationRegistryRoot}, Key, 'UninstallString', Uninstaller) then
    Result := 'The registered Switchboot uninstaller is missing.'
  else begin
    { Both supported installers register a quoted executable, without arguments.
      Never infer the uninstaller executable from the installation directory. }
    Uninstaller := RemoveQuotes(Trim(Uninstaller));
    if not FileExists(Uninstaller) then
      Result := 'The registered Switchboot uninstaller was not found: ' + Uninstaller;
  end;
end;

function RunRegisteredUninstaller(const Uninstaller, Directory: String;
  Legacy, Visible: Boolean): String;
var
  Parameters, WorkingDirectory: String;
  ExitCode, ShowCommand: Integer;
begin
  Result := '';
  WorkingDirectory := ExpandConstant('{tmp}');
  ShowCommand := SW_HIDE;
  if Visible then ShowCommand := SW_SHOWNORMAL;
  if Legacy then begin
    { _?= must be last and unquoted: NSIS runs in place, so Exec waits for
      removal instead of racing a copied uninstaller. }
    Parameters := '/S /UPDATE _?=' + Directory;
    if not Visible then WorkingDirectory := Directory;
  end else
    Parameters := '/SILENT /NORESTART /LOG';
  if not Exec(Uninstaller, Parameters, WorkingDirectory,
      ShowCommand, ewWaitUntilTerminated, ExitCode) then
    Result := 'Unable to start the Switchboot uninstaller: ' + SysErrorMessage(ExitCode)
  else if ExitCode <> 0 then
    Result := 'Switchboot uninstallation failed or was canceled (exit code ' + IntToStr(ExitCode) + ').';
end;

function WaitForInstallationRemoval(const Key, Directory, Uninstaller: String;
  Legacy: Boolean): String;
var
  Attempt: Integer;
begin
  Result := '';
  { Inno's first phase returns before its temporary second phase deletes the
    original uninstaller. Do not let it race newly installed uninstall files. }
  for Attempt := 1 to 150 do begin
    if not InstallationExists(Key) and
       not FileExists(AddBackslash(Directory) + 'switchboot.exe') and
       (Legacy or not FileExists(Uninstaller)) then Exit;
    Sleep(100);
  end;
  Result := 'The previous Switchboot installation was not fully removed. Setup has not installed new files.';
end;
