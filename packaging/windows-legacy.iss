{ Tauri/NSIS compatibility only. Native upgrades and maintenance stay separate. }
var
  MigrationDirectory: String;
  LegacyRemovalAttempted, LegacyPassiveUpgrade, LegacyInstallClickPosted: Boolean;
  LegacyUpgradeDirectory: String;

procedure InitializeLegacyUpdate;
begin
  if not HasSwitch('/UPDATE') or WizardSilent then Exit;
  LegacyUpgradeDirectory := LegacyDirectory;
  LegacyPassiveUpgrade := (HasSwitch('/P') or HasSwitch('/S')) and
    (LegacyUpgradeDirectory <> '') and
    FileExists(AddBackslash(LegacyUpgradeDirectory) + 'switchboot.exe');
  if not LegacyPassiveUpgrade then
    RaiseException('Automatic updates require native silent mode or legacy ' +
      'Tauri update arguments and an existing installation.');
  Log('Legacy passive upgrade: ' + LegacyUpgradeDirectory);
end;

procedure InitializeLegacyWizard;
begin
  if not LegacyPassiveUpgrade then Exit;
  WizardForm.DirEdit.Text := LegacyUpgradeDirectory;
  { Never automatically accept a Windows reboot. Failure/restart pages remain
    interactive; only the initial Install button is advanced automatically. }
  WizardForm.NoRadio.Checked := True;
  WizardForm.PreparingNoRadio.Checked := True;
end;

function MigrateLegacyInstallation: String;
var
  Uninstaller: String;
begin
  Result := '';
  if LegacyRemovalAttempted then Exit;
  MigrationDirectory := LegacyDirectory;
  if MigrationDirectory = '' then begin
    Result := 'Previous Switchboot InstallLocation is missing or invalid. Setup cannot continue.';
    Exit;
  end;
  Result := ReadRegisteredUninstaller(LegacyUninstallKey, Uninstaller);
  if Result = '' then
    Result := RunRegisteredUninstaller(Uninstaller, MigrationDirectory, True, False);
  if Result <> '' then begin
    Result := Result + ' Setup cannot continue until the previous installation is removed successfully.';
    Log(Result);
    Exit;
  end;
  Log('Previous NSIS installation removed.');
  LegacyRemovalAttempted := True;
end;

procedure CleanupLegacyInstallation;
begin
  { Retire leftover NSIS registration/uninstaller only after successful setup. }
  if (MigrationDirectory = '') or
     (CompareText(AddBackslash(MigrationDirectory), AddBackslash(ExpandConstant('{app}'))) <> 0) then Exit;
  if InstallationExists(LegacyUninstallKey) and
     not RegDeleteKeyIncludingSubkeys({#InstallationRegistryRoot}, LegacyUninstallKey) then
    Log('Unable to remove the previous installer registration.');
  if FileExists(ExpandConstant('{app}\uninstall.exe')) and
     not DeleteFile(ExpandConstant('{app}\uninstall.exe')) then
    Log('Unable to remove the previous uninstaller.');
end;

procedure LegacyPageChanged(PageID: Integer);
begin
  if not LegacyPassiveUpgrade or (PageID <> wpReady) or LegacyInstallClickPosted then Exit;
  { Defer navigation until the page callback finishes. Never auto-click error,
    application-close or reboot choices on other pages. }
  LegacyInstallClickPosted := PostMessage(WizardForm.NextButton.Handle, ButtonClickMessage, 0, 0);
  if LegacyInstallClickPosted then
    Log('Legacy passive upgrade: queued Install button click.')
  else
    Log('Unable to queue Install; the installer remains available for manual input.');
end;
