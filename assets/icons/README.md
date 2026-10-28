# 앱 아이콘 (`<키>.svg`)

앱이 쓰는 아이콘입니다. 파일 이름 = **키**(뜻·표시 크기·키 목록은 [`docs/icons.md`](../../docs/icons.md) 가 정본). 파일이 있는 키는 이 아이콘을, 없는 키는 macOS 기본 아이콘(SF Symbols)을 씁니다.

| 규칙 | 값 |
| --- | --- |
| 형식 | SVG, `viewBox` 24×24, 단색 `currentColor`(디자인 세트 규칙: 선 1.75) |
| 색 | 모양만. 선택·호버·비활성·유리 위 색은 앱이 입힘(템플릿 이미지) |
| 선택 상태에서 모양이 달라질 때 | `<키>.selected.svg` 를 따로 |
| 읽혀야 하는 크기 | 10~14pt(@2x 20~28px) |

`./build.sh` 는 이 폴더의 SVG·PDF를 앱 번들의 `Contents/Resources/Icons/` 로 복사하고 복사 개수를 출력합니다. 앱은 키와 선택 상태로 아이콘을 한 번만 읽어 캐시하며, `<키>.selected.svg|pdf` 가 없으면 일반 아이콘을, 일반 아이콘도 없으면 기존 SF Symbol을 사용합니다. 앱 시작 때 알 수 없는 키나 일반 아이콘 짝이 없는 `.selected` 파일은 macOS Console의 `EduTools: unrecognized bundled icon asset names` 로그로 한 번 알립니다. 파일은 있지만 디코딩할 수 없으면 해당 파일명과 SF Symbol 폴백을 별도 로그로 남깁니다.

## 디자인 세트 배치
디자이너가 내보낸 `eduTools-symbols/` 폴더는 직접 넣지 말고 아래 명령으로 배치합니다(다시 내보낼 때도 같은 명령).

```
assets/import-symbols.sh <eduTools-symbols 폴더>
```

- `svg/` 중 [`../symbol-map.tsv`](../symbol-map.tsv) 에 있는 것(디자인 이름 → 앱 키) → `assets/icons/<키>.svg`
- 나머지 `svg/`(수업 운영 아이콘 등)·`attr/`(속성 레이어)·`windows/`(Windows PNG·트레이 ICO)·`README.md` → [`../symbols/`](../symbols/README.md) 에 원본 이름·구조 그대로 보관
- 새 이름을 앱 키에 연결하려면 `symbol-map.tsv` 에 한 줄을 더하고 다시 실행
