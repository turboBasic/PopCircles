---
name: write-plan
description: Author a scratch implementation plan that run-plan can execute - tasks sized to one commit, each ending in a Verify line that can fail. Use when the user asks to write, draft or decompose an implementation plan.
---

# Write a plan

**Acting as architect.** It owns the plan, never the tree the plan is written against. A task quietly
pre-solved here is one the run can no longer discover is wrong.

`docs/ai/platform.md` "Implementation plans" fixes where a plan file lives, what it is for, and its six
sections — there rather than here, because two skills read a plan file and neither may own its shape.
What follows is what that shape cannot say: how work becomes tasks, and what makes a `Verify:` line
worth trusting. Executing the result is the `run-plan` skill.

Every rule below exists because that skill reads the file literally — first unchecked task, top to
bottom, executed as written, gated, committed. So one question sits behind each step: can the executor
act on this without asking the author what was meant?

## Steps

1. **Use `tmp/product-owner-plan.md` as optional context.** If it exists, check whether it says
   anything relevant to this issue. Carry relevant priority, dependency or scope decisions into the
   plan, and flag conflicts rather than silently changing them. If the plan changes something that
   file already tracks, it may be updated to match; otherwise no edit is needed. If it has no relevant
   information, proceed without inferring any.
2. **Start from the issue, not from a blank plan.** The issue and its thread are the decomposition; this
   file is that decomposition made executable, and the two are not allowed to disagree about what the
   work is. Read the whole thread and the roadmap issue above it first —
   `docs/ai/platform.md` "Issues" says why the body alone is not the requirement. Where the plan has to
   depart from the issue, the departure is a comment on the issue before it is a task here.
3. **Name the plan from its issue.** For a request to plan issue `#123`, use `tmp/gh-123/` as its
   directory. In general, for issue number `<issue>` and a short kebab-case description `<slug>`, write
   `tmp/gh-<issue>/gh-<issue>-<slug>.plan.md` and put all plan-specific scratch files,
   evidence and artifacts under `tmp/gh-<issue>/`. These angle-bracket forms are instructions to
   substitute the actual issue number and slug; never write them literally or use the example issue
   number 14 unless that is the issue being planned. Use the same path and name in links.
4. **Measure the tree before drafting.** Read the files the work will touch, run the gates, get the
   counts and the values. Cite what you found: line numbers, measured figures, what a command actually
   printed. A task naming what it will change is executable; a task naming a goal is a wish, and the
   difference only surfaces when someone tries to run it.
5. **Decompose into tasks first, write the sections after.** The task list is the plan and everything
   else is packaging. Order the tasks so each is executable when it is reached — a task needing a later
   one is a stall the executor cannot resolve, because it may not skip ahead.
6. **Size every task to one commit.** Keep tasks executable in order, and make the phase's final tree
   pass `run-plan`'s CI gate. A phase may include a sequence such as declare, then wire, then fill,
   when the final state only works once those tasks land together.
7. **Write each `Verify:` so that it can fail.** Name a command and what its output must show, or a
   state a reader can check and disagree with. "Tests pass" verifies nothing — CI runs at the phase
   boundary. Prefer the assertion that would have caught the mistake you are worried about:
   `rg -n 'scaffolding only'` returns nothing; the totals match the four figures the registry records;
   the same 1 test runs before and after. Prefer too an assertion the work under test cannot satisfy by
   editing itself — "the tolerance assertion passes" is met by widening the tolerance, while "the same 1
   test runs before and after" forces whoever would fake it to break something else.
8. **Give every phase a model and effort.** Start each phase with `Model:` and `Thinking effort:` lines,
   followed by a short justification for both the model and effort. Default to GPT-6-Luna
   (`gpt-6-luna`) and choose the lowest effort that fits that phase's uncertainty and risk. Use another
   model only when Luna is materially unfit for the work, and say why. Judge each phase independently;
   don't copy one setting across phases without considering their different demands. Phases exist where
   a shared prerequisite, a change of subject, or a meaningful review boundary makes grouping useful.
9. **Reserve a findings note at each phase end.** End each phase with `Reviewer findings (execution):`
   followed by `Pending phase review.` This is the only intentional placeholder in the plan: `run-plan`
   replaces it after review with the verdict, findings, resolution commit or follow-up, and verification
   evidence. Do not predict findings while authoring the plan, and do not leave the pending text after
   review.
10. **Make every Ground rule and every Out of scope entry rule something out.** A ground rule earns its
   place by naming a mistake available in *this* work, not by restating a convention the instruction
   layer already binds. An Out of scope entry carries the reason it lost, so a later reader cannot
   mistake the omission for an oversight.
11. **Make the last task land what outlives the file, and judge where it lands.** The plan is scratch and is
   thrown away, so the closing task is what survives it: the issue's checkboxes and the roadmap box ticked,
   the status line set to complete with the date, and every remaining obligation given a home. Which home is
   a judgment rather than a default — a remainder shaped like work someone will do is an issue, and
   `docs/follow-ups.md` takes only an obligation stating a condition the repository can answer, which is the
   bar that file sets. Forcing one into the register for want of anywhere else produces an entry no sweep can
   evaluate; ask where the shape is genuinely unclear. The Follow-ups section then names the identifiers of
   whatever entries exist, and says so when there are none. `run-plan` refuses a plan marked complete, which
   is the only thing keeping a finished plan from being run twice.

## Judgment

- **A `Verify:` line is the hardest part of a plan to fudge, and only when it points outside the change.**
  The task text says what to do and a determined reader can satisfy it badly; a verification aimed at
  something the change may itself edit is satisfiable the same way. Where a task resists a checkable
  verification, that is evidence the task is not yet understood — split it until one is available.
- **Phrase a task as an end state, not an action.** `run-plan` verifies an already-done task and checks
  it off rather than redoing it, and "add the module" is ambiguous the moment the module exists while
  "the module exists, declared in `lib.rs`, with these two functions" is not. Plans get partially
  overtaken by other work; the phrasing is what survives that.
- **A report-only task must land something durable.** Nothing here is committed, so its whole product is
  what it writes elsewhere — a register entry, a term in a list, a corrected document. A report-only task
  that leaves none of those behind evaporates when the file does.
- **A task that would break a non-negotiable does not belong in a plan.** A plan is not authorisation;
  writing one in is how the router's protocol gets bypassed by a document nobody re-reads. Same for a
  task whose verification needs fetched raster content — that gets marked, deselected and given its own
  task, or it is a test CI cannot run.
- **If the tasks keep coming out as "decide X", the decision is not made.** Those belong in the issue
  thread or in conversation, and the ones clearing platform.md's bar belong in a record. A plan carries
  work a ruling settled, and a plan full of open questions is a proposal wearing checkboxes.
- **A scratch file is not a licence to be vague.** The reader is a session with no memory of drafting
  it, which is exactly the reader a committed plan had. Uncommitted means unreviewed, not unspecified.
- **The sections are platform.md's, not yours to vary.** Adding one, renaming one, or copying a filled-in
  template into this file are all the same defect: a second owner for a fact that already has one.
- **A close-out task ticks the issue's checkboxes; it does not close the issue.** Leave the close itself
  to the PR's `Closes #N`, per `docs/ai/platform.md` "Git" — closing it here is the ambiguity that let #2
  get closed by hand hours before the PR carrying its work existed.
