# EduTools Windows 개발 인계서 (Rust, macOS Swift 유지)

2026-10-30 · 기준 `main` 6cc02e4(#91 팔레트 UI·렌더 불투명도 하한 0.75 머지 후) · Windows PC 에서 이 문서 하나로 착수한다. 정본: `docs/design.md`(기능·단축키) · `Sources/EduTools/*.swift`(동작) · `docs/icons.md`(아이콘 키).

표식: `[확인 필요]` = §9 에서 실기로 판별. Swift 줄 번호는 `6cc02e4` 기준이고 심볼·상수 이름을 함께 적었다 — 어긋나면 이름으로 찾는다.

## 작업 규칙 (Windows 쪽 사람·AI 도구 공통)

| 규칙 | |
| --- | --- |
| 동작 정본 = macOS Swift | 같은 입력 → 같은 결과. 애매하면 Swift 를 읽고, Swift 와 design.md 가 다르면 사용자에게 묻는다(알려진 불일치: §7 #19) |
| 먼저 읽을 것 | `docs/design.md` · `docs/icons.md` · `assets/icons/README.md` · `assets/symbols/README.md` · `Sources/EduTools/*.swift` · `fixtures/`(macOS 쪽이 만든 뒤 — 아직 없음, §5.4). `ROADMAP.md` 는 001~013 까지만 반영 — 021 이후 변경은 `git log`(제목의 `eduTools-NNN`)로 본다 |
| 편집 범위 | `windows/` · `.github/workflows/windows.yml` · `fixtures/pansetting/windows-v1.pansetting`(§2.2, §7 #17) 만(모두 아직 없음 — W1 에서 생성). `Sources/` · `build.sh` · `docs/` · `assets/` · `README.md` · `ROADMAP.md` 와 나머지 `fixtures/` 는 읽기 전용(macOS 쪽 소유) — 고칠 곳이 보이면 이슈/PR 본문으로 알린다 |
| 에셋 | 앱 아이콘 = `assets/icons/*.svg`(57키), 트레이 ICO = `assets/symbols/windows/tray/` — **읽어서 쓰고 사본을 만들지 않는다**(§3.2) |
| 기본 브랜치 직접 push 금지 | `feature/win-<단계>` → PR → CI 초록 + 실기 체크리스트 후 머지 |
| 시험 | macOS 는 `docs/testing/strategy.md` 의 T0~T6 시험 체계를 쓴다. 구 자체 시험 76확인 중 Windows 이식 대상 74확인은 §2.2의 새 시험 파일·ID를 원본으로 삼는다: A 64 는 같은 입력·기대값 그대로, B 10 은 §2.2 구분 열대로 OS 값만 Windows 값으로 치환, C 2 는 제외. **[결정 필요] Windows 쪽은 새 시험 체계를 만들지 않는다.** 목록 밖 영역의 Windows 시험 추가 여부는 §7 #14 |
| 의존성 | `windows`·`serde`·`serde_json` 외 추가는 사용자 승인 |

## 0. 선택지 판정 (사용자 결정: C — 앱 전체 Rust, 2026-10-28 확정)

| 안 | 한 줄 판정 |
| --- | --- |
| A. C#/.NET 또는 C++ 네이티브 | 설정 UI 는 쉽지만 핵심(오버레이·훅·캡처·DComp)은 결국 P/Invoke·COM 이고, .NET 8 은 2026-11 지원 종료라 쓰려면 .NET 10 |
| B. Rust 공통 코어 + Swift FFI | 공유 대상은 코드가 아니라 **파일 포맷**(.pansetting·단축키 토큰)이다. 판정 당시 순수 로직을 ≈3k줄로 보고, Ink/Board/원형 팔레트는 `NSColor`·클로저, Shortcut 은 `kVK_*` 에 묶여 있어 macOS 재배선 비용이 이득보다 크다고 봤다. 지금 순수 로직은 ≈7.7k줄(추정, §6 규모 근거)로 늘었지만 결론은 같다 — 오히려 아래 C 의 core 분리 이득이 커졌다 |
| **C. Rust 단독 Windows 앱** | **채택.** windows-rs 로 Win32·COM·WinRT 1급 바인딩, 단일 exe, 런타임 없음. 다만 코어 크레이트를 OS 무관으로 분리해 두면 나중에 B 로 갈 수 있다 |
| D. Tauri/Electron/Flutter | 설계서 판단이 Windows 에도 맞다. 클릭 통과 토글·WDA·WM_POINTER 압력·화면 캡처가 전부 네이티브 우회라 결국 Win32 를 직접 짠다 |

## 1. 크레이트·기술 선택

| 영역 | 선택 | 이유 |
| --- | --- | --- |
| OS 바인딩 | `windows`(Microsoft 공식 windows-rs) 하나만, feature 최소 지정. `windows-sys` 는 안 씀 | D2D·DComp·WIC·CoreAudio·WGC 가 COM/WinRT 라 `windows` 가 필수. 두 크레이트 혼용은 타입이 갈라진다 |
| 렌더링 | **DirectComposition + Direct2D**(D3D11 디바이스 공유), 창은 `WS_EX_NOREDIRECTIONBITMAP` | 프리멀티플라이드 알파 투명 오버레이를 GPU 로 그린다. `UpdateLayeredWindow` 는 4K 전체화면에서 CPU 비용이 크다. wgpu 는 2D 획·안티앨리어싱을 직접 짜야 하고 의존 트리가 크다 |
| 획 구조 | 완성 획 = 모니터별 D2D 비트맵, 진행 획 = 별도 DComp 비주얼 | macOS 구조(비트맵 + `CAShapeLayer`)와 1:1. 형광펜 "겹쳐도 안 진해짐" = 불투명으로 그리고 `PushLayer(opacity)` |
| 팔레트·힌트 렌더링 | 오버레이와 **별개의 작은 DComp+D2D 창**(원형 접힘 44·팔레트 지름 302 DIP, 힌트 창 별도). 면은 **RGB(20,23,31) 단색 반투명으로 직접 그리며 뒤 화면 블러 없음**. 1링 도넛·2링 부채꼴 캡슐·3링 트랙/노드 구조와 판정 근거는 §3 "팔레트 면 렌더링". 아이콘은 §3.2 | OS 백드롭(Acrylic·Mica)은 창 전체 사각형이거나 비활성 창에서 단색이 되고, DComp 효과는 비주얼 자신의 내용에만 적용돼 뒤 화면 샘플링이 공식 문서에 없다(커뮤니티: 불가). 단일 exe·`windows` 크레이트 1개 원칙 유지 |
| 텍스트 | DirectWrite | 도움말·타이머·배지·키 표시 |
| 줌 효과 | D2D 내장 효과(Saturation·Brightness) | `CIColorControls` 대응 |
| 이미지 저장 | WIC(PNG·JPEG), 클립보드 `CF_DIBV5` + `PNG` 등록 형식 | OS 내장(내보내기용 — 아이콘 그리기와 무관) |
| PDF | 직접 작성(페이지 = JPEG `DCTDecode` 이미지, ≈150줄) | Win32 에 PDF 작성 API 가 없다. 압축 크레이트 없이 됨. 벡터 품질 요구가 나오면 그때 경로 연산자로 |
| 설정 UI | **Win32 직접**(Common Controls v6: Tab·Trackbar·Edit·ComboBox). 단축키 기록은 **커스텀 버튼** | 의존 0, 한글 IME 기본 동작. `HOTKEY_CLASS` 는 Esc·Tab·Space·Enter·Del·Backspace 를 기록하지 못한다(§2.2 단축키 "기록 UI"). 비용이 커지면 WebView2(HTML) 가 대안 — 단 `webview2-com` 의존 추가 |
| 텍스트 도구·도움말 검색 | 오버레이 위에 띄우는 `EDIT` 컨트롤 | 한글 IME 조합을 직접 구현하지 않는다 |
| JSON | `serde` + `serde_json` | .pansetting 동치(§2.2). 속성 아이콘 `manifest.json` 읽기도 같은 크레이트 |
| 매니페스트·아이콘 | `build.rs` 에서 `rc.exe` → `.res` 링크: ① 매니페스트(PerMonitorV2·Common Controls v6) ② 앱 ICO(ID 1) ③ 트레이 ICO 4종(ID 101~104) | comctl v6 는 매니페스트가 있어야 로드된다. 크레이트 불필요. ICO 는 PNG 압축이라 `rc.exe` 수용을 W1 에서 확인(§9) |
| 아이콘 그리기 | D2D SVG(`ID2D1DeviceContext5`)로 `assets/icons/<키>.svg`·`attr/svg` 를 직접 그림. `include_bytes!` 임베드, 루트 `color` 로 틴트 | PNG 세트(`symbols/windows/png`)는 사용하지 않는다(보관). 의존 추가 없음. §3.2 |
| 폴백 글리프 | DirectWrite + Segoe Fluent Icons(Win11 내장) | 디자인 없는 키 4종만. 폰트 번들 금지 |
| 그 외 | 없음(UUID=`CoCreateGuid`, 시각=`SystemTime`/`GetLocalTime`, 로그=파일+`OutputDebugStringW`, 오류=`windows::core::Result`) | 의존을 `windows`·`serde`·`serde_json` 셋으로 묶는다 |

## 2. 모듈 구조

### 2.1 Swift ↔ Rust 대응표

크레이트 둘: `edutools-core`(OS 의존 0, 어느 OS 에서나 `cargo test`) · `edutools`(Windows 앱 바이너리). Swift 열의 `(N)` = 6cc02e4 줄 수. 순수부/나머지 분할은 "파일 전체 − UI·저장 매체 부분" 으로 가른 것.

| Swift | Rust | 크레이트 | 비고 |
| --- | --- | --- | --- |
| Ink.swift (630) | `ink.rs` | core | 획·점(비율 0~1)·`InkCommand`·Undo 100단계·`InkGeometry`(Shift 스냅·Catmull-Rom). `InkTool.memorySlot`(:37-76, 도구→기억 슬롯 대응 **한 곳**)도 여기. 색 `[f64;4]` |
| InkView.swift 의 `LaserRenderer` | `laser.rs` / `render.rs` | core / app | 점·꼬리 버퍼·줄이기는 시각 인자를 받는 순수 함수(core), 그리기는 app |
| Eraser.swift (418) · Ink.swift:104-182(`EraserMode`·`EraserSettings`) | `eraser.rs` | core | 부분/획/영역·판정 기준·대상 필터 |
| ShapeSelection.swift (518) | `shape.rs` | core | 도형 인식·올가미·선택 변환·화면 가리개 |
| Zoom.swift (226) | `zoom.rs` | core | 배율·따라가기 3방식·가장자리 고정 + 좌표 함수 `original_point`·`pointer_presentation_point`·`pointer_scale`(`Zoom.swift:179-225`) |
| TimerEngine.swift (233) · TimerCoordinator.swift (37) | `timer.rs` | core | 종료 시각 기반 계산, 자동 연결, `TimerGeometry`(배지 스냅 16px) |
| Board.swift (167) | `board.rs` | core | 페이지·패턴·블랙보드 색 치환 |
| SettingsModel.swift (1261) 순수부 | `settings.rs` | core | 스키마(`tools.pressureMinimumWidthRatio`·`tools.perToolMemory`·`tools.favorites`·`tools.paletteStyle`·`tools.paletteGlassOpacity`·`display` 포함)·`SettingsValidator`·`SettingsDocument`/`SettingsTransfer`·옛 키 이행(:345-379)·원형 면 불투명도 상수 `RadialPaletteGlassStyle`(:64-77)·색/호 상수 `RadialPaletteArcStyle`(:79-149)·**`SettingsStore` 의 로드·갱신·손상 보존 규칙**(§2.2 검증기·저장소)·백업 5개 회전(저장 매체는 trait). 영속 매체(`SettingsStore`, `SettingsBackupStore` → `settings.json`)만 app |
| DisplayPreset.swift (1006) | `display_preset.rs` | core | **OS 타입 0**(`import Foundation` 뿐, :1). 모델·`DisplayPresetValidator`·3×3 격자(`GridCell`·`DisplayGrid` 검증·`isConnected`, :1~290)는 `display` 묶음 왕복에 **필수**라 W2. `DisplayMatcher`·`DisplayPlanner.plan`·`DisplayGridLayout`·`DisplayPresetCapture`·`DisplayPlan.summary()` 는 W10(§3.1.6). 입력 `OnlineDisplay` 값의 열거는 app(`monitor.rs`). 범위 §7 #12 |
| Shortcut.swift (1239) | `shortcut.rs` | core | `PortableShortcut` 토큰 문법(:4)·`ShortcutCatalog`(정의 58개 = 리터럴 52 + 색 6, `annotation.pass-through`·기본 키 없는 6개와 `display.preset.1…8` 포함, :431-983)·`ShortcutBinding.make`·`assignableScopes`·`resolvedDefinitions`·`filtered`·`ShortcutConflictDetector`·`matches`. 카탈로그는 상수 배열이 아니라 `Vec` 데이터. kVK/Carbon 표는 `keymap.rs` |
| AppState.swift (1623) · CommandDispatcher.swift (252) | `state.rs` · `dispatch.rs` | core | 모드 전이·명령 라우팅(부작용은 콜백)·도구별 기억·즐겨찾기 스냅샷/적용(`AS:311-399`, §2.3)·`DisplayDisconnectPolicy`. `currentTool` didSet 한 곳에서 비지우개→지우개 전환의 `oldValue`를 비영속 `previousToolBeforeEraser`에 저장하고 `toggleEraserTool`이 복귀(`AS:84-85,186-200,891-897`; `AS`=`AppState.swift`). 명령 enum·라우팅은 `SC:208-240`, `CD:122-149`. 설정과 닿는 규칙(`should_consume`·keyUp 기억·한 단계 나가기 순서·숫자키 색)은 §2.2 단축키. `NSColor`→`[f64;4]`, `NSEvent`(전달 키)는 콜백. 통과 모드 상태 `PassThroughModeSource{key(키코드), timed}`·기한(`now` 주입)·`PassThroughDefaults`(`AppState.swift:6-12` — 시간제 첫 클릭 10초·마지막 입력 3초, 키 모드 안전망 최대 30초. 폴링 간격·신뢰창 2개는 macOS 전용이라 이식 안 함, §3 통과 모드) |
| RadialPaletteController.swift (2146) 순수부 · RadialPaletteGlassView.swift (896) 순수부 | `radial.rs` | core | 경계 표 §2.3: 그룹 정의 표(`RadialPaletteCatalog` `RP:177-257`)·즐겨찾기 동적 항목(`RP:1604-1629`)·`layoutCells`(`RP:1670-1787`)·`hitTarget`(`RP:1588-1602`)·`nextRadialPaletteFocus`(`RP:432-539`)·`RadialPaletteSectorGeometry`(`RG:5-31`)·`RadialPaletteMetrics`(`RG:33-72`)·렌더 상태 값(`RG:90-179`). 지금 Swift 는 `NSColor`·클로저를 가진 private 셀에 묶여 **분리돼 있지 않다** → Rust 에선 색=`[f64;4]`·동작=enum 으로 나눠 재구성 |
| BreakMusic.swift (588) 순수부 | `music.rs` | core | `BreakMusicURL`·`BreakMusicFade`(:11-44). `AppleScriptString`(:45)은 **이식 안 함**(§7 #7) |
| PresenterEffects.swift:5-71 `KeystrokeDisplayFilter` | `keystroke.rs` | core | 수정키 있는 입력만 표시. 입력 표현은 OS 별 |
| AttrIcon.swift (798) | `attr_icon.rs` | core | manifest 해석·도구↔manifest 카탈로그(:161-173)·두께 단계·조건·명암(halo) → 레이어 목록(§3.2.6). 순수 함수 `AttrIconPure`(:26-121), 스냅샷 입력 구성(:248-388), manifest 평가·halo 판정은 아래 §3.2.6 |
| `Tests/UnitTests/*.swift` · `Tests/IntegrationTests/AppStateIntegrationTests.swift` · `Tests/GUITests/PointerEnterExitTests.swift`(이관 대장 `Tests/MIGRATION-LEDGER.md`) | `selftest.rs` | core | §2.2의 새 시험 파일·ID와 정본 벡터를 이식 |
| main.swift (226) | `main.rs` | app | 메시지 루프·단일 인스턴스(명명 뮤텍스)·`EnableMouseInPointer`·시작 시 설정 오류 알림·속성 에셋 감사(:25-36)·화면 프리셋 적용기 배선(:90-102) |
| StatusMenu.swift (553) | `tray.rs` | app | `Shell_NotifyIconW`·`TrackPopupMenu`·트레이 ICO 4종 선택·교체(§3.2.5) + 화면 구성 하위 메뉴(:325-361). 메뉴 항목의 단축키 문구는 카탈로그에서 **메뉴를 열 때마다** 생성(캐시 금지 — 설정 변경이 반영돼야 한다, `StatusMenu.swift:363-387`) |
| HotkeyCenter.swift (134) | `hotkey.rs` · `kbhook.rs` · `mousehook.rs` | app | 전역 7개 + 사용자가 지정한 전역 슬롯을 `RegisterHotKey`, 모드 안 키는 `WH_KEYBOARD_LL`. `RegisterHotKey` 에는 눌림(`WM_HOTKEY`)만 있고 해제 통지가 없다 → 전역 도움말(`⌃⌥/` 누르고 있으면 잠깐 표시·짧게 누르면 고정, `HelpController.shortcutDidRelease`)은 `GetAsyncKeyState` 짧은 폴링으로 해제 감지 [확인 필요: §9]. 단축키 설정이 바뀌면 전역 키 전부 재등록 + 등록 실패 경고 갱신(`main.swift:123-132,142-195`). 통과 모드 시간제 복귀·키 모드 안전망용 마우스 관찰 → `mousehook.rs`(§3 통과 모드 — 전역 클릭 감시) |
| (Shortcut.swift 의 kVK 표 :11-53) | `keymap.rs` | app | 토큰 ↔ VK/스캔코드. Windows 보조 키 별칭 표(§7 #22) |
| ScreenIdentity.swift (28) · OnlineDisplayProvider.swift (110) | `monitor.rs` | app | 식별(§3.1.1, **`monitorDevicePath` 는 UUID 의 1:1 대응이 아니다** — 포트·독에 묶임)·`OnlineDisplay` 열거(`QueryDisplayConfig` 읽기 전용, §3.1.2)·HMONITOR→키 변환·화면 판정(`monitor_by_key`·`monitor_at_point`)·비-0·음수 원점 배치(eduTools-032)를 **이 파일 한 곳**에. 보관용 키 함수도 여기(§3 화면 사라짐·복원) |
| WindowLevels.swift (47) | `z_bands.rs` · `run_above_overlay` | app | 5단 계층 모델(오버레이<효과<도구<안내<대화)과 "대화상자가 오버레이 위에 뜨는" 헬퍼. Windows 엔 레벨 숫자가 없다 → §3 "창 높이 대역·z순서"·"모달 대화상자·소유 창" |
| OverlayController.swift (336) | `overlay.rs` | app | 모니터별 DComp 창·클릭 통과 토글(판서 on/off 와 통과 모드가 같은 창 속성 `set_click_through`). `monitor.rs` 이벤트 구독, 좌표·화면 판정은 §3 "오버레이 창 좌표·화면 판정" |
| InkView.swift (2036) · PointerEvents.swift (27) | `ink_view.rs` · `render.rs` | app | `WM_POINTER*` → core 입력 이벤트(위치 0~1·압력·접촉·지우개 끝·범위)·D2D 그리기·레이저 꼬리(eduTools-028)·우클릭 임시 지우개와 버튼 소유권 + 라이브/정지 줌 입력 매핑·진행 제스처 파생 상태(§3 입력). 장치 종류를 해당 이벤트에서만 읽는 가드는 §3 입력 |
| PaletteController.swift (584) | `palette.rs` | app | **가로 팔레트만**. 원형과 같은 D2D 면 스타일(패널 모서리 12·버튼 9, `PaletteController.swift:113-117,239`). Win32 버튼 컨트롤 안 씀 |
| RadialPaletteController.swift 나머지 | `radial_palette.rs` · `hint_window.rs` | app | 패널·이동·접기·펼침 위치·⌘ 키캡·키 입력 → `radial.rs` 호출. 창·포인터·훅·위치 파일·접근성 설정·힌트 창 |
| RadialPaletteGlassView.swift:287~ (SwiftUI) + `RadialSectorShape`(:206-241) + 원형·가로 공통 그리기 | `palette_render.rs` | app | 렌더 모델(칸 목록+호버·포커스·키캡·속성 구성 플래그, `RG:90-179`) → D2D. 원형의 어두운 단색 반투명 면은 §7 #13과 같이 이식 |
| AppIcon.swift (265) | `app_icon.rs` | app | 키 → 임베드 SVG 문서(캐시)·D2D 그리기·폴백 글리프·상태색·고대비·이름 점검(§3.2.2~4). 속성 아이콘 경로·가로 팔레트 배경 추정은 :71-110,254-264. 팔레트·도움말·원형 그리기가 이것만 호출 |
| build.sh 의 앱 아이콘 생성부 | `build.rs` + `app.rc` | app | 리소스 컴파일·에셋 임베드 표 생성·SVG 허용 요소 검사(§3.2.7) |
| HelpController.swift (733) | `help.rs` | app | 카탈로그에서 생성. 닫기 키도 `matches`, 길게 누르기는 눌린 키를 기억(§2.2 단축키) |
| LessonTimerController (712) · BreakTimerController (607) | `lesson_badge.rs` · `break_screen.rs` | app | |
| BreakMusic.swift 나머지 | `break_music.rs` | app | §3 |
| ZoomController.swift (1130) | `zoom_view.rs` · `capture.rs` | app | 캡처에 커서 미포함, D2D 포인터·시스템 커서 숨김·전환 워프, 라이브 판서·키 라우팅(§3) |
| PresenterEffects.swift 나머지 | `presenter.rs` | app | 스포트라이트·커서 강조·키 표시(`WH_*_LL`) |
| ExportController (357) · ArchiveController (379) | `export.rs` · `archive.rs` | app | 보관 키는 `monitor.rs` 의 키 함수를 그대로 쓴다(§3 화면 사라짐·복원). 프리셋 적용 중 사라지는 화면 강제 보관과 복원 질문 보류는 `ArchiveController.swift:99-160` |
| SettingsController.swift (1218) | `settings_ui.rs` · `settings_store.rs` | app | `%APPDATA%\EduTools\settings.json`. 탭 8개(일반·도구·지우개·줌·보드·타이머·화면·단축키), 도구 탭의 원형 면 불투명도 슬라이더(:557-568)와 필압 최소 굵기 슬라이더(:581-590). `update` 가 Err 면 사유 표시, 로드 실패 시 원본 보존 + 알림(`SettingsStoreIssue` `S:861-870`). 기본 키 없는 정의의 행·해제 버튼(:747-773,844-885)과 커스텀 기록 버튼(§1) 포함 |
| DisplayPresetsSettingsView.swift (834) | `settings_ui_display.rs` | app | "화면" 탭(격자 편집기·현재 구성 캡처·계획 보기). SwiftUI → Win32. 적용 버튼 없음(macOS 1단계와 동일) |
| DisplayApplier.swift (776) 순수부 / 나머지 | `display_operations.rs` / `display_apply.rs` | core / app | 순수 연산 목록 `DisplayConfigurationOperations.build`(:5-38)는 core 로 그대로 이식. app 은 직전 구성 스냅샷·CG 한 겹·`.forAppOnly`→15초 확인→`.permanently`·종료 복구 시도(:49-319). 자동 전환은 아직 Swift 코드 없음(§3.1.3~4). W11 |

### 2.2 동작 동치·스키마 동치를 지키는 방법

| 대상 | 방법 | 시험 수 영향 |
| --- | --- | --- |
| 순수 로직 | `Tests/MIGRATION-LEDGER.md`가 가리키는 새 Swift 시험의 입력·기대값을 `selftest.rs` 에 옮긴다. 단, 속성 아이콘 3군은 `fixtures/vectors/attr-icon-*.json`, 플래너 격자 배치는 `fixtures/vectors/display-grid-layouts.json` 이 정본이며 Windows `selftest.rs`도 이 JSON을 읽는다. 기대값이 같으면 동치. 이산 결과(갈라진 획 수·도형 종류·적중 여부)는 정확히, 좌표는 `1e-9` 허용 | macOS 는 `./build.sh test` 또는 `scripts/test.sh unit integration`; Rust 는 이식분만(`cargo test` 1개 + `edutools.exe --selftest`) |
| 동치 추적 | Rust 함수마다 대응 Swift 심볼 한 줄 주석. 이후 Swift 로직 PR 은 Rust 대응 수정 또는 이슈(`windows-sync` 라벨, §7 #18)를 동반 | — |
| .pansetting | 저장소 루트 `fixtures/pansetting/` 에 **macOS 가 내보낸 파일 1개 + Windows 가 내보낼 파일 1개**. `macos-v1.pansetting`은 현재 main 기본값으로 완료했고, `windows-v1.pansetting`은 Windows 쪽 산출물이다(§5.4). macOS 는 `Tests/UnitTests/SettingsTests.swift:56` `UnitTests.SettingsTests/exportIncludesPresetsAndExcludesComputerSpecificValues()`에서 macOS 파일을 `SettingsTransfer.decode` → `appliedSettings(current: .default, 전체 묶음)`으로 적용해 `.default` 와 같은지(`archiveFolderPath` 없음, `appVersion`·`createdOS` 무시) 본다. Windows `selftest.rs`도 같은 파일을 읽는다. macOS 파일은 묶음 9개·`display:{"presets":[]}`·`pressureMinimumWidthRatio`·`perToolMemory`·`favorites:[]`·`paletteStyle`·`paletteGlassOpacity`·`previewDimming`을 포함한다 | macOS: 기존 설정 시험에 입력 추가. 새 시험 ID 0 |
| 기본 단축키 | `fixtures/shortcuts-default.json` = `{"formatVersion":1,"bindings":{"<정의 id>\|<scope>":"<토큰>",…},"unbound":["<id>",…]}`(키 형식 = override 키 `SC:456-458`; 기본 키 없는 기존 정의 6개 + `display.preset.1…8` = 14개가 `unbound`). 예: `palette.radial.toggle\|annotation`=`M`, `help.show\|annotation`=`Shift+/`, `edit.redo\|annotation`=`Shift+Primary+Z`. 정의 id 는 아이콘 키(§3.2.3)와 같다. `annotation.pass-through\|annotation`=`U` 도 포함(#35·#42·#53 머지됨 — 정의 58개). macOS `Tests/UnitTests/ShortcutTests.swift:72` `UnitTests.ShortcutTests/catalogBindingsAreUniqueWithinScope()`와 Windows 카탈로그 점검이 같은 파일과 대조 | 같음 |
| 슬롯 override | `fixtures/`에 기본 키 없는 정의 1개의 허용 범위 override를 넣어 macOS `Tests/UnitTests/ShortcutTests.swift:72` `UnitTests.ShortcutTests/catalogBindingsAreUniqueWithinScope()`와 Windows 점검에서 같은 바인딩이 생성되는 케이스를 둔다. 전역 슬롯은 `AppMod` 필수, 비전역 슬롯은 `AppMod` 금지이며 잘못된 값은 해당 항목만 무시한다 | macOS: 기존 카탈로그 시험 입력에 추가. 새 시험 ID 0 |
| 기본값 | 위 픽스처가 "기본값 = 양 OS 동일" 을 강제. macOS 기본값을 바꾸면 픽스처가 빨개져 Windows 동기를 요구 | — |

**이식 목록** — `Tests/MIGRATION-LEDGER.md:6-10`의 산식을 따른다. 구 자체 시험의 **76확인**은 72호출이었고, 전부 새 시험 ID 64개(T1 58 + T2 5 + T3 1)로 이관됐다. 아래 A/B/C는 Windows 이식 구분이므로 개수는 A 64 · B 10 · C 2를 유지한다. A = 입력·기대값 그대로, B = 같은 구조·OS 값만 Windows 값으로, C = 이식 안 함.

| 범주 | 개수 | 새 macOS 시험 파일·ID | Rust | 구분 |
| --- | --- | --- | --- | --- |
| 단축키 토큰(`AppMod+P`·`AppMod+/` 파싱·직렬화, 거부 입력 7, 수정키 없는 `P` 거부) | 11 | `Tests/UnitTests/ShortcutTests.swift:10` `UnitTests.ShortcutTests/appModPContract()`<br>`ShortcutTests.swift:32` `UnitTests.ShortcutTests/appModSlashContract()`<br>`ShortcutTests.swift:62` `UnitTests.ShortcutTests/invalidSerializedValueIsRejected(_:)` | `shortcut.rs` | A |
| 단축키 OS 매핑(표시형 `⌃⌥P`·키코드 35/44·Carbon 플래그·왕복·AppMod 없음 거부) | 8 | `Tests/UnitTests/ShortcutTests.swift:10` `UnitTests.ShortcutTests/appModPContract()`<br>`ShortcutTests.swift:22` `UnitTests.ShortcutTests/appModPRoundTripsThroughCarbon()`<br>`ShortcutTests.swift:32` `UnitTests.ShortcutTests/appModSlashContract()`<br>`ShortcutTests.swift:41` `UnitTests.ShortcutTests/appModSlashRoundTripsThroughCarbon()`<br>`ShortcutTests.swift:67` `UnitTests.ShortcutTests/carbonValuesWithoutAppModifierAreRejected()` | `shortcut.rs`+`keymap.rs` | B — `Ctrl+Alt+P`·VK·`MOD_*` 값으로. 물리 키코드 35·44·Carbon 왕복은 이식 불가 → 토큰↔VK 왕복(`P`↔0x50, `/`↔0xBF)로 치환 |
| 카탈로그 중복 없음·판서 단축키 전부 디스패처 연결 | 2 | `Tests/UnitTests/ShortcutTests.swift:72` `UnitTests.ShortcutTests/catalogBindingsAreUniqueWithinScope()`<br>`ShortcutTests.swift:79` `UnitTests.ShortcutTests/annotationCatalogCommandsAreSupported()` | `shortcut.rs`·`dispatch.rs` | A (+`fixtures/shortcuts-default.json` 대조). 중복 검사는 OS 매핑 후 키로. 연결 단언은 Rust `match` 완전성이 컴파일 시 보장하며 `M`(`palette.radial.toggle`)·`U`(`annotation.pass-through`) 도 카탈로그·디스패처에 있다 |
| 속성 아이콘 순수 함수(두께 단계·조건 문법·명암/halo 판정 — #45) | 3 | `Tests/UnitTests/AttrIconTests.swift:120` `UnitTests.AttrIconTests/widthLevelsFollowVector(_:)`<br>`AttrIconTests.swift:134` `UnitTests.AttrIconTests/conditionsFollowVector(_:)`<br>`AttrIconTests.swift:158` `UnitTests.AttrIconTests/contrastFollowsVector(_:)`<br>정본 `fixtures/vectors/attr-icon-width-levels.json:1`·`attr-icon-conditions.json:1`·`attr-icon-contrast.json:1` | `attr_icon.rs` | A — Windows도 정본 JSON 3개를 읽어 구동(§3.2.6) |
| 도움말 필터(한글 이름 "형광펜" → `tool.highlighter`·표시 키 "[" → `tool.width.decrease`) | 2 | `Tests/UnitTests/ShortcutTests.swift:114` `UnitTests.ShortcutPresentationTests/helpFilterMatchesKoreanFeatureName()`<br>`ShortcutTests.swift:121` `UnitTests.ShortcutPresentationTests/helpFilterMatchesDisplayedKey()` | `shortcut.rs` | A — 표시 문자열이 OS 별(`Ctrl+`)이므로 `[` 기대값만 동일 |
| 키 입력 표시 필터 | 1 | `Tests/UnitTests/ShortcutTests.swift:91` `UnitTests.ShortcutPresentationTests/modifiedKeystrokeDisplayFiltersTyping()` | `keystroke.rs` | B (수정키 입력 표현) |
| 줌(배율 단계·가장자리 따라가기·정지 줌 역변환) | 4 | `Tests/UnitTests/ZoomTests.swift:9` `UnitTests.ZoomTests/magnificationStepsAndClamps()`<br>`ZoomTests.swift:16` `UnitTests.ZoomTests/edgeFollowingMovesOnlyBeyondInnerEdge()`<br>`ZoomTests.swift:41` `UnitTests.ZoomTests/centerClampKeepsViewportOnScreen()`<br>`ZoomTests.swift:52` `UnitTests.ZoomTests/frozenInputMapsBackToOriginalCoordinate()` | `zoom.rs` | A |
| 판서(좌표 왕복 1·Catmull-Rom 1·스냅 2·도형 인식 4·올가미/선택 변환/가리개 4·Undo·Redo 2) | 14 | `Tests/UnitTests/GeometryTests.swift:10` `UnitTests.GeometryTests/normalizedAndScreenCoordinatesRoundTrip()`<br>`Tests/UnitTests/InkGeometryTests.swift:11` `UnitTests.InkGeometryTests/catmullRomPassesThroughSourcePoints()`<br>`InkGeometryTests.swift:32` `UnitTests.InkGeometryTests/penShiftSnapsToThreeDirections()`<br>`InkGeometryTests.swift:52` `UnitTests.InkGeometryTests/highlighterShiftStaysHorizontal()`<br>`Tests/UnitTests/RecognitionTests.swift:9` `UnitTests.RecognitionTests/nearlyStraightStrokeIsRecognized()`<br>`RecognitionTests.swift:24` `UnitTests.RecognitionTests/closedFourSidedStrokeIsRectangle()`<br>`RecognitionTests.swift:43` `UnitTests.RecognitionTests/closedRoundStrokeIsEllipse()`<br>`RecognitionTests.swift:53` `UnitTests.RecognitionTests/openZigzagIsRejected()`<br>`Tests/UnitTests/SelectionTests.swift:11` `UnitTests.SelectionTests/concaveLassoRespectsBoundary()`<br>`SelectionTests.swift:25` `UnitTests.SelectionTests/movementOffsetsEveryNormalizedCoordinate()`<br>`SelectionTests.swift:36` `UnitTests.SelectionTests/overlappingCoverClickRemovesOnlyTopmost()`<br>`SelectionTests.swift:62` `UnitTests.SelectionTests/resizingMapsCoordinatesIntoNewBounds()`<br>`Tests/UnitTests/UndoTests.swift:11` `UnitTests.UndoTests/oneHundredOneStrokesPermitOneHundredUndos()`<br>`UndoTests.swift:30` `UnitTests.UndoTests/newStrokeClearsRedoHistory()` | `ink.rs`·`shape.rs` | A |
| 지우개(부분·대상 필터·영역) | 3 | `Tests/UnitTests/EraserTests.swift:11` `UnitTests.EraserTests/partialEraseThroughCenterLeavesTwoFragments()`<br>`EraserTests.swift:33` `UnitTests.EraserTests/highlighterOnlyErasePreservesPen()`<br>`EraserTests.swift:62` `UnitTests.EraserTests/areaEraseDistinguishesTouchingFromContained()` | `eraser.rs` | A |
| 보관(JSON 왕복·해상도 복원·화면 이동 정책)·전체 지우기 | 4 | `Tests/UnitTests/InkSerializationTests.swift:12` `UnitTests.InkSerializationTests/displayInkAndHistorySurviveJSONRoundTrip()`<br>`InkSerializationTests.swift:45` `UnitTests.InkSerializationTests/differentResolutionPreservesNormalizedCoordinates()`<br>`Tests/IntegrationTests/AppStateIntegrationTests.swift:10` `IntegrationTests.AppStateIntegrationTests/moveToPrimaryMergesAfterDestinationInk()`<br>`AppStateIntegrationTests.swift:51` `IntegrationTests.AppStateIntegrationTests/clearAllRestoresEveryDisplayWithOneUndo()` | `archive.rs`·`ink.rs` | A (형식은 Windows 고유 허용 — 불변식만 같게) |
| 보드(전환·페이지 Undo·블랙보드 색) | 3 | `Tests/IntegrationTests/AppStateIntegrationTests.swift:79` `IntegrationTests.AppStateIntegrationTests/repeatedBoardTransitionsPreserveScreenAndBoardInk()`<br>`AppStateIntegrationTests.swift:109` `IntegrationTests.AppStateIntegrationTests/boardPagesKeepIndependentUndoHistories()`<br>`Tests/UnitTests/BoardColorTests.swift:10` `UnitTests.BoardColorTests/blackStrokeDisplaysWhiteOnBlackboard()` | `board.rs` | A |
| 타이머(수업 잔여·일시정지·쉬는 시간 +10분·지정 시각·배지 스냅) | 5 | `Tests/UnitTests/TimerTests.swift:12` `UnitTests.TimerTests/countdownUsesInjectedClock()`<br>`TimerTests.swift:20` `UnitTests.TimerTests/pauseFreezesAndResumeContinues()`<br>`TimerTests.swift:33` `UnitTests.TimerTests/tenMinuteBreakUsesInjectedClock()`<br>`TimerTests.swift:41` `UnitTests.TimerTests/pastDirectBreakTimeIsRejected()`<br>`TimerTests.swift:61` `UnitTests.TimerTests/badgeSnapsInsideSixteenPixelsOnly()` | `timer.rs` | A |
| 쉬는 시간 음악(URL 검증·페이드 곡선) / AppleScript 이스케이프 | 2 / 1 | `Tests/UnitTests/BreakMusicTests.swift:9` `UnitTests.BreakMusicTests/playlistURLRequiresHTTPSMusicHost()`<br>`BreakMusicTests.swift:16` `UnitTests.BreakMusicTests/fadeIsLinearAndTracksEndDate()` / `BreakMusicTests.swift:37` `UnitTests.BreakMusicTests/appleScriptLiteralEscapesURLText()` | `music.rs` / — | A / **C** |
| 화면 구성 프리셋 계산(격자 배치 4종·주 모니터 부재·겹침 적용 불가 / 미러·마스터 승계·누락·프리셋 밖 / 격자 검증 / 자동 집합 우선순위 / 식별 매칭 — #38) | 6 | `Tests/UnitTests/DisplayPlannerTests.swift:123` `UnitTests.DisplayPlannerTests/gridLayoutsFollowVector(_:)`(정본 `fixtures/vectors/display-grid-layouts.json:1`)<br>`DisplayPlannerTests.swift:154` `UnitTests.DisplayPlannerTests/connectedMainAnchorsAtZeroAndOverlapIsRejected()`<br>`:221` `UnitTests.DisplayPlannerTests/mirrorSuccessionMissingAndOutsideRulesArePreserved()`<br>`:306` `UnitTests.DisplayPlannerTests/gridValidationRejectsDuplicateAndDiagonalCells()`<br>`:351` `UnitTests.DisplayPlannerTests/automaticSelectionRequiresExactSetAndPreservesPriority()`<br>`:416` `UnitTests.DisplayPlannerTests/matchingUsesValidHardwareIdentityWithoutReuse()` | `display_preset.rs` | A (격자 배치는 Windows도 정본 JSON을 읽고, 나머지는 Swift 시험 입력·기대값을 이식. 규칙은 §3.1.6) |
| 설정(왕복·줌 범위 검증·호환 디코딩·내보내기 규칙·백업 회전) | 5 | `Tests/UnitTests/SettingsTests.swift:9` `UnitTests.SettingsTests/jsonRoundTripPreservesValues()`<br>`SettingsTests.swift:17` `UnitTests.SettingsTests/validationRejectsOutOfRangeZoom()`<br>`:27` `UnitTests.SettingsTests/decodingIgnoresUnknownKeysAndDefaultsMissingValues()`<br>`:54` `UnitTests.SettingsTests/exportIncludesPresetsAndExcludesComputerSpecificValues()`<br>`Tests/IntegrationTests/AppStateIntegrationTests.swift:150` `IntegrationTests.SettingsBackupIntegrationTests/backupsRotateToMostRecentFive()` | `settings.rs` | A (백업은 매체만 다름). 입력은 아래 "설정 이식 입력" |
| 알려진 충돌 단축키 | 1 | `Tests/UnitTests/SettingsTests.swift:104` `UnitTests.SettingsTests/avoidedGlobalShortcutIsReportedAsConflict()` | `shortcut.rs` | B — 입력은 Windows 표의 값으로 교체(`AppMod+D` 는 macOS 목록) |
| 포인터 장치 종류 가드(진입·이탈에서는 읽지 않음 + 허용 이벤트 종류 표 — #50) | 1 | `Tests/UnitTests/PointerEventsTests.swift:68` `UnitTests.PointerEventsTests/pointingDeviceQueryPermissionMatchesEventKind(_:)`<br>`Tests/GUITests/PointerEnterExitTests.swift:11` `GUITests.PointerEnterExitTests/enterAndExitAvoidPointingDeviceQueries()` | — | **C** — AppKit `NSEvent` 합성 전용. Windows는 §3 입력 규칙으로 이식 |
| 합계 | 76 | | | A 64 · B 10 · C 2 |

설정 이식 입력: 호환 디코딩 = `futureSetting` 삽입 + `display` 키 제거 + `zoom.previewDimming` 제거 + 옛 `tools` 키 삽입·`perToolMemory` 제거 + `tools.paletteGlassOpacity` 제거 → 결과 == `.default`(`Tests/UnitTests/SettingsTests.swift:27` `UnitTests.SettingsTests/decodingIgnoresUnknownKeysAndDefaultsMissingValues()`). 내보내기·가져오기 = display 프리셋이 payload 에 포함·`payload.display` 에 `autoSwitchEnabled` 없음·가져온 뒤 `autoSwitchEnabled == false`·`includedBundles` 에 display 포함·`archiveFolderPath` 없음(`SettingsTests.swift:54` `UnitTests.SettingsTests/exportIncludesPresetsAndExcludesComputerSpecificValues()`). 백업 = 6회 append 후 count 5, 최신 pop = 6, pop 뒤 count 4(`Tests/IntegrationTests/AppStateIntegrationTests.swift:150` `IntegrationTests.SettingsBackupIntegrationTests/backupsRotateToMostRecentFive()`).

**옛 `tools` 키 이행 점검**: macOS `Tests/UnitTests/SettingsTests.swift:27` `UnitTests.SettingsTests/decodingIgnoresUnknownKeysAndDefaultsMissingValues()`는 옛 키에 기본값을 넣고 `paletteGlassOpacity`도 제거해 구 설정의 기본값 0.86 디코딩까지 `== .default` 한 단언으로 본다. 옛 크기 매핑은 값을 틀려도 통과할 수 있으므로 macOS 쪽 시험 입력은 그대로 두고, Windows `selftest.rs` 의 같은 단언 입력만 비기본값(pen 5·highlighter 20·text 30 → pen·arrow·rectangle·ellipse = 5)으로 둔다(W2).

**교차 플랫폼 동치 장치가 없는 남은 영역**: 원형 팔레트 기하·힌트 배치, 도구 → 기억 슬롯 선택(호환 디코딩 시험은 기본값 이행만 포함), 통과 모드 상태·기한 규칙, 아이콘 키 점검(`NSLog` 뿐), `DisplayPresetCapture`·`DisplayPresetValidator`(개수·이름·중복 모니터)·`matchesCurrent` → §7 #14. `nextRadialPaletteFocus` 탐색은 `Tests/UnitTests/RadialPaletteNavigationTests.swift:46` `UnitTests.RadialPaletteNavigationTests/navigationMatchesOriginalTable(_:)`가 생겼고, 즐겨찾기 칸 배치·히트는 macOS T3 `Tests/GUITests/RadialPaletteTests.swift:12` `GUITests.RadialPaletteTests/maximumFavoriteCellsDoNotOverlapAndHitTest()`가 본다. 속성 아이콘 두께 단계·조건 문법·명암/halo와 플래너 격자 배치는 §2.2의 JSON 벡터 4개가 정본이고, `DisplayMatcher`·`DisplayGrid`·자동 집합도 위 표의 T1 시험이 본다. 설정 스키마는 `Tests/Fixtures/settings/v0.1.0/settings.json:1`과 `Tests/IntegrationTests/SettingsGoldenIntegrationTests.swift:16` `IntegrationTests.SettingsGoldenIntegrationTests/everyVersionedFixtureRoundTripsThroughSettingsStore()`가 버전별 왕복을 본다. 디스플레이 쪽 규칙 원본은 §3.1.6.

serde 세부(Swift Codable 과 맞출 것):
- 필드명 그대로(camelCase). 기본값 허용은 **정확히 7곳**: `tools.pressureMinimumWidthRatio`(없으면 `0.2`), `tools.perToolMemory`(없으면 옛 키 이행, 아래), `tools.favorites`(없으면 `[]`), `tools.paletteStyle`(없으면 `radial`), `tools.paletteGlassOpacity`(없으면 `0.86`, `S:351-394`), `zoom.previewDimming`(없으면 `0.62`), 최상위 `display`(없으면 `{autoSwitchEnabled:false, presets:[]}`). 나머지 비-Option 키 누락은 실패이므로 `#[serde(default)]` 는 쓰지 않는다(Swift 는 비-Optional 키 누락 시 실패). 7곳도 `Option<T>` 로 받아 풀 것: Swift `decodeIfPresent` 는 키 부재뿐 아니라 `null` 도 "없음" 으로 받아, `default` 만 걸면 `null` 에서 갈라진다. **값이 있는데 틀리면**(`paletteStyle:"x"`) 기본값으로 덮지 않고 실패(`#[serde(other)]`·`unwrap_or_default` 금지).
- `display` 가 있으면 `autoSwitchEnabled`·`presets` 둘 다 필수. `perToolMemory` 가 있으면 6개 슬롯(`pen`·`highlighter`·`arrow`·`rectangle`·`ellipse`·`text`)과 슬롯마다 `colorIndex`·`size` 필수 — 부분 채움 없음.
- `Option` 은 `skip_serializing_if = "Option::is_none"` — Swift 는 nil 을 키 생략으로 쓴다. `Option` 필드(`archiveFolderPath`·`secondary1/2`·payload 각 묶음)는 serde 가 누락·null 을 None 으로 받으므로 `default` 불필요.
- **옛 키 읽기 이행**(`perToolMemory` 가 없을 때만, `S:345-394`): `penWidth` → pen·arrow·rectangle·ellipse 의 `size`, `highlighterWidth` → highlighter, `textFontSize` → text, 모든 `colorIndex`=0, 옛 키도 없으면 기본(3·18·3·3·3·24). 둘 다 있으면 `perToolMemory` 승. 옛 키는 **쓰지 않는다**: 읽기용 Wire 구조체(`Option<f64>` 3개)를 `ToolSettings` 로 변환하고 직렬화는 옛 키 없는 구조체로. `favorites`가 없는 구 설정은 `[]`. `pressureMinimumWidthRatio`와 `paletteGlassOpacity`는 이행값이 아니라 독립 선택 필드라 없으면 각각 0.2와 0.86. 이행 결과도 검증기를 통과해야 한다(범위 밖이면 전체가 손상).
- 알 수 없는 키 무시(`deny_unknown_fields` 금지, 최상위·내부 모두). **알 수 없는 enum 문자열은 실패**: `paletteStyle`·`appModifier`·`DisplayKind`(`"builtIn"`/`"external"`/`"virtual"`)·`includedBundles` 원소. 문자열 필드(`mode`·`followMode` 등)는 검증기의 값 목록이 판정. 화면의 `cell` 은 필수.
- 정수 필드는 **정수 리터럴로 쓴다**(`3`, `3.0` 아님): `i64`, **단 `vendor`·`model`·`serial` 은 `u32`**(Swift `UInt32`, 음수·초과는 디코딩 실패). 실수는 `f64`. 바이트 비교 금지, 구조 비교만.
- OS 별 의미가 다른 값은 **원값 보존 + 표시만 매핑**: `appModifier: controlOption` → Ctrl+Alt, `notificationSound: "Glass"` → Windows 시스템음 별칭, `musicBrowser` 동일 토큰, `DisplayIdentity.uuid`(macOS CGDisplay UUID — Windows 에서 매칭만 안 될 뿐 보존·왕복). 검증기가 상대 OS 값을 거부하면 안 된다.
- `DisplayPreset.id` = Swift `UUID`: 8-4-4-4-12 16진, 입력 대소문자 무관, 출력 대문자. Rust 는 `String` + 형식 검사(틀리면 디코딩 실패) + 중복 판정은 대문자 정규화 후. `uuid` 크레이트 추가 금지 — `CoCreateGuid` 결과를 대문자로 포맷.
- `formatVersion` 은 새 **키**가 늘어도 1 로 유지한다(새 키는 전부 선택·기본값 경로, `S:1049-1060`). 1 이외 → `unsupportedVersion`, `format` 문자열이 다르면 `unsupportedFormat`. 판정 순서: 구조 디코딩 → `format` → 버전 → `.default` 에 전 묶음 적용·검증(`S:1112-1163`). `SettingsBundle` 값(묶음) 추가는 키 추가가 아니라 값 집합 확장이다 — 구 리더는 `includedBundles` 의 모르는 값에서 파일 전체를 거부한다(§7 #33).
- `createdOS` 는 자유 문자열. `archiveFolderPath` 는 이 컴퓨터 전용(기본 제외) — 그대로. 보관(판서·보드) JSON 은 OS 간 교환 대상 아님 → Windows 고유 형식 허용. 단 파일에 `formatVersion` 을 처음부터 넣는다(macOS 보관 형식엔 없음).

**검증기·저장소**(`settings.rs` core, `settings_store.rs` app)

| 항목 | 규칙 |
| --- | --- |
| 갱신 | 모든 변경은 `update(\|s\| …) -> Result<(), SettingsValidationError>` 하나: 복사본에 적용 → **전체** 검증 → 통과 시에만 커밋·저장·관찰자 통지. 값이 같으면 저장·통지 생략(`S:887-892,939-958`). 설정 UI·`state.rs` 는 `Err` 를 버리지 않는다 — 사유를 사용자에게 보이고(macOS: "설정을 적용할 수 없습니다" 알림, `SettingsController.swift:474`) 컨트롤을 현재 값으로 되돌린다. 로그만 남기고 진행 금지 |
| 로드 | 디코딩·검증 중 하나라도 실패 → 기본값으로 시작 + 원본 바이트를 별도 보존(macOS 키 `EduTools.settings.corrupt.v1`; Windows `settings.corrupt.v1.json`)하고 **다시 읽어 일치를 확인한 뒤에만** 원본 제거, 보존 실패 시 원본 유지. 1회 알림(원인·"기본값으로 시작"·보존 위치). 기본값은 첫 변경 때 저장(`S:901-931`) |
| 저장 | 쓴 뒤 다시 읽어 일치 확인, 실패 → "설정을 저장하지 못했습니다" 알림(`S:988-1004`). Windows: 임시 파일 → `MoveFileExW(REPLACE_EXISTING)` → 재읽기 비교 |
| 백업 | 최근 5개 회전(`S:1225-1260`) |
| tools | `colors` 6개 필수. `pressureMinimumWidthRatio ∈ 0.05...1`, 기본 0.2. `perToolMemory` 슬롯마다 `colorIndex ∈ 0..5`, `size` 는 펜·형광펜·화살표·사각형·원 1~30, 텍스트 8~96(`Ink.swift:32-97`). `favorites`는 최대 8개이며 도구·색 인덱스·크기·화살표/지우개 세부가 유효해야 하고 중복 금지(`S:160-222,381-409,755-763`). 디코딩할 때 알 수 없는 도구, 범위 밖 색·크기, 잘못된 세부, 중복, 8개 초과는 **그 항목만 무시하고 로그**한다. `paletteGlassOpacity` 저장 허용은 0.55...0.90, 기본 0.86이고 UI·렌더는 `max(저장값, 0.75)`을 사용한다(`S:64-77,734-742`; 키 이름 불변). `colors` didSet 클램프(`S:286-290`)는 이식 불요 — `colors.count == 6` 강제로 도달 불가 |
| zoom | 시작·최대 1.25~8, 시작 ≤ 최대, 단계 0.25 또는 0.5, 부드러움 0~1, **`previewDimming` 0~1**(`S:776-811`). 항목별 사유 문구 |
| display | 프리셋 ≤ 8, `id` 중복 금지, 이름 trim 후 비어 있지 않음·≤ 40자(Swift `String.count` 자소 기준 — Rust `chars()` 는 결합 문자·ZWJ 이모지에서 더 엄격, §7 #32), 주 화면 모니터 ≥ 1, 보조2 ⇒ 보조1, 주 화면 칸 (0,0), 칸 `x,y ∈ -1..1`·서로 다름, 4방향 인접으로 주 화면에 이어짐(대각선만 닿으면 거부), 한 프리셋 안 같은 모니터 금지(`uuid` 대소문자 무시 / `serial≠0` 이면 vendor+model+serial / 가상은 vendor+model+name)(`DisplayPreset.swift:77-101,227-290`) |

**.pansetting 묶음**(`S:1007-1200`)
- `SettingsBundle` 9개, 순서 고정: general·tools·eraser·zoom·board·timer·**display**·shortcuts·help. `display` 묶음 payload 는 `{"presets":[…]}` 뿐 — **`autoSwitchEnabled` 는 내보내지도 가져오지도 않는다**(이 컴퓨터의 스위치, design.md 결정 7). 이 컴퓨터 전용은 `general.archiveFolderPath` 하나뿐이고 `display` 는 기본 포함.
- `tools` 묶음은 `ToolSettings` 전체이므로 `pressureMinimumWidthRatio`·`favorites`·`paletteGlassOpacity`도 내보내고 가져온다(`S:1068-1104`). 가져오기는 프리셋 목록 **전체 교체**(병합 아님, 우선순위 = 배열 순서)이고 직후 전체 재검증(`S:1135-1163`). 미리보기는 바뀐 항목만, `display` 는 한 행 "화면 구성 — 프리셋 N개 추가/교체"(`S:1166-1200`).
- "모니터 다시 지정" = 한 행의 `identity` 만 교체하고 칸·순서·`autoMatch` 는 유지(`DisplayPresetsSettingsView.swift:666-672`). 가져온 프리셋의 모니터가 연결 안 된 상태가 정상 경로다.

**단축키**(`shortcut.rs`·`dispatch.rs`·`kbhook.rs`; `SC`=`Shortcut.swift`, `CD`=`CommandDispatcher.swift`)

| 항목 | 규칙 |
| --- | --- |
| 토큰 | `[수정키+…]키`. 수정키 ∈ AppMod·Primary·Shift·Control·Option, 중복 금지, `AppMod` 는 단독. 키 토큰 = A–Z 0–9 `/ [ ] = -` Left Right Up Down **Delete(= Backspace 키 ⌫)** Tab Escape Space Return. 읽기는 수정키 순서 무관·대소문자 구분(`AppMod+p` 거부), 쓰기는 Control·Option·Shift·Primary 순(`SC:59-96`) |
| 생성·범위 | `ShortcutBinding.make` 한 곳이 global 은 `AppMod` 필수, annotation·zoom 은 금지를 강제한다(`SC:263-366`). 기본 바인딩이 없는 정의의 슬롯 범위는 `modes` 중 기본 바인딩이 없는 범위(`assignableScopes`, `SC:398-418`)이며, `overrides["<id>\|<scope>"]`가 있으면 `resolvedDefinitions`가 바인딩을 만든다(`SC:989-1029`) |
| 잘못된 override | 없는 정의·알 수 없거나 허용되지 않은 범위·토큰 파싱 실패·앱 수정키 규칙 위반은 그 값만 무시하고 사유별 한 번만 로그한다(`SC:1031-1129`). 기본 바인딩 슬롯이면 기본값을 유지하고, 기본 키 없는 슬롯이면 미지정 상태를 유지한다. 검증기는 override **키가 비지 않았는지만** 보고 값 파싱은 하지 않는다(`S:841-845`) — 값 하나로 설정 전체를 거부하지 않으며 모르는 값도 왕복 보존 |
| 판정 | (키, 수정키 집합 **정확 일치**) — `Shift+P` ≠ `P`. 키코드만 비교하는 코드 금지. 같은 범위 중복 = (scope, 키, 수정키) |
| keyUp | 비반복 keyDown 에서 `pressed[키] = 정의` 기록, keyUp 은 그 기록을 꺼내 연결(수정키를 무시하고 키코드로만 정의를 다시 찾던 방식은 엉뚱한 정의를 골랐다). `InkView.swift:265,711-723,750-770`·`ZoomController.swift:228,545-566`. keyUp 을 정의에 연결하는 명령은 도움말·줌 미리보기·통과 모드(`CD:93-105`). LL 훅 `KBDLLHOOKSTRUCT` 에는 반복 플래그가 없다(MS 문서) → 이 기록 집합이 반복 판정이다 |
| 소비 | `should_consume(command)`: `toggleRadialPalette`(**M**)는 `tools.paletteStyle == radial` 일 때만 소비. 아니면 소비하지 않고 pressed 도 기록하지 않는다(`CD:111-117`, `InkView.swift:711-723`). macOS 는 `super.keyDown`(응답자 체인에서 사라짐, 아래 앱 전달 아님)이고 Windows 훅의 "소비 안 함" 은 전경 앱에 도달한다 — 바인딩 없는 키와 같은 취급(§3 "판서 모드 한 글자 키" 행)으로 두되 이 차이를 인지할 것. 전역 `AppMod+M`(수업 타이머)과는 범위가 달라 충돌 아님 |
| 한 단계 나가기 | 통과 모드 끄기(`annotation.pass-through`, 키·시간제 모두) → 원형 팔레트 접기 → 도움말 닫기 → 선택 해제 → 펜으로 → 판서 종료. 앞 단계가 처리하면 나머지는 안 한다(`CD:213-230`). design.md 표도 같은 순서다 |
| 도움말 | 닫기 키도 `matches`(`HelpController.swift:684`). 길게 누르기는 눌린 키를 기억(`activeShortcutKeyCode`, `:162,207,694`)해 같은 물리 키 keyUp 에만 반응 |
| 숫자키 1~6 | `AppState.selectColor`(`AppState.swift:950-960`): 펜·형광펜·화살표·사각형·원·텍스트 → 그 슬롯 `colorIndex` 를 **설정에 쓴다**(저장·통지); 레이저 → `tools.laserColor = colors[i]`(프리셋 **값**, 인덱스 아님); 지우개·선택·가리개 → 상태 불변(키는 소비). 범위 밖 무시. 현재 색 = 현재 도구 슬롯, 슬롯 없는 도구는 마지막 슬롯 도구의 색(`lastColorTool`, `AS:84,186-205`). 도구 전환이 값을 복사하지 않는다 |
| 기록 UI | Esc·Tab·Space·Return·Delete 도 **기록 가능**(기본값이 Esc·Tab·Space 를 쓴다), 취소 = 기록 버튼 재클릭(`SettingsController.swift:949-1002`). `HOTKEY_CLASS` 는 이 키들을 `DefWindowProc` 로 넘겨 기록 못 함 → 커스텀 버튼(`WM_KEYDOWN`·`WM_SYSKEYDOWN` 직접). 전역 범위는 수정키 집합이 앱 수정키와 정확히 같을 때만 기록, 아니면 거부음. 기록 중에는 자기 `RegisterHotKey`·모드 키 훅이 가로채지 않게 풀어 둔다 [확인 필요: §9] |
| `Control`·`Primary` | design 원칙 6 은 ⌘↔Ctrl, ⌥↔Alt 이고 AppMod(⌃⌥)↔Ctrl+Alt 라서 **`Control` 과 `Primary` 가 Windows 에서 둘 다 Ctrl** — §7 #21. 잠정: 원값 보존, 중복 판정은 OS 매핑 후, Windows 기록기는 `Primary` 만 기록 |
| Windows 보조 키 | design.md 의 `Ctrl+Y`(다시 실행)·`F1`(도움말)·`Delete`(선택 삭제)는 카탈로그에 없다 — §7 #22. 잠정: `keymap.rs` 별칭 표에만 두고 카탈로그·픽스처·설정에는 넣지 않는다(기본값 양 OS 동일). 대가: 도움말·설정 단축키 표에 이 키들이 안 보인다 |
| 충돌 검사 | `avoidedAppModifierKeys`·`avoidedPortableShortcuts`(`SC:1164-1172`)는 **macOS 목록**(⌥⌘8·⇧⌘3~5·⌃Space…). Windows 는 design.md 「피한 단축키」 Windows 행으로 별도 표, 중복 검사·경고 구조만 공유 |
| 기본 키 없는 슬롯 | macOS는 기존 6개와 화면 프리셋 8개를 설정에 "없음"으로 표시하고 `modes`에서 허용 범위를 만든다. Windows도 W2에서 같은 슬롯·지정·해제를 구현하며 카탈로그를 런타임 `Vec`, 명령을 값 실린 변종(`applyDisplayPreset(i)`)으로 둔다. override 키 `<id>\|<scope>`와 항목 단위 무시·로그 규칙 유지 |

### 2.3 원형 팔레트 core/app 경계

**칸 목록 `layoutCells()` 하나에서 그리기·히트·키보드 이동이 모두 도출된다 — 별도 칸 목록·좌표표를 만들지 않는다**(`RP:1065-1101,1588-1602,1670-1787,1999-2008`, `RP`=`RadialPaletteController.swift`, `RG`=`RadialPaletteGlassView.swift`). 각도 = 12시 기준 시계방향 라디안.

원형 면 상수는 `RadialPaletteGlassStyle`과 `RadialPaletteArcStyle`이다(`S:64-149`, `S`=`SettingsModel.swift`). macOS 일반 면·중심·키캡·힌트는 RGB(20,23,31)를 `tools.paletteGlassOpacity`로 채우며 면 색조를 자동 전환하지 않는다. 기본값은 0.86, 저장 검증은 0.55...0.90, UI·렌더는 0.75...0.90이고 옛 저장값 0.55...0.74는 렌더할 때 0.75로 올린다. 바깥 테두리=흰색 α0.08, 호버=흰색 α0.07, 비활성 트랙=`#64748b` α0.6+검정 α0.28 미세 그림자, 활성 호=`#38bdf8`+글로우, 아이콘=비활성 `#9ca3af`/활성 흰색, 비활성 내용 α0.55다(`RG:287-599`; 힌트 `RP:541-633`). 속성 아이콘 배경 추정은 선택 시 강조색, 그 밖은 어두운 면 고정이다(`S:133-140`; `RG:737-758,850-885`). 흰 배경 위 합성면은 `Cα=α×(20,23,31)/255+(1−α)×1`, 채널 선형화는 `((Cα+0.055)/1.055)^2.4`, 휘도는 `L=0.2126R+0.7152G+0.0722B`다. α0.75/0.86/0.90에서 각각 `C=(0.30882,0.31765,0.34118)/(0.20745,0.21757,0.24455)/(0.17059,0.18118,0.20941)`, `L=0.08225/0.03884/0.02755`, 흰 내용 대비 `(1.05)/(L+0.05)=7.94:1/11.82:1/13.54:1`이다. 비활성 아이콘 `#9ca3af`의 선형 RGB=(0.33245,0.36625,0.42869), `L=0.36357`이며, 흰 배경에서 가장 밝은 하한 α0.75 합성면과의 대비는 `(0.36357+0.05)/(0.08225+0.05)=3.13:1(≥3:1)`, 기본 α0.86 합성면과의 대비는 `(0.36357+0.05)/(0.03884+0.05)=4.66:1`이다.

| 요소 | 크레이트 | Swift 원본 |
| --- | --- | --- |
| 치수·시간 상수(지름 44/302, 링 반지름 28–58·64–100·113–149, 2링 캡슐 두께 32, 끌기 4, 키캡 0.5초, 칸 간격 2°·속성 간격 9°) | core | `RG:33-56`; 캡슐 `RG:475-503` |
| 부채꼴 `contains(r, θ)`·`inset` (θ 정규화로 시작각이 음수여도 동작) | core | `RG:5-31` |
| 링 렌더 구조: 1링=구분선 없는 어두운 도넛+아이콘(선택 그룹 호는 바깥 가장자리), 2링=원반 없는 부채꼴 캡슐 안 도구 아이콘(선택 흰색 α0.18), 3링=안쪽 연속 비활성 트랙+선택 구간 활성 호+바깥 노드(색 스와치는 원반 없음, 두께·크기는 원반+값 글자) | app | `RG:287-535` |
| 그룹 6개(고정 위치 12·2·4·6·8·10시)와 2링 카탈로그: 작성=펜·형광펜·텍스트(3), 도형=화살표·사각형·원(3), 지우기=부분·획·영역 지우개·실행 취소·다시 실행·전체 지우기(6), 보드·설정=화이트보드·블랙보드·이전/다음 페이지·저장·복사·설정(7), 발표=레이저·스포트라이트·화면 가리개·통과 모드·줌·커서 강조(6), 즐겨찾기=동적. 제거된 도움말·원형/가로 팔레트·화면 구성 칸은 상태 메뉴·설정·단축키 경로만 유지 | core | `RP:14-21,177-257` |
| 즐겨찾기 스냅샷↔적용 규칙(도구 → 색 → 크기/지우개 크기 → 세부 순서, 최대 8·중복 판정) | core | `SettingsModel.swift:160-222`, `AS:311-399` |
| 즐겨찾기 칸 동적 구성([＋ 지금 설정 저장]+저장 칸), 속성 아이콘 조립·툴팁, 우클릭/⌥클릭 삭제 | app | `RP:747-755,1604-1663`, `AttrIcon.swift:161-173,248-388` |
| 칸 목록: 1링 슬라이스 2π/6, 2링 `span=clamp(개수×24°, 90°, 200°)`를 그룹 방향 중심 정렬. 3링은 칸당 최대 30°이고 섹션 1개면 선택 그룹 각도+180°에 호 중심, 2개 이상이면 12시부터 순서대로 배치; 섹션 간격 9°·칸 간격 2° | core | `RP:1670-1787`; `RG:47-49` |
| 3링 값: 색+굵기 [1,3,5,8,12], 형광펜 색+굵기 [6,12,18,24,30], 글자 색+크기 [16,24,36,48,72], 부분·획 지우개=설정 크기 프리셋, 영역 지우개=3링 없음(영역 칸 재클릭으로 사각형↔올가미), 레이저=색만. 투명도·꼬리·보정·화살촉은 3링에서 제거하고 설정 창에만 둔다 | core | `RP:1306-1324,1817-1973`; `RG:54-56` |
| 히트 판정: r≤22 → 중심 / 링 안 → 칸 / 그 외(링 사이 틈·창 모서리) → 없음 = **바깥 클릭(접기, 그리기 안 함)** | core | `RP:1588-1602` |
| 키보드 이동 `nextRadialPaletteFocus`(칸 목록·현재·키 → 다음 포커스) | core | `RP:432-539` |
| 그룹·범주 글자는 그리지 않고 3링의 굵기·크기 값 숫자만 미리보기 아래에 표시 | app | `RG:601-681` |
| 힌트 위치(선호 방향→반대→작업 영역 클램프)·창 중심 클램프·비율 위치 저장/복원·중심 선 굵기 `clamp(v/3,2,6)` | core | `RP:595-663,1487-1581,2100-2109` |
| 창 생성·포인터·캡처·타이머·키 훅·D2D·DirectWrite·모니터 목록·위치 파일·접근성 설정 | app | — |

macOS 쪽 리팩터링은 요청하지 않는다 — `RadialPaletteSectorGeometry`·`Metrics`(`RG:5-72`)와 순수 함수 `nextRadialPaletteFocus`만 컨트롤러에서 분리돼 있고 `layoutCells`·`hitTarget`은 NSColor·클로저·AppState 결합 메서드이므로 Windows 가 읽고 재구성한다. 탐색 입력·기대값은 `Tests/UnitTests/RadialPaletteNavigationTests.swift:46` `UnitTests.RadialPaletteNavigationTests/navigationMatchesOriginalTable(_:)`에서 이식한다. 즐겨찾기 배치·히트의 macOS T3는 `Tests/GUITests/RadialPaletteTests.swift:12`에 있지만, 기하·힌트 배치의 교차 플랫폼 벡터는 없어 그 부분은 검증 공백(§7 #14)이다.

## 3. 기능별 구현 수단·함정

| 기능 | 수단 | 함정 |
| --- | --- | --- |
| 투명 클릭 통과 오버레이 | 모니터별 `WS_EX_TOPMOST·TOOLWINDOW·NOACTIVATE·NOREDIRECTIONBITMAP` + DComp. 일반 모드 `WS_EX_TRANSPARENT`(+`WS_EX_LAYERED`) 켜기, 판서 모드 끄기(`set_click_through` 하나 — 통과 모드가 공유). `WM_POINTERACTIVATE` → `PA_NOACTIVATE` | 슬라이드쇼·다른 TOPMOST 창과 z순서 경쟁 → **대역 순서 전체**를 다시 세운다(아래 "창 높이 대역·z순서" 행). 오버레이 하나만 `HWND_TOPMOST` 로 다시 올리면 팔레트·대화상자 위로 올라온다. 시작 메뉴·알림 위는 `uiAccess=true`(서명 + Program Files 설치) 필요. 독점 전체화면 게임 위는 불가 |
| 오버레이 창 좌표·화면 판정(eduTools-032 의 Windows 판) | 모니터별 오버레이는 `EnumDisplayMonitors`+`GetMonitorInfo(MONITORINFOEX).rcMonitor` 를 `CreateWindowEx` 에 그대로 주고, 생성 직후 `SetWindowPos(rcMonitor)` 로 한 번 더 고정. `WM_DPICHANGED` 에서도 `rcMonitor` 를 적용(문서는 제안 사각형 사용을 강하게 권하며 어기면 DPI 변경이 재귀할 수 있다 [D14] → 재귀 여부 실기 확인). 화면 판정은 `monitor.rs` 한 곳(`monitor_by_key`·`monitor_at_point`=`MonitorFromPoint`)에서만, 창 프레임·`MonitorFromWindow` 로 하지 않는다. HMONITOR → 키 변환: `MONITORINFOEX.szDevice` ↔ `DISPLAYCONFIG_SOURCE_DEVICE_NAME.viewGdiDeviceName` → 그 소스를 쓰는 경로의 마스터 타깃 → §3.1.1 식별(복제 그룹은 HMONITOR 가 하나) | macOS 결함(032): `NSPanel(contentRect: screen.frame, …, screen: screen)` 의 `screen:` 때문에 원점이 (0,0)이 아닌 화면(Sidecar)에서 창이 화면 밖에 놓였다 — 수정 = `screen:` 제거 + 화면 판정을 창 프레임이 아니라 `NSScreen` 목록 기준으로 통일(`ScreenIdentity.swift:15-23`, 커밋 4967f37). Windows 에는 "화면 상대 contentRect" 가 없어 같은 결함은 구조적으로 드물지만 **대신 생기는 실수**: ① `rcWork`(작업 영역)로 만들기 ② 다른 DPI 모니터에서 만든 창이 `WM_DPICHANGED` 로 크기가 틀어짐 ③ `szDevice`(`\\.\DISPLAY1`)를 키로 저장(바뀐다). `EnumDisplayMonitors` 는 미러링 드라이버의 보이지 않는 의사 모니터도 열거한다 [D15] → 오버레이는 CCD 활성 소스와 대조해 거른다 [확인 필요: 150%·100% 두 모니터에서 생성 직후 `GetWindowRect == rcMonitor`] |
| 창 높이 대역·z순서 | macOS `AppWindowLevel`(`WindowLevels.swift:3-13`)의 5대역 오버레이 < 효과(줌·스포트라이트) < 도구(팔레트·수업 배지) < 안내(도움말·쉬는 시간 화면) < 대화(설정·대화상자)를 **전부 `WS_EX_TOPMOST` 한 그룹 안의 삽입 순서**로 구현. `z_bands.rs`: `enum Band` 하나가 정본, 창은 생성 때 대역에 등록, `raise_band_order()` 가 낮은 대역부터 `SetWindowPos(hwnd, HWND_TOPMOST, 0,0,0,0, SWP_NOMOVE\|SWP_NOSIZE\|SWP_NOACTIVATE)` 를 차례로 호출(마지막이 최상위). 호출부에서 순서를 직접 만지지 않는다(030 지시 §6 과 같은 규칙). 순서를 다시 세우는 시점: 모니터 동기화 직후 · 주기 재확보 · 대화상자 닫힘 | `HWND_TOPMOST` 는 "모든 비-TOPMOST 위, 그룹 맨 위"로 가므로 단발 호출은 다른 대역을 덮는다. 활성화된 TOPMOST 창은 그룹 맨 위로 올라오는데 [D16] 대화 대역에는 의도한 동작. 매번 전부 다시 올리면 깜박일 수 있다 [확인 필요: 4K 두 모니터에서 육안·`DWM` 프레임 타임] |
| 모달 대화상자·소유 창(030 의 Windows 판) | `MessageBox`·`TaskDialogIndirect`·`IFileDialog::Show` 의 owner 를 **대화 대역의 숨은 TOPMOST 앵커 창** 하나로 고정하는 헬퍼 `run_above_overlay`(macOS `NSAlert/NSSavePanel.runAboveOverlay`, `WindowLevels.swift:31-46` 대응). 설정 창·가져오기 미리보기·권한 안내·15초 확인 창도 같은 owner. `MessageBox` 는 `MB_SETFOREGROUND` 를 함께 준다 | macOS 결함(030): `runModal()` 이 창 높이를 모달 패널(8)로 덮어써 오버레이(1000) 아래에 깔리고 앱 모달이라 클릭도 안 됐다 → 시작 뒤 큐에서 높이 재설정(`:21-29`). Windows 는 높이를 덮어쓰지 않지만 **owner 가 NULL 이면 비-TOPMOST** 라 TOPMOST 오버레이 밑에 깔려 같은 증상이 난다. 해법의 근거: "TOPMOST 창이 소유한 창(대화상자)은 자동으로 TOPMOST 이고 소유자 위에 유지된다" [D16], owner 허용값에 NULL 포함 [D17], 소유자가 최소화되면 소유 창이 숨는다 [D10]. owner 를 오버레이로 쓰면 모달이 오버레이를 비활성화해 판서가 막힌다 → 앵커 사용 [확인 필요: 앵커 owner 에서 팔레트 클릭 허용 여부 = macOS 앱 모달과의 차이]. 핫키·트레이 처리 중에는 포그라운드 권한이 있고 타이머 콜백에서 띄우면 없다 [확인 필요: `SetForegroundWindow` 거부 시 작업 표시줄 깜박임만] |
| 전체 화면 앱 위·가상 데스크톱 | 일반 전체 화면(최대화·F11·슬라이드쇼) 위는 TOPMOST 로 충분(첫 행). 독점 전체 화면·보호 콘텐츠는 불가(첫 행). 가상 데스크톱: `IVirtualDesktopManager` 는 `GetWindowDesktopId`·`IsWindowOnCurrentVirtualDesktop`·`MoveWindowToDesktop` 세 메서드뿐 [D18] → macOS `.moveToActiveSpace`(`WindowLevels.swift:18`) ≈ 설정·대화상자를 열 때 `MoveWindowToDesktop(hwnd, GetWindowDesktopId(전경 창))` | `.canJoinAllSpaces`(오버레이, `OverlayController.swift:140`) 대응 공개 API 가 없다 [확인 필요: TOPMOST·TOOLWINDOW 창이 새 가상 데스크톱(Win+Ctrl+D)에서도 보이는지 — 안 보이면 데스크톱 전환 감지 후 창 이동]. 문서는 앱이 사용자를 다른 데스크톱으로 자동 전환시키지 말라고 한다 [D18]. `MoveWindowToDesktop` 이 호출 프로세스 소유 창만 옮기는지 [확인 필요] |
| 전역 단축키 | `RegisterHotKey`(`MOD_CONTROL\|MOD_ALT\|MOD_NOREPEAT`) | 실패 = 선점(설계서대로 즉시 알림). VK 기반이라 한글 IME 무관. AltGr 배열 감지 시 +Shift |
| 판서 모드 한 글자 키 | **`WH_KEYBOARD_LL`**(판서·줌 모드 중에만 설치), 바인딩 키만 삼키고 나머지 통과 | 오버레이가 포커스를 가지면 한글 IME 에서 `VK_PROCESSKEY` 로 와서 원칙 5 가 깨지고 ←/→ 전달을 `SendInput` 으로 되돌려야 한다 → 훅이 맞다. 훅 콜백이 `LowLevelHooksTimeout` 을 넘기면 **조용히 제거**됨(전용 스레드·즉시 반환). 관리자 권한 창이 전경이면 UIPI 로 못 받음. 텍스트 입력 중엔 통과 |
| 원형 팔레트 입력 | 창 = `WS_EX_TOPMOST·TOOLWINDOW·NOACTIVATE·NOREDIRECTIONBITMAP`, `WM_POINTERACTIVATE`→`PA_NOACTIVATE`, **`WM_NCHITTEST` 는 창 전체를 `HTCLIENT`**(투명 칸·모서리 클릭도 팔레트가 받아 접기 — `HTTRANSPARENT` 금지). 중심 끌기: 누른 위치와 **화면 좌표 델타 ≥ 4 DIP**(`GetCursorPos`, 창이 움직여도 안 변함)면 이동·놓을 때 위치 저장, 미만이면 클릭(펼침/접힘). **중앙 우클릭은 `stepBack`과 동일**해 통과 끄기→접기→도움말 닫기→선택 해제→펜→판서 종료 순서를 따른다(`RP:747-755`; `CD:213-230`). **바깥 클릭 접기**: 메시지 루프 `DispatchMessage` 직전에서 "팔레트 펼침∧`WM_POINTERDOWN` 대상이 팔레트 창이 아님(앱의 다른 창 전부 — 오버레이·배지·도움말)"이면 접고 메시지 폐기 | `EnableMouseInPointer` 는 프로세스 전체·1회(문서) → 팔레트·힌트 창의 마우스도 `WM_POINTER*`(`WM_MOUSE*` 아님). 호버 = `WM_POINTERUPDATE`+INRANGE−INCONTACT, 창 이탈 = `WM_POINTERLEAVE`. 드래그 중 캡처를 잃으면(`WM_POINTERCAPTURECHANGED`) 짝 `UP` 이 안 온다(문서) → 끌기·눌림 상태 초기화 |
| 좌표계·DPI(원형 팔레트) | 상수는 DIP. 각도 판정 `atan2(dx, −dy_screen)`, 힌트 방향 `(sin θ, −cos θ)`. 저장 위치 비율은 **위에서 아래** 기준(macOS 기본 y=0.85 는 y 위쪽 기준 → Windows 기본 0.15). 작업 영역 = `rcWork`(`visibleFrame` 대응), 모니터 키 = `monitor.rs` | AppKit 은 y 가 위쪽(+): 위치 저장·복원 `RP:1487-1535`를 그대로 옮기면 상하가 뒤집힌다(SwiftUI 그리기 `RG:235-239`만 y 아래쪽). 모니터 간 이동 시 `WM_DPICHANGED` 로 창 물리 크기 재계산(중심 기준 유지) |
| 원형 팔레트 키보드(↑↓←→·Enter·Space·`M`) | 가로채기는 기존 `WH_KEYBOARD_LL` 훅에서: **펼침+보임+텍스트 입력 아님+수정키(Ctrl·Alt·Shift·Win) 없음**일 때만 삼킨다. 훅이 읽는 값은 원자 플래그(`palette_expanded`·`text_input`), 이동 계산은 `PostMessage` 로 UI 스레드에서. 삼킨 keyDown 의 VK 를 집합에 넣어 **짝 keyUp 도 삼킨다**(중간에 접혀도). `↑`=바깥 링(중심→1링→현재 도구(없으면 첫 활성)→첫 값), `↓`=안쪽, `←/→`=같은 링(1·3링 순환, 2링 끝에서 멈춤, 1링 이동만 그룹 선택). **2링 도구 칸에서 `↑` 는 그 도구가 아직 선택 전이고 3링이 있으면 먼저 선택(클릭과 같은 dispatcher 경로)한 뒤 3링으로 들어간다**(`RP:1065-1101`, 순수 함수 밖 부작용). Enter·Space=클릭과 같은 경로(반복 입력 무시). `M`=`toggleExpansionFromKeyboard`(숨김이면 먼저 보이기, 펼치면 포커스 표시 즉시 켬) | 접힘·숨김 상태의 ←/→ 는 훅이 삼키지 않는다(보드 페이지·아래 앱으로 — 기존 규칙). 텍스트 입력 중 = 오버레이의 `EDIT` 컨트롤 보유 중. 포커스 표시는 키보드 사용 중에만: 마우스가 팔레트 위에서 움직이면 숨김(`RP:1380-1384`). 훅의 수정키 상태를 직접 추적할지 `GetAsyncKeyState` 로 볼지 [확인 필요: W2 실기 — 훅 시점의 `GetKeyState` 는 갱신 전일 수 있음] |
| 단축키 키캡(⌘ 길게) | macOS `⌘` 단독 0.5초 ↔ Windows **`Ctrl` 단독 0.5초**(⌘↔Ctrl 1:1 원칙, 설계서 단축키 원칙 6; §7 #23). 훅이 Ctrl-down 에서 `SetTimer`, 다른 키 down·수정키 추가·Ctrl-up 이면 취소(`RP:1168-1205`), 만료 시 현재 수정키가 정확히 Ctrl 뿐일 때만 표시. 글자는 `shortcut.rs` 표시 함수(Windows 키 표기) | 설계서는 `⌘` 만 적고 Windows 키를 안 적었다. `Ctrl+Alt`(앱 수정키)를 누르면 Alt 가 끼어 취소돼 전역 단축키와 안 겹친다 |
| 힌트 창(툴팁 대체) | 별도 top-level 창: `WS_EX_TOPMOST·TOOLWINDOW·NOACTIVATE·TRANSPARENT(+LAYERED)`(클릭 통과), `WDA_EXCLUDEFROMCAPTURE`, 팔레트 바로 위 단(macOS `tool+1` = `guidance` 와 같은 높이, `RP:541-563`). 글자 `이름  단축키`(공백 2), 12 DIP 중간 굵기·안쪽 여백 9/6, 위치 `RP:595-663`. 그룹 칸은 힌트를 띄우지 않는다(`RP:1425-1432`). 마우스 호버뿐 아니라 **키보드 포커스 이동에도 표시**(`RP:1405-1423`), 펼칠 때 접힌 상태 힌트는 숨김(`RP:1250-1258`) | 시스템 툴팁(`tooltips_class32`)은 마우스 호버 전용이라 키보드 포커스 힌트를 못 띄우고, 캡처 제외·면 스타일 통일도 안 된다 |
| 팔레트 면 렌더링 | **권장: D2D 어두운 단색 반투명 면, 블러 없음.** RGB(20,23,31)×`tools.paletteGlassOpacity`(키 이름 불변, 기본 0.86, 저장 0.55...0.90, UI·렌더 0.75...0.90), 바깥 테두리=흰색 α0.08, 호버=흰색 α0.07, 비활성 트랙=`#64748b` α0.6+미세 그림자, 활성 호=`#38bdf8`+글로우, 아이콘=비활성 `#9ca3af`/활성 흰색으로 그린다(`S:64-149`). 구조는 1링 도넛·2링 부채꼴 캡슐·3링 트랙/노드이며 §2.3 표를 그대로 쓴다(`RG:287-599`). 속성 아이콘 배경 추정은 선택 시 강조색, 그 밖은 어두운 면 고정(`S:133-140`). 흰 배경 위 α0.75 합성면과 흰 내용의 최저 대비는 §2.3 계산대로 7.94:1이다. 설정값 하나를 `settings.rs`→`state.rs`→모든 팔레트 면에 전달하고 사본 상수를 두지 않는다 | **대안 A(후속, 모양 있는 블러)**: 팔레트 창이 `WDA_EXCLUDEFROMCAPTURE` 라 W7 의 WGC/DDA 프레임에는 창 뒤 화면이 찍힌다 → D2D `GaussianBlur` 를 면 기하 마스크(`PushLayer`)로 깔 수 있다. OS 정책 독립, 비용은 캡처 상시 구동·1~2프레임 지연·WGC 테두리 이슈(W1 ⑥). **대안 B(보류)**: `Windows.UI.Composition` 호스트 백드롭 브러시+`CompositionGeometricClip`(1809+) — 부품은 문서에 있으나 Win32 호스트에서 검은 사각형 보고(MS Q&A)·클립과 조합한 문서 없음·D2D 연동 추가 → PoC 전엔 쓰지 않는다 |
| (위 행 근거 — OS 백드롭을 안 쓰는 이유) | — | ① `DwmEnableBlurBehindWindow`: Win8 부터 효과 없음(문서). ② `DWMWA_SYSTEMBACKDROP_TYPE`(22621+): Mica/Acrylic 을 "창 전체 경계 뒤"에 그림 — 모양은 `SetWindowRgn` 에 기대야 하나 공존 문서 없음. ③ Acrylic 은 창이 비활성이면 단색(문서) — `NOACTIVATE` 일시 창에서 어떻게 되는지는 문서에 없다(PoC §9). ④ Win32 Mica 공식 경로는 Windows App SDK 런타임 필요(작업 규칙 "의존성" 위배)·Mica 는 불투명. ⑤ `SetWindowCompositionAttribute` 는 비공개 API. ⑥ 표준 `CreateBackdropBrush` 는 "비주얼 바로 뒤"(자기 트리 안)만 샘플 |
| 접근성(투명 효과·고대비·테마) | macOS 는 시스템이 처리했지만 Windows 는 직접 그리므로 앱이 반응: `UISettings.AdvancedEffectsEnabled`=false(설정 > 투명 효과 끔)면 면 알파 1.0 · 고대비(`SPI_GETHIGHCONTRAST`·`HCF_HIGHCONTRASTON`)면 틴트·투명 모두 끄고 `GetSysColor`(WINDOW·WINDOWTEXT·HIGHLIGHT·HIGHLIGHTTEXT) 사용, 포커스 이중 테두리 유지 | 변경 통지: 고대비는 `WM_THEMECHANGED`(HIGHCONTRAST 문서), 투명 효과·라이트/다크는 `UISettings` 이벤트 또는 `WM_SETTINGCHANGE` [확인 필요: 실기에서 어느 메시지가 오는지] |
| 통과 모드(`U` — 누르는 동안만) | `overlay.rs` 의 `set_click_through(bool)` **하나**를 일반 모드와 공유(macOS `ignoresMouseEvents = !판서 ∨ 통과`, `OverlayController.swift:142-143,171,192-196`): 켜면 `WS_EX_TRANSPARENT`(+LAYERED) ON·판서는 그대로 표시, 끄면 OFF. 상태 출처 `PassThroughModeSource{key(키코드), timed}`(`AppState.swift:14-17`)가 시작·종료 규칙을 가른다. **키(홀드)**: 판서 모드에서 카탈로그 `annotation.pass-through`(기본 `U`, 수정키 없는 단일 바인딩만, 키 반복 무시)를 누르면 진행 획을 확정(`finishActiveInteraction`/`finishAllActiveInteractions`)하고 `beginKeyPassThrough(keyCode)`, 같은 키 keyUp 이면 `endKeyPassThrough`(`CommandDispatcher.swift:74-105`, `InkView.swift:750-770`). 보드 중이면 안 켜지고, 이미 시간제면 키가 시간제 자원을 지우고 키 모드로 바꾼다(`AppState.swift:463-476`). **시간제(팔레트·마우스)**: 원형 팔레트 발표 그룹 항목(`RadialPaletteController.swift:244`) → `togglePassThroughMode`: 꺼짐→시간제, 시간제→끔, 키 모드→무시(`AppState.swift:437-446`). 켜기 조건=판서 모드∧보드 아님∧**전역 마우스 감시 설치 성공**(실패 시 안 켜고 로그, `:448-457`). 시간제 종료: 팔레트 항목 재선택 · `Esc`(체인 맨 앞, §2.2 한 단계 나가기) · 켠 뒤 첫 입력 없이 10초 · 마지막 클릭·스크롤 뒤 3초(만료 때 버튼이 눌려 있으면 3초 연장, `:527-532`) · 판서 끔(`:157-164`)/보드 켬(`:714-723`). 상수는 `PassThroughDefaults`(`:6-12`). 표시: 팔레트 중심 아이콘을 통과 모드 정의로 바꾸고 강조색 0.72 틴트+흰 아이콘(`RP:1999-2052`, `RG:767-825`) | `WM_NCHITTEST→HTTRANSPARENT` 는 **같은 스레드의 아래 창에만** 유효(문서) → 다른 프로세스 앱엔 못 쓴다, 채택 불가. layered 창에 `WS_EX_TRANSPARENT` 이면 모양과 무관하게 마우스가 아래 창으로 간다(Window Features 문서). 키 반복·keyUp 이 아래 앱으로 새면 `u` 글자가 들어가므로 키 라우팅 행 참조 |
| 통과 모드 — 전역 클릭 감시·안전망 | `WH_MOUSE_LL`(권한 프롬프트 없음): 버튼 down/up(좌·우·기타)·휠만 처리(이동 제외, `AppState.swift:95-103`), **통과 켤 때 설치·끌 때 해제**, 콜백은 `PostMessage` 만(타임아웃 규칙은 키 훅과 같음). 커서 강조(`presenter.rs`)와 훅을 공유하면 참조 카운트. 두 출처 모두 마우스 입력이 오면 3초 타이머를 다시 건다(`passThroughInteractionDidOccur`, `:509-524`). 키 모드는 첫 입력 전엔 키 뗌만 기다리고, 마우스 입력이 한 번이라도 오면 마지막 입력 뒤 3초, 그리고 최대 30초(`keyMaximumHoldDuration`, `:537-547`)가 키 뗌을 놓쳤을 때의 안전망이다. 자기 창 클릭은 기한을 연장하지 않는다(macOS 전역 감시는 자기 앱 이벤트를 안 받는다 — 팔레트 클릭으로 3초가 늘면 안 됨): UI 스레드에서 클릭 점이 **자기 비-오버레이 창(팔레트·힌트·배지·도움말)의 사각형 안인지**로 판정(PID·`WindowFromPoint` 는 통과 중인 오버레이가 자기 PID 라 오판). 기한은 `SetTimer` 한 개를 갱신 | 관리자 권한 앱 위 클릭은 UIPI 로 훅이 못 받을 수 있다 → 시간제는 첫 입력 전 10초 만료로, 키 모드는 키 뗌·30초로만 복귀 [확인 필요: 실기]. 복귀 때 `SetForegroundWindow` 호출 금지(오버레이는 NOACTIVATE 라 활성화 불필요 — macOS "앱을 활성화하지 않음" 자동 충족) |
| 통과 모드 — 키 뗌 감지·키 라우팅(설계 차이) | macOS 는 아래 앱을 누르면 그 앱이 활성화돼 오버레이가 keyUp 을 못 받으므로 키 뗌을 4수단으로 잡는다: ① 로컬 keyUp(`InkView.keyUp` → `endKeyPassThrough`) ② `CGEventSource.keyState` 60Hz 폴링(`:466-494` — 시작 150ms 안에 "안 눌림"이면 신뢰 불가로 폴링 중단·로그) ③ 안전망(마지막 마우스 입력 3초·최대 30초) ④ 손쉬운 사용 권한이 있으면 활성 이벤트 탭이 대상 키코드 하나의 keyDown/keyUp 을 삼킨다(수정키가 있으면 통과, keyUp 에서 종료, `:496-566`). **Windows 는 `WH_KEYBOARD_LL` 이 전경 창과 무관하게 모든 키를 보므로 ① 을 훅의 keyUp 으로 대체하고 ②·④ 는 만들지 않는다**(§7 #24): 통과 중 훅이 대상 키(`U`, 수정키 없음)의 keyDown(반복 포함)·keyUp 만 삼키고 나머지 키는 통과 → 아래 앱에 `u` 글자가 안 들어가고 keyUp 에서 종료. ③ 은 **유지**. 시간제는 첫 클릭 전에 오버레이가 포커스를 가져 `Esc` 가 체인에 닿는 macOS 동작에 맞춰, 훅이 이 구간(첫 클릭·스크롤 전)에만 `Esc` 를 삼켜 종료로 연결하고 이후엔 아무것도 안 삼킨다 | UIPI: 전경 창이 관리자 권한이면 LL 훅이 키를 못 받는다 → keyUp 을 놓치므로 ③ 만으로 복귀한다. `LowLevelHooksTimeout` 초과로 훅이 조용히 제거돼도 같다(판서 모드 한 글자 키 행). 반대로 대상 키 외 키를 삼키면 아래 앱 타이핑이 막힌다. macOS 는 손쉬운 사용 권한이 없으면 ④ 가 꺼진다(로그만, `:498-502`) — Windows 훅에는 권한 개념이 없다. 설계서·Swift 주석에 Windows 규칙은 없다 |
| 펜 압력·지우개 끝 | `WM_POINTER*` + `GetPointerPenInfo(History)`, 압력 0~1024. 유효 굵기=`설정 굵기×(최소 비율+(1−최소 비율)×pressure)`이고 `tools.pressureMinimumWidthRatio`는 0.05...1·기본 0.2다(`Ink.swift:255-257`; `S:629,683,734-742`). `PEN_FLAG_ERASER`/`PEN_FLAG_INVERTED`도 같은 입력에서 읽는다 | Wacom 은 드라이버 "Windows Ink 사용" 이 꺼져 있으면 WM_POINTER 가 안 온다 → 첫 실행 안내. 합쳐진 점은 History 로 받아야 스무딩이 안 끊긴다 |
| 임시 지우개·입력 소유권 | `uses_eraser = 마우스 우클릭 ∨ 펜 측면 버튼(IsBarrelButtonPressed) ∨ 펜 지우개 끝(IsInverted/IsEraser) ∨ currentTool==eraser`. 어느 도구에서 시작해도 `currentTool`은 바꾸지 않고 현재 지우개 설정으로 down~up 한 제스처를 만든 뒤 `replaceStrokes` 한 번으로 Undo 1단위(`IV:523-682,875-925,1491-1508`; `AS:995-1016,1061-1075`). 소유 버튼/포인터 하나를 잡은 동안 다른 버튼은 무시하고, 다음 down 때 이전 소유 버튼이 실제로 안 눌렸으면 누락된 up으로 보고 진행 상호작용을 끝낸 뒤 새 입력을 받는다(`IV:207-219,250,523-534`) | Windows의 "실제로 눌림" 확인은 마우스는 `GetAsyncKeyState`, 펜은 포인터 상태를 쓰는 안이지만 API 조합은 **[확인 필요: W2 실기]**. `CANCELED`·`CAPTURECHANGED`도 같은 종료 경로. **장치 종류(`POINTER_INPUT_TYPE`·`IsInverted`)는 해당 `WM_POINTER` 이벤트에서만 읽고 진입·이탈 같은 다른 이벤트에서는 읽지 않는다**(#50; macOS는 잘못된 이벤트에서 예외, Windows는 잘못된 값 위험). 측면 버튼=우클릭=임시 지우개, Pencil 더블탭 대응은 §7 #34 |
| 레이저(점·꼬리) | `laser.rs`: 꼬리 = 누른 구간별 점 목록(위치·시각), 그릴 땐 중간점을 끝점·현재 점을 제어점으로 잇는 **이차 곡선** 한 `PathGeometry`(구간마다 Figure)·`StrokeStyle` 둥근 캡·조인·굵기 `max(4, 크기×1.25)`·색=레이저색을 **불투명으로 그리고 `PushLayer` 불투명도 0.7**(`Ink.swift:581-603`; `IV:21-43,117-129`, `IV`=`InkView.swift`). 점=반지름 `laserSize`, 글로우(색 동일·불투명 0.9·반경=크기)는 점+꼬리에 **한 번**(D2D `Shadow` 효과; 구간마다 주면 이음매가 구슬로 보인다, 027 수정). 줄이기: 프레임/16ms 마다 `cutoff=now−꼬리시간` 보다 오래된 점을 **오래된 끝부터** 제거하되 만료점 1개는 남겨 끝이 cutoff 에 맞게(`IV:97-115`), 점이 다 사라지고 누르는 중이 아니면 중단. 시각은 단조 시계(`QueryPerformanceCounter`). 도구 전환·판서 끔·보드 진입 시 `clear`(`IV:316-350`) | 입력 매핑은 다음 행 |
| 레이저 입력 매핑 | 호버(점만) = `WM_POINTERUPDATE`∧INRANGE∧¬접촉 · 누름(꼬리) = 펜·터치 `INCONTACT`, 마우스 `FIRSTBUTTON`(좌클릭만, macOS 도 좌 드래그만) · 놓음 = `UP` → 꼬리 추가 중단(기존 꼬리는 줄어듦) · 오버레이·범위 이탈 = `WM_POINTERLEAVE`/INRANGE 해제 → 점 숨김+꼬리 끊기(`tabletProximity`, `IV:498-505`) · `CANCELED`·`CAPTURECHANGED` 도 끊기 | 펜이 범위를 벗어났다 돌아오면 **새 `pointerId`**(POINTER_INFO 문서) → 이전 꼬리와 잇지 않는다(안 끊으면 점프 줄이 화면을 가른다). 펜 지우개 끝(`PEN_FLAG_ERASER/INVERTED`)은 레이저 선택 중에도 지우개로 동작 — 점 숨기고 지우개 커서(`IV:640-651,1491-1497`). 마우스의 `INCONTACT` 의미(우클릭만 눌러도 켜지는지) [확인 필요: W2 실기] |
| 도구별 마지막 설정(`state.rs`) | 슬롯 6개(`pen·highlighter·arrow·rectangle·ellipse·text`)=`{colorIndex,size}`, 저장소는 설정 `tools.perToolMemory`(.pansetting 대상) — `state.rs` 는 값을 안 갖고 `settings.rs` 를 읽고 쓴다. 슬롯 없음: 레이저·지우개·선택·가리개. 크기 범위 1–30(글자 8–96), 기본 pen 3·형광펜 18·화살표/사각/원=pen 값·글자 24(`Ink.swift:38-72`, `SettingsModel.swift:119-122,207-266`). **읽기**: 슬롯 도구=자기 슬롯, 슬롯 없는 도구=`lastColorTool`(마지막으로 슬롯 있던 도구, 초기 pen)의 색(`AS:84,186-205`) → 획 색·중심 테두리 색 모두. **숫자키** `selectColor(i)` 는 §2.2 단축키 "숫자키 1~6". `[` `]`=슬롯 크기 ±(범위 클램프), 지우개=지름 ±`delta×2`(`AS:845-850,950-973`, `Ink.swift:155`) | 도구 전환은 색·크기를 바꾸지 않는다(현재 도구 슬롯을 읽을 뿐). 프리셋 수가 줄면 인덱스는 마지막으로 클램프(설계서). 팔레트 3링 선택 표시는 이 값으로 판정: 색=`selectedColorIndex==i`, 굵기=`\|기억값−값\|<0.001`(`RP:1885-1954`) |
| 실시간 줌 | Win11: **WGC**(`CreateForMonitor`, `CreateFreeThreaded` 프레임 풀) → D3D11 → D2D. `capture.rs` 는 `GraphicsCaptureSession.IsCursorCaptureEnabled = false`; `zoom_view.rs` 는 앱 포인터를 D2D 최상단 레이어로 직접 그린다(macOS `showsCursor=false`·벡터 레이어 `ZoomController.swift:92-99,164-206,459-472`). Windows 는 `GetCursorInfo`와 커서 비트맵 취득으로 I-빔·손가락 등 실제 모양을 그릴 수 있어 macOS의 화살표 하나 한계가 없다. 크기는 라이브·정지에서 배율 `m`배, 미리보기 1배(`Zoom.swift:210-225`). 줌 창의 `WM_SETCURSOR`에서 투명/NULL 커서를 설정하고 전역 `ShowCursor` 카운터는 쓰지 않는다. 라이브↔정지와 정지 종료 때 `SetCursorPos`로 시각 위치를 보존한다(`ZoomController.swift:746-765,816-856,966-1015`) | macOS 는 스트림별 자기 앱 제외(`SCContentFilter`). Windows 의 스트림별 창 제외는 Magnification API(`MagSetWindowFilterList` EXCLUDE)뿐. 기본안: 줌 창 `WDA_EXCLUDEFROMCAPTURE`(macOS 줌 패널 `sharingType=.none` 과 같음). 따라서 줌 중 화상회의 공유에는 줌 창 안 판서도 보이지 않는다. **WGC 노란 테두리 끄기(`IsBorderRequired`)는 10.0.20348+ 이고 사용자 동의 + 패키지 capability `graphicsCaptureWithoutBorder` 필요** → 비패키지에서 안 되면 DDA. DDA 는 테두리 없음 대신 하이브리드 GPU·`ACCESS_LOST` 처리와 포인터 직접 합성이 필요하다. `SetCursorPos`는 물리 픽셀과 DIP 변환 및 음수 원점에 주의(§3 "고 DPI·혼합 DPI"). 실제 커서 비트맵의 핫스팟·다중 DPI·애니메이션 프레임 취득은 **[확인 필요: W7 실기]** |
| 라이브 줌 판서 | `zoom_view.rs`의 판서 뷰는 라이브·정지·미리보기 모두 표시하되 입력은 미리보기가 아니고 판서 모드가 켜졌을 때만 받는다. 입력 매핑은 라이브=`identity`, 정지=`original_point` 역변환이고, 표시는 둘 다 `zoom.rs`의 공통 순수 함수 `pointer_presentation_point`·`pointer_scale`·`original_point`를 사용한다(`ZoomController.swift:340-347,382-387,569-615`; `InkView.swift:167-176,523-552,1964-1987`). 진행 중 획·도형·가리개·지우기·선택·텍스트 입력·버튼 소유권에서 `gesture_in_progress`를 **파생**하고 별도 래치는 두지 않는다(`InkView.swift:269-277`); W3 입력과 같은 소유권 규칙으로 따라가기·배율·`Space`·`P`·`F`를 잠근다(`ZoomController.swift:794-864,935-964,1017-1067`). 키 이벤트는 줌 키 → 판서 키 우선순위로 라우팅한다. 줌 창은 포커스를 유지하되 마우스 이동마다 재요청하지 않고, 이미 올바른 대상이거나 판서 뷰 하위 텍스트 입력창이면 유지하는 멱등 포커스 함수 하나만 쓴다(`ZoomController.swift:232-254,545-566,722-724,1069-1078`) | 라이브 입력 좌표는 실제 포인터 화면 좌표와 같아 획 끝이 확대 포인터 끝과 일치하고, 표시 변환은 판서를 화면 좌표에 고정해 확대 위치가 바뀌어도 마우스를 따라가지 않는다. 미리보기는 입력만 막고 기존 판서는 표시한다. 텍스트 `EDIT`가 활성일 때 포커스를 빼앗거나 줌/판서 단축키를 적용하지 않는다. 포커스 재요청을 포인터 이동 경로에 넣으면 IME 조합이 끊긴다 |
| 공유에서 창 숨기기 | `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` — 팔레트·수업 배지·도움말(설정에 따라) | Win10 2004+, top-level·자기 프로세스 창만, DWM 합성 중에만. 2004 미만은 `WDA_MONITOR`(검은 사각형)로 동작. 런타임 토글 가능 |
| 쉬는 시간 음악 | 열기: `chrome.exe <URL>`(없으면 `ShellExecuteW`). 종료: SMTC 로 Chrome 세션만 `TryPauseAsync`(§7 #7 결정) | **CDP 불가**: Chrome 136+ 는 기본 프로필에서 `--remote-debugging-port` 를 무시한다. 탭은 남는다(사용자 결정). SMTC 에 Chrome 세션이 안 잡히는 경우 [확인 필요: W6 실기] |
| 볼륨 페이드 | `IAudioEndpointVolume::SetMasterVolumeLevelScalar`(CoreAudio 와 1:1). 선택: `ISimpleAudioVolume` 로 **Chrome 세션만** 페이드 | 페이드 중 사용자가 볼륨을 바꾸면 복원값 규칙 필요(macOS 와 동일 규칙으로) |
| 로그인 자동 실행 | 비패키지: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. MSIX: `StartupTask` | MSIX StartupTask 는 사용자가 설정 앱에서 끌 수 있고 앱이 다시 못 켠다 |
| 고 DPI·혼합 DPI | PerMonitorV2, `WM_DPICHANGED`, 오버레이·스왑체인을 물리 픽셀로 | 비율 좌표 저장이라 판서는 안전. 모니터 경계를 넘는 드래그(배지)는 DPI 가 다른 두 좌표계 — 물리 좌표로 계산. 모니터 좌표·크기는 전부 **물리 픽셀 가상 화면 좌표**로 다룬다(PerMonitorV2 는 각 모니터의 원시 픽셀을 본다 [D14], `GetMonitorInfo`/`EnumDisplayMonitors` 는 가상 화면 좌표 [D15], CCD 는 DPI 가상화를 안 한다 [D6][D9]). 보조 모니터가 주 모니터 왼쪽·위면 **음수 좌표가 정상**이다. DPI 가 비-PMv2 스레드 컨텍스트로 가상화되면 같은 API 가 다른 값을 돌려주므로 모니터 조회·창 생성은 PMv2 스레드에서만 한다 |
| 모니터 변경·잠자기 | 숨은 최상위 창의 `WM_DISPLAYCHANGE` + `WM_DEVICECHANGE`(`GUID_DEVINTERFACE_MONITOR`) + 오버레이 자신의 `WM_DPICHANGED` → 모두 `schedule_sync()` 하나로(0.3초 `SetTimer` 디바운스) → 멱등 동기화(현재 모니터 목록 vs 가진 오버레이 비교: 새 모니터=창 생성, 사라진 모니터=그리기 종료·판서 보관·창 파괴, 크기 변경=`SetWindowPos`+스왑체인 리사이즈). `WM_POWERBROADCAST`(`PBT_APMSUSPEND` 보관, `PBT_APMRESUMEAUTOMATIC` 타이머 재계산·동기화) | 메시지 전용 창은 브로드캐스트를 못 받는다. `WM_DISPLAYCHANGE` 는 해상도 변경 때만 온다는 문서 서술 [확인 필요: 위치만 바뀌는 변경] → 장치 알림으로 보강. 활성 경로 `targetAvailable=FALSE` 는 과도 상태. macOS 도 CG 콜백이 `NSScreen` 보다 먼저 와서 AppKit 알림을 함께 듣게 고쳤다(`OverlayController.swift:45-52`) — 알림원 하나에 의존 금지. 프리셋 판단용 5초 안정 시간은 별개(§3.1.4). 줌 중 대상 모니터 해제·크기 변경 = 줌 종료(`ZoomController.swift:1080-1102`)를 같은 이벤트로 |
| 화면 사라짐·복원(`ArchiveController` 대응) | `displayWasRemoved`(`ArchiveController.swift:35-60`) = 정책 `archive`(기본, 화면별 파일 보관)/`moveToPrimary`/`delete` 를 모니터 해제 동기화의 같은 지점에서. 다시 연결되면 `restoreDisconnectedDisplayIfNeeded`(`:63-97`)가 같은 키의 보관본을 묻거나(알림 → 대화 대역 대화상자) 자동 복원. 절전·덮개: `PBT_APMSUSPEND`에서 전 화면 보관, `PBT_APMRESUMEAUTOMATIC`에서 동기화(`:270-293`) → 모니터가 같은 키로 돌아오면 복원 제안. 프리셋 전환으로 사라지는 화면은 정책과 무관하게 보관(§3.1.3 #10) | **보관 키가 macOS 는 UUID 문자열**(`safePathComponent(displayUUID)`, `:215,308-332`)이다. Windows 에서 `monitorDevicePath` 를 키로 쓰면 포트·독이 바뀔 때 보관본을 못 찾는다 → 키 = **매칭된 식별**(vendor:model:serial, serial≠0 일 때; 아니면 devicePath) 로 한다. 키 규칙은 `monitor.rs` 의 한 함수 — 보관용 키를 따로 만들지 않는다. 복원 질문 창은 대화 대역 owner 로 [확인 필요: 덮개 닫힘 시 `PBT_APMSUSPEND` 가 `WM_DISPLAYCHANGE` 보다 먼저 와 보관이 끝나는지] |
| 키 표시·커서 강조 | `WH_KEYBOARD_LL`·`WH_MOUSE_LL`(권한 프롬프트 없음) | 관리자 창 전경 시 미수신(UIPI) |
| 트레이 아이콘 | `Shell_NotifyIconW` + ICO 4종(`on/off` × `light/dark-taskbar`), `LoadImageW`(§3.2.5) | 작업 표시줄 밝기(`SystemUsesLightTheme`) ≠ 앱 테마. `TaskbarCreated` 재등록. `NIF_GUID` 는 exe 경로·서명에 묶여 미사용 |
| 서명 | Authenticode. 2023-06 이후 OV/EV 키는 하드웨어(HSM/토큰) 보관 필수 | SmartScreen 평판은 다운로드가 쌓여야 생긴다. 클라우드 서명 서비스의 한국 개인·법인 자격 [확인 필요] |
| 배포 | MSIX(+`.appinstaller` 자동 업데이트, `graphicsCaptureWithoutBorder` 선언 가능) 또는 exe 설치기 | MSIX 안에서 `uiAccess` 가능 여부 [확인 필요]. winget 은 어느 쪽이든 매니페스트 PR. 설치기·exe·파일 형식 아이콘은 같은 `.ico`(§3.2.7) |
| 자동 업데이트 | MSIX: App Installer 내장. 비-MSIX: WinSparkle(Sparkle appcast 호환 — macOS 와 피드 공유) | WinSparkle 은 C DLL(Rust 밖 바이너리 의존) |
| .pansetting 연결 | 비패키지 `HKCU\Software\Classes`(확장자 → ProgID → `shell\open\command`), MSIX `fileTypeAssociation` | 아이콘 = `DefaultIcon` `"<exe>",0`(앱 아이콘 재사용, macOS 와 동일) + `SHChangeNotify(SHCNE_ASSOCCHANGED)`. exe 이동 시 경로 갱신 필요 |
| PNG(배경 포함) | 줌과 같은 캡처 경로 1프레임 | — |

### 3.1 화면 구성 프리셋(Windows)

macOS 는 **1단계(모델·계획·캡처·설정 탭)와 2단계(수동 적용·15초 확인·복구)**를 구현했다. 3단계 자동 전환만 Swift 코드가 없다. 규칙 정본은 `docs/design.md` "화면 구성 프리셋"(결정 1~9 + 안전장치, §7 #20)과 `DisplayApplier.swift`이며, OS 호출은 app 의 `monitor.rs`(읽기 전용) · `display_apply.rs`(적용·복구) 두 곳에만 둔다. 순수 로직은 core `display_preset.rs`와 연산 목록 생성(§3.1.3·6).

#### 3.1.1 식별 — `DisplayIdentity`(`DisplayPreset.swift:69`) 대응

| 필드 | Windows 값 | 출처·함정 |
| --- | --- | --- |
| uuid | `monitorDevicePath`(대소문자 무시 비교) | `DisplayConfigGetDeviceInfo(…GET_TARGET_NAME)` [D1]. 인스턴스 부분이 커넥터에 묶인다 [확인 필요: 포트·독 변경 시 바뀜] |
| vendor | EDID 바이트 8–9(빅엔디언 제조사 코드) | EDID 블록을 직접 파싱. `edidManufactureId` 는 `edidIdsValid` 일 때만 유효하고 바이트 순서 [확인 필요] [D1] |
| model | EDID 바이트 10–11(LE) | 위와 동일(`edidProductCodeId`) |
| serial | EDID 바이트 12–15(LE u32), 0 = 없음 | 아래 EDID 취득. `WmiMonitorID.SerialNumberID` 는 ASCII 기술자라 u32 와 다르다 [D2] |
| name | `monitorFriendlyDeviceName`, 비면 `"외장 디스플레이 %08X"`(model) | macOS 폴백과 같음(`OnlineDisplayProvider.swift:98-105`) |
| kind | `outputTechnology`: INTERNAL·DISPLAYPORT_EMBEDDED(11)·UDI_EMBEDDED(13)·LVDS(6) → builtIn / MIRACAST(15)·INDIRECT_WIRED(16)·INDIRECT_VIRTUAL(17) → virtual / 그 외 external | 문서: embedded 값이 있으면 INTERNAL 은 무시 [D3] |

- **EDID 취득**: `SetupDiGetClassDevs(GUID_DEVINTERFACE_MONITOR, DIGCF_DEVICEINTERFACE\|DIGCF_PRESENT)` → 인터페이스 `DevicePath` 를 `monitorDevicePath` 와 대소문자 무시 비교 → `SetupDiOpenDevRegKey(DIREG_DEV, KEY_READ)` → `Device Parameters\EDID` [D4][D5]. 이 레지스트리 값은 공식 스키마 문서가 없다(관행) [확인 필요: 실기 모니터 2종의 8–15 바이트를 macOS 가 같은 모니터로 내보낸 `.pansetting` 의 `identity` 와 대조 — 같으면 교차 OS 에서도 ②단계로 매칭].
- **매칭 3단은 core 그대로**(`DisplayPreset.swift:77-101`): ① uuid 일치 ② vendor+model+serial(serial≠0) ③ 양쪽 virtual 이고 vendor+model+name. 의미만 바뀐다. Windows 쪽 약점:
  - devicePath 가 포트에 묶이면 ①이 자주 깨지고 ②로 떨어진다. **serial 이 0·1 인 모니터·프로젝터는 ② 도 실패**(`:84` 의 `serial != 0` 가드) → macOS 보다 "연결 안 됨"이 많이 난다 [확인 필요: 실기 빔·노트북 내장·외부 모니터의 EDID serial].
  - 같은 모델 2대(serial 0)는 구분 불가 — 한계로 두고 설정 탭의 "다시 지정"(`DisplayPresetsSettingsView.swift:385`)에 맡긴다.
  - ③은 vendor·model 이 0 인 가상 모니터끼리 name 만으로 맞아 오매칭 가능 [확인 필요: Miracast·IDD 가 EDID 를 합성하는지].

#### 3.1.2 현재 구성 조회 — `OnlineDisplay`(`DisplayPreset.swift:292`) 만들기

| 항목 | 방법 |
| --- | --- |
| 소스 | `QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS \| QDC_VIRTUAL_MODE_AWARE)`, `ERROR_INSUFFICIENT_BUFFER` 면 크기부터 다시 [D6]. **`EnumDisplayMonitors` 는 쓰지 않는다** — 복제 그룹이 HMONITOR 하나라 타깃 단위인 매칭·자동 집합(`:169-173`)과 안 맞는다. macOS 가 `NSScreen.screens` 대신 CG 온라인 목록을 쓰는 이유와 같다(034 지시서 §2.2) |
| id | 조회마다 붙인 불투명 u32. app 이 (adapterId LUID, targetId) 표를 쥔다. **LUID 는 재부팅마다 바뀌므로 저장 금지** |
| size · origin | 소스 모드 `width/height`·`position`. 물리 픽셀이고 CCD 는 DPI 가상화를 안 한다 [D6][D8] |
| 주 화면 | 소스 `position == (0,0)` [D8] |
| `currentMirrorMasterID` | 같은 소스(adapterId, id)를 쓰는 활성 경로 묶음(= 복제 그룹)에서 `pathArray` 우선순위상 첫 타깃. **CCD 에 마스터 개념이 없다 — 이 관례가 규칙**(§7 #27) [D6][D7]. 묶음 첫 타깃은 nil |
| 연결 집합 | 활성 경로만 세면 Win+P "PC 화면만"에서 빔이 사라진 것으로 읽힌다. 비활성 연결 모니터는 위 SetupAPI present 목록으로 센다. `QDC_ALL_PATHS` 는 비용이 크고 비활성 경로의 타깃 정보가 기본값이라 쓰지 않는다 [D6] [확인 필요: 비활성 모니터가 present 로 남는지, 비활성 모니터의 size = 선호 모드(`GET_TARGET_PREFERRED_MODE`)로 계획 보기가 되는지] |

#### 3.1.3 적용·복구 — `display_apply.rs` (macOS 2단계 구현 반영, W11)

macOS 정본은 순수 연산 목록 뒤 CG 계층 한 겹에서 적용하고 상태를 `idle → applying → awaitingConfirmation → idle`로만 이동한다(`DisplayApplier.swift:5-38,49-79,134-319`). Windows도 같은 상태 기계를 쓰며 app이 core 결과를 재해석하지 않는다.

| # | 단계 | Windows 수단·규칙 |
| --- | --- | --- |
| 1 | 계획·연산 | core `DisplayPlanner::plan(preset, online)` 뒤 `DisplayConfigurationOperations::build(plan)`이 **미러 해제 → 미러 설정 → 프리셋 화면 원점 → 프리셋 밖 화면 원점** 순의 연산 목록을 만드는 순수 함수다(macOS `DisplayApplier.swift:5-38`). 같은 입력은 같은 목록을 내며 **출력은 core가 정하고 app은 바꾸지 않는다** |
| 2 | 주 화면 보장 | 소스 `(0,0)` = 주 [D8]. 플래너가 앵커(연결된 화면 중 역할 순 주 → 보조1 → 보조2 의 첫 화면)를 원점 (0,0)·`mainDisplayID` 로 삼으므로(R2·R7) 주 화면 모니터가 전부 빠져도 보조 화면이 (0,0) 의 주가 된다 — app 이 원점을 따로 옮기지 않는다. 계획 화면이 0개면 프리셋 밖 모니터 중 현재 주(원점≈0·미러 아님)가 먼저 (0,0)(R6). 앵커가 주 화면이 아니면 계획 글이 "주 화면 모니터 없음 → 보조N 이 주 디스플레이" 로 알린다(`DisplayPlan.summary()`, `:410-`) |
| 3 | 겹침 검사 | GDI 는 소스 사이 **틈·겹침을 금지**하고 `SetDisplayConfig` 가 "정의되지 않은 방식"으로 재배치한다 [D8]. 격자 이웃은 변을 맞대 붙으므로(R2) **틈은 규칙상 발생하지 않는다**(구 규칙 — 열·행 최대 폭 합산 — 은 #38 로 삭제). 겹침은 `DisplayPlan.layoutIssues` + `isApplicable == false`(`mainDisplayID` 없음도 불가, `:404-408`) → **적용하지 않고 사유를 알린다**(보정 금지; macOS 설정 탭의 "화면 구성 계획 · 적용 불가" 와 같은 문구). 적용 직전 `SDC_VALIDATE` 는 Windows 쪽 추가 안전장치 — GDI 가 그래도 거부하면 사유 표시 |
| 4 | 스냅샷 | 적용 직전 `QueryDisplayConfig(ONLY_ACTIVE\|VIRTUAL_MODE_AWARE)`의 path·mode 배열 전체를 복구본으로 보관하고 core 연산 목록으로도 만든다. macOS 스냅샷은 모든 디스플레이 미러 해제 → 원래 미러 복원 → 독립 디스플레이 원점 순의 `DisplayPlan`을 만든 뒤 같은 `build`를 재사용한다(`DisplayApplier.swift:366-398`) |
| 5 | 경로 구성 | 화면 마스터 = 자기 소스. 슬레이브 = **마스터의 소스 id 와 같은 소스 모드 인덱스**를 가리킨다(소스 모드는 소스 id 당 하나, 같은 소스를 쓰는 경로는 같은 `modeInfoIdx`) [D9]. 프리셋 밖 모니터 = 복제에서 빼 별도 소스에 확장(`plan.outsidePlacements`). 연결 안 된 프리셋 모니터는 건너뛴다. 어댑터가 다른 모니터끼리의 복제는 소스를 공유할 수 없다 [확인 필요: iGPU+dGPU 노트북에서 ERROR_BAD_CONFIGURATION → 실패로 알림] |
| 6 | 해상도 제외 | 이미 활성인 마스터는 스냅샷의 소스 모드(width·height·pixelFormat)와 타깃 모드를 그대로 두고 **`position` 만** 계획 원점으로 바꾼다. 비활성이던 모니터는 모드 인덱스 INVALID 로 켜서 best-mode 에 맡기고(문서) → 켜진 뒤 재조회·**재계획**(plan 은 순수라 새 `online` 으로 다시 돌린다) → `position` 만 고쳐 재적용. 토폴로지 전용 호출은 `SDC_TOPOLOGY_SUPPLIED`(모드 NULL·인덱스 INVALID, DB 항목 없으면 실패 → `USE_SUPPLIED` 로 재호출) [D9] |
| 7 | 검증·임시 적용 | `SDC_VALIDATE\|SDC_USE_SUPPLIED_DISPLAY_CONFIG\|SDC_VIRTUAL_MODE_AWARE` → 통과하면 `SDC_APPLY\|…`만 사용해 임시 적용(저장 안 함). 이는 macOS `.forAppOnly`에 대응한다. **적용에는 `SDC_ALLOW_CHANGES` 를 쓰지 않는다** — 문서상 시스템이 소스·타깃 모드를 "손봐서" 해상도를 바꿀 수 있어 결정 3(해상도 미저장·마스터 현재 모드 유지)과 충돌한다. `ERROR_BAD_CONFIGURATION` 이면 사유와 함께 적용 실패로 보고(복구 불필요: 아무것도 안 바뀜). 반환 후 `matchesCurrent`(`:804-825`, 미러·원점만 비교)에 더해 **마스터 소스 모드 width·height 가 스냅샷과 같은지**도 확인하고 다르면 복구 |
| 8 | 확인·확정 | 수동: `manualConfirmationInterval` 15초(`DisplayPreset.swift:8`) 비모달 확인 창(§3 "창 높이 대역" 행의 대화 대역). [유지]는 같은 구성을 `SDC_SAVE_TO_DATABASE`로 다시 적용해 macOS `.permanently`에 대응하고, [되돌리기]·`Esc`·창 닫기·시간 초과는 스냅샷을 적용한다. 자동: 적용 직후 `matchesCurrent` 검증 → 통과 시 저장, 실패 시 즉시 복구, 확인 창 없음(design.md) |
| 9 | 복구 | 스냅샷을 `SDC_APPLY\|SDC_USE_SUPPLIED_DISPLAY_CONFIG\|SDC_ALLOW_CHANGES` 로 재적용. 실패하면 `SDC_APPLY\|SDC_USE_DATABASE_CURRENT`(마지막 저장 구성으로 복귀 — 문서의 "임시 모드에서 복귀" 시나리오) [D9]. 15초 타이머는 UI 스레드 밖에서 만료 시 복구(화면이 안 나와도 돌아온다) |
| 10 | 판서·복원 질문 | 사라질 화면의 판서는 일반 정책과 무관하게 **보관**(결정 6). 훅 위치 = `OverlayController.synchronizePanels` 의 `displayWasRemoved` 호출(`:88-94`) 직전 — Windows `overlay.rs` 의 같은 지점. 확인 패널이 열린 동안 복원 질문을 보류하고 닫힌 뒤 재개한다(macOS `ArchiveController.swift:63-76,99-160`; `DisplayApplier.swift:201-250,274-318`) |

- `ERROR_ACCESS_DENIED` = 콘솔 세션이 아니거나 원격 세션 [D9]: RDP 에서는 프리셋 적용 비활성.
- Windows 는 **연결 모니터 집합별 마지막 구성을 DB 에서 스스로 복원**한다(`SDC_USE_DATABASE_CURRENT`). 저장하지 않은 임시 구성은 재연결·재부팅 뒤 사라지고, 저장하면 Win+P 의 "마지막 확장 구성"도 덮인다 [확인 필요: 8 단계 저장이 Win+P 복귀 구성에 미치는 영향]. 저장 정책 §7 #26.
- macOS의 `.forAppOnly`는 확인 중 앱이 종료되면 시스템이 복구하지만 Windows `SDC_APPLY`에는 앱 종료 자동 복구 대응이 없다. 비정상 종료에도 즉시 되돌릴 대체 안전장치가 필요하다 **[확인 필요: 별도 감시 프로세스 또는 다음 시작 시 미확정 구성 복구]** — §7 #36.

#### 3.1.4 변경 감지

| 항목 | 규칙 |
| --- | --- |
| 수신 창 | **숨은 최상위 `WS_POPUP`**. 메시지 전용(`HWND_MESSAGE`)은 브로드캐스트를 못 받는다 [D10]. `WM_DISPLAYCHANGE` 는 "해상도가 바뀔 때" 최상위 창에만 전송 [D11] → 위치만 바뀌는 변경·연결 해제가 안 올 수 있다 [확인 필요: 모니터 추가·위치 이동 시 도착 여부 — 실기 로그]. 보강: 같은 창에 `RegisterDeviceNotification(GUID_DEVINTERFACE_MONITOR)` → `DBT_DEVICEARRIVAL/REMOVECOMPLETE` [D12][D5]. 어느 알림이든 `schedule_sync()` 하나로 모은다 |
| 디바운스 | ① 0.3초 → 오버레이 재동기화(macOS `OverlayController.swift:73`, §3 "모니터 변경·잠자기" 행). ② `connectionStabilityInterval` 5초(`:6`) → 프리셋 판단. ③ 자동 적용 뒤 쿨다운 30초(`:7`) |
| 중간 상태 | 활성 경로의 `targetAvailable=FALSE` 는 문서가 밝힌 과도 상태 [D13] → 안정 시간 전에는 집합 판정에 쓰지 않는다. macOS 의 "CG 콜백이 `NSScreen` 보다 먼저 온다·Sidecar 가 중간 크기를 거친다"(`OverlayController.swift:45-46`)와 같은 부류 |
| 자체 변경 | 적용 뒤 **기대 상태**(plan 결과)를 기록하고, 디바운스 뒤 조회 결과가 기대와 같으면 자체 변경으로 무시한다. "일정 시간 모든 알림 무시"는 쓰지 않는다(사용자 변경을 삼킨다) |
| 트리거 | 프리셋 판단은 **연결 집합이 바뀔 때만**. 토폴로지만 바뀌는 Win+P(`SDC_TOPOLOGY_INTERNAL/CLONE/EXTEND/EXTERNAL` = PC 화면만·복제·확장·두 번째 화면만) 는 트리거가 아니다 [D9] — §7 #25 |
| 전원 | `WM_POWERBROADCAST`: `PBT_APMSUSPEND` ↔ macOS `willSleep`(전 화면 보관), `PBT_APMRESUMEAUTOMATIC` ↔ `didWake`→`refreshAfterWake`(`ArchiveController.swift:270-293`, `OverlayController.swift:255`). 복귀 직후 재열거가 늦으므로 5초 안정 시간이 흡수 |

#### 3.1.5 무선·독 (macOS Sidecar UUID 미확인 문제의 Windows 판)

`OnlineDisplayProvider.swift:109-110` 은 Sidecar·AirPlay UUID 의 재연결 유지 여부를 미확인으로 두고 ③단계(vendor+model+name)로 흡수한다. Windows 도 같은 설계 + 실기:
- **Miracast** = `outputTechnology` MIRACAST → virtual. 연결마다 타깃 id·devicePath 가 바뀔 수 있고 협상 중 경로가 나타났다 사라진다 → ③ + 5초 안정 시간 [확인 필요: 같은 수신기에 2회 연결해 devicePath·EDID 비교].
- **독·USB-C·MST**: 포트·허브가 바뀌면 devicePath 인스턴스가 바뀐다(①실패, ②로 흡수). **DisplayLink 류 IDD** 는 INDIRECT_* → virtual 로 분류되어 ③을 쓴다 — IDD 뒤 실제 모니터의 EDID 가 전달되는지 [확인 필요].
- 삼성 갤럭시 탭(화면 확장 앱)은 IDD/Miracast 계열일 가능성 — §7 #3 의 W1 ③ 실기에서 `outputTechnology` 와 devicePath 를 함께 기록한다.

#### 3.1.6 core 이식 경계 — `display_preset.rs` (모델·검증기 W2, 나머지 W10; OS 의존 0)

옮길 것: 모델·상수 전부(`:3-187`, 5초·30초·15초 상수 포함 — apply 층이 core 에서 읽는다) · `DisplayGrid.validationError/isConnected`(`:227-252`) · `DisplayPresetValidator`(`:254-290`) · `DisplayMatcher`(`:327-358`) · `DisplayGridLayout.origins`(`:468-623`) · `DisplayPlan.layoutIssues/isApplicable/summary`(`:396-`) · `DisplayPlanner.plan/overlappingLayoutIssues/presetForConnectedSet/matchesCurrent`(`:631-829`) · `DisplayPresetCapture.current`(`:852-1006`). 안 옮길 것: `OnlineDisplayProvider`(app). **지킬 규칙**(줄 = `DisplayPreset.swift`):

| # | 규칙 |
| --- | --- |
| R1 | 칸 x·y ∈ −1..1, 주 화면은 (0,0) 고정, 칸 중복 금지, 점유 칸이 상하좌우로 주 화면에 이어질 것(대각선만 닿으면 거부; 보조1 을 거치는 연결은 허용) `:228-251` |
| R2 | **앵커 = 연결된(마스터가 있는) 화면 중 역할 순(주 → 보조1 → 보조2) 첫 화면, 원점 (0,0).** 격자 이웃은 BFS(부모 = 배치 순, 후보 = 목록 순)로 변을 맞대 붙인다: 오른쪽 `(P.right, P.top)` · 왼쪽 `(P.left − 자기 폭, P.top)` · 아래 `(P.left, P.bottom)` · 위 `(P.left, P.top − 자기 높이)`. 열·행 최대 크기 합산은 없다(#38 로 삭제). **끊긴 화면**(중간 화면 미연결로 앵커에 안 이어짐)은 BFS 가 끝난 뒤 목록 순 첫 미배치 화면을 `detachedOrigin` 으로 앵커 방향의 묶음 바깥 변에 붙인다: 앵커 대비 칸 차이 (dx,dy) 에서 `dx≠0 ∧ \|dx\|≥\|dy\|` 면 가로 — dx>0 은 배치된 화면 우변 최댓값 x·그 우변에 닿는 첫(배치 순) 화면의 위쪽 y, dx<0 은 좌변 최솟값 − 자기 폭·그 좌변 첫 화면의 위쪽 y; 아니면 세로 — dy>0 은 하변 최댓값 y·그 하변 첫 화면의 왼쪽 x, dy<0 은 상변 최솟값 − 자기 높이·그 상변 첫 화면의 왼쪽 x. 배치 뒤 그 화면의 이웃으로 BFS 계속. 칸 중복 불가(precondition) `:468-523, 525-554, 556-618` |
| R3 | 슬롯 평탄화 순서 = 주 → 보조1 → 보조2, 화면 안은 목록 순 `:654-665` |
| R4 | 매칭 = 단계(uuid → serial → virtualName)별로, 프리셋 인덱스 오름차순으로, **아직 안 쓴 첫 온라인**을 잡는다(순회 순서가 곧 규칙). 결과는 프리셋 인덱스순 `:344-356` |
| R5 | 화면의 연결된 첫 모니터 = 마스터, 나머지 = 슬레이브(미러). 연결된 모니터가 0 이면 그 화면은 건너뜀(배치 대상에서도 빠진다) `:672-685` |
| R6 | 프리셋 밖 모니터: 온라인 순서대로, 시작 x = 계획 화면들의 (origin.x+폭) 최대(없으면 0), y = 그 우변에 닿는 **첫** 계획 화면의 origin.y(없으면 0), 폭만큼 누적 — 배치된 전체 묶음의 오른쪽 끝에 위를 맞춘다(design.md 결정 9; 구 규칙은 "마지막 계획 화면"). 계획 화면이 0 이면 "현재 주(원점≈0·미러 아님)"를 먼저(동률은 온라인 순서) `:715-741` |
| R7 | `mainDisplayID` = 앵커(첫 계획 화면) 마스터 ?? 첫 프리셋 밖 모니터 — 둘 다 원점 (0,0) `:759` |
| R8 | 미러 해제 = 현재 마스터가 있고 계획의 (슬레이브→마스터)와 다른 온라인 `:743-753` |
| R9 | 자동 집합: `autoMatch` 모니터가 1개 이상, 개수 == 온라인 개수, 전부 매칭. 선택 = 배열 **첫** 일치 `:169-173, 797-802` |
| R10 | `matchesCurrent` 허용 오차 0.5 `:827-829`. 이름 검증은 `String.count`(자소 단위) — Rust `chars()` 와 NFD·이모지에서 다르다, 허용 오차로 문서화 `:265`(§7 #32) |
| R11 | **겹침**: 계획 화면 쌍마다 사각형이 겹치면(변만 닿는 것은 허용) `layoutIssues` 에 "화면이 겹칩니다(역할 ↔ 역할)" 를 추가하고 `isApplicable = mainDisplayID != nil ∧ layoutIssues 없음`. `summary()` 는 "적용 불가: …" 와(앵커가 주가 아니면) "주 화면 모니터 없음 → 〈역할〉 이 주 디스플레이" 를 앞에 붙인다 `:404-408, 410-, 757, 770-794` |

**동치 확인 항목** — 이식 원본은 `Tests/UnitTests/DisplayPlannerTests.swift:123` `UnitTests.DisplayPlannerTests/gridLayoutsFollowVector(_:)`, `:154` `UnitTests.DisplayPlannerTests/connectedMainAnchorsAtZeroAndOverlapIsRejected()`, `:221` `UnitTests.DisplayPlannerTests/mirrorSuccessionMissingAndOutsideRulesArePreserved()`, `:306` `UnitTests.DisplayPlannerTests/gridValidationRejectsDuplicateAndDiagonalCells()`, `:351` `UnitTests.DisplayPlannerTests/automaticSelectionRequiresExactSetAndPreservesPriority()`, `:416` `UnitTests.DisplayPlannerTests/matchingUsesValidHardwareIdentityWithoutReuse()`다. 격자 배치 입력·기대값의 정본은 `fixtures/vectors/display-grid-layouts.json:1`이며 Windows `selftest.rs`도 이 JSON을 읽는다. 아래 표의 1~7은 정본 벡터의 설명용 요약이고, 어긋나면 JSON이 우선한다. 나머지는 새 Swift 시험의 입력·기대값을 옮긴다. 표는 같은 규칙을 d1~d3 고정 입력으로 정리한 보조 기대값이다. 고정 입력: d1=2560×1440 · d2=1920×1080 · d3=1280×720, 모두 external·vendor 1·model=serial=id·uuid `"U<id>"`·name `"M<id>"`. `p(주, 보조1@칸, 보조2@칸)`.

| # | 입력 | 기대 |
| --- | --- | --- |
| 1-3 | 격자A `p([d1], [d2]@(-1,0), [d3]@(1,0))` | 원점: 주 (0,0) · 보조1 (-1920,0) · 보조2 (2560,0) |
| 4 | 격자A 검증 | 오류 없음 |
| 5-6 | 격자B `p([d1], [d2]@(0,-1), [d3]@(0,1))` | 보조1 (0,-1080) · 보조2 (0,1440) |
| 7 | 격자C `p([d1], [d2]@(-1,0), [d3]@(0,1))` | 보조2 (0,1440) (주 아래 = 주 높이) |
| 8-9 | 주 `[d1,d2,d3]` | `mainDisplayID`=1 · mirrors = {2→1, 3→1} |
| 10-11 | `p([d1], [d2,d3]@(1,0))` | mirrors = {3→2} 만 · originsByMaster[2]=(2560,0), [1]=(0,0) |
| 12-13 | 위 프리셋, online=[d1,d3] | 보조1 마스터 id=3 · missing 이름 = ["M2"] |
| 14-15 | `p([d1])`, online=[d1,d2,d3] | outside id 집합={2,3} · 모두 origin.x ≥ 2560 (코드 도출: d2 (2560,0), d3 (4480,0)) |
| 16 | `p([d1], [d2]@(1,0), [d3]@(1,0))` | 검증 오류(같은 칸) |
| 17 | `p([d1], [d2]@(1,1))` | 검증 오류(대각선만 닿음) |
| 18 | `p([d1], [d2]@(1,0), [d3]@(1,1))` | 오류 없음 |
| 19-20 | a="수업"=주 `[d1✓,d2✓]`, b="수업2"=`p([d1✓],[d2✓]@(1,0))`, connected=[d1,d2] | 목록 [a,b] → "수업" · [b,a] → "수업2" |
| 21-23 | 같은 a,b 로 connected=[d1,d2,d3] / [d1] / 자동 체크 없는 프리셋만 | 모두 nil |
| 24 | 프리셋 d2 identity, 온라인 {id 9, uuid "NEW", vendor 1, model 2, serial 2} | 매칭 1건(② 단계) |
| 25 | 프리셋 {uuid "X", vendor 1, model 2, serial 0}, 온라인 {uuid "NEW", serial 0} | 매칭 0건(serial 0 폴백 없음) |
| 26 | 틈 결함 입력: 주 1920×1080, 보조1 2560×1080@(0,1), 보조2 1280×720@(1,0) | 보조2 (1920,0) — 주 오른쪽 변에 붙음(구 규칙은 x=2560 으로 640 틈) |
| 27 | 주 모니터 부재: 프리셋 주 = 미연결, 보조1 2560×1440@(-1,0), 보조2 1280×720@(1,0), online = 보조 둘 | `mainDisplayID` = 보조1 · 보조1 (0,0) · 보조2 (2560,0) (보조2 는 끊긴 화면 → 묶음 오른쪽 변) |
| 28 | 겹침: 주 1920×1080, 보조1 1280×1440@(1,0), 보조2 3000×720@(0,1) | `layoutIssues` 비어 있지 않음 · `isApplicable == false` |

**검증 공백**(기대값이 아직 없다): `matchesCurrent` · `DisplayPresetValidator`(개수·이름·중복 모니터) · `DisplayPresetCapture.current`(`closestConnectedCells` 의 `placementOrder` 순회·`<` 최소 갱신 `:941-993`). 이 영역의 시험 추가는 §7 #14(남은 영역) 승인 필요 — 승인 전에는 Rust 에서 같은 알고리즘·같은 순회 순서로 옮기고 공백으로 보고한다.

### 3.2 아이콘·에셋(Windows)

근거 표기: `에셋`=`assets/`. 에셋 수치는 전수 스캔(앱 SVG 57·보관 52·attr 47, `symbols/windows/png` 5크기×109, 트레이 ICO 4). macOS 쪽 아이콘 로더는 머지됐다(e702809, `AppIcon.swift` — 아래 §3.2.3 은 이 코드와 대조함).

#### 3.2.1 어느 파일을 쓰나

| 용도 | 파일 | 판정 |
| --- | --- | --- |
| 앱 UI 아이콘 57키 | `assets/icons/<키>.svg` | **사용.** exe 에 `include_bytes!` 임베드, D2D 가 SVG 를 직접 그린다 |
| 속성 레이어 47개 + 규칙 | `assets/symbols/attr/svg/*.svg` · `manifest.json`(+README) | **사용.** PNG 가 없다(PNG 109 = 앱 57 + 보관 52) |
| 트레이 | `assets/symbols/windows/tray/menubar.{on,off}.{light,dark}-taskbar.ico` | **사용**(§3.2.5) |
| `symbols/windows/png/<16·20·24·32·48>/` | — | **사용하지 않는다(보관).** ① 파일명이 디자인 이름(`tool.rect.png` ↔ 앱 키 `tool.rectangle`) ② 16 DIP 기준 5단계가 앱 표시 크기(10·13·14·17·18, 가로 팔레트 32칸 — `docs/icons.md`)와 안 맞는다 ③ 검정 고정(RGB 0,0,0)이라 마스크 합성이 필요하고 레이어 안 투명도(`fill-opacity .3` 등)가 사라진다 |
| `symbols/svg/` 52개 | `key.ctrl.svg` 하나만 | 수업 운영 아이콘 51개는 쓰는 화면이 생길 때 (미사용) |

#### 3.2.2 그리기 — D2D SVG 직접 그리기가 권장안

| 항목 | 규칙 |
| --- | --- |
| API | `ID2D1DeviceContext5::CreateSvgDocument(stream, viewport)` → `DrawSvgDocument`. Win10 1703+ 이므로 Win11 전용에 조건 없음 |
| 지원 범위 | 에셋이 쓰는 요소(svg·path·rect·circle·ellipse·**g**)·속성(`transform`·`fill-opacity`·`stroke-opacity`·`stroke-dasharray`·`fill-rule`·`stroke-*`)이 전부 D2D 지원 목록 안. **목록 밖 요소·속성은 조용히 무시**되므로 build.rs 가 임베드 SVG 에서 지원 요소(svg·g·path·rect·circle·ellipse·line·poly*·use·defs·clipPath·gradient·image) 밖의 요소(`text`·`filter`·`mask`·`<style>` 등)를 발견하면 빌드 오류(현재 0건) |
| 크기·DPI | 문서 viewport 는 24×24 고정, 그리는 쪽이 `SetTransform(scale = px/24)`. `px = round(DIP × dpi/96)`(100% 14 DIP→14, 125%→18, 150%→21, 200%→28, 300%→42). 아이콘 사각형 원점은 **물리 픽셀 정수**로 맞춘다. PNG 5단계 선택표는 필요 없다(벡터) |
| 색 입히기 | 에셋은 전부 `currentColor`(앱 SVG 57개 + attr 47개, 고정색 0). 루트 `<svg>` 에 `color` 속성을 넣는다(`GetRoot` → `SetAttributeValue(color)`). 레이어 안 투명도는 그대로 유지 = macOS 템플릿 틴트와 같은 위상 |
| 캐시 | 키 → 파싱한 `ID2D1SvgDocument`, 그릴 때마다 `color` 만 교체. `D2DERR_RECREATE_TARGET` 시 비운다 |
| 대안(비권장) | PNG 알파 마스크 + `FillOpacityMask` — §3.2.1 ③ 때문에 attr 는 못 쓰고 앱 아이콘만 쓰면 경로가 둘이 된다 |

[확인 필요: W1, Med] 루트 `color` → `currentColor` 전파는 공식 문서에 **지원 속성(`color`)·페인트 타입(`D2D1_SVG_PAINT_TYPE_CURRENT_COLOR`)** 만 있고 호출 예가 없다. 첫 빌드에서 57개 중 3개(선만·채움·반투명 레이어)로 1회 그려 본다. 안 되면 SVG 텍스트의 `currentColor` 를 `#rrggbb` 로 치환해 재파싱(캐시 키에 색 포함). 같은 시도에서 루트 `width/height=24` + `viewBox` + `SetTransform` 조합의 위치·클립도 본다.

#### 3.2.3 키로 찾기·폴백 — macOS `AppIcon.swift` 와 같은 우선순위 규칙

| 단계 | macOS(`AppIcon.swift`) | Windows |
| --- | --- | --- |
| 키 | `ShortcutDefinition.id`·그룹(`group.*`)·옵션(`option.*`)·`status.annotation.on/off` | 같다. 키 호출은 core 카탈로그(`shortcut.rs`)의 `id` 에서. 그 밖의 고정 키(그룹·옵션·상태)는 `app_icon.rs` 상수 — 값은 `assets/symbol-map.tsv` 2열과 같아야 한다 |
| 파일 | 번들 `Icons/<키>.svg` | `build.rs` 가 `assets/icons/*.svg` 를 스캔해 `(키, &[u8])` 표 생성(`include_bytes!`, 합계 ≈54KB). 외부 크레이트·설치 폴더 파일 없음 |
| 파일 없음 | SF Symbol(`iconName`) | **Segoe Fluent Icons 글리프**(아래). 모양이 SF 와 다르므로 "같은 폴백"이 아니라 **같은 우선순위 규칙**(디자인 SVG → 시스템 글리프) |
| 이름 점검 | 시작 시 `NSLog` 1회 | 임베드가 정적이므로 ① build.rs: 임베드 파일명이 **앱 키 집합**(`fixtures/shortcuts-default.json` id + 그룹·옵션·상태 상수, 없으면 `.selected` 짝)에 없으면 빌드 오류 — `symbol-map.tsv` 2열과의 대조는 import 스크립트가 만든 이름이라 항상 참이어서 무의미 ② 런타임: 키 집합 중 SVG 도 글리프도 없는 키는 시작 로그 1회 |
| 디코딩 실패 | `NSLog` 경고 + 폴백 | `CreateSvgDocument` 실패 → 로그 + 폴백(조용히 빈칸 금지) |
| 선택 상태 | `<키>.selected.svg` 우선(현재 0개) | 같다 |

- **대상 키는 4종뿐**: `annotation.pass-through`(U 통과 모드 — 카탈로그 정의 있음·SF `cursorarrow.click`, 디자인 SVG 없음. `docs/icons.md` 행은 macOS 쪽이 복구 예정)·`eraser.area-shape`·`palette.radial.toggle`(둘은 `symbol-map.tsv` 에 기존 디자인을 한 줄 더 연결하면 SVG 가 생겨 폴백 불요 — macOS 쪽 결정)·`color.1~6`(앱이 색 원을 직접 그림). SF Symbol 이름(`iconName`)은 core 에 이식하지 않는다. §7 #31.
- Segoe Fluent Icons 는 **Win11 내장**(Win10 에는 없음 — 최소 OS 가 11 이라 무관). 문서상 **다른 플랫폼 배포 불가·재배포 조건은 별도 확인** → 폰트를 번들하지 않고 시스템 폰트를 DirectWrite 로 그린다. 글리프 코드(예: Edit `E70F`, Cancel `E711`, Settings `E713`)는 폴백 키에만 `app_icon.rs` 표 한 곳에 둔다. `annotation.pass-through` 글리프는 [확인 필요: 글리프 표에서 터치·포인터 계열 선택].
- **OS 예외 하나**: macOS `keystroke-display.toggle` ← 디자인 `key.command`, Windows ← `symbols/svg/key.ctrl.svg`(map 에 없는 보관본). `import-symbols.sh` 는 OS 분기가 없으니 건드리지 않고, `windows/icon-os.tsv`(예정) 한 줄(`keystroke-display.toggle<TAB>key.ctrl`)을 build.rs 가 읽어 원본 파일을 임베드한다(사본 금지). `key.ctrl` 은 ⌃ 캐럿 키캡이라 Windows 사용자에게 "Ctrl" 글자만큼 읽히는지 [확인 필요: 사용자·W4a 실기 — 안 읽히면 글자 라벨로, §7 #30].
- 단축키 표기는 Windows 에서 글자(`Ctrl`·`Alt`·`Shift`·`Win`) — ⌘⌃⌥⇧ 기호는 쓰지 않는다(`symbols/README.md`).

#### 3.2.4 색·상태·테마 — 앱이 색을 입히는 방식 유지

| 항목 | macOS | Windows |
| --- | --- | --- |
| 앱 테마 | 시스템 의미색(자동) | `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize` 의 `AppsUseLightTheme`(0=어두움). 트레이는 `SystemUsesLightTheme`(§3.2.5) |
| 평상 / 보조 / 비활성 | `.labelColor` 계열 | WinUI 텍스트 토큰을 따른다 — 밝음 `#E4000000`·`#9E000000`·`#5C000000`, 어두움 `#FFFFFF`·`#C5FFFFFF`·`#5DFFFFFF` [확인 필요: 값은 WinUI 3 `Common_themeresources_any.xaml` 대조] |
| 선택 | `.controlAccentColor` 틴트 + 배경 0.28 | 시스템 강조색(`UISettings.GetColorValue(Accent)`; 변경은 `ColorValuesChanged`) 틴트 + 배경 α .28 |
| 호버·눌림 | — | 배경에 아이콘색 α 6%·10% 오버레이(제안, W4a 실기에서 조정 — §7 #30). 아이콘 자체는 안 바꾼다 |
| 고대비 | — | `SPI_GETHIGHCONTRAST`(`HCF_HIGHCONTRASTON`)를 시작 시·`WM_SYSCOLORCHANGE` 때 읽는다. 켜지면 **전 아이콘을 `COLOR_WINDOWTEXT`(선택=`COLOR_HIGHLIGHT`, 비활성=`COLOR_GRAYTEXT`) 단색**으로, 배경 이미지 생략, **attr 색·halo 끔**(공식 지침: 다색 이미지는 전경/배경 한 쌍으로). 펜 색은 색 칸이 따로 전달 |
| 표면 재질 | 원형=RGB(20,23,31) 단색 반투명 면 | Windows도 §3 "팔레트 면 렌더링"처럼 D2D 어두운 단색 반투명 면을 직접 그리고 블러하지 않는다. 속성 아이콘 배경색 인자는 원형=선택 시 강조색·그 밖 어두운 면 고정, 가로=선택 시 강조색·그 밖 시스템 모양 0.93/0.18(§3.2.6). 그 밖 창의 Mica/Acrylic(`DWMWA_SYSTEMBACKDROP_TYPE`, Win11 22H2+)은 이 절 범위 밖 |

#### 3.2.5 트레이 아이콘

| 항목 | 규칙 |
| --- | --- |
| 파일 ↔ 앱 키 | `menubar.on.*` = `status.annotation.on`(켜짐), `menubar.off.*` = `status.annotation.off`(꺼짐). 디자인 이름이라 `symbol-map.tsv`(`menubar.on→status.annotation.on`)로 이어진다 |
| 사양(실측) | 4파일 모두 16·20·24·32·48·64 px 6장, **PNG 압축** 32bpp. `light-taskbar` = 순검정(0,0,0), `dark-taskbar` = 순백(255,255,255) — 알파만 다르다 |
| 임베드 | `app.rc` 에 `ICON` 4개(ID 101~104), 앱 아이콘은 ID 1(§3.2.7). `build.rs` 의 `rc.exe` 경로에 합류 |
| 밝기 판정 | 위 `SystemUsesLightTheme`(1=밝은 작업 표시줄 → `light-taskbar`, 검정 아이콘). **앱 테마(`AppsUseLightTheme`)와 다를 수 있다**(작업 표시줄만 어둡게 설정 가능) |
| 변경 감지 | `WM_SETTINGCHANGE` 마다 레지스트리를 다시 읽고 **값이 바뀐 때만** 아이콘 교체. lParam `"ImmersiveColorSet"` 은 비공식이라 의존하지 않는다. 탐색기 재시작 `TaskbarCreated`(`RegisterWindowMessageW`) 때 `NIM_ADD` 재실행 |
| 크기·DPI | `px = GetSystemMetricsForDpi(SM_CXSMICON, 주 모니터 dpi)` → `LoadImageW(hinst, id, IMAGE_ICON, px, px, LR_DEFAULTCOLOR)`(정확 크기 항목 우선 — `LoadIconWithScaleDown` 은 16·32·48·256 표준 항목만 골라 축소하므로 쓰지 않는다). 100·125·150·200·300% = 16·20·24·32·48 로 ICO 항목과 정확히 맞고, 175%(28)·250%(40)는 가장 가까운 항목을 맞춰 변환(품질 W1 확인). PMv2 앱은 `GetSystemMetrics` 가 시스템 DPI 고정이라 `ForDpi` 를 쓴다(공식 지침). `WM_DPICHANGED`·`WM_DISPLAYCHANGE` 때 다시 로드 |
| 상태 전환 | 판서 모드 켜짐/꺼짐 → `NIM_MODIFY`(`NIF_ICON\|NIF_TIP`). 툴팁 = "판서 모드 켜짐/꺼짐"(macOS `description` 과 동일 문구) |
| 등록 | `NIM_ADD` 직후 `NIM_SETVERSION`(`NOTIFYICON_VERSION_4`). **`NIF_GUID` 는 쓰지 않는다** — GUID 등록은 exe 경로(와 서명)에 묶여 이동 시 설정이 사라진다(공식 문서) → `hWnd+uID` 로 식별 |
| 고대비 | 작업 표시줄 밝기를 레지스트리가 못 맞출 수 있다 [확인 필요: W1 실기 — 고대비 테마에서 아이콘 가시성] |

[확인 필요: W1, Med] PNG 압축 ICO 를 `rc.exe` 가 받아들이는지, 16·20·24 항목이 실제로 선택되는지. 안 되면 BMP(DIB) 항목으로 재생성(macOS 쪽 Pillow 등). 신규 트레이 아이콘이 Win11 에서 기본 숨김(오버플로)일 수 있어 첫 실행 안내 필요 [확인 필요: W1 실기].

#### 3.2.6 속성 아이콘(attr 레이어) — 순수 로직은 core, 그리기는 app

macOS 구현은 #43으로, `AttrIconPure` 단언 3개는 #45로 머지됐다. 규칙 정본 = `assets/symbols/attr/README.md`·`manifest.json`과 `AttrIcon.swift`(`AI`)의 확정 해석. Windows 는 같은 입력 → 같은 레이어 목록으로 맞춘다.

| 경계 | 내용 |
| --- | --- |
| core `attr_icon.rs` | 입력: `tool`(디자인 이름)·`attrs`(color RGBA·size·size 범위·heads·fill·trail·eraser 모드)·`ui_color`·`bg_color`. `manifest.json` 을 `include_str!`+`serde_json` 으로 **읽는다**(도구·레이어 하드코딩 금지). `{width}`→1~4, `{heads}`→single/double 치환 → 조건 평가 → 명암 판정. 출력 `Vec<Layer{name, paint: Ui\|Attr, halo: Option<Rgba>}>` = 그리는 순서(아래→위). OS 의존 0. Swift 순수 원본 `AI:26-121`, 도구 카탈로그·입력 구성 `AI:161-173,243-388`, 평가 `AI:420-516`; 두께 단계·조건 문법·명암/halo의 정본은 `fixtures/vectors/attr-icon-width-levels.json:1`·`attr-icon-conditions.json:1`·`attr-icon-contrast.json:1`이다. `Tests/UnitTests/AttrIconTests.swift:120` `UnitTests.AttrIconTests/widthLevelsFollowVector(_:)`, `:134` `UnitTests.AttrIconTests/conditionsFollowVector(_:)`, `:158` `UnitTests.AttrIconTests/contrastFollowsVector(_:)`와 Windows `selftest.rs`가 이 JSON을 읽는다 |
| app `app_icon.rs` | `name` → 임베드 `attr/svg/<name>.svg` 문서 → `paint` 색(ui=§3.2.4 상태색, attr=도구 색)을 루트 `color` 로 → 그림. halo 가 있으면 **같은 문서를 halo 색으로 ±0.5 DIP 4방향 어긋나게 먼저** 그린다(`AI:519-550`; D2D SVG 에서 굵은 Stroke 로 덮어쓰기는 채움 레이어에서 어렵다). "1pt" = 1 DIP |
| 쓰는 곳 | 원형 중심 18·2링 14·가로 팔레트 도구 버튼. 설정·호버·선택·통과·외관이 바뀌면 캐시 키를 달리하거나 비운다(`RG:342-425,737-825`; `AppIcon.swift:71-110,254-264`) |

| 규칙 | 정의·함정 |
| --- | --- |
| 도구 id | manifest `tools` 키는 **디자인 이름**(`tool.rect`·`tool.circle`), 앱 키는 `tool.rectangle`·`tool.ellipse`. Swift 대응 정본은 `AttrIconToolCatalog.manifestKeyByTool` 한 곳(`AI:161-173`). Windows도 같은 대응을 한 곳에서만 만들고 레이어 규칙은 manifest에서 읽는다 |
| 조건 문법 | layers 행의 3번째 칸에서 읽는다: `fill`·`trail`, `!x`, `mode=v`, `mode!=v`. manifest 의 `conditions` 사전은 4개뿐인데 layers 는 `mode=rect`·`mode=lasso`·`mode!=brush` 까지 7종을 쓴다 → **사전을 정본으로 쓰지 않는다**(검증: 모든 layer 이름이 파일로 존재, 미사용 파일 0) |
| 두께 단계 | `t=clamp((size−min)/(max−min),0,1)`, `level=1+min(3,floor(4t))`; `max==min`이면 2(`AI:32-36`). 범위는 원형 3링 배열, 지우개는 설정 프리셋 최솟값·최댓값(`AI:288-388`) |
| 지우개 모드 | Swift `EraserMode{partial,stroke,area}` + `EraserAreaShape{rectangle,lasso}`를 partial→`brush`, stroke→`stroke`, area+rectangle→`rect`, area+lasso→`lasso`로 넘긴다(`AI:355-373`). `stroke`는 `mode!=brush` 레이어만 남는 것이 현행 정본 |
| 명암·배경 추정 | 상대휘도·명암비는 `AI:85-111`; `ratio<1.8`이면 attr 레이어에 halo, 단 `*.fill` 제외(`AI:510-516`). halo 색은 배경 휘도 0.5 이상이면 검정 55%, 아니면 흰색 70%(`AI:4-10,641-646`). **원형 팔레트**는 선택 시 강조색, 그 밖은 어두운 면 고정(`RadialPaletteArcStyle.estimatedBackdropColor`, `S:132-140`); **가로 팔레트**는 선택 시 강조색, 그 밖은 시스템 모양에 따라 흰색 0.93/0.18(`AppIcon.swift:4-7,254-264`). 원형 아이콘색은 비활성 `#9ca3af`, 활성 흰색(`S:103-109,142-147`) |
| 고대비 | attr·halo 끔(§3.2.4) |

#### 3.2.7 설치·배포 자산

| 자산 | 현황 | Windows 처리 |
| --- | --- | --- |
| 앱 아이콘 | **에셋 없음.** macOS 는 `build.sh:94~215` 의 Swift 코드가 그려 `.icns` 생성(파란→보라 그라디언트 둥근 사각형+보드+연필) | macOS 쪽이 `assets/app-icon/`(예정)에 PNG(16·20·24·32·40·48·64·256 px)와 묶은 `.ico` 를 내보낸다(§5.4, §7 #29). 그 전엔 트레이 `menubar.on.light-taskbar.ico` 임시 사용 |
| `.pansetting` 아이콘 | macOS 도 문서 전용 아이콘 없음(`build.sh:305~340`, `CFBundleTypeIconFile` 없음) | 같게 앱 아이콘 재사용: `HKCU\Software\Classes\EduTools.Settings\DefaultIcon` = `"<exe>",0`(0 = 첫 `RT_GROUP_ICON`. `,-1` 은 `ExtractIcon` 계열에서 개수 조회 의미라 쓰지 않는다), 등록 후 `SHChangeNotify(SHCNE_ASSOCCHANGED)` |
| 설치기 아이콘 | — | 같은 `.ico`. 설치기 도구 확정 후 연결(§7 #6) |
| 리소스 ID | — | 1=앱, 101~104=트레이(`on-light`·`on-dark`·`off-light`·`off-dark`). 앱 아이콘이 최저 ID [확인 필요: 탐색기가 최저 ID 를 쓰는지 W9] |
| 빌드 경로 | — | `build.rs` 가 `../../../assets` 를 읽고 `cargo:rerun-if-changed` 로 `assets/icons`·`symbols/attr`·`symbols/windows/tray`·`symbol-map.tsv`·`windows/icon-os.tsv` 등록 |
| 에셋 갱신 | 디자이너 재내보내기 → `assets/import-symbols.sh <폴더>`(bash) | **갱신은 macOS 쪽 작업** — Windows 는 `assets/` 읽기 전용 소비자. Windows PC 에서 필요하면 Git for Windows 의 Git Bash 로 같은 스크립트(`cp -R`·`mktemp`·`grep -qxF`·`find -delete` 사용, [확인 필요: 1회 시험]). **PowerShell 이식 금지**(정본이 둘). CRLF 가 섞이면 bash 가 깨지므로 `core.autocrlf false`(§5.1) 유지 |

## 4. 저장소 배치 — 같은 저장소

`github.com/datasci-lee/eduTools`(공개) 에 추가. 별도 저장소면 픽스처·design.md 가 사본 둘이 된다. 아래 중 `windows/`·`fixtures/`·`.github/` 와 `assets/app-icon/` 은 **아직 없다(예정)**.

```
windows/
  Cargo.toml                  # [workspace] members = ["crates/*"], Cargo.lock 커밋
  icon-os.tsv                 # OS 별 아이콘 예외(keystroke-display.toggle → key.ctrl) 한 줄
  crates/edutools-core/       # OS 의존 0 (lib)
  crates/edutools/            # Windows 앱 (bin), build.rs·app.rc·app.manifest (앱 아이콘 .ico 는 macOS 쪽이 assets/app-icon/ 로 제공)
fixtures/pansetting/          # macos-v1.pansetting, windows-v1.pansetting
fixtures/shortcuts-default.json
.github/workflows/windows.yml
assets/                       # 읽기 전용 입력(앱 아이콘 SVG·트레이 ICO·속성 아이콘, §3.2) — 사본 금지
```

## 5. Windows PC 개발 환경

### 5.1 설치

| 항목 | 버전·선택 | 비고 |
| --- | --- | --- |
| Windows | Parallels "Windows 11" ARM64 VM(24H2 이상 권장) | x64 에뮬레이션으로 `x86_64-pc-windows-msvc` 앱 실행 |
| Visual Studio Build Tools | 2022(17.x) 이상, 워크로드 "C++를 사용한 데스크톱 개발" | MSVC v143 링커·`rc.exe`·`mt.exe` |
| Windows SDK | 10.0.26100 (Windows 11 24H2 SDK) | Build Tools 설치 시 함께 선택. `signtool`·`makeappx` 포함 |
| Rust | rustup → `stable-x86_64-pc-windows-msvc`, rustc 1.95 이상(macOS 쪽 실측 1.95.0) | ARM64 빌드는 만들지 않음 |
| Git | Git for Windows + `gh` | `git config core.autocrlf false`(Swift 원본 줄바꿈 보존) |
| 에디터 | VS Code + `rust-lang.rust-analyzer` · `ms-vscode.cpptools`(MSVC 디버거) · `tamasfe.even-better-toml` | 대안: RustRover |
| 진단 도구(선택) | Spy++(Build Tools 포함 안 됨 → VS 설치 시), Accessibility Insights, WinDbg | 창 스타일·z순서·메시지 확인 |
| 실기 장비 | 펜(Wacom + 드라이버 "Windows Ink 사용" 체크, 또는 Surface 펜), 배율 다른 모니터 2대(예 100%·150%), 프로젝터 복제 | §6 실기 항목 |
| 시험 앱 | PowerPoint, Chrome(YouTube Music 로그인), Teams 또는 Zoom, 한글 IME | |

- core 개발·시험은 맥에서 `cd windows && cargo test -p edutools-core`로 실행한다.

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
- Windows 앱 실행 시험은 Parallels "Windows 11"(ARM64, x64 에뮬레이션) VM에서 `x86_64-pc-windows-msvc` 빌드로 한다.

### 5.3 로컬 게이트

**GitHub Actions를 쓰지 않는다(사용자 지시).** 로컬 게이트는 맥의 `cargo test -p edutools-core`와 Parallels VM의 `x86_64-pc-windows-msvc` 빌드·`edutools.exe --selftest`다.

### 5.4 macOS 쪽에서 받아야 할 것

| 항목 | 누가 | 언제 |
| --- | --- | --- |
| `fixtures/pansetting/macos-v1.pansetting`(**현재 main 으로 내보낸 기본값** — §2.2) + `Tests/UnitTests/SettingsTests.swift:56` `UnitTests.SettingsTests/exportIncludesPresetsAndExcludesComputerSpecificValues()`의 픽스처 입력 | macOS 쪽 | 완료(#99) |
| `fixtures/shortcuts-default.json`(§2.2 형식, 정의 58개, `M`·`annotation.pass-through\|annotation`=`U`, 기존 6개 + `display.preset.1…8` = unbound 14개). 슬롯 override 케이스는 W2 `shortcut.rs`에서 후속 | macOS 쪽(카탈로그에서 생성) | 완료(#99) |
| `docs/design.md` 낡은 문장 3곳(:14 "Windows 10/11", :262 미디어 키 일시정지, :368 Fluent 아이콘)을 확정 결정(#1·#7·아이콘 세트)에 맞게 수정 | macOS 쪽 | W1 착수 전(§7 #19) |
| 앱 아이콘 PNG(16·20·24·32·40·48·64·256) + 묶은 `.ico` → `assets/app-icon/`(지금은 `build.sh` 가 코드로만 그림) | macOS 쪽 | W1 착수 전(없으면 임시 아이콘) |
| `symbol-map.tsv` 에 `eraser.area-shape`·`palette.radial.toggle` 연결 또는 폴백 확정, `annotation.pass-through` 디자인·`docs/icons.md` 행 복구 | macOS 쪽 | W4a 착수 전 |
| 원형 팔레트 각도·구역 판정 | 요청 없음 — Windows 가 `RadialPaletteSectorGeometry`·`Metrics` 를 읽고 나머지는 재구성(§2.3) | — |
| §7 #14 남은 영역 승인 시: 팔레트 기하·도구별 기억 이행의 시험 입력을 현 T1/T3 시험과 교차 플랫폼 JSON 벡터에 추가. `nextRadialPaletteFocus`는 `Tests/UnitTests/RadialPaletteNavigationTests.swift:46` `UnitTests.RadialPaletteNavigationTests/navigationMatchesOriginalTable(_:)`, 즐겨찾기 배치·히트는 `Tests/GUITests/RadialPaletteTests.swift:12` `GUITests.RadialPaletteTests/maximumFavoriteCellsDoNotOverlapAndHitTest()`, `DisplayPlanner` 격자 배치는 `fixtures/vectors/display-grid-layouts.json:1`로 완료 | macOS 쪽 | 해당 단계(W2·W4b) 착수 전 |
| 속성 아이콘 구현은 #43으로 완료. 두께 단계·조건 문법·명암 판정은 `Tests/UnitTests/AttrIconTests.swift:120` `UnitTests.AttrIconTests/widthLevelsFollowVector(_:)`, `:134` `UnitTests.AttrIconTests/conditionsFollowVector(_:)`, `:158` `UnitTests.AttrIconTests/contrastFollowsVector(_:)`과 `fixtures/vectors/attr-icon-*.json` 3개로 완료; Windows도 같은 JSON을 읽는다 | macOS 쪽 | 완료 |
| `ROADMAP.md`·`README.md` 에 021~039 반영 | macOS 쪽 | 착수 조건 아님(Windows 쪽은 `git log` 로 대체) |

## 6. 단계 로드맵

W 번호 유지(§3·§9 의 W1~W9 참조가 그대로 유효). 새 단계는 W4 분할(a/b)과 W10·W11 뿐. "대응 macOS" 의 번호는 `git log` 제목의 `eduTools-NNN`.

| 단계 | 대응 macOS | 산출물 | 검증 | 규모(Rust 줄) |
| --- | --- | --- | --- | --- |
| **W1 골격** | 001·002 일부·030·032 | workspace·CI, 트레이 아이콘·메뉴(종료) + **트레이 ICO 4종 임베드·밝기 판정·`TaskbarCreated`**(§3.2.5), 모니터별 투명 오버레이(DComp), `Ctrl+Alt+P` 판서 모드 토글(클릭 통과 on/off), WM_POINTER 로 압력 있는 선 긋기, PerMonitorV2, 단일 인스턴스 + **`monitor.rs`(식별·활성 경로·좌표 정본 — 오버레이를 만들 때부터 이 경로만)·`z_bands.rs` 5대역·`run_above_overlay` 앵커**(§3) + **`rc.exe` PNG-ICO 수용 확인·SVG 시험 1회**(루트 `color`·`SetTransform`, §3.2.2) | CI 초록. 실기: ① 두 모니터에 선 ② 판서 끄면 아래 앱 클릭 ③ 펜 압력 ④ **PowerPoint 슬라이드쇼 위 표시** ⑤ **오버레이에 WDA 걸면 Teams/Zoom 공유에서 빠지는지** ⑥ **WGC 모니터 캡처 노란 테두리를 비패키지로 끌 수 있는지** ⑦ 배율 다른 3화면(노트북·외부·빔)에서 창 위치·크기, 생성 직후 `GetWindowRect == rcMonitor` ⑧ 판서 켠 채 `MessageBox`·`IFileDialog` 가 오버레이 위에 뜨고 클릭됨 ⑨ 새 가상 데스크톱에서 오버레이 보임 여부 ⑩ 작업 표시줄 밝기 전환 3회에 트레이 아이콘 교체 | 1.7k |
| W2 판서 엔진 | 002·022·027·028·033·036·045·053·055·082·084 | core `ink`·`settings`(스키마 전체: `pressureMinimumWidthRatio`·`perToolMemory`·`favorites`·`display`·`paletteStyle`·`paletteGlassOpacity`·옛 키 이행·`SettingsStoreIssue`)·`shortcut`(정의 58개, unbound 14개, 기본 키 없는 슬롯 override)·`state`(도구별 설정 기억·즐겨찾기 스냅샷/적용·숫자키·`[ ]`·지우개 이전 도구 비영속 기억)·`laser.rs`(꼬리 점 버퍼·줄이기·이차 곡선 경로)·`display_preset.rs` 의 모델·serde·검증기(`display` 묶음 왕복용) + .pansetting/단축키 픽스처, 펜(필압 최소 굵기 공식)/형광펜/레이저(곡선 꼬리·누를 때만·범위 이탈 시 끊기)·Catmull-Rom·Undo 100, LL 훅 모드 키(P H L 1~6 [ ] Ctrl+Z), **통과 모드 `U`**(키 홀드 + 팔레트 시간제 — `PassThroughModeSource` 상태기계·`set_click_through`·`WH_KEYBOARD_LL` keyUp·`WH_MOUSE_LL` 안전망, `mousehook.rs` 를 여기서 처음 만들고 W8 이 재사용, §3) | `cargo test`(이식 selftest 중 해당분), 실기: 필압 최소 비율 0.05·0.2·1 · 한글 IME 상태에서 P · 기본 키 없는 전역/모드 슬롯 등록·해제 · 레이저 호버=점만/누르면 곡선 꼬리/펜 범위 이탈 시 꼬리 끊김 · 숫자키·`[ ]` 가 도구별로 따로 기억 · `U` 를 누르는 동안 아래 앱 클릭·스크롤이 전달되고 키를 떼면 판서로 복귀 · 훅이 keyUp 을 놓친 경우 마지막 마우스 입력 3초·최대 30초 안전망 · 팔레트 시간제: 브라우저 링크 클릭 후 3초 복귀·첫 입력 없이 10초 복귀·`Esc` · 관리자 앱 위(UIPI) | 3.8k |
| W3 지우개 | 003·045·052 | core `eraser`, 부분/획/영역, 펜 지우개 끝, 마우스 우클릭·펜 측면 버튼 임시 지우개, 버튼/포인터 소유권 가드, 장치 종류를 해당 `WM_POINTER` 이벤트에서만 읽는 가드, 전체/모니터 지우기 | 이식 selftest, 실기: 모든 도구에서 우클릭·측면 버튼·지우개 끝, 진입·이탈 뒤 화면 구성 변경, 다른 버튼 동시 입력 무시, up 유실 자가 복구, Undo 1단위 | 1.0k |
| **W4a 가로 팔레트·도움말·아이콘** | 004·037·038·039 | 가로 팔레트·Esc 단계·Tab, 도움말(카탈로그 생성, DirectWrite, 검색 EDIT), `app_icon.rs`(키 조회·SVG 캐시·폴백 글리프·상태색·고대비, §3.2.2~4) | 실기, 아이콘 없는 키 폴백, 100% 모니터·빔에서 10~14 DIP 아이콘 가독 | 1.6k |
| **W4b 원형 팔레트** | 021·029·031·035·043·044·047·048·055·087~094 | core `radial`(그룹 표·기하·히트·키보드 탐색·즐겨찾기 스냅샷/적용, §2.3) + 중심 + 1·2·3링·펼침/접기/이동·`M`·방향키·Ctrl 길게 키캡·힌트 창·즐겨찾기 동적 칸/삭제·접근성(투명 효과 끔·고대비)·D2D 어두운 단색 반투명 면(`palette_render.rs`, 저장 0.55...0.90·UI/렌더 0.75...0.90·기본 0.86) + `attr_icon.rs` + halo 그리기(§3.2.6) | 실기: ① 모든 링 클릭·호버·힌트 ② 즐겨찾기 저장·불러오기·우클릭 삭제 ③ 100%·150% 모니터 간 이동·가장자리 펼침 ④ ↑↓←→·Enter 이동, 텍스트 입력 중 안 가로챔 ⑤ 밝은·어두운 배경·슬라이드쇼 위에서 어두운 면 대비(0.75/0.86/0.90)와 속성 halo ⑥ 투명 효과 끔·고대비·다크 ⑦ 지우개 3칸·영역 재클릭·중앙 우클릭 ⑧ 펜·키보드 조작 | 2.3k |
| W5 보드·타이머 | 005·006 | 화이트·블랙보드 페이지·패턴, 수업 배지(드래그·스냅·WDA·위치 기억), 쉬는 시간 전체 화면, 잠자기 복귀 | core timer selftest, 실기 | 2.0k |
| W6 쉬는 시간 음악 | 007 | `chrome.exe` 열기·SMTC 일시정지·볼륨 페이드·복원 | 실기(Chrome·Premium) | 0.8k |
| W7 줌 | 008·049·050·084 | WGC(또는 W1 판정에 따라 DDA) 라이브·정지 줌, 따라가기 3방식, Space 미리보기(회색 효과), 배율 표시 + 캡처에 커서 미포함·D2D 실제 커서·시스템 커서 숨김 재설정·`SetCursorPos` 전환 보존 + 라이브 판서(identity/역변환·제스처 잠금·줌→판서 키 라우팅·텍스트 포커스 유지) | core zoom selftest, 실기 60fps·하이브리드 GPU·커서 지연/모양/숨김/이중 커서 없음/전환 보존·커서-획 일치·텍스트 입력창 위치·제스처 잠금·키보드 | 2.4k |
| W8 부가 | 009~011·086 | 도형·텍스트(EDIT+IME)·올가미, 펜 도형 자동 보정(입력 중 원본 유지, 0.5초 멈춤 뒤 후보만 계산, 놓을 때만 교체; `IV:934-1005`), 스포트라이트·커서 강조(W2 `mousehook.rs` 재사용)·키 표시·가리개, 내보내기(PNG·PDF·클립보드)·자동 보관·화면 사라짐/재연결 복원(§3) | 이식 selftest, 실기: 0.5초 멈춰도 누르는 동안 원본이며 놓는 순간만 보정 | 3.0k |
| W9 설정·배포 | 012·013·022·082·094 | Win32 설정 창(탭 7 — 화면 탭은 W10; 필압 최소 굵기 0.05...1·원형 면 불투명도 UI 0.75...0.90, 단축키 기록은 커스텀 버튼)·.pansetting 가져오기/내보내기·파일 연결·백업 5개·로드 실패 알림·자동 실행·아이콘(앱 `.ico`·`.pansetting` DefaultIcon·설치기 아이콘, §3.2.7)·서명·설치기·자동 업데이트·에셋 갱신 절차 1회 시연 | macOS 에서 내보낸 파일 실물 왕복, 서명 검증, 현재 `Ctrl+Alt+P` 를 같은 키로 재기록(§9) | 3.0k |
| **W10 화면 구성 프리셋(1단계)** | 034 | core `display_preset` 의 격자 배치·매처·플래너·캡처(모델·serde·검증기는 W2) + `monitor.rs` 의 `OnlineDisplay` 조립(§3.1.2) + "화면" 탭(편집·현재 구성 캡처·계획 보기, **적용 없음**) + `.pansetting` display 묶음 가져오기 | 이식 selftest(§2.2 의 화면 프리셋 단언 6개). 실기: 노트북+외부+빔으로 캡처 → 계획 글, macOS 에서 내보낸 프리셋을 가져와 "연결 안 됨" 표시·모니터 다시 지정 | 2.5k |
| W11(예약) 프리셋 적용·자동 전환 | 054·3단계 | macOS 2단계 설계·구현(수동 15초 복구) 실기 뒤 착수: 순수 연산 목록 + `display_apply.rs` 상태 기계(§3.1.3), 15초 확인 창, 비정상 종료 감시 안전장치(#36), 변경 감지·5초 안정·30초 쿨다운(§3.1.4), 판서 보관 연동. 선행: §7 #15·#25~#27·#36 | 실기: ① 내장+외부+빔(배율 상이) 확장 배치 적용 ② 확인 창 15초 무입력 → 자동 복구 ③ 복제(슬레이브) 적용 ④ 적용 중 케이블 분리 → 복구 ⑤ 확인 중 앱 강제 종료 → 감시 복구 ⑥ 적용 뒤 Win+P ⑦ Miracast 연결·해제 ⑧ RDP 에서 비활성 ⑨ 겹침 배치는 적용 불가 사유 표시(`isApplicable == false`) | 미정(추정 1.6k) |

macOS 쪽 선행 작업은 §5.4.

**규모 근거**: `wc -l Sources/EduTools/*.swift` = Swift 24,624줄, `rg --files Sources/EduTools -g '*.swift'` = 38파일(f34a0bc). 초기 15,113줄 대비 +9,511(+63%). #50~#53의 소스 순증은 1,733줄(삽입 1,861·삭제 128, `git diff --numstat 2bf5d8a f34a0bc -- Sources/EduTools`): `DisplayApplier.swift` 776·`PointerEvents.swift` 27이 새로 생기고, 즐겨찾기·단축키 슬롯·배선이 기존 파일에 추가됐다. 인계서와 같은 거친 가정(Swift→Rust ×1.15, Win32·DComp 보일러플레이트 포함)을 그대로 쓰면 ≈28.3k이며 위 단계 추정 합은 약 24.5k로 3.8k 낮다. **비례하지 않는 세 곳**: ① 원형 면 SwiftUI는 이식이 아니라 D2D 알파 면으로 대체 ② SwiftUI 설정 2파일은 Win32 Common Controls로 다시 구현 ③ AppKit CG 적용 776줄은 `SetDisplayConfig`와 감시 안전장치로 구조가 달라진다. 추정치는 W2와 W11 끝에서 실측으로 보정한다.

## 7. 사용자에게 물을 결정

✅ = 확정(날짜), 🔶 = 미결(권장안; #4·#14 는 부분 결정 포함). 번호는 §2~§9 의 `#N` 참조와 같다.

| # | 결정 | 상태·권장 |
| --- | --- | --- |
| 1 | 최소 Windows: 11 만 / 10 22H2 포함 | ✅ **결정(2026-10-28): Windows 11 부터.** 매니페스트 `supportedOS` 와 설치기에서 Win11 미만 거부 |
| 2 | CPU: x64 만 / x64+ARM64 | ✅ **결정(2026-10-30): x64 만.** ARM64 빌드는 제외한다. 개발 VM은 Parallels "Windows 11" ARM64이고 x64 에뮬레이션으로 실행한다 |
| 3 | 실기 장비: 펜 기종·모니터 2대·프로젝터 | ✅ **결정(2026-10-28): 펜 + 노트북 내장 화면·외부 모니터·빔 프로젝터(배율이 서로 다름).** 펜은 삼성 갤럭시 탭(S펜) 검토 중 — Android 기기라 Windows 펜 입력이 되려면 화면 확장 앱(예: Samsung Second Screen, SuperDisplay)이 Windows Ink 펜으로 압력을 넘겨야 한다 [확인 필요: W1 ③ 에서 `WM_POINTER` 펜 타입·압력 수신 여부] |
| 4 | Windows 쪽 개발 주체(사람 / AI 도구)와 PR 검수자 | 🔶 **부분 결정(2026-10-28): 개발은 별도 Windows PC 에서.** 주체·PR 검수자·macOS 쪽 검수 연결 여부는 미정 |
| 5 | 서명 인증서: OV/EV·보관 방식(토큰/클라우드 HSM)·명의 | ✅ **결정(2026-10-28): 셀프 서명으로 시작**, 실제 배포 시점에 공인 인증서로 교체. 셀프 서명은 인증서를 설치한 PC 에서만 경고가 사라진다(신뢰 루트·신뢰 게시자에 설치). 교체는 서명 단계만 바꾼다 |
| 6 | 배포 형식: MSIX(+자동 업데이트·테두리 capability) / exe 설치기(+WinSparkle) | ✅ **결정(2026-10-28): 일반 설치 파일(exe).** 자동 실행 = `HKCU\…\Run`, 파일 연결 = `HKCU\Software\Classes`, 자동 업데이트 = WinSparkle(또는 자체 확인). 비패키지라 `graphicsCaptureWithoutBorder` capability 를 선언할 수 없을 수 있다 → W1 ⑥ 에서 WGC 테두리를 못 끄면 줌은 DDA 로 |
| 7 | 음악 종료: Chrome 새 창 닫기 / SMTC 일시정지 / 확장 프로그램 | ✅ **결정(2026-10-28): SMTC 일시정지만.** 열기는 `chrome.exe <URL>`, 종료는 `GlobalSystemMediaTransportControlsSessionManager` 에서 Chrome 세션을 찾아 `TryPauseAsync`(토글이 아닌 명시적 일시정지). 탭·창은 닫지 않는다. Chrome 세션이 없으면 로그 + 알림(토글 미디어 키로 대체 금지 — 이미 멈춘 음악을 다시 튼다) |
| 8 | 볼륨 페이드 범위: 시스템 전체 / Chrome 만 | ✅ **결정(2026-10-28): 시스템 전체**(macOS 와 같음, `IAudioEndpointVolume`) |
| 9 | 줌 중 화상회의 공유에서 판서 숨김 허용 | 🔶 허용(아니면 Magnification API 경로 추가) |
| 10 | 계획서를 저장소 `docs/windows-plan.md` 로 올릴지 | ✅ 올림(2026-10-28, 이 파일) |
| 11 | iPad 동반 앱(설계서 언급) 범위 | 🔶 이번 범위 밖 |
| 12 | macOS 에서 확정·구현된 기능(원형 팔레트·`M`·키보드·즐겨찾기, 통과 모드 `U`, 도구별 설정 기억, 화면 구성 프리셋 1·2단계, 아이콘 세트, 대화상자 높이 계층)의 Windows 포함 범위 | 🔶 전부 포함(W2·W4a/b·W10·W11). 원형 면만 #13. 화면 프리셋을 좁게 가는 대안: `display` 모델·검증·왕복·보존까지만(가져온 프리셋이 안 사라지게) 하고 편집 UI·계획·캡처·적용·자동 전환은 제외 — 식별 값(`CGDisplay*`)이 OS 별이고 적용은 실기 위험이 크다는 이유 |
| 13 | 원형 팔레트 면 대응 수준 | 🔶 D2D로 RGB(20,23,31) 단색 반투명 면을 직접 그리고 블러하지 않는다(권장). `tools.paletteGlassOpacity` 저장 0.55...0.90·UI/렌더 0.75...0.90·기본 0.86, 바깥 테두리 흰색 α0.08, 호버 흰색 α0.07, 비활성 트랙 `#64748b` α0.6, 활성 `#38bdf8`+글로우, 아이콘 비활성 `#9ca3af`/활성 흰색. 1·2·3링 구조와 속성 아이콘 어두운 배경 추정은 §2.3·§3을 따른다. 흰 배경 위 최악 렌더 α0.75에서 흰 내용 대비 최저 보장은 7.94:1 |
| 14 | 동치 장치가 0 인 남은 영역(원형 팔레트 기하·힌트, 도구별 기억 이행, `DisplayPresetCapture`·`DisplayPresetValidator`·`matchesCurrent`)의 시험 입력과 Windows 시험 체계 범위 | 🔶 **[결정 필요] Windows 쪽은 새 시험 체계를 만들지 않는다.** 유지 여부는 미정. macOS는 `docs/testing/strategy.md`의 시험 체계를 이미 갖췄다. `nextRadialPaletteFocus` T1과 즐겨찾기 배치·히트 T3가 생겼고, `DisplayPlanner` 격자 배치와 속성 아이콘 3군은 §2.2의 JSON 벡터 4개를 정본으로 쓴다. 매칭·격자 검증·자동 집합과 설정 골든도 새 시험 ID가 있다. 남은 영역은 승인 권장 — 현 시험에 입력을 더하고 교차 플랫폼 대상은 `fixtures/` JSON을 양쪽이 같이 읽게. 아니면 Windows는 같은 알고리즘·같은 순회 순서로 이식하고 W4b·W10 실기로만 판정, 공백은 이슈로 보고 |
| 15 | 화면 프리셋 2·3단계(적용·자동 전환)의 Windows 시점 | 🔶 macOS 2단계 구현 완료, 실기 확인 뒤 W11. 그전엔 W10(계획 보기까지). W11 착수 전 #25~#27·#36 확정 필요 |
| 16 | PR #35(통과 모드) 머지를 W2 착수 조건으로 | ✅ **해소(2026-10-28): 머지됨(c42fcc0).** 사용자 확인 "U 확인했어, 머지하고 진행해". 통과 모드(`U` 를 누르는 동안만 + 팔레트 시간제)는 현행 정본이다(§3). 038 아이콘 로더도 e702809 로 머지됨 |
| 17 | `fixtures/pansetting/windows-v1.pansetting` 을 "편집 범위" 에 넣을지 | 🔶 넣는다(§2.2·§4 가 이미 요구). 나머지 `fixtures/` 는 macOS 쪽 소유 |
| 18 | Swift 로직 PR 이 Rust 대응을 못 넣을 때 남기는 곳 — `tasks/park-*` 는 이 저장소에 없다 | 🔶 GitHub 이슈(라벨 `windows-sync`) |
| 19 | `docs/design.md` 낡은 문장 3곳(:14 "Windows 10/11", :262 미디어 키 일시정지, :368 Fluent 아이콘)을 macOS 쪽이 확정 결정(#1·#7·아이콘 세트)에 맞게 고칠지 | 🔶 고친다 — 안 고치면 규칙 1 때문에 Windows 쪽이 그 자리마다 멈춰 묻는다 |
| 20 | 화면 구성 프리셋 규칙 = design.md 결정 1~9 일체(3×3 격자·우선순위·`.pansetting` 포함·자동 스위치 제외·해상도 제외·판서 보관)를 Windows 도 따름 | ✅ **결정(2026-10-28).** Windows 포함 범위·시점은 #12·#15 |
| 21 | `Control` 토큰의 Windows 대응 — `Control` 과 `Primary` 가 둘 다 Ctrl 이 된다(§2.2 단축키) | 🔶 Ctrl 로 읽고 중복 판정은 OS 매핑 후, 기록기는 `Primary` 만 기록. 대안: Windows 에서 `Control` 포함 override 는 "이 PC 에서 쓸 수 없음" 표시 |
| 22 | Windows 보조 키(`Ctrl+Y` 다시 실행·`F1` 도움말·`Delete` 선택 삭제 — design.md 에만 있고 카탈로그에 없음) | 🔶 카탈로그 밖 별칭(`keymap.rs` 표) — 픽스처 동치 유지. 대가: 도움말·설정 표에 안 보임. 대안: Windows 전용 바인딩으로 카탈로그에 |
| 23 | 키캡(⌘ 단독 길게 → 단축키 표시)의 Windows 키 — 설계서는 ⌘ 만 적음 | 🔶 `Ctrl` 단독 0.5초(⌘↔Ctrl 원칙) |
| 24 | 통과 중 훅이 삼킬 키 — macOS 키 모드는 활성 이벤트 탭이 대상 키(`U`, 수정키 없음)의 keyDown/keyUp 만 삼키고(손쉬운 사용 권한이 있을 때), 시간제는 포커스를 가진 오버레이가 `Esc` 를 받는다. Windows 훅은 항상 모든 키를 봄 | 🔶 키 모드: 대상 키 keyDown(반복 포함)·keyUp 만 삼키고 나머지 통과. 시간제: 첫 클릭·스크롤 전 `Esc` 만 삼키고 이후 0(안 하면 아래 앱 타이핑이 막힘). W2 실기로 확인 |
| 25 | Win+P(토폴로지만 변경)와 앱 시작 시 자동 전환 | 🔶 Win+P 는 트리거 아님(연결 집합이 바뀔 때만). 앱 시작 시 현재 집합이 자동 프리셋과 맞으면 적용할지는 W11 착수 전 확정 |
| 26 | 프리셋 적용의 저장 정책 | 🔶 임시 적용(저장 안 함) → 확정 시 `SDC_SAVE_TO_DATABASE`. 대안: 항상 저장 |
| 27 | 복제 마스터 — CCD 에 마스터가 없어 "경로 우선순위 첫 타깃" 으로 둠(§3.1.2) | 🔶 macOS 의 마스터(해상도·모드 기준)와 같은 의미로 본다 — 확인 |
| 28 | 아이콘 최소 표시 크기 — 디자이너 PNG 는 16 DIP 하한인데 macOS 표시는 10~14pt | 🔶 macOS 와 같은 크기로 시작하고 W4a 실기(§9)에서 안 읽히면 키움(레이아웃 영향 — 팔레트와 합의) |
| 29 | 앱 아이콘 원본 — macOS `build.sh` 의 코드 드로잉을 그대로 쓰나, 별도 디자인인가. 누가 `.ico` 를 만드나 | 🔶 macOS 쪽이 `assets/app-icon/` 에 PNG·`.ico` 를 내보냄(§3.2.7·§5.4). 별도 디자인이면 디자이너가 제공 |
| 30 | `keystroke-display.toggle` 의 Windows 아이콘(`key.ctrl` 키캡 vs "Ctrl" 글자)과 호버·눌림 표현(배경 α 6%·10%·WinUI 토큰 색 — 제안값) | 🔶 `key.ctrl` 과 제안값으로 시작, W4a 실기 후 취향 확인. 속성 아이콘 halo 배경은 별도 확정 규칙: 원형=선택 강조색/그 밖 어두운 면 고정, 가로=선택 강조색/그 밖 시스템 모양 0.93·0.18(§3.2.6) |
| 31 | 아이콘 없는 키: `annotation.pass-through`(U — 카탈로그 정의 있음·SF `cursorarrow.click`, 디자인 SVG 없음)·`eraser.area-shape`·`palette.radial.toggle`(기존 디자인 재사용 가능) | 🔶 후자 둘은 `symbol-map.tsv` 한 줄 연결(macOS 쪽 결정), `pass-through` 는 디자인 수령 또는 Segoe Fluent 글리프(§3.2.3). 즐겨찾기 저장 칸은 `plus`, 저장 항목은 `AttrIconToolCatalog`의 도구↔manifest 표와 속성 스냅샷을 사용 |
| 32 | 설정 이름 길이(≤40) 판정 — Swift 는 자소(`String.count`), Rust `chars()` 는 스칼라라 결합 문자·ZWJ 이모지에서 더 엄격 | 🔶 의존 추가(`unicode-segmentation`) 없이 스칼라 기준 + 알려진 편차(의존 추가는 승인 필요) |
| 33 | `SettingsBundle` 에 값을 더하면(예: `display`) 구 리더가 `includedBundles` 의 모르는 값에서 파일 전체를 거부 — `formatVersion` 을 올릴지 | 🔶 지금은 배포된 구 버전이 없어 무해. 다음 묶음 추가 PR 에서 결정 |
| 34 | Windows 펜 버튼·제스처 매핑 | 🔶 **권장: 펜 측면 버튼 = 마우스 우클릭 = 누르는 동안 임시 지우개**, 펜 지우개 끝도 같은 `uses_eraser` 판정. Apple Pencil `changeMode(with:)` 더블탭은 Sidecar 전용이라 Windows 대응 없음. 지우개 ↔ 이전 도구 전환이 필요하면 기본 키 없는 `tool.eraser-toggle`에 사용자가 키를 지정하고, Windows 더블탭 대응은 이번 범위 밖 |
| 35 | 줌 중 포인터 모양: 실제 Windows 커서 / 화살표 하나 | ✅ **확정(2026-10-31): 화살표 하나만 그린다(macOS 동일).** 실제 커서 비트맵 취득을 시도했으나 우리가 설정한 투명 커서가 `GetCursorInfo`/`GetIconInfo` 에 그대로 잡혀 취득이 항상 실패했다. 취득 코드는 제거했다 |
| 36 | 확인 대기 중 앱 비정상 종료 시 Windows 화면 구성 복구 | 🔶 **권장: 별도 감시 프로세스 + 다음 시작 시 미확정 구성 복구를 함께 둔다.** 부모가 스냅샷과 확인 토큰을 원자적으로 기록한 뒤 감시 프로세스를 시작하고, [유지] 토큰 없이 부모 종료 또는 15초 만료를 감지하면 감시 프로세스가 스냅샷을 `SetDisplayConfig`로 즉시 복구한다. 감시도 실패하거나 전원이 끊긴 경우 다음 시작에서 미확정 기록을 보고 복구를 다시 제안·시도한다. 대안은 다음 시작 복구만 두는 것이지만 앱 종료 직후 화면을 잃은 채 남는다. 프로세스 수명·서명·동시 실행과 스냅샷 파일 보호는 **[확인 필요: W11 강제 종료·전원 차단 실기]** |

## 8. 가장 큰 위험

1. **두 구현의 동작 표류(상승)** — 정본(Swift)이 계속 바뀐다: 15,113 → 24,624줄(+63%), #41~#53에서 원형 면 설정·입력·속성 아이콘·줌·단축키 슬롯·즐겨찾기·화면 프리셋 적용까지 추가됐다. §5.4의 `.pansetting`·기본 단축키 픽스처 2종은 **아직 없지만**, `fixtures/vectors/`에는 속성 아이콘 3종과 플래너 격자 배치 정본이 있다. 구 자체 시험 76확인은 새 시험 ID 64개로 전부 이관됐고, 팔레트 탐색 T1·즐겨찾기 배치/히트 T3·설정 골든 T2·화면 적용 상태 기계 T2도 생겼다. 아직 교차 플랫폼 입력이 없는 팔레트 기하·힌트, 도구별 기억 이행, 통과 모드, `DisplayPresetCapture`·`matchesCurrent`는 표류 위험이 남는다. 대응: 미완 픽스처 선행 + §7 #14 + Swift 로직 PR마다 Windows 영향 표시(#18).
2. **줌 캡처·입력 의미론** — 캡처에서 커서를 끄고 실제 Windows 커서를 별도 합성하며, 라이브/정지 입력 매핑과 제스처 잠금·키 라우팅까지 동치를 맞춰야 한다. 최소 버전·배포 형식은 #1·#6, 포인터 모양 권장은 #35에 기록했다 → W1 ⑤⑥과 W7 커서·판서 실기로 판별.
3. **입력 라우팅·z순서** — 변경 없음 + 새로 통과 모드(자동 복귀가 UIPI 영향)·대화상자 높이 계층(#12).
4. **시각 동치 확인(신규)** — 원형 팔레트는 RGB(20,23,31) 단색 반투명 면과 호/아이콘 상태색, 속성 아이콘의 어두운 배경 추정을 수치로 고정했다. 흰 배경 위 최저 렌더 α0.75와 흰 내용의 대비 7.94:1 계산과 별개로 실제 화면·빔 가독성 실기가 남는다(#13).
5. **화면 구성 적용은 실기 의존(신규)** — macOS 2단계 구현은 생겼지만 실기 확인 전이고, 잘못된 적용은 화면을 잃는다. Windows에는 `.forAppOnly`의 앱 종료 자동 복구 대응이 없어 감시 안전장치가 별도로 필요하다 → #15·#36 확정과 macOS 실기 전 W11 착수 금지.
6. **설정 스키마 손실 왕복(신규)** — `.pansetting`에 `display`·`pressureMinimumWidthRatio`·`perToolMemory`·`favorites`·`paletteGlassOpacity`·옛 키 이행이 들어 있다. Windows가 모르는 키를 버리면 macOS로 되돌아올 때 사라진다 → W2 첫 왕복 실물 시험.
7. **모니터 식별이 macOS 보다 약하다(신규)** — devicePath 는 포트·독에 묶이고 프로젝터·일부 모니터의 EDID serial 이 0·1 이라 ② 도 실패한다. 자동 전환은 "모르는 모니터가 있으면 전환 안 함"이라 안전하지만 "연결 안 됨"이 잦다. Miracast·IDD 는 연결마다 식별이 바뀔 수 있다(§3.1.1·§3.1.5).
8. **GDI 배치 제약 ↔ 계획 결과(신규)** — #38 로 격자 이웃을 변을 맞대 붙여 틈은 규칙상 없어졌고 겹침은 적용 불가로 걸러지지만, 크기가 다른 화면은 변이 일부만 닿는다. `SetDisplayConfig` 가 이 배치를 그대로 받는지(`SDC_VALIDATE`)는 W11 실기 [D8](§3.1.3 #3).
9. **Windows 자체 구성 DB·Win+P 와의 경합(신규)** — 저장 구성이 서로 덮고 Win+P 가 자동 전환과 싸울 수 있다(§3.1.3·§3.1.4).

해소: "원형 팔레트 진행 중" → 머지 완료(PR #32). 최소 OS·배포·서명·음악 종료 방식 미결 → 결정(#1·5·6·7). macOS 권한 중 손쉬운 사용·자동화 2종은 Windows 에 대응 없음(화면 캡처는 §3 줌 행의 WGC 동의 흐름).

## 9. 확인 필요

| 항목 | 확신 | 확인 방법 |
| --- | --- | --- |
| 비패키지 Win32 에서 `IsBorderRequired=false` 가 동의 프롬프트 없이 먹는가 | Low | W1 ⑥ 실기 1회 |
| `GetCursorInfo`로 얻은 실제 커서의 핫스팟·다중 DPI 크기·애니메이션 프레임을 D2D 최상단 레이어에서 지연 없이 재현할 수 있는가 | Med | W7: I-빔·손가락·화살표를 100%/150% 화면에서 라이브·정지·미리보기 비교, 애니메이션 커서 1종 |
| `WDA_EXCLUDEFROMCAPTURE` 가 DComp(`NOREDIRECTIONBITMAP`) 창에도 적용, WGC·DDA·Teams 에서 모두 빠짐 | Med | W1 ⑤ |
| 슬라이드쇼 위 TOPMOST 유지(재확보 필요 여부), `uiAccess` 없이 충분한가 | Med | W1 ④ |
| LL 훅에서 한글 IME 상태 `P` 가 `VK_P`/고정 스캔코드로 오는가 | Med-High | W2 실기 |
| `build.rs` 의 `rc.exe` 경로 탐색(Build Tools 개발자 셸 밖에서 `cargo build`) | Med | W1 첫 빌드 |
| PNG 압축 ICO 를 `rc.exe` 가 받는가, `LoadImageW` 가 20·24 항목을 정확히 고르는가 | Med | W1 첫 빌드 |
| 루트 `color` → `currentColor` 전파(D2D SVG `stroke="currentColor"` 가 `color` 를 따르는가), `SetTransform` 스케일·클립(`CreateSvgDocument`) — 안 따르면 루트 `<svg color=…>` 설정 또는 문자열 치환 | Med | W1 SVG 시험 1회 |
| `SystemUsesLightTheme` 갱신(`WM_SETTINGCHANGE`), 고대비 테마에서 트레이 가시성, 신규 트레이 아이콘 기본 숨김 | Med | W1 실기(테마 전환·고대비) |
| `monitorDevicePath` 가 같은 모니터의 포트·독 변경에도 같은가 | Low | 같은 모니터를 포트 2곳·독 경유로 연결해 값 비교 (W1) |
| EDID 바이트 8–15 가 macOS 의 vendor/model/serial 과 같은가, 빔·내장 패널의 serial (같으면 양 OS 프리셋이 ② 로 매칭) | Med | macOS 가 내보낸 `.pansetting` identity 와 대조, 또는 같은 모니터를 Mac·Windows 에 꽂아 값 비교 (W1) |
| `WM_DISPLAYCHANGE`·`DBT_DEVICEARRIVAL/REMOVECOMPLETE` 가 위치만 바뀌는 변경·연결 해제에 오는가 | Med | 확장 추가·위치 이동·케이블 분리 때 알림 로그 (W1) |
| 앵커 owner 대화상자의 팔레트 입력 허용, 타이머 콜백에서의 포그라운드 권한 | Med | W1 ⑧ |
| TOPMOST·TOOLWINDOW 오버레이가 새 가상 데스크톱에서 보이는가 | Low | Win+Ctrl+D (W1 ⑨) |
| 혼합 DPI 에서 소스 `position` 이 물리 픽셀로 `rcMonitor` 와 일치하는가 | Med | 150%+100% 에 적용 후 `EnumDisplayMonitors` 와 비교 |
| 통과 모드 복귀가 아래 앱이 관리자 권한일 때 UIPI 로 안 풀리는가(키 모드 = `WH_KEYBOARD_LL` keyUp, 시간제 = `WH_MOUSE_LL` 입력 — 못 받으면 30초/10초 안전망으로만 복귀) · LL 훅의 keyUp 만으로 키 모드 종료가 신뢰되는가 | Med | W2 실기 |
| 통과 중 키 삼킴 범위(키 모드는 대상 키 `U` 만, 시간제는 첫 클릭 전 `Esc` 만)로 아래 앱 타이핑이 막히지 않는가 | Med | W2 실기(§7 #24) |
| 마우스 `POINTER_FLAG_INCONTACT` 의미, `WM_POINTER` 드래그 캡처(`SetCapture`) 동작 | Med | W2 실기 |
| 임시 지우개 소유권 복구에서 마우스는 `GetAsyncKeyState`, 펜은 포인터 상태로 "이전 소유 버튼이 실제로 눌림"을 신뢰성 있게 판정할 수 있는가 | Med | W2: 우클릭·측면 버튼 down 중 다른 버튼, up 유실·`CAPTURECHANGED` 합성 입력 |
| 전역 키 해제 감지: `RegisterHotKey` 후 `GetAsyncKeyState` 폴링(≈30ms)으로 `⌃⌥/` 길게 누르기를 구현할 수 있는가(LL 훅 대안) | Med | W4a 도움말 실기 |
| 10~14 DIP 아이콘(선 0.7~1.0px @100%) 가독 — 디자이너 PNG 는 16 DIP 하한 | Med | W4a 실기(100% 모니터·빔) |
| `UISettings` 강조색·`ColorValuesChanged` 를 비패키지 Win32 에서 호출 | Med-High | W4a |
| 어두운 면 0.75/0.86/0.90과 흰 내용의 실기 가독성 — 흰 배경 위 최악 렌더 α0.75의 계산상 최저 대비는 7.94:1이지만 실제 디스플레이·빔 출력 확인 필요 | Med-High | W4b: 밝은·검정 바탕·PowerPoint 슬라이드쇼·빔에서 세 값 비교 |
| 속성 아이콘 배경 추정(원형 어두운 면 고정, 가로 0.93/0.18)과 halo가 선택·비활성·다크 모드에서 실제 뒤 면을 충분히 대표하는가 | Med | W4b: 8개 도구×선택/비선택, 라이트/다크, 저대비 색 실기 |
| DWM 시스템 백드롭 PoC(1시간, 대안 A 보다 쌈): 302 DIP·`NOREDIRECTIONBITMAP`·`NOACTIVATE` 창에 `DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE=DWMSBT_TRANSIENTWINDOW)`(+필요 시 `DwmExtendFrameIntoClientArea(-1)`) → ① 비활성인데 Acrylic 이 유지되나 ② `SetWindowRgn`(링 부채꼴 합집합)이 백드롭을 모양대로 자르나(가장자리 계단 정도) ③ 같은 HWND 의 DComp+D2D 알파 면과 겹쳐 그려지나 | Low | W4b 이후 1회 |
| 호스트 백드롭 브러시+`CompositionGeometricClip` 이 Win32 호스트에서 모양 있는 블러를 내는가 | Low | 대안 B 채택 시에만 PoC |
| Chrome `--new-window` 대신 SMTC 세션 목록에 Chrome(YouTube Music)이 잡히고 `TryPauseAsync` 가 되는가(창 찾기 불필요 — #7) | Med | W6 실기 |
| 공인 인증서로 교체할 때의 보관 방식·한국 명의 서명 자격(MSIX 는 비채택 — #5·#6) | Low | 서명 업체 문의, 배포 시점 |
| DComp 저지연 잉크(`IDCompositionInkTrailDevice` 류, Win11) 사용 가치 | Low | W2 지연 측정 후 필요 시 |
| 기록 버튼이 포커스일 때 자기 전역 키(`RegisterHotKey`)·모드 키 훅이 먼저 가로채는가(원형 팔레트의 "설정 열기" 로 판서 중에도 설정 창이 열린다) | Med(추론) | W9 실기: 현재 `Ctrl+Alt+P` 를 같은 키로 재기록 |
| 탐색기·작업 표시줄이 최저 ID 아이콘을 쓰는가, `DefaultIcon` `,0` 인덱스 | Med | W9 |
| Git Bash 에서 `import-symbols.sh` 실행 | Med-High | 1회 |
| macOS 에서 내보낸 `display` 묶음을 Windows 가 읽고 연결 안 된 모니터를 재지정할 수 있는가(`uuid`·`vendor`·`model`·`serial` 의 OS 별 의미) | Med | W10: macOS 가 내보낸 실물 파일 가져오기 |
| 가상·무선 화면 식별 안정성 — macOS 쪽 Sidecar·AirPlay UUID 재연결 유지가 미확인(`OnlineDisplayProvider.swift:109`). Windows 쪽 대응은 화면 확장 앱(갤럭시 탭)·Miracast 의 `kind=virtual` 판별 | Low | W1 ③ 에서 재연결 전후 모니터 경로 덤프 비교 |
| 대각선만 닿는 배치 금지(`DisplayGrid.isConnected` DisplayPreset.swift:237)가 Windows 배치 규칙과 맞는가 | Low | W10: 설정 앱에서 모서리 접촉 배치 시도 |
| 비활성 연결 모니터가 present 로 남고 선호 모드로 크기를 얻는가 | Med | Win+P "PC 화면만"에서 열거 (W10 착수) |
| 틈 있는 배치를 `SDC_VALIDATE` 가 거부·보정하는가, 복제 슬레이브 경로, `SDC_TOPOLOGY_SUPPLIED` 2단계 | Med | 합성 입력 3종으로 VALIDATE→APPLY (W11 착수) |
| 확정 시 `SDC_SAVE_TO_DATABASE` 가 Win+P 복귀 구성에 미치는 영향 | Low | 적용·저장 뒤 Win+P 순환 (W11) |
| Miracast·IDD 가 devicePath·EDID 를 연결마다 유지하는가 | Low | 같은 수신기에 2회 연결해 비교 (W11) |
| 별도 감시 프로세스가 부모 강제 종료·15초 만료를 감지해 저장된 스냅샷을 즉시 복구하고, 전원 차단 뒤 다음 시작 복구와 충돌하지 않는가 | Low | W11: 확인 중 프로세스 강제 종료·감시 프로세스 강제 종료·전원 차단·동시 실행을 각각 합성해 복구 결과와 미확정 기록 정리 확인(#36) |

출처
- SetWindowDisplayAffinity: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity
- MagSetWindowFilterList: https://learn.microsoft.com/en-us/windows/win32/api/magnification/nf-magnification-magsetwindowfilterlist
- GraphicsCaptureSession.IsBorderRequired: https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturesession.isborderrequired
- Chrome 136 remote debugging 변경: https://developer.chrome.com/blog/remote-debugging-port
- POINTER_PEN_INFO / 펜 플래그: https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-pointer_pen_info · https://learn.microsoft.com/en-us/windows/win32/inputmsg/pen-flags-constants
- Wacom "Windows Ink 사용" 필요성: 포럼·앱 소스 기반(KDE https://phabricator.kde.org/T6807, Blender) — 공식 문서 아님
- 팔레트 면·백드롭: DWM_SYSTEMBACKDROP_TYPE https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwm_systembackdrop_type · DwmEnableBlurBehindWindow https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmenableblurbehindwindow · Acrylic(비활성·투명 효과·배터리 절약 시 단색) https://learn.microsoft.com/en-us/windows/apps/design/style/acrylic · Win32 Mica(Windows App SDK 필요) https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-mica-win32
- CreateHostBackdropBrush https://learn.microsoft.com/en-us/uwp/api/windows.ui.composition.compositor.createhostbackdropbrush · CreateBackdropBrush https://learn.microsoft.com/en-us/uwp/api/windows.ui.composition.compositor.createbackdropbrush · CompositionGeometricClip https://learn.microsoft.com/uwp/api/windows.ui.composition.compositiongeometricclip · Win32 호스트 백드롭 검은 사각형 보고 https://learn.microsoft.com/en-us/answers/a/152907
- DComp 로 뒤 화면 블러 불가(공식 진술 없음, 커뮤니티): https://github.com/selastingeorge/Win32-Acrylic-Effect
- 접근성: UISettings.AdvancedEffectsEnabled https://learn.microsoft.com/en-us/uwp/api/windows.ui.viewmanagement.uisettings.advancedeffectsenabled · HIGHCONTRAST https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-highcontrasta
- 입력: WM_NCHITTEST(`HTTRANSPARENT` 같은 스레드) https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-nchittest · Window Features(layered 히트 테스트) https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features · WM_POINTERLEAVE https://learn.microsoft.com/en-us/windows/win32/inputmsg/wm-pointerleave · Pointer Flags https://learn.microsoft.com/en-us/windows/win32/inputmsg/pointer-flags-contants · POINTER_INFO https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-pointer_info · EnableMouseInPointer https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enablemouseinpointer
- 화면·창 [D1]~[D18]: [D1] https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-displayconfig_target_device_name · [D2] https://learn.microsoft.com/en-us/windows/win32/wmicoreprov/wmimonitorid · [D3] https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ne-wingdi-displayconfig_video_output_technology · [D4] https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdiopendevregkey · [D5] https://learn.microsoft.com/en-us/windows-hardware/drivers/install/guid-devinterface-monitor · [D6] https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-querydisplayconfig · [D7] https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-displayconfig_path_source_info · [D8] https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-displayconfig_source_mode · [D9] https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setdisplayconfig · [D10] https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features · [D11] https://learn.microsoft.com/en-us/windows/win32/gdi/wm-displaychange · [D12] https://learn.microsoft.com/en-us/windows/win32/devio/registering-for-device-notification · [D13] https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-displayconfig_path_target_info · [D14] https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows · [D15] https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumdisplaymonitors · [D16] https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos · [D17] https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-messageboxw · https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-imodalwindow-show · [D18] https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ivirtualdesktopmanager
- D2D SVG(§3.2.2·§3.2.4~5): https://learn.microsoft.com/en-us/windows/win32/direct2d/svg-support · https://learn.microsoft.com/en-us/windows/win32/api/d2d1svg/ne-d2d1svg-d2d1_svg_paint_type · https://learn.microsoft.com/en-us/windows/win32/api/d2d1_3/nf-d2d1_3-id2d1devicecontext5-createsvgdocument
- 고대비·Segoe Fluent Icons: https://learn.microsoft.com/en-us/windows/win32/winauto/high-contrast-parameter · https://learn.microsoft.com/en-us/windows/apps/design/style/segoe-fluent-icons-font
- 트레이: NOTIFYICONDATAW https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ns-shellapi-notifyicondataw · About Icons https://learn.microsoft.com/en-us/windows/win32/menurc/about-icons · LoadIconMetric https://learn.microsoft.com/en-us/windows/win32/api/commctrl/nf-commctrl-loadiconmetric · 테마 값 위치(`SystemUsesLightTheme`)는 공식 문서 없음(커뮤니티 관례, Med)
