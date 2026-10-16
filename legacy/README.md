# The original hand-rolled setup (2023)

These three files are the previous way PDFCROP was deployed, kept for reference
and for machines that were set up by hand before the installer existed.

| File | What it did |
| --- | --- |
| `context-reg.reg` | wrote `HKEY_CLASSES_ROOT\SystemFileAssociations\.pdf\shell\Item0` — a machine-wide right-click entry named `PDFCROP`, top position, icon `C:\pdfcrop\pdfcorp.ico`, running `C:\pdfcrop\pdcrop.bat "%1"` |
| `pdcrop.bat` | `cd`-ed into the file's folder and ran `pdfcrop.exe <name> --verbose`, then `pause` |
| `update.bat` | `git pull` in `C:\pdfcrop` |

It relied on a `pdfcrop.exe` that was not in this repository: the one MiKTeX
installs (`%LOCALAPPDATA%\Programs\MiKTeX\miktex\bin\x64\pdfcrop.exe`), i.e. the
Perl script plus pdfTeX plus Strawberry Perl plus MiKTeX's Ghostscript.

The current setup replaces all of it: `pdfcrop-setup-vX.Y.Z.exe` installs the
tool, a bundled Ghostscript and the same menu entry (as `PDFCROP`, in the
`.pdf` shell menu) per user, and uninstalls cleanly.

## Removing a machine set up by hand

Installing over the old layout leaves two entries in the menu (the old `Item0`
key and the new `PDFCROP` key). The installer removes `Item0` itself when it runs
elevated; otherwise delete it once from an administrator prompt:

```bat
reg delete "HKCR\SystemFileAssociations\.pdf\shell\Item0" /f
```

or import `remove-menu-entry.reg` (double-click, accept the UAC prompt).
