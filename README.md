# easime

A Korean / Japanese / Chinese input method for **KDE Plasma 6 (Wayland)**, written from scratch in Rust.
It does not depend on ibus or fcitx.

> Status: early development. Nothing usable yet.

## Roadmap
1. Korean (Dubeolsik) composition engine
2. Wayland input method frontend (`zwp_input_method_v1`, launched by KWin)
3. Tray icon + language indicator near the text cursor
4. Settings app (Qt 6 / Kirigami, native Plasma look)
5. XIM frontend for XWayland apps (games, Wine, Steam)
6. Japanese (romaji → kana, then kana → kanji using Mozc OSS data)
7. Chinese (pinyin, Simplified and Traditional)

## License
Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
