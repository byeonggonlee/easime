# easime

A Korean / Japanese / Chinese input method for **KDE Plasma 6 (Wayland)**, written from scratch in Rust.
It does not depend on ibus or fcitx.

> Status: early development. The engines and the KDE Wayland frontend are implemented and tested
> against a fake compositor. A user reported working Korean/English input on Fedora KDE at commit `107bb4b`;
> broader real-desktop testing remains incomplete. No release has been published.

Implemented so far: Korean (Dubeolsik) input, Japanese romaji to kana with word-by-word dictionary
conversion (Mozc OSS dictionary, shown in the candidate popup; only tested against a fake compositor so far), key repeat, language cycling, live settings reload,
a language indicator shown next to the cursor only when switching languages, an expert settings window (Kirigami; in Korean, English, Japanese and Chinese, the translations being unreviewed drafts; built and clicked through in CI, not packaged yet and never opened on a real Plasma desktop by a person), and `.deb` / `.rpm` packaging. Single-syllable Korean Hanja
conversion and a numbered candidate popup are implemented; real-KDE validation is pending. Still to do:
tray icon, beginner settings and first-run guide, XWayland (XIM), Chinese, GNOME, Windows, macOS.

See [docs/concept.md](docs/concept.md) for the product concept (in Korean).
See [DESIGN.md](DESIGN.md) for the UX/UI assessment and design guidelines (in Korean).
For Hanja controls and the resolved KDE floating-panel issue, see [KDE testing](docs/kde-testing.md).
All agents should start with [AGENTS.md](AGENTS.md) and read DESIGN.md before working on this project.

## Roadmap
1. ~~Korean (Dubeolsik) composition engine~~ done
2. ~~Wayland input method frontend (`zwp_input_method_v1`, launched by KWin)~~ basic Korean/English input user-tested on Fedora KDE; broader testing pending
3. Language indicator near the text cursor: done. Tray icon: todo
4. Settings app (Qt 6 / Kirigami, native Plasma look): expert version done (separate cargo project, checked in a Fedora CI job, not packaged yet); beginner version later
5. XIM frontend for XWayland apps (games, Wine, Steam)
6. Japanese: romaji → kana done; kana → kanji with Mozc OSS data: engine, dictionary, daemon wiring and candidate popup done (fake-compositor tests only); validation on a real KDE desktop and clause conversion todo
7. Chinese (pinyin, Simplified and Traditional)

## Platforms
Linux first (KDE Plasma 6, then GNOME), followed by Windows (TSF) and macOS (InputMethodKit).
Linux packages (`.deb` / `.rpm`) are built by CI and will be published on GitHub Releases when a `v*` tag is pushed.

## License
Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
