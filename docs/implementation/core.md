# 공통 코어와 엔진 구현 현황

기준: 2026-11-09, `main` 86648c2. 모든 OS가 같은 코드로 쓰는 부분이다. 한눈에 보는 현황은 [status.md](../status.md), 코어와 어댑터를 나누는 기준은 [platforms.md](../platforms.md), OS별 구현은 [linux-kde.md](linux-kde.md)와 [windows.md](windows.md)다.

코어 크레이트는 OS·UI 의존성이 없고, 모든 OS의 CI(Linux 전체, Windows·macOS는 포터블 크레이트)에서 `fmt`·`clippy -D warnings`·`test`를 통과한다. 이 시험은 코어가 맞다는 증거이지, 실제 데스크톱에서 입력이 된다는 증거가 아니다.

## 크레이트별 상태

| 크레이트 | 하는 일 | 상태 |
|---|---|---|
| `ime-core` | 키 이벤트(`LanguageSwitcher`의 `toggle`/`advance`, `SwitchMode`), `InputEngine`(`wants_key` 포함), `EngineOutput`, `CandidateList`(9개 쪽, 번호 1–9), `Language`/`LanguageSwitcher`, `Os` | 완료. `ime_core::contract`가 모든 엔진이 같은 약속을 지키는지 시험 |
| `ime-session` | `Session`: 엔진 구성, 언어 전환(전환 키 한 번 `switch_language`: 영어 ↔ 마지막 언어 또는 순환, Shift+전환 키 `next_language`: 순환의 다음 언어), 전환 때 조합 확정, 설정 적용, 후보를 못 띄울 때 취소. `ToggleTracker`(한/영 키·오른쪽 Alt 탭 판정, Shift를 누른 채면 `Decision::Next`), `JapaneseDictionary`(한 번만 열어 공유), `language_glyph` | 완료 (2026-11-07 Wayland 쪽에서 옮겨 옴) |
| `engine-hangul` | 두벌식, 겹모음·겹받침, 자모/글자 단위 백스페이스, 조합 중 한 글자 한자 변환 | 완료. 한자 변환은 실제 KDE 재검증 필요 |
| `engine-japanese` | 로마자 → 히라가나/가타카나, Space로 단어 단위 사전 변환, 사전 로더 | 완료. 실제 데스크톱에서는 시험하지 않음 |
| `easime-dict-tool` | Mozc OSS 사전 → easime 이진 사전 변환기 | 완료 |
| `easime-config` | TOML 설정, OS별 경로, 원자적 저장, 시스템 언어 읽기, 언어 기본값 | 완료 |
| `easime-settings-model` | 설정 앱이 보여 주는 값과 변경을 저장하는 규칙, 일본어 사전 상태. UI 툴킷 없음 | 완료 (2026-11-09 `settings/kde`에서 분리, KDE와 Windows 설정 앱이 공유) |
| `easime-cli` | 터미널 데모 | 완료 |
| `engine-chinese` | 병음 → 간체/번체 | **없음** ([architecture.md](../architecture.md)) |

## 한글 엔진 (`engine-hangul`)

- 두벌식 표준 자판, 겹모음(ㅘ ㅙ ㅚ ㅝ ㅞ ㅟ ㅢ), 겹받침, 종성의 다음 음절 초성 이동(`갑`+`ㅏ` → `가바`, `값`+`ㅏ` → `갑사`).
- 백스페이스는 자모 단위가 기본(`한` → `하` → `ㅎ`)이고 설정에서 글자 단위로 바꿀 수 있다.
- **한자 변환(조합 중 한 글자)**: 한자 키 → ↑↓/Space 이동, PgUp/PgDn 쪽 이동, Enter 또는 현재 쪽의 1–9로 확정, Esc/Backspace는 한글 조합으로 복귀. 후보가 없거나 후보창을 띄울 수 없으면 글자를 잃지 않는다. 한자 데이터는 `data/hanja/`(libhangul 데이터의 한 글자 항목, 원본 라이선스 보존). 단어·선택 영역·확정한 글자의 재변환과 한자 키가 없는 키보드의 대체 키는 후속이다.

## 일본어 엔진 (`engine-japanese`)

- 로마자 → 히라가나/가타카나. Space로 변환을 시작하고, 후보는 Space/↑↓로 순환, Enter로 확정, Esc/Backspace로 가나로 복귀, 숫자로 선택한다. 변환은 **단어 단위**이며 문절 분할과 문맥 변환, 학습은 없다.
- **사전**: `Dictionary` trait, `MemoryDictionary`(TSV), `FileDictionary`(이진, 로드 약 0.1초, Windows는 읽기 전용 메모리 매핑), `LayeredDictionary`, `load_default()`. 사전 폴더는 OS별이다(`loader.rs`, `ime-core::Os`): Linux는 `~/.local/share/easime/dictionary`와 `/usr/share/easime/dictionary`, Windows는 `%APPDATA%\easime\dictionary`와 DLL 옆 `dictionary`. 사전이 없으면 가나(히라가나·가타카나)만 후보로 나온다.
- **데이터**: Mozc OSS 사전(BSD-3, IPAdic·오키나와 사전 포함)을 `scripts/fetch-mozc-dictionary.sh`(고정 커밋, SHA-256 검증)로 받아 `scripts/build-dictionary.sh sample|mozc`로 이진 사전을 만든다. 저장소에는 데이터가 없고 원본 라이선스만 `data/mozc/`와 [THIRD_PARTY_LICENSES.md](../../THIRD_PARTY_LICENSES.md)에 있다. 패키지와 CI는 실제 Mozc 사전을 쓰고 `おおさか → 大阪` 조회를 확인한다.
- 후보창의 키 안내(PgUp/PgDn, 1–9, Enter, Esc)가 한국어와 같도록 쪽 이동과 쪽 기준 숫자 선택을 같게 맞췄다. 마지막 쪽에서 범위를 벗어난 숫자는 확정하지 않고 무시한다.
- 사전 상태(있음·찾지 못함·읽지 못함)는 설정 앱의 일반 화면에 있다. 입력 중에 경고창을 띄우지는 않는다.

## 설정과 언어 기본값 (`easime-config`)

- **입력 언어 기본값**(2026-11-07 사용자 결정): 설정 파일에 `languages.enabled`가 없으면 **영어 + 시스템 언어**다. 한국어 시스템이면 한국어, 일본어 시스템이면 일본어만 켜고, 그 밖의 시스템(영어, 프랑스어, 중국어 등)은 모두 꺼 영어만 입력된다. 한 번 고르면(`enabled = [...]`, 빈 목록 포함) 그 선택이 우선한다.
- 시스템 언어는 Linux에서 `LANGUAGE`(C/POSIX 로케일이면 무시), `LC_ALL`, `LC_MESSAGES`, `LANG` 순으로, Windows에서 `GetUserPreferredUILanguages`로 읽는다(`locale.rs`, macOS는 아직 없음). 데몬은 시작할 때와 설정을 다시 읽을 때 어느 쪽인지 로그에 남긴다.
- **중국어**는 간체(`chinese_simplified`)와 번체(`chinese_traditional`)가 별개의 입력 언어다(2026-11-07 사용자 결정). 로케일 대응: `zh_TW`/`zh_HK`/`zh_MO`/`Hant` → 번체, `zh_CN`/`zh_SG`/`Hans`/`zh` → 간체. 엔진이 없어 기본으로 켜지지 않고 설정 앱에서도 켤 수 없다("준비 중").
- 설정 파일 위치: Linux `~/.config/easime/config.toml`, Windows `%APPDATA%\easime\config.toml`, macOS `~/Library/Application Support/easime/config.toml`. 저장은 임시 파일 후 교체(원자적)이고, 읽을 수 없는 파일은 덮어쓰지 않는다.
- 설정 변경 감지는 OS별이다: Linux는 inotify(즉시), Windows는 `ConfigReloader`가 키·포커스 같은 일이 있을 때 파일 시각을 본다([platforms.md](../platforms.md)).

## 언어 전환 (2026-11-09 사용자 결정)

- `ime_core::LanguageSwitcher`가 순환(영어, 켠 언어들)과 "마지막 언어"를 들고 있다. `SwitchMode::ToggleLast`(기본)에서 전환 키 한 번은 영어와 마지막으로 쓴 언어 사이를 오가고, `SwitchMode::Cycle`에서는 순환의 다음 언어로 간다. `advance`(Shift를 누른 채 전환 키)는 어느 모드에서나 순환의 다음 언어다. 언어를 고르거나(`select`) 순환으로 가면 그 언어가 마지막 언어가 된다. 영어는 마지막 언어가 아니다.
- 설정은 `[languages] switch = "toggle_last" | "cycle"`(`easime-config`, 기본은 파일에 쓰지 않는다). 설정이 바뀌면 현재 언어와 마지막 언어를 지킨다.
- **마지막 언어는 기억하지 않는다**: 새로 만든 전환기는 켠 언어 중 첫 번째에서 시작하고 그것이 마지막 언어다(Windows는 프로그램마다 따로이므로 프로그램을 새로 열면 첫 언어로 돌아간다).
- 켠 언어가 영어와 하나뿐이면 두 모드가 같다.
- 전환 키 판정(`toggle.rs`)은 Shift 여부를 프런트엔드에서 받는다. 오른쪽 Alt 탭은 Shift와 함께 눌러도 전환으로 센다(그때는 `Next`). Ctrl이나 Win, 다른 Alt가 함께면 전환 키가 아니다.

## 아직 없는 것

- 중국어 엔진(병음, 간체·번체 출력).
- 일본어 문절·문맥 변환, 학습·사용자 사전(일본어 한자 변환 단계에서 추가하기로 했다, [decisions.md](../decisions.md)).
- 한글 세벌식(엔진은 나중에 붙일 수 있게 설계).
- 상태 표시 글자의 구분(`あ`·`中` 하나에 가나 종류·간체/번체를 겹쳐 넣는 규칙은 정하지 않았다, [DESIGN.md](../../DESIGN.md) 3.5).
