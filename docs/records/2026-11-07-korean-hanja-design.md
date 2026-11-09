# 한국어 한 글자 한자 변환과 KDE 실사용 피드백

> **기록, 상태: 구현됨(2026-11-07)** — 엔진, 후보창, Wayland·Windows 연결. 실제 KDE 재검증은 [KDE 시험 절차서](../testing/kde.md)의 K5·K6. 이 설계는 한국어 한 글자 변환만 다룬다.


## 확정한 범위

사용자가 Fedora KDE에서 `107bb4bbcc24bfc83b063cfd69c4a052b44fb616`의 한·영 입력이 동작한다고 보고했다.
언어 상태 박스가 나타날 때 하단 태스크바 전체가 위아래로 이동·리사이즈된다고 보고했다. 재부팅 전에는 실행 파일 버전만 6.8.2였으며 박스 등장·퇴장과 DBus visible=true/false 변화가 일치했고, 영상에서도 패널의 바닥 여백 변화가 확인됐다. 이후 easime 재설치 없이 재부팅해 KWin 6.8.2를 적용하자 태스크바 문제가 재현되지 않는다고 사용자가 확인했다. 재부팅 후 DBus 속성은 재측정하지 않았다.
사용자는 첫 한자 입력 범위로 **조합 중 한 글자 변환**을 선택했다.

## 한자 입력

- 한국어 상태에서 현재 조합 중인 한 글자에 한자 키를 누르면 후보창을 연다.
- 단어 전체나 이미 확정한 주변 문장은 이번 범위에서 바꾸지 않는다.
- 세로 목록에 번호와 후보를 표시하고 방향키로 이동한다. 긴 목록은 페이지로 나눈다.
- Enter는 선택 후보를 한 번만 확정하고 소비한다. 대상 앱으로 같은 Enter를 전달하지 않는다.
- Esc는 원래 한글 조합으로 돌아가며, 이어서 자모 단위 삭제·계속 입력이 가능해야 한다.
- 후보가 없거나 UI를 표시할 수 없으면 한글을 보존한다. 보이지 않는 후보를 확정하지 않는다.
- 조합 중이 아닐 때 한자 키는 기존 앱 동작을 방해하지 않는다.
- 후보 표시 중 상태 박스의 시간 만료가 후보창을 숨기지 않는다. 언어 전환·포커스 변경 시 후보는 닫는다.
- 한자 데이터는 출처·원본 버전·라이선스를 보존하고 Rust 엔진이 직접 조회한다. 기존 입력기 실행 파일이나 엔진 라이브러리에 의존하지 않는다.

단일 글자 변환은 기존 조합 구조를 보존하면서 한자 입력을 제공한다. 단어·선택 영역 변환은 확정 문자열의 편집과 복구까지 필요해 후속 범위로 남긴다.

## 태스크바 문제

현재 상태 박스는 `zwp_input_panel_surface_v1.set_overlay_panel`을 사용한다. 사용자가 시험한 커밋과 로컬 main의 해당 코드는 같다.
KWin Plasma/6.8 소스는 오버레이를 하단 가상 키보드의 창 재배치·가시성 계산에서 구분한다.
따라서 화면 하단 키보드 역할을 요청했다는 원인 가정은 현재 코드와 일치하지 않는다.

사용자 영상에서 하단 패널의 floating 여백 변화가 확인됐고, 재부팅해 KWin 6.8.2를 적용한 뒤 문제가 재현되지 않았다.
easime의 패널 역할을 바꾸는 우회 수정은 추가하지 않는다. 사용자 데스크톱 설정은 에이전트가 바꾸지 않는다.

참고: [KWin inputmethod.cpp](https://github.com/KDE/kwin/blob/Plasma/6.8/src/inputmethod.cpp),
[KWin inputpanelv1window.cpp](https://github.com/KDE/kwin/blob/Plasma/6.8/src/inputpanelv1window.cpp).

## 검증

엔진에서 후보 열기·이동·확정·취소·후보 없음·수정·언어 전환을 확인한다.
가짜 컴포지터에서 한자 키 처리, 후보 표시, Enter 소비, Esc 복귀와 타이머를 확인한다.
Mac에서는 OS 독립 crate를 실행하고 Linux 대상은 컴파일 검사한다. 기존 설치본의 태스크바 동작은 재부팅 후 사용자가 재확인했다. 새 한자 기능과 포커스 표시 변경은 수정본을 설치한 Fedora KDE에서 재확인해야 한다.

## 조사 및 검증 기록

- 한자 엔진 새 테스트 8개 실패를 확인한 뒤 구현했고, 현재 한자 동작 테스트 12개가 통과했다.
- OS 독립 crate 전체 188개 테스트, Mac에서 독립 실행한 기존 렌더링·키 변환 모듈 27개 테스트가 통과했다.
- Linux 대상 워크스페이스 전체와 테스트 타깃의 컴파일·clippy 검사를 통과했다. 후속 GitHub Actions에서 Linux 전체 테스트 294개(한자 통합 6개, 표시 테스트 18개 포함)가 통과했다. 새 기능의 실제 KDE 재검증은 남아 있다.
- 리뷰에서 일본어에 한국어용 페이지 안내가 노출되는 문제를 발견해, 이번 후보 UI 연결을 한국어로 한정했다.
- 태스크바 전체 이동은 KWin 6.8.1의 visible 오분류 → Plasma floating 상태 변경 경로와 일치한다. 사용자는 easime 재설치 없이 재부팅해 KWin 6.8.2를 적용하자 문제가 재현되지 않는다고 확인했다. [확인 기록](../testing/kde.md).

Host verification limitation: native Mac cargo test --workspace cannot compile the existing Linux-only memfd_create/timerfd/inotify imports. Linux-target all-target compilation and clippy passed locally; the full Linux suite subsequently passed in GitHub Actions.

## 후속 사용자 피드백: 포커스 진입 시 표시 제거

입력창 포커스가 잡힐 때의 언어 박스도 방해가 된다는 사용자 피드백에 따라, activate에서의 자동 표시를 제거한다.
언어 전환 때만 언어 박스를 표시하고 한자 후보창은 유지한다. 최초 포커스·일반 입력·재포커스로 표시되지 않으며, 명시적 전환 후에는 표시되는 것을 회귀 검사한다.
기존 크기·타이머·버퍼 재사용 검사는 먼저 전환 키를 눌러 표시하도록 갱신한다. 이는 태스크바 이동의 근본 원인 해결과는 구분한다.

Verification: Linux-target workspace/all-test-target clippy passed. The focused native test attempt could not compile the existing Linux-only memfd_create/timerfd/inotify calls on macOS; the subsequent remote Linux workspace suite passed, including the focus regression test.

Remote verification: [CI run 42342646904](https://github.com/byeonggonlee/easime/actions/runs/42342646904) passed Linux, macOS, Windows and Linux packaging before the main push.
