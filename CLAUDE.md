# easime — Rust로 직접 만드는 다국어 입력기 (한국어 → 일본어 → 중국어)

## 목표
기존 입력기(ibus-hangul, fcitx5, nimf, mozc 등)를 설치하지 않고, **Rust로 직접 만든 입력기**로
KDE Plasma 6(Wayland)에서 입력한다. 가능한 한 **모든 앱**(Wayland 네이티브, Electron/IDE, XWayland, 게임/Wine)을 지원한다.
- 1차: **한국어** (두벌식)
- 단기: **일본어** (로마자 → 가나 먼저, 그다음 한자 변환. Mozc 참고)
- 장기: **중국어** (병음, 간체/번체 모두)

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
| 설정 GUI | **단독 Kirigami 앱**으로 시작, 나중에 KCM으로 옮길 수 있게 |
| 상태 표시 | **트레이 아이콘** + **커서 아래 사각 박스**(`한`/`A`/`あ`/`中`): 전환할 때와 포커스가 들어올 때 약 1초 표시 |
| 학습/사용자 사전 | **일본어 한자 변환 단계에서** 추가, 로컬에만 저장 |
| 이름 | **easime** 확정 |
| 라이선스 | **MIT OR Apache-2.0** (권장안, 아래 참고) |

### 라이선스 메모
- Mozc는 코드가 **BSD-3-Clause**이고, OSS 사전 데이터도 허용적 라이선스(IPAdic 계열 등)이다.
  따라서 MIT/Apache-2.0 프로젝트에서 참고하거나 포함할 수 있다. 저작권 고지만 유지하면 된다.
- Rust 생태계의 표준인 `MIT OR Apache-2.0` 이중 라이선스를 쓴다.
  Qt/Kirigami(LGPL)는 동적 링크이므로 문제없다.
- 제3자 데이터(Mozc 사전, 중국어 사전 등)는 `data/<출처>/`에 원본 LICENSE를 함께 두고,
  `THIRD_PARTY_LICENSES.md`에 기록한다.
- 중국어 사전 후보 중 GPL/CC BY-SA 데이터(예: libpinyin, CC-CEDICT)는 본체와 섞지 말고
  **별도 데이터 패키지**로 분리하거나, 허용적 라이선스 데이터를 우선 검토한다.

## 대상 환경
- OS: Fedora, 데스크톱: **KDE Plasma 6 (6.8.x), Wayland 세션** — 주 플랫폼
- **Wayland 네이티브 앱**: KWin이 실행하는 Wayland input method 프로그램으로 연결
  - 프로토콜: `zwp_input_method_v1` (+ 키보드 grab)
  - 후보창과 커서 아래 상태 박스는 `zwp_input_panel_v1`의 panel surface로 그린다 (KWin이 커서 근처에 배치)
  - 앱 쪽은 KWin이 `text-input-v1/v2/v3`로 중계 → GTK/Qt/Firefox/Chromium(Wayland 모드)
  - KWin 설정: 시스템 설정 → 키보드 → 가상 키보드에서 easime 선택
    (`.desktop` 파일에 `X-KDE-Wayland-VirtualKeyboard=true` 필요)
- **Electron/Chromium 계열**: Wayland 모드 + text-input 플래그(`--enable-wayland-ime` 등)를 켜면 네이티브 경로로 동작.
  설정 GUI에서 안내하거나 `.desktop` 덮어쓰기를 도와주는 기능을 고려한다.
- **XWayland 앱 (게임, Wine, Steam, 일부 IDE)**: Wayland IM 경로가 닿지 않는다.
  → 데몬 안에 **XIM 서버**를 구현해서 대응한다 (`x11rb` 기반, XWayland에 `XMODIFIERS=@im=easime`).
  같은 엔진 인스턴스와 상태를 공유한다.
- ibus/fcitx 같은 IM 프레임워크에 **의존하지 않는다**.

## 아키텍처
Cargo workspace. 언어 엔진은 공통 trait 뒤에 두고, 프론트엔드(Wayland, XIM)도 여러 개를 붙일 수 있게 한다.

- `crates/ime-core` — 공통 타입과 trait (의존성 최소)
  - `trait InputEngine { fn process_key(&mut self, key: KeyEvent) -> EngineOutput; fn reset(&mut self); fn flush(&mut self) -> String; }`
  - `EngineOutput { commit: String, preedit: Preedit, candidates: Option<CandidateList>, handled: bool }`
  - **후보 목록을 처음부터 API에 포함한다** (일본어/중국어 필수, 한국어 한자 변환에도 사용).
  - preedit는 밑줄/하이라이트 구간(segment)과 커서 위치를 표현할 수 있게 한다 (일본어 문절 변환 대비).
  - 언어 순환 목록과 현재 언어 상태 관리(`LanguageSwitcher`)도 여기에 둔다.
- `crates/engine-hangul` — **순수 Rust 한글 조합 엔진** (외부 의존성 없음, libhangul 사용 안 함)
  - 두벌식 표준 자판, 겹모음(ㅘ,ㅙ,ㅚ,ㅝ,ㅞ,ㅟ,ㅢ), 겹받침(ㄳ,ㄵ,ㄶ,ㄺ,ㄻ,ㄼ,ㄽ,ㄾ,ㄿ,ㅀ,ㅄ)
  - 종성 → 다음 음절 초성 이동 (`갑`+`ㅏ` → `가바`, `값`+`ㅏ` → `갑사`)
  - 백스페이스: 자모 단위(기본) / 글자 단위
  - 음절 합성: `0xAC00 + (초성*21 + 중성)*28 + 종성`
- `crates/engine-japanese` — 1단계 로마자 → 히라가나/가타카나 (사전 없음)
  → 2단계 Mozc OSS 사전 기반 한자 변환, 후보창, 학습
- `crates/engine-chinese` — (장기) 병음 → 간체/번체, 사전/빈도 기반 후보
- `crates/easime` (데몬 바이너리)
  - `frontend/wayland`: `wayland-client` + `wayland-protocols`, `xkbcommon`으로 keysym 변환
  - `frontend/xim`: XWayland 앱용 XIM 서버 (단계적으로)
  - 처리하지 않는 키는 앱으로 전달(forward)
  - 후보창과 커서 아래 상태 박스 렌더링 (panel surface, Breeze 색상과 글꼴 따르기)
  - 트레이 아이콘 (StatusNotifierItem, D-Bus): 현재 언어 표시, 클릭으로 전환, 메뉴에서 설정 열기
  - D-Bus 인터페이스: 설정 다시 읽기, 현재 상태 조회 (설정 앱과 트레이에서 사용)
- `crates/easime-settings` — 설정 GUI (아래)
- `data/` — `.desktop` 파일(KWin 가상 키보드 등록용, 설정 앱 실행용), 아이콘, 제3자 사전 데이터와 그 LICENSE

## 설정 GUI (KDE Plasma 6 우선)
사용자의 주 환경이 Plasma 6이므로 **Plasma 시스템 설정과 똑같이 보이고 동작해야 한다.**
- 기술 스택: **Rust + cxx-qt (Qt 6) + QML + Kirigami**
  - Breeze 테마, 다크/라이트 모드, 강조색, 글꼴, HiDPI 배율을 시스템 설정 그대로 따른다.
  - egui/iced처럼 Plasma와 이질적으로 보이는 툴킷은 쓰지 않는다.
- UI 구성: Kirigami `ApplicationWindow` + 왼쪽 사이드바 페이지 (시스템 설정 모듈 스타일)
  - **일반**: 자동 시작, 트레이 아이콘, 커서 아래 상태 박스 켜기/끄기
  - **언어**: 순환에 넣을 언어 선택과 순서 (영어/한국어/일본어/중국어)
  - **단축키**: 전환 키 선택(한/영 키 / 오른쪽 Alt)
  - **한국어**: 자판(두벌식), 백스페이스 단위, (나중에) 한자 변환 키
  - **일본어**: 로마자 표, 가나 종류, (나중에) 사전/학습 관리
  - **중국어**: 간체/번체 선택 (엔진이 생기면 추가)
  - **모양**: 후보창 글꼴/크기, 가로/세로 배치, 상태 박스 표시 시간
  - **앱 호환성**: Electron/Chromium 앱의 Wayland IME 플래그 안내, XIM 상태 확인
- 설정 저장: `~/.config/easime/config.toml`. 데몬은 D-Bus 신호 또는 파일 감시로 재시작 없이 다시 읽는다.

## 개발 단계
1. **ime-core + 한글 엔진** — 단위 테스트를 먼저, 충분히 작성. Wayland 없이 검증 가능.
2. **CLI 데모** — 터미널에서 엔진을 수동으로 확인.
3. **Wayland IM 프론트엔드** — KWin 연결, preedit/commit, 키 전달, 한/영 전환.
4. **상태 표시** — 트레이 아이콘 + 커서 아래 상태 박스 (panel surface 기초 다지기).
5. **설정 GUI + KDE 통합** — Kirigami 앱, `.desktop`, 자동 실행, D-Bus로 설정 다시 읽기.
6. **XIM 프론트엔드** — XWayland 앱(게임/Wine/Steam/IDE) 지원.
7. **일본어 1단계** — 로마자 → 가나, 언어 순환에 추가.
8. **후보창** — panel surface (한국어 한자 변환으로 시험) → **일본어 2단계** (Mozc 사전, 학습).
9. **중국어** (장기) — 병음 엔진, 간체/번체, 사전/빈도.

## 개발 규칙
- **사용자와의 대화는 항상 한국어로 한다** (전역 규칙). 코드 식별자, 주석, 커밋 메시지는 영어.
- 엔진 crate에는 Wayland/X11/Qt 의존성을 넣지 않는다. UI와 언어 로직을 분리한다.
- 새 언어 엔진은 `InputEngine` trait만 구현하면 데몬과 설정 앱에 붙을 수 있어야 한다.
- 사전 데이터를 추가할 때는 라이선스를 확인하고 `THIRD_PARTY_LICENSES.md`에 출처를 기록한다.
- 변경 후 `cargo fmt`, `cargo clippy --all-targets`, `cargo test` 통과를 확인한다.

## 명령어
```sh
cargo build --release
cargo test -p engine-hangul
cargo run -p easime            # 수동 실행은 디버깅 용도 (평소에는 KWin이 실행)
cargo run -p easime-settings
```

## 참고
- Rust 툴체인: `rustup` stable (cargo 1.99.1 확인)
- Git: 기본 브랜치 `main`, 원격은 GitHub 공개 저장소 `byeonggonlee/easime` (`gh` 로그인됨)
- 빌드 의존성(Fedora): `libxkbcommon-devel`, `wayland-devel`, `libxcb-devel`,
  설정 GUI용 `qt6-qtbase-devel`, `qt6-qtdeclarative-devel`, `kf6-kirigami-devel`
- 유니코드: 한글 음절 U+AC00–U+D7A3, 호환 자모 U+3131–U+318E, 히라가나 U+3040–U+309F, 가타카나 U+30A0–U+30FF
- Mozc: https://github.com/google/mozc (BSD-3-Clause, OSS 사전은 `src/data/dictionary_oss`)
