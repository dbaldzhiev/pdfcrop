//! Ghostscript discovery and bounding-box measurement.
//!
//! Measurement is deliberately the *same* pipeline the original Perl `pdfcrop`
//! uses: Ghostscript's `bbox` device with `-dSAFER -dBATCH -dNOPAUSE -c "save
//! pop" -f <file>`, parsing the `%%BoundingBox` / `%%HiResBoundingBox` lines it
//! prints (Ghostscript writes them to stderr).

use crate::pagebox::Box4;
use std::path::{Path, PathBuf};
use std::process::Command;

const WINDOWS_CANDIDATES: [&str; 4] = ["gswin64c.exe", "gswin64.exe", "gs.exe", "mgs.exe"];
const UNIX_CANDIDATES: [&str; 3] = ["gs", "gswin64c", "gswin64c.exe"];

/// Locate Ghostscript: an explicit `--gscmd`, `PDFCROP_GS`, the copy bundled
/// next to this executable, then `PATH`.
pub fn find(explicit: Option<&str>, verbose: bool) -> Result<PathBuf, String> {
    if let Some(cmd) = explicit {
        let p = PathBuf::from(cmd);
        return if p.is_file() {
            Ok(p)
        } else {
            Err(format!("`{cmd}' is not a file"))
        };
    }

    if let Some(env) = std::env::var_os("PDFCROP_GS") {
        let p = PathBuf::from(env);
        if p.is_file() {
            if verbose {
                eprintln!("* Ghostscript from PDFCROP_GS: {}", p.display());
            }
            return Ok(p);
        }
        if verbose {
            eprintln!("* PDFCROP_GS does not point at a file: {}", p.display());
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        let exe = exe.canonicalize().unwrap_or(exe);
        if let Some(dir) = exe.parent() {
            // Bundled layout: <app>\gs\bin\gswin64c.exe (or a flat <app>\gs).
            let roots = [dir.join("gs"), dir.join("ghostscript"), dir.to_path_buf()];
            for root in roots {
                for sub in [root.join("bin"), root.clone()] {
                    for name in candidates() {
                        let candidate = sub.join(name);
                        if candidate.is_file() {
                            if verbose {
                                eprintln!("* Ghostscript (bundled): {}", candidate.display());
                            }
                            return Ok(candidate);
                        }
                    }
                }
            }
        }
    }

    for name in candidates() {
        if let Some(found) = which(name) {
            if verbose {
                eprintln!("* Ghostscript (PATH): {}", found.display());
            }
            return Ok(found);
        }
    }

    Err(format!(
        "no Ghostscript found (looked for {} next to this executable and on PATH; \
         set PDFCROP_GS or pass --gscmd)",
        candidates().join(", ")
    ))
}

fn candidates() -> Vec<&'static str> {
    let mut v = Vec::new();
    if cfg!(windows) {
        v.extend_from_slice(&WINDOWS_CANDIDATES);
    } else {
        v.extend_from_slice(&UNIX_CANDIDATES);
    }
    v
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Environment that makes a bundled Ghostscript find its own `lib`, `Resource`
/// and `iccprofiles` directories no matter where the installer put it.
///
/// `GS_LIB` *replaces* Ghostscript's search path, so every directory it needs is
/// listed explicitly (each `Resource/*` subdirectory, `lib`, `iccprofiles`) —
/// setting only a subset would break CMaps, font substitution or colour
/// management.
fn gs_env(gs: &Path) -> Vec<(String, String)> {
    let mut extra: Vec<String> = Vec::new();
    if let Some(bin) = gs.parent() {
        if let Some(root) = bin.parent() {
            extra.push(root.join("lib").display().to_string());
            extra.push(root.join("iccprofiles").display().to_string());
            let resource = root.join("Resource");
            if let Ok(entries) = std::fs::read_dir(&resource) {
                let mut subs: Vec<String> = entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .map(|p| p.display().to_string())
                    .collect();
                subs.sort();
                extra.extend(subs);
            } else if resource.is_dir() {
                extra.push(resource.display().to_string());
            }
        }
    }
    extra.retain(|p| Path::new(p).is_dir());
    if extra.is_empty() {
        return Vec::new();
    }
    if let Some(existing) = std::env::var_os("GS_LIB") {
        extra.push(existing.to_string_lossy().to_string());
    }
    vec![("GS_LIB".to_string(), extra.join(path_separator()))]
}

fn path_separator() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

/// Run the `bbox` device and return one box per page, in page order.
pub fn measure(
    gs: &Path,
    input: &Path,
    hires: bool,
    resolution: Option<&str>,
    papersize: Option<&str>,
) -> Result<Vec<Box4>, String> {
    if let Some(r) = resolution {
        if !r.chars().all(|c| c.is_ascii_digit() || c == 'x' || c == 'X') {
            return Err(format!("Invalid resolution ({r})"));
        }
    }
    if let Some(p) = papersize {
        if !p.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(format!("Invalid papersize ({p})"));
        }
    }

    let mut cmd = Command::new(gs);
    cmd.arg("-dSAFER")
        .arg("-sDEVICE=bbox")
        .arg("-dBATCH")
        .arg("-dNOPAUSE")
        .arg("-q");
    if let Some(p) = papersize {
        cmd.arg(format!("-sPAPERSIZE={p}"));
    }
    if let Some(r) = resolution {
        cmd.arg(format!("-r{r}"));
    }
    cmd.arg("-c").arg("save pop").arg("-f").arg(input);
    for (k, v) in gs_env(gs) {
        cmd.env(k, v);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("cannot run `{}': {e}", gs.display()))?;

    let prefix = if hires {
        "%%HiResBoundingBox:"
    } else {
        "%%BoundingBox:"
    };
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let mut boxes = Vec::new();
    let mut previous: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        // Ghostscript ports that print the box data twice on stderr (pdfcrop
        // works around the same thing).
        if previous.as_deref() == Some(line) {
            continue;
        }
        previous = Some(line.to_string());
        if let Some(rest) = line.strip_prefix(prefix) {
            if let Some(b) = parse_box(rest) {
                boxes.push(b);
            }
        }
    }

    if !output.status.success() {
        let tail: Vec<&str> = text
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim().starts_with("%%"))
            .collect();
        let tail = tail
            .iter()
            .rev()
            .take(4)
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join(" | ");
        return Err(format!(
            "Ghostscript exited with {}: {}",
            output.status, tail
        ));
    }

    Ok(boxes)
}

fn parse_box(rest: &str) -> Option<Box4> {
    let nums: Vec<f64> = rest
        .split_whitespace()
        .take(4)
        .filter_map(|t| t.parse::<f64>().ok())
        .collect();
    if nums.len() == 4 {
        Some(Box4::new(nums[0], nums[1], nums[2], nums[3]))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bounding_box_line() {
        let b = parse_box(" 75 87 567 738").unwrap();
        assert_eq!(b, Box4::new(75.0, 87.0, 567.0, 738.0));
    }

    #[test]
    fn parses_negative_and_fractional_values() {
        let b = parse_box(" -1.5 0.25 612.75 -3").unwrap();
        assert_eq!(b, Box4::new(-1.5, 0.25, 612.75, -3.0));
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_box(" some 1 2 3 4").is_none());
        assert!(parse_box(" 1 2").is_none());
    }
}
