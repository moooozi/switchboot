{ Application lifecycle handoff only; no service or firmware commands. }
#ifndef UpgradeGUIWaitMs
  #define UpgradeGUIWaitMs 35000
#endif
#ifndef UpgradeFileWaitAttempts
  #define UpgradeFileWaitAttempts 150
#endif

var
  UpgradeShutdownComplete: Boolean;

{ Inno Setup 6 runs a 32-bit installer, including on x64 Windows. }
function UpgradeOpenProcess(Access: Cardinal; InheritHandle: Boolean; PID: Cardinal): Longint;
  external 'OpenProcess@kernel32.dll stdcall';
function UpgradeOpenEvent(Access: Cardinal; InheritHandle: Boolean; Name: String): Longint;
  external 'OpenEventW@kernel32.dll stdcall';
function UpgradeSetEvent(Handle: Longint): Boolean;
  external 'SetEvent@kernel32.dll stdcall';
function UpgradeWait(Handle: Longint; Milliseconds: Cardinal): Cardinal;
  external 'WaitForSingleObject@kernel32.dll stdcall';
function UpgradeCloseHandle(Handle: Longint): Boolean;
  external 'CloseHandle@kernel32.dll stdcall';
function UpgradeProcessPath(Handle: Longint; Flags: Cardinal; Path: String; var Size: Cardinal): Boolean;
  external 'QueryFullProcessImageNameW@kernel32.dll stdcall';
function UpgradeOpenFile(Name: String; Access, Share: Cardinal; Security: Longint;
  Creation, Flags: Cardinal; Template: Longint): Longint;
  external 'CreateFileW@kernel32.dll stdcall';


function WaitForUpgradeFileRelease(const Directory: String): String;
var
  Attempt, FileHandle, ErrorCode: Integer;
begin
  { Opening for exclusive write access does not write bytes. Windows rejects
    this while an executable image is still mapped by the worker or service. }
  for Attempt := 1 to {#UpgradeFileWaitAttempts} do begin
    FileHandle := UpgradeOpenFile(AddBackslash(Directory) + 'switchboot.exe',
      $40000000, 0, 0, 3, 0, 0);
    if FileHandle <> -1 then begin
      UpgradeCloseHandle(FileHandle);
      Result := '';
      Exit;
    end;
    ErrorCode := DLLGetLastError;
    if (ErrorCode <> 5) and (ErrorCode <> 32) and (ErrorCode <> 33) then begin
      Result := 'Unable to check upgrade file readiness: ' + SysErrorMessage(ErrorCode);
      Exit;
    end;
    Sleep(100);
  end;
  Result := 'Switchboot is still in use after GUI shutdown. The CLI worker or service did not release the executable before the timeout. Setup cannot continue.';
end;

function RequestUpgradeShutdown(PID: Integer; const Directory: String): String;
var
  ProcessHandle, EventHandle: Longint;
  Path: String;
  Size, WaitResult: Cardinal;
begin
  Result := '';
  if PID <= 0 then begin
    Result := '/UPGRADE requires a valid /UPGRADEPID.';
    Exit;
  end;
  { SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION. Keep this handle open
    throughout the handoff so PID reuse cannot change the process we wait for. }
  ProcessHandle := UpgradeOpenProcess($00101000, False, PID);
  if ProcessHandle = 0 then begin
    Result := 'Unable to open the GUI process for upgrade: ' + SysErrorMessage(DLLGetLastError);
    Exit;
  end;
  try
    Size := 32768;
    Path := StringOfChar(#0, Size);
    if not UpgradeProcessPath(ProcessHandle, 0, Path, Size) then begin
      Result := 'Unable to verify the GUI executable: ' + SysErrorMessage(DLLGetLastError);
      Exit;
    end;
    SetLength(Path, Size);
    if CompareText(Path, AddBackslash(Directory) + 'switchboot.exe') <> 0 then begin
      Result := 'The upgrade GUI does not belong to the selected installation.';
      Exit;
    end;
    EventHandle := UpgradeOpenEvent($0002, False, 'Local\Switchboot.Upgrade.' + IntToStr(PID));
    if EventHandle = 0 then begin
      Result := 'Unable to open the GUI upgrade event: ' + SysErrorMessage(DLLGetLastError);
      Exit;
    end;
    try
      if not UpgradeSetEvent(EventHandle) then begin
        Result := 'Unable to request GUI shutdown: ' + SysErrorMessage(DLLGetLastError);
        Exit;
      end;
      Log('Upgrade handoff: requested GUI shutdown, PID ' + IntToStr(PID));
      WaitResult := UpgradeWait(ProcessHandle, {#UpgradeGUIWaitMs});
      if WaitResult = $00000102 then
        Result := 'Switchboot did not finish upgrade shutdown before the timeout. No files or service registration have been changed.'
      else if WaitResult <> 0 then
        Result := 'Unable to wait for GUI shutdown: ' + SysErrorMessage(DLLGetLastError)
      else
        Log('Upgrade handoff: GUI exited.');
    finally
      UpgradeCloseHandle(EventHandle);
    end;
  finally
    UpgradeCloseHandle(ProcessHandle);
  end;
end;

function PrepareUpgradeShutdown: String;
begin
  Result := '';
  if UpgradeShutdownComplete then Exit;
  Result := RequestUpgradeShutdown(UpgradePID, ExpandConstant('{app}'));
  if Result = '' then Result := WaitForUpgradeFileRelease(ExpandConstant('{app}'));
  if Result = '' then begin
    UpgradeShutdownComplete := True;
    Log('Upgrade handoff: executable released; shutdown complete.');
  end;
end;

function UpgradeEnumProcesses(PIDs: String; Size: Cardinal;
  var Needed: Cardinal): Boolean;
  external 'K32EnumProcesses@kernel32.dll stdcall';

function PrepareInstalledShutdown(const Directory: String): String;
var
  PIDs: String;
  Needed, Size, PID: Cardinal;
  Index, ProcessHandle, EventHandle: Integer;
  Path: String;
begin
  Result := '';
  if not FileExists(AddBackslash(Directory) + 'switchboot.exe') then Exit;
  { Only processes at this installation path with a GUI-owned event are
    signaled. CLI workers and the SCM service have no such event. }
  PIDs := StringOfChar(#0, 65536);
  if not UpgradeEnumProcesses(PIDs, Length(PIDs) * 2, Needed) or
     (Needed >= Cardinal(Length(PIDs) * 2)) then begin
    Result := 'Unable to enumerate running Switchboot instances.';
    Exit;
  end;
  for Index := 0 to (Needed div 4) - 1 do begin
    PID := Ord(PIDs[Index * 2 + 1]) + Ord(PIDs[Index * 2 + 2]) * 65536;
    ProcessHandle := UpgradeOpenProcess($1000, False, PID);
    if ProcessHandle <> 0 then begin
      try
        Size := 32768;
        Path := StringOfChar(#0, Size);
        if UpgradeProcessPath(ProcessHandle, 0, Path, Size) then begin
          SetLength(Path, Size);
          if CompareText(Path, AddBackslash(Directory) + 'switchboot.exe') = 0 then begin
            EventHandle := UpgradeOpenEvent($0002, False,
              'Local\Switchboot.Upgrade.' + IntToStr(PID));
            if EventHandle <> 0 then begin
              UpgradeCloseHandle(EventHandle);
              Result := RequestUpgradeShutdown(PID, Directory);
              if Result <> '' then Exit;
            end;
          end;
        end;
      finally
        UpgradeCloseHandle(ProcessHandle);
      end;
    end;
  end;
  Result := WaitForUpgradeFileRelease(Directory);
end;
