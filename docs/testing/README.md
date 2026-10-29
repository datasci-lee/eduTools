# EduTools 시험 빠른 시작

앱 번들은 기존 `build.sh`가 만들고, SwiftPM은 시험 전용으로 사용한다. 헤드리스 T1·T2, 사용자 맥의 WindowServer에서 실행하는 T3 GUI, 실제 앱을 격리 실행하는 T4 E2E 시험을 제공한다.

## 실행

```bash
scripts/test.sh unit
scripts/test.sh integration
scripts/test.sh gui
scripts/test.sh e2e
scripts/test.sh unit --filter 'ShortcutTests'
scripts/test.sh unit --coverage
scripts/test.sh all --xunit .build/test-results.xml
./build.sh test
scripts/verify-bundle.sh [--app <path>] [--dmg <path>]
```

`unit integration`처럼 두 계층을 함께 지정할 수도 있다. `--filter` 정규식은 선택된 시험 타깃의 전체 시험 ID에서 검색한다. `--coverage`는 시험 파일을 제외한 `EduTools` 소스별 `llvm-cov report` 표를 마지막에 출력한다. `gui`는 사용자 맥의 WindowServer 전용이다. `e2e`는 손쉬운 사용·화면 기록 권한이 있는 사용자 맥에서 실제 번들을 제어하며 [`e2e.md`](e2e.md)의 격리·위험 규칙을 따른다. 환경이 부족하면 둘 다 `UNRUNNABLE`과 종료코드 3을 반환한다. `all`과 `./build.sh test`는 GUI·E2E를 포함하지 않고 단위·통합 시험만 실행한다.

릴리스 전에는 `./build.sh dmg`를 실행한 뒤 `scripts/verify-bundle.sh`로 앱 번들과 DMG를 검증한다. 검증 스크립트는 앱 번들에 자체 시험 코드가 없으면 PASS, 발견되면 FAIL로 판정한다.

실제 장비·권한·사용자 흐름은 [`manual-checklist.md`](manual-checklist.md)에서 확인하고, 크래시의 증상·원인·수정·재현 시험은 [`crash-regressions.md`](crash-regressions.md)에 기록한다.

## `scripts/test.sh` 종료코드와 결과

아래 표는 `scripts/test.sh`의 종료코드 규약이다.

| 코드 | 뜻 | 조치 |
| --- | --- | --- |
| 0 | 선택한 시험 통과 | 출력된 시험·확인 수를 기록한다. |
| 1 | 빌드 또는 시험 실패 | 첫 컴파일 오류나 실패 시험 ID부터 확인한다. |
| 3 | UNRUNNABLE | 출력된 툴체인·런타임 부족 사유를 해소한 뒤 다시 실행한다. |
| 4 | 사용법 오류 | 지원 계층과 옵션을 확인한다. |

`scripts/verify-bundle.sh`도 같은 0/1/3/4 규약(통과/검증 실패/UNRUNNABLE/사용법 오류)을 따른다.

Swift Testing 출력의 `Test ... failed`가 실패한 시험이며, `#expect` 진단은 실제값과 기대값을 함께 보여 준다. xUnit 파일은 외부 결과 수집용이고 터미널 종료코드가 게이트의 정본이다. 커버리지는 빈 영역을 찾는 정보용 지도이지 통과 기준이 아니다.

## 새 시험 쓰는 법

1. 위험 인벤토리에서 근거와 가장 낮은 계층(T1 또는 T2)을 고른다.
2. 영역별 파일에 `@Suite("영역")`와 `@Test("입력에서 기대까지 한 문장")`을 쓴다.
3. 기대값은 설계·표·기존 계약에서 가져오고 구현 결과로 다시 계산하지 않는다.
4. 시계·난수·경로는 주입하며 T2 파일은 `TempDir`과 고유 UserDefaults suite를 사용한다.
5. 실패를 일으키는 수동 변이로 검출력을 확인한 뒤 원복하고 전체 게이트를 실행한다.
