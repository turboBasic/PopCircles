---
name: review-change
description: Review a landed change as architect - what in the diff decided whether it passed, then the diff against the non-negotiables, the layering rule, the comment and documentation rules, and whether it warranted a record. Use when a phase of a plan has landed, before a run reports, or when the user asks for an architecture review of a diff, a branch or named commits.
---

# Review a change

**Acting as architect.** The stance, and what it refuses, is [`architect`](../../agents/architect.md): it
owns whether the change is the right shape, rejecting it is an available verdict, and it does not implement.
The limit this workflow adds is that the deliverable is the verdict — a finding is answered by whoever holds
the tree, in a further commit, a task on the next phase, or an entry in `docs/follow-ups.md`.

The four sections every finding is measured against are that file's, and this one does not repeat them.

## What is already settled

The persona's rule is not to re-derive what a gate settled. This is how to know what that covers without
running anything — **a review's budget goes to what no gate can see, and re-checking L0 is duplicating a
layer.**

**A commit implies its hooks ran** on the files each one matches: the formatters, `cargo fmt`,
`cargo clippy`, cspell, markdownlint, the doc-pointer and structure-tree check, the two exemption guards,
the large-file and registry checks, and commitizen on the message. `.pre-commit-config.yaml` is the roster.

**A commit implies nothing about typecheck or tests.** No hook runs either; only `mise run ci` does. Those
are the two facts a caller has to state, and absent a statement they are unknown.

**A phase run by `run-plan` implies both**, because its tasks' commits run staged hooks and its step 7
runs `mise run ci` on the phase's final tree.

**A pushed ref needs no claim at all.** `gh api repos/:owner/:repo/commits/<sha>/check-runs` is
authoritative in one call, and it outranks anything a brief says.

**`--no-verify` is undetectable from a commit, and costs nothing to catch anyway.** A bypass leaves what the
hooks would have fixed — unformatted code, a spelling the dictionary refuses, a stale structure tree — and
that is visible while reading the diff. Finding one is a finding. Re-running the gate to look for one is not
this review's work.

## What the brief may say, and what it may not

A caller hands over facts. Anything absent from the brief is **unknown, not settled**, and any of these that
is missing and matters is asked for rather than assumed:

- the commit range or ref under review;
- whether `mise run ci` passed, at which commit, and whether the tree was clean;
- anything bypassed, and how;
- which touched files are generated rather than authored;
- the `Verify:` line of the task each commit lands under;
- the issue, record or follow-up entry the change claims to satisfy.

**It may not carry the implementer's reasoning** — why that shape was chosen, what alternatives were
rejected, or any self-assessment of the change. Reaching that independently is the whole reason this review
runs in its own context, and a brief that supplies it hands over a conclusion dressed as a fact.

**A false claim in the brief outranks every other finding**, and is reported first. It is not a slip to
absorb: the review's economy rests on those statements, so a wrong one is the most expensive thing in the
range.

## Ask first whether the change moved its own goalposts

Before anything else, look at what in the diff decides whether the change passes. That surface is small and
worth naming: a test's expected value, a tolerance, a test moved behind a deselect marker, an `#[allow]` or
a `# type: ignore`, a lint level in `Cargo.toml` or `pyproject.toml`, a gate's configuration, and the
`Verify:` line of the task the commit lands under.

Touching that surface is not the finding — a tolerance is sometimes wrong, a scope is sometimes too wide,
and a `Verify:` line sometimes cannot be run as written. The finding is touching it *as the way* the task
became satisfiable. Say which of the two this diff is: a change made because the criterion was wrong, or a
criterion changed because the code would not meet it. State that reading explicitly even when the answer is
the first one, because a review that stays silent here reads as a review that did not look.

## What to read

The diff first and the tree second, then each of the architect's four sections against what the diff did.

## What not to read

The surface is the diff, the files it touched, and those four sections. Anything else is read only because
the diff points at it, and a review that reaches past this bound spends its budget where it has no verdict
to reach.

- **History is not the surface.** `git log`, `git show` and `git blame` are for a commit *in the range under
  review*. What the tree used to be belongs to `docs/decisions/` and the issue thread, so a finding needing
  archaeology to state is a finding about a record rather than about this change.
- **Read the cited section, not its file**, and a touched file rather than its neighbours.
- **A whole-tree sweep is the `housekeeping` skill's**, and asking for one is an available verdict. The
  exception is a sweep the diff itself claims — a change asserting that no document mentions something is
  verified by running that sweep once.

## The verdict

One of three, stated outright rather than left to be inferred:

- **Accept** — nothing found, said plainly. A review that promotes a near-miss to fill the list is worse
  than a clean one.
- **Accept with findings** — each finding named, each with what it costs if left. Say which are worth a
  commit now and which are worth an entry in `docs/follow-ups.md`.
- **Reject** — the change is the wrong shape, and the reason is structural rather than a matter of taste.
  Name what shape it should have had.
