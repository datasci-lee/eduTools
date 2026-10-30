# 아이콘 목록

기본 모양은 **macOS 기본 아이콘(SF Symbols)** 이며, 같은 키의 파일이 [`assets/icons/`](../assets/icons/README.md) 에 있으면 그 파일을 대신 씁니다. "현재" 열은 에셋이 없을 때의 SF Symbol 폴백이고, "키" 는 실제 코드가 조회하는 이름입니다. 단축키 기능은 `ShortcutDefinition.id`, 원형 그룹은 원형 팔레트 정의의 키를 그대로 사용합니다. 원형 팔레트는 고정 어두운 면의 Minimal Arc 구조이며 그룹·범주 글자는 그리지 않습니다.

## 표시 크기 (아이콘이 실제로 그려지는 크기)
| 위치 | 크기(pt) | 비고 |
| --- | --- | --- |
| 원형 1링(그룹) | 13 | 아래 6개, 글자 없이 아이콘만 |
| 원형 2링(도구·명령) | 14 | 라벨 없음, 아이콘만 |
| 원형 3링(설정값) | 10 | 색·굵기·크기 미리보기와 굵기·크기 값 숫자, 별도 아이콘 없음 |
| 원형 중심 | 18 | 지금 도구의 아이콘, 색 테두리 안 |
| 가로 팔레트 버튼 | 32 칸 안 | 2링 아이콘과 같은 정의를 씀 |
| 도움말 줄 | 17 | 단축키 목록 왼쪽 |
| 메뉴바 | 시스템 표준 | 켜짐/꺼짐 2종 |

→ 10~14pt(@2x 로 20~28px)에서 읽혀야 하므로 단순한 형태, 한 가지 색(윤곽)으로 보내 주시면 됩니다. 색·선택·호버는 앱이 입힙니다.

## 1. 원형 1링 — 역할 그룹 (6)
| 키 | 뜻 | 현재 | 그룹에 들어 있는 것 |
| --- | --- | --- | --- |
| group.writing | 작성 | `pencil.and.outline` | 펜·형광펜·텍스트 |
| group.shapes | 도형 | `square.on.circle` | 화살표·사각형·원 |
| group.erasing | 지우기 | `eraser` | 부분·획·영역 지우개·실행 취소·다시 실행·전체 지우기 |
| group.board | 보드·설정 | `rectangle.on.rectangle` | 화이트·블랙보드·이전/다음 페이지·저장·복사·설정 |
| group.presenting | 발표 | `person.fill.viewfinder` | 레이저·스포트라이트·화면 가리개·통과 모드·줌·커서 강조 |
| group.favorites | 즐겨찾기 | `star.fill` | 지금 설정 저장 + 저장된 설정 최대 8개 |

## 2. 원형 2링 · 가로 팔레트 · 도움말 — 도구·명령
| 키 | 뜻 | 현재 |
| --- | --- | --- |
| tool.pen | 펜 | `pencil.tip` |
| tool.highlighter | 형광펜 | `highlighter` |
| tool.text | 텍스트 | `textformat` |
| tool.arrow | 화살표(한쪽/양쪽) | `arrow.right` |
| tool.rectangle | 사각형 | `rectangle` |
| tool.ellipse | 원 | `circle` |
| tool.eraser | 부분 지우개 | `eraser` |
| eraser.mode.stroke | 획 단위 지우개 | `eraser.line.dashed` |
| option.area.rectangle / option.area.lasso | 영역 지우개(현재 모양) | `rectangle.dashed` / `lasso` |
| edit.undo | 실행 취소 | `arrow.uturn.backward` |
| edit.redo | 다시 실행 | `arrow.uturn.forward` |
| edit.clear-all | 전체 지우기 | `trash` |
| board.white | 화이트보드 | `rectangle.fill` |
| board.black | 블랙보드 | `rectangle.inset.filled` |
| board.page.previous | 이전 보드 페이지 | `chevron.left` |
| board.page.next | 다음 보드 페이지 | `chevron.right` |
| export.save | PNG 저장 / 보드 PDF 저장 | `square.and.arrow.down` |
| export.copy | 클립보드 복사 | `doc.on.doc` |
| palette.settings | 설정 열기 | `gearshape` |
| tool.laser | 레이저 | `laser.burst` |
| spotlight.toggle | 스포트라이트 | `light.beacon.max` |
| tool.cover | 화면 가리개 | `rectangle.fill` |
| annotation.pass-through | 아래 화면 클릭(통과 모드, `U`를 누르고 있는 동안) | `cursorarrow.click` |
| zoom.toggle | 화면 줌 | `plus.magnifyingglass` |
| cursor-highlight.toggle | 커서 강조 | `cursorarrow.rays` |

## 3. 원형 3링 — 색·굵기·크기 미리보기
| 키 | 뜻 | 현재 |
| --- | --- | --- |
| — | 3링은 색 원·굵기 막대·글자 크기 `가`·지우개 크기 원만 표시 | 별도 아이콘 없음 |

## 4. 중심 원 (지금 도구 아이콘 = 위 2링의 같은 키)
선택·이동(`tool.select`) 도 중심에 나옵니다: `lasso`.

## 5. 팔레트에는 안 나오지만 가로 팔레트·도움말 목록에 쓰이는 것
| 키 | 뜻 | 현재 |
| --- | --- | --- |
| annotation.toggle | 판서 모드 켜기/끄기 | `pencil` |
| tool.select | 올가미 선택·이동 | `lasso` |
| tool.shape-recognition | 손그림 도형 자동 보정 | `wand.and.stars` |
| help.show | 도움말 | `questionmark.circle` |
| palette.style.radial | 팔레트 모양: 원형 | `circle.grid.3x3.fill` |
| palette.style.linear | 팔레트 모양: 가로 | `rectangle.split.3x1` |
| eraser.area-shape | 영역 지우개 사각형/올가미 | `rectangle.dashed` |
| tool.width.decrease / increase | 현재 도구 크기 줄이기 / 키우기 | `minus` / `plus` |
| color.1 ~ color.6 | 색상 1~6 (가로 팔레트에서는 색 원을 직접 그림) | `circle.fill` |
| edit.delete-selection | 선택한 항목 삭제 | `delete.left` |
| palette.toggle | 도구 팔레트 숨기기/보이기 | `rectangle.on.rectangle` |
| palette.radial.toggle | 원형 팔레트 열기/접기(M) | `circle.grid.cross` |
| annotation.step-back | 한 단계 나가기(Esc) | `escape` |
| zoom.increase / decrease / reset | 줌 배율 올리기 / 내리기 / 시작 배율 | `plus.magnifyingglass` / `minus.magnifyingglass` / `1.magnifyingglass` |
| zoom.freeze | 화면 멈추고 판서 | `pause.circle` |
| zoom.preview | 확대 위치 미리보기 | `rectangle.dashed` |
| zoom.follow | 따라가기 방식 바꾸기 | `cursorarrow.motionlines` |
| zoom.stop | 줌 종료 | `escape` |
| keystroke-display.toggle | 키 입력 표시 | `command` |
| timer.lesson.toggle | 수업 타이머 | `timer` |
| timer.break.open | 쉬는 시간 타이머 | `cup.and.saucer` |

## 6. 메뉴바
| 키 | 뜻 | 현재 |
| --- | --- | --- |
| status.annotation.off | 판서 모드 꺼짐 | `pencil` |
| status.annotation.on | 판서 모드 켜짐 | `pencil.circle.fill` |

## 7. 같은 아이콘을 두 번 쓰는 곳 (디자인할 때 구분해 주세요)
| 아이콘 | 겹치는 뜻 |
| --- | --- |
| `rectangle.fill` | 화이트보드 ↔ 화면 가리개 (같은 모양이라 구분이 안 됨) |
| `rectangle.dashed` | 영역 지우개 사각형 ↔ 확대 위치 미리보기 |
| `rectangle.on.rectangle` | 보드·설정 그룹 ↔ 도구 팔레트 숨기기 |
| `plus.magnifyingglass` | 화면 줌 켜기 ↔ 줌 배율 올리기 |
| `escape` | 한 단계 나가기 ↔ 줌 종료 |
| `arrow.right` | 화살표 도구 ↔ 화살표 한쪽 모양 |
| `pencil` / `pencil.tip` / `pencil.and.outline` | 판서 모드 / 펜 / 작성 그룹 — 세 가지가 비슷해 보임 |

## 8. 아이콘 연결 방식
- 파일: **SVG**(한 색, 윤곽 또는 채움, `viewBox` 24×24 권장) 또는 PDF 벡터. 파일 이름 = 위 "키" (예: `tool.pen.svg`).
- `./build.sh` 가 파일을 앱 번들의 `Contents/Resources/Icons/` 로 복사합니다.
- 앱은 **키별로 새 아이콘이 있으면 쓰고, 없으면 지금 SF Symbols 를 그대로** 씁니다. 일부만 먼저 바꿔도 됩니다.
- 색·선택·호버·비활성 표현은 앱이 입히므로 **단색 모양만** 주시면 됩니다(템플릿 이미지로 사용).
- 선택 상태에서 모양이 바뀌는 아이콘(예: 속이 빈/찬 모양)이 필요하면 `<키>.selected.svg` 로 따로 주세요.
- 앱 시작 때 키 목록에 없거나 일반 파일 짝이 없는 `.selected` 파일은 `NSLog` 로 한 번 알립니다. 파일이 있으나 디코딩할 수 없을 때도 경고 후 SF Symbol로 폴백합니다.

## 9. 디자인 적용 현황 (2026-10-30)
- 디자인 세트(`eduTools-symbols`)로 **57개 키**가 `assets/icons/<키>.svg` 로 연결됩니다(대응표 `assets/symbol-map.tsv`).
- **디자인이 아직 없는 키**(SF Symbols 유지): `annotation.pass-through`(`U` 통과 모드, 2026-10-28 신설), `eraser.area-shape`(가로 팔레트의 영역 모양 전환), `palette.radial.toggle`(`M`), `color.1`~`color.6`(앱이 색 원을 직접 그림). 앞의 세 키는 기존 아이콘을 재사용하려면 `symbol-map.tsv` 에 한 줄(예: 디자인 `palette.radial` 을 두 키에 연결)만 더하면 됩니다.
- **속성 레이어**는 `assets/symbols/attr/manifest.json`과 `svg/`의 47개 파일을 `Resources/Icons/Attr/`로 복사해 원형 중심·2링 도구·가로 팔레트에서 합성합니다. 색·굵기·화살촉·지우개 방식·레이저 꼬리를 현재 또는 도구별 마지막 설정에서 읽으며, 불완전한 도구 세트는 로그 후 위 일반 아이콘으로 폴백합니다. halo용 배경은 원형에서 선택 시 시스템 강조색·그 밖은 고정 어두운 면 0.07, 가로 팔레트에서 선택 시 시스템 강조색·그 밖은 시스템 모양에 따른 흰색 0.93/0.18로 근사합니다.
- **쓰이지 않는 디자인**은 `assets/symbols/` 에 보관: 수업 운영 아이콘 50개(`class.` `attn.` `act.` `aid.` `stamp.` `bg.` `dev.`), Windows 용 PNG·트레이 ICO.

