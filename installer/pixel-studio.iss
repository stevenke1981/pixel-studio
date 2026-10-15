; Inno Setup script for Pixel Studio (Windows x64).
; Build: ISCC.exe /DAppVersion=0.2.0 /DSrcDir=..\target\release installer\pixel-studio.iss
; Requires installer\ChineseTraditional.isl (fetched by scripts\build-installer.ps1).

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SrcDir
  #define SrcDir "..\target\release"
#endif
#define AppName "Pixel Studio"
#define AppExe "pixel-studio.exe"
#define ProgId "PixelStudio.Image"

[Setup]
AppId={{B7E2A4C1-6F0D-4E8B-9A53-2C1D7F9E4A60}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Ke Sheng Da
AppPublisherURL=https://github.com/stevenke1981/pixel-studio
AppSupportURL=https://github.com/stevenke1981/pixel-studio/issues
AppUpdatesURL=https://github.com/stevenke1981/pixel-studio/releases
VersionInfoVersion={#AppVersion}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE
OutputDir=..\dist
OutputBaseFilename=pixel-studio-setup-windows-x64
SetupIconFile=..\crates\studio-app\assets\pixel-studio.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName} {#AppVersion}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog commandline
ChangesAssociations=yes
CloseApplications=force
RestartApplications=no
ShowLanguageDialog=auto

[Languages]
Name: "chinesetraditional"; MessagesFile: "ChineseTraditional.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
chinesetraditional.GroupIntegration=整合：
chinesetraditional.TaskShell=在檔案總管右鍵選單加入「用 Pixel Studio 編輯／開啟」
chinesetraditional.TaskAssoc=加入圖片的「開啟檔案」清單（PNG、JPEG、WebP、GIF、BMP、ICO、TIFF；不變更預設程式）
chinesetraditional.TaskDefault=將 Pixel Studio 設為 PNG 與 WebP 的預設程式
chinesetraditional.ImageType=Pixel Studio 圖片
english.GroupIntegration=Integration:
english.TaskShell=Add "Edit / Open with Pixel Studio" to the Explorer right-click menu
english.TaskAssoc=Add Pixel Studio to "Open with" for images (PNG, JPEG, WebP, GIF, BMP, ICO, TIFF; keeps your default app)
english.TaskDefault=Make Pixel Studio the default app for PNG and WebP
english.ImageType=Pixel Studio Image

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "contextmenu"; Description: "{cm:TaskShell}"; GroupDescription: "{cm:GroupIntegration}"
Name: "assoc"; Description: "{cm:TaskAssoc}"; GroupDescription: "{cm:GroupIntegration}"
Name: "setdefault"; Description: "{cm:TaskDefault}"; GroupDescription: "{cm:GroupIntegration}"; Flags: unchecked

[Files]
Source: "{#SrcDir}\pixel-studio.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcDir}\studio-cli.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\models\*.onnx"; DestDir: "{app}\models"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.en.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; ProgID + "Open with" list (HKA = HKLM for admin installs, HKCU for per-user installs).
Root: HKA; Subkey: "Software\Classes\{#ProgId}"; ValueType: string; ValueName: ""; ValueData: "{cm:ImageType}"; Flags: uninsdeletekey; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\{#ProgId}\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"",0"; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\{#ProgId}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "{#AppName}"; Flags: uninsdeletekey; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\.png\OpenWithProgids"; ValueType: string; ValueName: "{#ProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".png"; ValueData: ""; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\.jpg\OpenWithProgids"; ValueType: string; ValueName: "{#ProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".jpg"; ValueData: ""; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\.jpeg\OpenWithProgids"; ValueType: string; ValueName: "{#ProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".jpeg"; ValueData: ""; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\.webp\OpenWithProgids"; ValueType: string; ValueName: "{#ProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".webp"; ValueData: ""; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\.gif\OpenWithProgids"; ValueType: string; ValueName: "{#ProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".gif"; ValueData: ""; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\.bmp\OpenWithProgids"; ValueType: string; ValueName: "{#ProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".bmp"; ValueData: ""; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\.ico\OpenWithProgids"; ValueType: string; ValueName: "{#ProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".ico"; ValueData: ""; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\.tif\OpenWithProgids"; ValueType: string; ValueName: "{#ProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".tif"; ValueData: ""; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\.tiff\OpenWithProgids"; ValueType: string; ValueName: "{#ProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc setdefault
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".tiff"; ValueData: ""; Tasks: assoc setdefault
; Opt-in default handler. Windows 10/11 may still ask the user to confirm in Settings.
Root: HKA; Subkey: "Software\Classes\.png"; ValueType: string; ValueName: ""; ValueData: "{#ProgId}"; Flags: uninsdeletevalue; Tasks: setdefault
Root: HKA; Subkey: "Software\Classes\.webp"; ValueType: string; ValueName: ""; ValueData: "{#ProgId}"; Flags: uninsdeletevalue; Tasks: setdefault

[Run]
Filename: "{app}\{#AppExe}"; Parameters: "--register-shell"; Flags: runhidden waituntilterminated; Tasks: contextmenu; StatusMsg: "{cm:TaskShell}"
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{app}\{#AppExe}"; Parameters: "--unregister-shell"; Flags: runhidden waituntilterminated; RunOnceId: "UnregisterShell"

[Code]
function SetEnvironmentVariable(lpName, lpValue: String): Boolean;
  external 'SetEnvironmentVariableW@kernel32.dll stdcall';

procedure CurStepChanged(CurStep: TSetupStep);
begin
  { English installs get English context-menu labels. }
  if CurStep = ssInstall then
    if ActiveLanguage = 'english' then
      SetEnvironmentVariable('PIXEL_STUDIO_LANG', 'en')
    else
      SetEnvironmentVariable('PIXEL_STUDIO_LANG', 'zh-TW');
end;
