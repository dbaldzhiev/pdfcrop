# PDFCROP

Crops the margins of a PDF, per page, from the right-click menu of Windows
Explorer — **without Perl, without a TeX distribution, without Python and
without a pre-installed Ghostscript.**

It is a drop-in replacement for TeX Live's [`pdfcrop`](https://ctan.org/pkg/pdfcrop)
(Perl, v1.42) as used on the studio workstations, where the old chain needed
MiKTeX's `pdfcrop.exe` + Strawberry Perl + pdfTeX + `mgs.exe` to crop one file.

## How it works

The original tool does two things:

1. **measure** every page with Ghostscript's `bbox` device
   (`gs -sDEVICE=bbox ...`), parsing `%%BoundingBox` / `%%HiResBoundingBox`;
2. **write** a new PDF by embedding each page through **pdfTeX** with
   `\pdfpagewidth`/`\pdfpageheight` set to that page's box.

PDFCROP keeps step 1 exactly (that is why the numbers are identical) and replaces
step 2 with a lossless page-box rewrite: `/MediaBox` and `/CropBox` are set per
page, and nothing else in the file is touched. Consequences:

* no TeX engine is involved at all — the measurement is the whole dependency;
* a 1.5 KB file stays 1.5 KB (the TeX route re-encodes every page, often growing
  the file);
* links, outlines, annotations, form fields and metadata survive (pdfTeX drops
  them: in testing, a 15-page sheet lost 155 links and its 16 bookmarks, another
  file lost all 588 annotations);
* `/Rotate`d pages are measured in display space and mapped back to page space.
  The Perl version applies the display-space numbers to the unrotated page, which
  crops rotated documents at the wrong place.

## Install

Download `pdfcrop-setup-vX.Y.Z.exe` from the [latest release](../../releases) and
run it. That is the whole procedure — nothing is downloaded during installation.

* **Per-user by default** (`%LOCALAPPDATA%\Programs\pdfcrop`, no UAC prompt).
  Choose "install for all users" in the dialog, or pass `/ALLUSERS`, for
  `C:\Program Files\pdfcrop`.
* Adds **PDFCROP** to the right-click menu of `.pdf` files. On Windows 11 the
  classic menu is behind **Show more options** (Shift+F10) — Microsoft reserves
  the top-level menu for packaged apps.
* Appears in **Settings → Apps → Installed apps**, and uninstalling removes the
  menu entry again.
* Not code-signed yet: Windows may show *"Windows protected your PC"* on first
  run — choose **More info → Run anyway**.

Running it on a file writes `<name>-crop.pdf` next to the original and leaves the
original untouched.

## Command line

```
pdfcrop.exe [options] <input[.pdf]> [output file]
```

| Option | Meaning |
| --- | --- |
| `--margins "<left> <top> <right> <bottom>"` | add margins in bp (one value = all sides, two = left/right, top/bottom) |
| `--bbox "<left> <bottom> <right> <top>"` | override the measured box |
| `--bbox-odd`, `--bbox-even` | same, for odd/even pages only |
| `--hires` | use `%%HiResBoundingBox` (fractional points) instead of whole points |
| `--resolution <res>` | passed to Ghostscript as `-r…` |
| `--papersize <name>` | passed to Ghostscript as `-sPAPERSIZE=…` |
| `--gscmd <path>` | use a specific Ghostscript (otherwise the bundled one, then `PDFCROP_GS`, then `PATH`) |
| `--verbose`, `--quiet` | more / less output |
| `--dry-run` | print the computed boxes, write nothing |
| `--pause` | wait for Enter before closing (used by the menu entry) |
| `--clip`, `--pdftex`, `--xetex`, `--luatex`, `--ini`, `--pdfversion` | accepted for compatibility; see below |

`--clip` is accepted and has no visible effect: page boxes already clip what a
viewer shows, and PDFCROP deliberately does not delete the hidden content from
the streams (the Perl version does; the difference is that its output cannot be
"un-cropped" any more). The TeX-engine options only existed to choose a writer,
so they are ignored.

## Verification

Measured 2026-10-16 against the original tool, on the operator's own files
(`neb.tec`):

* **7/7 files, 37 pages — pixel-identical output.** The MiKTeX `pdfcrop.exe`
  (v1.40, Perl + pdfTeX + `mgs.exe`) and PDFCROP were run on the same inputs;
  every page matched in size and rendered pixels (0 % differing), with the
  bundled Ghostscript 10.08 *and* with MiKTeX's `mgs.exe` (their `bbox` numbers
  are identical).
* **372 archived `-crop.pdf` files** produced by the Perl tool over the years;
  the 18-pair sample that is still byte-comparable matches exactly. The
  mismatches are snapshots whose *source* was re-exported afterwards — a fresh
  run of the original tool on today's source agrees with PDFCROP.
* Faster: 1.7 s vs 6.1 s (1-page A0 sheet), 7.8 s vs 19.2 s (13-page set);
  the Perl version needs one pdfTeX run per page.
* Raster/"render to find the ink" bbox detection (what pure-Python and pure-Rust
  PDF croppers do) was measured against Ghostscript's device on 99 PDFs /
  260 pages: only ~87 % of pages agreed within 5 pt, worst case 381 pt. That is
  why Ghostscript's analytic `bbox` device is bundled rather than reimplemented.

## Building

CI (`.github/workflows/build.yml`) builds the exe with Rust on a Windows runner,
unpacks the official Ghostscript 10.08 installer (sha256-pinned), trims it to the
files the tool needs, compiles the Inno Setup installer and attaches it to the
release on a `v*` tag. Locally on Windows:

```powershell
cargo build --release
pwsh scripts\build.ps1 -LocalGhostscript C:\path\to\unpacked\ghostscript
```

Tests: `cargo test` (box maths, rotation mapping, Ghostscript output parsing,
page-order handling). `tools/compare-with-original.py` runs the Perl tool and
PDFCROP over a folder of PDFs and reports geometry and pixel differences.

## What is bundled

| Component | Licence | Why |
| --- | --- | --- |
| `pdfcrop.exe` (this repository) | MIT | the tool |
| Ghostscript 10.08 | AGPL v3 | page measurement, unmodified, `gs\COPYING` |

Nothing else — no Python, no Perl, no TeX, no VC++ redistributable. The
Ghostscript runtime is 42 MB on disk, 19.7 MB compressed inside the installer.

## Repository layout

```
src/            Rust tool (main.rs, gs.rs, pagebox.rs, writer.rs)
installer/      Inno Setup script + install notice
assets/         icon used for the exe, installer and menu entry
scripts/        local Windows build helper
legacy/         the original 2023 .reg/.bat hand-rolled setup, kept for reference
```
