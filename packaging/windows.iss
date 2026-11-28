#ifndef EssaysVersion
  #error "Set EssaysVersion from Cargo.toml"
#endif
#ifndef RuntimeDll
  #error "Set RuntimeDll to the redistributable x64 vcruntime140.dll"
#endif

[Setup]
AppId=com.essays.app
AppName=Essays
AppVersion={#EssaysVersion}
AppPublisher=Essays
DefaultDirName={localappdata}\Programs\Essays
DefaultGroupName=Essays
DisableProgramGroupPage=yes
AllowNoIcons=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes
RestartApplications=no
SetupIconFile=..\desktop\assets\icon.ico
UninstallDisplayIcon={app}\essays.exe
OutputDir=..\dist
OutputBaseFilename=Essays-{#EssaysVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes

[Files]
Source: "..\target\x86_64-pc-windows-msvc\release\essays.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RuntimeDll}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Essays"; Filename: "{app}\essays.exe"

[Run]
Filename: "{app}\essays.exe"; Description: "Launch Essays"; Flags: nowait postinstall skipifsilent
