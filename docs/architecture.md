# easime 아키텍처와 플랫폼별 연결 방식

플랫폼 프로토콜, crate 구성, 핵심 trait, 설정 GUI, 패키징을 다룬다.
**무엇이 모든 OS 공통 코어이고 무엇이 OS마다 다른지는 [platforms.md](platforms.md)에 있다.**

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
- 설치: **MSI(WiX 5)** + TSF 등록(`regsvr32` 방식). 코드 서명은 선택 사항. 설치 프로그램의 틀은 이미 있다(`packaging/windows`, 아래 "패키징과 배포"). MSI는 TSF DLL을 Windows에 등록하고(`regsvr32`) 설치한 사용자의 언어 목록에 한국어를 켠다(사용자가 Windows 설정에서 언어를 직접 추가하지 않아도 된다). 텍스트 서비스는 한글을 TSF 조합으로 쓴다(실제 PC에서는 아직 시험하지 않았다).

### macOS (마지막)
- **InputMethodKit**: Objective-C/Swift 껍데기 + Rust 정적 라이브러리, `~/Library/Input Methods`에 `.app` 번들 설치
- 상태 표시는 OS 메뉴 막대의 입력 소스 메뉴를 사용한다.
- 배포: `.pkg`. 일반 배포에는 Apple 개발자 계정(서명, 공증)이 사실상 필요하다.

## 아키텍처
Cargo workspace. **언어 엔진은 OS 독립**, 프론트엔드와 설정 GUI는 OS별로 둔다.

```
crates/
  ime-core/          공통 타입, InputEngine trait, LanguageSwitcher, CandidateList (의존성 최소, OS 독립)
  ime-session/       모든 프런트엔드가 쓰는 Session: 엔진 구성, 언어 순환, 전환 키 판정, 사전 공유 (OS 독립)
  engine-hangul/     순수 Rust 한글 조합 엔진 (외부 의존성 없음)
  engine-japanese/   로마자→가나, 사전 기반 변환(Mozc OSS 사전), 사전 로더
  easime-dict-tool/  Mozc 사전 → easime 이진 사전 변환 도구 (OS 독립)
  engine-chinese/    (장기) 병음 → 간체 / 번체 (두 입력 언어)
  easime-config/     설정 스키마 + TOML 읽기/쓰기 + OS별 경로 (모든 프론트엔드와 GUI가 공유)
  easime-ffi/        Swift/C#용 바인딩 (UniFFI 검토) — 네이티브 설정 앱과 macOS 프론트엔드가 사용
  frontend-wayland/  KWin input-method-v1 (Linux)
  frontend-xim/      XIM 서버 (XWayland, X11)
  frontend-ibus/     IBus 호환 D-Bus 서비스 (GNOME)
  frontend-tsf/      Windows TSF DLL (키 입력 해석, 키 처리 파이프라인 `typing`(Windows API 없음), 프로필 하나의 등록과 순환 설정 `profiles`, TSF 등록, 한글을 TSF 조합으로 쓰는 텍스트 서비스, 전환 때 커서 옆에 뜨는 모드 상자 `indicator`와 그 위치 규칙 `placement`. 후보창·밑줄은 아직)
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
  - **중국어**: 간체와 번체는 별개 입력 언어(`简体中文`, `繁體中文`)로 언어 페이지에 따로 나온다. 중국어 전용 페이지는 병음 옵션이 생길 때 추가한다
  - **모양**: 후보창 글꼴/크기, 가로/세로 배치, 상태 박스 표시 시간
  - **앱 호환성** (Linux): Electron/Chromium 플래그 안내, XIM 상태 확인
- **단계 (2026-11-07 결정)**: 목표는 초보자용과 전문가용을 나누는 것이고, 단기에는 **전문가용만** 만든다. 전문가용은 아래 페이지에서 모든 옵션을 바로 바꾸고, 변경은 즉시 저장한다. 일본어·중국어 페이지는 실제 옵션이 생길 때 추가한다(빈 페이지를 만들지 않는다).
- **Linux 구현** (`settings/kde`, 메인 워크스페이스와 별개의 cargo 프로젝트: Qt 6와 Kirigami가 필요하므로):
  - `src/model.rs`: Qt 없는 상태·저장 로직(단위 테스트). `src/view.rs`: QML이 묶는 값. `src/backend.rs`: cxx-qt QObject(`EasimeSettings`, QML 싱글톤). `qml/`: Kirigami 화면(페이지마다 파일 하나).
  - 저장은 **바뀐 키만** `toml_edit`으로 고친다. 직접 고친 파일의 주석, 키 순서, 모르는 키가 남는다. 읽을 수 없는 파일은 절대 덮어쓰지 않고(변경을 거부하고 알림), 초기화는 `config.toml.bak`을 남긴다.
  - 화면의 문구는 모두 `qsTr()`이다. 번역(일본어 등)은 해당 언어 사용자의 검토가 필요하다.
- 설정 파일 위치: Linux `~/.config/easime/config.toml`, Windows `%APPDATA%\easime\config.toml`,
  macOS `~/Library/Application Support/easime/config.toml`. 변경되면 재시작 없이 다시 읽는다.

## 패키징과 배포
- **Linux**: GitHub Actions에서 빌드해 **GitHub Releases에 .deb / .rpm** 업로드(`cargo-deb`, `cargo-generate-rpm`).
  **배포판당 패키지는 하나다**(사용자 요청, 2026-11-07: 키보드와 설정을 따로 설치하게 하지 않는다). 데몬(`easime`), 설정 창(`easime-settings`), 가상 키보드 등록용 `easime.desktop`(메뉴에는 안 보임), 앱 메뉴의 "easime 설정"(`easime-settings.desktop`, 이름은 한·일·번체·간체로 현지화), 일본어 사전이 한 파일에 들어 있다. 패키지 메타데이터는 `crates/easime/Cargo.toml`에 있다.
  - Qt 6와 Kirigami가 있는 환경에서 만든다: `.rpm`은 Fedora 컨테이너에서, `.deb`는 Debian 13(trixie) 컨테이너에서 그 배포판의 라이브러리로 만든다. 그러므로 `.deb`는 Debian 13 이상, Ubuntu는 Plasma 6와 Qt가 충분히 새로운 25.04 이상을 대상으로 한다. 시험한 것은 clean `debian:trixie`와 `fedora:latest` 컨테이너뿐이다.
  - 데몬이 Qt 6·Kirigami에 의존하게 되므로 KDE가 아닌 데스크톱(GNOME 등)에 설치하면 Qt가 함께 설치된다. GNOME 지원이 생기면 다시 나눌지 따진다.
  - 빌드와 검증은 `.github/workflows/packages.yml`(재사용 워크플로) 하나다. CI와 릴리스(`release.yml`)가 같은 흐름과 **같은 실제 Mozc 사전**을 쓴다. 사전은 먼저 한 번 만들어 おおさか→大阪 같은 조회를 확인한 뒤 두 패키지가 쓴다. 각 패키지는 clean 컨테이너(Fedora, Debian)에 배포판의 의존성만으로 설치하고, 설치된 데몬이 시작되어 이유를 말하며 멈추는지(컴포지터가 없으므로)와 설치된 설정 창이 모든 페이지를 여는지 확인한 뒤에야 두 파일이 한 아티팩트(`easime-linux-packages-<commit>`)로 묶인다.
  - 패키지를 설치해도 easime가 입력기로 **선택되지는 않는다**: 시스템 설정 → 키보드 → 가상 키보드에서 고르고 다시 로그인해야 한다. 사용자의 데스크톱 설정을 설치 스크립트가 몰래 바꾸지 않기 때문이다(AGENTS.md). 확인과 되돌리기를 갖춘 활성화 화면은 초보자용 첫 실행 안내의 몫이다.
- **Windows**: **MSI(WiX 5)**, `packaging/windows/easime.wxs`. `build.ps1`이 만들고 `test.ps1`이 CI의 Windows 러너에 실제로 설치·사용·업그레이드·제거를 시험한다.
  담는 것: 텍스트 서비스 DLL(`easime_tsf.dll`, 64비트), 터미널 데모(`easime-cli.exe`), 라이선스.
  - **설치 때** DLL을 Windows에 등록하고(`regsvr32`, 실패하면 설치가 멈추고 뒤에서 실패하면 등록을 되돌린다), **설치한 사용자의** 언어 목록에 그 언어의 easime 프로필을 켠다(DLL의 `rundll32` 내보내기 `EnableForCurrentUser <언어>`가 `InstallLayoutOrTip`을 부른다. 그 언어가 목록에 없으면 언어도 함께 추가된다). 설치 창(WiX UI의 기능 트리)에서 언어를 기능("easime keyboard (listed under Korean)")으로 고르게 하므로 그 화면이 확인이고, 무인 설치(`/qn`)는 기본값(한국어)을 따른다. 기본 설치 폴더는 `C:\Program Files\easime`.
  - **제거 때** 그 사용자의 켠 프로필을 끄고(`DisableForCurrentUser`; 설치가 언어까지 더했다면 그 언어도 목록에서 사라지는 것을 CI에서 확인했다. 설치 전에 이미 있던 언어는 남을 것으로 예상하며 확인하지 않았다) Windows 등록을 푼 다음 파일을 지운다. 되돌리기는 제거다. 업그레이드는 옛 버전을 먼저 제거하고(이 과정이 위와 같다) 새 버전을 설치한다.
  - **같은 PC의 다른 사용자**는 등록된 프로필을 Windows 설정의 "키보드 추가"에서 직접 켠다(등록은 기계 전체, 켜기는 설치한 사용자만).
  - **타자할 수 있는 언어만** 켠다: `frontend-tsf::profiles::TYPEABLE`(지금은 한국어). 일본어·중국어는 후보창 등이 생겨 실제로 입력되면 기능과 프로필을 함께 더한다.
  - **한계**: DLL이 64비트뿐이라 32비트 프로그램에서는 쓸 수 없다. ARM64 Windows의 ARM64 프로그램도 마찬가지일 것으로 예상한다(확인하지 않음). 입력기를 켜서 쓰는 중에 제거·업그레이드하면 DLL이 프로그램에 불려 있어 다시 로그인하거나 재시작해야 지워질 수 있다고 예상한다(CI에서는 DLL을 쓰는 프로그램이 없어 확인하지 못했다). 코드 서명이 없어 설치할 때 SmartScreen 경고가 뜰 수 있다(예상).
  - 릴리스에는 올리지 않고 CI 아티팩트(`easime-windows-installer-<commit>`)로만 둔다: 키 처리가 끝나지 않았고 실제 PC에서 시험한 적이 없다.
  - CI의 `test.ps1`이 확인하는 것: 설치 후 Windows의 COM 클래스(설치된 DLL을 가리킴)와 TSF 프로필 등록(한국어만), 설치한 사용자의 언어 설정에 한국어 켜짐, 설치된 DLL을 COM이 불러 서비스를 만드는 것, 한국어 기능을 뺀 설치(등록은 있고 켜짐은 없음), 제거 후 아무것도 남지 않음(폴더, Windows 등록, 사용자 언어 목록), `ADDLOCAL=ALL`, 업그레이드(등록과 켜짐 유지), 다운그레이드 거부. 같은 단계의 결과와 `easime.log`가 CI 로그에 남는다.
  시스템 PATH에 폴더를 더하는 기능은 기본이 꺼져 있다(`ADDLOCAL=ALL`로 켠다): 시스템 설정을 바꾸는 일이기 때문이다.
- **macOS**: `.pkg` (서명/공증)
- 버전은 workspace 전체가 하나의 버전을 공유한다. 태그 `vX.Y.Z`를 푸시하면 릴리스 워크플로가 동작한다.

