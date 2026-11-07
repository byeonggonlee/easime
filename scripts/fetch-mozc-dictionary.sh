#!/bin/sh
# Download the Mozc OSS dictionary text files at a pinned commit.
#
# Usage: scripts/fetch-mozc-dictionary.sh DEST_DIR
#
# Every file is checked against a SHA-256 checksum recorded below (and in
# THIRD_PARTY_LICENSES.md). On any mismatch the script deletes the bad
# download, prints an error and exits non-zero without touching files that
# were already verified. Files in DEST_DIR that already match are not
# downloaded again.
#
# Build the easime dictionary from the result with:
#   cargo run --release -p easime-dict-tool -- build --out mozc.bin \
#       --mozc-commit "$MOZC_COMMIT" DEST_DIR
#
# Set MOZC_RAW_BASE to fetch from a mirror or a local file:// tree laid out
# like the Mozc repository (used by tests); checksums still apply.
#
# When you change the pinned commit, regenerate all checksums and update
# THIRD_PARTY_LICENSES.md and data/mozc/LICENSE together.

set -eu

MOZC_COMMIT=c4ed4683fb400b240e81d0a3895d024e3e21325e
BASE="${MOZC_RAW_BASE:-https://raw.githubusercontent.com/google/mozc/$MOZC_COMMIT}"

# "path-in-repo  sha256" pairs, one per line.
FILES='
src/data/dictionary_oss/dictionary00.txt 9e07ce292932c4fff327ed8cb06274c09acca556bd8bdf7c7ef1f6173eae3b61
src/data/dictionary_oss/dictionary01.txt 836fd5fd8399510de0d8ef7b33be913cca6bbaee7b712cc8045fba03f4c75a89
src/data/dictionary_oss/dictionary02.txt 93943858d0cfd092b849a78def6901eaf228177e1ac5663544e865a3a1e45a9e
src/data/dictionary_oss/dictionary03.txt cb9f1b0b4a7550d682d86244a021cd17e0b15366328819e69135b5f183a1a24b
src/data/dictionary_oss/dictionary04.txt 9fdc4e3948d03463de13ab5782adffbc11dd06e934cb80138c5920029cd3d2f6
src/data/dictionary_oss/dictionary05.txt 199e3b0f050b691823ae7ba943090d437f51af6436b27c5f3dc69cafa12000e4
src/data/dictionary_oss/dictionary06.txt b95b14f7770ae2a824bc8e9fad8b03c1bc45c504f93a36554b8b56daa0368d2a
src/data/dictionary_oss/dictionary07.txt 7f53649b529d4c8c495d3a8dd9c4d489dfe7696080f7423107ff01331174be48
src/data/dictionary_oss/dictionary08.txt 50a31bb5fc1646721b00be371a5695dffab1292bda143e1e59cc70cbbdd8643f
src/data/dictionary_oss/dictionary09.txt 4e5fcd4cbfbf11395166a37b506f547fc54d20edc0884f5184ecd3fc705f0d9a
LICENSE 44cdd923b91ea9199293abecc2762c70c87dbf1e581c027a94c416368d1a648c
'

if [ "$#" -ne 1 ] || [ -z "$1" ]; then
    echo "usage: $0 DEST_DIR" >&2
    exit 2
fi
dest=$1

fail() {
    echo "fetch-mozc-dictionary: $*" >&2
    exit 1
}

if command -v sha256sum >/dev/null 2>&1; then
    sha256_of() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then
    sha256_of() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
    fail "need sha256sum or shasum"
fi
command -v curl >/dev/null 2>&1 || fail "need curl"

mkdir -p "$dest"
tmp=$(mktemp -d "$dest/.fetch.XXXXXX") || fail "cannot create a temp dir in $dest"
trap 'rm -rf "$tmp"' EXIT INT TERM

# Phase 1: download and verify everything into the temp dir.
while read -r path sum; do
    [ -n "$path" ] || continue
    name=$(basename "$path")
    if [ -f "$dest/$name" ] && [ "$(sha256_of "$dest/$name")" = "$sum" ]; then
        echo "ok (already present) $name"
        continue
    fi
    echo "downloading $name"
    case "$BASE" in
        https://*) curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o "$tmp/$name" "$BASE/$path" ;;
        *) curl -fsSL --retry 3 -o "$tmp/$name" "$BASE/$path" ;;
    esac || fail "download failed: $BASE/$path"
    actual=$(sha256_of "$tmp/$name")
    if [ "$actual" != "$sum" ]; then
        rm -f "$tmp/$name"
        fail "checksum mismatch for $name: expected $sum, got $actual; refusing to use it"
    fi
    echo "ok (verified) $name"
done <<EOF
$FILES
EOF

# Phase 2: move the verified downloads into place.
for file in "$tmp"/*; do
    [ -f "$file" ] || continue
    mv -f "$file" "$dest/$(basename "$file")"
done

echo "Mozc dictionary files (commit $MOZC_COMMIT) are in $dest"
