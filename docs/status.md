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
  - Windows 준비(2026-11-07): 새 크레이트 `frontend-tsf`에 TSF 연결 없이 되는 부분 — 가상 키·스캔 코드를 `KeyEvent`와 전환 키 분류로 바꾸는 순수 함수(물리 키 기준 US 글자라 자판 배열과 무관) — 를 만들었다. 시스템 언어는 `GetUserPreferredUILanguages`로 읽고(CI Windows 러너에서 호출 확인), 설정 변경은 포터블 `ConfigReloader`로 감지하며, 설정·로그·사전 경로는 `Os`를 인자로 받는 하나의 코드다. 등록 방식은 **언어별 프로필**로 정해졌다(사용자 결정, 2026-11-08): `frontend-tsf::profiles`에 프로필 목록(언어, LANGID, 고정 GUID, 이름)이 있고, `ime-session`의 `Session::for_language`가 프로필 하나의 동작(그 언어와 영어만 오감, 설정의 언어 목록과 무관)을 맡는다. TSF 등록(레지스트리의 COM 클래스, 언어별 프로필, DLL의 `DllRegisterServer`/`DllUnregisterServer` 내보내기)은 만들었고, CI의 Windows 러너가 실제 Windows에서 등록 → TSF의 프로필 등록 기록 확인 → 제거 → 다시 제거를 시험한다(Windows가 언어 설정 화면에 나열하는지는 확인하지 못했다). **텍스트 서비스 1단계**(COM 클래스 팩터리, `ActivateEx`/`Deactivate`, TSF에 키 이벤트 싱크 등록)는 만들었다. CI의 Windows 러너가 `regsvr32`로 빌드된 DLL 등록 → COM이 그 DLL에서 서비스 생성 → TSF가 프로필을 이 프로세스에 활성화(`ActivateProfile`)하면 스레드 관리자가 easime의 싱크를 포그라운드 키 싱크로 보고하는 것 → 등록 해제를 확인하고, 팩터리가 서비스를 세고 놓아 주는 것을 확인한다. **키는 하나도 먹지 않아(조합·후보창·전환 키 없음) 등록해도 입력은 되지 않으므로** MSI는 등록하지 않고 여전히 터미널 데모만 설치한다.
  - 공통 코어 정리(2026-11-07): OS와 무관한 로직을 `frontend-wayland`에서 새 포터블 크레이트 `ime-session`으로 옮겼다(`Session`: 엔진 구성·언어 순환·전환 시 글자 확정·설정 적용·후보 못 띄울 때 취소, `ToggleTracker`: 한/영 키와 오른쪽 Alt 단독 탭 판정(키 식별을 어댑터가 `KeyRole`로 알려 주는 일반형), `JapaneseDictionary`, `language_glyph`). 후보 페이지 계산은 `ime-core::CandidateList`로, 사전 폴더는 OS별 함수(`engine-japanese::loader::Os`, Linux·Windows·macOS 규칙을 모든 OS에서 단위 테스트)로 옮겼다. Wayland 쪽 동작은 그대로이며 가짜 KWin 테스트 전체가 통과한다. **어디까지가 코어이고 OS마다 무엇이 다른지**는 [platforms.md](platforms.md)에 정리했다. 아직 코어 밖에 있는 OS 의존 항목(Windows·macOS의 시스템 언어 읽기, 설정 파일 감시, 키 코드 표 등)은 그 문서의 표에 "없음/계획"으로 적혀 있다.
  - 입력 언어 기본값(2026-11-07 사용자 결정): 설정 파일에 `languages.enabled`가 없으면 **영어 + 시스템 언어**. 한국어 시스템이면 한국어, 일본어 시스템이면 일본어만 켜고, 그 밖의 시스템(영어, 프랑스어, 중국어 등)은 모두 꺼 영어만 입력된다. 한 번 고르면(`enabled = [...]`, 빈 목록 포함) 그 선택이 우선한다. 시스템 언어는 `LANGUAGE`(C/POSIX 로케일이면 무시), `LC_ALL`, `LC_MESSAGES`, `LANG` 순으로 읽는다(`easime-config/src/locale.rs`, 단위 테스트). 데몬은 시작할 때와 설정을 다시 읽을 때 어느 쪽인지 로그에 남긴다. Windows·macOS는 환경 변수가 아니라 OS의 언어 설정을 읽어야 하므로 해당 프론트엔드를 만들 때 붙인다.
  - 중국어는 간체(`chinese_simplified`)와 번체(`chinese_traditional`)가 별개의 입력 언어다(2026-11-07 사용자 결정). 시스템 로케일 대응: `zh_TW`/`zh_HK`/`zh_MO`/`Hant` → 번체, `zh_CN`/`zh_SG`/`Hans`/`zh` → 간체. 엔진은 아직 없어 기본으로 켜지지 않고 설정 창에서도 켤 수 없다("준비 중"). 예전 이름 `chinese`는 이제 알 수 없는 값이다(어떤 버전도 쓴 적이 없다).
  - `engine-hangul`: 두벌식, 겹모음/겹받침, 자모/글자 단위 백스페이스. 조합 중 한 글자 한자 변환 코드와 후보창 추가(실제 KDE 재검증 필요). 한자 키 → ↑↓/Space 이동, PgUp/PgDn 페이지, Enter/1–9 확정, Esc/Backspace로 한글 복귀.
  - `engine-japanese`: 로마자 → 히라가나/가타카나, Space로 변환 시작(후보 순환 Space/↑↓, Enter 확정, Esc/Backspace 가나로 복귀, 숫자 선택).
    사전: `Dictionary` trait, `MemoryDictionary`(TSV), `FileDictionary`(이진, 로드 약 0.1초), `LayeredDictionary`, `load_default()`.
    **데몬에 연결했다**: 첫 일본어 엔진을 만들 때 사용자(`~/.local/share/easime/dictionary`)와 시스템(`/usr/share/easime/dictionary`) 폴더의 `*.bin`/`*.tsv`를 한 번 열고, 설정이 바뀌어 엔진을 다시 만들어도 그대로 공유한다. 결과(읽은 파일, 읽지 못한 파일, 아무것도 없음)는 로그의 `japanese dictionary:` 줄에 남는다. 사전이 없으면 가나(히라가나·가타카나)만 후보로 나온다. 문절 분할/문맥 변환은 없다(단어 단위).
    변환 중에는 한국어 한자와 같은 후보창을 쓴다: 머리글에 읽기, 일본어 글꼴 우선, PgUp/PgDn으로 페이지 이동, 숫자 1–9는 화면에 보이는 페이지 기준. 프리에딧에는 선택한 후보가 보이고 Enter로 확정(앱에는 전달하지 않음), Esc/Backspace는 읽기로 복귀, 한영 전환과 포커스 이탈은 보이는 후보를 한 번 확정한다.
    **가짜 컴포지터 테스트로만 확인했고 실제 KDE에서는 시험하지 않았다.** 후보창을 띄울 수 없으면 변환을 시작하지 않고 읽기를 유지한다. 사전이 없거나 읽지 못했을 때 창에서 알려 주는 문구는 아직 없다(로그만).
  - `easime-dict-tool`: Mozc OSS 사전 → easime 이진 사전 변환기. `scripts/fetch-mozc-dictionary.sh`(고정 SHA, 체크섬 검증),
    `scripts/build-dictionary.sh sample|mozc`. 라이선스(BSD-3, IPAdic, 오키나와 사전)는 `THIRD_PARTY_LICENSES.md`와 `data/mozc/`.
  - 설정 즉시 반영: 설정 파일을 inotify로 감시해 한/영 키, 백스페이스 단위, 언어 목록을 재시작 없이 적용(깨진 파일은 무시)
  - 패키징: **배포판당 패키지 하나**(`easime`: 데몬, 설정 창, 두 `.desktop`, 사전)를 `.deb`/`.rpm`으로 만든다. 설치가 한 번에 끝나도록 한 사용자 요청(2026-11-07)으로 데몬과 설정 창 패키지를 합쳤다. 빌드·설치 검증은 `.github/workflows/packages.yml` 하나이고, CI와 태그 `v*` 푸시 시 릴리스 워크플로가 같은 흐름과 **같은 실제 Mozc 사전**을 쓴다(CI 패키지에 샘플 사전이 들어 있어 일본어 변환이 가나만 내던 문제를 고친 것: 2026-11-07 실사용 보고). 태그와 두 크레이트(`easime`, `easime-settings`)의 버전이 같아야 릴리스가 진행된다.
  - Windows 설치 프로그램(MSI, WiX 5, `packaging/windows`): **터미널 데모와 라이선스만 담는다. Windows 입력기(TSF)가 없으므로 입력기를 설치하지 않는다.** CI의 Windows 러너에서 설치 → `easime-cli` 실행 → 제거 → PATH 옵션 → 업그레이드·다운그레이드 거부까지 시험한다. 릴리스에는 올리지 않는다. 사전과 설정 앱(WinUI 3)은 아직 넣지 않았다.
  - `frontend-wayland` + `easime` 데몬: KWin `zwp_input_method_v1` 클라이언트, 키 반복(자체 타이머와 컴포지터 `repeated`),
    한/영 키와 오른쪽 Alt 탭 전환, Caps Lock 무시, 언어 순환, 설정 읽기
  - **태스크바 트레이 아이콘**(2026-11-07 사용자 요청, 이미 정한 결정 "트레이 아이콘 + 커서 옆 박스"의 구현): 현재 언어를 커서 옆 박스와 같은 모양의 글자(`A`/`한`/`あ`)로 StatusNotifierItem에 보이고, 툴팁은 언어 이름(`English`/`한국어`/`日本語`). **아이콘을 클릭하면 다음 언어로**(한/영 키와 같은 동작) 바뀌고, 우클릭 메뉴에는 켠 언어 목록(라디오, 고른 언어로 바로 전환)과 "easime 설정"(설정 창 열기, 표시 언어는 시스템 언어를 따름)이 있다. 전환 요청은 입력기에 다른 스레드의 요청 채널(`frontend_wayland::select_language`/`next_language`, eventfd로 poll을 깨움)로 가서 입력 스레드가 직접 처리하므로, 조합 중인 글자는 전환 전에 확정되고 세션에는 입력 스레드만 손댄다. 입력 스레드는 채널에 소식만 보내고 D-Bus는 별도 스레드가 맡아 입력이 느려지지 않는다(`crates/easime/src/tray.rs`, 입력기 쪽 훅은 `frontend_wayland::observe_language`). 로그인 직후 셸이 아직 태스크바를 갖기 전에 시작해도 계속 시도한다.
    **검증 범위**: 실제 D-Bus에서 가짜 StatusNotifierWatcher(셸 역할)와 호스트 역할로 등록·속성·언어 변경에 따른 툴팁·아이콘 갱신을 시험한다(CI의 Linux 잡). 가짜 KWin 테스트로 언어 변경 알림이 시작과 모든 전환에서 오는지, 다른 스레드의 언어 선택·다음 언어 요청이 조합 중인 글자를 확정하고 언어를 바꾸며(이미 그 언어이거나 켜지 않은 언어는 무시) 처리되는지 확인한다. D-Bus 위에서는 셸이 보내는 `Activate`(클릭)가 "다음 언어" 요청이 되는 것까지 시험하고, 메뉴 항목의 동작은 단위 테스트로 확인한다(메뉴를 D-Bus로 눌러 보지는 않았다). **실제 Plasma의 태스크바에서 보이는 모습(크기, 밝은·어두운 테마에서의 가독성)은 확인하지 않았다.** 아이콘은 고정 다크 박스이므로 시스템 색상 연동 전까지 테마를 따르지 않는다.
  - 커서 옆 상태 박스(`frontend-wayland/src/{indicator,font,panel}.rs`): 사용자가 언어를 전환할 때만 `A`/`한`/`あ`/`中`
    48x48 박스를 `indicator_ms`(기본 1000) 동안 표시. `zwp_input_panel_surface_v1.set_overlay_panel`(KWin이 커서 사각형 바로 아래에 배치,
    키보드 포커스 안 가짐, 빈 input region), 표시/숨김은 버퍼 attach/null attach, `[appearance] indicator_box`로 끄기.
    사용자가 Fedora KDE의 `107bb4b`에서 한·영 입력 성공을 보고했다. 하단 패널 전체 이동은 easime 재설치 없이 재부팅해 KWin 6.8.2를 적용한 뒤 재현되지 않는다고 확인했다. 포커스 시 박스 표시 제거는 로컬 수정본 설치 후 검증이 필요하다. text-input-v1 앱은 커서 위치를 보내지 않아 창 왼쪽 위에 뜰 수 있다.
  - CI: Linux는 전체, Windows/macOS는 OS 독립 crate만 (`fmt`, `clippy -D warnings`, `test`). 설정 창은 Fedora 컨테이너 잡이 따로 검증한다(아래).
  - 설정 창(전문가용, `settings/kde`): Rust + cxx-qt + QML + Kirigami, 메인 워크스페이스와 별개의 cargo 프로젝트.
    페이지: 일반(파일 위치, 다시 읽기, 기본값으로 되돌리기), 언어(켜기/끄기, 순서, 전환 순서 미리보기), 단축키(전환 키), 한국어(백스페이스 단위), 모양(상태 상자).
    어떤 언어도 필수가 아니다(한국어만, 일본어만, 둘 다, 아무것도 없이 영어만). 처음에는 시스템 언어에 맞춰져 있고 그 사실을 언어 페이지가 알린다. 언어 페이지에는 简体中文과 繁體中文이 따로 "준비 중"으로 나온다. 변경은 바로 저장되고 데몬이 다시 읽는다. 바뀐 키만 `toml_edit`으로 고치므로 직접 쓴 주석과 모르는 키가 남고,
    읽을 수 없는 파일은 덮어쓰지 않으며(변경을 거부하고 알림), 초기화는 `config.toml.bak`을 남긴다. 모델은 Qt 없이 단위 테스트한다(24개).
    **검증 범위**: CI `settings` 잡(필수)이 Fedora 컨테이너에서 실제 Kirigami로 빌드하고, 오프스크린으로 5개 페이지를 모두 열어 QML 경고를 실패로 취급하며,
    Switch·라디오·체크박스를 `click()`으로 눌러 설정 파일이 바뀌는 것까지 확인한다(시스템 언어는 C.UTF-8이라 모든 언어가 꺼진 상태에서 시작). 시스템 언어별 기본 입력 언어(ko→한국어, ja→일본어, 그 밖과 중국어→없음, `LANGUAGE=ja:ko`→둘)도 실제 창을 띄워 확인한다. 페이지별 화면 캡처는 Actions 아티팩트 `settings-window-pages`에 남는다.
    **사람이 실제 Plasma에서 연 적은 없고**, 색·간격 같은 모양과 사이드바의 보이는 모습은 확인하지 못했다.
    **입력기 활성화(2026-11-08 사용자 결정)**: KDE에서 일반 페이지 맨 위 카드가 easime가 입력기로 선택되었는지 알려 준다(`kwinrc`의 `[Wayland] InputMethod`). 선택되지 않았으면 [입력기로 사용하기…]가 바꿀 파일과 지금 값을 보여 주고 확인을 받은 뒤 적용하며(다시 로그인하면 적용), 바꾸기 전의 값을 `~/.config/easime/activation-backup.toml`에 두어 [이전 입력기로 되돌리기…]로 정확히 그 값을 되돌린다. 되돌릴 때 그 사이 사용자가 다른 입력기를 골랐으면 건드리지 않는다. `kwinrc`의 다른 줄과 주석은 그대로 둔다. KDE가 아니면 카드가 보이지 않고, `easime.desktop`을 못 찾으면 이유를 알린다. 데몬이 이미 실행 중이면 "사용하고 있습니다"로 보인다. 로직은 Qt 없이 단위 테스트한다(19개 + 화면 값). CI의 Fedora 잡이 실제 Kirigami에서 버튼이 여는 확인 대화상자를 "OK"로 답해 `selected`가 되는 것과 투어 끝의 되돌리기를 다섯 언어로 확인하고, 패키지 설치 검증 잡은 설치된 `easime.desktop`이 발견되는 것(KDE 세션 가정)까지 확인한다. **실제 Plasma에서 켜고 로그인해 easime가 뜨는 것은 확인하지 않았다.**
    표시 언어: 한국어, English, 日本語, 繁體中文, 简体中文. 시스템 언어(`LANGUAGE`/`LANG`)를 따르고 그 밖의 언어는 영어이며, `--ui-language=ko|en|ja|zh-Hant|zh-Hans`로 지정할 수 있다.
    문구는 한국어 원문을 키로 한 표 한 개(`settings/kde/qml/tr.js`)에 있다. **한국어 외의 번역은 원어민 검토 전 초안**이고 번역된 화면의 일반 페이지가 그렇게 알린다.
    자동 검사: 번역표와 QML 문구의 대조(누락·미사용·빈 번역·`%1` 불일치, `tests/check_translations.py`), 언어 선택 로직(`tests/tst_tr.qml`), CI가 다섯 언어로 각각 창을 열어 클릭→파일 저장과 번역 누락·QML 경고 없음을 확인하고 시스템 언어 감지를 시험한다.
    **번역의 정확성·자연스러움과 언어별 한자 자형은 확인하지 않았다.**
    패키지: 데몬과 같은 하나의 `.rpm`(Fedora 컨테이너에서 빌드)/`.deb`(Debian 13 컨테이너에서 빌드)에 들어 있고, 앱 메뉴에 "easime 설정"(`easime-settings.desktop`, 한·일·번체·간체 이름)으로 등록된다. CI가 패키지를 clean Fedora·Debian 컨테이너에 설치해 설정 창이 열리고 데몬이 시작되는지 확인한다. 설치만으로는 easime가 입력기로 선택되지 않는다(시스템 설정의 가상 키보드에서 고른다).
    **실제 데스크톱에 설치해 메뉴에서 연 사람의 확인은 없다.**
    아직 없음: 자동 실행, 초보자용(첫 실행 안내), 번역의 원어민 검토, 설정 창 안에서 표시 언어를 고르는 선택지, 일본어·중국어 페이지(옵션이 생길 때 추가).
- **가짜 KWin 테스트 하네스**: `crates/frontend-wayland/tests/` (순수 Rust Wayland 서버로 activate → 키 입력 → commit/preedit를
  검증). 실제 KWin이 없어도 프로토콜 흐름을 시험할 수 있고, 이미 실제 버그(`KeyState::Repeated` 처리)를 잡았다.
- **실사용 보고(2026-11-07)**: Fedora 최신 + `107bb4bbcc24bfc83b063cfd69c4a052b44fb616`에서 사용자가 한·영 입력 동작을 확인했다. 앱별 범위는 미확인이다. 하단 패널 전체 이동은 KWin bug 527067 경로와 일치했고, 재부팅 전에는 박스 등장/퇴장 때 DBus visible=true/false 변화가 확인됐다. 이후 easime 재설치 없이 재부팅해 KWin 6.8.2를 적용하자 문제가 재현되지 않는다고 사용자가 확인했다. 재부팅 후 DBus 속성은 재측정하지 않았다. 구버전 easime의 포커스 표시 문제와 새 한자 기능은 수정본 설치 후 검증이 남아 있다. [진단과 한자 재검증](kde-testing.md). 일반 클라이언트에는 `zwp_input_method_v1`이 노출되지 않아
  (KWin 6.8.2 확인: 입력기로 등록된 프로세스에게만 제공) 가상 키보드로 등록해야 시험할 수 있다.
- 알려진 미구현: 일본어 변환의 실제 KDE 검증과 사전 상태 안내 문구, 설정 창의 자동 실행·초보자용, XIM, 문절 변환, 중국어, GNOME/Windows/macOS
- 알려진 한계: 상태 박스와 후보창은 고정 다크 테마다. Plasma 색상 연동은 아직 없다.
- 알려진 한계: 한글 자판은 활성 키보드 레이아웃의 keysym을 따른다(물리 키 위치 기준 아님). 레이아웃이 US가 아니면 어긋날 수 있다.

## 중단된 작업 (이어서 할 것)
토큰 소진으로 2026-11-07에 아래 두 작업을 중간에 멈췄다. 커밋은 로컬 브랜치/워크트리에 남아 있고 `main`에는 아직 없다
(`git branch -a`, `git worktree list`로 확인. 워크트리: `.claude/worktrees/`).
- `settings-gui-kde`: 형님 로컬에만 있는 Kirigami 설정 앱 중간본. `main`에 같은 경로(`settings/kde`)로 새 구현이 들어갔으므로 합치지 말고, 필요한 부분(화면 구성, 스크린샷)만 비교해서 가져올 것.
- `candidate-window`: 형님 로컬에만 있는 후보창 중간본. `main`에는 이미 후보창이 있다(`frontend-wayland/src/candidate.rs`, 한국어 한자와 일본어 공용). 합치기 전에 비교할 것.
- 병합 방법: `git cherry-pick <첫커밋>^..<마지막커밋>` 후 `fmt`/`clippy -D warnings`/`test --workspace` 확인, 푸시, CI 확인.
- 다음 후보 작업: 클립보드 기록(`Super+V`)과 이모지 선택기(`Super+.`) ([concept.md](concept.md), [decisions.md](decisions.md)), 시스템 색상 연동, XIM, 실제 KWin 시험.

