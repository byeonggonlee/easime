//! The task bar icon: the active language as a letter, so it can be seen
//! without typing anything (the box next to the text cursor only appears when
//! the user switches language).
//!
//! **OS-specific: Linux** (KDE Plasma, and any desktop with a
//! StatusNotifierItem host). Windows shows the language in the notification
//! area through its own input indicator and macOS in the menu bar's input
//! source menu (`docs/platforms.md`).
//!
//! The icon is drawn like the box next to the cursor (`frontend_wayland::indicator`)
//! so both look the same. Everything here runs off the input thread: the
//! input method only sends "the language is now X" through a channel, and a
//! worker thread talks to D-Bus, so a slow desktop shell can never delay typing.

use frontend_wayland::font::Fonts;
use frontend_wayland::indicator::{Bitmap, render};
use ime_core::Language;
use ime_session::indicator::{language_glyph, language_native_name};
use ksni::menu::StandardItem;
use ksni::{Category, Icon, MenuItem, Status, ToolTip, Tray};
use std::collections::HashMap;
use std::sync::mpsc::{self, Sender};

/// Sizes the icon is offered in; the shell picks the one that fits.
const ICON_SIZES: [u32; 4] = [16, 24, 32, 48];

/// Every glyph the tray can show, drawn at every size.
const GLYPHS: [Language; 3] = [Language::English, Language::Korean, Language::Japanese];

/// A drawn box as the StatusNotifierItem specification wants it: ARGB32 in
/// network byte order (the bytes are A, R, G, B) and **not** premultiplied.
/// The drawing is premultiplied B, G, R, A.
pub fn icon_from_bitmap(bitmap: &Bitmap) -> Icon {
    let mut data = Vec::with_capacity((bitmap.width * bitmap.height * 4) as usize);
    for y in 0..bitmap.height {
        for x in 0..bitmap.width {
            let [a, r, g, b] = bitmap.argb(x, y);
            let straight = |channel: u8| {
                if a == 0 {
                    0
                } else {
                    (u32::from(channel) * 255 / u32::from(a)).min(255) as u8
                }
            };
            data.extend_from_slice(&[a, straight(r), straight(g), straight(b)]);
        }
    }
    Icon {
        width: bitmap.width as i32,
        height: bitmap.height as i32,
        data,
    }
}

/// The icons of every language the tray can show.
pub struct Icons {
    by_glyph: HashMap<char, Vec<Icon>>,
}

impl Icons {
    /// Draw the boxes with a font that has each glyph; a glyph without a font
    /// leaves its box empty (the daemon's log already says so for the cursor box).
    pub fn draw(fonts: &Fonts) -> Self {
        let mut by_glyph = HashMap::new();
        for language in GLYPHS {
            let glyph = language_glyph(language);
            let font = fonts.font_for(glyph);
            let icons = ICON_SIZES
                .iter()
                .map(|&size| icon_from_bitmap(&render(glyph, font.as_ref(), size)))
                .collect();
            by_glyph.insert(glyph, icons);
        }
        Self { by_glyph }
    }

    fn of(&self, language: Language) -> Vec<Icon> {
        self.by_glyph
            .get(&language_glyph(language))
            .cloned()
            .unwrap_or_default()
    }
}

/// The words the tray shows, in the user's display language. The translations
/// are the settings window's (drafts until a native speaker has reviewed them).
#[derive(Debug, PartialEq, Eq)]
pub struct Labels {
    pub settings: &'static str,
}

const ENGLISH: Labels = Labels {
    settings: "easime Settings",
};
const KOREAN: Labels = Labels {
    settings: "easime 설정",
};
const JAPANESE: Labels = Labels {
    settings: "easime 設定",
};
const TRADITIONAL_CHINESE: Labels = Labels {
    settings: "easime 設定",
};
const SIMPLIFIED_CHINESE: Labels = Labels {
    settings: "easime 设置",
};

/// The labels for the first of `tags` that is one of the five display
/// languages (the same five as the settings window); English otherwise.
pub fn labels_for(tags: &[String]) -> &'static Labels {
    for tag in tags {
        match easime_config::language_for_tag(tag) {
            Some(Language::Korean) => return &KOREAN,
            Some(Language::Japanese) => return &JAPANESE,
            Some(Language::ChineseTraditional) => return &TRADITIONAL_CHINESE,
            Some(Language::ChineseSimplified) => return &SIMPLIFIED_CHINESE,
            Some(Language::English) | None => {
                if tag.to_ascii_lowercase().starts_with("en") {
                    return &ENGLISH;
                }
            }
        }
    }
    &ENGLISH
}

/// The StatusNotifierItem.
pub struct LanguageTray {
    language: Language,
    icons: Icons,
    labels: &'static Labels,
    open_settings: Box<dyn Fn() + Send>,
}

impl LanguageTray {
    pub fn new(
        language: Language,
        icons: Icons,
        labels: &'static Labels,
        open_settings: Box<dyn Fn() + Send>,
    ) -> Self {
        Self {
            language,
            icons,
            labels,
            open_settings,
        }
    }
}

impl Tray for LanguageTray {
    fn id(&self) -> String {
        "easime".into()
    }

    fn category(&self) -> Category {
        Category::ApplicationStatus
    }

    fn title(&self) -> String {
        "easime".into()
    }

    fn status(&self) -> Status {
        Status::Active
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        self.icons.of(self.language)
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "easime".into(),
            description: language_native_name(self.language).into(),
            ..ToolTip::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: self.labels.settings.into(),
                icon_name: "configure".into(),
                activate: Box::new(|tray: &mut Self| (tray.open_settings)()),
                ..StandardItem::default()
            }
            .into(),
        ]
    }
}

/// Start the settings window. Failing to start it (it is not installed, there
/// is no display) is reported to the log and nothing else.
pub fn open_settings_window() {
    match std::process::Command::new("easime-settings")
        .stdin(std::process::Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            // Reap it when it exits, so no zombie is left behind.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(e) => frontend_wayland::log!("tray: could not start easime-settings: {e}"),
    }
}

/// What the input method tells the tray.
pub type LanguageNews = (Language, Vec<Language>);

/// Put the icon in the task bar and keep it showing the active language.
///
/// Returns the way to tell it the language, immediately: the icon itself is
/// set up on a worker thread (finding fonts, talking to D-Bus), and a desktop
/// without a task bar just means the news goes nowhere.
pub fn start() -> Sender<LanguageNews> {
    let (sender, receiver) = mpsc::channel::<LanguageNews>();
    let spawned = std::thread::Builder::new()
        .name("easime-tray".into())
        .spawn(move || {
            use ksni::blocking::TrayMethods;
            let icons = Icons::draw(&Fonts::load_system());
            let labels = labels_for(&easime_config::system_language_tags());
            let tray = LanguageTray::new(
                Language::English,
                icons,
                labels,
                Box::new(open_settings_window),
            );
            // `assume_sni_available`: at login the daemon may start before the
            // desktop shell has a task bar; keep trying instead of giving up.
            let handle = match tray.assume_sni_available(true).spawn() {
                Ok(handle) => handle,
                Err(e) => {
                    frontend_wayland::log!("tray: no task bar icon: {e}");
                    for _ in receiver.iter() {} // nobody listens; do not let news pile up
                    return;
                }
            };
            frontend_wayland::log!("tray: icon registered");
            for (language, _cycle) in receiver.iter() {
                if handle.update(|tray| tray.language = language).is_none() {
                    frontend_wayland::log!("tray: the icon service stopped");
                    break;
                }
            }
            for _ in receiver.iter() {}
        });
    if let Err(e) = spawned {
        frontend_wayland::log!("tray: could not start its thread: {e}");
    }
    sender
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn tray(language: Language) -> (LanguageTray, Arc<AtomicUsize>) {
        let opened = Arc::new(AtomicUsize::new(0));
        let counter = opened.clone();
        let tray = LanguageTray::new(
            language,
            Icons::draw(&Fonts::empty()),
            &KOREAN,
            Box::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }),
        );
        (tray, opened)
    }

    fn bitmap_with(pixel: [u8; 4]) -> Bitmap {
        // The drawing is B, G, R, A per pixel.
        Bitmap {
            width: 1,
            height: 1,
            stride: 4,
            data: vec![pixel[2], pixel[1], pixel[0], pixel[3]],
        }
    }

    #[test]
    fn an_opaque_pixel_becomes_argb_in_network_order() {
        // r, g, b, a
        let icon = icon_from_bitmap(&bitmap_with([10, 20, 30, 255]));
        assert_eq!((icon.width, icon.height), (1, 1));
        assert_eq!(icon.data, [255, 10, 20, 30]);
    }

    #[test]
    fn a_translucent_pixel_is_unpremultiplied() {
        // Premultiplied by 128/255: the straight colour is about twice as bright.
        let icon = icon_from_bitmap(&bitmap_with([50, 100, 25, 128]));
        assert_eq!(icon.data[0], 128);
        assert_eq!(&icon.data[1..], [99, 199, 49]);
        // A clear pixel has no colour to restore.
        assert_eq!(
            icon_from_bitmap(&bitmap_with([0, 0, 0, 0])).data,
            [0, 0, 0, 0]
        );
        // Rounding must never overflow a channel.
        assert_eq!(
            icon_from_bitmap(&bitmap_with([255, 255, 255, 200])).data[1],
            255
        );
    }

    #[test]
    fn every_language_has_an_icon_in_every_size() {
        let (mut tray, _) = tray(Language::Korean);
        for language in GLYPHS {
            tray.language = language;
            let icons = tray.icon_pixmap();
            let sizes: Vec<i32> = icons.iter().map(|icon| icon.width).collect();
            assert_eq!(sizes, ICON_SIZES.map(|size| size as i32), "{language:?}");
            for icon in &icons {
                assert_eq!(icon.width, icon.height);
                assert_eq!(icon.data.len() as i32, icon.width * icon.height * 4);
            }
        }
    }

    #[test]
    fn the_two_chinese_scripts_share_the_letter_for_now() {
        let (mut tray, _) = tray(Language::ChineseSimplified);
        assert!(
            tray.icon_pixmap().is_empty(),
            "no Chinese engine, so no tray letter yet"
        );
        tray.language = Language::Korean;
        assert!(!tray.icon_pixmap().is_empty());
    }

    #[test]
    fn the_tooltip_names_the_language_in_its_own_script() {
        let (mut tray, _) = tray(Language::Korean);
        assert_eq!(tray.tool_tip().title, "easime");
        assert_eq!(tray.tool_tip().description, "한국어");
        tray.language = Language::Japanese;
        assert_eq!(tray.tool_tip().description, "日本語");
        tray.language = Language::English;
        assert_eq!(tray.tool_tip().description, "English");
    }

    #[test]
    fn the_item_is_an_active_application_status() {
        let (tray, _) = tray(Language::Korean);
        assert_eq!(tray.id(), "easime");
        assert!(matches!(tray.category(), Category::ApplicationStatus));
        assert!(matches!(tray.status(), Status::Active));
        assert_eq!(tray.icon_name(), "", "the drawn letter is the icon");
    }

    #[test]
    fn the_menu_opens_the_settings() {
        let (mut tray, opened) = tray(Language::Korean);
        let menu = tray.menu();
        assert_eq!(menu.len(), 1);
        let MenuItem::Standard(item) = &menu[0] else {
            panic!("a plain item");
        };
        assert_eq!(item.label, "easime 설정");
        (item.activate)(&mut tray);
        assert_eq!(opened.load(Ordering::SeqCst), 1);
    }

    fn tags(list: &[&str]) -> Vec<String> {
        list.iter().map(|tag| (*tag).to_owned()).collect()
    }

    #[test]
    fn the_labels_follow_the_first_display_language_among_the_tags() {
        assert_eq!(labels_for(&tags(&["ko_KR.UTF-8"])), &KOREAN);
        assert_eq!(labels_for(&tags(&["ja"])), &JAPANESE);
        assert_eq!(labels_for(&tags(&["zh_TW"])), &TRADITIONAL_CHINESE);
        assert_eq!(labels_for(&tags(&["zh-Hant-HK"])), &TRADITIONAL_CHINESE);
        assert_eq!(labels_for(&tags(&["zh_CN"])), &SIMPLIFIED_CHINESE);
        assert_eq!(labels_for(&tags(&["en_US", "ko"])), &ENGLISH);
        // A language we have no words for is skipped.
        assert_eq!(labels_for(&tags(&["fr_FR", "ja_JP"])), &JAPANESE);
        assert_eq!(labels_for(&tags(&["fr_FR"])), &ENGLISH);
        assert_eq!(labels_for(&[]), &ENGLISH);
    }
}
