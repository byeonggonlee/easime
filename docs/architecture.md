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
- 설치: **MSI(WiX 5)** + TSF 등록(`regsvr32` 방식). 코드 서명은 선택 사항. 설치 프로그램의 틀은 이미 있다(`packaging/windows`, 아래 "패키징과 배포"). TSF DLL이 아직 없어서 지금의 MSI는 입력기를 설치하지 않는다.

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
  패키지는 둘로 나뉜다. 설정 창이 Qt 6와 Kirigami를 요구하므로, 데몬을 그 의존성 없이 설치할 수 있게 하기 위해서다(엔지니어링 결정).
  - `easime`: 데몬, `easime.desktop`(가상 키보드 등록용, 메뉴에는 안 보임), 사전. 의존성: libxkbcommon 등. `easime-settings`를 Recommends한다.
  - `easime-settings`(`settings/kde`의 메타데이터): 설정 창과 `easime-settings.desktop`(앱 메뉴의 "easime 설정", 이름은 한·일·번체·간체로 현지화). `easime`와 Qt 6·Kirigami·qt6-wayland를 요구한다. `.rpm`은 Fedora 컨테이너에서, `.deb`는 Debian 13(trixie) 컨테이너에서 빌드한다(그 배포판의 Qt·Kirigami로 만들어야 하므로). 그러므로 `.deb`는 Debian 13 이상, Ubuntu는 Plasma 6와 Qt가 충분히 새로운 25.04 이상을 대상으로 한다. 시험한 것은 clean `debian:trixie`와 `fedora:latest` 컨테이너뿐이다.
  - 빌드와 검증은 `.github/workflows/packages.yml`(재사용 워크플로) 하나다. CI는 샘플 사전으로, 릴리스(`release.yml`)는 실제 Mozc 사전으로 같은 흐름을 부른다. 패키지마다 clean 컨테이너(Fedora, Debian)에 배포판의 의존성만으로 설치하고, 설치된 설정 창을 오프스크린으로 띄워 모든 페이지가 열리는지 확인한 뒤에야 네 파일이 한 아티팩트(`easime-linux-packages-<commit>`)로 묶인다.
- **Windows**: **MSI(WiX 5)**, `packaging/windows/easime.wxs`. `build.ps1`이 만들고 `test.ps1`이 CI의 Windows 러너에 실제로 설치·사용·업그레이드·제거를 시험한다.
  **지금은 터미널 데모(`easime-cli.exe`)와 라이선스만 담는다. Windows 입력기(TSF)가 아직 없으므로 입력기를 설치하지 않는다.**
  그래서 릴리스에는 올리지 않고 CI 아티팩트로만 둔다. TSF DLL이 생기면 메인 기능에 구성요소를 하나 더하고 설치 때 등록, 제거 때 등록 해제를 붙인다.
  시스템 PATH에 폴더를 더하는 기능은 기본이 꺼져 있다(`ADDLOCAL=ALL`로 켠다): 시스템 설정을 바꾸는 일이기 때문이다.
- **macOS**: `.pkg` (서명/공증)
- 버전은 workspace 전체가 하나의 버전을 공유한다. 태그 `vX.Y.Z`를 푸시하면 릴리스 워크플로가 동작한다.

