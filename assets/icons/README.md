# 앱 아이콘 (`<키>.svg`)

앱이 쓰는 아이콘입니다. 파일 이름 = **키**(뜻·표시 크기·키 목록은 [`docs/icons.md`](../../docs/icons.md) 가 정본). 파일이 있는 키는 이 아이콘을, 없는 키는 macOS 기본 아이콘(SF Symbols)을 씁니다.

| 규칙 | 값 |
| --- | --- |
| 형식 | SVG, `viewBox` 24×24, 단색 `currentColor`(디자인 세트 규칙: 선 1.75) |
| 색 | 모양만. 선택·호버·비활성·유리 위 색은 앱이 입힘(템플릿 이미지) |
| 선택 상태에서 모양이 달라질 때 | `<키>.selected.svg` 를 따로 |
| 읽혀야 하는 크기 | 10~14pt(@2x 20~28px) |

## 디자인 세트 배치
디자이너가 내보낸 `eduTools-symbols/` 폴더는 직접 넣지 말고 아래 명령으로 배치합니다(다시 내보낼 때도 같은 명령).

```
assets/import-symbols.sh <eduTools-symbols 폴더>
```

- `svg/` 중 [`../symbol-map.tsv`](../symbol-map.tsv) 에 있는 것(디자인 이름 → 앱 키) → `assets/icons/<키>.svg`
- 나머지 `svg/`(수업 운영 아이콘 등)·`attr/`(속성 레이어)·`windows/`(Windows PNG·트레이 ICO)·`README.md` → [`../symbols/`](../symbols/README.md) 에 원본 이름·구조 그대로 보관
- 새 이름을 앱 키에 연결하려면 `symbol-map.tsv` 에 한 줄을 더하고 다시 실행

앱 연결(번들 복사·키 조회·SF Symbols 폴백)은 `build.sh` 와 `AppIcon` 이 담당합니다.
