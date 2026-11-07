# easime 개발 환경과 KWin 시험

## 개발 모드로 KWin에서 시험하기 (사용자 환경을 바꾸므로 사용자가 직접 실행)
```sh
./scripts/install-dev.sh     # release 빌드 + ~/.local/share/applications/easime.desktop 등록
```
1. 시스템 설정 → 키보드 → 가상 키보드 → easime 선택 → 로그아웃/로그인
2. 로그: `~/.local/state/easime/easime.log`
3. 문제가 생겨 입력이 안 되면 같은 화면에서 원래 항목(Plasma Keyboard 등)을 고르거나
   `kwriteconfig6 --file kwinrc --group Wayland --key InputMethod ""` 후 재로그인
- 현재 `kwinrc [Wayland] InputMethod` 값: `/usr/share/applications/org.kde.plasma.keyboard.desktop` (복구용으로 기억할 것)

## 참고
- Rust 툴체인: `rustup` stable (cargo 1.99.1 확인)
- Git: 기본 브랜치 `main`, 원격 `origin` = GitHub 공개 저장소 `byeonggonlee/easime` (`gh` 로그인됨)
- Linux 빌드 의존성(Fedora): `libxkbcommon-devel`, `wayland-devel`, `libxcb-devel`, `dbus-devel`,
  설정 GUI용 `qt6-qtbase-devel`, `qt6-qtdeclarative-devel`, `kf6-kirigami-devel`
- 유니코드: 한글 음절 U+AC00–U+D7A3, 호환 자모 U+3131–U+318E, 히라가나 U+3040–U+309F, 가타카나 U+30A0–U+30FF
- Mozc: https://github.com/google/mozc (BSD-3-Clause, OSS 사전은 `src/data/dictionary_oss`)

## 설정 창 (settings/kde)
메인 워크스페이스와 별개의 cargo 프로젝트다. Fedora 의존성: `qt6-qtbase-devel`, `qt6-qtdeclarative-devel`, `kf6-kirigami`, `kf6-kirigami-devel`, `kf6-qqc2-desktop-style`.
```sh
cargo run   --manifest-path settings/kde/Cargo.toml                      # 설정 창 열기
cargo test  --manifest-path settings/kde/Cargo.toml                      # 모델·표시값 단위 테스트 (화면 없음)
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  settings/kde/target/debug/easime-settings --screenshots=/tmp/shots     # 모든 페이지를 열어 PNG로 저장하고 종료
```
설정 창은 실제 설정 파일(`~/.config/easime/config.toml`)을 바로 고친다. 시험할 때는 `XDG_CONFIG_HOME`을 임시 폴더로 바꿔 두는 편이 안전하다.
Kirigami가 없는 환경(예: Ubuntu)에서는 빌드와 단위 테스트까지만 되고 화면은 열리지 않는다. 그 경우 화면 검증은 CI의 `settings` 잡(Fedora 컨테이너)이 맡는다.
