# easime 아키텍처와 플랫폼별 연결 방식

플랫폼 프로토콜, crate 구성, 핵심 trait, 설정 GUI, 패키징을 다룬다.
**무엇이 모든 OS 공통 코어이고 무엇이 OS마다 다른지는 [platforms.md](platforms.md)에 있다.**
무엇이 구현되어 있고 어디까지 확인했는지는 [status.md](status.md)와 [구현 현황](implementation/core.md)에 있다. 이 문서는 구조와 연결 방식을 다루며, 계획인 것(GNOME, macOS, XIM, 중국어)과 구현된 것을 각 항목에 표시한다.

## 플랫폼별 연결 방식
### Linux — KDE Plasma 6 (Wayland) ★ 최우선, 사용자의 주 환경 (Fedora, Plasma 6.8.x) — 구현됨
- KWin이 실행하는 Wayland input method 프로그램: `zwp_input_method_v1` (+ 키보드 grab)
- 후보창과 커서 아래 상태 박스는 `zwp_input_panel_v1`의 panel surface (KWin이 커서 근처에 배치)
- 앱 쪽은 KWin이 `text-input-v1/v2/v3`로 중계 → GTK/Qt/Firefox/Chromium(Wayland 모드)
- 등록: 시스템 설정 → 키보드 → 가상 키보드 (`.desktop`에 `X-KDE-Wayland-VirtualKeyboard=true`)
- Electron/Chromium: Wayland 모드 + `--enable-wayland-ime` 등 플래그 필요 → 설정 GUI에서 안내
- **XWayland 앱 (게임, Wine, Steam, 일부 IDE) — 계획(아직 없음)**: 데몬 안의 **XIM 서버**(`x11rb`)로 대응 (`XMODIFIERS=@im=easime`)

### Linux — GNOME (Wayland) — 계획
- GNOME(Mutter/GNOME Shell)은 외부 입력기용 Wayland 프로토콜을 지원하지 않고 **IBus D-Bus 프로토콜만** 사용한다.
- 계획: ibus 패키지를 쓰지 않고 **IBus 호환 D-Bus 서비스를 easime가 직접 구현**한다 (`frontend-ibus`).
  착수 전에 GNOME Shell이 IBus 주소를 찾는 방식과 필요한 인터페이스 범위를 조사한다 (조사 필요).
- GNOME에는 기본 트레이가 없으므로 상태 표시는 GNOME 자체 입력 소스 표시기와 커서 아래 박스에 의존한다.
- wlroots 계열(Sway, Hyprland)은 `input-method-v2`를 쓰므로 필요하면 나중에 추가할 수 있다.

### Windows (Linux 다음) — 구현됨, 실제 PC 입력 시험(W1–W14) 문제 없음 보고
- **TSF(Text Services Framework)** 텍스트 서비스: `windows` crate로 COM 인터페이스를 구현하고 `cdylib` DLL(`easime_tsf.dll`)로 빌드한다. 한국어 아래 프로필 하나를 등록하고, 그 안에서 영어 → 한국어 → 일본어를 한 키로 순환한다.
- TSF DLL은 각 앱 프로세스에 로드되므로 모드는 프로세스마다 따로다. "전체 공통 입력 상태"를 위한 공유(공유 메모리 또는 named pipe 서비스)는 아직 없다.
- 조합은 TSF composition과 표시 속성(밑줄)으로, 후보창과 상태 박스는 Win32(GDI) 창으로 그린다. 후보를 직접 그리는 앱(전체 화면 게임 등)에는 `ITfCandidateListUIElement`로 목록을 넘긴다. 상시 표시는 TSF 언어 바 항목(입력 모드 단추, `langbar`)으로 작업 표시줄 알림 영역에 보이게 한다(클릭은 다음 언어, 메뉴는 언어 목록과 설정 앱).
- 설치: **WiX 5** MSI + 번들 `setup.exe`(`packaging/windows`, 아래 "패키징과 배포"). MSI가 DLL을 등록하고(`regsvr32`) 설치한 사용자의 입력 방법에 easime를 켠다. 코드 서명은 선택 사항이며 아직 없다.
- 설정 앱은 WinUI 3(Rust + `windows-reactor`)다. 구현 현황과 검증 범위: [implementation/windows.md](implementation/windows.md).

### macOS (마지막) — 계획
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
  easime-settings-model/ 설정 앱이 보여 주는 값과 변경을 저장하는 규칙, 일본어 사전 상태 (UI 툴킷 없음, 모든 OS의 설정 앱이 공유)
  easime-ffi/        Swift/C#용 바인딩 (UniFFI 검토) — 네이티브 설정 앱과 macOS 프론트엔드가 사용
  frontend-wayland/  KWin input-method-v1 (Linux)
  frontend-xim/      XIM 서버 (XWayland, X11)
  frontend-ibus/     IBus 호환 D-Bus 서비스 (GNOME)
  frontend-tsf/      Windows TSF DLL: 키 처리 파이프라인(`typing`, Windows API 없음), 프로필 하나의 등록과 순환 설정, 한글·일본어를 TSF 조합으로 쓰는 텍스트 서비스, 모드 상자, 조합 밑줄, 후보창과 TSF UI 요소 (모듈별 설명: implementation/windows.md)
  easime/            Linux 데몬 바이너리 (Wayland + XIM + IBus 프론트엔드, 트레이, D-Bus)
settings/
  kde/               Linux 설정 앱: Rust + cxx-qt + QML + Kirigami
  windows/           Windows 설정 앱: Rust + windows-reactor (WinUI 3)
  macos/             macOS 설정 앱: SwiftUI
platform/
  macos/             InputMethodKit 앱 번들 껍데기
packaging/           deb/rpm 메타데이터, WiX, macOS pkg 스크립트
data/                .desktop 파일, 아이콘, 제3자 사전 데이터와 LICENSE
```

### 핵심 trait (`ime-core`)
- `trait InputEngine { fn process_key(&mut self, key: KeyEvent) -> EngineOutput; fn wants_key(&self, key: KeyEvent) -> bool; fn reset(&mut self); fn flush(&mut self) -> String; }`
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
- **Windows**: **WinUI 3** (Windows 11 Fluent 디자인). Rust + `windows-reactor`(windows-rs의 Rust용 WinUI 3 라이브러리)로 만들고, Linux 설정 앱의 Qt 없는 저장 로직을 같이 쓴다. Windows App Runtime은 설치된 Microsoft 프레임워크 패키지를 쓰고(framework-dependent), 설치 프로그램(번들)이 없을 때만 Microsoft에서 받아 설치한다. 결정: [decisions.md](decisions.md) "설정 GUI"
- **macOS**: **SwiftUI**
- 공통 페이지 구성 (사이드바):
  - **일반**: 입력기 활성화·되돌리기(KDE), 지원·사전 상태, 설정 파일 위치·다시 읽기·초기화, (나중에) 자동 시작·트레이 아이콘 켜기/끄기
  - **언어**: 순환에 넣을 언어 선택과 순서 (영어/한국어/일본어/중국어)
  - **단축키**: 전환 키 선택 (한/영 키 / 오른쪽 Alt 단독 탭 / 둘 다)
  - **한국어**: 자판(두벌식), 백스페이스 단위, (나중에) 한자 변환 키
  - **일본어**: 로마자 표, 가나 종류, (나중에) 사전/학습 관리
  - **중국어**: 간체와 번체는 별개 입력 언어(`简体中文`, `繁體中文`)로 언어 페이지에 따로 나온다. 중국어 전용 페이지는 병음 옵션이 생길 때 추가한다
  - **모양**: 커서 옆 상태 박스 켜기/끄기·표시 시간, (나중에) 후보창 글꼴/크기, 가로/세로 배치
  - **앱 호환성** (Linux, 나중에): Electron/Chromium 플래그 안내, XIM 상태 확인
- **단계 (2026-11-07 결정)**: 목표는 초보자용과 전문가용을 나누는 것이고, 단기에는 **전문가용만** 만든다. 전문가용은 아래 페이지에서 모든 옵션을 바로 바꾸고, 변경은 즉시 저장한다. 일본어·중국어 페이지는 실제 옵션이 생길 때 추가한다(빈 페이지를 만들지 않는다).
- **Linux 구현** (`settings/kde`, 메인 워크스페이스와 별개의 cargo 프로젝트: Qt 6와 Kirigami가 필요하므로):
  - `crates/easime-settings-model`(메인 워크스페이스, 모든 OS에서 테스트): UI 툴킷 없는 상태·저장 로직(`model`)과 사전 상태(`dictionary`). Windows 설정 앱도 같은 것을 쓴다. `src/activation.rs`: KDE의 입력기 선택(kwinrc)과 되돌리기. `src/view.rs`: QML이 묶는 값. `src/backend.rs`: cxx-qt QObject(`EasimeSettings`, QML 싱글톤). `qml/`: Kirigami 화면(페이지마다 파일 하나).
  - 저장은 **바뀐 키만** `toml_edit`으로 고친다. 직접 고친 파일의 주석, 키 순서, 모르는 키가 남는다. 읽을 수 없는 파일은 절대 덮어쓰지 않고(변경을 거부하고 알림), 초기화는 `config.toml.bak`을 남긴다.
  - 화면의 문구는 모두 `qml/tr.js`의 `Tr.t(...)`를 거친다(한국어 원문을 키로 한 5개 언어 번역표). 한국어 외의 번역은 원어민 검토 전 초안이며 일반 페이지에서 이를 알린다.
- **Windows 구현** (`settings/windows`, 별개의 cargo 프로젝트): `src/app.rs`(windows-reactor 컴포넌트: NavigationView와 페이지, 메시지마다 공유 모델로 저장), `src/tr.rs`(KDE의 `qml/tr.js` 번역표를 빌드 때 읽어 같은 언어 선택 규칙으로 번역), 런타임은 실행 파일 옆에 두지 않는다(설치된 Microsoft 프레임워크 패키지를 쓴다). 입력기 켜기 화면은 없다(설치 프로그램 몫).
- 설정 파일 위치: Linux `~/.config/easime/config.toml`, Windows `%APPDATA%\easime\config.toml`,
  macOS `~/Library/Application Support/easime/config.toml`. 변경되면 재시작 없이 다시 읽는다.

## 패키징과 배포
- **Linux**: GitHub Actions에서 빌드해 **GitHub Releases에 .deb / .rpm** 업로드(`cargo-deb`, `cargo-generate-rpm`).
  **배포판당 패키지는 하나다**(사용자 요청, 2026-11-07: 키보드와 설정을 따로 설치하게 하지 않는다). 데몬(`easime`), 설정 창(`easime-settings`), 가상 키보드 등록용 `easime.desktop`(메뉴에는 안 보임), 앱 메뉴의 "easime 설정"(`easime-settings.desktop`, 이름은 한·일·번체·간체로 현지화), 다국어 키캡 정체성을 담은 앱 아이콘(`data/icons/`, 512×512~16×16 PNG 및 SVG가 `/usr/share/icons/hicolor/`에 설치), 일본어 사전이 한 파일에 들어 있다. 패키지 메타데이터는 `crates/easime/Cargo.toml`에 있다.
  - Qt 6와 Kirigami가 있는 환경에서 만든다: `.rpm`은 Fedora 컨테이너에서, `.deb`는 Debian 13(trixie) 컨테이너에서 그 배포판의 라이브러리로 만든다. 그러므로 `.deb`는 Debian 13 이상, Ubuntu는 Plasma 6와 Qt가 충분히 새로운 25.04 이상을 대상으로 한다. 시험한 것은 clean `debian:trixie`와 `fedora:latest` 컨테이너뿐이다.
  - 데몬이 Qt 6·Kirigami에 의존하게 되므로 KDE가 아닌 데스크톱(GNOME 등)에 설치하면 Qt가 함께 설치된다. GNOME 지원이 생기면 다시 나눌지 따진다.
  - 빌드와 검증은 `.github/workflows/packages.yml`(재사용 워크플로) 하나다. CI와 릴리스(`release.yml`)가 같은 흐름과 **같은 실제 Mozc 사전**을 쓴다. 사전은 먼저 한 번 만들어 おおさか→大阪 같은 조회를 확인한 뒤 두 패키지가 쓴다. 각 패키지는 clean 컨테이너(Fedora, Debian)에 배포판의 의존성만으로 설치하고, 설치된 데몬이 시작되어 이유를 말하며 멈추는지(컴포지터가 없으므로)와 설치된 설정 창이 모든 페이지를 여는지 확인한 뒤에야 두 파일이 한 아티팩트(`easime-linux-packages-<commit>`)로 묶인다.
  - 패키지를 설치해도 easime가 입력기로 **자동 선택되지는 않는다**: 사용자의 데스크톱 설정을 설치 스크립트가 몰래 바꾸지 않기 때문이다(AGENTS.md). 설정 창(`easime-settings`) 일반 화면의 활성화 카드(바꿀 설정 확인과 되돌리기 제공)에서 켜거나 시스템 설정 → 키보드 → 가상 키보드에서 고른 뒤 다시 로그인해야 한다.
- **Windows**: **MSI(WiX 5)**(`packaging/windows/easime.wxs`)와 번들 **`setup.exe`**(`bundle.wxs`). `build.ps1`이 만들고 `test.ps1`·`test-setup.ps1`이 CI의 Windows 러너에서 설치·사용·업그레이드·제거를 시험한다.
  - MSI가 담는 것: 텍스트 서비스 DLL(64비트), 일본어 사전(`dictionary\mozc.bin`과 Mozc 원본 라이선스, 빼고 설치할 수 있는 기능, 약 36MB), 설정 앱과 시작 메뉴 "easime", 터미널 데모, 앱 아이콘, 라이선스. 사전은 CI가 Linux 패키지와 같은 스크립트(`scripts/build-dictionary.sh mozc`)로 만들고, 텍스트 서비스는 DLL 옆 `dictionary` 폴더에서 찾는다(설치 위치를 바꿔도 따라감).
  - 설치 때 DLL을 등록하고 설치한 사용자의 입력 방법에 easime를 켜며(설치 창의 기능 선택이 확인), 제거가 되돌리기다. easime를 쓰는 프로그램은 닫지 않고 사용 중인 파일은 다음 재시작 때 바꾼다.
  - `setup.exe`는 MSI를 담고, 설정 앱이 필요로 하는 Windows App Runtime이 없을 때만 Microsoft의 공식 설치 파일을 받아 먼저 설치한다. easime는 런타임을 재배포하지 않는다.
  - 릴리스에는 올리지 않고 CI 아티팩트(`easime-windows-installer-<commit>`)로만 둔다. 실제 PC 시험은 입력(W1–W14)까지만 했고 설정 앱·`setup.exe`·제거는 아직이다.
  - 동작 상세, CI가 확인하는 것, 한계: [implementation/windows.md](implementation/windows.md)의 "설치 프로그램".
- **macOS**: `.pkg` (서명/공증)
- 버전은 workspace 전체가 하나의 버전을 공유한다. 태그 `vX.Y.Z`를 푸시하면 릴리스 워크플로가 동작한다.

