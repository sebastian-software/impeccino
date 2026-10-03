# Notices

## Impeccable

Impeccino is derived from Impeccable, Copyright 2025 Paul Bakaus, licensed under the Apache License 2.0. The skill text, engine, detector rules, and tests started from Impeccable and have been modified: renamed, reduced to one universal skill folder, and stripped of the installer, packaging, browser tooling, and image comps (see `docs/adr/`).

**Original work:** https://github.com/pbakaus/impeccable

**Original license:** Apache-2.0
**Author:** Paul Bakaus

## Platform Design Skills

The `skill/reference/ios.md` and `skill/reference/android.md` files are distilled from ehmo's `platform-design-skills` (Apple Human Interface Guidelines and Material Design 3 rules), rewritten in Impeccino's voice.

**Original work:** https://github.com/ehmo/platform-design-skills

**Source revision:** [`dc2be825d8b439caea78e9eaa8fb3ac23b0ff3e9`](https://github.com/ehmo/platform-design-skills/tree/dc2be825d8b439caea78e9eaa8fb3ac23b0ff3e9)

**Original license:** MIT
**Author:** ehmo

The upstream MIT notice and permission terms are reproduced here for these skill references.

```text
MIT License

Copyright (c) 2026

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## css-tree

`crates/html/src/cascade/csstree/` is a Rust port of the css-tree 3.2.1 subset used by the static CSS cascade. The upstream source and its license are pinned to the `v3.2.1` release.

**Original work:** https://github.com/csstree/csstree/tree/v3.2.1

**Source revision:** [`8a6caba481be4cae4b0e8690af643ff8e59271f2`](https://github.com/csstree/csstree/tree/8a6caba481be4cae4b0e8690af643ff8e59271f2)

**License file:** https://github.com/csstree/csstree/blob/8a6caba481be4cae4b0e8690af643ff8e59271f2/LICENSE

**License:** MIT
**Copyright:** Copyright (C) 2016-2026 by Roman Dvornov

```text
Copyright (C) 2016-2026 by Roman Dvornov

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
```

## fdlibm and V8

`crates/foundation/src/fdlibm_trig.rs` ports the trigonometric routines from V8's `src/base/ieee754.cc`, which adapts fdlibm. The source file retains the upstream copyright notices; these license terms also apply to the port.

**V8 source revision:** [`63da331b5d924cce47aed3d7d35d72c57f0a8282`](https://github.com/v8/v8/tree/63da331b5d924cce47aed3d7d35d72c57f0a8282)

**Source file:** [`src/base/ieee754.cc`](https://github.com/v8/v8/blob/63da331b5d924cce47aed3d7d35d72c57f0a8282/src/base/ieee754.cc)

**V8 license:** [`LICENSE`](https://github.com/v8/v8/blob/63da331b5d924cce47aed3d7d35d72c57f0a8282/LICENSE)
**fdlibm license:** [`LICENSE.fdlibm`](https://github.com/v8/v8/blob/63da331b5d924cce47aed3d7d35d72c57f0a8282/LICENSE.fdlibm)

The fdlibm notice from V8's `LICENSE.fdlibm` is:

```text
Copyright (C) 1993-2004 by Sun Microsystems, Inc. All rights reserved.

Developed at SunSoft, a Sun Microsystems, Inc. business.

Permission to use, copy, modify, and distribute this
software is freely granted, provided that this notice
is preserved.
```

The V8 source header records Copyright 2016 the V8 project authors. The BSD-style terms in the V8 root `LICENSE` are:

```text
Copyright 2014, the V8 project authors. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

* Redistributions of source code must retain the above copyright
  notice, this list of conditions and the following disclaimer.

* Redistributions in binary form must reproduce the above
  copyright notice, this list of conditions and the following
  disclaimer in the documentation and/or other materials provided
  with the distribution.

* Neither the name of Google Inc. nor the names of its
  contributors may be used to endorse or promote products derived
  from this software without specific prior written permission.

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
