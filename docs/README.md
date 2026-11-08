# 문서 안내

모든 역할은 [AGENTS.md](../AGENTS.md)에서 시작한다. 필수 읽기는 [DESIGN.md](../DESIGN.md)와 [CLAUDE.md](../CLAUDE.md)이며, 아래는 작업에 따라 찾는 전체 문서 목록이다.
새 문서를 만들거나 옮기면 이 목록과 관련 문서의 링크도 갱신한다. 목록에 있다는 이유로 과거 계획이나 제안이 현재 결정이 되는 것은 아니다.

## 목적·판단·현재 상태

- [프로젝트 소개](../README.md): 설치 대상과 공개 지원 범위.
- [제품 컨셉](concept.md): 사용자, 핵심 경험, 장기 목적.
- [확정한 결정](decisions.md): 사용자 결정과 제품 정책.
- [디자인 지침](../DESIGN.md): 모든 역할이 적용하는 UX 원칙과 관찰.
- [2026-11-08 개발·디자인 평가](assessment-2026-11-08.md): 기준 커밋의 평가, 근거와 권장 진행 순서. 새 범위 승인 문서가 아니다.
- [2026-11-08 UI/UX 변경 리뷰와 개선](ux-review-2026-11-08.md): 최근 에이전트 변경 검토, 언어 순서·표시 시간·사전 안내 개선과 검증 범위.
- [진행 상황](status.md): 구현·검증 범위, 알려진 공백과 이어갈 일.

## 개발·검증

- [구조](architecture.md): 구성 요소, 프런트엔드, 설정과 패키징.
- [플랫폼 경계](platforms.md): 공유 코어와 OS 의존 구현·한계.
- [개발 방법](development.md): 의존성, 실행, 진단과 복구.
- [KDE 실제 환경 검증](kde-testing.md): 재현 절차와 사용자 확인 기록.
- [Windows 실제 환경 검증](windows-testing.md): 설치·입력·제거 절차와 기록 양식(결과는 아직 비어 있음).

## 특정 시점의 설계와 실행 계획

아래 문서는 당시 작업 기록이다. 완료 여부와 현재 지원은 진행 상황·코드·검증 기록을 우선 확인한다.

- [2026-11-07 한국어 한자 설계](superpowers/specs/2026-11-07-korean-hanja-design.md).
- [2026-11-07 한국어 한자 실행 계획](superpowers/plans/2026-11-07-korean-hanja.md).
- [2026-11-08 Windows 다음 단계 계획](windows-plan-2026-11-08.md): 등록 방식 실험, 일본어, 사전을 넣은 MSI, WinUI 3 설정 앱의 순서와 추천안.

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
