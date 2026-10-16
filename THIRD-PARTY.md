# Third-party components

## Ghostscript 10.08 — AGPL v3

PDFCROP ships an unmodified subset of the official Ghostscript 10.08 Windows
build (release tag `gs10080`) and runs it as a separate process with
`-sDEVICE=bbox` to measure each page. The bundled files are:

```
gs\bin\gswin64c.exe, gs\bin\gsdll64.dll
gs\lib\**            gs\Resource\**            gs\iccprofiles\**
```

The licence text is installed as `gs\COPYING`. Source code for this version:
<https://github.com/ArtifexSoftware/ghostpdl/releases/tag/gs10080>.

The installer integrates Ghostscript by executing it (a "mere aggregation"); the
tool itself does not link against it. Ghostscript is a trademark of Artifex
Software, Inc.

## Rust crates

`lopdf` (MIT) is the only runtime crate; the full dependency tree and its
licences are listed in `Cargo.lock` and can be reproduced with
`cargo license` or `cargo about`.
