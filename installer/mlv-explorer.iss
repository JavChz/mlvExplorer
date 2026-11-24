#ifndef MyAppVersion
  #define MyAppVersion "0.0.1"
#endif
#ifndef SourceExe
  #define SourceExe "target\x86_64-pc-windows-msvc\release\mlv-explorer.exe"
#endif
#ifndef OutputDir
  #define OutputDir "dist"
#endif
#ifndef OutputBaseFilename
  #define OutputBaseFilename "mlv-explorer-v" + MyAppVersion + "-windows-x64-setup"
#endif

[Setup]
SourceDir=..
AppId={{8E9A74E2-4B3C-4E1D-9F6A-2C8B5A1D3E7F}
AppName=MLV Explorer
AppVersion={#MyAppVersion}
AppVerName=MLV Explorer {#MyAppVersion}
AppPublisher=MLV Explorer Contributors
AppPublisherURL=https://github.com/JavChz/mlvExplorer
AppSupportURL=https://github.com/JavChz/mlvExplorer/issues
AppUpdatesURL=https://github.com/JavChz/mlvExplorer/releases
DefaultDirName={autopf}\MLV Explorer
DefaultGroupName=MLV Explorer
AllowNoIcons=yes
LicenseFile=LICENSE
OutputDir={#OutputDir}
OutputBaseFilename={#OutputBaseFilename}
SetupIconFile=assets\icons\mlv-explorer.ico
UninstallDisplayIcon={app}\mlv-explorer.ico
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
CloseApplications=force
RestartApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "mlv-explorer.exe"; Flags: ignoreversion
Source: "README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "assets\icons\mlv-explorer.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\MLV Explorer"; Filename: "{app}\mlv-explorer.exe"; IconFilename: "{app}\mlv-explorer.ico"
Name: "{group}\{cm:UninstallProgram,MLV Explorer}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\MLV Explorer"; Filename: "{app}\mlv-explorer.exe"; IconFilename: "{app}\mlv-explorer.ico"; Tasks: desktopicon

[UninstallDelete]
Type: files; Name: "{app}\.installed"

[Run]
Filename: "{app}\mlv-explorer.exe"; Description: "{cm:LaunchProgram,MLV Explorer}"; Flags: nowait postinstall skipifsilent

[Code]
procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    SaveStringToFile(ExpandConstant('{app}\.installed'), ExpandConstant('{#MyAppVersion}'), False);
  end;
end;
