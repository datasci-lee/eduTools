# 크래시 회귀 대장

## 목적과 사용법

크래시마다 가능한 가장 낮은 계층에 재현 시험을 추가하고 이 표에 한 줄을 남긴다. 날짜·리포트, 사용자 증상과 확인된 원인, 수정 PR·커밋, 재현 시험 이름, 계층을 함께 기록하며 시험이 이관되면 이름/ID를 갱신한다.

샌드박스 시험 실행 크래시는 사용자 앱 크래시와 분리한다. `.ips`의 `parentProc`가 `Exited process`이고 스택에 `xctest`가 있으면 시험 실행이다. 사용자가 실행한 앱 프로세스는 `launchd`의 자식이다. WindowServer가 없는 환경의 T3/T4는 실패로 위장하지 않고 `UNRUNNABLE`로 기록한다.

> 현재 저장소에는 아래 원본 `.ips`가 없다. 리포트의 시각·스택 사실은 작업 지시서에서 옮겼고, 수정 PR·커밋·시험명은 `git log`, `git show f3cf5cc 8d4c0d8`, 현재 소스로 대조했다.

| 날짜 | 리포트 | 증상·원인 | 수정(PR) | 재현 시험 | 계층 |
| --- | --- | --- | --- | --- | --- |
| 2026-10-29 08:13:50 | `EduTools-2026-10-29-081350.ips` | 화면 구성 변경 직후 `ArchiveController.restoreDisconnectedDisplayIfNeeded`의 `NSAlert.runModal` 모달 루프가 처리한 `mouseEntered`에서 `InkView.usesEraser`가 `NSEvent.pointingDeviceType`을 읽어 `NSInternalInconsistencyException` → `SIGABRT`. | #50 `f3cf5cc`; 후속 #54 `8d4c0d8` | `UnitTests.PointerEventsTests/pointingDeviceQueryPermissionMatchesEventKind(_:)`<br>`GUITests.PointerEnterExitTests/enterAndExitAvoidPointingDeviceQueries()` | T1 + T3 |
| 2026-10-28 14:04:08 | `EduTools-2026-10-28-140408.ips` | **환경 한계, 제품 결함 아님.** Codex 샌드박스에서 `NSApplication init` → `HIServices _RegisterApplication`이 WindowServer 연결 불가로 abort. | 없음 | 없음. T3/T4는 샌드박스에서 `UNRUNNABLE` 처리 | 환경/T3·T4 |
| 2026-10-29 08:28~10:16 | 샌드박스 `./build.sh test` 크래시 11건 | 합성 마우스 이벤트 속성을 읽다가 발생한 시험 실행 예외. 사용자의 앱 프로세스 크래시가 아님. | #54 `8d4c0d8` | 위 T1 회귀 시험에서 WindowServer가 필요한 합성 마우스 이벤트 읽기를 제거하고 진입·이탈 가드와 이벤트 종류 표를 검사 | T1 |
