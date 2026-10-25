# AI Instructions — router

## Invariants

These apply to every task:

- Never commit a raster, a generated summation table, a rendered map, or a credential.
- Consult accepted records in `docs/decisions/` before an architecture or implementation choice.
  Supersede one with a new record rather than contradicting it.
- Documentation moves with the change that invalidates it. Stale framing is a defect, not a
  follow-up.

The first shadows a rule `.gitignore` already carries. That duplication is deliberate and bounded to
this list, which no other file may extend: a pushed raster fails in a way a later commit cannot undo,
and this list is where attention lands first.

**The list does not grow by judgment call.** A rule that feels important enough to add here almost
always has a lower layer that already owns it, and belongs there instead; adding it here is how the
one sanctioned duplication turns into a general licence. A genuine addition needs a record in
`docs/decisions/` saying why the bypass it guards is unrecoverable.

## Non-negotiables

A rule is non-negotiable when breaking it is irreversible, weakens security, or erases a module
boundary: any invariant above, `unsafe` code, a blanket `#[allow]` or `# type: ignore`, loosening a
lint level or a tool mode to clear an error, regenerating committed config, an unpinned CI action.

Treat a change to one as a design change, not a task. Before implementing it, in a short paragraph:
name the rule, state concretely what breaks without it, and offer the smallest alternative that
still meets the underlying need. Then stop and wait.

- **Report the conflict even when it is incidental.** A change that erodes one as a side effect gets
  the same treatment as a request to drop it outright. Drift is how they are actually lost.
- **Once the objection is heard and the request restated, implement it fully.** Do not relitigate,
  hedge the implementation, or leave the old path in place as a safety net.
- **Never weaken one silently** to make a task easier.
- Do not object over conventions: line length, naming, file placement, or how a test is organised.

## Working style

Read the file, run the tool, check the config rather than guessing at structure or conventions. Ask
when genuinely ambiguous; take the sensible default otherwise and say so. Match existing patterns
over personal preference. Scope to the request — no refactoring adjacent code or improving what was
not asked about.

Add no prose the change does not owe. Explanation earns its place by holding a WHY the file cannot
show on its own; if it restates the code, the config or the diff, cut it. History is never such a
WHY: `git log` and `docs/decisions/` own what changed and what it replaced, so a comment or doc
written as before-and-after narration belongs there or nowhere. Growth is this repo's standing
failure mode: when the explanation of a change outweighs the change, the explanation is what goes.

## Layering

Instruction files are layered, and **the lowest layer that can hold a fact owns it; every layer
above links to that owner instead of repeating it.** To test a suspected restatement, delete the
sentence mentally and ask whether a route or a fact was lost: if a route, it belongs; if a fact, the
owner already holds it. Lowest first:

| Layer | Holds | Where |
| --- | --- | --- |
| L0 enforced | facts a machine checks | `mise.toml`, workspace lints in `Cargo.toml`, `.pre-commit-config.yaml`, `.gitignore`, `.github/workflows/` |
| L1 records | why a constraint exists, and what lost to it | `docs/decisions/` |
| L2 router | project invariants and the layering rule | this file |
| L3 domain docs | judgment no tool can enforce | `docs/ai/` |
| L4 skills and personas | one task's workflow and the judgment that task needs, or a standing stance and what it refuses | `.claude/skills/*/SKILL.md`, `.claude/agents/*.md` |
| L5 entry points | routing only | `CLAUDE.md`, `AGENTS.md`, `.codex/agents/*.toml`, `.github/copilot-instructions.md`, `opencode.json` |

Within L3, [`ai/platform.md`](ai/platform.md) wins wherever it and an application doc would
conflict. The human layer includes [`README.md`](../README.md), [`USAGE.md`](../USAGE.md),
[`PERFORMANCE.md`](../PERFORMANCE.md) and [`CONTRIBUTING.md`](../CONTRIBUTING.md). They may restate
what these files own, each earning it by a way of being read that no owner serves: README is read
first, USAGE with a terminal open and returned to, PERFORMANCE for technical details about runtime
and search work, and CONTRIBUTING by someone preparing a change. Where any of them and the instruction
layer disagree, the instruction layer governs.
`data/registry.toml` is the dataset registry and owns what a machine reads of each dataset;
`data/README.md` owns the provenance and the fetching mechanics, and is a restater of the rest — the
split [ADR 0009](decisions/0009-input-data-is-published-not-versioned.md) took.
Those are the only licences to restate an owned fact; anywhere else, a restatement is
a defect to correct at the owner. Granting a further one takes a record in `docs/decisions/`, and the
bar is that reader property. That a human might read the file is true of everything here and grants
nothing.
