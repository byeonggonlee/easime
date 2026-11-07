#!/bin/sh
# Build the dictionary file that the .deb and .rpm install as
# /usr/share/easime/dictionary/mozc.bin. The output is always
# target/dictionary/mozc.bin (the path the package metadata points at).
#
# Usage:
#   scripts/build-dictionary.sh sample   # tiny dictionary from the committed
#                                        # fixture; no download (quick local
#                                        # tries only: it cannot convert real
#                                        # words, so never package it)
#   scripts/build-dictionary.sh mozc [SRC_DIR]
#                                        # real dictionary: fetches the pinned
#                                        # Mozc files (checksum verified) into
#                                        # SRC_DIR (default target/mozc-src)
#                                        # and converts them (used by the CI
#                                        # and the release workflow)
#
# Run from the repository root.

set -eu

mode=${1:-}
out=target/dictionary/mozc.bin
commit=$(sed -n 's/^MOZC_COMMIT=//p' scripts/fetch-mozc-dictionary.sh)

mkdir -p target/dictionary
case "$mode" in
    sample)
        cargo run --release --locked -p easime-dict-tool -- build --out "$out" \
            --mozc-commit "$commit" crates/easime-dict-tool/tests/fixtures/mozc-sample.txt
        ;;
    mozc)
        src=${2:-target/mozc-src}
        sh scripts/fetch-mozc-dictionary.sh "$src"
        cargo run --release --locked -p easime-dict-tool -- build --out "$out" \
            --mozc-commit "$commit" "$src"
        ;;
    *)
        echo "usage: $0 sample|mozc [SRC_DIR]" >&2
        exit 2
        ;;
esac
ls -l "$out"
