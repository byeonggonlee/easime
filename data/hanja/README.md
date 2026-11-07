# Korean Hanja data

Source: https://github.com/libhangul/libhangul/blob/23d2cd9d10d516d523d60c562dbcc7ea08139a09/data/hanja/hanja.txt

Pinned commit: `23d2cd9d10d516d523d60c562dbcc7ea08139a09`.

Upstream SHA-256: `b1004034589f1357daaea3534a6136f6b5ef825afa8779886b20f0b7908bbe3b`.

`single-syllable.txt` retains the upstream file header and, in original order, only rows whose reading is one precomposed Hangul syllable (U+AC00–U+D7A3) and whose value is one Unicode scalar. No readings, values, or annotations were rewritten. Format: `reading:value:annotation`.

The data file has its own BSD-3-Clause notice; see [LICENSE](LICENSE). No libhangul code is included or linked. Binary packages reproduce this notice in `THIRD_PARTY_LICENSES.md`.
