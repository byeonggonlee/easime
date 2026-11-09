# easime

**Easy Eastern Asian IME**: a Korean, Japanese and (planned) Chinese input method written from scratch in Rust.
It does not sit on top of ibus, fcitx or any other input method framework.

> **Early development. No release has been published.**
> Everything below is built and tested by CI. People have tried only part of it: basic Korean/English typing on Fedora KDE,
> and typing, Hanja and Japanese conversion on a real Windows PC (no problems reported, details not recorded).

## What works

| Platform | State |
|---|---|
| **Linux, KDE Plasma 6 (Wayland)** | Korean (Dubeolsik), Japanese (romaji to kana, word-by-word Mozc dictionary conversion), single-syllable Korean Hanja, a candidate popup, a language indicator, a tray icon, an expert settings window (Kirigami), `.deb` and `.rpm` packages. Korean/English typing was user-tested; the rest is tested against a fake compositor and in clean containers. |
| **Windows** | A TSF text service for Korean and Japanese (underlined compositions, candidate window), an installer (`setup.exe` with an MSI inside) and a WinUI 3 settings app. Tested in CI against a simulated TSF application, and the typing, Hanja and Japanese conversion tests were run on a real PC (no problems reported). The settings app, `setup.exe` on a PC without the runtime, and uninstalling have not been tried by a person. |
| GNOME, macOS, XWayland apps, Chinese | Planned. |

Japanese conversion is word by word (no clause or context conversion yet). Settings-window translations other than Korean are unreviewed drafts.
See [docs/status.md](docs/status.md) for what is verified and what is not.

## Try it

There are no published downloads yet. Packages are built by CI on every push (Actions artifacts `easime-linux-packages-<commit>` and `easime-windows-installer-<commit>`).

- **Linux**: install the `.rpm` (Fedora) or `.deb` (Debian 13 or later) and open "easime Settings" from the app menu. Its first card switches easime on, after asking, and can undo it. You have to log in again.
- **Windows**: run `easime-setup-<version>-x64.exe`. It installs the text service and the settings app, and downloads Microsoft's Windows App Runtime only if the PC lacks it.
- **From source**: `cargo build --release`. Build dependencies, running the daemon in KWin and recovery steps are in [docs/development.md](docs/development.md).

If typing ever stops working on KDE, pick the old keyboard again under System Settings → Keyboard → Virtual Keyboard.

## Documentation

- [Document index](docs/README.md) (most documents are in Korean)
- [Product concept](docs/concept.md), [decisions](docs/decisions.md), [UX/UI guidelines](DESIGN.md)
- [Status](docs/status.md), [architecture](docs/architecture.md), [development](docs/development.md)
- Contributors and coding agents: start with [AGENTS.md](AGENTS.md)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Third-party data and its licenses: [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
