; Shard Node Installer Script for Inno Setup
; Download Inno Setup: https://jrsoftware.org/isinfo.php

#define MyAppName "Shard Node"
#define MyAppVersion "0.4.2"
#define MyAppPublisher "Shard Project"
#define MyAppURL "https://github.com/TrentPierce/Shard"
#define MyAppExeName "ShardAI.exe"

[Setup]
AppId={{8A3E4F2D-9B1C-4E5A-8F6D-2C7B3A1E9D0F}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\ShardNode
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
LicenseFile=..\LICENSE
OutputDir=..\dist
OutputBaseFilename=Shard_Setup_{#MyAppVersion}_windows_x64
SetupIconFile=..\web\public\icon.ico
Compression=lzma
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "startup"; Description: "Run Shard Node at Windows startup"; GroupDescription: "Startup Options:"

[Files]
Source: "..\dist\ShardAI\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
; Auto-start via Run key
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "ShardNode"; ValueData: """{app}\{#MyAppExeName}"" --background"; Flags: uninsdeletevalue; Tasks: startup

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[Code]
function InitializeSetup(): Boolean;
begin
  Result := True;
end;
