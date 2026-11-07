# easime

Easy Eastern Asian IME: a Korean (then Japanese, Chinese) input method written from scratch in Rust, without ibus/fcitx.
Platforms: Linux (KDE Plasma 6 Wayland first, then GNOME), then Windows, then macOS. License: MIT OR Apache-2.0.
Status: early; not yet tried on a real KWin. The `docs/` files are in Korean; read them only when needed (table at the end).

## Working with the user
- Talk to the user in **Korean**. Code identifiers, comments and commit messages are English.
- Product decisions are the user's (`docs/decisions.md`). When blocked on one, ask with a choice prompt instead of guessing.
- Never change the user's desktop settings yourself (kwinrc, global shortcuts, input method registration): give the command and let them run it.
  The same holds for the product: changing a user's settings needs their confirmation and an undo.

## Commands
```sh
cargo build --release
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace          # one crate: cargo test -p engine-hangul
cargo run -p easime             # debugging only; KWin normally launches the daemon
```
Linux builds need `libxkbcommon` and Wayland dev packages (list in `docs/development.md`).

## Crates (`crates/`)
`ime-core` (key events, `InputEngine`, `LanguageSwitcher`), `engine-hangul`, `engine-japanese` (romaji to kana, dictionaries),
`easime-dict-tool` (Mozc dictionary to binary), `easime-config` (TOML settings, OS paths), `easime-cli` (terminal demo),
`frontend-wayland` (KWin `zwp_input_method_v1`), `easime` (Linux daemon). Planned crates: `docs/architecture.md`.
Portable crates, which also build on Windows and macOS CI: `ime-core`, `engine-hangul`, `engine-japanese`, `easime-dict-tool`, `easime-config`, `easime-cli`.

## Rules
- Engine crates (`ime-core`, `engine-*`) have no OS or UI dependencies (Wayland, X11, Qt, Win32). OS-specific code goes in its own crate or under `#[cfg(target_os = ...)]`.
- A language engine only implements `InputEngine`; frontends and settings apps must work with any engine.
- Tests in portable crates must not hard-code absolute paths or path separators (this has failed the Windows CI).
- New third-party data: check its license, keep the original LICENSE in `data/<source>/`, record it in `THIRD_PARTY_LICENSES.md`.
- Before every commit run fmt, clippy and test, chained with `&&` so a failure aborts the commit.
- Small commits, pushed right after committing. Stage explicit paths, never `git add -A` (agent worktrees were committed that way once).
- **Until v0.1.0 is released, push straight to `main` and do not open pull requests** (decided by the user; revisit after the release).
  CI only runs after the push, so run the checks above first, and fix a red CI on `main` before any other work.
  For OS-specific code, run CI by hand on a branch first (Actions > CI > Run workflow). Do not push tags: releases are the user's call.

## LLM attribution (every tool)
- End each commit that an LLM wrote or substantially changed with `Co-Authored-By: <tool> <model and version> <noreply address>`.
  Name the model and version, not just the tool; one line per model. Trivial help (autocomplete, typo fixes) needs no trailer.
- Claude Code adds its trailer itself; other tools do not, so add it yourself.
- Fill in "LLM assistance" in the PR template. Model names belong only in trailers and PR descriptions, never in code comments or docs.

## Read when needed
| File | When |
|---|---|
| `docs/concept.md` | product concept, target user, what the popups and indicator should feel like |
| `docs/decisions.md` | any design or product question: goals, decided choices, license notes |
| `docs/architecture.md` | touching a frontend or protocol (KWin, GNOME, Windows, macOS), config format, settings GUI, packaging |
| `docs/status.md` | choosing what to do next: roadmap, what exists, known gaps, unfinished branches |
| `docs/development.md` | trying the daemon in KWin, recovery steps, build dependencies |
