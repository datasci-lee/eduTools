# eduTools 심볼 (Mac·Windows 공용)

- 격자 24×24, 선 1.75, 단색. 원본은 `svg/` 이고 색은 `currentColor`.
- 파일 이름은 추정 키입니다. `eduTools-icon-list.md` 의 실제 키와 다르면 이름만 바꿔 쓰세요.

## Mac
- `svg/` 를 에셋 카탈로그에 넣고 Preserve Vector Data 켬, Render As: Template Image.
- 메뉴바는 `menubar.off`, `menubar.on` (템플릿이라 다크·라이트 자동).

## Windows
- `windows/png/<크기>/` : 검정 PNG. 16·20·24·32·48 = 배율 100·125·150·200·300%(16 기준).
  색을 입힐 때는 OpacityMask(WPF) 등으로 모양만 쓰세요.
- `windows/tray/` : 작업 표시줄 트레이 ICO. 작업 표시줄이 밝으면 `light`, 어두우면 `dark`.
- 단축키 표시는 Mac `key.command`, Windows `key.ctrl` 을 쓰거나 "Ctrl" 글자로 표시하세요.

## 교육 운영 아이콘 (50개)
접두어로 묶었습니다: `class.`(수업 흐름) `attn.`(주목·관리) `act.`(수업 활동) `aid.`(판서 보조) `stamp.`(도장) `bg.`(판 배경) `dev.`(화면·장치).
- `stamp.*` 는 판 위에 찍히는 내용이라 UI 색이 아니라 **도장 색(attr)** 으로 칠하세요. 명암 보정 규칙도 attr/README.md 와 같습니다.
- `bg.*` 는 판 배경 서식(모눈·원고지·오선지 등) 고르기용입니다.
