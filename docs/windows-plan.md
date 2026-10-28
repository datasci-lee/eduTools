# EduTools Windows 개발 인계서 (Rust, macOS Swift 유지)

2026-10-28 · Windows PC 에서 이 문서 하나로 착수한다. 정본: `docs/design.md`(기능·단축키) · `Sources/EduTools/*.swift`(동작). `[확인 필요]` 는 §9 에서 실기로 판별.

## 작업 규칙 (Windows 쪽 사람·AI 도구 공통)

| 규칙 | |
| --- | --- |
| 동작 정본 = macOS Swift | 같은 입력 → 같은 결과. 애매하면 Swift 를 읽고, Swift 와 design.md 가 다르면 사용자에게 묻는다 |
| 편집 범위 | `windows/`·`fixtures/`·`.github/workflows/windows.yml` 만. `Sources/`·`build.sh` 는 읽기 전용 |
| 기본 브랜치 직접 push 금지 | `feature/win-<단계>` → PR → CI 초록 + 실기 체크리스트 후 머지 |
| 시험 | 새 시험 체계를 만들지 않는다. `SelfTest.swift` 의 단언을 같은 입력·기대값으로 옮긴 것만(§2.2) |
| 의존성 | `windows`·`serde`·`serde_json` 외 추가는 사용자 승인 |

## 0. 선택지 판정 (사용자 결정: C)

| 안 | 한 줄 판정 |
| --- | --- |
| A. C#/.NET 또는 C++ 네이티브 | 설정 UI 는 쉽지만 핵심(오버레이·훅·캡처·DComp)은 결국 P/Invoke·COM 이고, .NET 8 은 2026-11 지원 종료라 쓰려면 .NET 10 |
| B. Rust 공통 코어 + Swift FFI | 공유 대상은 코드가 아니라 **파일 포맷**(.pansetting·단축키 토큰)이다. 순수 로직은 ≈3k줄이고 Ink/Board 는 `NSColor`, Shortcut 은 `kVK_*` 에 묶여 있어 macOS 재배선 비용이 이득보다 크다 |
| **C. Rust 단독 Windows 앱** | **채택.** windows-rs 로 Win32·COM·WinRT 1급 바인딩, 단일 exe, 런타임 없음. 다만 코어 크레이트를 OS 무관으로 분리해 두면 나중에 B 로 갈 수 있다 |
| D. Tauri/Electron/Flutter | 설계서 판단이 Windows 에도 맞다. 클릭 통과 토글·WDA·WM_POINTER 압력·화면 캡처가 전부 네이티브 우회라 결국 Win32 를 직접 짠다 |

## 1. 크레이트·기술 선택

| 영역 | 선택 | 이유 |
| --- | --- | --- |
| OS 바인딩 | `windows`(Microsoft 공식 windows-rs) 하나만, feature 최소 지정. `windows-sys` 는 안 씀 | D2D·DComp·WIC·CoreAudio·WGC 가 COM/WinRT 라 `windows` 가 필수. 두 크레이트 혼용은 타입이 갈라진다 |
| 렌더링 | **DirectComposition + Direct2D**(D3D11 디바이스 공유), 창은 `WS_EX_NOREDIRECTIONBITMAP` | 프리멀티플라이드 알파 투명 오버레이를 GPU 로 그린다. `UpdateLayeredWindow` 는 4K 전체화면에서 CPU 비용이 크다. wgpu 는 2D 획·안티앨리어싱을 직접 짜야 하고 의존 트리가 크다 |
| 획 구조 | 완성 획 = 모니터별 D2D 비트맵, 진행 획 = 별도 DComp 비주얼 | macOS 구조(비트맵 + `CAShapeLayer`)와 1:1. 형광펜 "겹쳐도 안 진해짐" = 불투명으로 그리고 `PushLayer(opacity)` |
| 텍스트 | DirectWrite | 도움말·타이머·배지·키 표시 |
| 줌 효과 | D2D 내장 효과(Saturation·Brightness) | `CIColorControls` 대응 |
| 이미지 저장 | WIC(PNG·JPEG), 클립보드 `CF_DIBV5` + `PNG` 등록 형식 | OS 내장 |
| PDF | 직접 작성(페이지 = JPEG `DCTDecode` 이미지, ≈150줄) | Win32 에 PDF 작성 API 가 없다. 압축 크레이트 없이 됨. 벡터 품질 요구가 나오면 그때 경로 연산자로 |
| 설정 UI | **Win32 직접**(Common Controls v6: Tab·Trackbar·Edit·ComboBox·`HOTKEY_CLASS`) | 의존 0, 한글 IME 기본 동작. 비용이 커지면 WebView2(HTML) 가 대안 — 단 `webview2-com` 의존 추가 |
| 텍스트 도구·도움말 검색 | 오버레이 위에 띄우는 `EDIT` 컨트롤 | 한글 IME 조합을 직접 구현하지 않는다 |
| JSON | `serde` + `serde_json` | .pansetting 동치(§2.2). |
| 그 외 | 없음(UUID=`CoCreateGuid`, 시각=`SystemTime`/`GetLocalTime`, 로그=파일+`OutputDebugStringW`, 오류=`windows::core::Result`) | 의존을 `windows`·`serde`·`serde_json` 셋으로 묶는다 |
| 매니페스트·아이콘 | `build.rs` 에서 `rc.exe` → `.res` 링크(PerMonitorV2·Common Controls v6 선언) | comctl v6 는 매니페스트가 있어야 로드된다. 크레이트 불필요 |

## 2. 모듈 구조

### 2.1 Swift ↔ Rust 대응표

크레이트 둘: `edutools-core`(OS 의존 0, 어느 OS 에서나 `cargo test`) · `edutools`(Windows 앱 바이너리).

| Swift | Rust | 크레이트 | 비고 |
| --- | --- | --- | --- |
| Ink.swift | `ink.rs` | core | 획·점(비율 좌표 0~1)·`InkCommand`·Undo 100단계. 색은 `[f64;4]` RGBA |
| Eraser.swift | `eraser.rs` | core | 부분/획/영역·판정 기준·대상 필터 |
| ShapeSelection.swift | `shape.rs` | core | 도형 인식·올가미·선택 변환·화면 가리개 |
| Zoom.swift | `zoom.rs` | core | 배율·따라가기 3방식·가장자리 고정 |
| TimerEngine.swift · TimerCoordinator.swift | `timer.rs` | core | 종료 시각 기반 계산, 자동 연결 |
| Board.swift | `board.rs` | core | 페이지·패턴·블랙보드 색 치환 |
| SettingsModel.swift | `settings.rs` | core | 스키마·검증기·`SettingsDocument`·묶음 선택. 저장소(UserDefaults)만 앱 쪽 |
| Shortcut.swift | `shortcut.rs` | core | 토큰 문법(`AppMod+P`)·카탈로그·충돌 검사. 키코드 표는 앱 쪽 |
| AppState.swift · CommandDispatcher.swift | `state.rs` · `dispatch.rs` | core | 모드 전이·명령 라우팅(부작용은 콜백) |
| BreakMusic.swift(순수부: URL 검증·페이드 곡선) | `music.rs` | core | |
| SelfTest.swift | `selftest.rs` | core | §2.2 |
| main.swift | `main.rs` | app | 메시지 루프·단일 인스턴스(명명 뮤텍스)·`EnableMouseInPointer` |
| StatusMenu.swift | `tray.rs` | app | `Shell_NotifyIconW`·`TrackPopupMenu` |
| HotkeyCenter.swift | `hotkey.rs` · `kbhook.rs` | app | 전역 7개 `RegisterHotKey` / 모드 안 키 `WH_KEYBOARD_LL` |
| (Shortcut.swift 의 kVK 표) | `keymap.rs` | app | 토큰 ↔ 스캔코드/VK |
| ScreenIdentity.swift | `monitor.rs` | app | `QueryDisplayConfig` 의 `monitorDevicePath` 가 UUID 대응(`\\.\DISPLAY1` 은 바뀐다) |
| OverlayController.swift | `overlay.rs` | app | 모니터별 DComp 창, 클릭 통과 토글 |
| InkView.swift | `ink_view.rs` · `render.rs` | app | WM_POINTER 입력·D2D 그리기 |
| PaletteController.swift | `palette.rs` | app | 가로 + 원형. 원형의 각도·구역 판정은 core 로 |
| HelpController.swift | `help.rs` | app | 카탈로그에서 생성 |
| LessonTimerController · BreakTimerController | `lesson_badge.rs` · `break_screen.rs` | app | |
| BreakMusic.swift(나머지) | `break_music.rs` | app | §3 |
| ZoomController.swift | `zoom_view.rs` · `capture.rs` | app | §3 |
| PresenterEffects.swift | `presenter.rs` | app | 스포트라이트·커서 강조·키 표시(`WH_*_LL`) |
| ExportController · ArchiveController | `export.rs` · `archive.rs` | app | |
| SettingsController.swift | `settings_ui.rs` · `settings_store.rs` | app | `%APPDATA%\EduTools\settings.json` |

### 2.2 동작 동치·스키마 동치를 지키는 방법

| 대상 | 방법 | 시험 수 영향 |
| --- | --- | --- |
| 순수 로직 | `SelfTest.swift` 의 단언을 **같은 입력·같은 기대값으로** `selftest.rs` 에 옮긴다. 기대값이 같으면 동치. 이산 결과(갈라진 획 수·도형 종류·적중 여부)는 정확히, 좌표는 `1e-9` 허용 | macOS 0, Rust 는 이식분만(`cargo test` 1개 + `edutools.exe --selftest`) |
| 동치 추적 | Rust 함수마다 대응 Swift 심볼 한 줄 주석. 이후 Swift 로직 PR 은 Rust 대응 수정 또는 `tasks/park-*` 를 동반 | — |
| .pansetting | 저장소 루트 `fixtures/pansetting/` 에 **macOS 가 내보낸 파일 1개 + Windows 가 내보낸 파일 1개**. 양쪽 SelfTest 의 기존 왕복 점검(`SelfTest.swift:783` 부근)이 두 파일을 decode 해 기본값과 같은지 본다 | macOS: 기존 점검 입력만 추가. 새 함수 0 |
| 기본 단축키 | `fixtures/shortcuts-default.json`(명령 → 토큰·범위). 양쪽 카탈로그 점검이 같은 파일과 대조 | 같음 |
| 기본값 | 위 픽스처가 "기본값 = 양 OS 동일" 을 강제. macOS 기본값을 바꾸면 픽스처가 빨개져 Windows 동기를 요구 | — |

serde 세부(Swift Codable 과 맞출 것):
- 필드명 그대로(camelCase), `#[serde(default)]` 금지 — Swift 는 비-Optional 키 누락 시 실패한다.
- `Option` 은 `skip_serializing_if = "Option::is_none"` — Swift 는 nil 을 키 생략으로 쓴다.
- 알 수 없는 키 무시 = 양쪽 기본 동작(`deny_unknown_fields` 금지).
- 정수 필드는 `i64`, 실수는 `f64`. Rust 는 `18.0`, Swift 는 `18` 을 쓰지만 상호 decode 된다 — 바이트 비교 금지, 구조 비교만.
- OS 별 의미가 다른 값은 **원값 보존 + 표시만 매핑**: `appModifier: controlOption` → Ctrl+Alt, `notificationSound: "Glass"` → Windows 시스템음 별칭, `musicBrowser` 동일 토큰. 검증기가 상대 OS 값을 거부하면 안 된다.
- `createdOS` 는 자유 문자열. `archiveFolderPath` 는 이 컴퓨터 전용(기본 제외) — 그대로.
- 보관(판서·보드) JSON 은 OS 간 교환 대상 아님 → Windows 고유 형식 허용. 단 파일에 `formatVersion` 을 처음부터 넣는다(macOS 보관 형식엔 없음).

## 3. 기능별 구현 수단·함정

| 기능 | 수단 | 함정 |
| --- | --- | --- |
| 투명 클릭 통과 오버레이 | 모니터별 `WS_EX_TOPMOST·TOOLWINDOW·NOACTIVATE·NOREDIRECTIONBITMAP` + DComp. 일반 모드 `WS_EX_TRANSPARENT`(+`WS_EX_LAYERED`) 켜기, 판서 모드 끄기. `WM_POINTERACTIVATE` → `PA_NOACTIVATE` | 슬라이드쇼·다른 TOPMOST 창과 z순서 경쟁 → 주기적 재확보. 시작 메뉴·알림 위는 `uiAccess=true`(서명 + Program Files 설치) 필요. 독점 전체화면 게임 위는 불가 |
| 전역 단축키 | `RegisterHotKey`(`MOD_CONTROL\|MOD_ALT\|MOD_NOREPEAT`) | 실패 = 선점(설계서대로 즉시 알림). VK 기반이라 한글 IME 무관. AltGr 배열 감지 시 +Shift |
| 판서 모드 한 글자 키 | **`WH_KEYBOARD_LL`**(판서·줌 모드 중에만 설치), 바인딩 키만 삼키고 나머지 통과 | 오버레이가 포커스를 가지면 한글 IME 에서 `VK_PROCESSKEY` 로 와서 원칙 5 가 깨지고 ←/→ 전달을 `SendInput` 으로 되돌려야 한다 → 훅이 맞다. 훅 콜백이 `LowLevelHooksTimeout` 을 넘기면 **조용히 제거**됨(전용 스레드·즉시 반환). 관리자 권한 창이 전경이면 UIPI 로 못 받음. 텍스트 입력 중엔 통과 |
| 펜 압력·지우개 끝 | `WM_POINTER*` + `GetPointerPenInfo(History)`, 압력 0~1024, `PEN_FLAG_ERASER`/`PEN_FLAG_INVERTED` | Wacom 은 드라이버 "Windows Ink 사용" 이 꺼져 있으면 WM_POINTER 가 안 온다 → 첫 실행 안내. 합쳐진 점은 History 로 받아야 스무딩이 안 끊긴다 |
| 실시간 줌 | Win11: **WGC**(`CreateForMonitor`, `CreateFreeThreaded` 프레임 풀) → D3D11 → D2D. 대안 Desktop Duplication | macOS 는 스트림별 자기 앱 제외(`SCContentFilter`). Windows 의 스트림별 창 제외는 Magnification API(`MagSetWindowFilterList` EXCLUDE)뿐. 기본안: 줌 창 `WDA_EXCLUDEFROMCAPTURE`(macOS 줌 패널 `sharingType=.none` 과 같음) + 줌 중 그 모니터 오버레이 숨김(라이브 줌엔 macOS 도 판서를 안 그린다 — `ZoomController.swift:302` 는 정지 줌에서만 inkView 표시). 차이: 줌 중 화상회의 공유에도 판서가 안 보인다. **WGC 노란 테두리 끄기(`IsBorderRequired`)는 10.0.20348+ 이고 사용자 동의 + 패키지 capability `graphicsCaptureWithoutBorder` 필요** → Win10 은 테두리 불가. DDA 는 테두리 없음 대신 하이브리드 GPU·`ACCESS_LOST`(모드 변경·UAC) 처리, 커서 직접 합성 |
| 공유에서 창 숨기기 | `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` — 팔레트·수업 배지·도움말(설정에 따라) | Win10 2004+, top-level·자기 프로세스 창만, DWM 합성 중에만. 2004 미만은 `WDA_MONITOR`(검은 사각형)로 동작. 런타임 토글 가능 |
| 쉬는 시간 음악 | 열기: `chrome.exe <URL>`(없으면 `ShellExecuteW`). 종료: SMTC 로 Chrome 세션만 `TryPauseAsync`(§7 #7 결정) | **CDP 불가**: Chrome 136+ 는 기본 프로필에서 `--remote-debugging-port` 를 무시한다. 탭은 남는다(사용자 결정). SMTC 에 Chrome 세션이 안 잡히는 경우 [확인 필요: W6 실기] |
| 볼륨 페이드 | `IAudioEndpointVolume::SetMasterVolumeLevelScalar`(CoreAudio 와 1:1). 선택: `ISimpleAudioVolume` 로 **Chrome 세션만** 페이드 | 페이드 중 사용자가 볼륨을 바꾸면 복원값 규칙 필요(macOS 와 동일 규칙으로) |
| 로그인 자동 실행 | 비패키지: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. MSIX: `StartupTask` | MSIX StartupTask 는 사용자가 설정 앱에서 끌 수 있고 앱이 다시 못 켠다 |
| 고 DPI·혼합 DPI | PerMonitorV2, `WM_DPICHANGED`, 오버레이·스왑체인을 물리 픽셀로 | 비율 좌표 저장이라 판서는 안전. 모니터 경계를 넘는 드래그(배지)는 DPI 가 다른 두 좌표계 — 물리 좌표로 계산 |
| 모니터 변경·잠자기 | `WM_DISPLAYCHANGE` 0.3초 디바운스(`SetTimer`), `WM_POWERBROADCAST` 복귀 시 타이머 재계산 | — |
| 키 표시·커서 강조 | `WH_KEYBOARD_LL`·`WH_MOUSE_LL`(권한 프롬프트 없음) | 관리자 창 전경 시 미수신(UIPI) |
| 서명 | Authenticode. 2023-06 이후 OV/EV 키는 하드웨어(HSM/토큰) 보관 필수 | SmartScreen 평판은 다운로드가 쌓여야 생긴다. 클라우드 서명 서비스의 한국 개인·법인 자격 [확인 필요] |
| 배포 | MSIX(+`.appinstaller` 자동 업데이트, `graphicsCaptureWithoutBorder` 선언 가능) 또는 exe 설치기 | MSIX 안에서 `uiAccess` 가능 여부 [확인 필요]. winget 은 어느 쪽이든 매니페스트 PR |
| 자동 업데이트 | MSIX: App Installer 내장. 비-MSIX: WinSparkle(Sparkle appcast 호환 — macOS 와 피드 공유) | WinSparkle 은 C DLL(Rust 밖 바이너리 의존) |
| .pansetting 연결 | 비패키지 `HKCU\Software\Classes`, MSIX `fileTypeAssociation` | — |
| PNG(배경 포함) | 줌과 같은 캡처 경로 1프레임 | — |

## 4. 저장소 배치 — 같은 저장소

`github.com/datasci-lee/eduTools`(공개) 에 추가. 별도 저장소면 픽스처·design.md 가 사본 둘이 된다.

```
windows/
  Cargo.toml                  # [workspace] members = ["crates/*"], Cargo.lock 커밋
  crates/edutools-core/       # OS 의존 0 (lib)
  crates/edutools/            # Windows 앱 (bin), app.manifest·icon
fixtures/pansetting/          # macos-v1.pansetting, windows-v1.pansetting
fixtures/shortcuts-default.json
.github/workflows/windows.yml
```

## 5. Windows PC 개발 환경

### 5.1 설치

| 항목 | 버전·선택 | 비고 |
| --- | --- | --- |
| Windows | 11 x64 (24H2 이상 권장) | 개발 PC = 실기 1호 |
| Visual Studio Build Tools | 2022(17.x) 이상, 워크로드 "C++를 사용한 데스크톱 개발" | MSVC v143 링커·`rc.exe`·`mt.exe` |
| Windows SDK | 10.0.26100 (Windows 11 24H2 SDK) | Build Tools 설치 시 함께 선택. `signtool`·`makeappx` 포함 |
| Rust | rustup → `stable-x86_64-pc-windows-msvc`, rustc 1.95 이상(macOS 쪽 실측 1.95.0) | `rustup target add aarch64-pc-windows-msvc`(ARM64 빌드 시) |
| Git | Git for Windows + `gh` | `git config core.autocrlf false`(Swift 원본 줄바꿈 보존) |
| 에디터 | VS Code + `rust-lang.rust-analyzer` · `ms-vscode.cpptools`(MSVC 디버거) · `tamasfe.even-better-toml` | 대안: RustRover |
| 진단 도구(선택) | Spy++(Build Tools 포함 안 됨 → VS 설치 시), Accessibility Insights, WinDbg | 창 스타일·z순서·메시지 확인 |
| 실기 장비 | 펜(Wacom + 드라이버 "Windows Ink 사용" 체크, 또는 Surface 펜), 배율 다른 모니터 2대(예 100%·150%), 프로젝터 복제 | §6 실기 항목 |
| 시험 앱 | PowerPoint, Chrome(YouTube Music 로그인), Teams 또는 Zoom, 한글 IME | |

### 5.2 클론 후 첫 빌드 (W1 에서 workspace 생성 후)

```powershell
git clone https://github.com/datasci-lee/eduTools.git; cd eduTools\windows
cargo build                                  # 디버그
cargo test -p edutools-core                  # 이식 selftest
cargo run -p edutools                        # 트레이 아이콘 확인, Ctrl+Alt+P
.\target\debug\edutools.exe --selftest      # 0 종료 = 통과
cargo build --release --locked               # 배포용
```

- 매니페스트(PerMonitorV2·Common Controls v6)·아이콘은 `crates/edutools/build.rs` 에서 `rc.exe` 로 `.res` 를 만들어 링크한다(`cargo:rustc-link-arg=<res>`). 외부 크레이트 불필요.
- macOS 빌드(`build.sh`)는 Windows 에서 안 된다 — Swift 는 읽기만.
- macOS 교차 컴파일·폐쇄망 벤더링은 이 계획의 범위 밖.

### 5.3 CI (`windows-latest`, 공개 저장소라 무료)

```yaml
on: { push: { paths: ['windows/**','fixtures/**'] }, pull_request: { paths: ['windows/**','fixtures/**'] } }
jobs:
  build:
    runs-on: windows-latest
    strategy: { matrix: { target: [x86_64-pc-windows-msvc, aarch64-pc-windows-msvc] } }
    defaults: { run: { working-directory: windows } }
    steps:
      - uses: actions/checkout@v4
      - run: rustup target add ${{ matrix.target }}
      - run: cargo test -p edutools-core --locked
      - run: cargo build --release --locked --target ${{ matrix.target }}
      - run: target/x86_64-pc-windows-msvc/release/edutools.exe --selftest
        if: matrix.target == 'x86_64-pc-windows-msvc'
      - uses: actions/upload-artifact@v4
        with: { name: edutools-${{ matrix.target }}, path: windows/target/${{ matrix.target }}/release/edutools.exe }
```

### 5.4 macOS 쪽에서 받아야 할 것

| 항목 | 누가 | 언제 |
| --- | --- | --- |
| `fixtures/pansetting/macos-v1.pansetting`(기본값 그대로 내보낸 파일) + `SelfTest.swift` 기존 왕복 점검이 이 파일과 `windows-v1` 을 읽게 수정 | macOS 쪽 | W2 착수 전 |
| `fixtures/shortcuts-default.json`(명령 → 토큰·범위) | macOS 쪽(카탈로그에서 생성) | W2 착수 전 |
| 원형 팔레트의 각도·구역 판정 순수 함수 | macOS 쪽 진행 중 작업 | W4 착수 전 |

## 6. 단계 로드맵

| 단계 | 대응 macOS | 산출물 | 검증 | 규모(Rust 줄, 대략) |
| --- | --- | --- | --- | --- |
| **W1 골격** | 001·002 일부 | workspace·CI, 트레이 아이콘·메뉴(종료), 모니터별 투명 오버레이(DComp), `Ctrl+Alt+P` 판서 모드 토글(클릭 통과 on/off), WM_POINTER 로 압력 있는 선 긋기, PerMonitorV2, 단일 인스턴스 | CI 초록. 실기: ① 두 모니터에 선 ② 판서 끄면 아래 앱 클릭 ③ 펜 압력 ④ **PowerPoint 슬라이드쇼 위 표시** ⑤ **오버레이에 WDA 걸면 Teams/Zoom 공유에서 빠지는지** ⑥ **WGC 모니터 캡처 노란 테두리를 비패키지로 끌 수 있는지** | 1.5k |
| W2 판서 엔진 | 002 | core `ink`·`settings`·`shortcut` 이식 + .pansetting 픽스처, 펜/형광펜/레이저·Catmull-Rom·Undo 100, LL 훅 모드 키(P H L 1~6 [ ] Ctrl+Z) | `cargo test`(이식 selftest), 실기: 한글 IME 상태에서 P | 2.5k |
| W3 지우개 | 003 | core `eraser`, 부분/획/영역, 펜 지우개 끝, 전체/모니터 지우기 | 이식 selftest, 실기 지우개 끝 | 1k |
| W4 팔레트·도움말 | 004 | 가로·원형 팔레트(macOS 원형 완료 후 그 기하를 core 로), Esc 단계·Tab, 도움말(카탈로그 생성, DirectWrite, 검색 EDIT) | 실기 | 2k |
| W5 보드·타이머 | 005·006 | 화이트·블랙보드 페이지·패턴, 수업 배지(드래그·스냅·WDA·위치 기억), 쉬는 시간 전체 화면, 잠자기 복귀 | core timer selftest, 실기 | 2k |
| W6 쉬는 시간 음악 | 007 | Chrome 새 창 열기·닫기, SMTC/미디어 키 대체, 볼륨 페이드·복원 | 실기(Chrome·Premium 계정) | 0.8k |
| W7 줌 | 008 | WGC(또는 W1 판정에 따라 DDA) 라이브·정지 줌, 따라가기 3방식, Space 미리보기(회색 효과), 배율 표시 | core zoom selftest, 실기 60fps·하이브리드 GPU | 2k |
| W8 부가 | 009~011 | 도형·텍스트(EDIT+IME)·올가미, 스포트라이트·커서 강조·키 표시·가리개, PNG·PDF·클립보드, 자동 보관·모니터 재연결 복원 | 이식 selftest, 실기 | 3k |
| W9 설정·배포 | 012·013 | Win32 설정 창, .pansetting 가져오기/내보내기·파일 연결·백업 5개, 자동 실행, 아이콘, 서명, 설치기/MSIX, 자동 업데이트 | macOS 에서 내보낸 파일을 Windows 에서 가져오기(실물 왕복), 서명 검증 | 2.5k |

합계 ≈17k줄(Swift 15k 대비 +15% — Win32 설정 UI·DComp 보일러플레이트).

macOS 쪽 선행 작업은 §5.4.

## 7. 사용자에게 물을 결정

| # | 결정 | 권장 |
| --- | --- | --- |
| 1 | 최소 Windows: 11 만 / 10 22H2 포함 | ✅ **결정(2026-10-28): Windows 11 부터.** 매니페스트 `supportedOS` 와 설치기에서 Win11 미만 거부 |
| 2 | CPU: x64 만 / x64+ARM64 | 둘 다(CI 행렬 한 줄, Snapdragon·VM 대응) |
| 3 | 실기 장비: 펜 기종·모니터 2대·프로젝터 | ✅ **결정(2026-10-28): 펜 + 노트북 내장 화면·외부 모니터·빔 프로젝터(배율이 서로 다름).** 펜은 삼성 갤럭시 탭(S펜) 검토 중 — Android 기기라 Windows 펜 입력이 되려면 화면 확장 앱(예: Samsung Second Screen, SuperDisplay)이 Windows Ink 펜으로 압력을 넘겨야 한다 [확인 필요: W1 ③ 에서 `WM_POINTER` 펜 타입·압력 수신 여부] |
| 4 | Windows 쪽 개발 주체(사람 / AI 도구)와 PR 검수자 | macOS 쪽 검수 연결 여부 |
| 5 | 서명 인증서: OV/EV·보관 방식(토큰/클라우드 HSM)·명의 | ✅ **결정(2026-10-28): 셀프 서명으로 시작**, 실제 배포 시점에 공인 인증서로 교체. 셀프 서명은 인증서를 설치한 PC 에서만 경고가 사라진다(신뢰 루트·신뢰 게시자에 설치). 교체는 서명 단계만 바꾼다 |
| 6 | 배포 형식: MSIX(+자동 업데이트·테두리 capability) / exe 설치기(+WinSparkle) | ✅ **결정(2026-10-28): 일반 설치 파일(exe).** 자동 실행 = `HKCU\…\Run`, 파일 연결 = `HKCU\Software\Classes`, 자동 업데이트 = WinSparkle(또는 자체 확인). 비패키지라 `graphicsCaptureWithoutBorder` capability 를 선언할 수 없을 수 있다 → W1 ⑥ 에서 WGC 테두리를 못 끄면 줌은 DDA 로 |
| 7 | 음악 종료: Chrome 새 창 닫기 / SMTC 일시정지 / 확장 프로그램 | ✅ **결정(2026-10-28): SMTC 일시정지만.** 열기는 `chrome.exe <URL>`, 종료는 `GlobalSystemMediaTransportControlsSessionManager` 에서 Chrome 세션을 찾아 `TryPauseAsync`(토글이 아닌 명시적 일시정지). 탭·창은 닫지 않는다. Chrome 세션이 없으면 로그 + 알림(토글 미디어 키로 대체 금지 — 이미 멈춘 음악을 다시 튼다) |
| 8 | 볼륨 페이드 범위: 시스템 전체 / Chrome 만 | ✅ **결정(2026-10-28): 시스템 전체**(macOS 와 같음, `IAudioEndpointVolume`) |
| 9 | 줌 중 화상회의 공유에서 판서 숨김 허용 | 허용(아니면 Magnification API 경로 추가) |
| 10 | 계획서를 저장소 `docs/windows-plan.md` 로 올릴지 | 올림(2026-10-28, 이 파일) |
| 11 | iPad 동반 앱(설계서 언급) 범위 | 이번 범위 밖 |

## 8. 가장 큰 위험

1. **두 구현의 동작 표류** — 정본(Swift)이 계속 바뀐다(원형 팔레트 진행 중). Swift 로직 PR 이 Rust 대응을 동반하지 않으면 .pansetting·단축키·지우개 결과가 갈라진다. 픽스처 2종이 유일한 자동 장치다.
2. **줌 캡처 의미론** — Windows 엔 스트림별 자기 창 제외가 없고(WDA 는 전역), WGC 테두리는 Win11+ 패키지 capability, DDA 는 하이브리드 GPU 취약. 최소 버전·배포 형식과 한 묶음으로 W1 에 판별.
3. **입력 라우팅·z순서** — 포커스 없는 오버레이 + LL 훅(타임아웃 시 조용히 제거, 관리자 창 UIPI), 슬라이드쇼·TOPMOST 경쟁, `uiAccess` 는 서명·설치 위치에 묶임.

## 9. 확인 필요

| 항목 | 확신 | 확인 방법 |
| --- | --- | --- |
| 비패키지 Win32 에서 `IsBorderRequired=false` 가 동의 프롬프트 없이 먹는가 | Low | W1 ⑥ 실기 1회 |
| `WDA_EXCLUDEFROMCAPTURE` 가 DComp(`NOREDIRECTIONBITMAP`) 창에도 적용, WGC·DDA·Teams 에서 모두 빠짐 | Med | W1 ⑤ |
| 슬라이드쇼 위 TOPMOST 유지(재확보 필요 여부), `uiAccess` 없이 충분한가 | Med | W1 ④ |
| LL 훅에서 한글 IME 상태 `P` 가 `VK_P`/고정 스캔코드로 오는가 | Med-High | W2 실기 |
| `build.rs` 의 `rc.exe` 경로 탐색(Build Tools 개발자 셸 밖에서 `cargo build`) | Med | W1 첫 빌드 |
| Chrome `--new-window` 창을 제목으로 안정적으로 찾는가(YouTube Music 제목 변화) | Med | W6 실기 |
| MSIX 에서 `uiAccess`·`graphicsCaptureWithoutBorder` 선언 가능 여부, 한국 명의 클라우드 서명 자격 | Low | MS 문서·서명 업체 문의 |
| DComp 저지연 잉크(`IDCompositionInkTrailDevice` 류, Win11) 사용 가치 | Low | W2 지연 측정 후 필요 시 |

출처
- SetWindowDisplayAffinity: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity
- MagSetWindowFilterList: https://learn.microsoft.com/en-us/windows/win32/api/magnification/nf-magnification-magsetwindowfilterlist
- GraphicsCaptureSession.IsBorderRequired: https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturesession.isborderrequired
- Chrome 136 remote debugging 변경: https://developer.chrome.com/blog/remote-debugging-port
- POINTER_PEN_INFO / 펜 플래그: https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-pointer_pen_info · https://learn.microsoft.com/en-us/windows/win32/inputmsg/pen-flags-constants
- Wacom "Windows Ink 사용" 필요성: 포럼·앱 소스 기반(KDE https://phabricator.kde.org/T6807, Blender) — 공식 문서 아님
