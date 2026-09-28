; Inno Setup script for the Windows installer. Built in CI:
;   ISCC.exe /DAppVersion=0.1.1 /DSourceExe=C:\path\to\ccswitch.exe /Oout packaging\windows\ccswitch.iss
; Installs for the current user (no admin prompt) and adds the install folder to the user PATH.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceExe
  #define SourceExe "..\..\target\x86_64-pc-windows-msvc\release\ccswitch.exe"
#endif

[Setup]
AppId={{8F3C2A51-6B1E-4C7D-9A2F-3E5B7C9D1A42}
AppName=ccswitch
AppVersion={#AppVersion}
AppVerName=ccswitch {#AppVersion}
AppPublisher=Mostafa Elgazar
AppPublisherURL=https://github.com/mostafaelgazar48/ccswitch
AppSupportURL=https://github.com/mostafaelgazar48/ccswitch/issues
DefaultDirName={localappdata}\Programs\ccswitch
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesEnvironment=yes
LicenseFile=..\..\LICENSE
OutputBaseFilename=ccswitch-{#AppVersion}-windows-x86_64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayName=ccswitch

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; \
  ValueData: "{olddata};{app}"; Check: NeedsAddPath(ExpandConstant('{app}'))

[Code]
function PathIndex(Dir, Path: string): Integer;
begin
  Result := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Path) + ';');
end;

function NeedsAddPath(Dir: string): Boolean;
var
  Path: string;
begin
  if not RegQueryStringValue(HKCU, 'Environment', 'Path', Path) then
    Result := True
  else
    Result := PathIndex(Dir, Path) = 0;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Path, Dir: string;
  P: Integer;
begin
  if CurUninstallStep <> usPostUninstall then
    exit;
  if not RegQueryStringValue(HKCU, 'Environment', 'Path', Path) then
    exit;
  Dir := ExpandConstant('{app}');
  P := PathIndex(Dir, Path);
  if P = 0 then
    exit;
  if P = 1 then
    Delete(Path, 1, Length(Dir) + 1)   { entry and the ';' after it }
  else
    Delete(Path, P - 1, Length(Dir) + 1); { the ';' before it and the entry }
  RegWriteExpandStringValue(HKCU, 'Environment', 'Path', Path);
end;
