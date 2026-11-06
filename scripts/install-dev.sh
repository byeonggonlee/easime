#!/usr/bin/env bash
# Build easime and register it as a KDE virtual keyboard for the current user.
# Afterwards select it in: System Settings > Keyboard > Virtual Keyboard,
# then log out and back in (or restart KWin) for it to take effect.
#
# Set EASIME_SKIP_BUILD=1 to only (re)write the .desktop file.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
bin="$root/target/release/easime"

# These characters cannot be expressed safely in a Desktop Entry Exec= value
# without double escaping, so refuse them instead of writing a broken file.
case "$bin" in
    *[\"\`\$\\%]* | *$'\n'*)
        echo "error: the repository path contains one of \" \` \$ \\ % or a newline: $root" >&2
        exit 1
        ;;
esac

if [[ -z "${EASIME_SKIP_BUILD:-}" ]]; then
    cargo build --release --manifest-path "$root/Cargo.toml" -p easime
fi

exec_value="$bin"
if [[ "$bin" == *[[:space:]]* ]]; then
    exec_value="\"$bin\""
fi

apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
mkdir -p "$apps"
tmp="$(mktemp "$apps/.easime.desktop.XXXXXX")"
trap 'rm -f "$tmp"' EXIT
awk -v exec_value="$exec_value" '/^Exec=/ { print "Exec=" exec_value; next } { print }' \
    "$root/data/easime.desktop" > "$tmp"
chmod 644 "$tmp"
mv "$tmp" "$apps/easime.desktop"
trap - EXIT

echo "Installed $apps/easime.desktop -> $bin"
echo "Log file: ${XDG_STATE_HOME:-$HOME/.local/state}/easime/easime.log"
