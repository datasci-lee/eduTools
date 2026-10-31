# EduTools 문서

소프트웨어 개발 문서 체계(`03.02`)에 맞춘 문서 11종과 기존 정본 문서를 모은 색인이다. 문서는 **초안 v0.1**이며 사용자 승인 전이다. `[가정]`·`[확인 필요]` 표시는 승인 전에 해소한다.

## 03.02 문서 (작성 순서)
| 순서 | 문서 | 내용 |
|---|---|---|
| 01 | [BRD](01_BRD.md) | 사업 목표·이해관계자·요구사항(BR)·범위 |
| 02 | [PRD](02_PRD.md) | 제품 요구사항(PR)·KPI·MoSCoW·릴리스 |
| 03 | [기능 요구사항 정의서](03_functional-requirements.md) | 기능 ID(FR)·규칙·추적표 |
| 04 | [화면 설계서](04_screen-design.md) | 화면(SCR)·흐름·사용자 스토리·인수 조건 |
| 05 | [데이터 모델 / ERD](05_data-model-erd.md) | 설정·판서·보관 파일 구조 |
| 06 | [기술 스택 결정서](06_tech-stack-decision.md) | Swift·Rust 선택 근거 |
| 07 | [API(인터페이스) 명세서](07_api-spec.md) | 파일 형식·단축키 토큰·프로세스·외부 연동 |
| 08 | [작업 실행 계획](08_task-plan.md) | 마일스톤·남은 태스크·일정 |
| 09 | [테스트 계획서](09_test-plan.md) | 계층·추적표·완료 기준·결과 |
| 10 | [회고](10_retrospective.md) | 0.1.x 릴리스 회고 |

선후행: 01 → 02 → 03 → 04·05 → 06 → 07 → 08 → 09 → 10.

## 기존 정본 문서 (중복하지 않고 링크)
| 문서 | 역할 |
|---|---|
| [design.md](design.md) | 기능·단축키·화면 규격의 정본 |
| [windows-plan.md](windows-plan.md) | Windows 구현 인계서·결정 표 |
| [icons.md](icons.md) | 아이콘 키·에셋 규칙 |
| [testing/](testing/README.md) | 시험 체계(전략·E2E·수동 체크리스트·크래시 대장) |
| [../ROADMAP.md](../ROADMAP.md) | PR 단위 진행 실적 |
