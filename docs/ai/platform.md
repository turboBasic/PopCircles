# Platform

Read this when you change tooling, dependencies, input data, quality gates, Git conventions, or CI;
work on a GitHub issue; decide whether a change needs a record under `docs/decisions/`; write its
plan; or move the structure tree.

Committed configuration defines its declared behavior. Read `mise.toml`, `Cargo.toml`, `pyproject.toml`,
and `.pre-commit-config.yaml`; do not guess. Extend these files; never regenerate them. This document
adds the judgments they cannot express.

## Tooling hierarchy

1. **A mise task** — `mise run <task>`. Never bypass one that exists.
2. **prek** — `prek run`. Hooks run through prek, not pre-commit; the header comment in
   `.pre-commit-config.yaml` says why the filename is what it is.
3. **`cargo <cmd>` / `uv run <tool>`** — the language-local tools.
4. **`mise exec -- <tool>`** — system tools mise manages.

`mise.toml` lists pinned tools and tasks. It declares what `lint`, `typecheck`, `test`, and `ci` run;
repeating a task's command in prose can make the documentation go stale. Run `mise run ci` to reproduce
CI locally before reporting work complete.

Install nothing globally. Pin new runtimes and CLIs in `mise.toml`. Do not use `pip install`, activate a
virtual environment by hand, or install a Rust toolchain outside mise.

If a task is not yet part of `lint` or `ci`, its comment explains why. Meet the stated condition before
adding it. The two Python tasks are waiting for source files and tests to exist.

## AI harnesses

Resolve relative paths in reused skills and personas from their source files. Follow their workflows
with the current harness's tools. Claude Code frontmatter, hooks, and permission settings do not
configure other harnesses. Follow a persona's editing limits in Codex; its custom agents inherit the
parent's permissions, not Claude Code's withheld tools.

When a workflow requests a persona, use its custom agent. If the harness cannot select a named agent,
start a fresh subagent and tell it to read the source persona and the workflow brief.

## Dependencies

- Rust: workspace settings live in the root `Cargo.toml` (`[workspace.package]`, `[workspace.lints]`).
  Crates inherit them with `field.workspace = true`. Commit `Cargo.lock`.
- Python: dev tooling in `[dependency-groups].dev`. No `setup.py`, `setup.cfg` or
  `requirements.txt`. Run `uv lock` after editing dependencies and commit the result in the same
  change.
- Node: use it only to host CLIs, never as an application dependency. Pin `node` and each tool in
  `mise.toml`'s `[tools]` via the `npm:` backend (e.g. `"npm:markdownlint-cli2" = "0.23.2"`);
  never `npm install -g`. Its prek hook calls the pinned binary directly (`language: system`),
  as `cargo-fmt` and `taplo-fmt`; do not let the hook manage its own runtime.
- Before adding a dependency, check whether the tree already has one that covers the need. Prefer the
  standard library for small tasks.
- Introducing a new file type or framework updates `.editorconfig`, `.gitattributes` and
  `.gitignore` in the same change.

## Large input data

Store input datasets in `data/`, one directory per kind. Publish large datasets for fetching. Commit a
small dataset only when every clone and CI job needs it and fetching costs more than storing it.
[`data/registry.toml`](../../data/registry.toml) records each dataset's grid, CRS, nodata value,
byte length, checksum, fetch URL, licence, and figure attribution. `mise run data:get`, the CLI, and the
renderer use this registry; builds name the row they need. [`data/README.md`](../../data/README.md)
describes the datasets for readers, records their provenance, and explains how to fetch them ([Getting
it](../../data/README.md#getting-it)). Add a dataset to both files in the same change.

The judgment around that mechanism:

- Add a new **input** dataset to `data/<kind>/` with a registry entry. Do so only deliberately.
  Generated products are neither committed nor placed there.
- **Publish rasters based on their needs, not just because they live in `data/`**
  ([ADR 0009](../decisions/0009-input-data-is-published-not-versioned.md)). Commit a dataset as a Git
  blob if every clone and CI job needs it and fetching it costs more than storing it. The registry
  records this choice for each dataset, and `check-added-large-files` limits its size.
- If a raster is missing, code that reads it reports the missing file and names `mise run data:get`;
  it must not report a format error.

## Quality gates

### Linting

- Run hooks through prek. Run `cargo clippy` through its mise task. Do not call `ruff` directly.
- Treat clippy warnings as errors. Do not silence them with a blanket `#[allow]`. A narrow, commented
  `#[allow]` on one item is acceptable when the lint is wrong.
- ruff for Python lint and format. Never add black, isort, flake8 or pylint.
- TOML is formatted with taplo.
- If a hook reformats files, stage them again and rerun the hook.
- cspell checks every tracked file. Add legitimate flagged terms to the relevant section of
  `.cspell/project.txt`; do not use inline ignores.
- markdownlint-cli2 checks Markdown, including `docs/decisions/`. `.markdownlint-cli2.jsonc` records
  which rules are disabled globally or for a specific path, and why.

### Type checking

Do not use blanket ignores or loosen a mode to clear an error. Use a narrow per-line ignore only at a
library boundary, and state the reason. Pyright checks Python in strict mode.

### Testing

- Rust: put unit tests in the module under `#[cfg(test)]` and integration tests in `tests/`. Property
  tests are welcome for geometry and summation invariants.
- Python: pytest. Never `unittest.TestCase` classes.
- Numeric tests must assert the invariants in [`application.md`](application.md) "Correctness
  invariants"; merely exercising the code is not enough.
- **Do not make tests depend on fetched rasters.** The test suite runs on clones without raster files,
  so CI cannot run tests that need them. Build fixtures in code or commit a small, reduced raster.
- Mark tests that need network access, real credentials, or fetched rasters. Deselect them by default
  and give them a separate task. Do not run them in CI.
- Run the suite when asked or when verifying a fix, not after every edit. Fix any test that fails after
  a change before reporting the work complete.

## Architecture decisions

Read accepted ADRs before making architecture choices, as required by `docs/ai-instructions.md`. This
section explains when to write an ADR and how to scope it. The `write-adr` skill defines its format.

**Write an ADR to answer a question, not to document a work package.** An issue requires choices at
many levels, but completing it does not mean every choice needs a record. That is the rule in
[ADR 0001](../decisions/0001-a-record-carries-one-ruling.md). Most issues need no ADR. One issue may
raise two independent questions and need two. A question raised in conversation may also need an ADR.

### The bar

A change needs a record only when all three hold:

1. **Reversing it costs more than a PR.** It crosses a crate or language boundary, changes a published
   format, or changes project-wide policy.
2. **A competent person could have chosen differently.** The choice had real alternatives, not just
   different preferences.
3. **The code cannot answer "why is it like this?"** Someone will need the reason later.

Two corollaries do most of the work in practice:

- **Do not record a choice that a gate already pins.** A test that asserts a tolerance or a manifest
  that confines a dependency already records the choice.
- **If the Consequences say a decision is cheap to reverse, it fails test 1.** Stop the draft instead
  of softening that statement.

### Scope

1. **One record, one ruling, one `scope:`.** A numbered list of decisions means the record covers
   more than one ruling.
2. **Record the constraint, not its implementation.** "The build takes no non-Rust prerequisite"
   remains true if the crate changes. The crate and its feature flags do not; they enforce the same
   constraint.
3. **Do not list schemas, fields, or file layouts.** State the property the artefact must have. Keep
   its fields in code and its version constant. Otherwise, each list change would require an ADR edit.
4. **Put the reason in the ADR and the current rule in the instruction layer.** `docs/ai/` describes
   what is true now, in editable present tense. The ADR records what was chosen, what was rejected, and
   when. Keep each concise and link them with one line.
5. **Limit each ADR to 80 lines, including frontmatter.** This limit enforces the four rules above.
   Put implementation details in the PR, evidence in the issue, and independent decisions in separate
   ADRs.

Keep measured figures in their dated source, such as the issue thread or PR. Cite the deciding number
inline in the ADR; do not add tables.

### Mechanics

- Name records `docs/decisions/NNNN-<kebab-slug>.md`, using the lowest unused number.
- Supersede accepted records with a new one; do not edit them. The only allowed edits are `status:`,
  `superseded_by:`, and one line under the title.
- Record decisions that have been **made**. A ruling without a date is still a proposal; keep it in the
  issue thread or conversation until it is settled.
- Apply this section to every record in the directory. Each record should show the format for new ones
  and meet the 80-line limit.

## Implementation plans

**Track work in GitHub issues**, one issue per deliverable, under the roadmap issue that serves as its
milestone's epic. Issues are the durable record of the work. Do not commit task lists elsewhere in the
repository, including beside an ADR; they will drift
([ADR 0001](../decisions/0001-a-record-carries-one-ruling.md)).

A **plan file** turns one issue's steps into executable tasks, each with a check that can fail. Store it
in the gitignored `tmp/`, run it, and never commit it. The issue's checked boxes and resulting commits
remain after the plan is gone. The `write-plan` and `run-plan` skills both use this format, which is
defined here.

- **Omit frontmatter.** The repository does not index uncommitted files.
- **Put a status line** directly below the title. Use `**Status: in progress (YYYY-MM-DD).**` or
  `**Status: complete (YYYY-MM-DD).**`. These are the only valid values. Do not run or check off a
  completed plan again.
- **Use ground rules** to constrain every task in the plan. They supplement the executing skill's loop;
  they do not replace it.
- **List out-of-scope work** that was considered and deliberately excluded. This helps readers
  distinguish deliberate omissions from oversights.
- **Group tasks into phases.** A phase may include a `Model:` note naming the expected model.
- **Number tasks as checkboxes** using `<phase>.<task>`. End each with a `Verify:` line that states
  what proves the task is done. Each task is one unit of execution and one commit.
- **List follow-up identifiers only.** Record resulting obligations in [`../follow-ups.md`](../follow-ups.md),
  which defines their format, statuses, and acceptance criteria.

## Issues

Read the whole issue thread, not just its body. The body states the initial position; comments may cut
scope, settle a figure, or reorder steps. Those decisions stay in the comments. Before starting, read
the comments and the parent roadmap issue so you do not implement a requirement that was later
withdrawn.

The agent running this repository from Actions cannot use `gh`; `opencode.json` denies all access,
including reads. Use `WebFetch` to read issues and comments from
`api.github.com/repos/:owner/:repo/issues/<n>` and `.../issues/<n>/comments`, rather than retrying
`gh issue view`.

**You may propose scope or requirement changes at any time**, for the current issue or another open
issue affected by a discovery. Work may reveal that a later step is unnecessary, misordered, or based on
a false assumption. Raise the finding without asking permission first; otherwise, the context may be
lost and someone will have to rediscover it.

This permission covers proposing a change, not making it. Comment on the issue whose scope would
change. For a downstream discovery, comment on that downstream issue, not the one you are working. Make
the change after the proposal is agreed. Do not silently build something the issue does not request.

**When all proposals in a proposal-only comment land, edit it to start with
`**Discharged YYYY-MM-DD** — <what landed>`.** Name each finding if the comment raised several. This
dated sentence tells readers what happened without requiring another status update. If a change is
reverted, the sentence remains historically accurate. Mark only comments that contain proposals and
nothing else, such as a scope review or a structural finding. Never mark a comment that includes a
decision or measured figure; readers need that context.

### Relationships are structural, never prose

**Set relationships that GitHub models through its API; do not write them into issue bodies.** Use a
sub-issue link for containment and a dependency for blocking:
`gh api repos/:owner/:repo/issues/<n>/dependencies/blocked_by -F issue_id=<id>`, read back from either end
with that path or `dependencies/blocking`. A `- Blocked by: #57` line under `## Relationship` duplicates
the API relationship and can go stale. GitHub updates its panel when an issue closes, is renamed, or
is superseded; the sentence does not.

Keep `- Relates to:` in prose because GitHub has no structured relationship for it. Remove a
`## Relationship` section when its only contents are blocking links.

### Milestones, epics and labels

A milestone is one release increment and contains the sub-issues of exactly one roadmap issue. That
roadmap issue is the epic; its body describes the milestone's contents. Update the body when the scope
changes. Otherwise, the milestone can accumulate themes and work without an explicit decision. Each
sub-issue describes one deliverable: `## Goal` states the user story and `## Done when` lists acceptance
criteria.

Every issue has an area label, a `type:`, and a `size:`. Valid types are `feature`, `enabler`, `debt`,
`bug`, and `decision`. Valid sizes are `S`, `M`, and `L`, meaning about half a day, one to two days,
and three to five days. An epic is the exception: it carries only `roadmap`. Use sizes to balance
milestones against each other; an epic's area and size would only summarize its children.
**Use sizes only to balance milestones.** They are estimates, so summing them does not produce a
reliable date.

## Prose published to GitHub

**Keep each paragraph on one line.** GitHub renders issue, comment, PR, and release text as GFM with
hard breaks enabled. Each newline inside a paragraph becomes a `<br>`, so wrapped source text appears
as ragged short lines. Use blank lines to separate paragraphs. Tables, lists, and fenced blocks keep
their structure, but each list item stays on one line regardless of length. Consecutive blockquote
lines join too, and hard breaks make quotes harder to read. Repository Markdown does not have this
problem because MD013 is off. To compare the renderings, call `gh api -X POST /markdown -f mode=gfm -f text=…`
and compare it with `mode=markdown`.

## Git

- Use Conventional Commits with commitizen's default types. PR titles follow the same format. The
  commit-msg hook checks commits locally, and the shared workflow checks them in CI.
- Commit or push only when asked. If you are on the default branch, create a branch first. The owner
  may request a direct commit for documentation-only changes, including comments and task descriptions.
  Branch all other changes so they can be reviewed.
- **Do not create merge commits.** Keep history linear by rebasing branches onto `main`. A server-side
  ruleset requires linear history on `main` and disables the merge-commit button, so PRs use squash or
  rebase. `mise run git:ff-only` configures the local clone to reject accidental merge commits, but
  `git merge --no-ff` can override it. Treat this as a guardrail, not an enforced gate. Never resolve a
  divergence by merging `main` into the branch.
- **A failed check blocks a merge for everyone except the owner.** The ruleset requires `CI`,
  `commits / PR title`, and `commits / Commit messages` to pass. Its only bypass actor is the
  `Repository admin` role in `always` mode. Write or maintain access does not qualify, but an admin
  collaborator does. Consider this before granting admin access. The rule has three consequences:
  - **The owner may push directly to `main`, for small changes that touch no code.** This is a
    convention: `required_status_checks` cannot set path conditions, and a push ruleset would apply to
    every push, including PR merges. Everyone else must branch because direct pushes fail; two checks
    run only on `pull_request` and would not report on a direct push.
  - **The bypass also allows merging failed PRs.** GitHub offers `always` or pull-requests-only bypass
    modes, so an actor who can push directly to `main` can also merge a red PR. The
    `guard-direct-push` hook in `.pre-commit-config.yaml` partly covers checks skipped by a direct push:
    it runs `mise run ci` before pushing. Like `git:ff-only`, it is a guardrail, not an enforced gate.
  - **Renaming a job can block every PR.** GitHub matches required checks by name. If the shared
    `turboBasic/github-actions` workflow renames a job, the old check never reports. Update the ruleset
    when changing that dependency, and verify the job names. Branches do not have to be up to date with
    `main`: requiring that would force rebases whenever `main` changes, while CI already catches stale
    results on the merge commit.
- **A GitHub App writes branches and PRs for the agent.** It has write access to contents, issues, and
  pull requests ([ADR 0010](../decisions/0010-automation-brings-its-own-provider.md)). It has no other
  permissions and cannot modify workflows, which could change the checks it must pass. The app is not a
  bypass actor, so GitHub blocks it from pushing to `main`, just as it blocks other contributors. The
  workflow's step list does not provide this protection because the agent's shell has the credential.
  Its repository-wide `contents: write` permission does let it force-push other branches.
- Never commit a secret, a generated artefact, or a raster.
- Close issues through the PR that delivers the work. Add `Closes #N` (or `Fixes` or `Resolves`) to
  the PR body and merge it into the default branch. Do not close the issue directly before the PR
  exists. GitHub links and closes it at merge time; closing it earlier leaves nothing for the PR to
  close.

## CI

- Release authorization trusts `turboBasic`'s current GitHub SSH signing identity
  ([ADR 0013](../decisions/0013-release-authorization-trusts-a-github-account.md)).
- **Pin every action to a full SHA** and add its version in a trailing comment. Never use `@main`. A
  reusable workflow from `turboBasic/github-actions` may use a version tag because Dependabot tracks
  that repository (`.github/dependabot.yml`, `github-actions` ecosystem). The tag gets updated with the
  same dependency updates as a SHA. Pin all other actions, first- or third-party, to a SHA.
- **Use a `turboBasic/github-actions` workflow when one fits.** Do not fork one for this repository.
  `ci.yml` calls `project-ci`, which runs this repository's `lint`, `typecheck`, and `test` tasks for
  the component, regardless of language.
- **Add `depends = ["deps"]` to each called task that needs installed dependencies.** A called workflow
  does not install them for the caller. Add the dependency to leaf tasks, not the `lint`, `typecheck`, or
  `test` aggregators, because `depends` runs in parallel.
- **Name each calling job after the component it checks.** The job id starts the required context, so
  `ci.yml`'s `popcircles` produces `popcircles / project-ci`. If you rename the job, update the ruleset
  in the same change.
- **Name workflows after their files and add a marker:** `🧩` for workflows used only through calls,
  `🌜` for workflows with their own triggers. The marker explains why a call-only workflow has an empty
  sidebar entry.
- **Use lowercase kebab-case for job ids and names.** Omit `name:` when it would repeat the job id.
  GitHub appends matrix values to the id automatically.
- **Give each workflow one scenario.** Put steps shared by two scenarios in a local `workflow_call`
  workflow that both call; do not condition the workflow on the event.
- **Run CI through mise tasks** so CI and `mise run ci` stay in sync.
- **Lint, typecheck and test only.**
- **Take secrets from CI environment secrets or OIDC.**
- **Follow [`code.md`](code.md) "Comments and docs" for workflow comments**, with these extra limits:
  do not cite ADRs, issues, or follow-ups; do not describe the file's history; and write sentences that
  readers can understand on first read. `docs/decisions/` holds the reasoning. Citations in YAML would
  duplicate links that can go stale.

## Structure

Maintain this tree by hand. Update it whenever a root is added or removed. When adding a file under
`docs/ai/`, also add its `@` import to `CLAUDE.md`; otherwise, the file will not load, and no commit
check will catch the omission.

**List roots only, not every file**, so omissions are not mistaken for drift. Committed configuration
at the repository root documents itself. The layering table in `docs/ai-instructions.md` names files
that own enforced rules. Flag a listed path that is missing on disk as drift. Flag an unlisted path only
if it is a new root.

```text
CLAUDE.md                        Claude entry point — imports only
AGENTS.md                        Codex entry point — follows Claude's imports
ACCESSIBILITY.md                 accessibility priorities, reporting and known limits
PERFORMANCE.md                   runtime, memory and search trade-offs
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
