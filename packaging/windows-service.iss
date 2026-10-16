{ Windows service lifecycle. Only the installed executable performs SCM operations. }
var
  ServiceError: String;

function RunServiceCommand(const Action: String): String;
var
  ExitCode: Integer;
begin
  Result := '';
  if not Exec(ExpandConstant('{app}\switchboot.exe'), '--cli /' + Action,
      ExpandConstant('{app}'), SW_HIDE, ewWaitUntilTerminated, ExitCode) then
    Result := 'Unable to run the Switchboot service command: ' + SysErrorMessage(ExitCode)
  else if ExitCode <> 0 then
    Result := 'Switchboot service command ' + Action + ' failed (exit code ' + IntToStr(ExitCode) + ').';
  if Result <> '' then Log(Result);
end;

procedure PrepareServiceReplacement;
var
  Error: String;
begin
  if not FileExists(ExpandConstant('{app}\switchboot.exe')) then Exit;
  Error := RunServiceCommand('uninstall_service');
  if Error <> '' then
    Log('Continuing after best-effort service removal: ' + Error);
end;

procedure InstallWindowsService;
begin
  ServiceError := RunServiceCommand('install_service');
  if ServiceError <> '' then RaiseException(ServiceError);
end;

procedure UninstallWindowsService;
var
  Error: String;
begin
  Error := RunServiceCommand('uninstall_service');
  if Error <> '' then RaiseException(Error + ' Application files have been kept.');
end;

function ServiceSetupExitCode: Integer;
begin
  { Inno catches ssPostInstall exceptions, so explicitly report failure. }
  Result := 0;
  if ServiceError <> '' then Result := 1;
end;

procedure ShowServiceSetupResult(PageID: Integer);
begin
  if (PageID = wpFinished) and (ServiceError <> '') then
    WizardForm.FinishedLabel.Caption :=
      'Switchboot service setup failed. The application files have been installed, ' +
      'but installed mode is unavailable. Resolve the error and run Setup again.' + #13#10#13#10 + ServiceError;
end;
