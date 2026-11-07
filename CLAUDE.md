# easime — Easy Eastern Asian IME

Rust로 직접 만드는 다국어 입력기 (한국어 → 일본어 → 중국어).
**Linux 우선**, 그다음 **Windows**, 그다음 macOS를 지원한다.

## 목표
기존 입력기(ibus-hangul, fcitx5, nimf, mozc, MS IME 등)에 의존하지 않고 **Rust로 직접 만든 입력기**로 입력한다.
- 언어: 1차 **한국어**(두벌식) → 단기 **일본어**(로마자→가나, 그다음 Mozc 참고 한자 변환) → 장기 **중국어**(병음, 간체/번체)
- 플랫폼: **Linux(KDE Plasma 6 Wayland 최우선 → GNOME)** → **Windows** → macOS
- 앱: 가능한 한 **모든 앱** (Wayland 네이티브, Electron/IDE, XWayland, 게임/Wine)
- 배포: Linux는 **GitHub Releases에 .deb/.rpm**, Windows/macOS는 각 OS의 설치 패키지

## 결정 사항 (2026-11-07 브레인스토밍)
| 항목 | 결정 |
|---|---|
| 한/영 전환 키 | **한/영 키** 또는 **오른쪽 Alt** 중 GUI에서 선택 (기본: 한/영 키) |
| 언어 전환 | **순환**: 한 키로 영어 → 한국어 → 일본어 → 중국어 → … (설정에서 켠 언어만, 순서 변경 가능) |
| 입력 상태 범위 | **전체 공통** (모든 창이 하나의 상태 공유). 창별 기억은 나중 옵션 |
| 한국어 자판 | **두벌식만**. 엔진은 세벌식을 나중에 붙일 수 있게 설계 |
| 백스페이스 | **자모 단위** 기본 (`한` → `하` → `ㅎ`), 설정에서 글자 단위로 변경 가능 |
| 한자 변환(한국어) | **나중에**. 후보창을 처음 시험하는 대상으로 사용 |
| 일본어 | **로마자 입력**. 1단계 가나만 → 2단계 한자 변환(Mozc 데이터 참고) |
| 중국어 | **병음 입력**, 출력은 **간체/번체 둘 다** (설정에서 선택) |
| 대상 앱 | 브라우저, 터미널/KDE 앱, Electron/IDE, 게임/Wine/Steam — **가능한 한 전부** |
| 플랫폼 순서 | **Linux(KDE → GNOME) → Windows → macOS** |
| Linux 데스크톱 | **KDE Plasma 6** 최우선, 그다음 **GNOME** |
| Linux 패키지 | **GitHub Releases에 .deb / .rpm** (GitHub Actions로 빌드) |
| 설정 GUI | **OS별 네이티브**: Linux = Kirigami, Windows = WinUI 3, macOS = SwiftUI |
| 상태 표시 | **트레이 아이콘** + **커서 아래 사각 박스**(`한`/`A`/`あ`/`中`): 전환할 때와 포커스가 들어올 때 약 1초 표시 |
| 학습/사용자 사전 | **일본어 한자 변환 단계에서** 추가, 로컬에만 저장 |
| 이름 | **easime** (Easy Eastern Asian IME) 확정 |
| 라이선스 | **MIT OR Apache-2.0** |
| 저장소 | GitHub 공개 `byeonggonlee/easime`, 기본 브랜치 `main` |

### 라이선스 메모
- Mozc는 코드가 **BSD-3-Clause**이고, OSS 사전 데이터도 허용적 라이선스(IPAdic 계열 등)이다.
  따라서 MIT/Apache-2.0 프로젝트에서 참고하거나 포함할 수 있다. 저작권 고지만 유지하면 된다.
- Qt/Kirigami(LGPL)는 동적 링크이므로 문제없다.
- 제3자 데이터는 `data/<출처>/`에 원본 LICENSE를 함께 두고 `THIRD_PARTY_LICENSES.md`에 기록한다.
- 중국어 사전 후보 중 GPL/CC BY-SA 데이터(예: libpinyin, CC-CEDICT)는 본체와 섞지 말고
  **별도 데이터 패키지**로 분리하거나, 허용적 라이선스 데이터를 우선 검토한다.

## 플랫폼별 연결 방식
### Linux — KDE Plasma 6 (Wayland) ★ 최우선, 사용자의 주 환경 (Fedora, Plasma 6.8.x)
- KWin이 실행하는 Wayland input method 프로그램: `zwp_input_method_v1` (+ 키보드 grab)
- 후보창과 커서 아래 상태 박스는 `zwp_input_panel_v1`의 panel surface (KWin이 커서 근처에 배치)
- 앱 쪽은 KWin이 `text-input-v1/v2/v3`로 중계 → GTK/Qt/Firefox/Chromium(Wayland 모드)
- 등록: 시스템 설정 → 키보드 → 가상 키보드 (`.desktop`에 `X-KDE-Wayland-VirtualKeyboard=true`)
- Electron/Chromium: Wayland 모드 + `--enable-wayland-ime` 등 플래그 필요 → 설정 GUI에서 안내
- **XWayland 앱 (게임, Wine, Steam, 일부 IDE)**: 데몬 안의 **XIM 서버**(`x11rb`)로 대응 (`XMODIFIERS=@im=easime`)

### Linux — GNOME (Wayland)
- GNOME(Mutter/GNOME Shell)은 외부 입력기용 Wayland 프로토콜을 지원하지 않고 **IBus D-Bus 프로토콜만** 사용한다.
- 계획: ibus 패키지를 쓰지 않고 **IBus 호환 D-Bus 서비스를 easime가 직접 구현**한다 (`frontend-ibus`).
  착수 전에 GNOME Shell이 IBus 주소를 찾는 방식과 필요한 인터페이스 범위를 조사한다 (조사 필요).
- GNOME에는 기본 트레이가 없으므로 상태 표시는 GNOME 자체 입력 소스 표시기와 커서 아래 박스에 의존한다.
- wlroots 계열(Sway, Hyprland)은 `input-method-v2`를 쓰므로 필요하면 나중에 추가할 수 있다.

### Windows (Linux 다음)
- **TSF(Text Services Framework)** 텍스트 서비스: `windows` crate로 COM 인터페이스 구현, `cdylib` DLL로 빌드
- TSF DLL은 각 앱 프로세스에 로드된다 → "전체 공통 입력 상태"를 위해 상태 공유 방법이 필요하다
  (공유 메모리 또는 named pipe 서비스).
- 후보창과 상태 박스는 Win32 레이어드 창으로 그린다. 상태는 알림 영역 아이콘(또는 언어 바)으로 표시한다.
- 설치: MSI(WiX) 또는 설치 프로그램 + TSF 등록(`regsvr32` 방식). 코드 서명은 선택 사항.

### macOS (마지막)
- **InputMethodKit**: Objective-C/Swift 껍데기 + Rust 정적 라이브러리, `~/Library/Input Methods`에 `.app` 번들 설치
- 상태 표시는 OS 메뉴 막대의 입력 소스 메뉴를 사용한다.
- 배포: `.pkg`. 일반 배포에는 Apple 개발자 계정(서명, 공증)이 사실상 필요하다.

## 아키텍처
Cargo workspace. **언어 엔진은 OS 독립**, 프론트엔드와 설정 GUI는 OS별로 둔다.

```
crates/
  ime-core/          공통 타입, InputEngine trait, LanguageSwitcher (의존성 최소, OS 독립)
  engine-hangul/     순수 Rust 한글 조합 엔진 (외부 의존성 없음)
  engine-japanese/   로마자→가나, 나중에 Mozc 사전 기반 변환
  engine-chinese/    (장기) 병음 → 간체/번체
  easime-config/     설정 스키마 + TOML 읽기/쓰기 + OS별 경로 (모든 프론트엔드와 GUI가 공유)
  easime-ffi/        Swift/C#용 바인딩 (UniFFI 검토) — 네이티브 설정 앱과 macOS 프론트엔드가 사용
  frontend-wayland/  KWin input-method-v1 (Linux)
  frontend-xim/      XIM 서버 (XWayland, X11)
  frontend-ibus/     IBus 호환 D-Bus 서비스 (GNOME)
  frontend-tsf/      Windows TSF DLL
  easime/            Linux 데몬 바이너리 (Wayland + XIM + IBus 프론트엔드, 트레이, D-Bus)
settings/
  kde/               Linux 설정 앱: Rust + cxx-qt + QML + Kirigami
  windows/           Windows 설정 앱: WinUI 3
  macos/             macOS 설정 앱: SwiftUI
platform/
  macos/             InputMethodKit 앱 번들 껍데기
packaging/           deb/rpm 메타데이터, WiX, macOS pkg 스크립트
data/                .desktop 파일, 아이콘, 제3자 사전 데이터와 LICENSE
```

### 핵심 trait (`ime-core`)
- `trait InputEngine { fn process_key(&mut self, key: KeyEvent) -> EngineOutput; fn reset(&mut self); fn flush(&mut self) -> String; }`
- `EngineOutput { commit: String, preedit: Preedit, candidates: Option<CandidateList>, handled: bool }`
- **후보 목록을 처음부터 API에 포함한다** (일본어/중국어 필수, 한국어 한자 변환에도 사용).
- preedit는 하이라이트 구간(segment)과 커서 위치를 표현할 수 있게 한다 (일본어 문절 변환 대비).
- `KeyEvent`는 OS 독립 표현으로 정의한다 (keysym/가상 키 코드 → 공통 키 enum 변환은 각 프론트엔드가 담당).

### 한글 엔진 (`engine-hangul`)
- 두벌식 표준 자판, 겹모음(ㅘ,ㅙ,ㅚ,ㅝ,ㅞ,ㅟ,ㅢ), 겹받침(ㄳ,ㄵ,ㄶ,ㄺ,ㄻ,ㄼ,ㄽ,ㄾ,ㄿ,ㅀ,ㅄ)
- 종성 → 다음 음절 초성 이동 (`갑`+`ㅏ` → `가바`, `값`+`ㅏ` → `갑사`)
- 백스페이스: 자모 단위(기본) / 글자 단위
- 음절 합성: `0xAC00 + (초성*21 + 중성)*28 + 종성`

## 설정 GUI — OS별 네이티브
모든 설정 앱은 같은 설정 파일 형식(`easime-config`)을 읽고 쓴다. 화면 구성도 OS 간에 동일하게 유지한다.
- **Linux (KDE Plasma 6)**: Rust + cxx-qt (Qt 6) + QML + **Kirigami**
  - Plasma 시스템 설정과 똑같이 보이고 동작해야 한다 (Breeze, 다크/라이트, 강조색, 글꼴, HiDPI).
  - 단독 앱으로 시작하고, 나중에 KCM으로 옮길 수 있게 만든다. GNOME에서도 동작은 해야 한다.
- **Windows**: **WinUI 3** (Windows 11 Fluent 디자인)
- **macOS**: **SwiftUI**
- 공통 페이지 구성 (사이드바):
  - **일반**: 자동 시작, 트레이 아이콘, 커서 아래 상태 박스 켜기/끄기
  - **언어**: 순환에 넣을 언어 선택과 순서 (영어/한국어/일본어/중국어)
  - **단축키**: 전환 키 선택 (한/영 키 / 오른쪽 Alt)
  - **한국어**: 자판(두벌식), 백스페이스 단위, (나중에) 한자 변환 키
  - **일본어**: 로마자 표, 가나 종류, (나중에) 사전/학습 관리
  - **중국어**: 간체/번체 선택
  - **모양**: 후보창 글꼴/크기, 가로/세로 배치, 상태 박스 표시 시간
  - **앱 호환성** (Linux): Electron/Chromium 플래그 안내, XIM 상태 확인
- 설정 파일 위치: Linux `~/.config/easime/config.toml`, Windows `%APPDATA%\easime\config.toml`,
  macOS `~/Library/Application Support/easime/config.toml`. 변경되면 재시작 없이 다시 읽는다.

## 패키징과 배포
- **Linux**: GitHub Actions에서 빌드해 **GitHub Releases에 .deb / .rpm** 업로드
  (`cargo-deb`, `cargo-generate-rpm`). 패키지에는 데몬, 설정 앱, `.desktop`, 아이콘, 사전 데이터를 포함한다.
  의존성: Qt 6, Kirigami(KF6), libxkbcommon, wayland.
- **Windows**: MSI(WiX), TSF DLL 등록 포함
- **macOS**: `.pkg` (서명/공증)
- 버전은 workspace 전체가 하나의 버전을 공유한다. 태그 `vX.Y.Z`를 푸시하면 릴리스 워크플로가 동작한다.

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
  - `engine-hangul`: 두벌식, 겹모음/겹받침, 자모/글자 단위 백스페이스
  - `engine-japanese`: 로마자 → 히라가나/가타카나 (Enter 확정, Space 확정+공백, Esc 취소). 한자 변환은 아직 없음
  - `frontend-wayland` + `easime` 데몬: KWin `zwp_input_method_v1` 클라이언트, 키 반복(자체 타이머와 컴포지터 `repeated`),
    한/영 키와 오른쪽 Alt 탭 전환, Caps Lock 무시, 언어 순환, 설정 읽기
  - CI: Linux는 전체, Windows/macOS는 OS 독립 crate만 (`fmt`, `clippy -D warnings`, `test`)
- **가짜 KWin 테스트 하네스**: `crates/frontend-wayland/tests/` (순수 Rust Wayland 서버로 activate → 키 입력 → commit/preedit를
  검증). 실제 KWin이 없어도 프로토콜 흐름을 시험할 수 있고, 이미 실제 버그(`KeyState::Repeated` 처리)를 잡았다.
- **아직 실제 KWin에서 시험하지 않았다.** 일반 클라이언트에는 `zwp_input_method_v1`이 노출되지 않아
  (KWin 6.8.2 확인: 입력기로 등록된 프로세스에게만 제공) 가상 키보드로 등록해야 시험할 수 있다.
- 알려진 미구현: 설정 변경 즉시 반영(파일 감시), 트레이/상태 박스, 후보창, 설정 GUI, XIM, 일본어 한자 변환, 중국어, 패키징
- 알려진 한계: 한글 자판은 활성 키보드 레이아웃의 keysym을 따른다(물리 키 위치 기준 아님). 레이아웃이 US가 아니면 어긋날 수 있다.

## 작업 습관 (이 저장소에서 있었던 실수에서)
- 커밋은 작게, 커밋 직후 푸시. 검사(`fmt`/`clippy -D warnings`/`test`)는 `&&`로 이어서 실패하면 커밋되지 않게 한다.
- `git add -A` 대신 경로를 지정한다 (에이전트 워크트리가 딸려 들어간 적이 있음).
- OS 독립 crate의 테스트는 절대경로·구분자를 하드코딩하지 않는다 (Windows CI가 잡아낸 적이 있음).

## 개발 모드로 KWin에서 시험하기 (사용자 환경을 바꾸므로 사용자가 직접 실행)
```sh
./scripts/install-dev.sh     # release 빌드 + ~/.local/share/applications/easime.desktop 등록
```
1. 시스템 설정 → 키보드 → 가상 키보드 → easime 선택 → 로그아웃/로그인
2. 로그: `~/.local/state/easime/easime.log`
3. 문제가 생겨 입력이 안 되면 같은 화면에서 원래 항목(Plasma Keyboard 등)을 고르거나
   `kwriteconfig6 --file kwinrc --group Wayland --key InputMethod ""` 후 재로그인
- 현재 `kwinrc [Wayland] InputMethod` 값: `/usr/share/applications/org.kde.plasma.keyboard.desktop` (복구용으로 기억할 것)

## 개발 규칙
- **사용자와의 대화는 항상 한국어로 한다** (전역 규칙). 코드 식별자, 주석, 커밋 메시지는 영어.
- 엔진 crate(`ime-core`, `engine-*`)에는 OS/UI 의존성(Wayland, X11, Qt, Win32 등)을 넣지 않는다.
  Windows/macOS에서도 그대로 컴파일되어야 한다.
- 새 언어 엔진은 `InputEngine` trait만 구현하면 모든 프론트엔드와 설정 앱에 붙을 수 있어야 한다.
- OS별 코드는 해당 crate 또는 `#[cfg(target_os = ...)]` 안에만 둔다.
- 사전 데이터를 추가할 때는 라이선스를 확인하고 `THIRD_PARTY_LICENSES.md`에 출처를 기록한다.
- 변경 후 `cargo fmt`, `cargo clippy --all-targets`, `cargo test` 통과를 확인한다.
- 결정이 필요한 질문은 선택 화면(AskUserQuestion)으로 묻는다.

## 명령어
```sh
cargo build --release
cargo test -p engine-hangul
cargo run -p easime            # 수동 실행은 디버깅 용도 (평소에는 KWin이 실행)
```

## 참고
- Rust 툴체인: `rustup` stable (cargo 1.99.1 확인)
- Git: 기본 브랜치 `main`, 원격 `origin` = GitHub 공개 저장소 `byeonggonlee/easime` (`gh` 로그인됨)
- Linux 빌드 의존성(Fedora): `libxkbcommon-devel`, `wayland-devel`, `libxcb-devel`, `dbus-devel`,
  설정 GUI용 `qt6-qtbase-devel`, `qt6-qtdeclarative-devel`, `kf6-kirigami-devel`
- 유니코드: 한글 음절 U+AC00–U+D7A3, 호환 자모 U+3131–U+318E, 히라가나 U+3040–U+309F, 가타카나 U+30A0–U+30FF
- Mozc: https://github.com/google/mozc (BSD-3-Clause, OSS 사전은 `src/data/dictionary_oss`)
