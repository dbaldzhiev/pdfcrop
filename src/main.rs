//! pdfcrop — crop PDF margins per page.
//!
//! A drop-in replacement for TeX Live's `pdfcrop` (Perl) that keeps the original
//! measurement pipeline (Ghostscript's `bbox` device) and replaces the writer
//! (pdfTeX/XeTeX/LuaTeX) with a lossless page-box rewrite:
//!
//!   * no Perl, no TeX — the only bundled third-party component is Ghostscript,
//!   * links, outlines, annotations, metadata and vector content are preserved
//!     untouched instead of being re-embedded by a TeX engine,
//!   * `/Rotate`d pages are measured in display space and mapped back to page
//!     space (the original tool applies display-space numbers to the unrotated
//!     page, which crops rotated documents incorrectly).

mod gs;
mod pagebox;
mod writer;

use pagebox::{Box4, Margins};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const PROGRAM: &str = "pdfcrop";
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let opts = match Options::parse(&argv) {
        Ok(ParseResult::Help) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Ok(ParseResult::Version) => {
            println!("{PROGRAM} {VERSION} (Ghostscript measurement, no Perl, no TeX)");
            return ExitCode::SUCCESS;
        }
        Ok(ParseResult::Run(o)) => o,
        Err(msg) => {
            eprintln!("!!! Error {msg}");
            eprintln!("Try `{PROGRAM} --help`.");
            return ExitCode::from(1);
        }
    };

    match run(&opts) {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("!!! Error {msg}");
            wait_for_key(opts.pause, false);
            ExitCode::from(2)
        }
    }
}

fn run(opts: &Options) -> Result<ExitCode, String> {
    if !opts.quiet {
        println!("{PROGRAM} {VERSION} - crops margins per page (Ghostscript measurement, no Perl, no TeX)");
    }
    if opts.verbose {
        eprintln!("* Input file: {}", opts.input.display());
        eprintln!("* Output file: {}", opts.output.display());
        eprintln!(
            "* Margins: {} {} {} {}",
            opts.margins.left, opts.margins.top, opts.margins.right, opts.margins.bottom
        );
    }

    let gs = gs::find(opts.gs_cmd.as_deref(), opts.verbose)
        .map_err(|e| format!("Ghostscript not found: {e}"))?;
    if opts.verbose {
        eprintln!("* Ghostscript: {}", gs.display());
    }

    // --- measurement -----------------------------------------------------
    let measured = if opts.bbox.is_some() && opts.bbox_odd.is_none() && opts.bbox_even.is_none() {
        Vec::new() // fixed box: no need to run Ghostscript at all
    } else {
        if opts.verbose {
            eprintln!("* Running Ghostscript for BoundingBox calculation ...");
        }
        gs::measure(&gs, &opts.input, opts.hires, opts.resolution.as_deref(), opts.papersize.as_deref())
            .map_err(|e| format!("bounding box calculation failed: {e}"))?
    };

    // --- page geometry ---------------------------------------------------
    let mut doc = writer::Document::open(&opts.input)?;
    let page_count = doc.page_count();
    if opts.verbose {
        eprintln!("* Pages: {page_count}");
    }
    if measured.is_empty() && opts.bbox.is_none() {
        return Err("Ghostscript does not report bounding boxes".into());
    }

    let mut cropped = 0usize;
    let mut kept: Vec<usize> = Vec::new(); // pages kept at original size
    let mut plan: Vec<(usize, Option<Box4>)> = Vec::with_capacity(page_count);

    for index in 1..=page_count {
        let media = doc.media_box(index)?;
        let rotate = doc.rotate(index)?;

        // pdfcrop semantics: an explicit --bbox wins for every page, but the
        // odd/even variants only apply to their own pages.
        let base = if index % 2 == 1 {
            opts.bbox_odd.or(opts.bbox)
        } else {
            opts.bbox_even.or(opts.bbox)
        }
        .or_else(|| measured.get(index - 1).copied());

        let Some(base) = base else {
            return Err(format!(
                "page {index}: Ghostscript reported no bounding box and no --bbox was given"
            ));
        };

        let display = pagebox::apply_margins(base, &opts.margins);

        // Empty box: keep the page at its original size (pdfcrop does the same).
        if pagebox::is_empty(&display) {
            eprintln!(
                "\n!!! Warning: Empty Bounding Box!\n!!!   Page {index}: {} {} {} {}\n!!! Recovery is tried by embedding the page in its original size.\n",
                display.x0, display.y0, display.x1, display.y1
            );
            kept.push(index);
            plan.push((index, None));
            continue;
        }

        let page_box = pagebox::display_to_page_space(display, &media, rotate);
        if opts.verbose {
            eprintln!(
                "* Page {index}: {} {} {} {} (display) -> {} {} {} {} (page, rotate {})",
                fmt(display.x0, opts.hires),
                fmt(display.y0, opts.hires),
                fmt(display.x1, opts.hires),
                fmt(display.y1, opts.hires),
                fmt(page_box.x0, opts.hires),
                fmt(page_box.y0, opts.hires),
                fmt(page_box.x1, opts.hires),
                fmt(page_box.y1, opts.hires),
                rotate
            );
        }
        plan.push((index, Some(page_box)));
        cropped += 1;
    }

    if opts.dry_run {
        if opts.pause {
            println!();
        }
        for (index, b) in &plan {
            match b {
                Some(b) => println!(
                    "{index}\t{}\t{}\t{}\t{}",
                    fmt(b.x0, opts.hires),
                    fmt(b.y0, opts.hires),
                    fmt(b.x1, opts.hires),
                    fmt(b.y1, opts.hires)
                ),
                None => println!("{index}\toriginal"),
            }
        }
        return Ok(ExitCode::SUCCESS);
    }

    for (index, b) in &plan {
        if let Some(b) = b {
            doc.set_page_box(*index, *b, opts.hires)?;
        }
    }

    if page_count == 0 {
        return Err("input has no pages".into());
    }
    doc.save(&opts.output)?;

    if !opts.quiet {
        println!(
            "==> {cropped} page{} cropped, {} kept at original size, written on `{}'.",
            if cropped == 1 { "" } else { "s" },
            kept.len(),
            opts.output.display()
        );
    }
    wait_for_key(opts.pause, opts.quiet);
    Ok(ExitCode::SUCCESS)
}

/// Format a coordinate the way pdfcrop does: integer boxes by default (they are
/// what `%%BoundingBox` reports), reals only with `--hires`.
fn fmt(v: f64, hires: bool) -> String {
    if hires {
        format!("{v:.5}")
    } else {
        format!("{}", v.round() as i64)
    }
}

/// Keep the console window open when launched from the Explorer context menu.
fn wait_for_key(pause: bool, quiet: bool) {
    if !pause {
        return;
    }
    if quiet {
        println!();
    }
    print!("Press Enter to close ...");
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
}

struct Options {
    input: PathBuf,
    output: PathBuf,
    margins: Margins,
    bbox: Option<Box4>,
    bbox_odd: Option<Box4>,
    bbox_even: Option<Box4>,
    hires: bool,
    resolution: Option<String>,
    papersize: Option<String>,
    gs_cmd: Option<String>,
    verbose: bool,
    quiet: bool,
    dry_run: bool,
    pause: bool,
}

enum ParseResult {
    Help,
    Version,
    Run(Options),
}

impl Options {
    fn parse(argv: &[String]) -> Result<ParseResult, String> {
        let mut input: Option<String> = None;
        let mut output: Option<String> = None;
        let mut margins = Margins::default();
        let mut bbox = None;
        let mut bbox_odd = None;
        let mut bbox_even = None;
        let mut hires = false;
        let mut resolution = None;
        let mut papersize = None;
        let mut gs_cmd = None;
        let mut verbose = false;
        let mut quiet = false;
        let mut dry_run = false;
        let mut pause = false;

        let mut i = 0;
        while i < argv.len() {
            let a = argv[i].as_str();
            let value = |i: &mut usize| -> Result<String, String> {
                *i += 1;
                argv.get(*i)
                    .cloned()
                    .ok_or_else(|| format!("Missing value for option `{a}'"))
            };
            match a {
                "--help" | "-h" => return Ok(ParseResult::Help),
                "--version" => return Ok(ParseResult::Version),
                "--verbose" | "--debug" => {
                    verbose = true;
                    if a == "--debug" {
                        quiet = false;
                    }
                }
                "--no-verbose" | "--no-debug" => verbose = false,
                "--quiet" => quiet = true,
                "--no-quiet" => quiet = false,
                "--hires" => hires = true,
                "--no-hires" => hires = false,
                // Accepted for compatibility. Our page boxes clip the page just
                // like the original's \pageclip does; we deliberately do not
                // rewrite content streams to physically delete the hidden data.
                "--clip" | "--no-clip" => {}
                "--dry-run" => dry_run = true,
                // Explorer launches a console program with no console of its own,
                // so the window would vanish before anything could be read.
                "--pause" => pause = true,
                "--margins" => margins = Margins::parse(&value(&mut i)?)?,
                "--bbox" => bbox = Some(Box4::parse(&value(&mut i)?, "--bbox")?),
                "--bbox-odd" => bbox_odd = Some(Box4::parse(&value(&mut i)?, "--bbox-odd")?),
                "--bbox-even" => bbox_even = Some(Box4::parse(&value(&mut i)?, "--bbox-even")?),
                "--resolution" => resolution = Some(value(&mut i)?),
                "--papersize" => papersize = Some(value(&mut i)?),
                "--gscmd" => gs_cmd = Some(value(&mut i)?),
                // pdfcrop options that only exist to select a TeX engine: the
                // writer is gone, so they are accepted and ignored.
                "--pdftex" | "--xetex" | "--luatex" | "--no-ini" | "--ini" => {}
                "--pdftexcmd" | "--xetexcmd" | "--luatexcmd" | "--pdfversion" | "--uncompress" => {
                    let _ = value(&mut i)?;
                }
                other if other.starts_with("--") => {
                    return Err(format!("Unknown option `{other}'"))
                }
                other => {
                    if input.is_none() {
                        input = Some(other.to_string());
                    } else if output.is_none() {
                        output = Some(other.to_string());
                    } else {
                        return Err(format!("Unexpected argument `{other}'"));
                    }
                }
            }
            i += 1;
        }

        let mut input = PathBuf::from(input.ok_or("Missing input file name")?);
        if !input.is_file() {
            let with_ext = PathBuf::from(format!("{}.pdf", input.display()));
            if with_ext.is_file() {
                input = with_ext;
            } else {
                return Err(format!("Input file `{}' not found", input.display()));
            }
        }

        let output = match output {
            Some(o) => PathBuf::from(o),
            None => {
                let stem = input
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "output".into());
                let dir = input.parent().unwrap_or_else(|| Path::new("."));
                dir.join(format!("{stem}-crop.pdf"))
            }
        };

        Ok(ParseResult::Run(Options {
            input,
            output,
            margins,
            bbox,
            bbox_odd,
            bbox_even,
            hires,
            resolution,
            papersize,
            gs_cmd,
            verbose,
            quiet,
            dry_run,
            pause,
        }))
    }
}

const USAGE: &str = "\
pdfcrop — crop PDF margins per page (Ghostscript measurement, no Perl, no TeX)

Syntax:   pdfcrop [options] <input[.pdf]> [output file]
Function: Margins are calculated and removed for each page in the file.
          The default output file is `<input>-crop.pdf' next to the input.

Options:
  --help              print usage
  --version           print version number
  --verbose           verbose printing
  --quiet             silence normal output
  --margins \"<left> <top> <right> <bottom>\"
                      add extra margins, unit is bp; one number applies to all
                      margins, two numbers give (left/right, top/bottom)
  --bbox \"<left> <bottom> <right> <top>\"
                      override the bounding box Ghostscript found for every page
  --bbox-odd, --bbox-even
                      the same, but for odd or even pages only
  --hires             use Ghostscript's HiResBoundingBox (fractional points)
  --clip              accepted for compatibility; page boxes already clip the
                      visible area, content streams are left untouched
  --resolution <res>  pass Ghostscript's option -r (e.g. 72 or 300x300)
  --papersize <name>  pass Ghostscript's option -sPAPERSIZE=<name>
  --gscmd <name>      Ghostscript executable to use (default: the bundled
                      Ghostscript, else PDFCROP_GS, else one from PATH)
  --dry-run           print the computed page boxes instead of writing a file
  --pause             wait for Enter before exiting (used by the right-click menu,
                      which otherwise closes the console window instantly)

Input file: `-` is not supported; pass a file path.
";
