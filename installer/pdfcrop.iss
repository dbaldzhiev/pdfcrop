; Inno Setup script for PDFCROP.
;
; Packages the Rust tool plus a trimmed Ghostscript runtime into one setup
; executable. Nothing is downloaded during installation and no Perl, TeX, Python
; or pre-installed Ghostscript is required on the target machine.
;
; Build with:  iscc /DAppVersion=2.0.0 installer\pdfcrop.iss
; Expected layout before compiling:
;   staging\pdfcrop.exe        (cargo build --release)
;   staging\gs\bin\gswin64c.exe, gs\bin\gsdll64.dll, gs\lib, gs\Resource,
;   gs\iccprofiles, gs\COPYING

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

#define AppName "PDFCROP"
#define AppPublisher "Dimitar Baldzhiev"
#define AppUrl "https://github.com/dbaldzhiev/pdfcrop"
#define ToolExe "pdfcrop.exe"
; Never change AppId — it is how Windows recognises an upgrade of this app.
#define AppId "{{9C1E6A44-7B52-4A6D-A1F0-3E5B8C2D9F17}"

[Setup]
AppId={#AppId}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppUrl}
AppSupportURL={#AppUrl}/issues
AppUpdatesURL={#AppUrl}/releases

; Per-user by default (no UAC prompt); the user can switch to an all-users
; install in the dialog, and `/ALLUSERS` does the same from the command line.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog commandline
DefaultDirName={autopf}\pdfcrop
DisableDirPage=auto
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
AllowNoIcons=yes
UninstallDisplayIcon={app}\{#ToolExe}
UninstallDisplayName={#AppName} {#AppVersion}

OutputDir=..\dist
OutputBaseFilename=pdfcrop-setup-v{#AppVersion}
SetupIconFile=..\assets\pdfcrop.ico
InfoBeforeFile=NOTICE.txt
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "contextmenu"; Description: "Add ""PDFCROP"" to the right-click menu of PDF files"; GroupDescription: "Shell integration:"

[Files]
Source: "..\staging\{#ToolExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\staging\gs\*"; DestDir: "{app}\gs"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\THIRD-PARTY.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Uninstall {#AppName}"; Filename: "{uninstallexe}"

[Registry]
; One verb for .pdf files. HKA = HKLM when the installer runs elevated, HKCU
; otherwise, so the same script covers both install modes.
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.pdf\shell\PDFCROP"; ValueType: string; ValueName: ""; ValueData: "PDFCROP"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.pdf\shell\PDFCROP"; ValueType: string; ValueName: "MUIVerb"; ValueData: "PDFCROP"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.pdf\shell\PDFCROP"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#ToolExe}"",0"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.pdf\shell\PDFCROP"; ValueType: string; ValueName: "Position"; ValueData: "top"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\SystemFileAssociations\.pdf\shell\PDFCROP\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#ToolExe}"" ""%1"" --pause"; Flags: uninsdeletekey; Tasks: contextmenu

; The 2023 hand-rolled setup wrote a machine-wide Item0 key through a .reg file.
; Remove it on elevated installs so the menu does not show PDFCROP twice.
Root: HKCR; Subkey: "SystemFileAssociations\.pdf\shell\Item0"; Flags: deletekey dontcreatekey; Check: IsAdminInstallMode

[Run]
; Nothing to launch: this is a context-menu tool. Offer the folder instead.
Filename: "{app}"; Description: "Open the installation folder"; Flags: postinstall shellexec skipifsilent unchecked
