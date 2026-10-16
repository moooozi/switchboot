{ Manual installed-app actions. Automatic /UPGRADE and legacy /UPDATE bypass this page. }
var
  MaintenancePage: TWizardPage;
  MaintenanceDirectory, MaintenanceKey: String;
  MaintenanceLegacy, MaintenanceClosing, MaintenanceReinstalled: Boolean;
  MaintenanceUninstallButton, MaintenanceReinstallButton, MaintenanceLaunchButton: TNewButton;

procedure DetectMaintenanceInstallation;
begin
  if HasSwitch('/UPGRADE') or HasSwitch('/UPDATE') then Exit;
  if InstallationExists(NativeUninstallKey) then
    MaintenanceKey := NativeUninstallKey
  else if InstallationExists(LegacyUninstallKey) then begin
    MaintenanceKey := LegacyUninstallKey;
    MaintenanceLegacy := True;
  end;
  if MaintenanceKey = '' then Exit;
  if not ReadInstallationDirectory(MaintenanceKey, MaintenanceDirectory) then
    RaiseException('The existing Switchboot installation has no registered installation directory.');
  if not DirExists(MaintenanceDirectory) then
    RaiseException('The registered Switchboot installation directory is missing: ' + MaintenanceDirectory);
  if WizardSilent then
    RaiseException('Switchboot is already installed. Run Setup interactively to Uninstall, Reinstall or Launch it.');
end;

procedure SetMaintenanceEnabled(Enabled: Boolean);
begin
  MaintenanceUninstallButton.Enabled := Enabled;
  MaintenanceReinstallButton.Enabled := Enabled;
  MaintenanceLaunchButton.Enabled := Enabled;
  WizardForm.CancelButton.Enabled := Enabled;
end;

function RunMaintenanceUninstaller: String;
var
  Uninstaller: String;
begin
  Result := ReadRegisteredUninstaller(MaintenanceKey, Uninstaller);
  if Result <> '' then Exit;
  if not MaintenanceLegacy then begin
    Result := PrepareInstalledShutdown(MaintenanceDirectory);
    if Result <> '' then Exit;
  end;
  Log('Maintenance: running ' + Uninstaller);
  Result := RunRegisteredUninstaller(Uninstaller, MaintenanceDirectory, MaintenanceLegacy, True);
  if Result = '' then
    Result := WaitForInstallationRemoval(MaintenanceKey, MaintenanceDirectory, Uninstaller, MaintenanceLegacy);
end;

procedure MaintenanceAction(Sender: TObject);
var
  Error: String;
  ExitCode: Integer;
begin
  if Sender = MaintenanceLaunchButton then begin
    if not ShellExecAsOriginalUser('open', AddBackslash(MaintenanceDirectory) + 'switchboot.exe',
        '', MaintenanceDirectory, SW_SHOWNORMAL, ewNoWait, ExitCode) then begin
      MsgBox('Unable to launch Switchboot: ' + SysErrorMessage(ExitCode), mbError, MB_OK);
      Exit;
    end;
  end else begin
    Log('Maintenance: uninstalling the registered installation.');
    SetMaintenanceEnabled(False);
    try
      Error := RunMaintenanceUninstaller;
      Log('Maintenance uninstaller result: ' + Error);
    finally
      SetMaintenanceEnabled(True);
    end;
    if Error <> '' then begin
      MsgBox(Error, mbError, MB_OK);
      Exit;
    end;
    if Sender = MaintenanceReinstallButton then begin
      MaintenanceReinstalled := True;
      WizardForm.NextButton.Visible := True;
      PostMessage(WizardForm.NextButton.Handle, ButtonClickMessage, 0, 0);
      Exit;
    end;
  end;
  MaintenanceClosing := True;
  WizardForm.CancelButton.Visible := True;
  PostMessage(WizardForm.CancelButton.Handle, ButtonClickMessage, 0, 0);
end;

procedure InitializeMaintenancePage;
begin
  if MaintenanceKey = '' then Exit;
  WizardForm.DirEdit.Text := MaintenanceDirectory;
  MaintenancePage := IssConfigCreateActionPage(wpWelcome,
    'Switchboot is already installed', 'Choose what you would like to do with your installation.');
  MaintenanceUninstallButton := IssConfigAddPageAction(MaintenancePage, 'Uninstall', @MaintenanceAction, False);
  MaintenanceReinstallButton := IssConfigAddPageAction(MaintenancePage, 'Reinstall', @MaintenanceAction, False);
  MaintenanceLaunchButton := IssConfigAddPageAction(MaintenancePage, 'Launch', @MaintenanceAction, True);
end;

procedure MaintenancePageChanged(PageID: Integer);
begin
  { Never navigate back into actions for an installation already removed. }
  if MaintenanceReinstalled then WizardForm.BackButton.Visible := False;
end;

function MaintenanceCanContinue(PageID: Integer): Boolean;
begin
  Result := True;
  if MaintenancePage <> nil then
    if PageID = MaintenancePage.ID then Result := MaintenanceReinstalled;
end;

function MaintenanceCanCloseWithoutConfirmation(PageID: Integer): Boolean;
begin
  Result := MaintenanceClosing or ((MaintenancePage <> nil) and (PageID = MaintenancePage.ID));
end;
