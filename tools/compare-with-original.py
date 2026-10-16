#!/usr/bin/env python3
"""Compare PDFCROP's output with the original Perl pdfcrop, page by page.

Run it on a folder of PDFs (no '-crop.pdf' files needed) with both tools
available; it crops every file with each tool and reports geometry and rendered
pixel differences.

    python tools/compare-with-original.py --folder /path/to/pdfs \
        --ours target/release/pdfcrop --reference "pdfcrop"

The reference defaults to `pdfcrop` from PATH (the Perl script, which needs
Ghostscript and a TeX engine). Requires PyMuPDF and NumPy:
`pip install pymupdf numpy`.

Exit code 0 when every page matches, 1 when differences were found, so it can be
wired into CI.
"""
from __future__ import annotations

import argparse
import os
import shlex
import shutil
import subprocess
import sys
import tempfile

import numpy as np
import pymupdf


def run(cmd: list[str], cwd: str | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)


def render(page: pymupdf.Page, max_px: float = 1400.0) -> np.ndarray:
    longest = max(page.rect.width, page.rect.height, 1.0)
    scale = min(max_px / longest, 3.0)
    pix = page.get_pixmap(matrix=pymupdf.Matrix(scale, scale), colorspace=pymupdf.csGRAY, alpha=False)
    return np.frombuffer(pix.samples, dtype=np.uint8).reshape(pix.height, pix.stride)[:, : pix.width].astype(np.int16)


def pixel_diff(a: np.ndarray, b: np.ndarray, trim: float = 0.02) -> tuple[float, float]:
    """Fraction of clearly-different pixels and mean difference, borders trimmed.

    Trimming avoids counting sub-pixel rasterisation differences at the page edge.
    """
    h, w = min(a.shape[0], b.shape[0]), min(a.shape[1], b.shape[1])
    ty, tx = int(h * trim), int(w * trim)
    a, b = a[ty : h - ty, tx : w - tx], b[ty : h - ty, tx : w - tx]
    if a.size == 0:
        return 0.0, 0.0
    d = np.abs(a - b)
    return float((d > 32).mean()), float(d.mean())


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--folder", required=True, help="folder holding the input PDFs")
    ap.add_argument("--ours", default="target/release/pdfcrop", help="PDFCROP executable")
    ap.add_argument("--reference", default="pdfcrop",
                    help="original pdfcrop command, optionally with arguments")
    ap.add_argument("--tolerance", type=float, default=1.5, help="allowed page-size difference in points")
    ap.add_argument("--keep", action="store_true", help="keep the generated files")
    args = ap.parse_args()

    ours = os.path.abspath(args.ours)
    if not os.path.isfile(ours):
        print(f"error: {ours} not found (build it first)", file=sys.stderr)
        return 2

    pdfs = sorted(
        os.path.join(args.folder, f)
        for f in os.listdir(args.folder)
        if f.lower().endswith(".pdf") and not f.lower().endswith("-crop.pdf")
    )
    if not pdfs:
        print("error: no PDFs in that folder", file=sys.stderr)
        return 2

    work = tempfile.mkdtemp(prefix="pdfcrop-compare-")
    failures = 0
    pages_checked = 0
    try:
        for src in pdfs:
            name = os.path.basename(src)
            ref_out = os.path.join(work, f"ref--{name}")
            our_out = os.path.join(work, f"ours--{name}")
            ref = run([*shlex.split(args.reference), src, ref_out])
            if ref.returncode != 0 or not os.path.exists(ref_out):
                print(f"{name}: reference failed ({ref.stdout[-200:]}{ref.stderr[-200:]}) - skipped")
                continue
            our = run([ours, src, our_out, "--quiet"])
            if our.returncode != 0 or not os.path.exists(our_out):
                print(f"{name}: PDFCROP FAILED: {our.stderr.strip()[:200]}")
                failures += 1
                continue

            rd, od = pymupdf.open(ref_out), pymupdf.open(our_out)
            if rd.page_count != od.page_count:
                print(f"{name}: page count differs: {rd.page_count} vs {od.page_count}")
                failures += 1
            size_bad, worst_frac, worst_page = 0, 0.0, None
            for i in range(min(rd.page_count, od.page_count)):
                rp, opi = rd[i], od[i]
                pages_checked += 1
                if (abs(rp.rect.width - opi.rect.width) > args.tolerance
                        or abs(rp.rect.height - opi.rect.height) > args.tolerance):
                    size_bad += 1
                    print(f"  page {i + 1}: size {rp.rect.width:.1f}x{rp.rect.height:.1f} vs "
                          f"{opi.rect.width:.1f}x{opi.rect.height:.1f}")
                frac, mean = pixel_diff(render(rp), render(opi))
                if frac > worst_frac:
                    worst_frac, worst_page = frac, i + 1
            verdict = "ok" if size_bad == 0 and worst_frac <= 0.002 else "DIFFERS"
            if verdict != "ok":
                failures += 1
            print(f"{name}: {verdict}  pages={rd.page_count} size_mismatch={size_bad} "
                  f"worst_pixels={worst_frac:.4%} (page {worst_page})")
            rd.close()
            od.close()
    finally:
        if args.keep:
            print(f"outputs kept in {work}")
        else:
            shutil.rmtree(work, ignore_errors=True)

    print(f"\n{len(pdfs)} files, {pages_checked} pages compared, {failures} differing")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
