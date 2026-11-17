mod extract;
mod guard;
mod moodle;
mod server;
mod sync;

use anyhow::{Result, bail};
use std::io::Write;

const USAGE: &str = "moodle-mcp — Moodle MCP server for Claude Code

usage:
  moodle-mcp login [url]           sign in once (stores a token, never the password)
  moodle-mcp status                show which Moodle account is configured
  moodle-mcp logout                delete the saved login
  moodle-mcp                       run the MCP server on stdio (Claude Code starts this)
  moodle-mcp sync <course> [dir]   mirror a course (id or URL) to disk
  moodle-mcp guard                 PreToolUse hook protecting exam-prep practice folders
  moodle-mcp ws <function> k=v...  call a raw web-service function (debug)";

fn prompt(label: &str) -> Result<String> {
    eprint!("{label}");
    std::io::stderr().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_owned())
}

async fn login(url_arg: Option<&String>) -> Result<()> {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        bail!(
            "`moodle-mcp login` must be run by the person themselves in an interactive terminal \
             (it asks for their Moodle password). AI agents: don't run it, ask the user to."
        );
    }
    eprintln!("Sign in to Moodle. Your password is sent to Moodle only and is not stored.\n");
    let raw = match url_arg {
        Some(u) => u.clone(),
        None => prompt("Moodle URL (any page of your Moodle works, e.g. a course link): ")?,
    };
    let base = moodle::normalize_base_url(&raw);
    if base.is_empty() {
        bail!("no URL given");
    }
    eprintln!("Moodle site: {base}");
    let username = prompt("Username: ")?;
    let password = rpassword::prompt_password("Password: ")?;
    let token = moodle::request_token(&base, &username, &password).await?;
    let m = moodle::Moodle::with_token(&base, &token)?;
    let info = m.ws("core_webservice_get_site_info", &[]).await?;
    let path = moodle::save(&moodle::Saved { base_url: base.clone(), username, token })?;
    eprintln!(
        "\nSigned in as {} on {}.\nSaved to {}. Restart Claude Code so it picks up the login.",
        moodle::s(&info, "fullname"),
        moodle::s(&info, "sitename"),
        path.display()
    );
    Ok(())
}

async fn status() -> Result<()> {
    let m = moodle::Moodle::connect()?;
    let info = m.ws("core_webservice_get_site_info", &[]).await?;
    println!(
        "OK: signed in as {} on {} ({}).",
        moodle::s(&info, "fullname"),
        moodle::s(&info, "sitename"),
        m.base_url()
    );
    println!(
        "PDF text extraction: {}",
        if extract::pdftotext_available() { "pdftotext found" } else { "pdftotext not found (optional; Claude then reads PDFs directly)" }
    );
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("serve") => server::run().await,
        Some("guard") => std::process::exit(guard::run()),
        Some("login") => login(args.get(1)).await,
        Some("status") => status().await,
        Some("logout") => {
            let p = moodle::config_path()?;
            if p.exists() {
                std::fs::remove_file(&p)?;
            }
            println!("Removed {}", p.display());
            Ok(())
        }
        Some("sync") if args.len() >= 2 => {
            let m = moodle::Moodle::connect()?;
            let id = moodle::parse_id(&args[1])?;
            let opts = sync::SyncOptions {
                dest: args.get(2).map(Into::into).unwrap_or_else(server::default_dest),
                include_feedback: true,
                max_file_mb: 100,
            };
            let r = sync::sync_course(&m, id, &opts).await?;
            println!("{}", r.summary());
            Ok(())
        }
        Some("ws") if args.len() >= 2 => {
            let m = moodle::Moodle::connect()?;
            let params: Vec<(&str, String)> = args[2..]
                .iter()
                .filter_map(|a| a.split_once('='))
                .map(|(k, v)| (k, v.to_owned()))
                .collect();
            let v = m.ws(&args[1], &params).await?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(())
        }
        Some("--version" | "-V") => {
            println!("moodle-mcp {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => {
            eprintln!("{USAGE}");
            Ok(())
        }
    }
}
