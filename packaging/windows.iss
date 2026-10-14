#ifndef VERSION
  #error VERSION must be defined
#endif

; Switchboot owns branding, installation policy and lifecycle events.
#define IssConfigHeading "Install Switchboot"
#define IssConfigIntroduction "Switchboot lets you manage your boot order and choose which operating system starts next."
#define IssConfigFolderSuffix "Switchboot"
#define IssConfigSupportText "Switchboot is free and open source. <a id=""donate"">Donate to support development</a>."
; Branding palette follows crates/switchboot_ui/src/style.rs.
#define IssConfigLightBackground "F5F5F5"
#define IssConfigDarkBackground "171717"
#define IssConfigAccent "00B9E8"
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
WizardStyle=modern dynamic hidebevels
; Neutral backgrounds follow the app palette in crates/switchboot_ui/src/style.rs.
WizardBackColor=#{#IssConfigLightBackground}
WizardBackColorDynamicDark=#{#IssConfigDarkBackground}
WizardImageBackColor=#{#IssConfigLightBackground}
WizardImageBackColorDynamicDark=#{#IssConfigDarkBackground}
WizardSmallImageBackColor=none
WizardSmallImageBackColorDynamicDark=none
WizardSmallImageFile=..\crates\switchboot_core\icons\32x32.png,..\crates\switchboot_core\icons\64x64.png,..\crates\switchboot_core\icons\128x128.png,..\crates\switchboot_core\icons\256x256.png
WizardSmallImageFileDynamicDark=..\crates\switchboot_core\icons\32x32.png,..\crates\switchboot_core\icons\64x64.png,..\crates\switchboot_core\icons\128x128.png,..\crates\switchboot_core\icons\256x256.png
Compression=lzma2
SolidCompression=yes
CloseApplications=yes
RestartApplications=no

[Files]
Source: "..\target\release\switchboot.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\crates\switchboot_core\resources\icons\ico\*.ico"; DestDir: "{app}\resources\icons\ico"; Flags: ignoreversion

[Icons]
Name: "{commonprograms}\Switchboot"; Filename: "{app}\switchboot.exe"

[Code]
#define ISSCONFIG_CODE
#include "../issconfig/issconfig.iss"

const
  DonationURL = '';

procedure OpenDonationLink(Sender: TObject);
var
  ErrorCode: Integer;
begin
  if DonationURL = '' then Exit;
  if not ShellExecAsOriginalUser('open', DonationURL, '', '', SW_SHOWNORMAL,
      ewNoWait, ErrorCode) then
    MsgBox('Unable to open the donation page in your browser.', mbError, MB_OK);
end;

procedure InitializeWizard;
begin
  if DonationURL <> '' then IssConfigOnSupportLink := @OpenDonationLink;
  IssConfigInitialize;
end;

procedure DeinitializeSetup;
begin
  IssConfigShutdown;
end;

const
  LegacyUninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\Switchboot';

var
  ServiceError: String;

function LegacyDirectory: String;
var
  Command: String;
  QuotePosition: Integer;
begin
  Result := '';
  if RegQueryStringValue(HKLM64, LegacyUninstallKey, 'InstallLocation', Result) then
    if DirExists(Result) then
      Exit;
  Result := '';
  if not RegQueryStringValue(HKLM64, LegacyUninstallKey, 'UninstallString', Command) then
    Exit;
  if (Length(Command) > 0) and (Command[1] = '"') then begin
    Delete(Command, 1, 1);
    QuotePosition := Pos('"', Command);
    if QuotePosition = 0 then
      Exit;
    Command := Copy(Command, 1, QuotePosition - 1);
  end;
  { Read the old install location without executing a registry command. }
  if (CompareText(ExtractFileName(Command), 'uninstall.exe') = 0) and FileExists(Command) then
    Result := ExtractFileDir(Command);
end;

function GetDefaultDirectory(Param: String): String;
begin
  Result := LegacyDirectory;
  if Result = '' then
    Result := ExpandConstant('{autopf}\Switchboot');
end;

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
  if Result <> '' then
    Log(Result);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  { Remove the existing service before replacing its executable. }
  if FileExists(ExpandConstant('{app}\switchboot.exe')) then
    Result := RunServiceCommand('uninstall_service');
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  OldDirectory: String;
begin
  if CurStep = ssPostInstall then begin
    { Also handle a service left behind by an older installation. }
    ServiceError := RunServiceCommand('uninstall_service');
    if ServiceError = '' then
      ServiceError := RunServiceCommand('install_service');
    if ServiceError <> '' then
      RaiseException(ServiceError);
  end;
  if (CurStep = ssDone) and (ServiceError = '') then begin
    OldDirectory := LegacyDirectory;
    { Retire the old NSIS registration only after this install succeeds. }
    if (OldDirectory <> '') and
       (CompareText(AddBackslash(OldDirectory), AddBackslash(ExpandConstant('{app}'))) = 0) then begin
      if not RegDeleteKeyIncludingSubkeys(HKLM64, LegacyUninstallKey) then
        Log('Unable to remove the previous installer registration.');
      if not DeleteFile(ExpandConstant('{app}\uninstall.exe')) then
        Log('Unable to remove the previous uninstaller.');
    end;
  end;
end;

function GetCustomSetupExitCode: Integer;
begin
  { Inno catches ssPostInstall exceptions, so explicitly report failure. }
  Result := 0;
  if ServiceError <> '' then
    Result := 1;
end;

procedure CurPageChanged(CurPageID: Integer);
begin
  IssConfigPageChanged(CurPageID);
  if (CurPageID = wpFinished) and (ServiceError <> '') then
    WizardForm.FinishedLabel.Caption :=
      'Switchboot service setup failed. The application files have been installed, ' +
      'but installed mode is unavailable. Resolve the error and run Setup again.' + #13#10#13#10 + ServiceError;
end;

procedure InitializeUninstallProgressForm;
var
  Error: String;
begin
  { Runs after confirmation, before file deletion; an exception is fatal. }
  Error := RunServiceCommand('uninstall_service');
  if Error <> '' then
    RaiseException(Error + ' Application files have been kept.');
end;
