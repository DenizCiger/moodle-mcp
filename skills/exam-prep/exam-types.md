# Exam types

Classify during scope. Mixed exams (e.g. theory questions + a coding task) use several sections; split the time plan by the points each part is worth.

The core loop never changes: diagnose → why → example → trace the teacher's material → explain back → faded practice → spot the error → timed full task → mock exam. What changes is the medium, the drills and how answers are checked.

---

## A. Coding / building (e.g. POSE, SYP practical parts)
**Conventions to extract**: project layout, naming, libraries and versions, how things are wired (DI, MVVM, persistence), the teacher's implementation order.

**Diagnose**: "What runs when the user does X, in what order?" · explain the purpose of 2–3 starter files · one tiny hands-on change.

**Drills**: completion (one method/binding missing) → bigger gaps → from the starter. Check with tests where practical (xUnit for logic/ViewModels, Avalonia.Headless for UI) or a short manual checklist. Break-it drills with typical bugs (wrong binding path, missing change notification, DI not registered, wrong DataContext, async not awaited…).

**Readiness**: timed rebuilds of each assignment from a fresh starter without AI; mock exam = a new app in the same shape, graded against the grading scheme.

---

## B. Calculation and procedures (e.g. BWM/accounting, maths, networking/subnetting, SQL by hand)
**Conventions to extract**: the teacher's solution layout (tables, booking records, which intermediate steps must be shown), notation, rounding rules, formula sheets allowed or not.

**Diagnose**: one easy and one medium task from the material; watch where the procedure breaks (concept, step order, arithmetic).

**Drills**: worked example → same task type with the last steps missing → missing middle steps → full task; then variations that change one parameter or add a twist the teacher likes. Interleave task types once each is solid (the exam mixes them). Check numerically, then step by step: a correct result with missing steps loses points.

**Spot the error**: a worked solution with one wrong step; the student finds and fixes it.

**Readiness**: timed sets of mixed tasks, old tests if available; a one-page procedure card per task type in PLAYBOOK.md.

---

## C. Writing (e.g. Deutsch/English text types, Erörterung, Textanalyse, Zusammenfassung, Leserbrief)
**Conventions to extract**: the task verbs (Operatoren like "analysieren", "erörtern", "beurteilen") and what each demands; required structure per text type; word count; the **grading grid** (Beurteilungsraster) and how points split (content, structure, expression, language correctness). Old graded texts with comments in `notes/` are gold.

**Diagnose**: the student outlines an answer to a real past prompt in 10 minutes; check whether they understood the task verb and the text type's structure.

**Drills** (texts live in `practice/`, marked `.student`):
- analyze a model text: identify its structure and the moves that earn points;
- outline only, for several prompts (fast, high repetition);
- write one part (introduction, one argument paragraph with claim → reasoning → example, conclusion);
- full timed text.

**Feedback**: grade against the grid criterion by criterion with short quotes from their text. Mark problems (structure, missing task-verb coverage, recurring language errors) and give one example of the pattern, but **never rewrite their text**. Track recurring language errors in STUDY.md and turn them into cards.

**Readiness**: full texts under exam time, a personal checklist in PLAYBOOK.md (read task → mark verbs → outline → write → final check for their typical errors).

---

## D. Knowledge and understanding (e.g. theory parts of SYP, NVS theory, history-like content)
**Conventions to extract**: the terms and definitions the teacher uses (in their wording), typical question formats (define, compare, explain why, apply to a scenario), how detailed answers are expected to be.

**Diagnose**: a few "explain why" and "compare X and Y" questions; check whether answers are understanding or memorized phrases.

**Drills**: explain back in their own words; compare/contrast tables; apply a concept to a new scenario; concept maps of how topics connect; short written answers in exam format graded against the expected detail. Cards for definitions and lists, but test understanding with "why" and "what if" questions, not just recall.

**Readiness**: a mixed question set in exam format, timed; a one-page summary per topic written by the student (Claude reviews, doesn't write it).

---

## E. Oral exams
**Conventions to extract**: question pool (if published), expected answer length, whether there's preparation time, whether the student presents something first.

**Drills**: Claude asks like an examiner, in the chat: one opening question, then follow-ups that dig deeper ("why?", "give an example", "what happens if…?") or move sideways to a connected topic. The student answers in full sentences, as they would out loud (typing or dictation). Feedback on correctness, structure (answer first, then reasoning, then example) and the gaps the follow-ups exposed.

**Readiness**: a full mock oral with the real time limit; a list of "if I blank, start with…" anchors per topic in PLAYBOOK.md.
