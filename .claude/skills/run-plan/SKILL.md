---
name: run-plan
description: Execute the remaining phases of a scratch implementation plan through completion - every task verified, checked off and committed on its own, with review at each phase boundary. Use when the user asks to run, execute, or continue an implementation plan.
---

# Run a plan

**Acting as developer.** It owns executing a plan's tasks as written. It does not re-scope: the issue behind
the plan is the product owner's and the plan's shape the architect's, so a task this role believes wrong goes
back to them.

One invocation advances a plan from its first unchecked task through completion. The task stays the
unit of verification and the unit of commit; the phase is a review checkpoint, not a stopping point.
Never stop at a phase boundary unless the user explicitly asks you to stop there. Neither a task nor a
phase boundary is crossed with work uncommitted. The loop below is the plan-execution contract;
`docs/ai/platform.md` "Implementation plans" owns what a plan file contains and where it lives, and a
plan's own ground rules add to this loop without replacing it.

The plan file is scratch and uncommitted; the issue it came from is the durable half. So the tick of a
box is a note to this session, and what actually records progress is the commit the task lands and the
issue checkbox the closing task reaches.

## Steps

1. **Locate the plan.** For `gh-<issue>` requests, look first in `tmp/gh-<issue>/`. Otherwise resolve
   the number, slug or bare description against `*.plan.md` files under `tmp/`. If more than one plan
   matches, ask rather than guessing.
2. **Read the status line first.** A plan marked complete is frozen and is never executed — say so
   and stop.
3. **Settle the scope before the first task, never during.** By default, run every remaining phase
   through completion. Stop after a named task or at a named phase boundary only when the user
   explicitly limits the run that way; never infer a phase stop from the plan's structure.
4. **Read the plan's ground rules and the phase heading.** The rules constrain how every task in the
   run is done. A `Model:` note the running session is weaker than stops the run before the first
   task; a stronger session is worth naming and no reason to stop.
5. **Execute the first unchecked task as written.** The task text is the contract. If executing it
   reveals the task is wrong, misordered or already superseded, that ends the run: report it and ask,
   do not silently rescope it and do not step over it to the next task.
6. **Verify** exactly what the task's `Verify:` line asks for, and nothing less. A `Verify:` naming a
   command means running it, not reasoning about what it would print.
7. **After the phase's tasks are committed, run the gate** — `mise run ci` — and fix what fails. Its
   lint tasks run the repository-wide hooks and language-specific checks; do not run a second full
   `prek` pass. Stage new files first because the hook runner reads tracked files only. Re-stage what
   a hook reformats and rerun the gate; that is expected behaviour, not a defect to investigate. A
   task's `Verify:` line may ask for less than CI; it never licenses less.
8. **Land it, then tick the box, before the next task starts.** Read `docs/ai/platform.md` "Git" before
   the run's first commit rather than after — it constrains which branch may receive one, and a commit on
   the wrong branch is not something the next task can undo. The task's change is one Conventional
   Commit, and the box is ticked in the scratch file once that commit exists. **Never before it** — the
   commit is the durable half, and a tick standing where no commit does is the one state this loop cannot
   recover from. **Never batched to the phase boundary either**, which is the tempting shape because the
   ticks are cheap and the boundary is where the file is read: a run interrupted mid-phase then
   under-reports what landed, and the next one re-derives work already committed.
9. **At each phase boundary, review, then continue.** First run the `ponytail:review` skill over the
   phase's commits and capture its complete output. Then run `review-change` as the `architect` agent,
   passing it that output and the facts its brief asks for. State that the phase's final tree passed
   `mise run ci` at step 7, and each commit's staged hooks passed during the commit, so neither review
   needs to rerun those gates. Review once per phase, not per task.
10. **Account for every review finding before proceeding.** A rejection or finding is work to address,
    not a reason to abandon the plan. Make a further commit in this phase, append a task to a later
    phase, or record an obligation in `docs/follow-ups.md`, as appropriate. Verify any fix and rerun
    the phase gate if its tree changed; then repeat both reviews on the changed phase commits and pass
    the updated ponytail report to the Architect. Do not start the next phase until both reviews are
    recorded, every finding has one of those three answers, and the final tree passes CI. A failure
    this session cannot resolve ends the run with the current task unchecked or phase unresolved.
11. **Record the execution reviews at the end of the phase in the plan.** Replace its
    `Reviewer findings (execution): Pending phase review.` note with both verdicts, their findings,
    how each was resolved (commit or follow-up), and verification evidence. Do not leave the pending
    note once review is complete.
12. **Continue with the next unchecked task**, including the first task of the next phase, unless the
    user explicitly asked to stop at this boundary. At plan completion, report the tasks that landed,
    phase review results and finding resolutions, and the final plan status. Do not report the plan
    complete while any phase has an unresolved finding or failed gate.

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
