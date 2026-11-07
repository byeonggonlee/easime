#!/usr/bin/env python3
"""Static checks of the settings window's translations, no Qt needed.

* every text passed to Tr.t() in qml/*.qml is in the table of qml/tr.js,
* every entry of the table is used (a changed Korean text must not leave a
  dead entry behind),
* every entry has all four translations, and %1 appears as often as in the
  Korean text,
* nothing still goes through qsTr() (it would not be translated).

Run from anywhere: python3 settings/kde/tests/check_translations.py
"""
import json
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parent.parent
qml = sorted((root / "qml").glob("*.qml"))
source = (root / "qml" / "tr.js").read_text(encoding="utf-8")

block = source.split("// BEGIN TABLE", 1)[1].split("// END TABLE", 1)[0]
block = block.strip()
assert block.startswith("var table = ") and block.endswith(";"), "table block not found"
table = json.loads(block[len("var table = "):-1])

languages = re.search(r'var languages = (\[.*?\])', source).group(1)
languages = json.loads(languages)
translated = [l for l in languages if l != "ko"]

used = set()
problems = []
for file in qml:
    text = file.read_text(encoding="utf-8")
    if "qsTr(" in text:
        problems.append(f"{file.name}: qsTr() is not translated, use Tr.t()")
    for match in re.finditer(r'Tr\.t\("((?:[^"\\]|\\.)*)"\)', text):
        used.add(json.loads('"' + match.group(1) + '"'))

for text in sorted(used - set(table)):
    problems.append(f"not in the table: {text!r}")
for text in sorted(set(table) - used):
    problems.append(f"in the table but not used: {text!r}")
for text, entry in table.items():
    for language in translated:
        if not entry.get(language):
            problems.append(f"{language} is empty for {text!r}")
        elif entry[language].count("%1") != text.count("%1"):
            problems.append(f"%1 differs in {language} for {text!r}")
    extra = set(entry) - set(translated)
    if extra:
        problems.append(f"unknown languages {sorted(extra)} for {text!r}")

if problems:
    print("\n".join(problems))
    sys.exit(1)
print(f"{len(used)} texts, {len(translated)} translations each: ok")
