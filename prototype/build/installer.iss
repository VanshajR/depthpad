; Depthpad installer (Inno Setup 6).
; Build: iscc /DAppVersion=0.1.0 build\installer.iss   (after PyInstaller has produced dist\Depthpad)
; Per-user install: no admin rights needed for Depthpad itself. The ViGEmBus driver installer, run only when
; the driver is missing, asks for admin rights on its own.

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#define AppName "Depthpad"
#define AppExe "Depthpad.exe"
#define ViGEmSetup "ViGEmBus_1.22.0_x64_x86_arm64.exe"

[Setup]
AppId={{6F3D2B7A-8C41-4E5B-9A0D-1D2E3F4A5B6C}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppName}
AppPublisherURL=https://github.com/VanshajR/depthpad
AppSupportURL=https://github.com/VanshajR/depthpad/issues
AppUpdatesURL=https://github.com/VanshajR/depthpad/releases
DefaultDirName={localappdata}\Programs\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=..\dist
OutputBaseFilename=Depthpad-{#AppVersion}-setup
SetupIconFile=assets\depthpad.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
LicenseFile=..\LICENSE
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes
RestartApplications=no
VersionInfoVersion={#AppVersion}
VersionInfoProductName={#AppName}

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked
Name: "autostart"; Description: "Start Depthpad with Windows (in the tray)"; GroupDescription: "Start-up:"; Flags: unchecked

[Files]
Source: "..\dist\Depthpad\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "redist\{#ViGEmSetup}"; DestDir: "{tmp}"; Flags: deleteafterinstall; Check: NeedsViGEmBus

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "{#AppName}"; \
    ValueData: """{app}\{#AppExe}"" --minimized"; Tasks: autostart

[Run]
Filename: "{tmp}\{#ViGEmSetup}"; StatusMsg: "Installing the ViGEmBus controller driver…"; \
    Flags: shellexec waituntilterminated; Check: NeedsViGEmBus
Filename: "{app}\{#AppExe}"; Description: "Start Depthpad now"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{cmd}"; Parameters: "/C taskkill /IM {#AppExe} /F"; Flags: runhidden; RunOnceId: "StopDepthpad"

[Code]
function NeedsViGEmBus: Boolean;
begin
  Result := not RegKeyExists(HKLM, 'SYSTEM\CurrentControlSet\Services\ViGEmBus');
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  { The app can turn on "Start with Windows" itself; never leave that entry behind. }
  if CurUninstallStep = usPostUninstall then
    RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', '{#AppName}');
end;
