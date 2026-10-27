# eduTools

수업용 macOS 메뉴바 판서 앱. 설계서: [`docs/design.md`](docs/design.md) · 진행: [`ROADMAP.md`](ROADMAP.md)

## 빌드 (Xcode 불필요 — Command Line Tools 만)

```sh
./build.sh          # dist/EduTools.app
./build.sh test     # 빌드 후 자체 점검(--selftest)
open dist/EduTools.app
```

macOS 26 이상.
