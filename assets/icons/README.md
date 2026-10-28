# 아이콘 에셋

디자인한 아이콘 파일을 여기에 둡니다. 목록·뜻·표시 크기는 [`docs/icons.md`](../../docs/icons.md) 가 정본입니다.

| 규칙 | 값 |
| --- | --- |
| 형식 | SVG(권장, `viewBox` 24×24, 단색) 또는 PDF 벡터 |
| 파일 이름 | `docs/icons.md` 의 **키** + 확장자. 예: `tool.pen.svg`, `group.writing.svg` |
| 색 | 한 가지 모양만. 색·선택·호버·비활성은 앱이 입힘(템플릿 이미지) |
| 선택 상태에서 모양이 달라질 때 | `<키>.selected.svg` 를 따로 |
| 읽혀야 하는 크기 | 10~14pt(@2x 20~28px). 선은 굵게, 디테일은 단순하게 |
| 부분 교체 | 파일이 있는 키만 새 아이콘, 없는 키는 지금의 SF Symbols |

`./build.sh` 는 이 폴더의 SVG·PDF를 앱 번들의 `Contents/Resources/Icons/` 로 복사하고 복사 개수를 출력합니다. 앱은 키와 선택 상태로 아이콘을 한 번만 읽어 캐시하며, `<키>.selected.svg|pdf` 가 없으면 일반 아이콘을, 일반 아이콘도 없으면 기존 SF Symbol을 사용합니다. 앱 시작 때 알 수 없는 키나 일반 아이콘 짝이 없는 `.selected` 파일은 macOS Console의 `EduTools: unrecognized bundled icon asset names` 로그로 한 번 알립니다. 파일은 있지만 디코딩할 수 없으면 해당 파일명과 SF Symbol 폴백을 별도 로그로 남깁니다.
