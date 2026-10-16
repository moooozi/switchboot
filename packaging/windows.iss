#ifndef VERSION
  #error VERSION must be defined
#endif

; Switchboot owns identity, payload and lifecycle; issconfig owns appearance.
#define IssConfigHeading "Install Switchboot"
#define IssConfigIntroduction "Switchboot lets you manage your boot order and choose which operating system starts next."
#define IssConfigFolderSuffix "Switchboot"
#include "../issconfig/issconfig.iss"

[Setup]
AppId=dev.zidane.switchboot
AppName=Switchboot
AppVersion={#VERSION}
AppVerName= Switchboot v{#VERSION}
AppPublisher=zidane.dev
DefaultDirName={code:GetDefaultDirectory}
DisableProgramGroupPage=yes
DisableWelcomePage=yes
DisableDirPage=no
DisableReadyPage=yes
DisableFinishedPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
OutputDir=..\artifacts
OutputBaseFilename=Switchboot_x64-setup
SetupIconFile=..\crates\switchboot_core\icons\icon.ico
UninstallDisplayIcon={app}\switchboot.exe
WizardSmallImageFile=..\crates\switchboot_core\icons\32x32.png,..\crates\switchboot_core\icons\64x64.png,..\crates\switchboot_core\icons\128x128.png,..\crates\switchboot_core\icons\256x256.png
WizardSmallImageFileDynamicDark=..\crates\switchboot_core\icons\32x32.png,..\crates\switchboot_core\icons\64x64.png,..\crates\switchboot_core\icons\128x128.png,..\crates\switchboot_core\icons\256x256.png
Compression=lzma2
SolidCompression=yes
CloseApplications=yes
; One original-user [Run] launch, after service setup, including legacy clients.
RestartApplications=no

[Files]
Source: "..\target\release\switchboot.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\crates\switchboot_core\resources\icons\ico\*.ico"; DestDir: "{app}\resources\icons\ico"; Flags: ignoreversion

[Icons]
Name: "{commonprograms}\Switchboot"; Filename: "{app}\switchboot.exe"

[Run]
Filename: "{app}\switchboot.exe"; Flags: nowait runasoriginaluser; Check: ShouldRestartApp

[Code]
#define ISSCONFIG_CODE
#include "../issconfig/issconfig.iss"
#include "windows-installation.iss"
#include "windows-shutdown.iss"
#include "windows-maintenance.iss"
#include "windows-legacy.iss"
#include "windows-service.iss"

function GetDefaultDirectory(Param: String): String;
begin
  Result := LegacyDirectory;
  if Result = '' then Result := ExpandConstant('{autopf}\Switchboot');
end;

function ShouldRestartApp: Boolean;
begin
  Result := (ServiceError = '') and
    (MaintenanceReinstalled or HasSwitch('/RESTARTAPP') or HasSwitch('/R'));
end;

function InitializeSetup: Boolean;
begin
  DetectMaintenanceInstallation;
  ValidateUpgradeArguments;
  InitializeLegacyUpdate;
  Result := True;
end;

procedure InitializeWizard;
begin
  IssConfigInitialize;
  InitializeMaintenancePage;
  InitializeLegacyWizard;
end;

procedure DeinitializeSetup;
begin
  IssConfigShutdown;
end;

function ShouldSkipPage(PageID: Integer): Boolean;
begin
  Result := (LegacyPassiveUpgrade or (MaintenanceKey <> '')) and (PageID = wpSelectDir);
end;

function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := MaintenanceCanContinue(CurPageID);
end;

procedure CancelButtonClick(CurPageID: Integer; var Cancel, Confirm: Boolean);
begin
  if MaintenanceCanCloseWithoutConfirmation(CurPageID) then Confirm := False;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  if LegacyRemovalAttempted or MaintenanceReinstalled then Exit;
  if HasSwitch('/UPGRADE') then begin
    Result := PrepareUpgradeShutdown;
    if Result <> '' then begin
      Log('Upgrade handoff failed: ' + Result);
      Exit;
    end;
  end;
  if InstallationExists(LegacyUninstallKey) then
    Result := MigrateLegacyInstallation
  else
    PrepareServiceReplacement;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then InstallWindowsService;
  if (CurStep = ssDone) and (ServiceError = '') then CleanupLegacyInstallation;
end;

function GetCustomSetupExitCode: Integer;
begin
  Result := ServiceSetupExitCode;
end;

procedure CurPageChanged(CurPageID: Integer);
begin
  IssConfigPageChanged(CurPageID);
  MaintenancePageChanged(CurPageID);
  LegacyPageChanged(CurPageID);
  ShowServiceSetupResult(CurPageID);
end;

procedure InitializeUninstallProgressForm;
var
  Error: String;
begin
  Error := PrepareInstalledShutdown(ExpandConstant('{app}'));
  if Error <> '' then RaiseException(Error + ' Application files have been kept.');
  UninstallWindowsService;
end;
