# moodle-exam-prep

A personal exam tutor for Claude Code that works from your own Moodle courses.

It downloads what your teacher posted (files, assignment descriptions, starter projects, links, feedback), works out what you already understand and what you don't, and teaches the gaps step by step. You do the practice work yourself, so you can do it alone in the exam. Everything it builds is yours: your diagnosis, your study plan, your drills, your notes. Two people using the same course end up with different setups.

It has three parts, installed together as one Claude Code plugin:

| Part | What it does |
|---|---|
| `moodle-mcp` (small program) | Read-only access to your Moodle: list courses, show a course's structure, mirror a whole course to disk with text versions of PDFs/Office files and unpacked `.zip` starters. |
| `exam-prep` skill | The tutor: sets up a study folder, diagnoses you, teaches concept by concept (why → example → teacher's code → explain it back → practice), runs mock exams, remembers progress between sessions. |
| Guard hook | Stops Claude from writing your practice work for you once a drill is handed to you. |

> Setting this up with an AI agent? Give it this repository and tell it to follow [AGENT_SETUP.md](AGENT_SETUP.md). You'll still have to do step 2 yourself.

## Setup (about 5 minutes)

### 1. Install the `moodle-mcp` program
Pick one:
- **Download**: get the file for your system from the [latest release](https://github.com/DenizCiger/moodle-mcp/releases/latest), unpack it, and put `moodle-mcp` (`moodle-mcp.exe` on Windows) in a folder on your `PATH`.
- **Build it** (needs [Rust](https://rustup.rs)):
  ```sh
  cargo install --git https://github.com/DenizCiger/moodle-mcp
  ```

Check it worked: `moodle-mcp --version`

Optional: install `pdftotext` (part of poppler: `brew install poppler`, `sudo apt install poppler-utils`, `scoop install poppler`) for better PDF text. Without it, Claude reads PDFs directly, which also works.

### 2. Sign in to Moodle (you, in a terminal)
```sh
moodle-mcp login https://edufs.edu.htl-leonding.ac.at/moodle
```
Enter your Moodle username and password. The password goes to Moodle only and is **not saved**; `moodle-mcp` keeps just the access token Moodle gives back (the same kind the Moodle phone app uses), in your user config folder. Check with `moodle-mcp status`.

Never type your password into a chat with Claude or another agent. The login command is the only place it goes.

### 3. Install the plugin in Claude Code
In Claude Code, run:
```
/plugin install moodle-exam-prep --marketplace DenizCiger/moodle-mcp
```
(or `/plugin marketplace add DenizCiger/moodle-mcp`, then `/plugin install moodle-exam-prep@moodle-mcp`). Restart Claude Code afterwards.

## Using it

**First session.** Make an empty folder for the exam (e.g. `~/school/exam-prep/pose-2026-12`), open Claude Code in it, and say what you need, e.g.:

> Prepare me for the POSE exam on 10 December. Course: https://edufs.edu.htl-leonding.ac.at/moodle/course/view.php?id=5154

Claude will ask a few questions (exam format, what's covered, which assignments you did yourself or skipped), sync the course, check what you already know, and start teaching. Be honest about skipped or AI-written assignments; that's exactly what it's for.

**What ends up in the folder**

| | |
|---|---|
| `STUDY.md` | your topics with status, plan to the exam, session log, homework |
| `lessons/` | a short lesson per topic, written after you've worked through it |
| `practice/` | your drills. Folders with a `.student` file are yours; Claude only gives hints and reviews there |
| `PLAYBOOK.md` | your exam procedure and checklists, partly in your own words |
| `notes/` | put your own material here: notes, photos of handouts, old tests, grading grids |
| `cards.csv` | flashcards to import into Anki |
| `moodle-data/` | the course mirror (re-syncable, not part of your work) |

**Next sessions.** Open Claude Code in the same folder and say "weiter" (or anything). It reads where you left off, looks at what you practised since, asks a few review questions, then continues. Start a fresh session per study block (1–2 hours) instead of one endless chat. At the end it updates `STUDY.md`, gives you homework, and saves a local git commit.

**Between sessions**: do the drills in your own IDE/editor, and your Anki cards.

**Before the exam**: it runs timed rebuilds and a mock exam. Do those with Claude closed, like the real thing.

Works for other subjects too (writing, calculation, theory, oral exams); it picks the approach from the exam type.

## Privacy and safety
- Read-only: it never submits, posts or changes anything on Moodle.
- It doesn't download your own submissions.
- The token in your config folder gives access to your Moodle account like the phone app does. Don't share that file. `moodle-mcp logout` deletes it; you can also revoke tokens in Moodle under Preferences → Security keys.
- Course files land in your study folder and are read by Claude like any other file you open with it.

## Troubleshooting
| Problem | Fix |
|---|---|
| Claude says it's not signed in / login expired | Run `moodle-mcp login …` again, restart Claude Code |
| `moodle-mcp: command not found` | The program's folder isn't on your `PATH` (cargo installs to `~/.cargo/bin`) |
| Login fails although the password is right | If your school signs in through another service (SSO), create a token in Moodle → Preferences → Security keys and set the environment variables `MOODLE_URL` and `MOODLE_TOKEN` instead |
| Tools don't show up in Claude | `/plugin` to check it's enabled; restart Claude Code |

## Command reference
```
moodle-mcp login [url]           sign in (interactive)
moodle-mcp status                show the configured account
moodle-mcp logout                delete the saved login
moodle-mcp sync <course> [dir]   mirror a course from the terminal
moodle-mcp ws <function> k=v...  raw web-service call (debugging)
```
MCP tools: `list_courses`, `get_course_outline`, `sync_course`, `get_assignment`, `download_file`.
