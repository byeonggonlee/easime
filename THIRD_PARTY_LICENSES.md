# Third-party licenses

## input-method-unstable-v1.xml

- File: `crates/frontend-wayland/protocols/input-method-unstable-v1.xml`
- Source: Wayland/Weston `input-method-unstable-v1` protocol (copy obtained from
  https://github.com/pop-os/wayland-protocols, `unstable/input-method/`)
- License: MIT-style, reproduced below.

```
Copyright © 2012, 2013 Intel Corporation

Permission is hereby granted, free of charge, to any person obtaining a
copy of this software and associated documentation files (the "Software"),
to deal in the Software without restriction, including without limitation
the rights to use, copy, modify, merge, publish, distribute, sublicense,
and/or sell copies of the Software, and to permit persons to whom the
Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice (including the next
paragraph) shall be included in all copies or substantial portions of the
Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.  IN NO EVENT SHALL
THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
```

## Mozc OSS dictionary (kana to kanji conversion data)

- Files: `data/mozc/LICENSE`, `data/mozc/dictionary_oss-README.txt` (verbatim copies from Mozc)
- Source: https://github.com/google/mozc, directory `src/data/dictionary_oss/`,
  files `dictionary00.txt` ... `dictionary09.txt`
- Pinned commit: `c4ed4683fb400b240e81d0a3895d024e3e21325e` (2026-11-03). The data is **not**
  stored in this repository. `scripts/fetch-mozc-dictionary.sh` downloads exactly these files and
  refuses to continue unless their SHA-256 checksums match; release packages ship the
  converted binary dictionary `/usr/share/easime/dictionary/mozc.bin` built from them with
  `crates/easime-dict-tool`.
- Not used: `connection_single_column.txt`, `id.def`, `aux_dictionary.tsv`, the evaluation and
  filter files, and `src/data/dictionary_manual/`.
- The small test fixture `crates/easime-dict-tool/tests/fixtures/mozc-sample.txt` consists of a
  few lines of this data and is covered by the same notices.

Mozc's own `LICENSE` file has three parts, all of which apply to the dictionary (the second and
third are marked `Files: src/data/dictionary*`). Mozc's `src/data/dictionary_oss/README.txt` says
the OSS dictionary is "basically the same as that of IPAdic" (mecab-ipadic-2.7.0-20070801), is
enriched with the Okinawa Dictionary, plus entries added by Google (extra adjectives and verbs,
katakana words collected from Web text, compound words) which are covered by Mozc's BSD-3-Clause
license.

| Part | Applies to | Redistribution inside a MIT OR Apache-2.0 project's packages |
| --- | --- | --- |
| BSD-3-Clause, Google Inc. | Mozc code and Google-authored dictionary additions | Allowed. Binary redistribution must reproduce the copyright notice, conditions and disclaimer in the documentation; the name of Google or its contributors may not be used to endorse the product. |
| IPAdic / ICOT Free Software terms (NAIST) | Most dictionary entries | Allowed ("Use, reproduction, and distribution of this software is permitted"), for the original and for modified copies, provided that every copy includes the NAIST copyright notice and the paragraphs, including the ICOT "NO WARRANTY" section, and that distribution does not violate the laws of the countries involved. |
| Okinawa Dictionary | Named entities | Public domain; no restriction. |

None of these terms is copyleft or restricts the license of the rest of the project, so the
data can be shipped in packages of a MIT OR Apache-2.0 project as a separate data file that keeps
its own notices. What easime must do: (1) keep the three notices below with every package that
contains the converted dictionary (the `.deb` installs this file as
`/usr/share/doc/easime/THIRD_PARTY_LICENSES.md`, the `.rpm` as
`/usr/share/licenses/easime/THIRD_PARTY_LICENSES.md`); (2) never use the name of Google for
endorsement. The converted file also embeds a short attribution text. This is a summary of the
license texts as the project understood them, not legal advice.

### Mozc license, part 1 (BSD-3-Clause)

```
Copyright 2010-2018, Google Inc.
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

  * Redistributions of source code must retain the above copyright
    notice, this list of conditions and the following disclaimer.
  * Redistributions in binary form must reproduce the above
    copyright notice, this list of conditions and the following disclaimer
    in the documentation and/or other materials provided with the
    distribution.
  * Neither the name of Google Inc. nor the names of its
    contributors may be used to endorse or promote products derived from
    this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### Mozc license, part 2 (IPAdic and ICOT Free Software, applies to `src/data/dictionary*`)

```
Files: src/data/dictionary*

Copyright 2000, 2001, 2002, 2003 Nara Institute of Science
and Technology.  All Rights Reserved.

Use, reproduction, and distribution of this software is permitted.
Any copy of this software, whether in its original form or modified,
must include both the above copyright notice and the following
paragraphs.

Nara Institute of Science and Technology (NAIST),
the copyright holders, disclaims all warranties with regard to this
software, including all implied warranties of merchantability and
fitness, in no event shall NAIST be liable for
any special, indirect or consequential damages or any damages
whatsoever resulting from loss of use, data or profits, whether in an
action of contract, negligence or other tortuous action, arising out
of or in connection with the use or performance of this software.

A large portion of the dictionary entries
originate from ICOT Free Software.  The following conditions for ICOT
Free Software applies to the current dictionary as well.

Each User may also freely distribute the Program, whether in its
original form or modified, to any third party or parties, PROVIDED
that the provisions of Section 3 ("NO WARRANTY") will ALWAYS appear
on, or be attached to, the Program, which is distributed substantially
in the same form as set out herein and that such intended
distribution, if actually made, will neither violate or otherwise
contravene any of the laws and regulations of the countries having
jurisdiction over the User or the intended distribution itself.

NO WARRANTY

The program was produced on an experimental basis in the course of the
research and development conducted during the project and is provided
to users as so produced on an experimental basis.  Accordingly, the
program is provided without any warranty whatsoever, whether express,
implied, statutory or otherwise.  The term "warranty" used herein
includes, but is not limited to, any warranty of the quality,
performance, merchantability and fitness for a particular purpose of
the program and the nonexistence of any infringement or violation of
any right of any third party.

Each user of the program will agree and understand, and be deemed to
have agreed and understood, that there is no warranty whatsoever for
the program and, accordingly, the entire risk arising from or
otherwise connected with the program is assumed by the user.

Therefore, neither ICOT, the copyright holder, or any other
organization that participated in or was otherwise related to the
development of the program and their respective officials, directors,
officers and other employees shall be held liable for any and all
damages, including, without limitation, general, special, incidental
and consequential damages, arising out of or otherwise in connection
with the use or inability to use the program or any product, material
or result produced or otherwise obtained by using the program,
regardless of whether they have been advised of, or otherwise had
knowledge of, the possibility of such damages at any time during the
project or thereafter.  Each user will be deemed to have agreed to the
foregoing by his or her commencement of use of the program.  The term
"use" as used herein includes, but is not limited to, the use,
modification, copying and distribution of the program and the
production of secondary products from the program.

In the case where the program, whether in its original form or
modified, was distributed or delivered to or received by a user from
any person, organization or entity other than ICOT, unless it makes or
grants independently of ICOT any specific warranty to the user in
writing, such person, organization or entity, will also be exempted
from and not be held liable to the user for any such damages as noted
above as far as the program is concerned.
```

### Mozc license, part 3 (Okinawa Dictionary, applies to `src/data/dictionary*`)

```
Files: src/data/dictionary*

Public Domain Dataです。使用・変更・配布に関しては一切の制限をつけません。
商品などに組み込むことも自由に行なってください。すでにいくつかの辞書には沖縄辞書が採用されています。
勝手ながら、沖縄辞書に寄贈された辞書も in the Public Domain' 扱いとさせていただきます。
```

### SHA-256 checksums of the pinned files

```
9e07ce292932c4fff327ed8cb06274c09acca556bd8bdf7c7ef1f6173eae3b61  dictionary00.txt
836fd5fd8399510de0d8ef7b33be913cca6bbaee7b712cc8045fba03f4c75a89  dictionary01.txt
93943858d0cfd092b849a78def6901eaf228177e1ac5663544e865a3a1e45a9e  dictionary02.txt
cb9f1b0b4a7550d682d86244a021cd17e0b15366328819e69135b5f183a1a24b  dictionary03.txt
9fdc4e3948d03463de13ab5782adffbc11dd06e934cb80138c5920029cd3d2f6  dictionary04.txt
199e3b0f050b691823ae7ba943090d437f51af6436b27c5f3dc69cafa12000e4  dictionary05.txt
b95b14f7770ae2a824bc8e9fad8b03c1bc45c504f93a36554b8b56daa0368d2a  dictionary06.txt
7f53649b529d4c8c495d3a8dd9c4d489dfe7696080f7423107ff01331174be48  dictionary07.txt
50a31bb5fc1646721b00be371a5695dffab1292bda143e1e59cc70cbbdd8643f  dictionary08.txt
4e5fcd4cbfbf11395166a37b506f547fc54d20edc0884f5184ecd3fc705f0d9a  dictionary09.txt
```

## Korean Hanja dictionary data

Source: libhangul/libhangul, data/hanja/hanja.txt, commit `23d2cd9d10d516d523d60c562dbcc7ea08139a09`.

Only single-syllable rows are bundled as `data/hanja/single-syllable.txt` and embedded in the Korean engine. Original order, values and annotations are retained. The data file carries the following BSD-3-Clause notice independently of the upstream code. No libhangul code is linked. See `data/hanja/README.md` for extraction and checksums.

```
Copyright (c) 2005,2006 Choe Hwanjin
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice,
   this list of conditions and the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.
3. Neither the name of the author nor the names of its contributors
   may be used to endorse or promote products derived from this software
   without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE
LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
POSSIBILITY OF SUCH DAMAGE.
```
