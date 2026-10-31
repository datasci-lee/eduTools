# 기술 스택 결정서

| 항목 | 내용 |
|---|---|
| 문서 ID | 03.02.06 |
| 작성자 | 이경환 (AI 초안) |
| 작성일 | 2026-10-31 |
| 버전 | 0.1 |
| 상태 | 초안 |
| 선행 문서 | 02_PRD, 05_데이터모델_ERD |
| 후행 문서 | 07_API명세서, 08_작업실행계획 |

## 1. 개요
### 1.1 문서 목적
이미 내려진 기술 결정(macOS Swift, Windows Rust)의 근거와 기각된 대안을 한 곳에 정리한다. 결정의 원문은 [`design.md`](design.md) 개요·기술 구조와 [`windows-plan.md`](windows-plan.md) §0·§1·§7(결정 표)이다. 아래 점수는 사후 정리한 **작성자 판단 [가정]**이며, 결정 권한은 사용자에게 있다(macOS: 2026-10-28, Windows: 2026-10-28 "C — 앱 전체 Rust").

### 1.2 결정 범위
데스크톱 앱 셸·렌더링·입력·화면 캡처·저장·배포·빌드·시험. 서버·DB·클라우드는 범위 밖(로컬 앱).

## 2. 결정 기준
### 2.1 평가 항목 (성능, 생산성, 생태계, 비용, 보안, 팀 역량)
| 항목 | 이 제품에서의 의미 |
|---|---|
| 성능 | 투명 오버레이, 60 fps 줌 캡처, 펜 입력 지연, 유휴 CPU |
| 생산성 | 구현 속도, 시험 용이성(순수 로직 분리) |
| 생태계 | OS API 접근, 라이브러리·문서 |
| 비용 | 라이선스·런타임 배포·서버 비용 |
| 보안 | 외부 의존성·공급망, 권한 모델, 샌드박스 |
| 팀 역량 | 1인 개발 + AI 협업이 유지·검증 가능한가 |

### 2.2 가중치
성능 25 · 생산성 15 · 생태계 10 · 비용 10 · 보안 15 · 팀 역량 25 (합 100). 점수는 1~5, 표의 합계는 `가중합/5`(100점 만점).

## 3. 영역별 후보 비교
### 3.1 프론트엔드
**macOS 앱 셸**
| 후보 | 성능 | 생산성 | 생태계 | 비용 | 보안 | 팀 | 합계 |
|---|---|---|---|---|---|---|---|
| **Swift + AppKit(설정만 SwiftUI)** | 5 | 4 | 4 | 5 | 5 | 4 | **90** |
| Electron | 2 | 4 | 5 | 5 | 3 | 3 | 66 |
| Flutter | 3 | 3 | 3 | 5 | 4 | 2 | 62 |

**Windows 앱 셸**
| 후보 | 성능 | 생산성 | 생태계 | 비용 | 보안 | 팀 | 합계 |
|---|---|---|---|---|---|---|---|
| **Rust + windows-rs(Win32·COM·WinRT)** | 5 | 2 | 3 | 5 | 5 | 3 | **77** |
| C# / WinUI·.NET | 4 | 3 | 4 | 4 | 4 | 3 | 72 |
| Tauri / Electron | 3 | 3 | 4 | 4 | 3 | 3 | 64 |

Windows는 핵심(오버레이·저수준 훅·캡처·DirectComposition)이 어느 언어든 COM·P/Invoke라 생산성 이점이 작고, 런타임 없는 단일 exe가 배포에 유리하다. 코어(`edutools-core`)를 OS 무관으로 분리해 Mac에서도 `cargo test`가 돈다.

### 3.2 백엔드
없음(서버 없음). 상태 관리자(AppState) 하나가 입력을 받아 엔진(판서·지우개·줌·타이머)에 전달한다.

### 3.3 데이터베이스
| 후보 | 판정 |
|---|---|
| **JSON 파일 + 원자적 쓰기(임시 파일 후 교체)** | 채택 — 사람이 읽고 `.pansetting`·픽스처와 같은 형식, 외부 의존 없음 |
| SQLite | 기각 — 질의·동시성 요구가 없고 의존성이 늘어남. 판서는 문서(획 배열) 단위 |
| UserDefaults(macOS 설정) | 채택(macOS 설정) — OS 표준, Windows는 `settings.json` |

### 3.4 인프라 / 클라우드
없음. 배포 채널은 GitHub Releases(프리릴리스)이며, 정식 배포는 Developer ID 공증 DMG와 코드 서명된 Windows 설치 파일을 목표로 한다.

### 3.5 CI/CD 및 운영 도구
| 후보 | 판정 |
|---|---|
| **로컬 빌드·시험** (`./build.sh`, `scripts/test.sh`, `cargo`, VM 스크립트) | 채택 — 사용자 결정(CI·GitHub Actions 사용 안 함) |
| GitHub Actions | 기각 — 위 결정 |
| Windows 설치기: **Inno Setup 6** | 채택 — 일반 exe, 한국어, `.pansetting` 연결 |
| Windows 설치기: MSIX | 기각 — 셀프 서명 초기 운영과 맞지 않음(결정 #6) |

### 3.6 외부 서비스 / SaaS
| 대상 | 용도 | 결정 |
|---|---|---|
| Google Chrome·YouTube Music | 쉬는 시간 음악(URL 열기) | 채택(크롬 쿠키 읽기·앱 내 로그인 금지) |
| 로컬 폴더 재생(`AVAudioPlayer` / `MediaPlayer`) | 음악 대안 소스 | 채택(2026-10-31, 번들 음원 없음) |
| Apple Developer Program / 공인 코드 서명 인증서 | 서명·공증 | **미확보** [확인 필요] |
| Sparkle / WinSparkle | 자동 업데이트 | 보류(배포 주소 미정) |

**핵심 API 선택**
| 영역 | macOS | Windows |
|---|---|---|
| 오버레이·렌더 | 투명 `NSPanel` + Core Animation | DirectComposition + Direct2D, `WS_EX_NOREDIRECTIONBITMAP` |
| 화면 캡처(줌·내보내기) | ScreenCaptureKit(`SCStream`) | Windows Graphics Capture(WGC) — DDA 대신 |
| 전역 단축키 | Carbon `RegisterEventHotKey` | `RegisterHotKey` + 범위 한정 저수준 훅 |
| 펜 입력 | `NSEvent` 태블릿 이벤트(압력) | `WM_POINTER` |
| 공유 제외 | `NSWindow.sharingType = .none` | `WDA_EXCLUDEFROMCAPTURE` |
| 음악 제어 | AppleScript(탭 닫기)·미디어 키, `AVAudioPlayer` | SMTC 일시정지, `Windows.Media.Playback.MediaPlayer` |
| 시험 | Swift Testing(SwiftPM) | `cargo test` + VM 자체 시험 |
| 의존 라이브러리 | 없음 | `windows`·`serde`·`serde_json` 3개 |

## 4. 최종 선택
### 4.1 선택 결과 요약
macOS: Swift + AppKit, `swiftc`(Command Line Tools, Xcode 불필요), macOS 26+. Windows: Rust(`windows-rs`) 단독 앱, Windows 11 x64, 코어/앱 크레이트 분리. 저장은 JSON, 배포는 DMG·Inno Setup exe.

### 4.2 선택 근거
- 투명 오버레이·클릭 통과·전체화면 앱 위 표시·실시간 줌·펜 압력은 네이티브가 가장 지연이 적다.
- 외부 의존성을 최소화해 공급망·오프라인 빌드 부담을 줄였다.
- 순수 로직을 OS 무관 코어로 분리해 헤드리스 시험이 가능하다(T1·T2).

### 4.3 기각된 대안과 이유
| 대안 | 이유 |
|---|---|
| Electron·Flutter·Tauri | 줌 캡처·오버레이·클릭 통과·압력이 결국 네이티브 우회 |
| Rust 공통 코어 + Swift FFI(Windows 안 B) | 공유할 것은 코드가 아니라 파일 포맷, macOS 재배선 비용이 이득보다 큼 |
| C#/.NET | 핵심이 P/Invoke·COM, 런타임 배포·지원 주기 부담 |
| MSIX·Mica·Acrylic | 초기 셀프 서명 운영과 맞지 않고, 창 전체 효과라 팔레트 면에 부적합 |

## 5. 아키텍처 개요
### 5.1 시스템 구성도
```
 입력(포인터·펜·키·전역 단축키·설정 파일)
        │
        ▼
   AppState / dispatch  ←── 설정(AppSettings) · 단축키 카탈로그
        │
  ┌─────┼───────────┬────────────┬──────────┐
  판서 엔진  지우개 엔진   줌 엔진      타이머·음악 엔진
  (획·Undo)  (교차 판정)  (캡처·좌표)  (시계 기준·페이드)
  └─────┴───────┬───┴────────────┴──────────┘
                ▼
   출력: 모니터별 오버레이 · 줌 창 · 팔레트·배지 · 쉬는 시간 화면
        (Windows: edutools-core = OS 무관 로직 / edutools = Win32·D2D·WGC)
```

### 5.2 주요 컴포넌트
| 컴포넌트 | 책임 |
|---|---|
| 오버레이 관리자 | 모니터별 창 생성·동기화, 5단 창 계층 |
| 판서·지우개 엔진 | 획·점·Undo·교차 판정 |
| 줌 엔진·줌 창 | 캡처, 확대 기하, 포인터 표시 |
| 단축키·디스패치 | 카탈로그 → 범위별 실행 가능 명령 |
| 설정 저장소 | 검증·원자적 저장·백업·`.pansetting` |
| 음악 관리자 | 유튜브 소스·로컬 폴더 소스·페이드 |
| 보관 컨트롤러 | 자동·연결 해제 보관 JSON |

## 6. 리스크 및 대응
| 리스크 | 대응 |
|---|---|
| 두 구현(Swift·Rust)의 동작 이탈 | 교차 플랫폼 JSON 픽스처·골든 설정 시험, macOS Swift를 동작 정본으로 |
| Windows 실기 미검증 | VM 실측 + 사용자 실기 체크리스트 |
| Parallels VM(ARM64+x64 에뮬레이션) 성능이 실기와 다름 | 성능 수치는 VM 기준임을 명시, 실기에서 재측정 |
| 공인 서명·공증 지연 | 프리릴리스 경고 안내, 인증서 확보 후 재배포 |
| 단일 개발자 의존 | 정본 문서·작업 지시서·시험으로 기록 |

## 7. 결정 기록 (ADR 링크)
ADR 폴더는 없다. 결정 기록 위치:
- macOS: [`design.md`](design.md) 상단 결정 박스·개요·기술 구조
- Windows: [`windows-plan.md`](windows-plan.md) §0(선택지 판정), §1(크레이트 선택), §7(결정 표 #1~#36)
- 최근 결정: 전체 지우기 전역 키 `⌃⌥E` 예외(2026-10-31), 로컬 폴더 음악(2026-10-31), 줌 포인터 화살표만(#35, 2026-10-31)
- 별도 ADR 파일화는 필요 시 [확인 필요]

## 변경 이력

| 버전 | 일자 | 변경 내용 | 작성자 |
|---|---|---|---|
| 0.1 | 2026-10-31 | 최초 작성(기존 결정 사후 정리, 점수는 작성자 가정) | 이경환 (AI 초안) |
