---
name: run-plan
description: Execute a phase of a scratch implementation plan under tmp/ - every task in it verified, checked off and committed on its own, stopping at the phase boundary. Use when the user asks to run, execute, or continue an implementation plan.
---

# Run a plan

**Acting as developer.** It owns executing a plan's tasks as written. It does not re-scope: the issue behind
the plan is the product owner's and the plan's shape the architect's, so a task this role believes wrong goes
back to them.

One invocation advances a plan by one phase. The task stays the unit of verification and the unit of
commit — the phase is only how far the run goes without a human in the loop, and neither a task nor a
phase boundary is crossed with work uncommitted. The loop below is the plan-execution contract;
`docs/ai/platform.md` "Implementation plans" owns what a plan file contains and where it lives, and a
plan's own ground rules add to this loop without replacing it.

The plan file is scratch and uncommitted; the issue it came from is the durable half. So the tick of a
box is a note to this session, and what actually records progress is the commit the task lands and the
issue checkbox the closing task reaches.

## Steps

1. **Locate the plan.** Resolve the number, slug or bare description the user gave against the
   `*.plan.md` files under `tmp/`; if more than one matches, ask rather than guessing. A plan outside
   `tmp/` is not a plan this skill runs — the only home a plan file has is that directory.
2. **Read the status line first.** A plan marked complete is frozen and is never executed — say so
   and stop.
3. **Settle the scope before the first task, never during.** The default is the phase holding the
   first unchecked task, run to its end. Ask up front where the request reads narrower or wider than
   that — one named task, a phase that is not the current one, the rest of the plan — because a scope
   question raised after the second commit is a question about work already landed.
4. **Read the plan's ground rules and the phase heading.** The rules constrain how every task in the
   run is done. A `Model:` note the running session is weaker than stops the run before the first
   task; a stronger session is worth naming and no reason to stop.
5. **Execute the first unchecked task as written.** The task text is the contract. If executing it
   reveals the task is wrong, misordered or already superseded, that ends the run: report it and ask,
   do not silently rescope it and do not step over it to the next task.
6. **Verify** exactly what the task's `Verify:` line asks for, and nothing less. A `Verify:` naming a
   command means running it, not reasoning about what it would print.
7. **Run the gate** — `mise run ci` — and fix what fails. Its lint tasks run the repository-wide
   hooks and language-specific checks; do not run a second full `prek` pass. Stage new files first
   because the hook runner reads tracked files only. Re-stage what a hook reformats and rerun the
   gate; that is expected behaviour, not a defect to investigate. A task's `Verify:` line may ask
   for less than CI; it never licenses less.
8. **Land it, then tick the box, before the next task starts.** Read `docs/ai/platform.md` "Git" before
   the run's first commit rather than after — it constrains which branch may receive one, and a commit on
   the wrong branch is not something the next task can undo. The task's change is one Conventional
   Commit, and the box is ticked in the scratch file once that commit exists. **Never before it** — the
   commit is the durable half, and a tick standing where no commit does is the one state this loop cannot
   recover from. **Never batched to the phase boundary either**, which is the tempting shape because the
   ticks are cheap and the boundary is where the file is read: a run interrupted mid-phase then
   under-reports what landed, and the next one re-derives work already committed.
9. **Take the next unchecked task in the same phase and repeat from step 5.** At the phase boundary,
   stop. Do not roll into the next phase.
10. **Run the `review-change` skill as the `architect` agent over the phase's commits.** Hand it the facts
    that skill's brief asks for, and state the one this loop owns: each task's final tree passed
    `mise run ci` at step 7, and each commit's staged hooks passed during the commit, so no gate in them
    is the review's to re-run. Facts only — what the tasks were and what passed, never why a shape was
    having only from a reader who has not been told. Once for the phase rather than per
    task — per-task invocation multiplies the cost by the task count for a commit that is still amendable
    inside its phase. Every task is committed and its box ticked by now, so a rejection cannot reopen
    anything; what it obliges is one of three answers, and ticked boxes stay ticked because the commit is
    the durable half. The three are a further commit inside this phase, a task appended to the next
    phase, or an entry in `docs/follow-ups.md`.
11. **Report** the tasks that landed, what the review found and how each finding was answered, and what
    the next phase holds. **The phase is not reported complete until every finding has one of those three
    answers or the owner overrides it** — an unanswered finding is the one thing that makes the boundary a
    formality, and a report written before the review is the same thing with the order hidden.

## Judgment

- **A failure ends the run only once it resists fixing.** A failed verification or a red gate is the
  work rather than an exit — diagnose it, fix it, verify again. What ends the run is a failure this
  session cannot resolve: a missing credential, a task built on a decision nobody made, a gate
  demanding something outside the plan. Then the box stays unchecked and the report names the task and
  what failed, because a box checked optimistically makes the plan lie about the repository, and the
  next run believes it.
- **An already-done task is verified and checked off, not redone.** Plans get partially executed by
  other work; confirm the end state the task describes actually holds, then check the box.
- **A report-only task lands no commit of its own.** Its product is the report, delivered in the session
  and durable only where it puts something — a register entry, a term added to a sweep's list, a
  corrected document. Where it puts nothing, say so in the report rather than holding the task open for
  a file change that is not coming.
- **A task that names a non-negotiable is a stop, not a licence.** A plan is not authorisation to
  break one; the router's protocol applies inside a plan exactly as outside it.
- **The last task closes the plan and hands the work to the issue** — the issue's checkboxes and the
  roadmap box ticked, each remaining obligation landed in the home that task judged for it (an issue where
  it is work someone will do, `docs/follow-ups.md` where it states a condition the repository can answer,
  and that file owns the format and the bar), the Follow-ups section naming whatever identifiers resulted,
  and the status line set to complete with the date. Once that is done the scratch file has
  no reader left, and step 2 will refuse it if anyone tries.
