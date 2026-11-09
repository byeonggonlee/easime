# 문서 안내

모든 역할은 [AGENTS.md](../AGENTS.md)에서 시작한다. 필수 읽기는 [DESIGN.md](../DESIGN.md)와 [CLAUDE.md](../CLAUDE.md)이며, 아래는 작업에 따라 찾는 전체 문서 목록이다.
새 문서를 만들거나 옮기면 이 목록과 관련 문서의 링크도 갱신한다. 목록에 있다는 이유로 과거 계획이나 제안이 현재 결정이 되는 것은 아니다.

문서는 네 종류로 나뉜다. 어느 종류인지에 따라 믿는 방법이 다르다.

| 종류 | 위치 | 성격 |
|---|---|---|
| 방향·결정 | `concept`, `decisions`, `DESIGN` | 사용자가 정한 것과 지침. 새 제안으로 조용히 바꾸지 않는다 |
| 현재 상태 | `status`, `implementation/` | 지금 코드와 검증 범위. 코드가 바뀌면 함께 고친다 |
| 구조·방법 | `architecture`, `platforms`, `development`, `testing/` | 어떻게 짜여 있고 어떻게 만들고 시험하는가 |
| 기록 | `records/` | 특정 날짜의 평가·계획·설계. 당시 근거로 남기며 고치지 않는다 |

## 목적·결정·현재 상태

- [프로젝트 소개](../README.md): 무엇인지, 어디까지 되는지, 어떻게 써 보는지(영어).
- [제품 컨셉](concept.md): 사용자, 핵심 경험, 장기 목적.
- [확정한 결정](decisions.md): 사용자 결정과 제품 정책, 결정별 구현 상태.
- [디자인 지침](../DESIGN.md): 모든 역할이 적용하는 UX 원칙과 관찰.
- [진행 상황](status.md): 영역별 구현·자동 검증·실제 환경 확인 현황판, 알려진 한계, 이어서 할 일.

## 구현 현황

무엇이 구현되어 있고 어떻게 확인했는지(자동 검증과 실제 환경을 구분)를 영역별로 적는다.

- [공통 코어와 엔진](implementation/core.md): `ime-core`, `ime-session`, 한글·일본어 엔진, 사전, 설정.
- [Linux (KDE Plasma 6)](implementation/linux-kde.md): 데몬, 상태 박스·트레이, 설정 창, 활성화, 패키지.
- [Windows](implementation/windows.md): TSF 텍스트 서비스, 등록, 설치 프로그램, 설정 앱, CI가 확인하는 것.

## 구조와 개발

- [구조](architecture.md): 플랫폼별 연결 방식, 크레이트 구성, 핵심 trait, 설정 GUI, 패키징.
- [플랫폼 경계](platforms.md): 공유 코어와 OS 의존 구현, 새 OS 어댑터가 제공할 것.
- [개발 방법](development.md): 의존성, 빌드·실행, CI 구성, 진단과 복구.

## 실제 환경 시험

CI가 대신할 수 없는 시험의 절차서이자 결과 기록 양식이다. 결과 표에는 지금까지의 보고만 있다: Windows는 2026-11-09 W1–W14 문제 없음(상세 미기록)과 크기 피드백, KDE는 크기 보고뿐이다.

- [KDE 시험 절차서](testing/kde.md): 한국어·일본어·트레이·설정·복구 시험과 하단 패널 문제의 확인 기록.
- [Windows 시험 절차서](testing/windows.md): 설치·입력·설정 앱·제거·작업 표시줄 단추·언어 전환 시험(W1–W20).

## 특정 시점의 기록

아래 문서는 당시 작업 기록이다. 완료 여부와 현재 지원은 [진행 상황](status.md)·코드·검증 기록을 우선 확인한다.

- [2026-11-07 한국어 한자 설계](records/2026-11-07-korean-hanja-design.md), [실행 계획](records/2026-11-07-korean-hanja-plan.md): 조합 중 한 글자 한자 변환(구현됨).
- [2026-11-08 개발·디자인 평가](records/2026-11-08-assessment.md): 기준 커밋의 평가, 근거와 권장 진행 순서. 새 범위 승인 문서가 아니다.
- [2026-11-08 UI/UX 변경 리뷰와 개선](records/2026-11-08-ux-review.md): 언어 순서·표시 시간·사전 안내 개선과 검증 범위.
- [2026-11-08 Windows 다음 단계 계획](records/2026-11-08-windows-plan.md): 등록 방식 실험(결과와 되돌림), 일본어, 사전, 설정 앱의 순서. 진행 순서는 모두 끝났다.

## 에이전트·기여 양식

- [공통 에이전트 지침](../AGENTS.md), [Claude 진입점](../CLAUDE.md).
- [버그 보고 양식](../.github/ISSUE_TEMPLATE/bug_report.md).
- [기능 제안 양식](../.github/ISSUE_TEMPLATE/feature_request.md).
- [PR 양식](../.github/PULL_REQUEST_TEMPLATE.md): 양식의 존재는 v0.1.0 전 `main` 직접 푸시 정책을 바꾸지 않는다.

## 데이터와 라이선스

- [외부 데이터 라이선스](../THIRD_PARTY_LICENSES.md).
- [한자 데이터 설명](../data/hanja/README.md), [한자 데이터 라이선스](../data/hanja/LICENSE).
- [Mozc 데이터 라이선스](../data/mozc/LICENSE).
- [프로젝트 MIT 라이선스](../LICENSE-MIT), [프로젝트 Apache 2.0 라이선스](../LICENSE-APACHE).
