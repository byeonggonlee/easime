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
설정 창은 실제 설정 파일(`~/.config/easime/config.toml`)을 바로 고친다. 일반 페이지의 "입력기로 사용하기"는 KDE의 `~/.config/kwinrc`(`[Wayland] InputMethod`)를 바꾼다(확인을 받고, 되돌릴 수 있다). 시험할 때는 `XDG_CONFIG_HOME`을 임시 폴더로 바꿔 두는 편이 안전하다(`XDG_CURRENT_DESKTOP=KDE`로 KDE 세션인 척할 수 있다).
Kirigami가 없는 환경(예: Ubuntu)에서는 빌드와 단위 테스트까지만 되고 화면은 열리지 않는다. 그 경우 화면 검증은 CI의 `settings` 잡(Fedora 컨테이너)이 맡는다.

### 패키지 만들기 (배포판 컨테이너에서)
패키지는 배포판당 하나이고(데몬, 설정 창, 사전), Qt와 Kirigami가 있는 환경에서 만든다. CI는 `.github/workflows/packages.yml`에서 Fedora(`.rpm`)와 Debian 13(`.deb`) 컨테이너로 만든다. 같은 순서를 손으로 하면:
```sh
sh scripts/build-dictionary.sh mozc                                   # 실제 Mozc 사전(내려받기, 체크섬 확인). sample은 시험용이라 패키지에 넣지 않는다
cargo build --release --locked -p easime
cargo build --release --locked --manifest-path settings/kde/Cargo.toml
cargo generate-rpm -p crates/easime     # Fedora: target/generate-rpm/easime-*.rpm  (cargo install cargo-generate-rpm)
cargo deb -p easime --no-build          # Debian: target/debian/easime_*.deb        (cargo install cargo-deb)
```
`.desktop` 파일은 `data/easime.desktop`(가상 키보드 등록)과 `data/easime-settings.desktop`(앱 메뉴)이다. 배포판이 요구하는 패키지 이름은 `crates/easime/Cargo.toml`의 `depends`/`requires`에 있다.

### Windows 설치 프로그램 만들기와 시험
Windows에서만 된다. WiX 5가 필요하다: `dotnet tool install --global wix --version 5.0.2`.
```powershell
pwsh packaging\windows\build.ps1                       # target\easime-<버전>-x64.msi
# 시험은 컴퓨터를 바꾼다(설치·제거). CI 러너나 가상 머신에서만 실행한다.
pwsh packaging\windows\build.ps1 -Version 0.0.1 -OutDir target\upgrade-test
pwsh packaging\windows\test.ps1 -Msi target\easime-<버전>-x64.msi -OldMsi target\upgrade-test\easime-0.0.1-x64.msi
```
지금의 MSI는 터미널 데모와 라이선스만 설치한다. 입력기(TSF)는 아직 없다.

### 설정 창의 표시 언어와 번역
표시 언어는 시스템 언어를 따르고, 시험할 때는 `--ui-language=ko|en|ja|zh-Hant|zh-Hans`(또는 `LANGUAGE=ja`)로 바꾼다.
```sh
cargo run --manifest-path settings/kde/Cargo.toml -- --ui-language=ja
python3 settings/kde/tests/check_translations.py          # 번역표와 QML 문구 대조 (Qt 필요 없음)
QT_QPA_PLATFORM=offscreen /usr/lib64/qt6/bin/qmltestrunner -input settings/kde/tests/tst_tr.qml   # 언어 선택 로직
```
화면의 문구는 `Tr.t("한국어 원문")`으로 쓰고, 같은 원문을 `settings/kde/qml/tr.js`의 표에 네 언어로 넣는다(원문을 고치면 표의 키도 고친다). 한국어 외의 번역은 원어민이 검토하기 전까지 초안이다.
