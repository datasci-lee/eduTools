#ifndef AppVersion
  #error AppVersion must be supplied with /DAppVersion=...
#endif
#ifndef ExePath
  #error ExePath must be supplied with /DExePath=...
#endif

#define AppName "EduTools"
#define AppPublisher "EduTools"

[Setup]
AppId=EduTools
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
DefaultDirName={localappdata}\Programs\EduTools
DefaultGroupName=EduTools
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.22000
SetupIconFile=..\..\assets\app-icon\edutools.ico
UninstallDisplayIcon={app}\edutools.exe
ChangesAssociations=yes
CloseApplications=yes
CloseApplicationsFilter=edutools.exe
RestartApplications=no
OutputDir=output
OutputBaseFilename=EduTools-{#AppVersion}-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupLogging=yes

[Languages]
#ifdef UseKorean
Name: "korean"; MessagesFile: "compiler:Languages\Korean.isl"
#else
Name: "english"; MessagesFile: "compiler:Default.isl"
#endif

[Messages]
#ifdef UseKorean
korean.WinVersionTooLowError=Windows 11 이상에서만 설치할 수 있습니다.
#else
english.WinVersionTooLowError=Windows 11 이상에서만 설치할 수 있습니다.
english.SetupAppTitle=EduTools 설치
english.SetupWindowTitle=EduTools {#AppVersion} 설치
english.UninstallAppTitle=EduTools 제거
english.UninstallAppFullTitle=EduTools 제거
#endif

[Tasks]
Name: "startup"; Description: "로그인할 때 자동 실행"; GroupDescription: "추가 작업:"; Flags: unchecked

[Files]
Source: "{#ExePath}"; DestDir: "{app}"; DestName: "edutools.exe"; Flags: ignoreversion restartreplace

[Icons]
Name: "{group}\EduTools"; Filename: "{app}\edutools.exe"; WorkingDir: "{app}"

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "EduTools"; ValueData: """{app}\edutools.exe"""; Tasks: startup; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\.pansetting"; ValueType: string; ValueData: "EduTools.Settings"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\EduTools.Settings"; ValueType: string; ValueData: "EduTools 설정 파일"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\EduTools.Settings\DefaultIcon"; ValueType: string; ValueData: """{app}\edutools.exe"",0"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\EduTools.Settings\shell\open\command"; ValueType: string; ValueData: """{app}\edutools.exe"" ""%1"""; Flags: uninsdeletekey

[Code]
var
  DeleteUserSettings: Boolean;

function InitializeUninstall(): Boolean;
begin
  DeleteUserSettings :=
    MsgBox(
      'EduTools 사용자 설정도 삭제하시겠습니까?' + #13#10 +
      '아니오를 선택하면 %APPDATA%\EduTools 설정은 남습니다.',
      mbConfirmation,
      MB_YESNO or MB_DEFBUTTON2
    ) = IDYES;
  Result := True;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'EduTools');
    if DeleteUserSettings then
      DelTree(ExpandConstant('{userappdata}\EduTools'), True, True, True);
  end;
end;
