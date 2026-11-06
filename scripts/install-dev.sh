#!/usr/bin/env bash
# Build easime and register it as a KDE virtual keyboard for the current user.
# Afterwards select it in: System Settings > Keyboard > Virtual Keyboard,
# then log out and back in (or restart KWin) for it to take effect.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cargo build --release --manifest-path "$root/Cargo.toml" -p easime

apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
mkdir -p "$apps"
sed "s|^Exec=.*|Exec=$root/target/release/easime|" "$root/data/easime.desktop" \
    > "$apps/easime.desktop"
echo "Installed $apps/easime.desktop -> $root/target/release/easime"
echo "Log file: ${XDG_STATE_HOME:-$HOME/.local/state}/easime/easime.log"
