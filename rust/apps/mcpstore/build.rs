use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn main() -> io::Result<()> {
    generate_web_assets()?;
    Ok(())
}

fn generate_web_assets() -> io::Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let out_file = out_dir.join("embedded_web_assets.rs");

    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by Cargo"));
    let repo_web_dir = manifest_dir.join("../../../web/dist");
    let packaged_web_dir = manifest_dir.join("assets/web");
    let web_dir = if repo_web_dir.exists() {
        repo_web_dir
    } else {
        packaged_web_dir
    };

    println!("cargo:rerun-if-changed={}", web_dir.display());

    if !web_dir.exists() {
        write_empty_assets(&out_file)?;
        return Ok(());
    }

    let mut files = Vec::new();
    collect_files(&web_dir, &web_dir, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));

    for (_, absolute) in &files {
        println!("cargo:rerun-if-changed={}", absolute.display());
    }

    write_assets(&out_file, &files)
}

fn write_empty_assets(out_file: &Path) -> io::Result<()> {
    fs::write(
        out_file,
        r#"pub struct WebAsset {
    pub path: &'static str,
    pub contents: &'static [u8],
    pub mime: &'static str,
}

pub static ASSETS: &[WebAsset] = &[];
"#,
    )
}

fn write_assets(out_file: &Path, files: &[(String, PathBuf)]) -> io::Result<()> {
    let mut generated = String::from(
        r#"pub struct WebAsset {
    pub path: &'static str,
    pub contents: &'static [u8],
    pub mime: &'static str,
}

pub static ASSETS: &[WebAsset] = &[
"#,
    );

    for (relative, absolute) in files {
        let path_literal = format!("{relative:?}");
        let absolute_literal = format!("{:?}", absolute.to_string_lossy());
        let mime_literal = format!("{:?}", mime_for_path(relative));
        generated.push_str("    WebAsset {\n");
        generated.push_str(&format!("        path: {path_literal},\n"));
        generated.push_str(&format!(
            "        contents: include_bytes!({absolute_literal}),\n"
        ));
        generated.push_str(&format!("        mime: {mime_literal},\n"));
        generated.push_str("    },\n");
    }

    generated.push_str("];\n");
    fs::write(out_file, generated)
}

fn collect_files(root: &Path, dir: &Path, files: &mut Vec<(String, PathBuf)>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .expect("asset path is under root")
            .to_string_lossy()
            .replace('\\', "/");
        files.push((relative, path));
    }
    Ok(())
}

fn mime_for_path(path: &str) -> &'static str {
    match Path::new(path).extension().and_then(|ext| ext.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("ico") => "image/x-icon",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("wasm") => "application/wasm",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}
