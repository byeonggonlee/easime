# easime 진행 상황

개발 단계, 현재 상태, 중단된 작업. 코드가 바뀌면 이 문서를 같이 고친다.

## 개발 단계
1. **ime-core + 한글 엔진** — 단위 테스트를 먼저, 충분히 작성. OS 독립으로 검증.
2. **CLI 데모** — 터미널에서 엔진을 수동으로 확인.
3. **Wayland IM 프론트엔드 (KDE)** — KWin 연결, preedit/commit, 키 전달, 한/영 전환.
4. **상태 표시** — 트레이 아이콘 + 커서 아래 상태 박스 (panel surface).
5. **설정 GUI (Kirigami) + KDE 통합** — `easime-config`, `.desktop`, 자동 실행, D-Bus로 설정 다시 읽기.
6. **Linux 패키징** — GitHub Actions, .deb/.rpm, GitHub Releases.
7. **XIM 프론트엔드** — XWayland 앱(게임/Wine/Steam/IDE).
8. **일본어 1단계** — 로마자 → 가나, 언어 순환에 추가.
9. **후보창** → **일본어 2단계** (Mozc 사전, 학습).
10. **GNOME** — IBus 호환 D-Bus 서비스 (조사 먼저).
11. **Windows** — TSF 프론트엔드, WinUI 3 설정 앱, MSI.
12. **중국어** (장기) — 병음 엔진, 간체/번체.
13. **macOS** — InputMethodKit, SwiftUI 설정 앱, pkg.

## 현재 상태 (2026-11-07)
- 완료
  - `ime-core`(키 이벤트, `InputEngine`, `Language`/`LanguageSwitcher`), `easime-config`(TOML 설정, OS별 경로, 원자적 저장)
  - `engine-hangul`: 두벌식, 겹모음/겹받침, 자모/글자 단위 백스페이스. 조합 중 한 글자 한자 변환 코드와 후보창 추가(실제 KDE 재검증 필요). 한자 키 → ↑↓/Space 이동, PgUp/PgDn 페이지, Enter/1–9 확정, Esc/Backspace로 한글 복귀.
  - `engine-japanese`: 로마자 → 히라가나/가타카나, Space로 변환 시작(후보 순환 Space/↑↓, Enter 확정, Esc/Backspace 가나로 복귀, 숫자 선택).
    사전: `Dictionary` trait, `MemoryDictionary`(TSV), `FileDictionary`(이진, 로드 약 0.1초), `LayeredDictionary`, `load_default()`.
    **데몬에는 아직 연결하지 않았다** (연결 시 시작할 때 한 번 `engine_japanese::load_default()` 호출). 문절 분할/문맥 변환은 없다(단어 단위).
  - `easime-dict-tool`: Mozc OSS 사전 → easime 이진 사전 변환기. `scripts/fetch-mozc-dictionary.sh`(고정 SHA, 체크섬 검증),
    `scripts/build-dictionary.sh sample|mozc`. 라이선스(BSD-3, IPAdic, 오키나와 사전)는 `THIRD_PARTY_LICENSES.md`와 `data/mozc/`.
  - 설정 즉시 반영: 설정 파일을 inotify로 감시해 한/영 키, 백스페이스 단위, 언어 목록을 재시작 없이 적용(깨진 파일은 무시)
  - 패키징: `.deb`/`.rpm`(`crates/easime/Cargo.toml` 메타데이터), CI `packages` 잡(샘플 사전), 태그 `v*` 푸시 시 릴리스 워크플로(실제 Mozc 사전 포함)
  - `frontend-wayland` + `easime` 데몬: KWin `zwp_input_method_v1` 클라이언트, 키 반복(자체 타이머와 컴포지터 `repeated`),
    한/영 키와 오른쪽 Alt 탭 전환, Caps Lock 무시, 언어 순환, 설정 읽기
  - 커서 옆 상태 박스(`frontend-wayland/src/{indicator,font,panel}.rs`): 사용자가 언어를 전환할 때만 `A`/`한`/`あ`/`中`
    48x48 박스를 `indicator_ms`(기본 1000) 동안 표시. `zwp_input_panel_surface_v1.set_overlay_panel`(KWin이 커서 사각형 바로 아래에 배치,
    키보드 포커스 안 가짐, 빈 input region), 표시/숨김은 버퍼 attach/null attach, `[appearance] indicator_box`로 끄기.
    사용자가 Fedora KDE의 `107bb4b`에서 한·영 입력 성공을 보고했다. 하단 패널 전체 이동은 easime 재설치 없이 재부팅해 KWin 6.8.2를 적용한 뒤 재현되지 않는다고 확인했다. 포커스 시 박스 표시 제거는 로컬 수정본 설치 후 검증이 필요하다. text-input-v1 앱은 커서 위치를 보내지 않아 창 왼쪽 위에 뜰 수 있다.
  - CI: Linux는 전체, Windows/macOS는 OS 독립 crate만 (`fmt`, `clippy -D warnings`, `test`)
- **가짜 KWin 테스트 하네스**: `crates/frontend-wayland/tests/` (순수 Rust Wayland 서버로 activate → 키 입력 → commit/preedit를
  검증). 실제 KWin이 없어도 프로토콜 흐름을 시험할 수 있고, 이미 실제 버그(`KeyState::Repeated` 처리)를 잡았다.
- **실사용 보고(2026-11-07)**: Fedora 최신 + `107bb4bbcc24bfc83b063cfd69c4a052b44fb616`에서 사용자가 한·영 입력 동작을 확인했다. 앱별 범위는 미확인이다. 하단 패널 전체 이동은 KWin bug 527067 경로와 일치했고, 재부팅 전에는 박스 등장/퇴장 때 DBus visible=true/false 변화가 확인됐다. 이후 easime 재설치 없이 재부팅해 KWin 6.8.2를 적용하자 문제가 재현되지 않는다고 사용자가 확인했다. 재부팅 후 DBus 속성은 재측정하지 않았다. 구버전 easime의 포커스 표시 문제와 새 한자 기능은 수정본 설치 후 검증이 남아 있다. [진단과 한자 재검증](kde-testing.md). 일반 클라이언트에는 `zwp_input_method_v1`이 노출되지 않아
  (KWin 6.8.2 확인: 입력기로 등록된 프로세스에게만 제공) 가상 키보드로 등록해야 시험할 수 있다.
- 알려진 미구현: 트레이 아이콘, 일본어 사전의 데몬 연결과 변환 검증, 설정 GUI(`settings-gui-kde` 브랜치에 중간본), XIM, 문절 변환, 중국어, GNOME/Windows/macOS
- 알려진 한계: 상태 박스와 후보창은 고정 다크 테마다. Plasma 색상 연동은 아직 없다.
- 알려진 한계: 한글 자판은 활성 키보드 레이아웃의 keysym을 따른다(물리 키 위치 기준 아님). 레이아웃이 US가 아니면 어긋날 수 있다.

## 중단된 작업 (이어서 할 것)
토큰 소진으로 2026-11-07에 아래 두 작업을 중간에 멈췄다. 커밋은 로컬 브랜치/워크트리에 남아 있고 `main`에는 아직 없다
(`git branch -a`, `git worktree list`로 확인. 워크트리: `.claude/worktrees/`).
- `settings-gui-kde`: Kirigami 설정 앱(`settings/kde`, 워크스페이스에서 제외). 완성 여부는 브랜치 로그와 스크린샷으로 확인할 것.
- `candidate-window`: 후보창(입력 패널 공유, 페이지 나누기). 완성 여부는 브랜치 로그로 확인할 것.
- 병합 방법: `git cherry-pick <첫커밋>^..<마지막커밋>` 후 `fmt`/`clippy -D warnings`/`test --workspace` 확인, 푸시, CI 확인.
- 다음 후보 작업: 클립보드 기록(`Super+V`)과 이모지 선택기(`Super+.`) ([concept.md](concept.md), [decisions.md](decisions.md)), 시스템 색상 연동, 사전을 데몬에 연결(`engine_japanese::load_default()`, 시작 시 한 번, 약 0.1초), 트레이 아이콘, XIM, 실제 KWin 시험.

