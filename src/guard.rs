//! `moodle-mcp guard`: PreToolUse hook for exam-prep workspaces.
//!
//! A workspace is a folder with STUDY.md. Inside its `practice/`, any folder that
//! contains a `.student` marker (or `practice/.student` itself) belongs to the
//! student: Claude may not change files there. Claude can still prepare new
//! drills in unmarked folders. Exit code 2 blocks the call; stderr goes to Claude.

use regex::Regex;
use serde_json::Value;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

const MSG: &str = "is the student's practice work (marked with .student). Don't write it for them: \
give the next hint from the hint ladder, point to the lesson or teacher file, or review their \
work and let them fix it. Only the student removes .student markers.";

static WRITES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(>|\btee\b|\bsed\b[^|;&]*\s-i|\bperl\b[^|;&]*\s-p?i|\bmv\b|\bcp\b|\brm\b|\btouch\b|\bpatch\b|\btruncate\b|\bgit\s+(checkout|restore|apply|reset|stash|clean)\b|\bdotnet\s+(new|add)\b|\bpython3?\b|\bnode\b|\b(Set|Add|Out|Remove|Move|Copy|New)-(Content|Item)\b|\b(del|copy|move|ren)\b)",
    )
    .unwrap()
});
static REPO_WIDE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bgit\s+(stash|clean|reset\s+--hard|checkout\s+(\.|--\s*\.)|restore\s+\.)").unwrap()
});
static PATHS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"[^\s'"=;&|<>()]*practice[^\s'"=;&|<>()]*"#).unwrap());

/// Lexically normalizes `..` and `.` (the target may not exist yet).
fn clean(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// The student-owned folder that contains `path`, if any.
pub fn guarded_dir(path: &Path) -> Option<PathBuf> {
    let path = clean(path);
    let comps: Vec<_> = path.components().collect();
    for idx in (1..comps.len()).rev() {
        if comps[idx].as_os_str() != "practice" {
            continue;
        }
        let root: PathBuf = comps[..idx].iter().collect();
        if !root.join("STUDY.md").is_file() {
            continue;
        }
        let mut dir = root.join("practice");
        let rest = &comps[idx + 1..];
        let mut step = 0;
        loop {
            if dir.join(".student").is_file() {
                return Some(dir);
            }
            if !dir.is_dir() || step >= rest.len() {
                return None;
            }
            dir = dir.join(rest[step]);
            step += 1;
        }
    }
    None
}

fn any_marked_below(dir: &Path) -> Option<PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if e.file_name() == ".student" {
                if let Some(g) = guarded_dir(&p) {
                    return Some(g);
                }
            } else if p.is_dir() && e.file_name() != ".git" && e.file_name() != "moodle-data" {
                stack.push(p);
            }
        }
    }
    None
}

fn check(input: &Value) -> Option<PathBuf> {
    let tool = input["tool_name"].as_str().unwrap_or("");
    let ti = &input["tool_input"];
    let cwd = PathBuf::from(input["cwd"].as_str().map(String::from).unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_default().display().to_string()
    }));
    match tool {
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => {
            let target = ti["file_path"].as_str().or(ti["notebook_path"].as_str())?;
            guarded_dir(&cwd.join(target))
        }
        "Bash" | "PowerShell" => {
            let cmd = ti["command"].as_str().unwrap_or("");
            if !WRITES.is_match(cmd) {
                return None;
            }
            let mut candidates: Vec<PathBuf> =
                PATHS.find_iter(cmd).map(|m| cwd.join(m.as_str())).collect();
            candidates.push(cwd.join("x"));
            if REPO_WIDE.is_match(cmd) {
                candidates.push(cwd.clone());
            }
            candidates.into_iter().find_map(|c| {
                guarded_dir(&c).or_else(|| if c.is_dir() { any_marked_below(&c) } else { None })
            })
        }
        _ => None,
    }
}

pub fn run() -> i32 {
    let mut raw = String::new();
    if std::io::stdin().read_to_string(&mut raw).is_err() {
        return 0;
    }
    let Ok(input) = serde_json::from_str::<Value>(&raw) else { return 0 };
    match check(&input) {
        Some(dir) => {
            eprintln!("Blocked: {} {MSG}", dir.display());
            2
        }
        None => 0,
    }
}
