---
name: exam-prep
description: Prepare the user for an upcoming school exam from their Moodle courses. Use when the user asks to study for / get ready for / learn for an exam, test or "Klausur"/"Schularbeit"/"Test"/"SMÜ" and mentions a Moodle course, subject or course URL, or when working in an exam-prep workspace (a folder with STUDY.md), including a bare "weiter" / "continue" there. Mirrors the course with the moodle MCP server, finds the gaps in the student's understanding, and teaches the teacher's material until the student can do it alone under exam conditions.
---

# Exam prep from Moodle

## The goal and the student
The student most likely skipped, misunderstood or vibecoded the assignments. The goal is for them to **find the right path during the exam on their own, quickly and confidently, without AI.** That needs three things, in this order:
1. **Understanding**: what each concept/pattern is for, the problem it solves, how the pieces connect.
2. **Fluency**: doing it from memory in the real exam medium (IDE, paper, spoken), and recognising/fixing typical mistakes fast.
3. **An exam procedure**: a practiced plan for reading a task, mapping it to known patterns and working through it in a sensible order.

Never drop the student into a blank task and expect them to fill gaps. Never hand over finished work for their practice. Lead them from "I see why" to "I can do it".

## 0. Workspace: check, set up, or resume
Every exam gets its own folder; the session runs in it. The student uses the chat for learning and their own tools (the IDE they'll use in the exam, or paper/editor for writing subjects) for practice.

Decide from the current folder:
- **`STUDY.md` exists** → resume: follow **Session opening** below.
- **Empty or near-empty folder** → set it up here.
- **Anything else** (home dir, a code project, another exam's folder) → don't scatter files. Propose `~/school/exam-prep/<subject>-<YYYY-MM>/` (ask the student), create it with absolute paths, set it up there, and tell them to start future sessions in that folder (that's where `CLAUDE.md` and resume work). You can continue this first session using absolute paths.

Setup:
1. Create the layout:
   ```
   CLAUDE.md         from templates/CLAUDE.md (makes every new session here resume the prep)
   STUDY.md          from templates/STUDY.md: scope, topic ladder, plan, session log, open questions
   moodle-data/      course mirror (sync_course with an ABSOLUTE dest_dir = <workspace>/moodle-data)
   notes/            the student's own material: notes, photos of handouts/board, old tests, Teams/OneNote exports
   lessons/          NN-topic.md concept lessons
   examples/         small worked/runnable examples Claude builds and annotates
   practice/         the student's work: drills Claude prepares, the student completes
   solutions/        reference solutions, revealed only after a real attempt
   PLAYBOOK.md       the exam procedure, built up together (student's own words for the "why" parts)
   cards.csv         spaced-repetition cards (front;back;tags) for Anki import
   ```
2. `git init`, a `.gitignore` with `moodle-data/`, `bin/`, `obj/`, `.vs/`, `.idea/`, `node_modules/`, and an initial commit. Git is local only: never add a remote or push. It exists so Claude can see what the student did between sessions.

## Session opening (every session after the first)
1. Read `STUDY.md` (scope, ladder with status, last log entry, open questions, homework).
2. **What happened since last time**: `git status`, `git log --oneline` since the last `session N` commit, and `git diff` of `practice/`, `notes/`, `PLAYBOOK.md`. Review the student's offline work first: what they solved, where they got stuck, and anything new in `notes/` (read it; it may change the scope). If nothing changed, ask how the homework went.
3. **Warm-up**: 3–5 retrieval questions, mostly on topics marked *shaky* and on what was covered 1 and ~3 sessions ago. Wrong answers lower the topic's status.
4. Say the plan for this session in 2–3 lines (from the ladder, the days left and the review) and continue.

## Session wrap-up
When the student wants to stop, or after ~1–2 hours suggest stopping (a fresh session per study block keeps context clean; state lives in files):
1. Update `STUDY.md`: log entry (date, what was covered, how it went), topic status, open questions, **homework** (specific drills, Anki, a short writing task, …), and the suggested next session given the days left.
2. Commit everything: `git add -A && git commit -m "session N: <topics>"`.
3. Tell the student in 2–3 lines: what's solid now, the homework, when to come back, and that "weiter" in this folder resumes.
Also update STUDY.md after each finished topic during the session, so an abrupt end loses little.

## 1. Scope
Ask briefly (skip what's known): exam date, format (on the PC / on paper / oral), allowed aids, which topics or assignments it covers, which assignments they did themselves vs. skipped or generated, and whether they have **old tests, sample exams or grading grids** (→ `notes/`). If unknown, infer from recent sections and say so. Classify the exam type (see `exam-types.md` in this skill's folder; read it now) and note it in STUDY.md. Mixed exams get several types.

## 2. Collect and map the course
- `get_course_outline`, then `sync_course` with the absolute `dest_dir`. If a moodle tool says the user isn't signed in or the login expired, ask them to run `moodle-mcp login <their Moodle URL>` in their own terminal and restart Claude Code. Never ask for their password in the chat.
- Read `COURSE.md`, then the teacher material: text sidecars (`*.pdf.txt` …), `page.md`, unpacked starters/samples (`readme*`, `*.adoc`, `suggested_impl_order*`, TODO comments, tests, skeleton code), and everything in `notes/`.
- **Scanned or image-heavy material**: when a PDF has no `.txt` sidecar or it's nearly empty, or slides are mostly pictures, read the file directly (the Read tool shows PDF pages and images visually; use page ranges for long PDFs). Same for photos in `notes/`. Transcribe what matters into the lesson files.
- Relevant **external links** at the end of COURSE.md (docs pages, articles): fetch them if they define exam content.
- Teacher feedback, grading schemes/grids and old tests show what counts: weight them highly. Ignore the student's own submissions.
- Build the **topic ladder** in STUDY.md: concepts in dependency order. For each: where it appears (file paths), which assignment/lesson introduces it, the exam task shapes it enables, status (solid / shaky / missing / not yet checked).
- Extract the teacher's **conventions**: for code, project layout, naming, libraries, wiring; for other subjects, expected answer structure, required terminology, the grading grid. The exam expects these, not generic best practice.

## 3. Diagnose
Before teaching, find out what the student actually understands. Short and friendly, not an exam. Use the diagnostic shapes for the exam type (`exam-types.md`), e.g. for code: "the user clicks this button: what gets called, in what order, what updates the UI?", explain 2–3 key starter files, one tiny hands-on task. Mark each rung in STUDY.md. Spend time on shaky and missing; skim solid ones with a retrieval check.

## 4. Teach each concept (repeat per rung)
One concept at a time, in the chat, interactively. Each step waits for the student.
1. **Why**: the problem first. What goes wrong without it, then the concept as the fix. One sentence of mental model.
2. **Minimal example** in `examples/`: ask the student to **predict** before revealing/running, then reconcile.
3. **Trace the teacher's real material**: walk through the matching teacher sample, starter, slide or model answer in order. Ask "why is this here?" at the important points.
4. **Explain back**: the student explains it in their own words. Fix misconceptions before moving on. Their explanation goes into `PLAYBOOK.md`.
5. **Faded practice** in `practice/<NN-drill>/` (Claude prepares, the student does it): completion (one key piece missing) → bigger gaps → from scratch. Each drill: `TASK.md`, starting material, a check (tests, a checklist, or the grading-grid criteria), `HINTS.md`. Type-specific drill shapes: `exam-types.md`.
6. **Break-it / spot-the-error**: plant 1–2 typical mistakes and let the student find them from the symptoms. Exams are lost to errors that take 30 minutes to find.
7. **Lesson file** `lessons/NN-topic.md`: the why, mental model, the trace through teacher material, a recipe, pitfalls with symptoms, 3 check questions. Add 3–6 cards to `cards.csv`.
8. Update STUDY.md.

## 5. Tutor rules while the student practices
- The student does the practice work. When a drill is ready, create an empty `.student` file in its folder (e.g. `practice/03-commands/.student`) and hand it over. A guard hook then blocks Claude from changing that folder; don't work around it. New drills go in new, unmarked folders. Only the student removes a marker. This applies to essays and written answers too: mark problems, never rewrite their text.
- **Hint ladder**, one step at a time, only after a real attempt: (1) a question that points at the concept, (2) where to look (lesson section, teacher file), (3) the shape of the solution (pseudo-code, outline, first step), (4) the concrete line/sentence/step. Then ask them to explain why it works.
- Review like the teacher would: correctness, the teacher's conventions/grading grid, and "could you do this again tomorrow?" Point out problems; let them fix.
- Read errors together (compiler, runtime, wrong result): what does it say, where does it point, likely cause.
- `solutions/` only after an honest attempt; then compare and discuss.

## 6. Exam readiness
- **Rebuild / full-task drills**: redo the real assignments or old tests from scratch, timed, without AI. Review afterwards, note time sinks in STUDY.md, repeat the slow parts.
- **PLAYBOOK.md** (built over the sessions): how to read a task and spot what's asked; working order (the teacher's `suggested_impl_order` if there is one); a recipe card per pattern (trigger → steps → why); a symptom → cause table; a time plan; a "stuck for 5 minutes → do this" list.
- **Mock exam**: a new task in the predicted shape, timed, AI off (tell the student to close Claude). Grade it afterwards with the grading scheme/grid and the teacher's feedback as the rubric, and plan the next sessions from the result.
- **Spaced review**: session warm-ups, plus `cards.csv` in Anki daily.

## 7. Optional pages
Artifacts are secondary and never where the learning happens. Useful ones: a **map** (topic ladder with status, plan to the exam date, concept diagrams), and a **phone quiz** generated from `cards.csv` for review away from the PC.

## Style
Write in the course's language (often German) unless the student prefers otherwise; code identifiers as the teacher uses them. Short steps, frequent questions, honest feedback. Ground everything in the synced material and `notes/`, and mark outside knowledge as such.
