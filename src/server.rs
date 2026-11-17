//! MCP tool surface.

use crate::extract;
use crate::moodle::{Moodle, arr, i, parse_id, s};
use crate::sync::{self, SyncOptions, date};
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router,
};
use serde::Deserialize;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::OnceCell;

const INSTRUCTIONS: &str = "Read-only access to the user's Moodle (school LMS). \
Typical flow: list_courses → get_course_outline → sync_course, then read COURSE.md and the \
downloaded files/text sidecars with normal file tools (grep works well). Courses and \
assignments can be given as numeric ids or as Moodle URLs (…view.php?id=…). \
Teacher-provided material (files, assignment descriptions, starter archives, links, feedback) \
is what matters; the user's own submissions are not downloaded.";

pub fn default_dest() -> PathBuf {
    std::env::var_os("MOODLE_MCP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_default()
                .join("moodle-data")
        })
}

#[derive(Clone)]
pub struct MoodleServer {
    moodle: Arc<OnceCell<Arc<Moodle>>>,
    tool_router: ToolRouter<Self>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CourseArg {
    /// Course id or course URL (e.g. https://…/course/view.php?id=5154)
    pub course: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SyncArgs {
    /// Course id or course URL
    pub course: String,
    /// Folder to mirror into (a subfolder per course is created). Default: $MOODLE_MCP_DIR or ./moodle-data
    pub dest_dir: Option<String>,
    /// Also fetch grades / teacher feedback comments per assignment (default true)
    pub include_feedback: Option<bool>,
    /// Skip files larger than this many MB (default 100)
    pub max_file_mb: Option<u64>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AssignArg {
    /// Assignment course-module id or URL (…/mod/assign/view.php?id=…)
    pub assignment: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DownloadArgs {
    /// A Moodle file URL (…/pluginfile.php/…); the auth token is added automatically
    pub url: String,
    /// Folder to save into. Default: <MOODLE_MCP_DIR or ./moodle-data>/_downloads
    pub dest_dir: Option<String>,
}

fn err(e: impl std::fmt::Display) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

fn ok(text: String) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

#[tool_router]
impl MoodleServer {
    pub fn new() -> Self {
        Self {
            moodle: Arc::new(OnceCell::new()),
            tool_router: Self::tool_router(),
        }
    }

    async fn m(&self) -> Result<Arc<Moodle>, McpError> {
        self.moodle
            .get_or_try_init(|| async { Moodle::connect().map(Arc::new) })
            .await
            .cloned()
            .map_err(err)
    }

    #[tool(
        description = "List the user's enrolled Moodle courses (id, shortname, full name, last access), most recently accessed first."
    )]
    async fn list_courses(&self) -> Result<CallToolResult, McpError> {
        let m = self.m().await?;
        let info = m
            .ws("core_webservice_get_site_info", &[])
            .await
            .map_err(err)?;
        let v = m
            .ws(
                "core_enrol_get_users_courses",
                &[("userid", s(&info, "userid"))],
            )
            .await
            .map_err(err)?;
        let mut courses: Vec<_> = v.as_array().cloned().unwrap_or_default();
        courses.sort_by_key(|c| -i(c, "lastaccess"));
        let mut out = format!(
            "Signed in as {} on {}\n\n",
            s(&info, "fullname"),
            m.base_url()
        );
        for c in courses {
            let _ = writeln!(
                out,
                "- {} | {} | {} | last access {}",
                i(&c, "id"),
                s(&c, "shortname"),
                s(&c, "fullname"),
                date(i(&c, "lastaccess"))
            );
        }
        ok(out)
    }

    #[tool(
        description = "Show a course's structure: sections, and each module (files, folders, pages, links, assignments with due dates) with its id and file names. Cheap; use before sync_course."
    )]
    async fn get_course_outline(
        &self,
        Parameters(a): Parameters<CourseArg>,
    ) -> Result<CallToolResult, McpError> {
        let m = self.m().await?;
        let id = parse_id(&a.course).map_err(err)?;
        let info = sync::course_info(&m, id).await.map_err(err)?;
        let contents = m
            .ws("core_course_get_contents", &[("courseid", id.to_string())])
            .await
            .map_err(err)?;
        let assigns = m
            .ws(
                "mod_assign_get_assignments",
                &[("courseids[0]", id.to_string())],
            )
            .await
            .unwrap_or_default();
        let due: std::collections::HashMap<i64, i64> = arr(&assigns, "courses")
            .iter()
            .flat_map(|c| arr(c, "assignments"))
            .map(|a| (i(a, "cmid"), i(a, "duedate")))
            .collect();
        let mut out = format!("# {} ({})\n", s(&info, "fullname"), s(&info, "shortname"));
        for sec in contents.as_array().into_iter().flatten() {
            let mods = arr(sec, "modules");
            if mods.is_empty() {
                continue;
            }
            let _ = writeln!(out, "\n## {}", s(sec, "name"));
            for md in mods {
                let kind = s(md, "modname");
                let mut line = format!("- [{kind}] {} (id {})", s(md, "name"), i(md, "id"));
                if let Some(d) = due.get(&i(md, "id")).filter(|d| **d > 0) {
                    let _ = write!(line, " due {}", date(*d));
                }
                let files: Vec<String> = arr(md, "contents")
                    .iter()
                    .map(|c| {
                        if s(c, "type") == "url" {
                            s(c, "fileurl")
                        } else {
                            format!("{} ({} KB)", s(c, "filename"), i(c, "filesize") / 1000)
                        }
                    })
                    .collect();
                if !files.is_empty() && kind != "page" && kind != "book" {
                    let _ = write!(line, ": {}", files.join(", "));
                }
                let _ = writeln!(out, "{line}");
            }
        }
        ok(out)
    }

    #[tool(
        description = "Mirror a whole course to disk: writes COURSE.md (all teacher text: section summaries, labels, pages, assignment descriptions, feedback, links) and downloads every file, adding .txt/.md text sidecars for PDF/Office/HTML and unpacking .zip archives. Incremental on re-run. Returns the folder path; then read files with normal file tools."
    )]
    async fn sync_course(
        &self,
        Parameters(a): Parameters<SyncArgs>,
    ) -> Result<CallToolResult, McpError> {
        let m = self.m().await?;
        let id = parse_id(&a.course).map_err(err)?;
        let opts = SyncOptions {
            dest: a.dest_dir.map(PathBuf::from).unwrap_or_else(default_dest),
            include_feedback: a.include_feedback.unwrap_or(true),
            max_file_mb: a.max_file_mb.unwrap_or(100),
        };
        let r = sync::sync_course(&m, id, &opts).await.map_err(err)?;
        ok(r.summary())
    }

    #[tool(
        description = "Get one assignment in full: description, instructions, attachment URLs, due date, and the teacher's grade/feedback if any."
    )]
    async fn get_assignment(
        &self,
        Parameters(a): Parameters<AssignArg>,
    ) -> Result<CallToolResult, McpError> {
        let m = self.m().await?;
        let cmid = parse_id(&a.assignment).map_err(err)?;
        let cm = m
            .ws(
                "core_course_get_course_module",
                &[("cmid", cmid.to_string())],
            )
            .await
            .map_err(err)?;
        let course = i(&cm["cm"], "course");
        let v = m
            .ws(
                "mod_assign_get_assignments",
                &[("courseids[0]", course.to_string())],
            )
            .await
            .map_err(err)?;
        let Some(asg) = arr(&v, "courses")
            .iter()
            .flat_map(|c| arr(c, "assignments"))
            .find(|x| i(x, "cmid") == cmid)
            .cloned()
        else {
            return Err(McpError::invalid_params(
                format!("assignment {cmid} not found"),
                None,
            ));
        };
        let mut out = format!(
            "# {}\n\nDue: {}\n\n",
            s(&asg, "name"),
            date(i(&asg, "duedate"))
        );
        let _ = writeln!(out, "{}\n", extract::html_to_md(&s(&asg, "intro")));
        let act = s(&asg, "activity");
        if !act.trim().is_empty() {
            let _ = writeln!(out, "## Instructions\n\n{}\n", extract::html_to_md(&act));
        }
        let atts = arr(&asg, "introattachments");
        if !atts.is_empty() {
            let _ = writeln!(out, "## Attachments (use download_file)");
            for f in atts {
                let _ = writeln!(
                    out,
                    "- {} ({} KB): {}",
                    s(f, "filename"),
                    i(f, "filesize") / 1000,
                    s(f, "fileurl")
                );
            }
        }
        if let Ok(st) = m
            .ws(
                "mod_assign_get_submission_status",
                &[("assignid", s(&asg, "id"))],
            )
            .await
            && let Some(fb) = st.get("feedback")
        {
            let _ = writeln!(
                out,
                "\n## Feedback\n\nGrade: {}",
                extract::html_to_md(&s(fb, "gradefordisplay"))
            );
            for p in arr(fb, "plugins") {
                for f in arr(p, "editorfields") {
                    let t = extract::html_to_md(&s(f, "text"));
                    if !t.is_empty() {
                        let _ = writeln!(out, "\n{t}");
                    }
                }
                for area in arr(p, "fileareas") {
                    for f in arr(area, "files") {
                        let _ = writeln!(
                            out,
                            "- feedback file {}: {}",
                            s(f, "filename"),
                            s(f, "fileurl")
                        );
                    }
                }
            }
        }
        ok(out)
    }

    #[tool(
        description = "Download a single Moodle file URL (pluginfile.php) to disk and extract its text (PDF/Office/HTML) or unpack it (.zip). Returns the saved paths."
    )]
    async fn download_file(
        &self,
        Parameters(a): Parameters<DownloadArgs>,
    ) -> Result<CallToolResult, McpError> {
        let m = self.m().await?;
        if !m.is_own_file(&a.url) {
            return Err(McpError::invalid_params(
                "only file URLs on the configured Moodle site are supported",
                None,
            ));
        }
        let dir = a
            .dest_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| default_dest().join("_downloads"));
        let clean = a.url.split('?').next().unwrap_or(&a.url);
        let name = clean
            .rsplit('/')
            .next()
            .filter(|n| !n.is_empty())
            .unwrap_or("file");
        let dest = dir.join(name.replace(['/', '\\'], "_").trim_start_matches('.'));
        let bytes = m.download(&a.url, &dest, 500_000_000).await.map_err(err)?;
        let mut out = format!("Saved {} ({} KB)", dest.display(), bytes / 1000);
        let d = dest.clone();
        match tokio::task::spawn_blocking(move || extract::extract(&d)).await {
            Ok(Ok(extract::Extracted::Text(p))) => {
                let _ = write!(out, "\nText: {}", p.display());
            }
            Ok(Ok(extract::Extracted::Unzipped(p, n))) => {
                let _ = write!(out, "\nUnpacked {n} files to {}", p.display());
            }
            Ok(Err(e)) => {
                let _ = write!(out, "\nText extraction failed: {e}");
            }
            _ => {}
        }
        ok(out)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for MoodleServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("moodle-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

pub async fn run() -> anyhow::Result<()> {
    let service = MoodleServer::new().serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
