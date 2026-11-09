# easime 플랫폼별 코어 공유 범위와 OS 의존 항목

easime는 Linux(KDE → GNOME) → Windows → macOS 순서로 간다([decisions.md](decisions.md)).
입력기는 OS마다 키를 받는 방식, 글자를 넣는 방식, 창을 띄우는 방식이 다르다.
이 문서는 **무엇을 모든 OS가 같은 코드로 공유하고, 무엇을 OS마다 따로 만드는지**, 그리고 **OS마다 코어가 어디까지인지**를 정한다.
새 프런트엔드를 만들거나 로직을 어디에 넣을지 고민할 때 먼저 읽는다. 구조의 큰 그림은 [architecture.md](architecture.md)에 있다.

## 읽는 법: 상태 표기

이 문서의 상태는 DESIGN.md의 구분을 따른다. 없는 것을 있는 것처럼 쓰지 않는다.

| 표기 | 뜻 |
|---|---|
| **있음** | 코드가 있고 테스트나 CI가 확인한다. 어디까지 확인했는지는 같이 적는다 |
| **있음(미검증)** | 코드는 있으나 그 OS에서 실제로 돌려 본 적이 없다 |
| **계획** | 설계 메모만 있다. 코드 없음 |
| **조사 필요** | 그 OS의 동작을 아직 확인하지 않았다. 아래의 "예상"은 결정이나 사실이 아니다 |

## 세 계층

```
┌───────────────────────────────────────────────────────────────┐
│ 공통 코어   모든 OS가 같은 코드. 모든 OS의 CI에서 빌드·테스트     │
│   ime-core  ime-session  engine-hangul  engine-japanese       │
│   easime-config  easime-settings-model                        │
│   (easime-dict-tool, easime-cli)                              │
├───────────────────────────────────────────────────────────────┤
│ OS 어댑터   프런트엔드. OS의 입력기 API ↔ 코어                  │
│   frontend-wayland (있음)  frontend-tsf (있음, 실제 PC 미시험)  │
│   frontend-ibus (계획)  macOS 껍데기(Swift) + easime-ffi (계획) │
├───────────────────────────────────────────────────────────────┤
│ OS 고유     코어 밖. 설정 앱, 설치·등록, 트레이, 프로세스 구조    │
│   settings/kde (있음)  settings/windows (있음)  settings/macos │
│   deb/rpm (있음)  MSI·setup.exe (있음)  pkg (계획)             │
└───────────────────────────────────────────────────────────────┘
```

### 어디에 넣을지 정하는 기준

- **"키를 받았을 때 무엇이 일어나야 하는가"는 코어**에 둔다. 같은 타이핑은 어느 OS에서나 같은 결과를 내야 한다.
  예: 한/영 키는 누르는 즉시 전환, 오른쪽 Alt는 단독으로 눌렀다 뗄 때만 전환, 전환 직전에 조합 중인 글자를 확정, 후보창을 못 띄우면 변환을 취소.
- **"그것을 OS에서 어떻게 받고 어떻게 보여 주는가"는 어댑터**에 둔다. 키 코드 표, 글자를 넣는 API, 창, 타이머, 파일 감시.
- 확인 질문 두 가지: ① OS의 키 코드·프로토콜·창 타입이 코드에 나오는가? → 어댑터. ② 둘 이상의 OS에서 같은 결과여야 하는가? → 코어.
  둘 다 해당하면 코어에 규칙을 두고, OS가 달라지는 부분만 인자(예: `KeyRole`)나 함수 이음새로 뺀다.
- 코어 크레이트는 OS·UI 의존성(Wayland, X11, Qt, Win32)을 갖지 않는다([AGENTS.md](../AGENTS.md)). OS별 경로 같은 불가피한 차이는 **OS를 인자로 받는 순수 함수**로 두어서 어느 OS에서든 모든 OS의 규칙을 테스트할 수 있게 한다(`ime-core`의 `Os`, `engine-japanese`의 `loader.rs`, `easime-config`의 `store.rs`).

## 공통 코어가 가진 것

| 크레이트 | 맡은 일 | OS 이음새 |
|---|---|---|
| `ime-core` | `KeyEvent`, `InputEngine`, `EngineOutput`, `CandidateList`(페이지 9개, 번호 1–9, 페이지·선택 계산), `Language`, `LanguageSwitcher`, `Os` | `Os` enum 정의 |
| `ime-session` | `Session`(활성 언어의 엔진에 키 전달, 설정을 바꾸거나 언어를 전환하거나 포커스를 잃어도 글자를 잃지 않는 규칙, 보이지 않는 후보 선택 금지), `ToggleTracker`(전환 키 판정), `JapaneseDictionary`(한 번만 열어 공유), `language_glyph`(언어를 나타내는 한 글자) | 키 식별은 어댑터가 `KeyRole`로 알려 준다. 로그는 어댑터가 `LogFn`으로 준다 |
| `engine-hangul` | 두벌식 조합, 한자 변환(조합 중 한 글자) | 없음 |
| `engine-japanese` | 로마자 → 가나, 사전 변환, 사전 로더 | **사전 폴더**가 OS별이다(`loader.rs`, `ime-core::Os` 사용) |
| `easime-config` | 설정 스키마, TOML 읽기·쓰기, 언어 기본값(영어 + 시스템 언어) | **설정 파일 위치**(`store.rs`)와 **시스템 언어 읽기**(`locale.rs`)가 OS별이다 |
| `easime-settings-model` | 설정 앱이 보여 주는 값과 변경을 저장하는 규칙, 일본어 사전 상태(UI 툴킷 없음) | 없음. KDE와 Windows 설정 앱이 공유한다 |

코어를 쓰는 방법은 한 줄이다. 어댑터가 OS의 키를 `KeyEvent`로 바꿔 `Session::process_key`에 주고, 돌아온 `EngineOutput`(확정할 글자, 조합 중인 글자, 후보 목록)을 OS의 방식으로 보여 준다.

## 어댑터가 제공해야 하는 것 (계약)

새 OS의 프런트엔드는 아래를 모두 구현해야 한다. 오른쪽 열은 Linux/Wayland(`frontend-wayland`)에서 어디에 있는지다.

| # | 어댑터의 일 | 코어와의 접점 | Wayland에서 |
|---|---|---|---|
| 1 | OS 키를 `KeyEvent`(+`Modifiers`)로 바꾼다 | `ime_core::Key` | `translate.rs` (X keysym) |
| 2 | 각 키가 전환 키인지 분류한다 | `KeyRole` → `ToggleTracker` | `translate.rs`의 `toggle_role` |
| 3 | 엔진이 소비한 키와 소비하지 않은 키를 OS에 알맞게 처리한다 (`handled == false`면 앱에 돌려준다. 소비한 키의 뗌 이벤트도 처리한다). 키를 가져갈지 **먼저 묻는** OS(Windows TSF)는 부작용 없이 답한다 | `EngineOutput::handled`, `Session::wants_key` | `app.rs`의 `on_key`, `Swallowed` (Windows: `frontend-tsf::typing`) |
| 4 | 확정할 글자와 조합 중인 글자를 앱에 넣는다 | `EngineOutput::commit`, `preedit` | `app.rs`의 `apply` |
| 5 | 포커스가 들어오면 `Session::reset`, 나가면 `Session::flush` 결과를 확정한다 | `Session` | `activate`, `release_focus` |
| 6 | 후보 목록을 화면에 띄운다. **띄울 수 없으면 `Session::cancel_conversion`** | `CandidateList`, `Session` | `candidate.rs`, `panel.rs` |
| 7 | 언어 전환 때 상태(언어 글자)를 보여 준다 | `language_glyph` | `indicator.rs`, `panel.rs` |
| 8 | 설정 파일이 바뀌면 `Session::apply_config`(+전환 키 설정 갱신) | `Config`, `Session` | `watch.rs`, `on_settings_event` |
| 9 | 로그를 남긴다 | `LogFn` | `log.rs` |
| 10 | OS가 키 반복을 해 주지 않으면 직접 반복한다 | 없음(어댑터 몫) | `repeat.rs`, `timer.rs` |
| 11 | 사전 폴더에 사전을 두고, 시스템 언어를 코어에 알려 준다 | `default_dictionary_dirs`, `languages_for_tags` | 패키지가 `/usr/share/easime/dictionary`에 둔다 |

## OS별로 어디까지가 코어인가

모든 OS에서 코어(위 표의 크레이트들)는 같다. OS마다 달라지는 것은 어댑터 쪽과 그 주변이다.
**"예상"은 조사 전의 짐작**이므로 그 OS 프런트엔드를 만들 때 확인한다.

### Linux — KDE Plasma 6 (Wayland): 있음

- 프로세스: 데몬 하나(`easime`)를 KWin이 가상 키보드로 실행한다. 모든 창이 이 프로세스 하나를 공유하므로 "전체 공통 입력 상태"가 자연스럽게 지켜지고, 일본어 사전도 한 번만 열린다.
- 코어: 전부 쓴다. 어댑터는 `frontend-wayland`다.
- 어댑터가 하는 일: `zwp_input_method_v1` 프로토콜, 키보드 grab과 xkb 키맵, **우리 쪽 키 반복**(키를 grab하면 반복도 직접 해야 한다), 입력 패널 창(상태 박스와 후보창), inotify 설정 감시, timerfd.
- 확인한 범위: 한·영 입력은 사용자가 Fedora KDE에서 확인했다. 한자·일본어 변환은 가짜 KWin 테스트로만 확인했다([status.md](status.md)).

### Linux — GNOME (IBus 호환): 계획

- 코어는 그대로다. 어댑터는 `frontend-ibus`(IBus 호환 D-Bus 서비스, 조사 필요)다.
- 예상: 키는 IBus가 전달하므로 키 반복은 우리 몫이 아니다(조사 필요). 상태 표시는 GNOME 입력 소스 표시기에 기댄다. 트레이가 없다.
- Wayland 어댑터와 달라지는 것: 프로토콜, 키 코드 해석, 창 표시. 전환 키 판정·세션·엔진은 같다.

### Windows (TSF): 있음 (CI 확인, 실제 PC 미시험)

구현 현황과 CI가 확인하는 것, 실제 PC에서 확인하지 않은 것은 **[implementation/windows.md](implementation/windows.md)**에 있다. 여기서는 코어와 어댑터를 나누는 관점에서 Windows만의 차이를 적는다.

- 코어: 전부 쓴다. 코어는 `frontend-tsf`(`cdylib` DLL)에 정적으로 링크된다. 어댑터가 하는 일: TSF 텍스트 서비스 등록, 키 이벤트 싱크(가상 키 코드), 조합 중인 글자는 TSF composition, 후보창, 모드 상자, 표시 속성(밑줄).
- 키가 하는 일은 TSF 없이 되는 `frontend-tsf::typing`(`Typist`)에 있어 모든 OS의 CI가 시험하고, TSF 쪽(`service.rs`)은 그 결과를 편집 세션 안에서 문서에 쓴다. 키 코드 해석(`keys`), 프로필·순환 설정(`profiles`), 밑줄 구간(`marks`), 후보 내용(`page`), 상자 위치 규칙(`placement`), DLL 옆 경로(`install`)도 Windows API 없이 모든 OS에서 시험한다.
- **Windows에서만 생기는 문제** (구조 때문에 코어 바깥에서 풀어야 한다):
  - DLL은 **응용 프로그램마다** 로드된다. 프로세스마다 `Session`이 하나씩 생기므로 "전체 공통 입력 상태"(현재 언어)는 프로세스 사이에 공유되지 않는다(공유 메모리나 named pipe 서비스가 필요하며 아직 없다). 설정은 파일이라 모든 프로세스가 같은 것을 따른다.
  - 일본어 사전은 앱마다 읽으면 비싸므로 읽기 전용 **메모리 매핑**으로 열고 **첫 변환 때** 연다(2026-11-08). 프로세스들이 같은 페이지를 나눠 쓰는지는 재지 않았다.
  - 32비트 응용 프로그램을 지원하려면 32비트 DLL도 필요하다. 지금은 64비트뿐이다.
  - 오른쪽 Alt: 일부 자판에서는 AltGr(Ctrl+Alt)로 동작한다. 수정 키가 함께 눌린 오른쪽 Alt는 `Typist`가 `KeyRole::Other`로 분류해 전환을 막는다(실제 AltGr 자판 PC 시험은 아직 필요).
  - 키 반복은 OS가 한다(예상, 실제 PC 미확인). 한/영 키를 누르고 있을 때 Windows가 보내는 반복은 전환이 아니다(`Typist`가 막는다).
  - TSF는 먹은 키를 나중에 다시 보낼 수 없다. 그래서 오른쪽 Alt 누름은 응용 프로그램에 그대로 가고 탭의 뗌에서만 전환한다(Wayland와 다름).
  - TSF는 키를 가져갈지 먼저 묻는다(`OnTestKeyDown`). 가져가겠다고 하고 `OnKeyDown`에서 통과시키는 키(조합을 확정하고 Space·Enter·숫자를 응용 프로그램에 넘기는 경우)가 모든 응용 프로그램에서 키를 잃지 않는지는 실제 PC에서 확인해야 한다.
- **응용 프로그램이 정한 것을 따른다**: 비밀번호 칸(`GUID_COMPARTMENT_KEYBOARD_DISABLED`)이나 글을 받지 않는 곳(`GUID_COMPARTMENT_EMPTYCONTEXT`)에서는 전환 키를 포함해 모든 키를 응용 프로그램에 넘기고, `VK_PACKET`은 키 위치로 읽지 않는다.
- **등록 방식**(사용자 결정, 2026-11-08): 프로필은 **하나**이고 한국어 아래에 등록하며, 그 안에서 한/영 키가 Linux와 같은 순환(영어 → 켠 언어 → 영어)을 돈다. 순환에는 타자가 되는 언어(`profiles::TYPEABLE`: 한국어, 일본어)만 넣는다. 모든 언어 아래 등록은 시험해 적용했다가 되돌렸고, 사용자 언어 아래에서 찾게 하는 다른 방법은 실제 PC 시험 뒤에 다시 정한다([decisions.md](decisions.md) "Windows 등록 방식").
- 코어 바깥(OS 고유): WinUI 3 설정 앱(`settings/windows`), MSI와 `setup.exe`(`packaging/windows`, WiX 5). 설치 동작과 한계는 [implementation/windows.md](implementation/windows.md)의 "설치 프로그램".

### macOS (InputMethodKit): 계획

- 프로세스: 입력기는 별도 프로세스(`.app` 번들)이므로 Linux 데몬처럼 프로세스 하나가 모든 창을 맡는다(예상). 코어는 Rust 정적 라이브러리로 만들고 Swift 껍데기가 `easime-ffi`(계획) 경계 뒤의 `Session`을 부른다.
- 어댑터가 하는 일: `IMKInputController`로 키 수신(키 코드는 macOS 키 코드), 조합 중 글자와 확정, 후보창(IMK의 후보 창 사용 가능, 조사 필요).
- **macOS에서 달라질 가능성이 큰 것** (모두 조사 필요):
  - 일반 Mac 키보드에는 전용 한/영 키가 없다. 전환 키의 기본값과 선택지는 **제품 결정이 필요하다**(OS의 입력 소스 전환 단축키와의 관계 포함).
  - 오른쪽 Option을 단독으로 눌렀다 떼는 것은 키 이벤트가 아니라 수정 키 상태 변화로 온다. `ToggleTracker`에 줄 "누름/뗌"으로 어댑터가 바꿔 주어야 한다.
  - 언어 상태는 메뉴 막대의 입력 소스 메뉴가 보여 준다. 커서 옆 상태 박스를 쓸지는 따로 정한다.
  - 일본어에서 macOS 기본 입력기와 겹치는 키 동작(예: 변환 키)은 시험해 보아야 한다.
- 코어 바깥: SwiftUI 설정 앱, `.pkg`(서명·공증).

## OS 의존 항목 목록

코어 안에 있어도 OS에 따라 달라지는 것, 어댑터에 있는 것을 파일 단위로 모았다. 코드를 고치면 이 표도 고친다.

| 항목 | 위치 | Linux | Windows | macOS |
|---|---|---|---|---|
| 설정 파일 위치 | `easime-config/src/store.rs` | **있음** `~/.config/easime/config.toml` (`XDG_CONFIG_HOME`) | **있음(미검증)** `%APPDATA%\easime\config.toml` | **있음(미검증)** `~/Library/Application Support/easime/config.toml` |
| 설정 저장(임시 파일 후 교체) | `easime-config/src/store.rs` | **있음** | **있음(미검증)**: 다른 프로세스가 파일을 열고 있을 때의 교체 실패는 조사 필요 | **있음(미검증)** |
| 설정 변경 감지 | `frontend-wayland/src/watch.rs`, `ime-session/src/reload.rs` | **있음** inotify(즉시; 설정 파일이 링크이면 가리키는 폴더도 감시) | **있음(미검증)** `ConfigReloader`: 키·포커스 같은 일이 있을 때 파일 시각을 한 번씩 본다(앱마다 감시자를 두지 않으려고). 단위 테스트만 | **있음(미검증)** 같은 `ConfigReloader`(FSEvents로 바꿀지는 조사 필요) |
| 사전 폴더 | `engine-japanese/src/loader.rs` (`Os`) | **있음** `~/.local/share/easime/dictionary`, `/usr/share/easime/dictionary` (패키지가 설치) | **있음(코드·테스트만)** 텍스트 서비스는 `%APPDATA%\easime\dictionary`, 그다음 **DLL 옆 `dictionary` 폴더**(설치 위치를 바꿔도 따라감, `frontend-tsf::install`)를 읽는다. 앱마다 DLL이 들어가므로 `.bin`은 **메모리 매핑**해 앱들이 한 벌을 나눠 쓰고, **첫 변환 때** 연다(2026-11-08). MSI가 사전을 DLL 옆에 설치한다. `loader.rs`의 Windows 시스템 폴더 규칙(`%ProgramW6432%\easime\dictionary`)은 텍스트 서비스가 쓰지 않는다 | **있음(코드·테스트만)** `~/Library/Application Support/easime/dictionary`, `/Library/Application Support/easime/dictionary` (설치 없음) |
| 사전 읽기 | `engine-japanese/src/file_dict.rs` | **있음** 파일 전체를 메모리로 | **있음(코드·CI)** 읽기 전용 메모리 매핑(`Access::Map`), 첫 변환 때 연다. 앱마다 같은 페이지를 나눠 쓰는지는 실제 PC에서 재지 않았다 | 프로세스 하나라 Linux와 같을 것(예상) |
| 시스템 언어 읽기 | `easime-config/src/locale.rs` | **있음** `LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, `LANG` | **있음** `GetUserPreferredUILanguages`. CI의 Windows 러너에서 호출이 되는 것만 확인했고 한국어·일본어 Windows에서는 돌려 보지 않았다 | **없음**: 환경 변수가 비어 있어 영어만(안전한 폴백). 선호 언어 API가 필요 |
| 로그 위치 | `easime-config/src/store.rs`(`log_path_for`), 쓰기는 `frontend-wayland/src/log.rs` | **있음** `~/.local/state/easime/easime.log` | 경로 **있음**(`%LOCALAPPDATA%\easime\easime.log`, 모든 OS에서 테스트), 쓰는 쪽은 **있음**(`frontend-tsf`의 로그 기록) | 경로 **있음**(`~/Library/Logs/easime/easime.log`), 쓰는 쪽은 **계획** |
| 키 코드 → `KeyEvent` | `frontend-wayland/src/translate.rs`, `frontend-tsf/src/keys.rs` | **있음** X keysym | **있음** 가상 키 코드 + 스캔 코드(물리 키 기준 US 글자, 레이아웃과 무관). 모든 OS에서 단위 테스트, TSF에 연결됨 | **계획** macOS 키 코드 |
| 전환 키 분류(`KeyRole`) | `translate.rs`의 `toggle_role`, `frontend-tsf/src/keys.rs`의 `toggle_role` | **있음** | **있음** 한/영 키, 오른쪽 Alt(확장 키 표시, Ctrl·Shift·Win 동반 시 제외). 실제 AltGr 자판 PC 검증은 **조사 필요** | **계획** (수정 키 이벤트 조사 필요) |
| 키 반복 | `frontend-wayland/src/repeat.rs`, `timer.rs` | **있음** 직접 반복 | OS가 한다(예상) | OS가 한다(예상) |
| 조합·확정 전달 | `frontend-wayland/src/app.rs`의 `apply` | **있음** `commit_string`, `preedit_string` | **있음** TSF composition(편집 세션 안에서 쓰고 끝냄. 밑줄은 TSF 표시 속성) | **계획** `setMarkedText`, `insertText` |
| 후보창 | `frontend-wayland/src/candidate.rs`, `panel.rs`, `frontend-tsf/src/candidate.rs`, `element.rs` (내용은 `page.rs`) | **있음** 입력 패널 + 직접 그림 | **있음(모의 앱 CI 확인)** 커서 아래 GDI 창, 후보를 직접 그리는 앱에는 TSF UI 요소 | **계획** |
| 언어 표시 글자 | `ime-session/src/indicator.rs` | **있음(공통)** | 공통 | 공통 |
| 표시 글꼴 | `frontend-wayland/src/font.rs`, `frontend-tsf/src/indicator.rs`, `candidate.rs` | **있음** Noto Sans CJK 등 | **있음(상태 상자, 후보창)** `Malgun Gothic`, 일본어 후보는 `Yu Gothic UI` (GDI) | **계획** OS의 CJK 글꼴 |
| 상태 표시 방식 | `panel.rs`(박스), `crates/easime/src/tray.rs`(트레이), `frontend-tsf/src/indicator.rs` | **있음** 커서 옆 박스, 트레이 아이콘(StatusNotifierItem, 클릭·메뉴로 언어 전환; 실제 Plasma 미검증, D-Bus 위의 가짜 셸로 시험) | **있음(커서 옆 박스, 모의 앱 CI 확인)** / 상시 표시(알림 영역 아이콘 또는 언어 바)는 **계획** | **계획** 메뉴 막대 입력 소스 |
| 프로세스 구조와 등록 | `data/easime.desktop` 등 | **있음** KWin 가상 키보드 | **있음** TSF DLL 등록(`regsvr32`, MSI) | **계획** `~/Library/Input Methods` |
| 설정 앱 | `settings/` | **있음** `settings/kde` (Kirigami) | **있음(CI 확인, 실제 PC 미시험)** `settings/windows` (Rust + windows-reactor, WinUI 3) | **계획** SwiftUI |
| 패키지 | `crates/easime`, `settings/kde`, `packaging/` | **있음** `.deb`, `.rpm` | **있음(CI 확인, 실제 PC 미시험)** MSI와 `setup.exe`(Windows App Runtime이 없으면 Microsoft에서 받음) | **계획** `.pkg` |
| 어댑터 테스트 | `frontend-wayland/tests/` | **있음** 가짜 KWin | **있음** 실제 TSF + 모의 앱, 등록·DLL 검사(실제 앱 미검증) | **계획** 새 방법 필요 |

설정 앱은 OS마다 따로 만들지만 **같은 설정 파일과 같은 페이지 구성**을 쓴다. 설정 앱을 코어에 합치지 않는다.

## OS 사이의 동작 차이와 지침

제품 결정(전체 창 공통 입력 상태, 한/영 키 기본, 키보드 중심)은 OS가 달라도 유지한다. 구현은 OS에 맞게 다르다([DESIGN.md](../DESIGN.md) 6절: 보존할 것은 정보의 의미와 행동 결과다).

| 차이 | 지침 |
|---|---|
| 프로세스 구조(데몬 하나 / 앱마다 DLL) | 전체 공통 상태를 OS에 맞는 방법으로 지킨다. 코어는 프로세스 하나가 `Session` 하나를 가진다고만 가정한다 |
| 키 반복 | OS가 해 주면 어댑터는 반복하지 않는다. 코어는 반복을 알지 못한다. 단, 한/영 키를 누르고 있을 때 Windows가 보내는 반복은 전환이 아니다(`Typist`가 막는다) |
| 먹은 키를 나중에 다시 보내기 | Wayland는 오른쪽 Alt 누름을 붙들었다가 다른 키가 오면 응용 프로그램에 다시 보낸다. TSF는 그럴 수 없으므로 Windows는 누름을 그대로 통과시키고 탭의 뗌만 먹는다 |
| 키를 가져갈지 먼저 묻는 OS (Windows `OnTestKeyDown`) | 엔진은 부작용 없이 답하는 `InputEngine::wants_key`를 갖고, `false`는 "이 키로 `process_key`는 아무것도 하지 않는다"는 약속이다(`ime_core::contract`가 모든 엔진을 시험). `true`는 넉넉해도 된다 |
| 후보창을 띄울 수 없을 때 | 변환을 되돌려 보이지 않는 선택이 일어나지 않게 한다(Wayland와 같은 규칙. Windows는 `Typist::candidates_not_shown`) |
| 전환 키로 쓸 수 있는 키 | `KeyRole`로 추상화했다. 키가 없는 OS는 설정 선택지와 기본값을 제품 결정으로 정한다 |
| 상태 표시 방법 | 정보(현재 언어)는 같고 표현(박스, 트레이, 메뉴 막대)은 OS를 따른다 |
| 글꼴과 글자꼴 | 언어별 지역 글자꼴 원칙(DESIGN.md 6절)은 같고 글꼴 이름은 OS별이다 |
| 사전과 설정 위치 | OS의 관례를 따른다(위 표). 코드는 `Os`를 인자로 받아 어느 OS에서든 테스트한다 |

## 새 OS 어댑터를 만들 때

1. 위의 **계약** 11개를 구현한다. 코어에 OS 코드를 넣지 않는다. 코어가 부족하면 코어를 고치고 모든 OS의 CI를 통과시킨다.
2. 키 코드 표와 `toggle_role`에 해당하는 분류를 만든다. 오른쪽 Alt 판정이 AltGr과 섞이지 않는지 시험한다.
3. 설정 파일 감시, 사전 위치, 시스템 언어 읽기를 그 OS의 방법으로 채우고 이 문서의 **OS 의존 항목 목록**을 고친다.
4. 코어 테스트가 그 OS의 CI에서 통과하는 것과, 그 OS에서 설치하고 실제로 입력하는 것을 **구분해서 기록한다**. 앞의 것이 뒤의 것을 대신하지 않는다([status.md](status.md), DESIGN.md 8절).
5. 한·일·중 가운데 시험한 언어와 앱을 따로 적는다. 한 언어의 성공을 다른 언어의 검증으로 세지 않는다.
