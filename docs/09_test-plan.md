# 테스트 계획서

| 항목 | 내용 |
|---|---|
| 문서 ID | 03.02.09 |
| 작성자 | 이경환 (AI 초안) |
| 작성일 | 2026-10-31 |
| 버전 | 0.1 |
| 상태 | 초안 |
| 선행 문서 | 03_기능요구사항정의서, 07_API명세서 |
| 후행 문서 | 10_회고 |

## 1. 개요
### 1.1 문서 목적
기능 요구사항(FR)을 어떤 시험으로 검증하는지, 무엇이 자동이고 무엇이 사용자 실기인지, 언제 "끝났다"고 보는지 정한다. 시험 체계의 정본은 [`testing/strategy.md`](testing/strategy.md)·[`testing/README.md`](testing/README.md)이며 이 문서는 FR 단위 계획·추적·완료 기준을 맡는다.

### 1.2 테스트 범위
macOS 앱(Swift), Windows 앱(Rust: `edutools-core`·`edutools`), 두 OS 공유 계약(JSON 픽스처·`.pansetting`), 배포 번들·설치 파일. **범위 밖**: 유튜브 서비스 자체, OS 자체 기능, 성능 벤치마크 자동화.

## 2. 테스트 전략
### 2.1 테스트 레벨 (단위, 통합, 시스템, 인수)
| 계층 | 검증 | 실행자 | 진입점 |
|---|---|---|---|
| T0 빌드 | 두 아키텍처 컴파일·번들·서명 | 개발 | `./build.sh` |
| T1 단위 | 순수 로직(입력→출력) | 개발, 헤드리스 | `scripts/test.sh unit` · `cargo test` |
| T2 통합 | 모듈 합성·임시 I/O·골든·가짜 계층 | 개발, 헤드리스 | `scripts/test.sh integration` |
| T3 GUI | 실제 뷰·창·합성 이벤트 | 사용자 맥(WindowServer) | `scripts/test.sh gui` |
| T4 E2E | 실제 앱·OS 입력·화면 픽셀(스모크 9) | 사용자(권한 필요) | `scripts/test.sh e2e` |
| T5 릴리스 | 번들·plist·리소스·자체 시험 코드 부재·DMG | 릴리스 담당 | `scripts/verify-bundle.sh` |
| T6 수동 | 펜·Sidecar·다중 모니터·프로젝터·권한 | 사용자 실장비 | `docs/testing/manual-checklist.md`(69항목) |
| W-VM | Windows 빌드·`--selftest`·SendInput 시나리오·스크린샷 | 개발(Parallels VM) | `vm-*.ps1`(작업 공간 `~/jia/.worktrees/_tools`) |
| W-실기 | Windows 실장비 확인 18항목 | 사용자 | 작업 공간 `reports/eduTools-windows-user-checklist.md` |
시스템·인수 시험은 T4·T6·W-실기가 맡는다.

### 2.2 테스트 유형 (기능, 성능, 보안, 호환성)
| 유형 | 방법 |
|---|---|
| 기능 | T1~T4 + 수동 |
| 성능 | VM에서 줌 fps·유휴 CPU·점당 CPU·`ui_blocked` 측정(스크립트·로그). 실기 재측정은 T-07 |
| 보안 | 경로 거부(UNC·`\??\`)·URL 허용 목록은 설정 시험에서 거부 입력 행으로 확인, 변이로 검출력 확인. 보안 검토 지적은 즉시 시험에 반영 |
| 호환성 | 구버전 골든 설정 왕복, macOS↔Windows `.pansetting` 픽스처, 모니터별 DPI(VM 제한) |
| 회귀 | `crash-regressions.md` 대장의 재현 시험 |

### 2.3 자동화 범위
**자동**: T0·T1·T2·T5, Windows 코어 `cargo test`, VM selftest·시나리오. **수동/사용자**: T3·T4(권한)·T6·W-실기(소리 청취, 펜 필압, 다중 모니터, IME 등). 정책: 위험 근거 없는 시험·같은 동작 나열·소스 문구 고정 시험 금지, 기존 시험 약화·`skip` 금지, 실패를 재시도로 덮지 않는다. 시험 수는 늘리지 않는 쪽을 기본으로 하되 eduTools는 자체 시험 체계의 개발 게이트를 따른다(순수 로직·설정 스키마 변경은 해당 계층 시험 추가 가능).

## 3. 테스트 환경
### 3.1 환경 구성
| 환경 | 구성 |
|---|---|
| macOS 개발 | macOS 26, Command Line Tools(Xcode 없음), SwiftPM 시험 |
| Windows 개발 | Mac에서 `cargo`(코어 시험), Parallels "Windows 11" ARM64 VM(x64 에뮬레이션)에서 앱 빌드·실행. 저장소 private이므로 VM에는 소스를 **공유 폴더로 동기**(`vm-build-src.ps1`)하고 자격 증명을 두지 않는다 |
| 실장비 | 펜 태블릿(기종 미정), 외부 모니터·프로젝터, iPad(Sidecar) [확인 필요] |

### 3.2 테스트 데이터
- 픽스처: `fixtures/shortcuts-default.json`, `fixtures/pansetting/{macos,windows}-v1.pansetting`, 골든 설정 `Tests/Fixtures/settings/v0.1.0/settings.json`
- 시드 고정 난수·주입 시계·`TempDir` 임시 폴더
- 음악: 테스트 `wav`(2초) 폴더 — 숨김 파일·텍스트 파일·손상 파일·하위 폴더 포함(VM 스크립트가 생성)

## 4. 테스트 케이스
### 4.1 케이스 ID 체계
- 자동: `<타깃>.<스위트>/<시험 이름>`(예: `UnitTests.ShortcutTests`), Rust는 `<crate>::tests/<파일>::<함수>`
- 수동: `<영역 2자>-<번호>`(예: `ZC-01`) — `manual-checklist.md`
- Windows 실기: `W-<번호>` (체크리스트 1~18)
- VM 시나리오: `vm-<이름>.ps1`

### 4.2 기능별 테스트 케이스 목록
현재 자동 시험: **macOS 100건/31스위트(`./build.sh test`, 경고 0), Rust 155건(`cargo test --workspace`), VM `cargo test -p edutools` 54건 + `--selftest`**. 수동 69항목(PW 필압·GL/RP 팔레트·RE/PE 지우개·ZC/LZ 줌·SR 도형·AI 아이콘·FV 즐겨찾기·SC 키 지정·DP 화면 구성·LC 판서 생명주기·SM 스모크·CR 크래시).

### 4.3 요구사항 추적표 (기능 ID ↔ 테스트 ID)
| FR 영역 | 자동 시험(macOS / Windows 코어) | 수동·VM |
|---|---|---|
| FR-TL | AttrIconTests, RadialPalette{Navigation,Preview}Tests / `attr_icon`, `radial` | GL-01~05, RP-01~03, AI-01~05, FV-01~04 |
| FR-IK | InkGeometryTests, InkSerializationTests, UndoTests, RecognitionTests, SelectionTests / `ink`, `shape` | SR-01, LC-01~04(판서 생명주기), LZ-01~07, VM 판서·전체 지우기 시나리오(`vm-ckey`) |
| FR-ER | EraserTests / `eraser` | RE-01~04, PE-01~04 |
| FR-BD | BoardColorTests / `board` | SM 스모크 |
| FR-ZM | ZoomTests, GeometryTests / `zoom` | ZC-01~06, LZ-01~07, 줌 fps VM 측정 |
| FR-PR | KeystrokeDisplay 관련(Windows `keystroke`) | SM 스모크 |
| FR-TM | TimerTests / `timer` | VM 쉬는 시간 화면 시나리오 |
| FR-MU | BreakMusicTests, MusicPlaylistTests / `music`(수집·정렬·순서기·실패 연속) | VM `vm-music-*`(이름순·반복·셔플·다음 곡·끄기/켜기·조기 종료), W-16·17(소리·예외) |
| FR-DS | DisplayPlannerTests, ScreenPositionTests, DisplayApplierIntegrationTests / `display_planner`, `display_preset` | DP-01~10, PE-01~04, W-다중 모니터 |
| FR-SV | ExportIntegrationTests, ArchiveIntegrationTests, FileNameTests / `settings_store` | VM `vm-exp5`(PNG·클립보드·PDF·배경 포함·`ui_blocked`) |
| FR-ST | SettingsTests, SettingsRangeTests, SettingsTokenTests, SettingsGoldenIntegrationTests, ShortcutTests / `settings`, `settings_transfer`, `shortcut` | SC-01~05(키 지정), 설정 UI VM 시나리오 |
| FR-HL | ShortcutTests(자동 생성 표) | E2E 스모크 |
| FR-SY | `scripts/verify-bundle.sh`(PASS 17/13) / 설치기 VM(설치→연결→열기→제거) | E2E, W-15 |
각 FR 마다 최소 1개 시험 또는 수동 항목이 연결돼 있다. 연결이 약한 FR: FR-PR-01~04, FR-TM-03·04 (수동·VM 위주) [확인 필요: 자동 시험 보강 여부].

## 5. 일정 및 역할
| 활동 | 시점 | 담당 |
|---|---|---|
| G1(unit+integration) | 모든 라운드의 PR 전 | Claude 실행·Codex 보조 |
| 전체 스위트·교차 소비자 점검 | 계획 묶음 종료 시 1회 | Claude |
| VM 시나리오 | Windows 변경 라운드마다 | Claude |
| T3·T4·T6·W-실기 | 릴리스 전(`v0.1.2`, 정식) | 이경환 |

## 6. 결함 관리
### 6.1 결함 등급 기준
| 등급 | 기준 | 예 |
|---|---|---|
| P0 | 판서 유실·크래시·전면 먹통·보안 노출 | UNC 경로로 SMB 접속 |
| P1 | 핵심 기능 불가 또는 우회 어려움 | 줌 시작 실패, 전역 키 미동작 |
| P2 | 기능 일부 오작동·UI 깨짐 | 내보내기 폴백, 로그 소음 |
| P3 | 미관·문구 | 안내 문구 |

### 6.2 결함 처리 절차
발견(실기·시험·보안 검토) → 재현 방법 기록 → 가장 낮은 계층의 재현 시험 → 수정 라운드(지시서) → 게이트 → 머지 → 크래시·데이터 유실은 `crash-regressions.md`에 증상·원인·수정·재현 시험 기록. 우선 처리 순서 P0 → P3. 해결하지 않는 결함은 `park` 문서로 남긴다.

## 7. 완료 기준 (Exit Criteria)
릴리스 후보(`v0.1.2` 및 정식)는 아래를 모두 만족해야 한다.
| 기준 | 수치 |
|---|---|
| G1 | `scripts/test.sh unit integration` 종료코드 0, 실패 0 |
| macOS `./build.sh test` | 통과, 컴파일 경고 0 |
| Windows | `cargo test --workspace` 실패 0, VM `cargo test -p edutools` 실패 0, `--selftest` 종료코드 0 |
| T5 | `verify-bundle.sh` FAIL 0 (앱·DMG) |
| 수동 | `manual-checklist` P0·P1 항목 통과, W-실기 18항목 중 미통과 0 또는 사유 승인 |
| 결함 | 미해결 P0 0건, P1 0건 |
| 성능(VM·실기) | 줌 ≥ 50 fps, 유휴 CPU ≤ 1 %, `ui_blocked` ≤ 150 ms |
| 보안 | 가져온 설정의 UNC·장치 경로 거부 확인 |

## 8. 테스트 결과 보고
**2026-10-31 기준(`main` 3a93ba6)**
| 항목 | 결과 |
|---|---|
| macOS `./build.sh test` | 100 passed / 31 suites, 경고 0 |
| Rust `cargo test --workspace` | 155 passed, 실패 0 |
| VM(Windows) | `cargo test -p edutools` 54 passed, selftest 종료코드 0 |
| `verify-bundle.sh` | 앱 PASS 13, DMG PASS 17 (FAIL 0) |
| VM 시나리오 | 내보내기(PNG·클립보드·PDF)·로컬 폴더 음악·전체 지우기 전역 키·성능 측정 통과 |
| 변이 증거 | 경로 구분자 규칙·정렬·셔플 직전곡 회피에서 RED 확인 후 원복 |
| 실기 | **미실시**(W-실기 18, T6 69) |
결과 보고는 라운드 종료 시 사용자에게 한국어로, 실패·미실시를 그대로 적는다.

## 변경 이력

| 버전 | 일자 | 변경 내용 | 작성자 |
|---|---|---|---|
| 0.1 | 2026-10-31 | 최초 작성(기존 시험 체계 기준 역작성) | 이경환 (AI 초안) |
