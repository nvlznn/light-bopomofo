; 輕注音 for Windows — Inno Setup script.
; Built by windows/build.ps1 (locally) or CI from a staging folder:
;   stage\x64\lightbopomofo.dll    stage\x86\lightbopomofo.dll    stage\arm64\lightbopomofo.dll
;   stage\x64\LightBopomofo.exe    stage\arm64\LightBopomofo.exe
;   stage\lexicon.bin  stage\lightbopomofo.ico  stage\lightbopomofo-input.ico  stage\licenses\*
; iscc /DAppVersion=0.1.0 /DStage=path\to\stage /DOutput=path\to\dist installer.iss

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef Stage
  #define Stage "..\target\windows-stage"
#endif
#ifndef Output
  #define Output "..\dist"
#endif

[Setup]
AppId={{6F1B3C7A-2E54-4C1B-9B0D-7C3A5E1F2B90}
AppName=輕注音
AppVerName=輕注音 {#AppVersion}
AppVersion={#AppVersion}
AppPublisher=Ino Chueh
AppPublisherURL=https://lightbopomofo.noky.dev
AppSupportURL=https://github.com/nvlznn/light-bopomofo/issues
DefaultDirName={autopf}\LightBopomofo
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible arm64
ArchitecturesInstallIn64BitMode=x64compatible arm64
MinVersion=10.0
OutputDir={#Output}
OutputBaseFilename=LightBopomofo-Setup
SetupIconFile={#Stage}\lightbopomofo.ico
UninstallDisplayIcon={app}\LightBopomofo.exe
UninstallDisplayName=輕注音
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
CloseApplications=no

[Languages]
#if FileExists(AddBackslash(SourcePath) + "ChineseTraditional.isl")
Name: "zh"; MessagesFile: "ChineseTraditional.isl"
#else
Name: "en"; MessagesFile: "compiler:Default.isl"
#endif

[Files]
; Shared data. lexicon.bin is compiled from the chewing tsi.csv shipped in licenses\.
Source: "{#Stage}\lexicon.bin"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\lightbopomofo.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\lightbopomofo-input.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion
; The input method, one DLL per architecture of the applications that load it.
; On Arm64, x64 applications (emulated) share the 64-bit registration with
; Arm64 ones, so only the Arm64 DLL is registered there.
; uninsrestartdelete: applications still running when uninstalling hold the
; DLL; it is then deleted at the next restart.
Source: "{#Stage}\x64\lightbopomofo.dll"; DestDir: "{app}\x64"; Flags: ignoreversion uninsrestartdelete; Check: IsX64OS
Source: "{#Stage}\arm64\lightbopomofo.dll"; DestDir: "{app}\arm64"; Flags: ignoreversion uninsrestartdelete; Check: IsArm64
Source: "{#Stage}\x86\lightbopomofo.dll"; DestDir: "{app}\x86"; Flags: ignoreversion uninsrestartdelete
; The app.
Source: "{#Stage}\x64\LightBopomofo.exe"; DestDir: "{app}"; Flags: ignoreversion; Check: IsX64OS
Source: "{#Stage}\arm64\LightBopomofo.exe"; DestDir: "{app}"; Flags: ignoreversion; Check: IsArm64

[Icons]
; AppUserModelID: lets the app show notifications (update.rs APP_ID).
Name: "{autoprograms}\輕注音"; Filename: "{app}\LightBopomofo.exe"; AppUserModelID: "Noky.LightBopomofo"

[Registry]
; lightbopomofo:update — what the update notification opens.
Root: HKLM; Subkey: "Software\Classes\lightbopomofo"; ValueType: string; ValueName: ""; ValueData: "URL:輕注音"; Flags: uninsdeletekey
Root: HKLM; Subkey: "Software\Classes\lightbopomofo"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKLM; Subkey: "Software\Classes\lightbopomofo\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\LightBopomofo.exe"" ""%1"""

[Run]
Filename: "{sys}\regsvr32.exe"; Parameters: "/s ""{app}\x64\lightbopomofo.dll"""; Flags: runhidden waituntilterminated; Check: IsX64OS
Filename: "{sys}\regsvr32.exe"; Parameters: "/s ""{app}\arm64\lightbopomofo.dll"""; Flags: runhidden waituntilterminated; Check: IsArm64
Filename: "{syswow64}\regsvr32.exe"; Parameters: "/s ""{app}\x86\lightbopomofo.dll"""; Flags: runhidden waituntilterminated
; Check for updates at sign-in and once a day (the app can turn it off).
Filename: "{app}\LightBopomofo.exe"; Parameters: "--register-update-task"; Flags: runhidden waituntilterminated
; Add 輕注音 to the input methods of the user who ran the installer.
Filename: "{app}\LightBopomofo.exe"; Parameters: "--enable"; Flags: runasoriginaluser runhidden waituntilterminated
Filename: "{app}\LightBopomofo.exe"; Description: "開啟輕注音"; Flags: postinstall nowait skipifsilent runasoriginaluser

[UninstallRun]
Filename: "{app}\LightBopomofo.exe"; Parameters: "--unregister-update-task"; Flags: runhidden waituntilterminated; RunOnceId: "unregtask"
Filename: "{app}\LightBopomofo.exe"; Parameters: "--disable"; Flags: runhidden waituntilterminated; RunOnceId: "disable"
Filename: "{sys}\regsvr32.exe"; Parameters: "/u /s ""{app}\x64\lightbopomofo.dll"""; Flags: runhidden waituntilterminated; Check: IsX64OS; RunOnceId: "unreg64"
Filename: "{sys}\regsvr32.exe"; Parameters: "/u /s ""{app}\arm64\lightbopomofo.dll"""; Flags: runhidden waituntilterminated; Check: IsArm64; RunOnceId: "unregarm64"
Filename: "{syswow64}\regsvr32.exe"; Parameters: "/u /s ""{app}\x86\lightbopomofo.dll"""; Flags: runhidden waituntilterminated; RunOnceId: "unreg32"


[Code]
{ A loaded DLL cannot be overwritten, but it can be renamed. Every running
  application that types has the input method loaded, so before copying the
  new files, move the old DLLs aside: running applications keep using the
  old copy, new ones load the new one, and nobody has to restart. Copies
  left from earlier updates are deleted when no longer in use. }
procedure MoveFileAside(const Dir, Name, Stem: String);
var
  Dll, Old: String;
  N: Integer;
begin
  Dll := Dir + '\' + Name;
  if not FileExists(Dll) then
    Exit;
  N := 0;
  repeat
    Old := Dir + '\' + Stem + '.' + IntToStr(N) + '.old';
    if FileExists(Old) then
      DeleteFile(Old);
    N := N + 1;
  until (not FileExists(Old)) or (N > 50);
  if not RenameFile(Dll, Old) then
    Log('Could not move ' + Dll + ' aside');
end;

procedure MoveAside(const Dir: String);
begin
  MoveFileAside(Dir, 'lightbopomofo.dll', 'lightbopomofo');
end;

procedure CleanOld(const Dir: String);
var
  FindRec: TFindRec;
begin
  if FindFirst(Dir + '\*.old', FindRec) then
  begin
    try
      repeat
        DeleteFile(Dir + '\' + FindRec.Name);
      until not FindNext(FindRec);
    finally
      FindClose(FindRec);
    end;
  end;
end;

const
  MOVEFILE_DELAY_UNTIL_REBOOT = 4;

function MoveFileExW(Existing: String; NewName: Cardinal; Flags: Cardinal): Boolean;
  external 'MoveFileExW@kernel32.dll stdcall';

{ Delete DLLs moved aside by updates; one still loaded by a running
  application is deleted at the next restart, and so is its folder. }
procedure DeleteOldOrAtRestart(const Dir: String);
var
  FindRec: TFindRec;
  Path: String;
begin
  if FindFirst(Dir + '\*.old', FindRec) then
  begin
    try
      repeat
        Path := Dir + '\' + FindRec.Name;
        if not DeleteFile(Path) then
          MoveFileExW(Path, 0, MOVEFILE_DELAY_UNTIL_REBOOT);
      until not FindNext(FindRec);
    finally
      FindClose(FindRec);
    end;
  end;
  if not RemoveDir(Dir) then
    MoveFileExW(Dir, 0, MOVEFILE_DELAY_UNTIL_REBOOT);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    DeleteOldOrAtRestart(ExpandConstant('{app}\x64'));
    DeleteOldOrAtRestart(ExpandConstant('{app}\x86'));
    DeleteOldOrAtRestart(ExpandConstant('{app}\arm64'));
    DeleteOldOrAtRestart(ExpandConstant('{app}'));
    if not RemoveDir(ExpandConstant('{app}')) then
      MoveFileExW(ExpandConstant('{app}'), 0, MOVEFILE_DELAY_UNTIL_REBOOT);
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssInstall then
  begin
    MoveAside(ExpandConstant('{app}\x64'));
    MoveAside(ExpandConstant('{app}\x86'));
    MoveAside(ExpandConstant('{app}\arm64'));
    { The app may be open (it starts the installer when updating itself). }
    MoveFileAside(ExpandConstant('{app}'), 'LightBopomofo.exe', 'LightBopomofo');
  end;
  if CurStep = ssPostInstall then
  begin
    CleanOld(ExpandConstant('{app}\x64'));
    CleanOld(ExpandConstant('{app}\x86'));
    CleanOld(ExpandConstant('{app}\arm64'));
    CleanOld(ExpandConstant('{app}'));
  end;
end;
