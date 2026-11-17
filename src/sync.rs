//! Mirrors a Moodle course to disk: one COURSE.md index with all teacher text,
//! plus every file (with text sidecars / unpacked archives) in section folders.

use crate::extract;
use crate::moodle::{Moodle, arr, i, s};
use anyhow::Result;
use futures::StreamExt;
use regex::Regex;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

const STATE_FILE: &str = ".moodle-sync.json";

pub struct SyncOptions {
    pub dest: PathBuf,
    pub include_feedback: bool,
    pub max_file_mb: u64,
}

#[derive(Default)]
pub struct SyncReport {
    pub root: PathBuf,
    pub title: String,
    pub sections: usize,
    pub modules: usize,
    pub downloaded: usize,
    pub unchanged: usize,
    pub failed: Vec<String>,
    pub external_links: Vec<(String, String)>,
}

impl SyncReport {
    pub fn summary(&self) -> String {
        let mut o = String::new();
        let _ = writeln!(o, "Synced \"{}\" to {}", self.title, self.root.display());
        let _ = writeln!(
            o,
            "{} sections, {} modules; {} files downloaded, {} unchanged, {} failed.",
            self.sections,
            self.modules,
            self.downloaded,
            self.unchanged,
            self.failed.len()
        );
        let _ = writeln!(
            o,
            "Start with {}/COURSE.md (index of all teacher text with links to every file; .txt/.md sidecars hold extracted text, archives are unpacked next to the .zip).",
            self.root.display()
        );
        if !crate::extract::pdftotext_available() {
            let _ = writeln!(
                o,
                "Note: pdftotext isn't installed, so PDFs have no .txt sidecar; read the PDF files directly."
            );
        }
        if !self.failed.is_empty() {
            let _ = writeln!(o, "\nFailed:");
            for f in &self.failed {
                let _ = writeln!(o, "- {f}");
            }
        }
        if !self.external_links.is_empty() {
            let _ = writeln!(
                o,
                "\nExternal links (not downloaded, fetch separately if relevant):"
            );
            for (label, url) in &self.external_links {
                let _ = writeln!(o, "- {label}: {url}");
            }
        }
        o
    }
}

struct Job {
    url: String,
    rel: PathBuf,
    modified: i64,
}

#[derive(Default)]
struct Ctx {
    jobs: Vec<Job>,
    claimed: HashSet<PathBuf>,
    links: Vec<(String, String)>,
    seen_links: HashSet<String>,
}

impl Ctx {
    /// Queues a download; returns the (deduplicated) relative path.
    fn file(&mut self, url: &str, rel: PathBuf, modified: i64) -> PathBuf {
        let mut rel = rel;
        let mut n = 2;
        while self.claimed.contains(&rel) {
            let stem = rel
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let ext = rel
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            rel = rel.with_file_name(format!("{stem}-{n}{ext}"));
            n += 1;
        }
        self.claimed.insert(rel.clone());
        self.jobs.push(Job {
            url: url.to_owned(),
            rel: rel.clone(),
            modified,
        });
        rel
    }

    fn link(&mut self, label: &str, url: &str) {
        if self.seen_links.insert(url.to_owned()) {
            self.links.push((label.to_owned(), url.to_owned()));
        }
    }
}

pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
        if out.len() > 60 {
            break;
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "untitled".into()
    } else {
        out
    }
}

/// A file name that can't escape its folder.
fn safe_name(s: &str) -> String {
    let n: String = s
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c.is_control() || c == ':' {
                '_'
            } else {
                c
            }
        })
        .collect();
    let n = n.trim().trim_start_matches('.').to_owned();
    if n.is_empty() { "file".into() } else { n }
}

fn safe_subpath(filepath: &str) -> PathBuf {
    filepath
        .split('/')
        .filter(|p| !p.is_empty() && *p != "." && *p != "..")
        .map(safe_name)
        .collect()
}

pub fn date(ts: i64) -> String {
    if ts <= 0 {
        return "-".into();
    }
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|d| {
            d.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

static PLUGINFILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?:src|href)="([^"]*?/pluginfile\.php/[^"]+)""#).unwrap());
static HREF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"href="(https?://[^"]+)""#).unwrap());

/// HTML → Markdown, downloading embedded Moodle files into `_media/` and
/// collecting external links. `up` is the relative path back to the course root.
fn rich(ctx: &mut Ctx, m: &Moodle, html: &str, up: &str, label: &str) -> String {
    if html.trim().is_empty() {
        return String::new();
    }
    let mut html = html.to_owned();
    let urls: Vec<String> = PLUGINFILE
        .captures_iter(&html)
        .map(|c| c[1].replace("&amp;", "&"))
        .collect();
    for url in urls {
        let clean = url.split('?').next().unwrap_or(&url);
        let name = clean.rsplit('/').next().unwrap_or("file");
        let name = safe_name(&percent_decode(name));
        let tag = &sha1_smol::Sha1::from(clean).digest().to_string()[..8];
        let rel = PathBuf::from("_media").join(format!("{tag}-{name}"));
        let rel = if ctx.claimed.contains(&rel) {
            rel
        } else {
            ctx.file(&url, rel, 0)
        };
        html = html.replace(
            &url.replace('&', "&amp;"),
            &format!("{up}{}", rel.display()),
        );
        html = html.replace(&url, &format!("{up}{}", rel.display()));
    }
    for c in HREF.captures_iter(&html) {
        let u = c[1].replace("&amp;", "&");
        if !m.is_own_file(&u) && !u.starts_with(m.base_url()) {
            ctx.link(label, &u);
        }
    }
    extract::html_to_md(&html)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == b'%'
            && idx + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(&s[idx + 1..idx + 3], 16)
        {
            out.push(b);
            idx += 3;
            continue;
        }
        out.push(bytes[idx]);
        idx += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn indent(text: &str) -> String {
    text.lines()
        .map(|l| {
            if l.is_empty() {
                ">".to_owned()
            } else {
                format!("> {l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub async fn course_info(m: &Moodle, course_id: i64) -> Result<Value> {
    let v = m
        .ws(
            "core_course_get_courses_by_field",
            &[("field", "id".into()), ("value", course_id.to_string())],
        )
        .await?;
    Ok(arr(&v, "courses").first().cloned().unwrap_or(Value::Null))
}

pub async fn sync_course(m: &Moodle, course_id: i64, opts: &SyncOptions) -> Result<SyncReport> {
    let info = course_info(m, course_id).await?;
    let title = s(&info, "fullname");
    let short = s(&info, "shortname");
    let contents = m
        .ws(
            "core_course_get_contents",
            &[("courseid", course_id.to_string())],
        )
        .await?;
    let assigns = m
        .ws(
            "mod_assign_get_assignments",
            &[("courseids[0]", course_id.to_string())],
        )
        .await
        .unwrap_or(Value::Null);
    let assign_by_cm: HashMap<i64, Value> = arr(&assigns, "courses")
        .iter()
        .flat_map(|c| arr(c, "assignments"))
        .map(|a| (i(a, "cmid"), a.clone()))
        .collect();
    let pages = m
        .ws(
            "mod_page_get_pages_by_courses",
            &[("courseids[0]", course_id.to_string())],
        )
        .await
        .unwrap_or(Value::Null);
    let page_by_cm: HashMap<i64, String> = arr(&pages, "pages")
        .iter()
        .map(|p| (i(p, "coursemodule"), s(p, "content")))
        .collect();

    let root = opts
        .dest
        .join(slug(if short.is_empty() { &title } else { &short }));
    std::fs::create_dir_all(root.join(".raw"))?;
    std::fs::write(
        root.join(".raw/contents.json"),
        serde_json::to_string_pretty(&contents)?,
    )?;
    std::fs::write(
        root.join(".raw/assignments.json"),
        serde_json::to_string_pretty(&assigns)?,
    )?;

    let mut ctx = Ctx::default();
    let mut report = SyncReport {
        root: root.clone(),
        title: title.clone(),
        ..Default::default()
    };
    let mut md = String::new();
    let _ = writeln!(md, "# {title}\n");
    let _ = writeln!(
        md,
        "- Moodle: {}/course/view.php?id={course_id}",
        m.base_url()
    );
    let _ = writeln!(
        md,
        "- Synced: {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M")
    );
    let summary = rich(&mut ctx, m, &s(&info, "summary"), "", "Course summary");
    if !summary.is_empty() {
        let _ = writeln!(md, "{summary}\n");
    }

    let sections = contents.as_array().cloned().unwrap_or_default();
    for (n, sec) in sections.iter().enumerate() {
        let name = s(sec, "name");
        let mods = arr(sec, "modules");
        let sec_summary = s(sec, "summary");
        if mods.is_empty() && sec_summary.trim().is_empty() {
            continue;
        }
        report.sections += 1;
        let sec_dir = PathBuf::from(format!("{:02}-{}", n, slug(&name)));
        let _ = writeln!(md, "\n## {name}\n");
        let t = rich(&mut ctx, m, &sec_summary, "", &name);
        if !t.is_empty() {
            let _ = writeln!(md, "{t}\n");
        }
        let mut mod_dirs = HashSet::new();
        for module in mods {
            if module.get("uservisible") == Some(&Value::Bool(false)) {
                continue;
            }
            report.modules += 1;
            let cmid = i(module, "id");
            let kind = s(module, "modname");
            let mname = s(module, "name");
            let mut mdir_name = slug(&mname);
            if !mod_dirs.insert(mdir_name.clone()) {
                mdir_name = format!("{mdir_name}-{cmid}");
            }
            let mdir = sec_dir.join(&mdir_name);
            let up = "../../";
            if kind == "label" {
                let t = rich(&mut ctx, m, &s(module, "description"), "", &name);
                let _ = writeln!(md, "{t}\n");
                continue;
            }
            let _ = writeln!(md, "### {mname}  `{kind}`\n");
            let murl = s(module, "url");
            if !murl.is_empty() {
                let _ = writeln!(md, "<{murl}>\n");
            }
            let label = format!("{name} / {mname}");

            if kind == "assign"
                && let Some(a) = assign_by_cm.get(&cmid)
            {
                let due = i(a, "duedate");
                if due > 0 {
                    let _ = writeln!(md, "Due: {}\n", date(due));
                }
                let intro = rich(&mut ctx, m, &s(a, "intro"), "", &label);
                if !intro.is_empty() {
                    let _ = writeln!(md, "{}\n", indent(&intro));
                }
                let act = rich(&mut ctx, m, &s(a, "activity"), "", &label);
                if !act.is_empty() {
                    let _ = writeln!(md, "**Instructions:**\n\n{}\n", indent(&act));
                }
                for f in arr(a, "introattachments") {
                    let rel = ctx.file(
                        &s(f, "fileurl"),
                        mdir.join(safe_subpath(&s(f, "filepath")))
                            .join(safe_name(&s(f, "filename"))),
                        i(f, "timemodified"),
                    );
                    let _ = writeln!(md, "- {{{{FILE:{}}}}}", rel.display());
                }
                if opts.include_feedback
                    && let Ok(fb) = feedback(m, &mut ctx, i(a, "id"), &mdir).await
                    && !fb.is_empty()
                {
                    let _ = writeln!(md, "\n**Teacher feedback:**\n\n{fb}");
                }
                let _ = writeln!(md);
                continue;
            }

            let desc = rich(&mut ctx, m, &s(module, "description"), "", &label);
            if !desc.is_empty() {
                let _ = writeln!(md, "{}\n", indent(&desc));
            }
            if kind == "page" {
                if let Some(html) = page_by_cm.get(&cmid) {
                    let body = rich(&mut ctx, m, html, up, &label);
                    let path = root.join(&mdir).join("page.md");
                    std::fs::create_dir_all(path.parent().unwrap())?;
                    std::fs::write(&path, format!("# {mname}\n\n{body}\n"))?;
                    let _ = writeln!(
                        md,
                        "Page content: [{0}]({0})\n",
                        mdir.join("page.md").display()
                    );
                    if body.len() < 3000 {
                        let _ = writeln!(md, "{}\n", indent(&body));
                    }
                }
                continue;
            }
            for c in arr(module, "contents") {
                let url = s(c, "fileurl");
                match s(c, "type").as_str() {
                    "url" => {
                        let _ = writeln!(md, "- Link: <{url}>");
                        ctx.link(&label, &url);
                    }
                    "file" if !url.is_empty() => {
                        let rel = mdir
                            .join(safe_subpath(&s(c, "filepath")))
                            .join(safe_name(&s(c, "filename")));
                        let rel = ctx.file(&url, rel, i(c, "timemodified"));
                        let _ = writeln!(md, "- {{{{FILE:{}}}}}", rel.display());
                    }
                    _ => {}
                }
            }
            if kind == "forum" {
                let posts = forum(m, &mut ctx, i(module, "instance"), &label)
                    .await
                    .unwrap_or_default();
                if !posts.is_empty() {
                    let _ = writeln!(md, "{posts}");
                }
            }
            let _ = writeln!(md);
        }
    }

    // Download everything (incrementally) and extract text.
    let state_path = root.join(STATE_FILE);
    let mut state: BTreeMap<String, i64> = std::fs::read(&state_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let max = opts.max_file_mb * 1_000_000;
    let jobs: Vec<(Job, bool)> = std::mem::take(&mut ctx.jobs)
        .into_iter()
        .map(|job| {
            let fresh = job.modified != 0
                && state.get(&job.rel.display().to_string()) == Some(&job.modified)
                && root.join(&job.rel).exists();
            (job, fresh)
        })
        .collect();
    let root_ref = &root;
    let results: Vec<(PathBuf, i64, Result<bool, String>)> = futures::stream::iter(jobs)
        .map(move |(job, fresh)| async move {
            if fresh {
                return (job.rel, job.modified, Ok(false));
            }
            let dest = root_ref.join(&job.rel);
            let r = match m.download(&job.url, &dest, max).await {
                Ok(_) => {
                    let _ = tokio::task::spawn_blocking(move || extract::extract(&dest)).await;
                    Ok(true)
                }
                Err(e) => Err(e.to_string()),
            };
            (job.rel, job.modified, r)
        })
        .buffer_unordered(6)
        .collect()
        .await;
    for (rel, modified, r) in results {
        let key = rel.display().to_string();
        match r {
            Ok(true) => {
                report.downloaded += 1;
                state.insert(key, modified);
            }
            Ok(false) => report.unchanged += 1,
            Err(e) => report.failed.push(format!("{key}: {e}")),
        }
    }
    std::fs::write(&state_path, serde_json::to_string_pretty(&state)?)?;

    // Fill in file links now that sidecars exist.
    let md = FILE_REF
        .replace_all(&md, |c: &regex::Captures| {
            file_line(&root, Path::new(&c[1]))
        })
        .into_owned();
    let mut md = md;
    if !ctx.links.is_empty() {
        let _ = writeln!(md, "\n## External links (not downloaded)\n");
        for (label, url) in &ctx.links {
            let _ = writeln!(md, "- {label}: <{url}>");
        }
    }
    std::fs::write(root.join("COURSE.md"), md)?;
    report.external_links = ctx.links;
    Ok(report)
}

static FILE_REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\{FILE:([^}]+)\}\}").unwrap());

fn file_line(root: &Path, rel: &Path) -> String {
    let name = rel.file_name().unwrap_or_default().to_string_lossy();
    let mut line = format!("[{name}]({})", rel.display());
    let abs = root.join(rel);
    if !abs.exists() {
        line.push_str(" (download failed)");
        return line;
    }
    for suffix in [".txt", ".md"] {
        let mut side = rel.as_os_str().to_owned();
        side.push(suffix);
        if root.join(&side).exists() {
            let _ = write!(line, " · text: [{0}]({0})", Path::new(&side).display());
        }
    }
    let unzipped = rel.with_extension("");
    if rel
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
        && root.join(&unzipped).is_dir()
    {
        let _ = write!(line, " · unpacked: [{0}/]({0}/)", unzipped.display());
    }
    line
}

/// Grade + teacher comments (+ feedback files) for one assignment.
async fn feedback(m: &Moodle, ctx: &mut Ctx, assign_id: i64, mdir: &Path) -> Result<String> {
    let v = m
        .ws(
            "mod_assign_get_submission_status",
            &[("assignid", assign_id.to_string())],
        )
        .await?;
    let mut out = String::new();
    let Some(fb) = v.get("feedback") else {
        return Ok(out);
    };
    let grade = s(fb, "gradefordisplay");
    if !grade.is_empty() {
        let _ = writeln!(out, "- Grade: {}", extract::html_to_md(&grade));
    }
    for p in arr(fb, "plugins") {
        for field in arr(p, "editorfields") {
            let t = rich(ctx, m, &s(field, "text"), "", "Feedback");
            if !t.is_empty() {
                let _ = writeln!(out, "{}", indent(&t));
            }
        }
        for area in arr(p, "fileareas") {
            for f in arr(area, "files") {
                let name = safe_name(&s(f, "filename"));
                let rel = ctx.file(
                    &s(f, "fileurl"),
                    mdir.join("feedback").join(name),
                    i(f, "timemodified"),
                );
                let _ = writeln!(out, "- {{{{FILE:{}}}}}", rel.display());
            }
        }
    }
    Ok(out)
}

/// Announcement / forum posts (first post of each discussion).
async fn forum(m: &Moodle, ctx: &mut Ctx, forum_id: i64, label: &str) -> Result<String> {
    let v = m
        .ws(
            "mod_forum_get_forum_discussions",
            &[("forumid", forum_id.to_string()), ("perpage", "50".into())],
        )
        .await?;
    let mut out = String::new();
    for d in arr(&v, "discussions") {
        let _ = writeln!(
            out,
            "#### {} ({}, {})\n",
            s(d, "subject"),
            s(d, "userfullname"),
            date(i(d, "created"))
        );
        let t = rich(ctx, m, &s(d, "message"), "", label);
        let _ = writeln!(out, "{}\n", indent(&t));
        for f in arr(d, "attachments") {
            let rel = ctx.file(
                &s(f, "fileurl"),
                PathBuf::from("_forum")
                    .join(i(d, "discussion").to_string())
                    .join(safe_name(&s(f, "filename"))),
                i(f, "timemodified"),
            );
            let _ = writeln!(out, "- {{{{FILE:{}}}}}", rel.display());
        }
    }
    Ok(out)
}
