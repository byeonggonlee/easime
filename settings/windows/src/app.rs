//! The window: a navigation pane with the pages of the KDE window (General,
//! Languages, Shortcuts, Korean, Appearance) in WinUI 3, the way Windows' own
//! Settings app looks. Every change is saved at once through the shared model
//! (`easime-settings-model`), which rewrites only the keys that change and
//! never overwrites a file it cannot read. The text service reads the file
//! again on its own (`ConfigReloader`), so nothing has to restart.
//!
//! There is no page for switching easime on: the installer does that, and
//! uninstalling undoes it (docs/decisions.md, the row on installing and
//! switching on).
//! There is no Japanese page until Japanese has options (DESIGN.md 5.5).

use crate::install;
use crate::tr::{self, Words};
use easime_config::{BackspaceUnit, ToggleKey};
use easime_settings_model::dictionary::{self, DictionaryState};
use easime_settings_model::model::{Failure, FailureKind, Model, SELECTABLE};
use ime_core::Language;
use ime_core::SwitchMode;
use std::path::PathBuf;
use windows_reactor::*;

pub fn run() {
    if let Err(error) = App::run_component::<Settings>(()) {
        eprintln!("easime settings: {error}");
        std::process::exit(1);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Page {
    General,
    Languages,
    Shortcuts,
    Korean,
    Appearance,
}

impl Page {
    const ALL: [Page; 5] = [
        Page::General,
        Page::Languages,
        Page::Shortcuts,
        Page::Korean,
        Page::Appearance,
    ];

    fn tag(self) -> &'static str {
        match self {
            Page::General => "general",
            Page::Languages => "languages",
            Page::Shortcuts => "shortcuts",
            Page::Korean => "korean",
            Page::Appearance => "appearance",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Page::General => "일반",
            Page::Languages => "언어",
            Page::Shortcuts => "단축키",
            Page::Korean => "한국어",
            Page::Appearance => "모양",
        }
    }

    fn from_tag(tag: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|page| page.tag() == tag)
    }
}

/// The choices of the Shortcuts page, in the order shown.
const TOGGLE_KEYS: [(ToggleKey, &str); 3] = [
    (ToggleKey::Hangul, "한/영 키"),
    (ToggleKey::RightAlt, "오른쪽 Alt"),
    (ToggleKey::Both, "둘 다 (현재 기본값)"),
];

/// What the plain switch key does, in the order shown (Shortcuts page).
const SWITCH_MODES: [(SwitchMode, &str); 2] = [
    (
        SwitchMode::ToggleLast,
        "영어와 마지막 언어 사이를 오가기 (기본값)",
    ),
    (SwitchMode::Cycle, "순서대로 순환"),
];

/// The choices of the Korean page.
const BACKSPACE: [(BackspaceUnit, &str); 2] = [
    (BackspaceUnit::Jamo, "자모 단위 (한 → 하 → ㅎ)"),
    (BackspaceUnit::Syllable, "글자 단위 (한 → 지움)"),
];

/// What a language is called in the list: in its own script, like the KDE window.
fn language_name(words: &Words, language: Language) -> String {
    match language {
        Language::Korean => "한국어".to_owned(),
        Language::Japanese => "日本語".to_owned(),
        Language::English => words.t("영어"),
        Language::ChineseSimplified => "简体中文".to_owned(),
        Language::ChineseTraditional => "繁體中文".to_owned(),
    }
}

/// The Japanese dictionary folders the text service reads ([`install`]).
fn dictionary_dirs() -> Vec<PathBuf> {
    install::dictionary_dirs(
        engine_japanese::loader::user_dictionary_dir(),
        std::env::current_exe().ok().as_deref(),
    )
}

pub enum Message {
    Show(Option<String>),
    Language(Language, bool),
    Move(Language, bool),
    ToggleKey(Option<usize>),
    SwitchMode(Option<usize>),
    Backspace(Option<usize>),
    IndicatorBox(bool),
    IndicatorSeconds(Option<f64>),
    Reload,
    AskReset,
    Reset(ContentDialogResult),
    Dismiss,
}

/// The line at the top of the page that says what the last change did.
#[derive(Clone)]
struct Notice {
    text: String,
    severity: InfoBarSeverity,
}

pub struct Settings {
    model: Model,
    words: Words,
    page: Page,
    notice: Option<Notice>,
    dictionary: DictionaryState,
    confirming_reset: bool,
}

impl Settings {
    fn problem(words: &Words, failure: &Failure) -> Notice {
        let text = match failure.kind {
            FailureKind::NoHome => words.t("홈 폴더를 찾지 못해 설정을 저장할 수 없습니다."),
            _ => words.t(
                "설정 파일을 읽지 못해 기본값을 보여 드리고 있습니다. 파일을 직접 고친 뒤 '다시 읽기'를 누르거나, '일반'에서 '기본값으로 되돌리기'를 눌러 새로 시작하세요. 읽을 수 없는 동안에는 파일을 지우지 않도록 변경을 저장하지 않습니다.",
            ),
        };
        Notice {
            text: format!("{text}\n{}", failure.detail),
            severity: InfoBarSeverity::Warning,
        }
    }

    fn failed(&self, failure: &Failure) -> Notice {
        let w = &self.words;
        let text = match failure.kind {
            FailureKind::Unreadable => w.t("설정 파일을 읽을 수 없어 저장하지 않았습니다."),
            FailureKind::NoHome => w.t("홈 폴더를 찾지 못해 저장하지 못했습니다."),
            FailureKind::WriteFailed => format!(
                "{}\n{}",
                w.t("저장하지 못했습니다. 변경하기 전 값을 그대로 보여 드립니다."),
                failure.detail
            ),
            FailureKind::Invalid | FailureKind::ActivationChanged => {
                w.t("알 수 없는 값이라 적용하지 않았습니다.")
            }
        };
        Notice {
            text,
            severity: InfoBarSeverity::Error,
        }
    }

    fn done(&self, source: &str) -> Notice {
        Notice {
            text: self.words.t(source),
            severity: InfoBarSeverity::Success,
        }
    }
}

impl Component for Settings {
    type Input = ();
    type Message = Message;

    fn create(_input: &(), _context: &ComponentContext<Self>) -> Self {
        let requested = tr::requested(std::env::args());
        let words = Words::new(tr::detect(
            requested.as_deref(),
            &easime_config::system_language_tags(),
        ));
        let model = Model::open(easime_config::config_path());
        let notice = model
            .problem()
            .map(|failure| Settings::problem(&words, failure));
        Settings {
            model,
            words,
            page: Page::General,
            notice,
            dictionary: dictionary::inspect(&dictionary_dirs()),
            confirming_reset: false,
        }
    }

    fn update(&mut self, message: Message, _context: &ComponentContext<Self>) {
        let result = match message {
            Message::Show(tag) => {
                if let Some(page) = tag.as_deref().and_then(Page::from_tag) {
                    self.page = page;
                }
                return;
            }
            Message::Language(language, on) => self.model.set_language_enabled(language, on),
            Message::Move(language, up) => self.model.move_language(language, up),
            Message::ToggleKey(index) => {
                let Some(&(key, _)) = index.and_then(|i| TOGGLE_KEYS.get(i)) else {
                    return;
                };
                if key == self.model.config().shortcuts.toggle_key {
                    return;
                }
                self.model.set_toggle_key(key)
            }
            Message::SwitchMode(index) => {
                let Some(&(mode, _)) = index.and_then(|i| SWITCH_MODES.get(i)) else {
                    return;
                };
                if mode == self.model.config().languages.switch {
                    return;
                }
                self.model.set_switch_mode(mode)
            }
            Message::Backspace(index) => {
                let Some(&(unit, _)) = index.and_then(|i| BACKSPACE.get(i)) else {
                    return;
                };
                if unit == self.model.config().korean.backspace {
                    return;
                }
                self.model.set_backspace(unit)
            }
            Message::IndicatorBox(on) => {
                if on == self.model.config().appearance.indicator_box {
                    return;
                }
                self.model.set_indicator_box(on)
            }
            Message::IndicatorSeconds(seconds) => {
                let Some(seconds) = seconds.filter(|s| s.is_finite()) else {
                    return;
                };
                let milliseconds = (seconds.clamp(0.1, 10.0) * 1000.0).round() as u32;
                if milliseconds == self.model.config().appearance.indicator_ms {
                    return;
                }
                self.model.set_indicator_ms(milliseconds)
            }
            Message::Reload => {
                self.model.reload();
                self.dictionary = dictionary::inspect(&dictionary_dirs());
                self.notice = Some(match self.model.problem() {
                    Some(failure) => Settings::problem(&self.words, failure),
                    None => self.done("설정 파일을 다시 읽었습니다."),
                });
                return;
            }
            Message::AskReset => {
                self.confirming_reset = true;
                return;
            }
            Message::Reset(answer) => {
                self.confirming_reset = false;
                if answer != ContentDialogResult::Primary {
                    return;
                }
                self.notice = Some(match self.model.reset_to_defaults() {
                    Ok(Some(backup)) => Notice {
                        text: self.words.t1(
                            "기본값으로 되돌렸습니다. 이전 파일은 %1에 보관했습니다.",
                            &backup.display().to_string(),
                        ),
                        severity: InfoBarSeverity::Success,
                    },
                    Ok(None) => self.done("기본값으로 되돌렸습니다."),
                    Err(failure) => self.failed(&failure),
                });
                return;
            }
            Message::Dismiss => {
                self.notice = None;
                return;
            }
        };
        self.notice = Some(match result {
            Ok(()) => self.done("저장했습니다. 실행 중인 easime에 곧 반영됩니다."),
            Err(failure) => self.failed(&failure),
        });
    }

    fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
        let w = &self.words;
        context.window_title(w.t("easime 설정"));
        context.window_visuals(
            WindowVisuals::new()
                .backdrop(WindowBackdrop::Mica)
                .client_size(900.0, 640.0),
        );
        let items: Vec<(&str, View)> = Page::ALL
            .into_iter()
            .map(|page| {
                let item = NavigationViewItem::new()
                    .tag(page.tag())
                    .is_selected(page == self.page)
                    .slot(NavigationViewItemSlot::Content, w.t(page.title()));
                (page.tag(), item)
            })
            .collect();
        NavigationView::new()
            .pane_display_mode(NavigationViewPaneDisplayMode::Left)
            // The pane is always open, so the menu (hamburger) button has
            // nothing to do; on a real PC pressing it broke the window
            // (2026-11-10), so it is not shown.
            .is_pane_toggle_button_visible(false)
            .is_settings_visible(false)
            .is_back_button_visible(NavigationViewBackButtonVisible::Collapsed)
            .open_pane_length(220.0)
            .on_selected_tag_changed(context.callback(Message::Show))
            .slots([
                SlotView::collection(NavigationViewSlot::MenuItems, items),
                SlotView::new(NavigationViewSlot::Content, self.page_view(context)),
            ])
    }
}

/// Views side by side, keyed by position.
fn row(views: Vec<View>) -> View {
    StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(8.0)
        .keyed_children(views.into_iter().enumerate())
}

fn heading(text: String) -> View {
    TextBlock::new()
        .text(text)
        .font_size(28.0)
        .font_weight(FontWeight::SEMI_BOLD)
        .into()
}

fn label(text: String) -> View {
    TextBlock::new()
        .text(text)
        .font_weight(FontWeight::SEMI_BOLD)
        .into()
}

fn paragraph(text: String) -> View {
    TextBlock::new()
        .text(text)
        .text_wrapping(TextWrapping::WrapWholeWords)
        .is_text_selection_enabled(true)
        .into()
}

impl Settings {
    fn page_view(&self, context: &mut ViewContext<Self>) -> View {
        let w = &self.words;
        let mut views = vec![heading(w.t(self.page.title()))];
        if let Some(notice) = &self.notice {
            views.push(
                InfoBar::new()
                    .message(notice.text.clone())
                    .severity(notice.severity)
                    .is_open(true)
                    .is_closable(true)
                    .on_closed(context.callback(|()| Message::Dismiss))
                    .into(),
            );
        }
        views.extend(match self.page {
            Page::General => self.general(context),
            Page::Languages => self.languages(context),
            Page::Shortcuts => self.shortcuts(context),
            Page::Korean => self.korean(context),
            Page::Appearance => self.appearance(context),
        });
        ScrollViewer::new()
            .vertical_scroll_bar_visibility(ScrollBarVisibility::Auto)
            .content(
                StackPanel::new()
                    .spacing(16.0)
                    .margin(32.0)
                    .max_width(760.0)
                    .horizontal_alignment(HorizontalAlignment::Left)
                    .keyed_children(views.into_iter().enumerate()),
            )
    }

    fn general(&self, context: &mut ViewContext<Self>) -> Vec<View> {
        let w = &self.words;
        let mut views = Vec::new();
        if self.model.languages().is_empty() {
            views.push(
                InfoBar::new()
                    .message(w.t(
                        "지금은 영어로만 입력됩니다. 한국어나 日本語로도 입력하려면 언어 화면에서 켜 주세요.",
                    ))
                    .severity(InfoBarSeverity::Informational)
                    .is_open(true)
                    .is_closable(false)
                    .into(),
            );
            views.push(
                Button::new()
                    .on_click(context.callback(|()| Message::Show(Some("languages".into()))))
                    .content(w.t("언어 화면 열기")),
            );
        }
        views.push(paragraph(w.t(
            "바꾼 내용은 바로 설정 파일에 저장되고, 실행 중인 easime가 곧 다시 읽습니다. 다시 시작할 필요는 없습니다.",
        )));
        if w.is_draft() {
            views.push(paragraph(w.t(
                "이 설정 창의 번역은 초안이며 원어민 검토를 거치지 않았습니다.",
            )));
        }
        let path = |path: Option<PathBuf>| {
            path.map(|p| p.display().to_string())
                .unwrap_or_else(|| w.t("(없음)"))
        };
        views.push(label(w.t("설정 파일:")));
        views.push(paragraph(path(self.model.path().map(PathBuf::from))));
        views.push(label(w.t("로그 파일:")));
        views.push(paragraph(path(easime_config::log_path())));
        views.push(label(w.t("직접 고쳤다면:")));
        views.push(
            Button::new()
                .on_click(context.callback(|()| Message::Reload))
                .content(w.t("다시 읽기")),
        );

        views.push(heading(w.t("지원 상태")));
        for (name, state) in [
            (
                "한국어:",
                "두벌식 입력 · 조합 중 한 글자 한자 변환(시험 중)",
            ),
            (
                "日本語:",
                "로마자 → 가나 입력 · 단어 단위 한자 변환(시험 중)",
            ),
            ("中文:", "준비 중"),
        ] {
            views.push(row(vec![label(w.t(name)), paragraph(w.t(state))]));
        }
        views.push(label(w.t("일본어 사전:")));
        let dictionary = match &self.dictionary {
            DictionaryState::Found(_) => w.t1(
                "있음 (파일 %1개). 단어 단위로 한자를 고를 수 있습니다.",
                &self.dictionary.detail(),
            ),
            DictionaryState::Missing(_) => w.t1(
                "찾지 못했습니다. 일본어를 켜도 가나만 후보로 나옵니다. easime 패키지를 다시 설치하거나 사전 파일을 아래 폴더 중 한 곳에 두세요:\n%1",
                &self.dictionary.detail(),
            ),
            DictionaryState::Unreadable { .. } => w.t1(
                "읽지 못한 사전 파일이 %1개 있습니다. 나머지로 변환하며, 자세한 이유는 로그 파일에 있습니다.",
                &self.dictionary.detail(),
            ),
            DictionaryState::Broken(_) => w.t1(
                "사전 파일 %1개를 모두 읽지 못해 일본어를 켜도 가나만 후보로 나옵니다. 로그 파일에서 이유를 확인하고 easime 패키지를 다시 설치해 주세요.",
                &self.dictionary.detail(),
            ),
        };
        views.push(paragraph(dictionary));

        views.push(heading(w.t("처음부터 다시")));
        views.push(label(w.t("설정 초기화:")));
        views.push(
            Button::new()
                .on_click(context.callback(|()| Message::AskReset))
                .content(w.t("easime 설정을 기본값으로 되돌리기")),
        );
        views.push(
            ContentDialog::new()
                .is_open(self.confirming_reset)
                .title(w.t("기본값으로 되돌릴까요?"))
                .primary_button_text(w.t("확인"))
                .close_button_text(w.t("취소"))
                .on_closed(context.callback(Message::Reset))
                .content(paragraph(w.t(
                    "easime의 설정이 모두 기본값으로 바뀝니다. 지금 설정 파일은 config.toml.bak으로 보관되며, 입력기 선택이나 다른 프로그램의 설정은 바뀌지 않습니다.",
                ))),
        );
        views
    }

    fn languages(&self, context: &mut ViewContext<Self>) -> Vec<View> {
        let w = &self.words;
        let enabled = self.model.languages();
        // The switched-on languages in their order, then the others.
        let mut order = enabled.clone();
        order.extend(SELECTABLE.iter().filter(|l| !enabled.contains(l)));
        let mut views = vec![paragraph(w.t(
            "쓰는 언어만 켜 두세요. 어느 언어도 꼭 켜야 하는 것은 아닙니다. 아래 순서는 Shift를 누른 채 전환 키를 누르거나 순환을 고른 경우에 전환되는 순서입니다. 켠 언어는 열려 있는 모든 창에 똑같이 적용됩니다.",
        ))];
        if self.model.follows_system_language() {
            views.push(paragraph(w.t(
                "아직 직접 고르지 않아 시스템 언어에 맞춰져 있습니다. 직접 바꾸면 그 선택이 저장됩니다.",
            )));
        }
        for language in order {
            let on = enabled.contains(&language);
            let place = enabled.iter().position(|&l| l == language);
            let name = language_name(w, language);
            views.push(row(vec![
                ToggleSwitch::new()
                    .is_on(on)
                    .on_toggled(context.callback(move |on| Message::Language(language, on)))
                    .automation_name(name.clone())
                    .min_width(160.0)
                    .slot(ToggleSwitchSlot::Header, name.clone()),
                Button::new()
                    .is_enabled(place.is_some_and(|p| p > 0))
                    .on_click(context.callback(move |()| Message::Move(language, true)))
                    .automation_name(w.t1("%1: 전환 순서 앞으로", &name))
                    .content("↑"),
                Button::new()
                    .is_enabled(place.is_some_and(|p| p + 1 < enabled.len()))
                    .on_click(context.callback(move |()| Message::Move(language, false)))
                    .automation_name(w.t1("%1: 전환 순서 뒤로", &name))
                    .content("↓"),
            ]));
        }
        let cycle: Vec<String> = std::iter::once(w.t("영어"))
            .chain(enabled.iter().map(|&l| language_name(w, l)))
            .chain(std::iter::once(w.t("영어")))
            .collect();
        views.push(paragraph(w.t1("전환 순서: %1", &cycle.join(" → "))));
        if enabled.is_empty() {
            views.push(paragraph(w.t("켜진 언어가 없어 영어로만 입력합니다.")));
        }
        views
    }

    fn shortcuts(&self, context: &mut ViewContext<Self>) -> Vec<View> {
        let w = &self.words;
        let current = self.model.config().shortcuts.toggle_key;
        let mode = self.model.config().languages.switch;
        vec![
            RadioButtons::new()
                .items_source(TOGGLE_KEYS.iter().map(|(_, text)| w.t(text)))
                .selected_index(TOGGLE_KEYS.iter().position(|(key, _)| *key == current))
                .on_selection_changed(context.callback(Message::ToggleKey))
                .slot(RadioButtonsSlot::Header, w.t("언어 전환 키:")),
            paragraph(w.t(
                "한/영 키는 누르는 즉시 전환합니다. 오른쪽 Alt는 다른 키를 누르지 않고 눌렀다 뗄 때만 전환하므로, Alt와 다른 키를 함께 누르는 단축키는 그대로 쓸 수 있습니다.",
            )),
            RadioButtons::new()
                .items_source(SWITCH_MODES.iter().map(|(_, text)| w.t(text)))
                .selected_index(SWITCH_MODES.iter().position(|(m, _)| *m == mode))
                .on_selection_changed(context.callback(Message::SwitchMode))
                .slot(RadioButtonsSlot::Header, w.t("언어 전환 방식:")),
            paragraph(w.t(
                "전환 키를 한 번 누르면 영어와 마지막으로 쓴 언어 사이를 오갑니다. 한국어와 영어를 섞어 쓸 때 늘 한 번이면 됩니다. Shift를 누른 채 전환 키를 누르면 다음 언어로 갑니다. 순환을 고르면 전환 키가 영어, 한국어, 일본어 순서로 한 바퀴씩 돕니다. 켠 언어가 영어와 하나뿐이면 두 방식이 같습니다.",
            )),
        ]
    }

    fn korean(&self, context: &mut ViewContext<Self>) -> Vec<View> {
        let w = &self.words;
        let current = self.model.config().korean.backspace;
        vec![
            RadioButtons::new()
                .items_source(BACKSPACE.iter().map(|(_, text)| w.t(text)))
                .selected_index(BACKSPACE.iter().position(|(unit, _)| *unit == current))
                .on_selection_changed(context.callback(Message::Backspace))
                .slot(RadioButtonsSlot::Header, w.t("Backspace로 지우는 단위:")),
            paragraph(w.t(
                "글자를 조합하는 중에만 적용됩니다. 이미 입력을 마친 글자는 앱의 Backspace 동작을 따릅니다.",
            )),
        ]
    }

    fn appearance(&self, context: &mut ViewContext<Self>) -> Vec<View> {
        let w = &self.words;
        let appearance = &self.model.config().appearance;
        vec![
            ToggleSwitch::new()
                .is_on(appearance.indicator_box)
                .on_toggled(context.callback(Message::IndicatorBox))
                .slot(
                    ToggleSwitchSlot::Header,
                    w.t("언어를 바꿀 때 커서 옆에 상자로 알려 주기"),
                ),
            NumberBox::new()
                .minimum(0.1)
                .maximum(10.0)
                .value(f64::from(appearance.indicator_ms) / 1000.0)
                .is_enabled(appearance.indicator_box)
                .on_value_changed(context.callback(Message::IndicatorSeconds))
                .automation_name(w.t("표시 시간 (초)"))
                .slot(NumberBoxSlot::Header, w.t("표시 시간 (초)")),
            paragraph(w.t(
                "입력창을 누르거나 창을 옮길 때는 표시하지 않고, 사용자가 언어를 바꿀 때만 잠깐 나타납니다.",
            )),
        ]
    }
}
