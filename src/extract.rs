//! Turns downloaded files into something Claude can read: text sidecars for
//! PDFs / Office docs / HTML, and unpacked folders for zip archives.

use anyhow::{Context, Result, bail};
use regex::Regex;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// Folders inside archives that are build output or tooling noise.
const SKIP_DIRS: &[&str] = &[
    "bin", "obj", ".git", "node_modules", ".vs", ".idea", "__MACOSX", "target", ".gradle", "dist",
];
const ZIP_MAX_TOTAL: u64 = 200_000_000;

pub enum Extracted {
    Text(PathBuf),
    Unzipped(PathBuf, usize),
    None,
}

/// Writes `<file>.txt` / `<file>.md` or an unpacked folder next to `file`.
pub fn extract(file: &Path) -> Result<Extracted> {
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let sidecar = |suffix: &str| {
        let mut name = file.file_name().unwrap().to_os_string();
        name.push(suffix);
        file.with_file_name(name)
    };
    let text = match ext.as_str() {
        "pdf" => pdf_text(file)?,
        "docx" => office_text(file, |n| n == "word/document.xml")?,
        "pptx" => office_text(file, |n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"))?,
        "xlsx" => office_text(file, |n| n == "xl/sharedStrings.xml")?,
        "odt" | "odp" | "ods" => office_text(file, |n| n == "content.xml")?,
        "html" | "htm" => {
            let html = std::fs::read_to_string(file)?;
            let out = sidecar(".md");
            std::fs::write(&out, html_to_md(&html))?;
            return Ok(Extracted::Text(out));
        }
        "zip" => {
            let dir = file.with_extension("");
            let dir = if dir == file { sidecar("_unzipped") } else { dir };
            let n = unzip(file, &dir)?;
            return Ok(Extracted::Unzipped(dir, n));
        }
        _ => return Ok(Extracted::None),
    };
    if text.trim().is_empty() {
        return Ok(Extracted::None);
    }
    let out = sidecar(".txt");
    std::fs::write(&out, text)?;
    Ok(Extracted::Text(out))
}

fn pdf_text(file: &Path) -> Result<String> {
    let out = std::process::Command::new("pdftotext")
        .arg("-layout")
        .arg(file)
        .arg("-")
        .output()
        .context("pdftotext not installed (poppler)")?;
    if !out.status.success() {
        bail!("pdftotext: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Text of the XML parts of an OOXML/ODF archive, parts in natural order.
fn office_text(file: &Path, want: impl Fn(&str) -> bool) -> Result<String> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(file)?)?;
    let mut names: Vec<String> = zip.file_names().filter_map(|n| n.ok().map(|c| c.into_owned())).filter(|n| want(n)).collect();
    names.sort_by_key(|n| {
        let digits: String = n.chars().filter(char::is_ascii_digit).collect();
        (digits.parse::<u64>().unwrap_or(0), n.clone())
    });
    let mut out = String::new();
    for (idx, name) in names.iter().enumerate() {
        let mut xml = String::new();
        zip.by_name(name)?.read_to_string(&mut xml)?;
        if names.len() > 1 {
            out.push_str(&format!("\n--- {} {} ---\n", if name.contains("slide") { "Slide" } else { "Part" }, idx + 1));
        }
        out.push_str(&xml_text(&xml));
    }
    Ok(out)
}

static PARA_END: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"</(w:p|a:p|text:p|text:h|si)>|<(w:br|text:line-break|a:br)\s*/?>").unwrap());
static TAB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<(w:tab|text:tab)\s*/>").unwrap());
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());
static BLANKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());

fn xml_text(xml: &str) -> String {
    let t = PARA_END.replace_all(xml, "\n");
    let t = TAB.replace_all(&t, "\t");
    let t = TAG.replace_all(&t, "");
    let t = unescape(&t);
    BLANKS.replace_all(&t, "\n\n").into_owned()
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

pub fn html_to_md(html: &str) -> String {
    let md = html2md::parse_html(html);
    BLANKS.replace_all(md.trim(), "\n\n").into_owned()
}

/// Unpacks a zip safely (no path escapes, size cap, build folders skipped).
fn unzip(file: &Path, dest: &Path) -> Result<usize> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(file)?)?;
    if dest.exists() {
        std::fs::remove_dir_all(dest)?;
    }
    let mut total = 0u64;
    let mut count = 0;
    for idx in 0..zip.len() {
        let mut entry = zip.by_index(idx)?;
        let Some(rel) = entry.enclosed_name() else { continue };
        if rel.components().any(|c| SKIP_DIRS.contains(&c.as_os_str().to_string_lossy().as_ref())) {
            continue;
        }
        let out = dest.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        total += entry.size();
        if total > ZIP_MAX_TOTAL {
            bail!("archive unpacks to more than {} MB", ZIP_MAX_TOTAL / 1_000_000);
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut f = std::fs::File::create(&out)?;
        std::io::copy(&mut entry, &mut f)?;
        count += 1;
        // Nested docs inside archives get text sidecars too.
        if matches!(
            out.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref(),
            Some("pdf" | "docx" | "pptx" | "odt" | "odp")
        ) {
            let _ = extract(&out);
        }
    }
    Ok(count)
}

pub fn pdftotext_available() -> bool {
    std::process::Command::new("pdftotext").arg("-v").output().is_ok()
}
