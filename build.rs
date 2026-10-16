//! Windows only: embed the icon and version metadata in pdfcrop.exe so the
//! executable, the context-menu entry and the "Open with" dialog show it.
//!
//! Failures are downgraded to warnings: a missing resource compiler must not
//! break the build, the tool works fine with a default icon.

fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/pdfcrop.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/pdfcrop.ico");
        res.set("ProductName", "PDFCROP");
        res.set("FileDescription", "PDFCROP - crop PDF margins per page");
        res.set("CompanyName", "Tectonica");
        res.set("LegalCopyright", "MIT licence; bundles Ghostscript 10.08 (AGPL v3)");
        res.set("OriginalFilename", "pdfcrop.exe");
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
        res.set("ProductVersion", &version);
        res.set("FileVersion", &version);
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the Windows resources: {e}");
        }
    }
}
