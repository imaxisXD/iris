; Iris for Windows: a per-user installer (no administrator prompt), as Zed's. Built by CI with
;   ISCC.exe /DBundle=<folder> /DVersion=<x.y.z> /DOut=<folder> iris.iss
; The bundle is iris.exe beside the helper (bundle.ps1): iris-brain.exe, Swift's runtime, and
; llama.cpp's server with Microsoft's C++ runtime under llama\.
; Updates run this silently from Iris (update_win.rs), which opens Iris again itself afterwards.

#ifndef Version
  #define Version "0.0.0"
#endif

[Setup]
; Never change AppId: Windows knows Iris by it, for updates and for uninstalling.
AppId={{E90833F5-BB4D-40A5-8000-F0B8D74BD218}
AppName=Iris
AppVersion={#Version}
AppVerName=Iris {#Version}
AppPublisher=Iris
VersionInfoVersion={#Version}
VersionInfoProductName=Iris
DefaultDirName={localappdata}\Programs\Iris
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
OutputDir={#Out}
OutputBaseFilename=Iris-Setup-x64
SetupIconFile=..\res\iris.ico
UninstallDisplayIcon={app}\iris.exe
UninstallDisplayName=Iris
; Iris, its brain or its model server still running: closed first (an update quits Iris itself).
CloseApplications=force
RestartApplications=no
WizardStyle=modern
Compression=lzma2/ultra64
SolidCompression=yes

[Files]
Source: "{#Bundle}\*"; DestDir: "{app}"; Flags: recursesubdirs ignoreversion

[Icons]
Name: "{userprograms}\Iris"; Filename: "{app}\iris.exe"

[Run]
; After an install by hand, open Iris. An update opens it itself (it must reopen even when the
; installer fails).
Filename: "{app}\iris.exe"; Description: "Open Iris"; Flags: nowait postinstall skipifsilent

[Registry]
; Open at login: removed with Iris (Iris writes it when the switch is on).
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "Iris"; ValueType: none; Flags: uninsdeletevalue dontcreatekey

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM iris.exe /T"; Flags: runhidden; RunOnceId: "StopIris"

[Code]
// Uninstalling: the update download and its log always go; the model (2.8-4.6 GB, in
// %LOCALAPPDATA%\Iris\models, downloaded by setup) goes too unless the person says keep it (its size as setup gave it). Yes is
// the default, and a silent uninstall removes it. Settings (%APPDATA%\Iris) stay, as an update
// keeps them, so installing again picks up where they were.

function ModelTenthsOfGB(Dir: String): Int64;
var
  Rec: TFindRec;
begin
  Result := 0;
  if FindFirst(Dir + '\*.gguf', Rec) then
  try
    repeat
      Result := Result + Int64(Rec.SizeHigh) * 4294967296 + Rec.SizeLow;
    until not FindNext(Rec);
  finally
    FindClose(Rec);
  end;
  // Tenths of a GB as setup counts them (1,000,000,000 bytes: "2.8 GB"), not Explorer's GiB.
  Result := (Result + 50000000) div 100000000;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Iris, Models, Size: String;
  Tenths: Int64;
begin
  if CurUninstallStep <> usPostUninstall then
    exit;
  Iris := ExpandConstant('{localappdata}\Iris');
  Models := Iris + '\models';
  DelTree(Iris + '\update', True, True, True);
  DeleteFile(Iris + '\update.log');
  if DirExists(Models) then
  begin
    Tenths := ModelTenthsOfGB(Models);
    Size := '';
    if Tenths > 0 then
      Size := ' It takes ' + IntToStr(Tenths div 10) + '.' + IntToStr(Tenths mod 10) + ' GB.';
    if UninstallSilent or (MsgBox('Delete the model Iris downloaded too?' + Size + #13#10#13#10 +
        'Keep it only if you''ll install Iris again and don''t want to download it twice.',
        mbConfirmation, MB_YESNO or MB_DEFBUTTON1) = IDYES) then
      DelTree(Models, True, True, True);
  end;
  RemoveDir(Iris);
end;
