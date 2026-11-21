#ifndef MyAppVersion
  #define MyAppVersion "0.2.0"
#endif

[Setup]
AppId={{B7F5C4D2-9A3E-4E1F-8C2A-6D4B8F1E3A9C}
AppName=Sunset Launcher
AppVersion={#MyAppVersion}
AppPublisher=svarka
AppPublisherURL=https://github.com/sunset-launcher/sunset-launcher
AppSupportURL=https://github.com/sunset-launcher/sunset-launcher/issues
AppUpdatesURL=https://github.com/sunset-launcher/sunset-launcher/releases
DefaultDirName={autopf}\Sunset Launcher
DefaultGroupName=Sunset Launcher
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\..\dist
OutputBaseFilename=sunset-launcher-windows-x64-setup
SetupIconFile=..\..\logo.ico
UninstallDisplayIcon={app}\sunset-launcher.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "ukrainian"; MessagesFile: "compiler:Languages\Ukrainian.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "polish"; MessagesFile: "compiler:Languages\Polish.isl"
Name: "portuguese"; MessagesFile: "compiler:Languages\Portuguese.isl"
Name: "turkish"; MessagesFile: "compiler:Languages\Turkish.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\..\dist\sunset-launcher-windows-x64.exe"; DestDir: "{app}"; DestName: "sunset-launcher.exe"; Flags: ignoreversion
Source: "..\..\logo.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Sunset Launcher"; Filename: "{app}\sunset-launcher.exe"; IconFilename: "{app}\logo.ico"
Name: "{autodesktop}\Sunset Launcher"; Filename: "{app}\sunset-launcher.exe"; IconFilename: "{app}\logo.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\sunset-launcher.exe"; Description: "{cm:LaunchProgram,Sunset Launcher}"; Flags: nowait postinstall skipifsilent

