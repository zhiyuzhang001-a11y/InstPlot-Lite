#define AppName "InstPlot Lite"
#define AppVersion GetEnv("INSTPLOT_VERSION")
#define ProjectDir SourcePath + "\..\.."
#define RepositoryDir ProjectDir + "\.."
#define ReleaseExe ProjectDir + "\target\release\instplot-lite.exe"

[Setup]
AppId={{A10B1F24-2AF3-4AA5-90E1-23972F7F2F71}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=InstPlot Project
DefaultDirName={localappdata}\Programs\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir={#ProjectDir}\target\package
OutputBaseFilename=InstPlot-Lite-{#AppVersion}-windows-x64-setup
SetupIconFile={#RepositoryDir}\logo.ico
UninstallDisplayIcon={app}\instplot-lite.exe
LicenseFile={#RepositoryDir}\LICENSE
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "chinesesimp"; MessagesFile: "ChineseSimplified.isl"

[Tasks]
Name: "desktopicon"; Description: "创建桌面快捷方式"; GroupDescription: "附加快捷方式："; Flags: unchecked

[Files]
Source: "{#ReleaseExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepositoryDir}\LICENSE"; DestDir: "{app}\licenses"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "{#ProjectDir}\THIRD_PARTY_NOTICES.md"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "{#ProjectDir}\assets\OFL-Liberation.txt"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "{#ProjectDir}\assets\OFL.txt"; DestDir: "{app}\licenses"; DestName: "OFL-Noto-Sans-SC.txt"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\instplot-lite.exe"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\instplot-lite.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\instplot-lite.exe"; Description: "启动 {#AppName}"; Flags: nowait postinstall skipifsilent
