#!/usr/bin/env bash
# Read-only diagnosis of overlay panels being reported as virtual keyboards.
set -euo pipefail

case "${1:-}" in
    ""|--watch) ;;
    *) echo "Usage: bash scripts/diagnose-kde-panel.sh [--watch]" >&2; exit 2 ;;
esac

echo "Installed packages:"
if command -v rpm >/dev/null 2>&1; then
    rpm -q kwin plasma-workspace || true
fi

echo "Running KWin:"
if command -v qdbus6 >/dev/null 2>&1; then
    qdbus6 org.kde.KWin /KWin supportInformation | sed -n '/^KWin version:/p'
elif command -v busctl >/dev/null 2>&1; then
    # Print only the running version, not the complete hardware/config report.
    busctl --user call org.kde.KWin /KWin org.kde.KWin supportInformation |
        python3 -c 'import re,sys; m=re.search(r"KWin version: ([^\\]+)", sys.stdin.read()); print(m.group(0) if m else "Running version unavailable")'
else
    echo "qdbus6 or busctl is required to query the running session."
fi

if [[ "${1:-}" != "--watch" ]]; then
    echo "Run with --watch to observe virtual-keyboard visibility while switching languages."
    exit 0
fi

if ! command -v busctl >/dev/null 2>&1; then
    echo "busctl is required for --watch." >&2
    exit 1
fi

echo "Switch languages in a text field during the next 12 seconds."
echo "An overlay should leave visible=false. true/false changes matching the box suggest KDE bug 527067."
previous=""
for ((sample = 0; sample < 60; sample++)); do
    value="$(busctl --user get-property org.kde.KWin /VirtualKeyboard org.kde.kwin.VirtualKeyboard visible)"
    if [[ "$value" != "$previous" ]]; then
        echo "$value"
        previous="$value"
    fi
    sleep 0.2
done
