//! Thin Moodle web-service client. Signs in once with `moodle-mcp login`, which
//! stores only the web-service token (never the password) in the user's config dir.

use anyhow::{Context, Result, anyhow, bail};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

pub const SERVICE: &str = "moodle_mobile_app";

#[derive(Serialize, Deserialize, Clone)]
pub struct Saved {
    pub base_url: String,
    pub username: String,
    pub token: String,
}

pub fn config_path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("MOODLE_MCP_CONFIG") {
        return Ok(PathBuf::from(p));
    }
    Ok(dirs::config_dir()
        .ok_or_else(|| anyhow!("no config directory on this system"))?
        .join("moodle-mcp")
        .join("config.json"))
}

pub fn load_saved() -> Option<Saved> {
    serde_json::from_slice(&std::fs::read(config_path().ok()?).ok()?).ok()
}

pub fn save(saved: &Saved) -> Result<PathBuf> {
    let path = config_path()?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, serde_json::to_vec_pretty(saved)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(path)
}

/// `https://host/moodle/course/view.php?id=1` → `https://host/moodle`.
pub fn normalize_base_url(raw: &str) -> String {
    let mut s = raw
        .trim()
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .trim_end_matches('/')
        .to_owned();
    for marker in [
        "/course/",
        "/my",
        "/login/",
        "/mod/",
        "/user/",
        "/index.php",
        "/admin/",
    ] {
        if let Some(at) = s.find(marker) {
            s.truncate(at);
        }
    }
    if !s.starts_with("http://") && !s.starts_with("https://") && !s.is_empty() {
        s = format!("https://{s}");
    }
    s.trim_end_matches('/').to_owned()
}

fn http() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(concat!("moodle-mcp/", env!("CARGO_PKG_VERSION")))
        .build()?)
}

/// Exchanges username + password for a web-service token (login/token.php).
pub async fn request_token(base_url: &str, username: &str, password: &str) -> Result<String> {
    let resp: Value = http()?
        .post(format!("{base_url}/login/token.php"))
        .form(&[
            ("username", username),
            ("password", password),
            ("service", SERVICE),
        ])
        .send()
        .await?
        .json()
        .await
        .context("Moodle's login endpoint didn't answer with JSON (is the URL right?)")?;
    if let Some(t) = resp.get("token").and_then(Value::as_str) {
        return Ok(t.to_owned());
    }
    let code = s(&resp, "errorcode");
    match code.as_str() {
        "invalidlogin" => bail!("wrong username or password"),
        _ => bail!(
            "login failed: {} ({code}). If your school signs in through another service (SSO), \
             get a token from Moodle → Preferences → Security keys and set MOODLE_URL + MOODLE_TOKEN instead.",
            s(&resp, "error")
        ),
    }
}

pub struct Moodle {
    http: reqwest::Client,
    base_url: String,
    token: String,
}

impl Moodle {
    /// `MOODLE_URL` + `MOODLE_TOKEN` from the environment, else the saved login.
    pub fn connect() -> Result<Self> {
        let (base_url, token) = match (std::env::var("MOODLE_URL"), std::env::var("MOODLE_TOKEN")) {
            (Ok(u), Ok(t)) => (u, t),
            _ => {
                let saved = load_saved().ok_or_else(|| {
                    anyhow!("not signed in to Moodle yet: the user has to run `moodle-mcp login` once in a terminal")
                })?;
                (saved.base_url, saved.token)
            }
        };
        Ok(Self {
            http: http()?,
            base_url: normalize_base_url(&base_url),
            token,
        })
    }

    pub fn with_token(base_url: &str, token: &str) -> Result<Self> {
        Ok(Self {
            http: http()?,
            base_url: normalize_base_url(base_url),
            token: token.to_owned(),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Calls a web-service function.
    pub async fn ws(&self, function: &str, params: &[(&str, String)]) -> Result<Value> {
        let mut form: Vec<(&str, String)> = vec![
            ("wstoken", self.token.clone()),
            ("wsfunction", function.to_owned()),
            ("moodlewsrestformat", "json".to_owned()),
        ];
        form.extend(params.iter().cloned());
        let resp = self
            .http
            .post(format!("{}/webservice/rest/server.php", self.base_url))
            .form(&form)
            .send()
            .await?;
        let status = resp.status();
        let v: Value = resp
            .json()
            .await
            .with_context(|| format!("{function}: HTTP {status}, not JSON"))?;
        if v.get("exception").is_some() || v.get("errorcode").is_some() {
            let code = s(&v, "errorcode");
            if code == "invalidtoken" {
                bail!(
                    "{function}: the saved Moodle login expired; the user has to run `moodle-mcp login` again"
                );
            }
            bail!("{function}: {} ({code})", s(&v, "message"));
        }
        Ok(v)
    }

    /// Turns any pluginfile link into one that the token can open.
    pub async fn authed_file_url(&self, url: &str) -> Result<reqwest::Url> {
        let mut u = reqwest::Url::parse(url).with_context(|| format!("bad file url {url}"))?;
        if u.path().contains("/pluginfile.php") && !u.path().contains("/webservice/pluginfile.php")
        {
            let p = u
                .path()
                .replacen("/pluginfile.php", "/webservice/pluginfile.php", 1);
            u.set_path(&p);
        }
        if !u.query_pairs().any(|(k, _)| k == "token") {
            u.query_pairs_mut().append_pair("token", &self.token);
        }
        Ok(u)
    }

    pub fn is_own_file(&self, url: &str) -> bool {
        url.starts_with(&self.base_url) && url.contains("pluginfile.php")
    }

    /// Streams a Moodle file to `dest`. Returns bytes written; errors if over `max_bytes`.
    pub async fn download(&self, url: &str, dest: &Path, max_bytes: u64) -> Result<u64> {
        let u = self.authed_file_url(url).await?;
        let resp = self.http.get(u).send().await?;
        let status = resp.status();
        if !status.is_success() {
            bail!("HTTP {status}");
        }
        let ct = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        if let Some(len) = resp.content_length()
            && len > max_bytes
        {
            bail!("too large ({} MB)", len / 1_000_000);
        }
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let tmp = dest.with_extension("part");
        let mut file = tokio::fs::File::create(&tmp).await?;
        let mut stream = resp.bytes_stream();
        let mut written = 0u64;
        let mut first = true;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if first {
                first = false;
                // Moodle answers bad tokens with a JSON error body and 200.
                if ct.contains("application/json") && chunk.starts_with(b"{\"error") {
                    let _ = tokio::fs::remove_file(&tmp).await;
                    bail!("{}", String::from_utf8_lossy(&chunk));
                }
            }
            written += chunk.len() as u64;
            if written > max_bytes {
                drop(file);
                let _ = tokio::fs::remove_file(&tmp).await;
                bail!("too large (> {} MB)", max_bytes / 1_000_000);
            }
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        drop(file);
        tokio::fs::rename(&tmp, dest).await?;
        Ok(written)
    }
}

/// Accepts a course id or any Moodle URL with `id=` / `courseid=`.
pub fn parse_id(input: &str) -> Result<i64> {
    let s = input.trim();
    if let Ok(n) = s.parse() {
        return Ok(n);
    }
    let u = reqwest::Url::parse(s).with_context(|| format!("not an id or URL: {s}"))?;
    for (k, v) in u.query_pairs() {
        if k == "id" || k == "courseid" {
            return v.parse().with_context(|| format!("bad id in {s}"));
        }
    }
    bail!("no id= in {s}")
}

pub fn s(v: &Value, k: &str) -> String {
    match v.get(k) {
        Some(Value::String(x)) => x.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

pub fn i(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}

pub fn arr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}
