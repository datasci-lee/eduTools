# eduTools

수업용 macOS 메뉴바 판서 앱. 설계서: [`docs/design.md`](docs/design.md) · 진행: [`ROADMAP.md`](ROADMAP.md) · 문서 색인: [`docs/README.md`](docs/README.md)

## 설치

1. `EduTools-<버전>.dmg`를 열고 `EduTools.app`을 `Applications` 링크로 끌어다 놓습니다.
2. 공증 전 로컬 빌드처럼 신뢰할 수 있는 출처인데도 macOS가 실행을 막으면 다음 명령으로 격리 속성을 제거합니다.

```sh
xattr -dr com.apple.quarantine /Applications/EduTools.app
```

## 빌드 (Xcode 불필요 — Command Line Tools 만)

```sh
./build.sh          # dist/EduTools.app
./build.sh test     # 앱 빌드 후 단위·통합 시험
./build.sh dmg      # dist/EduTools-<버전>.dmg
open dist/EduTools.app
```

두 아키텍처(arm64·x86_64)를 합친 유니버설 앱을 만들며, 외부 의존성과 Xcode는 사용하지 않습니다. macOS 26 이상에서 실행됩니다. `EDUTOOLS_SIGN_IDENTITY`가 없으면 ad-hoc 서명하고, 있으면 hardened runtime과 Apple Events entitlement를 적용합니다. `EDUTOOLS_NOTARY_PROFILE`도 있으면 DMG를 공증하고 staple합니다.

```sh
EDUTOOLS_SIGN_IDENTITY="<codesign identity>" \
EDUTOOLS_NOTARY_PROFILE="<notarytool keychain profile>" \
./build.sh dmg
```

## 필요한 권한

| 권한 | 쓰는 기능 | 없을 때 |
| --- | --- | --- |
| 화면 및 시스템 오디오 녹음 | 화면 줌, PNG 저장(배경 포함) | 권한 안내 후 해당 기능 제한 |
| 손쉬운 사용 | 키 입력 표시, 클릭 강조, 판서 중 슬라이드 넘김 키 전달, 통과 모드(`U` 누르는 동안)에서 아래 앱 입력창에 `u` 가 찍히는 것 방지 | 해당 기능만 꺼짐(통과 모드는 그대로 동작) |
| 자동화(Google Chrome 제어) | 쉬는 시간 종료 때 앱이 연 음악 탭 닫기 | 미디어 키 방식으로 대체 |

상세 동작은 설계서의 [필요한 권한](docs/design.md#필요한-권한)을 참고합니다. 전역 단축키 자체는 별도 권한이 필요하지 않습니다.

자동 업데이트: 배포 주소 확정 후
