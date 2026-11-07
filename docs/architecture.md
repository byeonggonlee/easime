# easime 아키텍처와 플랫폼별 연결 방식

플랫폼 프로토콜, crate 구성, 핵심 trait, 설정 GUI, 패키징을 다룬다.

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
  engine-japanese/   로마자→가나, 사전 기반 변환(Mozc OSS 사전), 사전 로더
  easime-dict-tool/  Mozc 사전 → easime 이진 사전 변환 도구 (OS 독립)
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

