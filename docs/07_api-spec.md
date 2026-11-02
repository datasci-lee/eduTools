# API 명세서

| 항목 | 내용 |
|---|---|
| 문서 ID | 03.02.07 |
| 작성자 | 이경환 (AI 초안) |
| 작성일 | 2026-10-31 |
| 버전 | 0.1 |
| 상태 | 초안 |
| 선행 문서 | 04_화면설계서, 05_데이터모델_ERD, 06_기술스택결정서 |
| 후행 문서 | 08_작업실행계획, 09_테스트계획서 |

## 1. 개요
### 1.1 문서 목적
EduTools에는 **네트워크 서버·REST API가 없다**. 이 문서는 템플릿의 "API"를 **앱이 외부와 맞닿는 인터페이스**(파일 형식, 단축키 토큰, 프로세스 인터페이스, 외부 앱 연동, 내부 메시지)로 읽어 명세한다. 두 OS 구현(Swift·Rust)이 같은 계약을 지키는 것이 핵심이다.

### 1.2 API 설계 원칙 (REST / GraphQL / gRPC)
1. **파일 계약 우선**: 두 OS가 공유하는 것은 코드가 아니라 파일 형식·토큰이다.
2. **추가만 허용(additive)**: 새 키는 선택 필드, 없으면 기본값, 모르는 키는 무시.
3. **OS 중립 표기**: 단축키는 `AppMod+P`처럼 저장하고 표시할 때만 OS 표기로 바꾼다.
4. **실패는 거부 후 현재 상태 유지**: 잘못된 입력은 부분 적용하지 않는다.

### 1.3 버전 관리 정책
`.pansetting`·보관 파일·단축키 픽스처는 각각 `formatVersion`(현재 모두 1)을 갖는다. 새 선택 키 추가는 버전을 올리지 않는다. 의미가 바뀌는 변경만 버전을 올리고 이전 버전 파일은 가져오기 시 새 형식으로 변환한다. 앱 버전은 `vYY...`가 아닌 `0.1.x`(SemVer, macOS `CFBundleShortVersionString` / Windows `Cargo.toml`)다.

## 2. 공통 사항
### 2.1 Base URL / 환경
없음(로컬). 데이터 위치:
| 항목 | macOS | Windows |
|---|---|---|
| 설정 | UserDefaults(`kr.datasci.edutools`) | `%APPDATA%\EduTools\settings.json` |
| 로그 | 시스템 로그 | `%LOCALAPPDATA%\EduTools\edutools.log` |
| 내보내기 | 사진 폴더 `EduTools` 또는 설정한 폴더 | 동일 |

### 2.2 인증 / 인가 방식
없음. OS 권한(화면 기록·손쉬운 사용·자동화)만 사용(03 3.7).

### 2.3 공통 요청 헤더
해당 없음. 파일 공통 머리(`.pansetting`): `format`, `formatVersion`, `appVersion`, `createdOS`.

### 2.4 공통 응답 형식
호출 결과는 UI 알림 + 로그로 반환한다. 오류 표시는 한국어 문장(원인 + 조치). 로그 한 줄 형식: `YYYY-MM-DD HH:mm:ss.fff <메시지>`(KST).

### 2.5 오류 코드 체계
| 구분 | 표현 |
|---|---|
| 설정 검증 오류 | `SettingsError`(키·값 범위·허용 목록 위반) → 가져오기 거부, 현재 설정 유지 |
| 파일 I/O 오류 | 원인 문자열 + 경로, 원자적 쓰기 실패 시 기존 파일 유지 |
| OS 호출 오류 | HRESULT/NSError를 로그에 남김(예: `hr=0xC00D36C4` 미지원 형식) |
| 외부 앱 오류 | 크롬 없음 → 기본 브라우저, 권한 없음 → 미디어 키 대체 |

### 2.6 페이징 / 정렬 / 필터 규칙
목록 상한: 재생목록 5, 프리셋 8, 로컬 곡 500, 설정 백업 5. 곡 정렬은 상대경로 대소문자 무시·숫자 인식.

### 2.7 Rate Limit
해당 없음. 화면 구성 자동 적용은 연결 안정 5초·쿨다운 30초를 둔다.

## 3. 엔드포인트 목록
| ID | 인터페이스 | 방향 | 계약 형식 |
|---|---|---|---|
| IF-01 | 설정 저장소 | 앱 ↔ 파일/UserDefaults | AppSettings JSON(05 3.3) |
| IF-02 | `.pansetting` 내보내기·가져오기 | 앱 ↔ 파일 | JSON, `formatVersion` 1 |
| IF-03 | 판서 보관 파일 | 앱 ↔ 파일 | JSON, `formatVersion` 1 |
| IF-04 | 단축키 토큰·기본 카탈로그 | 앱 ↔ 파일 | `fixtures/shortcuts-default.json` |
| IF-05 | 프로세스 인터페이스 | OS → 앱 | 인자·파일 연결·단일 인스턴스 |
| IF-06 | 외부 앱 연동 | 앱 → Chrome·OS 미디어 | URL·AppleScript·SMTC |
| IF-07 | 내부 메시지 | 모듈 ↔ 모듈(Windows) | `WM_APP + n` |
| IF-08 | 이미지·문서 내보내기 | 앱 → 파일/클립보드 | PNG·PDF·`CF_DIBV5`+`PNG` |

## 4. 엔드포인트 상세
### 4.1 API ID / 이름
IF-01 ~ IF-08(3장). 아래 상세는 외부 계약이 있는 IF-02·03·04·05·06·07·08.

### 4.2 메서드 / 경로
| IF | 동작 | 경로·진입점 |
|---|---|---|
| IF-02 | 내보내기 / 가져오기 | 설정 → 일반 [내보내기…][가져오기…], 메뉴바·트레이 메뉴, `.pansetting` 더블클릭 |
| IF-03 | 쓰기 / 읽기 | `<보관 루트>/<yyyy-MM-dd>/<시각>-<해시8>.json`, `<보관 루트>/disconnected/<해시8>.json` |
| IF-04 | 읽기 | 앱 내장 카탈로그 + `settings.shortcuts.overrides` |
| IF-05 | 실행 | `edutools[.exe] [<파일>.pansetting]`, `edutools.exe --selftest`(Windows 자체 시험, GUI 서브시스템이라 출력은 파이프로만) |
| IF-06 | 열기·정지 | `open -a "Google Chrome" <URL>` / `chrome.exe <URL>`, AppleScript 탭 닫기, SMTC 일시정지 |
| IF-07 | 게시 | `PostMessageW(owner, WM_APP+n, …)` |
| IF-08 | 저장 | Ctrl+S / Ctrl+C(`⌘S` / `⌘C`), 메뉴 항목 |

### 4.3 설명
- **IF-02**: 묶음(general·tools·eraser·zoom·board·timer·display·shortcuts·help)을 골라 하나의 파일로 내보내거나 가져온다. 컴퓨터 전용 값(모니터별 위치·보관 폴더 경로·`autoSwitchEnabled`)은 기본 제외. 가져오기 순서: 검사 → 미리보기 → 단축키 충돌 검사 → 자동 백업 후 적용 → 되돌리기.
- **IF-04**: 키 토큰 문법 — 수정자 `AppMod`(macOS ⌃⌥ / Windows Ctrl+Alt), `Primary`(⌘ / Ctrl), `Shift`, `Alt`; 키는 영문·숫자·`Delete`·`Escape`·`/` 등. 범위: `global`(앱 수정키 조합만), `annotation`·`zoom`(앱 수정키 불가).
- **IF-05**: 이미 실행 중이면 새 프로세스가 인자를 `WM_COPYDATA`(Windows)로 전달하고 종료한다.
- **IF-06**: URL은 `https://music.youtube.com/`로 시작해야 한다. 크롬 쿠키를 읽지 않는다.

### 4.4 요청 (Path, Query, Body 스키마)
**IF-02 `.pansetting` 본문**
| 키 | 타입 | 설명 |
|---|---|---|
| `format` | string | `kr.datasci.edutools.pansetting` |
| `formatVersion` | int | 1 |
| `appVersion` | string | 만든 앱 버전 |
| `createdOS` | string | `macOS …` / `Windows 11` |
| `includedBundles` | string[] | 포함 묶음 |
| `includesComputerSpecific` | bool | 컴퓨터 전용 값 포함 여부 |
| `payload` | object | 묶음 이름 → 05 3.3의 묶음 스키마 |

**IF-04 `shortcuts-default.json`**: `{ "formatVersion": 1, "bindings": { "<정의 id>|<범위>": "<토큰>" }, "unbound": ["<정의 id>"] }`.

### 4.5 응답 (성공 / 실패 스키마)
| IF | 성공 | 실패 |
|---|---|---|
| IF-02 가져오기 | 변경 항목 미리보기 → 적용 → "이전 설정으로 되돌리기" | 형식·버전·범위·경로 규칙 위반 시 거부 + 사유, 현재 설정 유지 |
| IF-03 | 원자적 쓰기 완료 | 쓰기 실패 로그, 판서는 메모리에 유지 |
| IF-06 | 음악 시작 / 정지 | 크롬 없음→기본 브라우저, 권한 없음→미디어 키 |
| IF-08 | 파일·클립보드 + 완료 로그(`내보내기 완료 … file=…`) | `배경 캡처 실패` → 판서만 저장하고 알림 |

### 4.6 예시
```json
{
  "format": "kr.datasci.edutools.pansetting",
  "formatVersion": 1,
  "appVersion": "0.1.1",
  "createdOS": "Windows 11",
  "includedBundles": ["timer", "shortcuts"],
  "includesComputerSpecific": false,
  "payload": {
    "timer": { "musicSource": "folder", "musicFolderPath": "C:\\Music\\Break",
               "musicOrder": "name", "musicRepeat": "all" },
    "shortcuts": { "overrides": { "edit.clear-all|global": "AppMod+E" } }
  }
}
```
거부 예: `"musicFolderPath": "\\\\host\\share"` → 네트워크 경로(UNC) 거부.

### 4.7 연결 기능 ID / 화면 ID
| IF | FR | SCR |
|---|---|---|
| IF-01 | FR-ST-01, 04 | SCR-08 |
| IF-02 | FR-ST-03, 04 | SCR-08, SCR-12 |
| IF-03 | FR-SV-04, FR-DS-02 | SCR-11 |
| IF-04 | FR-ST-02, FR-SY-05, FR-HL-01 | SCR-08, SCR-09 |
| IF-05 | FR-SY-03, 04 | - |
| IF-06 | FR-MU-01~03 | SCR-07 |
| IF-07 | (전 기능, 내부) | - |
| IF-08 | FR-SV-01~03 | SCR-11 |

## 5. 데이터 스키마 정의
스키마의 정본은 [`05_data-model-erd.md`](05_data-model-erd.md) 3.3·5.1과 검증기(`SettingsValidator`·`SettingsRanges`)다. 값 범위·허용 목록·경로 규칙은 04 4.6.

## 6. 이벤트 / 웹훅
웹훅은 없다. Windows 내부 이벤트는 모듈별 `WM_APP + n`(`0x8000 + n`)으로 UI 스레드에 전달한다. **새 ID는 `rg`로 미사용 확인 후 사용**한다.
| 모듈 | n |
|---|---|
| 키보드 훅 / 라디얼·팔레트 | 3, 70~71 / 4~11 |
| 텍스트 입력 / 도움말 | 40~42 / 60~62 |
| 수업 배지 / 쉬는 시간 화면 | 100~101 / 160~161 |
| 음악 | 120(실행 결과), 121(일시정지 결과), 122(폴더 이벤트) |
| 줌 캡처·줌 창 | 140~145, 180 |
| 설정 창 | 220, 239~244 |
| 화면 구성 적용 | 240~241 |
| 발표 효과(마우스·키) | 300~301 |
| 내보내기 | 330(프레임), 331(실패), 332(풀 재생성), 333(작업 스레드 완료) |
| 비고 | 설정 창 일부(240~243)와 화면 구성 적용(240~241)의 대역이 숫자상 겹친다 — 창이 다르므로 현재 충돌은 없으나 신규 ID 부여 시 전수 확인 [확인 필요] |

macOS는 관찰자 레지스트리(`ObserverRegistry`)로 상태 변경을 통지한다.

## 7. 변경 및 폐기 정책
- 키 추가: 선택 필드·기본값·모르는 키 무시. 키 삭제·의미 변경: `formatVersion` 증가 + 이전 버전 변환 + 골든 시험 갱신.
- 단축키 정의 id는 변경하지 않는다(사용자 override 키가 된다). 폐기는 "기본 키 없음"으로 전환해 override를 보존한다.
- 앱 버전 `0.x`는 호환 약속이 느슨하나 설정 파일은 위 규칙을 지킨다.

## 변경 이력

| 버전 | 일자 | 변경 내용 | 작성자 |
|---|---|---|---|
| 0.1 | 2026-10-31 | 최초 작성(서버 API 없는 앱이므로 인터페이스 명세로 대체) | 이경환 (AI 초안) |
