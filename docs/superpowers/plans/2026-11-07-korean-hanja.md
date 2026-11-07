# Korean Hanja Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task. Preserve existing uncommitted design documents.

**Goal:** Convert one composing Hangul syllable to Hanja and record/investigate the KDE panel regression.

**Architecture:** Keep conversion in the portable Hangul engine. Reuse the existing single Wayland overlay for candidates and status, with separate display lifetimes.

**Tech Stack:** Rust, current InputEngine/CandidateList interfaces, existing bitmap fonts and Wayland input-panel protocol.

**Spec:** [Korean Hanja design](../specs/2026-11-07-korean-hanja-design.md), [DESIGN.md](../../../DESIGN.md).

## Global Constraints

- Only convert the currently composing syllable; preserve the original Composer on cancellation.
- Enter commits once and is consumed; no hidden candidate selection if the panel is unavailable.
- Preserve data attribution and do not depend on another IME runtime.
- Do not change desktop settings. Do not claim real KDE verification from fake compositor tests.

## Review Focus

- Esc followed by Backspace must edit the restored jamo state, not a flattened string.
- A missing dictionary entry must preserve the current syllable.
- More than nine candidates must have matching displayed numbers and selected candidates.
- Indicator expiry and configuration changes must not accidentally hide an active candidate list.
- Missing panel globals must not allow invisible conversion or consume normal application shortcuts.

## Task 1: Portable conversion

Files: `crates/ime-core/src/key.rs`, `crates/engine-hangul/src/{lib,engine,hanja}.rs`, `data/hanja/`, `THIRD_PARTY_LICENSES.md`.

- [x] Add behavioral tests for open, move, commit, cancel, resumed editing, no matching candidates and multiple pages; observe failure.
- [x] Add `Key::Hanja` and page keys; add a pinned, attributed Hanja dataset and lookup function.
- [x] Extend HangulEngine with conversion state while retaining Composer; pass portable fmt, clippy and tests.

## Task 2: Candidate overlay

Files: `crates/frontend-wayland/src/{app,panel,candidate,translate}.rs`, `crates/frontend-wayland/tests/{flow,indicator}.rs`.

- [x] Add translation and integration tests for the Hanja key, visible candidates, Enter suppression, Esc restoration, missing globals and indicator expiry. Linux runtime execution remains pending below.
- [x] Draw numbered candidate pages in the existing panel, add explicit indicator/candidate visibility tracking, and wire candidates to engine output.
- [ ] Check Linux targets and run Linux tests when a Linux execution environment is available; report limitations.

## Task 3: Real-use feedback and final verification

Files: `DESIGN.md`, `README.md`, `AGENTS.md`, `docs/status.md`, `docs/decisions.md`, `docs/concept.md`.

- [x] Record the user's KDE test commit and success, open taskbar report, current Hanja behavior and remaining verification.
- [x] Investigate panel movement against the user's exact symptom; only fix a supported cause.
- [x] Run portable tests, Linux compile checks, formatting and diff checks. Review the final changes.

## Execution record

Portable baseline passed before implementation. Hanja RED: 8 behavior failures, 1 idle-key test passed; GREEN: 12 Hanja tests plus portable suite passed. Linux target compile/clippy passed; actual Linux integration execution remains unchecked. Candidate painter and translation tests executed through a temporary Mac harness.

Review: one P2 Japanese control mismatch was addressed by limiting new UI wiring to Korean; no other actionable findings. Taskbar investigation identified upstream bug 527067. The user confirmed DBus visible toggling with the indicator, and supplied video shows the panel's bottom floating margin disappearing and returning with the box. After rebooting to apply KWin 6.8.2, the user confirmed the taskbar issue no longer reproduced without reinstalling easime. Post-reboot DBus visibility was not measured. No easime panel workaround was added. The old installed easime still shows the focus popup; installing and verifying the local focus fix and Hanja feature remain pending. No desktop settings were changed. This record describes local verification before committing and remote CI; integration follows the repository commit/push policy.

Host verification limitation: native Mac cargo test --workspace cannot compile the existing Linux-only memfd_create/timerfd/inotify imports. Linux-target all-target compilation and clippy passed instead; Linux tests were not executed.
