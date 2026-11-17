# moodle-mcp

A read-only [MCP](https://modelcontextprotocol.io) server for Moodle. It lets AI agents (Claude Code, Claude Desktop, Cursor, or any other MCP client) browse your courses and mirror course material to disk as files they can read.

- Works with any Moodle site that has the mobile web service enabled (most do, since the Moodle app needs it).
- Read-only: it never submits, posts or changes anything.
- Signs in once with your username and password. Only the access token is stored, never the password.

Looking for the exam tutor built on top of this? See [moodle-exam-prep](https://github.com/DenizCiger/moodle-exam-prep).

## Tools

| Tool | What it does |
|---|---|
| `list_courses` | Your enrolled courses, most recently accessed first |
| `get_course_outline(course)` | Sections and modules (files, folders, pages, links, assignments with due dates) |
| `sync_course(course, dest_dir?, include_feedback?, max_file_mb?)` | Mirrors a course to disk: a `COURSE.md` index with all teacher text (section summaries, labels, pages, assignment descriptions, feedback, announcements, links), every file, `.txt`/`.md` text versions of PDF/Office/HTML files, and unpacked `.zip` archives (build folders skipped). Re-runs only download what changed. |
| `get_assignment(assignment)` | Description, instructions, attachments, due date, grade and feedback |
| `download_file(url, dest_dir?)` | One Moodle file to disk, with text extraction or unpacking |

Courses and assignments can be given as numeric ids or as Moodle URLs. Your own submissions are not downloaded.

## Install

**Prebuilt**: download the archive for your system from the [latest release](https://github.com/DenizCiger/moodle-mcp/releases/latest) (check it against its `.sha256`), unpack it, and put `moodle-mcp` (`moodle-mcp.exe` on Windows) in a folder on your `PATH`.

**From source** (needs [Rust](https://rustup.rs)):
```sh
cargo install --git https://github.com/DenizCiger/moodle-mcp
```

Optional: `pdftotext` (poppler) for PDF text versions: `brew install poppler`, `sudo apt install poppler-utils`, or `scoop install poppler`. Without it, PDFs are still downloaded, just without a `.txt` next to them.

## Sign in
Run this yourself in a terminal (it asks for your password, so agents can't and shouldn't do it for you):
```sh
moodle-mcp login https://your-school.example/moodle
```
Any page URL of your Moodle works. Check with `moodle-mcp status`; `moodle-mcp logout` deletes the saved login.

The token is saved in your user config folder (`~/.config/moodle-mcp/config.json` on Linux, the equivalent on macOS/Windows; override with `MOODLE_MCP_CONFIG`). It gives access to your Moodle account like the Moodle phone app does, so don't share it. You can revoke tokens in Moodle under Preferences → Security keys.

**SSO sites**: if your Moodle signs in through another service and `login` fails, create a token in Moodle → Preferences → Security keys and set `MOODLE_URL` and `MOODLE_TOKEN` in the environment instead.

## Connect your MCP client
**Claude Code**
```sh
claude mcp add --scope user moodle -- moodle-mcp
```

**Claude Desktop, Cursor and other clients**: add a stdio server:
```json
{
  "mcpServers": {
    "moodle": { "command": "moodle-mcp" }
  }
}
```

`sync_course` writes to `dest_dir`, or `$MOODLE_MCP_DIR`, or `./moodle-data` in the server's working directory. Passing an absolute `dest_dir` is the most predictable.

## Command line
```
moodle-mcp login [url]           sign in (interactive)
moodle-mcp status                show the configured account
moodle-mcp logout                delete the saved login
moodle-mcp                       run the MCP server on stdio
moodle-mcp sync <course> [dir]   mirror a course from the terminal
moodle-mcp ws <function> k=v...  raw web-service call (debugging)
```

## License
MIT
