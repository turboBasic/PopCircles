# Platform

Read this when changing tooling, dependencies, input data, quality gates, git conventions or CI, when
working a GitHub issue, when deciding whether a change warrants a record under `docs/decisions/` or
writing the plan that carries it, or when the structure tree moves.

Committed configuration is authoritative for what it declares — read `mise.toml`, `Cargo.toml`,
`pyproject.toml` and `.pre-commit-config.yaml` rather than assuming. Extend those
files; never regenerate them. What follows is the judgment around them, not a second copy of them.

## Tooling hierarchy

1. **A mise task** — `mise run <task>`. Never bypass one that exists.
2. **prek** — `prek run`. Hooks run through prek, not pre-commit; the header comment in
   `.pre-commit-config.yaml` says why the filename is what it is.
3. **`cargo <cmd>` / `uv run <tool>`** — the language-local tools.
4. **`mise exec -- <tool>`** — system tools mise manages.

`mise.toml` is the roster of both the pinned tools and the tasks: what `lint`, `typecheck`, `test`
and `ci` actually run is declared there, and prose repeating a task's command line is a drift site.
`mise run ci` reproduces CI locally and is the check to run before reporting work done.

Nothing is installed globally. A new runtime or CLI is pinned in `mise.toml`. Never `pip install`,
never activate a venv by hand, never install a Rust toolchain outside mise.

A task deliberately not yet wired into `lint` or `ci` carries a comment saying why. Believe the
comment and satisfy its stated condition before wiring the task in — the two Python tasks are
waiting on sources and tests to exist, not on someone noticing them.

## AI harnesses

Resolve reused skills' and personas' relative paths from their real source files. Follow their
workflows with the current harness's tools; Claude Code frontmatter, hooks and permission settings
do not configure another harness. A persona's editing limits remain instructions in Codex, whose
custom agents inherit the parent's permissions rather than Claude Code's withheld tools.

When a workflow requests a persona, use its custom agent. Where the harness cannot select a named
agent, use a fresh subagent instructed to read the source persona and the workflow's brief.

## Dependencies

- Rust: workspace-level settings live in the root `Cargo.toml` (`[workspace.package]`,
  `[workspace.lints]`); a crate inherits with `field.workspace = true`. Commit `Cargo.lock`.
- Python: dev tooling in `[dependency-groups].dev`. No `setup.py`, `setup.cfg` or
  `requirements.txt`. Run `uv lock` after editing dependencies and commit the result in the same
  change.
- Node: a CLI host only, never an application dependency. Pin `node` and the tool itself in
  `mise.toml`'s `[tools]` via the `npm:` backend (e.g. `"npm:markdownlint-cli2" = "0.23.2"`);
  never `npm install -g`. Its prek hook calls the pinned binary directly (`language: system`),
  the same shape `cargo-fmt`/`taplo-fmt` use, rather than letting the hook manage its own runtime.
- Before adding a dependency, check whether one already in the tree covers the need. Prefer the
  standard library for anything small.
- Introducing a new file type or framework updates `.editorconfig`, `.gitattributes` and
  `.gitignore` in the same change.

## Large input data

Input datasets live in `data/`, one directory per kind. A large one is published and fetched rather
than carried in the repository, a small one is committed.
[`data/registry.toml`](../../data/registry.toml) is the registry and owns each dataset's grid, CRS,
nodata value, byte length, checksum, fetch URL, licence and the attribution a figure owes — it is what
`mise run data:get`, the CLI and the renderer read, and naming a row is how a build says what to open. [`data/README.md`](../../data/README.md) is the same datasets
described for a person, and owns the provenance that identifies each one, plus the mechanics of fetching
them ([Getting it](../../data/README.md#getting-it)). A dataset gets its row in both, in the same change
that adds it.

The judgment around that mechanism:

- A new **input** dataset goes to `data/<kind>/` with a registry entry, and only deliberately.
  Generated products are neither committed nor placed there.
- **Publishing is the answer for a raster, not for `data/` by location**
  ([ADR 0009](../decisions/0009-input-data-is-published-not-versioned.md)). A dataset every clone and
  every CI job needs, small enough that fetching it would cost more than carrying it, is a Git blob —
  the registry states that trade for each row, and `check-added-large-files` is what still bounds it.
- Code that reads a raster fails with a clear message naming `mise run data:get` when the file is
  absent, rather than reporting it as a format error.

## Quality gates

### Linting

- prek is the entry point for hooks; `cargo clippy` also runs through its own mise task. Never call
  `ruff` directly.
- clippy warnings are errors. Never silence one with a blanket `#[allow]`; a narrow, commented
  `#[allow]` on a single item is acceptable when the lint is genuinely wrong.
- ruff for Python lint and format. Never add black, isort, flake8 or pylint.
- TOML is formatted with taplo.
- When a hook reformats files, re-stage and re-run. That is expected behaviour, not an error to
  investigate.
- cspell checks every tracked file. A legitimate term it flags goes in `.cspell/project.txt`, in the
  section it belongs to — never an inline ignore.
- Markdown is linted with markdownlint-cli2, `docs/decisions/` included; `.markdownlint-cli2.jsonc`
  owns which rules are disabled globally versus scoped to one path, and why.

### Type checking

Never use a blanket ignore or loosen a mode to clear an error. Narrow per-line ignores are
acceptable only at a library boundary, with the reason stated. Python type checking is pyright in
strict mode.

### Testing

- Rust: unit tests in the module under `#[cfg(test)]`, integration tests in `tests/`. Property tests
  are welcome for the geometry and summation invariants.
- Python: pytest. Never `unittest.TestCase` classes.
- What the numeric code's tests must pin, as against merely exercise, is
  [`application.md`](application.md) "Correctness invariants".
- **Never make a test depend on a fetched raster.** The suite runs on a clone with no raster in it, so
  a test that needs raster bytes to pass is a test CI cannot run. Build the fixture in code, or
  decimate one small enough to commit.
- Tests needing network, real credentials or fetched rasters are marked, deselected by default, and
  given their own task. They never run in CI.
- Do not run the suite after every edit — run it when asked or when verifying a fix. A test failing
  after a change is fixed before the work is reported done.

## Architecture decisions

Consulting accepted ADRs before an architecture choice is a project invariant, stated in
`docs/ai-instructions.md`. This section governs when one is warranted and how it is scoped; the
record's own shape is the `write-adr` skill's.

**The trigger is a question, not a work package.** Working an issue forces dozens of choices at every
level of impact, and recording them because an issue was worked is what
[ADR 0001](../decisions/0001-a-record-carries-one-ruling.md) was written against. Most issues warrant
no record; one may warrant two, having raised two independent questions; and a question raised in
conversation with no issue behind it warrants one just the same.

### The bar

A change needs a record only when all three hold:

1. **Reversing it costs more than a PR.** It crosses a crate or language boundary, changes a published
   format, or changes a project-wide policy.
2. **A competent person would have chosen differently.** There was a real option, not a preference.
3. **Someone will ask "why is it like this?" and not be able to answer from the code.**

Two corollaries do most of the work in practice:

- **A choice a gate already pins does not need a record.** If a test asserts the tolerance or a
  manifest confines the dependency, the gate is the record.
- **A record whose own Consequences say it is cheap to reverse has failed test 1**, and the draft
  admitting that is the signal to stop rather than a paragraph to soften.

### Scope

1. **One record, one ruling, one `scope:`.** A numbered list of decisions is the symptom that the
   record is more than one record.
2. **Rule the constraint, not the implementation of it.** "The build takes no non-Rust prerequisite"
   survives a crate swap; the crate and its feature flags do not, and protect the same thing.
3. **Never enumerate a schema, a field list or a file layout.** State the property the artefact must
   have; the fields belong to the code and its version constant. A record that lists them guarantees a
   record when the list changes.
4. **The record holds the why; the instruction layer holds the rule.** `docs/ai/` says what is true
   now, in present tense and editable. The record says what was chosen, what lost and when, frozen.
   Neither restates the other at length, and the link between them is one line.
5. **One page — 80 lines including frontmatter, hard.** Not a style preference: it is the forcing
   function for the four rules above. What will not fit is the implementation, which is the PR, the
   evidence, which is the issue, or a second record.

A measured figure belongs to whatever measured it — the issue thread and the PR are already dated and
frozen. The record cites inline the one number that decided it, and carries no tables.

### Mechanics

- A record is `docs/decisions/NNNN-<kebab-slug>.md`, taking the lowest unused prefix.
- An accepted record is superseded by a new one, never edited. Its `status:`, its `superseded_by:` and
  one line under its title are the only sanctioned edits.
- A record records a decision **made**. A ruling that cannot carry a date is a proposal, and belongs in
  the issue thread or in conversation until it is settled.
- **Every record in the directory satisfies this section**, so any of them reads as the shape a new one
  takes and none is exempt from the ceiling.

## Implementation plans

**Work decomposition is GitHub issues**, one issue per deliverable, tracked from the roadmap issue that
is its milestone's epic. That is the only durable home it has, and a committed task list anywhere in
this tree — beside a record included — is the drift to avoid
([ADR 0001](../decisions/0001-a-record-carries-one-ruling.md)).

A **plan file** is the executable form of one issue's work: the same steps rewritten as tasks a skill
can run, each with a verification that can fail. It is scratch — written under the gitignored `tmp/`,
executed, and never committed. What outlives it is the issue's ticked boxes and the commits it
produced. Two skills read one, `write-plan` and `run-plan`, so its shape is fixed here rather than in
either.

- **No frontmatter.** Nothing indexes a file that is never committed.
- **Status line** is the paragraph directly under the title, opening
  `**Status: in progress (YYYY-MM-DD).**` or `**Status: complete (YYYY-MM-DD).**`. Those two values are
  all there are, and a plan marked complete is never executed or ticked off again.
- **Ground rules** constrain how every task in that plan is done. They add to the executing skill's
  loop and never replace it.
- **Out of scope** records what was weighed and deliberately left out, so a later reader does not
  mistake an omission for an oversight.
- **Phases** group tasks and may carry a `Model:` note naming the model the phase expects.
- **Tasks** are checkboxes numbered `<phase>.<task>`, each ending in a `Verify:` line stating what
  proves it done. The task is the unit of execution and the unit of commit.
- **Follow-ups** hold identifiers only: the obligations the work produced are entries in
  [`../follow-ups.md`](../follow-ups.md), which owns their format, statuses and the bar their
  conditions must meet.

## Issues

An issue is worked from its whole thread, never its body alone. The body is the opening position; the
comments are where scope was cut, a figure settled or a step reordered, and none of that gets folded
back up into the body. Read them, and the roadmap issue this one hangs from, before starting — else the
requirement implemented may be one withdrawn three comments in.

The agent working this repository from Actions has no `gh` at all — `opencode.json` denies it wholesale,
reads included. Read an issue and its thread there through `WebFetch` against
`api.github.com/repos/:owner/:repo/issues/<n>` and `.../issues/<n>/comments`, rather than retrying
`gh issue view`.

**Proposing a change of scope or requirements is always in bounds** — for the issue in hand and for any
other open issue a discovery affects. Executing a step is what exposes that a later one is unnecessary,
misordered or resting on something untrue, and whoever hits that is the only one positioned to say so.
Raising it needs no permission asked for first and is never overstepping. Sitting on it costs the
discovery, which is then re-found later without the context that made it visible.

The licence covers the proposal, not the change. It lands as a comment on the issue whose scope would
move — for a downstream discovery that is that issue, not the one in hand — and the scope moves once the
proposal is agreed. Quietly building something other than what the issue asks for is the failure this
permission exists to make unnecessary, not the one it grants.

**A proposal comment whose proposals have all landed is edited to open with `**Discharged YYYY-MM-DD** —`
and what landed**, naming each finding where it carried several. That is the whole of the marker: a
reader skipping the comment knows why, and nobody adds a fourth comment to report on the third. It is
dated and says what happened rather than tagging a status, because a status has to be maintained and a
reverted change would leave `[resolved]` lying where a dated sentence stays true. **Only a comment that
is wholly a proposal takes one** — a scope review, a structural finding, a "this should move". A comment
carrying a decision or a measured figure is never discharged, and marking one tells a reader to skip the
context they most need.

### Relationships are structural, never prose

**A relationship GitHub models is set through its API, not written into a body.** Containment is the
sub-issue link, and blocking is the dependency —
`gh api repos/:owner/:repo/issues/<n>/dependencies/blocked_by -F issue_id=<id>`, read back from either end
with that path or `dependencies/blocking`. A `- Blocked by: #57` line under a `## Relationship` heading is
the same fact in a second place, and the copy that rots: the panel updates when an issue closes, is
renamed or is superseded, and the sentence does not.

`- Relates to:` stays prose, because GitHub models nothing for it. That is the whole of what the heading
is for, so a `## Relationship` section holding only blocking lines goes when they become links.

### Milestones, epics and labels

A milestone is one release increment, and it holds exactly one roadmap issue's sub-issues. That issue is the
milestone's epic and its body describes what the milestone contains — an epic still describing what it used
to contain is how a milestone acquires a second theme and twice the size without anyone deciding to. A
sub-issue is one deliverable: its `## Goal` is the story, its `## Done when` the acceptance criteria.

Every issue carries an area label, a `type:` and a `size:`. The types are `feature`, `enabler`, `debt`,
`bug` and `decision`; the sizes are `S`, `M` and `L`, read as about half a day, one to two days, three to
five. An epic is the exception and carries `roadmap` alone: it is the container the sizes are balanced
inside, so an area and a size on it are a sum of its children rather than a fact about it.
**The sizes exist to balance milestones against each other and for nothing else.** They are bands, and a
date derived from summing them is arithmetic on guesses.

## Prose published to GitHub

**One paragraph is one line.** An issue body, a comment, a PR body and a release body are rendered as GFM
with hard breaks on, so every newline inside a paragraph becomes a `<br>` and the wrapping habit of a
committed file arrives as ragged short lines. Blank lines separate paragraphs; nothing else wraps. Tables,
lists and fenced blocks are structure rather than wrapping and are unaffected — but one list item is still
one line however long it runs, and consecutive blockquote lines join the same way, because a hard break
reads no better inside a quote. Committed Markdown is the
opposite — MD013 is off, so wrapping there costs nothing — which is why this crosses over unnoticed.
`gh api -X POST /markdown -f mode=gfm -f text=…` renders the difference against `mode=markdown`.

## Git

- Conventional Commits, commitizen's default types; the PR title is held to the same format. Both
  are checked — locally by the commit-msg hook, in CI by the shared workflow.
- Commit or push only when asked. Branch first if on the default branch, unless the owner asks for a
  direct commit and the change touches no code — a documentation edit, a comment, a task description.
  Anything else branches even when asked, because the reason to branch is review rather than the ruleset.
- **No merge commits.** History is linear, so a branch is rebased onto `main` rather than merged into
  it. The gate is server-side: a ruleset requires linear history on `main`, and the merge-commit
  button is off, so a PR lands as a squash or a rebase. `mise run git:ff-only` configures the clone to
  refuse the accidental case, but `git merge --no-ff` overrides it — it is a guardrail, not a second
  gate, and describing it as one would promise something nobody is holding. Never resolve a divergence
  by merging `main` in.
- **A red check blocks the merge for everyone but the owner.** The same ruleset requires three checks to
  pass — `CI`, `commits / PR title` and `commits / Commit messages` — and carries exactly one bypass
  actor, the `Repository admin` role in mode `always`. A contributor with write or maintain access never
  qualifies; an admin collaborator would, which is the one thing to weigh before granting that access.
  Three consequences, none of them visible from a green PR:
  - **The owner may push straight to `main`, and it is for small changes that touch no code.** That
    restriction is a convention and cannot be otherwise: `required_status_checks` takes no path
    conditions, and a push ruleset that would take them applies to every push including a PR merge.
    Anyone else still branches, because for them the push is refused — two of the three checks run on
    `pull_request` only, so on a directly pushed commit they never report.
  - **The bypass cannot be narrowed to pushes.** GitHub offers `always` or pull-requests-only, so the
    same actor that may push to `main` may also merge a red PR. Nothing enforces that they do not, and
    the `guard-direct-push` hook in `.pre-commit-config.yaml` is what partly replaces the gate a direct
    push skips — it runs `mise run ci` before the push, and like `git:ff-only` it is a guardrail rather
    than a second gate.
  - **A renamed job locks the branch.** A required check is matched by name, so if the shared
    `turboBasic/github-actions` workflow renames a job, the old context never reports and every PR
    blocks until the ruleset is updated. Bumping that dependency means checking the job names with it.
    The branch is not up-to-date-enforced, deliberately: that would force a rebase every time `main`
    moved under an open PR, for a staleness CI on the merge result already catches.
- **A second actor may write here, and it is not a person.** A GitHub App authors the agent's branches and
  pull requests ([ADR 0010](../decisions/0010-automation-brings-its-own-provider.md)). It holds contents,
  issues and pull requests write and nothing else — never `Workflows`, which would let it rewrite the
  checks it has to pass. **It is no bypass actor, and that is what refuses it `main`**, exactly as it
  refuses any contributor: not the workflow's step list, which bounds nothing once the agent's own shell
  holds a credential. What the workflow's scope does still leave is repository-wide `contents: write`, so
  any branch but `main` is force-pushable by it.
- Never commit a secret, a generated artefact, or a raster.
- An issue this work closes is closed by the PR that carries it — a `Closes #N` (or `Fixes`/
  `Resolves`) line in the PR body, merged into the default branch — not by a direct close run before
  the PR exists. GitHub links and closes an issue against its state at merge time, so closing it by
  hand first leaves nothing for the PR to attach to.

## CI

- **Pin every action to a full SHA**, with the version in a trailing comment, never `@main`. A reusable
  workflow call into `turboBasic/github-actions` may pin a version tag instead: Dependabot tracks that
  repository (`.github/dependabot.yml`, `github-actions` ecosystem), so a tag costs no more than the SHA
  it would bump to. Every other action, first- or third-party, takes the SHA.
- **Call a `turboBasic/github-actions` workflow wherever one fits**, and never fork one to fit this
  repository. `ci.yml` calls `project-ci`, which is bounded by the component rather than the language and
  runs this repository's own `lint`, `typecheck` and `test`.
- **Declare `depends = ["deps"]` on any task a called workflow reaches that needs dependencies
  installed.** A called workflow prepares nothing on a caller's behalf. Declare it on the leaf tasks and
  never on the `lint`, `typecheck` or `test` aggregators, because `depends` runs in parallel.
- **Name a calling job after the component it judges.** The job id is the first half of the required
  context, so `ci.yml`'s `popcircles` composes `popcircles / project-ci`, and renaming the job means
  editing the ruleset in the same change.
- **Name a workflow after its own file, behind a marker**: `🧩` where it is reachable only by a call,
  `🌜` where it has triggers of its own. A call-only workflow's sidebar entry is permanently empty, and
  the marker is what keeps that from reading as broken.
- **Write job ids and job names in lowercase kebab-case**, and give a job no `name:` where the name
  would only repeat its id. GitHub appends a matrix leg's values to the id on its own.
- **One workflow is one scenario.** Work two scenarios share is a local `workflow_call` workflow both
  call, not a condition on the event.
- **Drive CI through mise tasks**, so what CI runs and what `mise run ci` runs cannot drift apart.
- **Lint, typecheck and test only.**
- **Take secrets from CI environment secrets or OIDC.**
- **Keep a workflow comment to [`code.md`](code.md) "Comments and docs"**, plus one restriction that
  section does not carry: no ADR, issue or follow-up citation, no account of what the file used to be,
  and no sentence a reader has to unpack before it parses. `docs/decisions/` owns the reasoning, and a
  citation in a YAML file is a second place it goes stale.

## Structure

Hand-maintained. Update it in the change that adds or removes a root. A new file under `docs/ai/` gets
its `@` import line in `CLAUDE.md` in the same change too — without one it never loads, and nothing at
commit time says so.

**Roots only**, so that an omission is not read as drift: where a kind of thing lives, never an
inventory of files. Committed configuration sits at the repository root and documents itself, and the
layering table in `docs/ai-instructions.md` names the files that own an enforced fact. A path here and
absent on disk is drift and a finding; a path on disk and absent here is a finding only when it is a
new root.

```text
CLAUDE.md                        Claude entry point — imports only
AGENTS.md                        Codex entry point — follows Claude's imports
.agents/skills                   Codex skill discovery — symlink to .claude/skills
.codex/agents/                   Codex persona adapters — routing only
.github/copilot-instructions.md  Copilot entry point — routing only
opencode.json                    the agent's entry point — routing only
.github/workflows/               CI
docs/ai-instructions.md          the router: invariants and layering
docs/ai/                         the instruction layer, one file per subject
docs/decisions/                  decision records, one ruling and one page each
docs/follow-ups.md               the register of pending obligations
docs/ponytail.md                 the ponytail plugin's use here, and its limits
.claude/agents/                  one file per standing persona
.claude/skills/                  one directory per task workflow
crates/                          Rust workspace — the search
data/                            input datasets; registry in data/registry.toml
python/                          Python project — src/ holds the packages, tests/ the suite
results/                         the answers this project publishes, as the documents that printed them
```
