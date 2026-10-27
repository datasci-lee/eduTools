# eduTools

수업용 macOS 메뉴바 판서 앱. 설계서: [`docs/design.md`](docs/design.md) · 진행: [`ROADMAP.md`](ROADMAP.md)

## 빌드 (Xcode 불필요 — Command Line Tools 만)

```sh
./build.sh          # dist/EduTools.app
./build.sh test     # 빌드 후 자체 점검(--selftest)
open dist/EduTools.app
```

두 아키텍처(arm64·x86_64)를 합친 ad-hoc 서명 앱을 만들며, 외부 의존성과 Xcode는 사용하지 않습니다. macOS 26 이상에서 실행됩니다.
