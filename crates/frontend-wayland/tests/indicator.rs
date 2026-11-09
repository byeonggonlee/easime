//! End-to-end tests of the language indicator box against the fake KWin:
//! what the client sends over `wl_compositor`, `wl_shm` and
//! `zwp_input_panel_v1`, and the pixels it puts in the buffer.

mod common;

use common::{Fake, Globals, PanelEv, Req, Shot};
use easime_config::Config;
use ime_core::Language;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const KEY_G: u32 = 34;
const KEY_K: u32 = 37;
const KEY_HANGEUL: u32 = 122;

/// A long time, so a box never disappears during a test by accident.
const LONG_MS: u32 = 60_000;

/// Korean and Japanese enabled; the box stays `indicator_ms` milliseconds.
fn config(indicator_ms: u32) -> Config {
    let mut config = Config::default();
    config.languages.enabled = Some(vec![Language::Korean, Language::Japanese]);
    config.languages.switch = ime_core::SwitchMode::Cycle;
    config.appearance.indicator_ms = indicator_ms;
    config
}

/// True if this machine has fonts for all the glyphs the tests draw. Tests
/// that look at glyph pixels (not just the box) skip themselves otherwise.
fn have_fonts() -> bool {
    static HAVE: OnceLock<bool> = OnceLock::new();
    *HAVE.get_or_init(|| {
        let fonts = frontend_wayland::font::Fonts::load_system();
        let all = ['A', '한', 'あ', '中']
            .iter()
            .all(|&c| fonts.font_for(c).is_some());
        if !all {
            eprintln!("note: no installed font has every indicator glyph; glyph pixel checks skip");
        }
        all
    })
}

fn tap(fake: &mut Fake, key: u32) {
    fake.tap_all(&[key]);
}

fn count(events: &[PanelEv], wanted: impl Fn(&PanelEv) -> bool) -> usize {
    events.iter().filter(|e| wanted(e)).count()
}

fn is_null_commit(e: &PanelEv) -> bool {
    matches!(
        e,
        PanelEv::Commit {
            attached_null: true,
            ..
        }
    )
}

fn is_shown_commit(e: &PanelEv) -> bool {
    matches!(
        e,
        PanelEv::Commit {
            buffer: Some(_),
            ..
        }
    )
}

#[test]
fn focusing_and_typing_do_not_show_a_language_box() {
    let mut fake = Fake::start_with(config(LONG_MS));
    fake.activate();
    fake.tap_all(&[KEY_G, KEY_K]);
    let events = fake.settle_panel();
    assert!(!events.iter().any(is_shown_commit), "{events:#?}");
    assert!(!fake.indicator_mapped());

    fake.deactivate();
    fake.activate();
    let events = fake.settle_panel();
    assert!(!events.iter().any(is_shown_commit), "{events:#?}");
    assert!(!fake.indicator_mapped());

    tap(&mut fake, KEY_HANGEUL);
    assert!(fake.wait_for_indicator_shown().visible_pixels() > 0);
}

#[test]
fn toggling_sets_the_panel_role_before_the_first_visible_commit() {
    let mut fake = Fake::start_with(config(LONG_MS));
    fake.activate();
    tap(&mut fake, KEY_HANGEUL);
    let events = fake.wait_for_indicator_events();

    let position = |wanted: &dyn Fn(&PanelEv) -> bool| {
        events
            .iter()
            .position(wanted)
            .unwrap_or_else(|| panic!("event missing in {events:#?}"))
    };
    let surface = position(&|e| *e == PanelEv::SurfaceCreated);
    let role = position(&|e| *e == PanelEv::GetPanelSurface);
    let overlay = position(&|e| *e == PanelEv::SetOverlayPanel);
    let region = position(&|e| *e == PanelEv::SetInputRegion { empty: true });
    let pool = position(&|e| matches!(e, PanelEv::ShmPool { .. }));
    let commit = position(&is_shown_commit);
    // KWin picks the placement mode from set_overlay_panel, so it must come
    // before the first commit maps the surface.
    assert!(
        surface < role && role < overlay && overlay < commit,
        "{events:#?}"
    );
    assert!(region < commit && pool < commit, "{events:#?}");
    assert_eq!(count(&events, |e| *e == PanelEv::SetToplevel), 0);

    let Some(PanelEv::Commit {
        buffer: Some(shot), ..
    }) = events.last()
    else {
        panic!("last event is not the commit: {events:#?}");
    };
    assert_eq!((shot.width, shot.height), (48, 48));
    assert_eq!(shot.stride, 48 * 4);
    assert_eq!(shot.format, 0, "ARGB8888");
    assert_eq!(shot.pixels.len(), 48 * 48 * 4);
    assert!(shot.visible_pixels() > 0, "the box must not be transparent");
    // Translucent background: next to the glyph the box is neither clear nor
    // opaque. Corners are rounded, so they are fully transparent.
    let alpha = |x: usize, y: usize| shot.pixels[y * shot.stride as usize + x * 4 + 3];
    assert!(alpha(3, 24) > 0 && alpha(3, 24) < 255);
    assert_eq!(alpha(0, 0), 0);
    if have_fonts() {
        assert!(shot.bright_pixels() > 20, "no glyph drawn");
    }
}

#[test]
fn the_box_has_the_buffer_size_the_pool_and_buffer_requests_announced() {
    let mut fake = Fake::start_with(config(LONG_MS));
    fake.activate();
    tap(&mut fake, KEY_HANGEUL);
    let events = fake.wait_for_indicator_events();
    assert!(
        events.contains(&PanelEv::ShmPool { size: 48 * 48 * 4 }),
        "{events:#?}"
    );
    assert!(
        events.contains(&PanelEv::ShmBuffer {
            width: 48,
            height: 48,
            stride: 192,
            format: 0,
        }),
        "{events:#?}"
    );
}

#[test]
fn toggling_shows_the_box_of_the_new_language_and_reuses_surface_and_buffers() {
    let mut fake = Fake::start_with(config(LONG_MS));
    fake.activate();
    tap(&mut fake, KEY_HANGEUL); // Korean -> Japanese
    let japanese = fake.wait_for_indicator_shown();
    tap(&mut fake, KEY_HANGEUL); // Japanese -> English
    let english = fake.wait_for_indicator_shown();
    tap(&mut fake, KEY_HANGEUL); // English -> Korean
    let korean = fake.wait_for_indicator_shown();
    tap(&mut fake, KEY_HANGEUL); // Korean -> Japanese
    let japanese_again = fake.wait_for_indicator_shown();

    for shot in [&korean, &japanese, &english, &japanese_again] {
        assert_eq!((shot.width, shot.height), (48, 48));
        assert!(shot.visible_pixels() > 0);
    }
    if have_fonts() {
        let digests = [korean.digest(), japanese.digest(), english.digest()];
        assert_ne!(digests[0], digests[1], "한 and あ look the same");
        assert_ne!(digests[0], digests[2], "한 and A look the same");
        assert_ne!(digests[1], digests[2], "あ and A look the same");
        assert_eq!(japanese.digest(), japanese_again.digest());
    }

    // Everything was created once: one surface and role, one buffer per glyph.
    let events = fake.settle_panel();
    assert_eq!(
        count(&events, |e| *e == PanelEv::SurfaceCreated),
        0,
        "{events:#?}"
    );
    assert_eq!(count(&events, |e| *e == PanelEv::GetPanelSurface), 0);
    assert_eq!(
        count(&events, |e| matches!(e, PanelEv::ShmBuffer { .. })),
        0
    );
}

#[test]
fn only_one_surface_and_one_buffer_per_glyph_are_ever_created() {
    let mut fake = Fake::start_with(config(LONG_MS));
    fake.activate();
    tap(&mut fake, KEY_HANGEUL);
    let mut events = fake.wait_for_indicator_events();
    for _ in 0..3 {
        tap(&mut fake, KEY_HANGEUL);
        events.extend(fake.wait_for_indicator_events());
    }
    assert_eq!(count(&events, |e| *e == PanelEv::SurfaceCreated), 1);
    assert_eq!(count(&events, |e| *e == PanelEv::GetPanelSurface), 1);
    assert_eq!(count(&events, |e| *e == PanelEv::SetOverlayPanel), 1);
    // 한, あ and A: three buffers for four shows.
    assert_eq!(
        count(&events, |e| matches!(e, PanelEv::ShmBuffer { .. })),
        3
    );
    assert_eq!(count(&events, is_shown_commit), 4);
}

#[test]
fn the_box_disappears_after_the_configured_time() {
    let mut fake = Fake::start_with(config(50));
    fake.activate();
    tap(&mut fake, KEY_HANGEUL);
    fake.wait_for_indicator_shown();
    let shown_at = Instant::now();
    assert!(fake.indicator_mapped());

    fake.wait_for_indicator_hidden();
    assert!(!fake.indicator_mapped());
    // The timer cannot fire before 50 ms after the commit; allow slack for
    // the time between the commit and our noticing it.
    assert!(
        shown_at.elapsed() >= Duration::from_millis(25),
        "hidden after only {:?}",
        shown_at.elapsed()
    );
    // Hiding unmaps (null buffer); the surface itself stays for the next time.
    let events = fake.take_panel_events();
    assert!(events.iter().any(is_null_commit), "{events:#?}");
    assert_eq!(count(&events, |e| *e == PanelEv::SurfaceDestroyed), 0);
}

#[test]
fn the_box_stays_while_the_time_has_not_passed() {
    let mut fake = Fake::start_with(config(LONG_MS));
    fake.activate();
    tap(&mut fake, KEY_HANGEUL);
    fake.wait_for_indicator_shown();
    std::thread::sleep(Duration::from_millis(80));
    let events = fake.settle_panel();
    assert!(fake.indicator_mapped());
    assert_eq!(count(&events, is_null_commit), 0, "{events:#?}");
}

#[test]
fn the_box_can_be_shown_again_after_it_disappeared() {
    let mut fake = Fake::start_with(config(40));
    fake.activate();
    tap(&mut fake, KEY_HANGEUL);
    fake.wait_for_indicator_shown();
    fake.wait_for_indicator_hidden();
    fake.take_panel_events();

    tap(&mut fake, KEY_HANGEUL);
    let again = fake.wait_for_indicator_shown();
    assert!(again.visible_pixels() > 0);
    fake.wait_for_indicator_hidden();
}

#[test]
fn losing_focus_hides_the_box_at_once() {
    let mut fake = Fake::start_with(config(LONG_MS));
    fake.activate();
    tap(&mut fake, KEY_HANGEUL);
    fake.wait_for_indicator_shown();
    fake.deactivate();
    fake.wait_for_indicator_hidden();
    assert!(!fake.indicator_mapped());

    // Refocusing must not remap the box. Only a new language switch does.
    fake.take_panel_events();
    fake.activate();
    let events = fake.settle_panel();
    assert!(!events.iter().any(is_shown_commit), "{events:#?}");
    assert!(!fake.indicator_mapped());
    tap(&mut fake, KEY_HANGEUL);
    fake.wait_for_indicator_shown();
}

#[test]
fn the_box_does_not_get_in_the_way_of_typing() {
    let mut fake = Fake::start_with(config(LONG_MS));
    fake.activate();
    // Cycle back to Korean, showing the box through explicit switches.
    for _ in 0..3 {
        tap(&mut fake, KEY_HANGEUL);
        fake.wait_for_indicator_shown();
    }
    fake.take_requests();

    // No second keyboard grab or other input method requests came from it;
    // Korean composition works as usual while the box is up.
    fake.tap_all(&[KEY_G, KEY_K]);
    let requests = fake.settle();
    assert!(
        requests
            .iter()
            .any(|r| matches!(r, Req::PreeditString { text, .. } if text == "하")),
        "{requests:#?}"
    );
    assert!(!requests.contains(&Req::GrabKeyboard), "{requests:#?}");
}

#[test]
fn the_box_is_not_shown_when_disabled_in_settings() {
    let mut config = config(LONG_MS);
    config.appearance.indicator_box = false;
    let mut fake = Fake::start_with(config);
    fake.activate();
    tap(&mut fake, KEY_HANGEUL);
    tap(&mut fake, KEY_HANGEUL);
    let events = fake.settle_panel();
    assert_eq!(events, [], "no surface, buffer or commit expected");
    assert!(!fake.indicator_mapped());
}

fn lacking(globals: Globals, what: &str) {
    let mut fake = Fake::start_with_globals(config(LONG_MS), globals);
    fake.activate();
    tap(&mut fake, KEY_HANGEUL); // Korean -> Japanese
    tap(&mut fake, KEY_HANGEUL); // -> English
    tap(&mut fake, KEY_HANGEUL); // -> Korean
    let events = fake.settle_panel();
    assert!(
        !events.iter().any(is_shown_commit) && !fake.indicator_mapped(),
        "a box was shown without {what}: {events:#?}"
    );
    assert_eq!(
        count(&events, |e| *e == PanelEv::GetPanelSurface),
        0,
        "{events:#?}"
    );

    // The daemon itself keeps working.
    fake.take_requests();
    fake.tap_all(&[KEY_G, KEY_K]);
    let requests = fake.settle();
    assert!(
        requests
            .iter()
            .any(|r| matches!(r, Req::PreeditString { text, .. } if text == "하")),
        "typing broke without {what}: {requests:#?}"
    );
}

#[test]
fn the_daemon_works_without_wl_shm() {
    lacking(
        Globals {
            shm: false,
            ..Globals::default()
        },
        "wl_shm",
    );
}

#[test]
fn the_daemon_works_without_wl_compositor() {
    lacking(
        Globals {
            compositor: false,
            ..Globals::default()
        },
        "wl_compositor",
    );
}

#[test]
fn the_daemon_works_without_zwp_input_panel() {
    lacking(
        Globals {
            input_panel: false,
            ..Globals::default()
        },
        "zwp_input_panel_v1",
    );
}

/// A fresh settings file path in a private temp directory.
fn settings_file(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("easime-indicator-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("config.toml")
}

#[test]
fn saving_settings_with_the_box_off_hides_it_and_stops_new_ones() {
    let path = settings_file("off");
    let config = config(LONG_MS);
    config.save_to(&path).unwrap();
    let mut fake = Fake::start_watching(config.clone(), Some(path.clone()));
    fake.activate();
    tap(&mut fake, KEY_HANGEUL);
    fake.wait_for_indicator_shown();

    let mut updated = config;
    updated.appearance.indicator_box = false;
    updated.save_to(&path).unwrap();
    // The visible box goes away at once ...
    fake.wait_for_indicator_hidden();
    fake.take_panel_events();

    // ... and toggling or regaining focus no longer shows one.
    tap(&mut fake, KEY_HANGEUL);
    fake.deactivate();
    fake.activate();
    let events = fake.settle_panel();
    assert!(!events.iter().any(is_shown_commit), "{events:#?}");
    assert!(!fake.indicator_mapped());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn saving_settings_with_the_box_on_makes_it_appear_from_then_on() {
    let path = settings_file("on");
    let mut config = config(LONG_MS);
    config.appearance.indicator_box = false;
    config.save_to(&path).unwrap();
    let mut fake = Fake::start_watching(config.clone(), Some(path.clone()));
    fake.activate();
    assert_eq!(fake.settle_panel(), []);

    let mut updated = config;
    updated.appearance.indicator_box = true;
    updated.save_to(&path).unwrap();
    fake.barrier();
    tap(&mut fake, KEY_HANGEUL);
    let shot = fake.wait_for_indicator_shown();
    assert!(shot.visible_pixels() > 0);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn saving_a_new_duration_applies_to_the_next_box() {
    let path = settings_file("duration");
    let config = config(LONG_MS);
    config.save_to(&path).unwrap();
    let mut fake = Fake::start_watching(config.clone(), Some(path.clone()));
    fake.activate();
    tap(&mut fake, KEY_HANGEUL);
    fake.wait_for_indicator_shown();

    let mut updated = config;
    updated.appearance.indicator_ms = 40;
    updated.save_to(&path).unwrap();
    fake.barrier();
    tap(&mut fake, KEY_HANGEUL);
    fake.wait_for_indicator_shown();
    fake.wait_for_indicator_hidden();
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// Write `shot` as a PNG (un-premultiplied RGBA).
fn write_png(path: &std::path::Path, shot: &Shot) {
    let bitmap = frontend_wayland::indicator::Bitmap {
        width: shot.width as u32,
        height: shot.height as u32,
        stride: shot.stride as u32,
        data: shot.pixels.clone(),
    };
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(file, bitmap.width, bitmap.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&bitmap.to_rgba())
        .unwrap();
}

/// Not a check, a way to look at the result: with `EASIME_INDICATOR_PNG_DIR`
/// set, the boxes the client really put on the wire (한, あ, A) are written
/// there, plus all four glyphs (中 too, which the daemon cannot switch to
/// yet) enlarged on a grey backdrop in `sheet.png`.
#[test]
fn dump_the_boxes_as_png_when_asked() {
    let Some(dir) = std::env::var_os("EASIME_INDICATOR_PNG_DIR") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut fake = Fake::start_with(config(LONG_MS));
    fake.activate();
    tap(&mut fake, KEY_HANGEUL);
    let mut wire = vec![("japanese", fake.wait_for_indicator_shown())];
    tap(&mut fake, KEY_HANGEUL);
    wire.push(("english", fake.wait_for_indicator_shown()));
    tap(&mut fake, KEY_HANGEUL);
    wire.push(("korean", fake.wait_for_indicator_shown()));
    for (name, shot) in &wire {
        write_png(&dir.join(format!("wire-{name}.png")), shot);
    }

    // The contact sheet: every glyph, 6x, over a grey backdrop.
    const SCALE: u32 = 6;
    const PAD: u32 = 4;
    let fonts = frontend_wayland::font::Fonts::load_system();
    let glyphs = ['A', '한', 'あ', '中'];
    let cell = 48 * SCALE + PAD * 2;
    let (width, height) = (cell * glyphs.len() as u32, cell);
    let mut sheet = vec![0u8; (width * height * 4) as usize];
    for (n, &glyph) in glyphs.iter().enumerate() {
        let font = fonts.font_for(glyph);
        let bitmap = frontend_wayland::indicator::render(glyph, font.as_ref(), 48);
        let rgba = bitmap.to_rgba();
        for y in 0..height {
            for x in 0..cell {
                let out = (((y * width) + n as u32 * cell + x) * 4) as usize;
                let (sx, sy) = (x.wrapping_sub(PAD) / SCALE, y.wrapping_sub(PAD) / SCALE);
                let (mut r, mut g, mut b) = (0x90u8, 0x90u8, 0x98u8);
                if x >= PAD && y >= PAD && sx < 48 && sy < 48 {
                    let i = ((sy * 48 + sx) * 4) as usize;
                    let a = rgba[i + 3] as u32;
                    let mix =
                        |fg: u8, bg: u8| ((fg as u32 * a + bg as u32 * (255 - a)) / 255) as u8;
                    r = mix(rgba[i], r);
                    g = mix(rgba[i + 1], g);
                    b = mix(rgba[i + 2], b);
                }
                sheet[out..out + 4].copy_from_slice(&[r, g, b, 255]);
            }
        }
    }
    let file = std::fs::File::create(dir.join("sheet.png")).unwrap();
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&sheet)
        .unwrap();
}
