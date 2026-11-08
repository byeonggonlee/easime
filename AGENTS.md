# easime

Easy Eastern Asian IME: a Korean (then Japanese, Chinese) input method written from scratch in Rust, without ibus/fcitx.
Platforms: Linux (KDE Plasma 6 Wayland first, then GNOME), then Windows, then macOS. License: MIT OR Apache-2.0.
Status: early; a user confirmed basic Korean/English input on Fedora KDE. Broader testing is pending (see `docs/status.md`).

## Required reading for every agent and role
- At the start of every task, read [DESIGN.md](DESIGN.md) in full and [CLAUDE.md](CLAUDE.md). This applies to implementers, reviewers, and delegated agents, regardless of tool name.
- Read [docs/concept.md](docs/concept.md) and [docs/decisions.md](docs/decisions.md) when making product decisions. Distinguish existing decisions, current observations, and new proposals in DESIGN.md.
- For changes and reviews, apply the relevant guidance on text preservation, mode clarity, accessibility, language-specific typography, and recovery. Preserve the keyboard-first, native-OS experience and Korean → Japanese → Chinese scope.
- User decisions take priority. Do not treat proposals or unverified support as approved or implemented. Update the relevant design guidance and verified scope when UX, defaults, or support status changes.
- Include these required-reading instructions when delegating work, and preserve the links if agent instructions are reorganized.

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
cargo test --manifest-path settings/kde/Cargo.toml   # settings window, its own project (needs Qt 6 + Kirigami)
```
Linux builds need `libxkbcommon` and Wayland dev packages (list in `docs/development.md`).

## Crates (`crates/`)
`ime-core` (key events, `InputEngine`, `LanguageSwitcher`), `ime-session` (the `Session` every frontend drives: engines, language cycle, switch-key rules, dictionary), `engine-hangul`, `engine-japanese` (romaji to kana, dictionaries),
`easime-dict-tool` (Mozc dictionary to binary), `easime-config` (TOML settings, OS paths), `easime-cli` (terminal demo),
`frontend-wayland` (KWin `zwp_input_method_v1`), `frontend-tsf` (Windows: key handling and the key pipeline that needs no Windows API, the TSF profile and language cycle, TSF registration, a text service that types Korean and Japanese into underlined TSF compositions, the cursor-adjacent mode box, and candidates in its own window or handed to applications that draw them; not yet tried on a real PC), `easime` (Linux daemon, with the task bar icon). Planned crates: `docs/architecture.md`.
Portable crates, which also build on Windows and macOS CI: `ime-core`, `ime-session`, `frontend-tsf`, `engine-hangul`, `engine-japanese`, `easime-dict-tool`, `easime-config`, `easime-cli`.

## Rules
- Engine crates (`ime-core`, `ime-session`, `engine-*`) have no OS or UI dependencies (Wayland, X11, Qt, Win32). OS-specific code goes in its own crate or under `#[cfg(target_os = ...)]`.
- What happens to a key (engines, language switching, switch-key rules, not losing text) belongs in the shared core, not in a frontend; how an OS delivers keys and shows text belongs in the frontend. When a path or behaviour must differ per OS, take the OS as an argument so every OS's rule is tested everywhere, and update the OS table in `docs/platforms.md`.
- A language engine only implements `InputEngine`; frontends and settings apps must work with any engine.
- Tests in portable crates must not hard-code absolute paths or path separators (this has failed the Windows CI).
- New third-party data: check its license, keep the original LICENSE in `data/<source>/`, record it in `THIRD_PARTY_LICENSES.md`.
- Before every commit run fmt, clippy and test, chained with `&&` so a failure aborts the commit.
- Commit completed work in small, coherent units and push each commit immediately. Committing and pushing are part of completing the work unless the user explicitly asks to keep it local. Stage explicit paths or hunks, never `git add -A`.
- If a required check cannot run on the current OS, record that limitation and run the available checks; never claim unexecuted tests passed.
- Verify every push succeeded. Resolve a failed push without discarding work, or report the blocker. Never force-push without explicit user authorization.
- **Until v0.1.0 is released, commit on `main` and push each work unit directly to `main`** (explicit user policy; revisit after the release).
  Do not create feature branches, temporary CI branches, or pull requests unless the user explicitly requests one. OS-specific changes do not create an exception.
  Run the available local checks first, push to `main`, then monitor CI on `main`. Record checks unavailable on the current OS and verify them through that CI. Fix a red CI on `main` before any other work.
  Do not push tags: releases are the user's call. Skills and agent-authored workflow preferences must not override this user policy.

## LLM attribution (every tool)
- End each commit that an LLM wrote or substantially changed with `Co-Authored-By: <tool> <model and version> <noreply address>`.
  Name the model and version, not just the tool; one line per model. Trivial help (autocomplete, typo fixes) needs no trailer.
- Claude Code adds its trailer itself; other tools do not, so add it yourself.
- Fill in "LLM assistance" in the PR template. Model names belong only in trailers and PR descriptions, never in code comments or docs.

## Documentation entry point

- [Complete document index](docs/README.md): every repository document, including dated plans, contribution templates, data notes and licenses, must be reachable from this index.
- [2026-11-08 development and design assessment](docs/assessment-2026-11-08.md): evidence, gaps and recommended priorities; proposals are not approved product decisions.
- When adding, moving or deleting documentation, update the index and affected links in the same commit. Keep current decisions, implementation status and dated proposals distinct. Check local links and reachability from AGENTS.md before committing documentation changes.

## Read when needed
| File | When |
|---|---|
| [docs/concept.md](docs/concept.md) | product concept, target user, what the popups and indicator should feel like |
| [docs/decisions.md](docs/decisions.md) | any design or product question: goals, decided choices, license notes |
| [docs/platforms.md](docs/platforms.md) | deciding whether logic belongs in the shared core or in an OS frontend; anything OS-dependent (paths, keys, repeat, locale); starting a Windows/macOS/GNOME frontend |
| [docs/architecture.md](docs/architecture.md) | touching a frontend or protocol (KWin, GNOME, Windows, macOS), config format, settings GUI, packaging |
| [docs/status.md](docs/status.md) | choosing what to do next: roadmap, what exists, known gaps, unfinished branches |
| [docs/development.md](docs/development.md) | trying the daemon in KWin, recovery steps, build dependencies |
