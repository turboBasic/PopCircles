# Follow-up register

Every follow-up this repository has recorded, in one place, and the only committed one — a plan file is
scratch, so its Follow-ups section names identifiers that must already exist here. This is a live
document: entries change status as the repository moves under them, and an entry whose status is stale is
worse than no entry at all.

**Every entry states a condition the repository can answer.** A file, a command's output, a count, a
hook's result — something a sweep can evaluate without asking the user what they have experienced.
Prose that cannot meet that bar is not a follow-up and does not belong here.

**An entry arrives when work stumbles over one, so this register is not a survey.** A module named
nowhere below has not been found clean; it has not been looked at, and finding out takes a deliberate
sweep rather than a read of this file.

## Statuses

- `dormant` — the condition is checkable and has not been met.
- `due` — the condition has fired and the fix has not landed.
- `closed` — the obligation is resolved; the entry says when and what closed it, which need not be
  the fix.
- `retired` — the entry can never fire; the entry says why.

Closed and retired entries stay in the register, so a reader can tell "never fired" from "cannot
fire". They sit under their own heading below the live ones, because a sweep and a reader both want the
`dormant` and `due` entries and neither should page through the discharged ones to reach them.
A discharged entry's Condition and Fix record what was checked against the tree they ran on, so a later
rename corrects the live entries and leaves these: editing one to satisfy a text sweep would falsify a
measurement.

An entry that goes `due` gets an issue in the next milestone, and the Status names it. Where the Fix needs
something nobody has — a paid identity, an account, hardware — it stays here and out of a milestone, and the
Status says which of the two it is, so an obligation nobody can schedule is not read as one nobody has
picked up. `FU-13` is the standing example of the second.

## Entry format

An entry is a level-three heading, `FU-NN - <title>`, followed by three fields:

- **Status** — one of the four above, carrying the date when `closed`, the reason when `retired`.
- **Condition** — what has to become true, worded so a later reader can evaluate it against this
  repository rather than against the author's memory.
- **Fix** — what becomes correct once the condition holds. Checked against the tree it would run on,
  so the entry does not prescribe something untested.

Identifiers are flat, sequential and never reused.

## Entries

### FU-13 - A published binary carries no Developer ID, and a user is told to clear an attribute by hand

- **Status** — `due` (2026-08-15): `v0.1.0` is published, so both halves of the condition read true —
  `gh release list` returns it, and `release.yml` names no signing tool. The published macOS asset was
  measured on the day: `gh release download` leaves it carrying `com.apple.provenance` and no
  `com.apple.quarantine`, and it runs and reports `popcircles 0.1.0`. That is the README's claim holding,
  not the identity arriving.
- **Condition** — a Release exists while no artifact carries a Developer ID signature. The sweep is
  `gh release list` returning at least one release, together with
  `rg -n 'codesign|notarytool|notarize' .github/workflows/release.yml` returning nothing. Not "unsigned":
  the dry run measured `codesign -dv` on the published macOS asset as `Signature=adhoc` with
  `TeamIdentifier=not set`, which is the linker's default and is what lets it run on Apple silicon at all.
  What is missing is the identity Gatekeeper will accept. The consequence is written down in `README.md`'s
  Releases section, which tells a macOS user to clear `com.apple.quarantine` off a browser download —
  measured too: quarantined, the binary is killed with "Apple could not verify"; with the attribute gone it
  runs. That is a documented workaround for a missing identity rather than a property of the tool, and the
  entry exists because such an instruction reads as normal once it has sat in a README for a while.
- **Fix** — sign and notarize the macOS artifact in the publish job, which drops the README line rather
  than explaining it better. It was put out of scope for a reason that is a prerequisite and not a
  preference: it needs a paid Apple identity and a certificate in CI secrets, so this cannot be closed by
  anyone who does not hold the account. Until then the honest form is the documented attribute, which is
  why the entry's condition is about a release existing rather than about the README's wording.

### FU-14 - A dataset's grid step approaches the tolerance two grids are one within

- **Status** — `dormant` (2026-08-15): the registry holds one dataset, at 0.008333° — six orders of
  magnitude above the constant.
- **Condition** — a row in [`data/registry.toml`](../data/registry.toml) names a grid resolution within
  three orders of magnitude of `BOUNDARY_TOLERANCE_DEG` in `crates/popcircles/src/grid.rs`. The sweep is each
  row's `lat_step`/`lon_step` against the constant: 0.008333333333333333° against 1e-9° today, and
  what fires it is a dataset finer than about 1e-6°. Two grids within the tolerance are one table — a cache
  compares the geometry within it while the dimensions compare exactly — so a cell small enough
  to land in that gap makes a stale cache indistinguishable from the right one, which is the failure
  [ADR 0005](decisions/0005-derived-artefacts-are-keyed-and-refused.md) exists to catch.
- **Fix** — a record, not a smaller number chosen in passing. The constant is shared with the raster reader
  by design (`grid.rs` says why a second copy would be two answers to one question), so tightening it for
  the cache alone reintroduces exactly what that comment forbids, and tightening it for both changes what
  rasters the reader accepts. What a record has to weigh is that 1e-9° is scaled to a measured rounding in
  the registry raster's own geotransform, and a dataset fine enough to fire this would bring a measurement
  of its own to scale it to. Issue #45 already ruled out the two shortcuts: exact bit equality
  on the four numbers, and a per-caller tolerance.

### FU-17 - The full-resolution search is page faults, not arithmetic

- **Status** — `dormant` (2026-10-22): #18 still asks for a named handful of full-resolution
  certifications, explicitly not a sweep. The #72 full-resolution #73 measurements were one
  fixed-radius, all-country composition pass per route, not a command needing many radii.
  Neither meets the condition. The measured route costs and reconciliation are recorded in [#72's
  execution evidence](https://github.com/turboBasic/PopulationCircles2026/issues/72#issuecomment-6340314761).
  The earlier #10 figure remains the full-resolution search baseline: one radius at 30 arcsec is
  207 s of which **13.4 s is CPU**, with `iostat` reporting ~7 000 transfers a second against the
  7.5 GB payload. Every answer this repository has published came from a decimated table or from
  three certifying radii, and both are affordable at that rate.
- **Condition** — a command that needs many full-resolution radii becomes something this repository asks
  for: `smallest-for-share` or `sweep` at `--decimate 1` named in a mise task, in `USAGE.md`, or in
  an open issue's acceptance. Two dozen probes at 207 s is 90 minutes, and issue #18's per-country sweep is
  the first plausible caller — ninety-plus countries at full resolution is not that multiplied by one.
  The figure is re-measurable in one line, which is what keeps this checkable rather than remembered:
  `/usr/bin/time -l` around a `most-populous --decimate 1` run, and the entry has fired as long as user plus
  system time stays under a fifth of real.
- **Fix** — a record, because the candidates are algorithm changes rather than tuning. What the measurement
  says is that locality is the cost: `circle::population` walks a kernel's rows for one centre, and
  consecutive table rows are 345 KB apart, so one evaluation touches ~111 MB of scattered pages and the next
  centre re-walks the same stride one column over. Inverting that loop — one row band, every candidate column
  in it, before moving on — reuses each page across a whole level instead of once, and is a change nobody
  could have justified without this figure. It is not free: it changes the order the fold adds
  in, which `search`'s determinism tests pin and `application.md` "Determinism" makes a stated rule, so a
  record has to weigh a changed answer's bits against the wall clock. Prefetching or `madvise` are the
  cheaper half-measures a record should rule on beside it, and neither touches the order.

### FU-18 - Diagnostics are line-oriented and nothing consumes them as data

- **Status** — `dormant` (2026-08-15): every diagnostic this repository emits is read by a person. The `log`
  facade puts them behind a seam where the library emits
  records and the binary alone chooses a stream, a level and a format, which is what makes the line-oriented
  form a choice rather than an accident — and the right one while the reader is human.
- **Condition** — something in the tree parses a diagnostic rather than displaying it: a mise task, a
  script under `python/`, a workflow step or a test that greps, cuts or regex-matches what the CLI writes
  to stderr in order to obtain a value. `rg -n 'stderr' mise.toml .github/workflows/ python/` naming a
  step that extracts rather than shows is the sweep. The day one exists the log is an interface, and an
  interface whose shape is a formatting decision breaks the first time the wording is improved.
- **Fix** — decide, and record the decision rather than reach for `tracing` by reflex. The consumer may be
  better served by the JSON document on stdout, which is already versioned and already a contract, than by
  structure in a stream that is not; where it genuinely wants the diagnostic, structured fields on the few
  emissions that have a consumer may be the whole of it. Replacing the facade reaches
  every emission site and changes what a person watching a run sees — weighable, but not to be paid for a
  consumer that does not exist. Issue #64 is the scheduled look at this question; this entry is what fires
  if that issue closes with "not yet".

### FU-19 - The record shape is machine-checkable and unchecked

- **Status** — `dormant` (2026-08-15): [ADR 0001](decisions/0001-a-record-carries-one-ruling.md) caps a
  record at 80 lines, allows it one `scope:` from a closed list and forbids a numbered decision list, and
  the housekeeping sweep is the only thing that looks. Every record in the directory was written to those
  rules by the pass that adopted them, so nothing has yet had the chance to drift.
- **Condition** — any record exceeds that 80-line ceiling, carries a
  `scope:` value absent from the closed list in the `write-adr` skill, or carries none at all.
  `wc -l docs/decisions/*.md` and `rg -n '^scope:' docs/decisions/` answer both halves,
  and either answering wrong means the rules held for exactly as long as someone was watching.
- **Fix** — a check in `python/src/repo_tools/lint_docs.py`, wired into `mise run lint:docs` the way the
  pointer and structure-tree checks already are, with its cases in `python/tests/test_lint_docs.py`. It
  asserts the line count
  and the `scope:` value over every record, since none is exempt. The numbered-list rule
  is the one part to leave out: `## Options` legitimately contains an ordered structure, and a lint that
  guesses at the difference is worse than the sweep reading the file.

### FU-20 - The renderer resolves committed data from its own source path

- **Status** — `dormant` (2026-08-15): the project is installed editable, so `__file__` is in the
  checkout and `data/` is beside it. The path is `parents[3]` of a module under
  `python/src/population_circles/`, which is the repository root for exactly as long as that holds.

  **A second resolution joined the first, and one consumer of it writes** (2026-08-16, issue #57).
  `dataset_registry.py`'s `REGISTRY` reaches `data/registry.toml` the same way `render_map.py`'s
  `COASTLINE` reaches the basemap, so the count this entry sweeps for is two. `mise run data:get` then
  resolves paths against that same root to place 428 MB, so under a wheel it would `mkdir` under
  `site-packages` — it happens to fail loudly first, because the coastline sorts before the raster and
  has no `fetch_url`, but the message it gives is about a damaged checkout and would be wrong. The registry is the worse of the
  two: it is read for the attribution a figure owes, so under a wheel a figure would fail before it
  could credit anybody.
- **Condition** — anything installs this project other than editable from a checkout: a `uv tool
  install`, a `uv publish`, or a workflow running an entry point out of a built wheel. Then both paths
  resolve under `site-packages`, where no `data/` sits beside the package, and each raises
  `FileNotFoundError` at read time rather than at import. The sweep is
  `rg -n 'parents\[3\]' python/src/population_circles/`.
- **Fix** — ship the two files as package data, or take each path as an argument defaulting to the
  current location. The first duplicates committed files into the wheel and owes
  [`data/README.md`](../data/README.md) a line saying so; the second keeps one copy and moves the choice
  to the caller, which is the shape the rest of the renderer already takes. One fix covers both, and
  splitting them would leave a figure half-resolvable.

### FU-21 - The direct push to main is a concession to an early, solo repository

- **Status** — `dormant` (2026-08-16): one account holds admin, and the open work is documentation and
  backlog shaping, which is the state the concession was granted for.

  **It is meant to be reverted, not inherited.** `main`'s ruleset carries one bypass actor, the
  `Repository admin` role in mode `always`, so the owner may push straight to `main` while everyone else
  still branches. That was taken deliberately, to spare a one-person repository a branch, a pull request, a
  merge and a cleanup for every comment and every documentation edit. It buys nothing once the repository
  is not that, and the ordinary state — every change to `main` arriving through a reviewed pull request
  with three green checks — is the safe one. This entry exists so the concession expires on a condition
  rather than on somebody noticing.
- **Condition** — whichever of these comes first.
  - **The repository stops being one person's.** A second account holds admin:
    `gh api repos/:owner/:repo/collaborators --jq '[.[] | select(.permissions.admin) | .login]'` returning
    more than one login. Write and maintain do not qualify, which is what makes the grant of admin the
    event rather than the invitation — and what makes this fire by surprise, since nobody adding a
    collaborator intends to hand out a red-PR merge.
  - **The repository stops being early.** Milestone `v0.4: usable and distributable` closes:
    `gh api repos/:owner/:repo/milestones --jq '.[] | select(.title | startswith("v0.4")) | .state'`
    reporting `closed`. That milestone is where a distribution channel and signed release provenance land,
    so it is the point at which strangers install what `main` holds, and unreviewed commits on `main` stop
    being a private matter. The milestone is named rather than a date because a date would be a guess.
- **Fix** — harden back: drop the bypass actor from the ruleset, delete the `guard-direct-push` hook and
  `pre-push` from `default_install_hook_types`, and restore what the three documents said before — that a
  red check blocks the merge for everyone including the owner, that `main` takes no direct push, and
  `CONTRIBUTING.md`'s unqualified "do not push to `main` directly". Keeping the concession instead is a
  decision that has to be argued for on the tree as it is then, not the default that happens by silence.

  There is no narrower setting to reach for, which is why the answer is revert rather than tighten:
  GitHub's bypass modes are `always` or pull-requests-only, and `required_status_checks` takes no path
  conditions, so neither "pushes only" nor "documentation only" was ever expressible.
  `guard-direct-push` runs `mise run ci` before a direct push, but it is a clone-side guardrail — a second
  admin has to install it, and `--no-verify` skips it.

### FU-22 - The registry-row hook does not tell a published row from a committed one

- **Status** — `dormant` (2026-08-16).
- **Condition** — a row in `data/registry.toml` carrying a `fetch_url` names a path that `git ls-files`
  also names. `geo-data-registered` asks only whether a row exists, so a published dataset committed to the
  tree satisfies it: the row it needs is the row it has. ADR 0009 rules that which side of the line a
  dataset falls on is stated per dataset rather than derived from where it sits, and `data/registry.toml`'s
  own header is what states it — `fetch_url` present means fetched and never carried, absent means a
  committed blob. Nothing checks the first half.

  `check-added-large-files --enforce-all` bounds the damage at 500 KB on size alone, which is why this is
  an entry and not a task: the raster it would matter most for is 428 MB and refused already. What is
  unguarded is a *small* published dataset, and on 2026-08-16 there is none —
  `rg -c '^fetch_url' data/registry.toml` is 1, and that row's path is gitignored and untracked.
- **Fix** — the check reads the registry rather than grepping it and asserts both halves: every file
  matching the extension set has a row, and no file whose row carries a `fetch_url` is tracked. That means
  a `python/src/repo_tools/` entry point reusing `population_circles.dataset_registry`, which is where the
  registry is parsed — `repo_tools/fetch_data.py` is the precedent for a repo tool importing it, and
  reusing it rather than writing a second parser is the constraint the hook's own comment raises.

  **This reverses a decision the issue thread made outright**: #85 settled on a grep so the hook would stay
  the six-line `bash -c` it is, with a comment match and the coupling to taplo's `path = "…"` spelling as
  accepted costs. Both retire with the grep, and a later reader should not take the grep for an oversight.

### FU-23 - The busiest pointer hub in the tree is outside the pointer gate

- **Status** — `dormant` (2026-08-16).
- **Condition** — a repo-relative pointer or a quoted heading in `data/README.md` resolves to nothing.
  `lint_docs.py`'s `scope_files()` does not sweep that file, and `docs/ai-instructions.md` "Layering" names
  it a licensed restater of what `data/registry.toml` and the instruction layer own — so it carries
  pointers in both directions and none of them is checked.

  Measured on the tree that rewrote it, and the measurement cuts two ways. Every pointer the file carries
  today passes: `check_pointers()` run over it by hand returns 0 findings, so the file is not currently
  broken and the risk is the absence of a gate rather than a backlog behind one. But two pointers *did* go
  stale in that rewrite and both were found by hand — and **neither is a form this gate checks even where it
  runs**. One was in `data/registry.toml`, a TOML comment that no markdown scope would ever reach; the other
  was in this file and written `` `CONTRIBUTING.md` under "…" ``, whose quote is not adjacent to the path,
  which is what `_QUOTE_ADJACENT_RE` requires. So the evidence is that the file's pointers rot unobserved,
  not that widening the scope would have caught these two.
- **Fix** — `data/README.md` comes inside the pointer check. **Not a one-line addition to `scope_files()`**:
  that set is shared, its own comment declaring it the scope of the `housekeeping` skill's "Duplication"
  check as well, and that check's carve-out for licensed restatement covers the human layer —
  `README.md`, `USAGE.md`, `CONTRIBUTING.md` — and not this file. Widening the set therefore reads
  `data/README.md` for duplication too, where being a restater is the whole point and every legitimate
  restatement would report. So the fix is to separate the two scopes, or to carve this file out of the
  duplication half, and that shape is what makes it an entry rather than the one line it looks like.

### FU-24 - The reader hand-copies the wire format's version and its kinds

- **Status** — `dormant` (2026-08-17): the two sides agree today, so the copy is correct and nothing
  says it has to stay that way.
- **Condition** — the constants diverge. Two halves, each a one-line sweep:
  `rg -o 'const KIND: &.static str = "[a-z-]+"' crates/popcircles/src/report.rs` against the
  `DocumentKind` literal in `python/src/population_circles/circle_document.py`, and `SCHEMA_VERSION` in
  each file. On 2026-08-17 the Rust sweep returns ten and the reader lists nine, which is the count
  agreeing rather than disagreeing: the tenth is `envelope-fixture` under `#[cfg(test)]`, so a sweep that
  does not exclude the test module reads a divergence that is not one. `SCHEMA_VERSION` is `1` on both
  sides. What fires it is a kind added to `report.rs` — the reader then refuses a document the format
  says is valid, at a boundary whose whole job is to refuse the ones that are not — or the constant
  rising on one side alone, where the version gate stops meaning what it says.
- **Fix** — a pair in `python/src/repo_tools/lint_version_bumps.py`, which is where a wire-format
  constant is already coupled to what governs it. It does not fit that hook's existing shape and the
  entry says so rather than leaving it to be discovered: `STRUCT_TRIGGERS` compares HEAD against the
  index, so it fires on a *change*, while what is wanted here is an equality between two files that
  holds on every run — which is a `lint:docs`-shaped check living in a hook that is not. Whoever fires
  this decides which of the two it joins, and the reason the copy exists at all is not a defect to
  remove: `circle_document.py` is the boundary [ADR 0002](decisions/0002-rust-computes-python-presents.md)
  puts between the halves, and reading the Rust source at runtime would be the coupling that record
  exists to prevent.

### FU-25 - Three workflows install a 137 MB agent none of them runs

- **Status** — `dormant` (2026-08-17): `opencode` sits in `mise.toml` `[tools]`, which is where
  `platform.md` "Tooling hierarchy" says a pinned tool belongs, and the roster is installed whole by
  whoever asks for any of it.
- **Condition** — the cost stops being paid only by cache misses. `jdx/mise-action` installs the whole
  `[tools]` roster, and `rg -l 'jdx/mise-action' .github/workflows/` names four workflows on 2026-08-17 —
  `ci.yml`, `build-binaries.yml`, `release.yml` and `agent-run.yml` — of which only the last invokes
  `opencode`. Measured the same day: `du -sh ~/.local/share/mise/installs/opencode/1.18.18` is 137 MB, one
  binary, the largest single entry the roster has. What fires this is a figure rather than the arrangement:
  the mise install step in `ci.yml` taking materially longer than it did before the pin, on a cold cache,
  which is what a reader of a slow pull request would go looking for. Every clone's `mise run setup` pays
  it once too, which is the smaller half.
- **Fix** — move the pin from `[tools]` to the task that uses it, so the roster stops carrying it: mise
  takes a `tools` table on a task, and `agent-run.yml` calls `opencode` from exactly one step. That keeps
  the version pinned in `mise.toml` where `platform.md` wants it and keeps the comment beside it, while
  taking the download off three workflows that never run the binary. Checked against the shape those
  workflows have now: none of them invokes `opencode`, so none needs a second install step added back.
  Not done at the time of writing because the arrangement is correct and only the cost is suspect, and a
  figure nobody has measured is not a reason to complicate a manifest.

### FU-26 - Two entry points name the instruction layer, and nothing couples them

- **Status** — `dormant` (2026-08-17): both lists name the same four files today, so the copy is correct and
  nothing says it stays that way.
- **Condition** — the lists diverge. One sweep, two halves compared:
  `rg -o '^@\S+' CLAUDE.md | sed 's|^@||'` against
  `python3 -c "import json;print('\n'.join(json.load(open('opencode.json'))['instructions']))"`. On
  2026-08-17 both return `docs/ai-instructions.md`, `docs/ai/platform.md`, `docs/ai/code.md` and
  `docs/ai/application.md`. What fires it is a fifth file added under `docs/ai/` and imported by one entry
  point alone — `platform.md` "Structure" already says a new file there gets its `CLAUDE.md` import line in
  the same change, and it now needs two lines rather than one, with nothing at commit time saying so. The
  failure is silent and asymmetric: Claude reads the new subject and the agent holding a write token does
  not.
- **Fix** — the equality becomes a case in `python/src/repo_tools/lint_docs.py`, which is already the
  entry point that checks a pointer resolves and is already wired into `lint:docs`. It fits that hook's
  shape rather than `lint_version_bumps.py`'s: this is an equality that must hold on every run, not a
  coupling that fires on a change. The duplication itself is not removable — opencode does not expand
  `CLAUDE.md`'s `@` syntax, which is why `opencode.json` exists at all — so coupling the two lists is the
  whole of what can be done.

### FU-27 - The agent's authorisation is one association string, trustworthy because one person has write access

- **Status** — `dormant` (2026-08-17): one account can write here, so `OWNER` and "whoever may spend the
  provider key" are the same set. The gate is correct today for a reason that is about the repository rather
  than about the gate.
- **Condition** — a second account gains **write** access:
  `gh api repos/:owner/:repo/collaborators --jq 'length'` returning more than 1. Write is the event, not
  admin, which is what separates this from [FU-21](#fu-21---the-direct-push-to-main-is-a-concession-to-an-early-solo-repository) —
  that entry's condition is a second *admin* and its Fix is the direct-push concession. A contributor with
  write access can push branches and open pull requests but is not `OWNER`, so `/agent` silently keeps
  refusing them while the intent behind the gate has changed: the question stops being "is this the owner"
  and becomes "may this person spend the key".
- **Fix** — `.github/workflows/agent.yml`'s condition stops being an equality against one string. What it
  becomes is the decision this entry defers rather than pre-empts: a set of associations, a team check, or a
  label a trusted person applies. The cost of getting it wrong is not a broken build — a public repository,
  a metered credential and a write-scoped token are three things whose composition is the security boundary
  [ADR 0010](decisions/0010-automation-brings-its-own-provider.md) names, so loosening it is argued for on
  the tree as it is then. Note while deciding: `author_association` is computed by GitHub per comment and
  cannot be narrowed to "this person may spend money", so whatever replaces it is a proxy too.

### FU-29 - A committed document is what the program printed, and nothing checks that it still is

- **Status** — `dormant` (2026-08-19, issue #68): the three documents in `results/` are each one line, as
  the CLI wrote them. One was reformatted by hand while this work was open and the corpus test passed over
  it unchanged, which is how the gap was found rather than reasoned about.
- **Condition** — a file in [`results/`](../results/) is more than one line, which
  `awk 'END{exit NR<2}' results/*.json` answers per file. `python/tests/test_result_corpus.py` reads each
  document through `json.loads`, so any formatting parses and
  [ADR 0011](decisions/0011-a-published-answer-is-committed-as-its-document.md)'s byte-for-byte ruling is a
  convention with no gate under it.
- **Fix** — a one-line assertion in that test, since it already opens every document. Not now, because the
  layout carries nothing a consumer reads and a reformatted document is still the same answer: what would
  make this due is a diff nobody can review, and one hand-edited file is not that.

## Closed and retired

### FU-08 - Nothing couples the search's initial spacing to a measured figure

- **Status** — `closed` (2026-10-20), #71: the library derives spacing from each radius and grid,
  the CLI permits an exact positive override, and the four-radius comparison is recorded on the issue.
  The exclusions were rechecked: `decimated_search.rs` still fixes 32 for its ignored pruning fixture,
  `smallest.rs` keeps synthetic unit fixtures in the default suite, and `registry_validation.rs` still
  fixes 256 for its ignored real-raster fixture. Their thresholds and answer assertions are unchanged.
- **Condition** — a caller outside `search.rs` itself **chooses** the search's initial spacing. Choosing is
  what fires it, not calling: a caller that takes a spacing as a parameter and forwards it has made no
  choice, and the number it passes on is still its own caller's. The sweep is therefore two-part —
  `rg -n 'most_populous\(' crates/popcircles-cli/src crates/popcircles/src --glob '!search.rs'` for the call
  sites, and for each one, whether the spacing it passes is a literal or a constant rather than something it
  was given.

  On 2026-08-14 that names exactly one call site, `crates/popcircles/src/smallest.rs`, and it forwards: the
  parameter arrives from its own caller untouched, and `smallest.rs` declares no spacing of its own —
  `rg -n 'NonZeroU32::new' crates/popcircles/src/smallest.rs` matches only inside its tests. So #7 came and
  went without firing this. #8 is where it fires, when a command surface has to put a number in a flag's
  default or in a help string.

  **#8 came and went without firing it either** (2026-08-14). The three commands that drive a search take
  `--spacing` as a **required** flag with no default, and its help string names no figure — checkable as
  `cargo run -p popcircles-cli -- most-populous --help` carrying no digit on the `--spacing` line. A command
  that takes a spacing and forwards it has made no choice, which is the distinction this Condition draws, so
  the entry is recorded as still dormant rather than left to read as an oversight. The note is here for the
  reason the #7 one is: silence in a register is ambiguous, and a later reader should be able to tell "the
  surface arrived and did not choose" from "nobody looked".

  `crates/popcircles/tests/decimated_search.rs` is deliberately outside the sweep. It picks 32, but a
  deselected fixture choosing a spacing to exercise pruning is a fixture and not a default, and widening the
  sweep to catch it would leave this entry permanently `due` with nothing to do about it. The unit fixtures in
  `smallest.rs` are outside it for the same reason.
  `crates/popcircles/tests/registry_validation.rs` joins that exclusion (2026-08-15). It picks 256, on the
  plateau issue #10's sweep measured, and it is a deselected fixture for the same reason the two above are.

  **The curve this entry was waiting for exists, and it corrects the Condition's own arithmetic**
  (2026-08-15, issue #10's spacing sweep). Swept over the
  5 arcmin table at 200, 800, 3 300 and 8 000 km, the wall clock falls **monotonically** with spacing in
  every row and flattens from about 256 — a sixteenth of the grid's width — rather than peaking at a knee.
  The pruning *fraction* falls with it, from 97.3% at spacing 8 to 77.2% at 256, so the figure the Condition
  reasons from is the one that must not be maximised: a fine first level prunes almost every block it makes
  and pays for making them. What the search costs is circles evaluated, and at 3 300 km that is 4 447 at
  spacing 8 against 390 at 256.

  So "the ceiling is a sanity limit, two orders of magnitude away from the answer" is wrong, and the Fix
  below is right for a reason it did not claim: the ceiling is the neighbourhood of the answer, and clamping
  against it is the derivation rather than a guard on one. Nothing else about the entry changes.

- **Fix** — a derivation of the initial spacing from the radius and the grid, in `search` beside the loop
  that consumes it, bounded above by the ceiling `slack_km` already documents: once `radius + slack` reaches
  half the circumference the widened circle is the whole sphere, every bound equals the raster's total and
  the level prunes nothing, so a spacing past that point is strictly wasted work. The figure inside that
  ceiling is #10's to measure — 32 on the k=10 shape prunes 86.9% of blocks at a 200 km radius, which is one
  point and not a curve. The entry exists so the first caller does not silently become the default, and it
  cannot be discharged by a benchmark alone: what it wants is a function of the two inputs, not a constant
  that happened to measure well once.

### FU-30 - CI compiles from cold, because a called workflow caches only its hook runner

- **Status** — `closed` (2026-09-16): measured rather than fixed. The first run without the cargo cache
  took 1.3 minutes against a 0.9-minute median across the 100 runs that had it, so the cache was worth
  about 24 seconds and a cold compile is cheap enough to leave alone.
- **Condition** — `mise run lint`, `typecheck` and `test` run under
  `turboBasic/github-actions/.github/workflows/project-ci.yml`, whose published inputs name no cache, so
  nothing restores `~/.cargo/registry`, `~/.cargo/git` or `build/target` between runs. A caller cannot
  add a step to a job it does not own, so this was never fixable here.
- **Fix** — none. The shape a fix would take is a cache input on the capability, taking paths and a key
  from the caller — language-agnostic, so ADR 0004 upstream does not forbid one — and never a fork of the
  shared workflow. What reopens this is a `ci.yml` run over three minutes.

### FU-02 - Nothing checks that a pointer resolves

- **Status** — `closed` (2026-08-14): `python/src/repo_tools/lint_docs.py` implements the fix below, wired into
  `mise run lint:docs` (in `lint`) and the `doc-pointers` prek hook.
- **Condition** — any pointer in a live Markdown document fails to resolve, in any of its four forms:
  a relative Markdown link, a **backticked repository-relative path**, an `ADR NNNN` reference, or an
  `@` import line in `CLAUDE.md`; and for a pointer naming a section, the quoted heading does not
  exist in the file it names. The scope is the scope of the housekeeping sweep's duplication check —
  the instruction layer, the human layer, and the live documents in `.github/`.
- **Fix** — a documentation lint that resolves all four pointer forms, asserts every quoted section
  heading exists in the file it names, asserts every `ADR NNNN` reference names a record in
  `docs/decisions/`, asserts the `@` import set in `CLAUDE.md` covers `docs/ai/` exactly, and asserts
  the roots in `platform.md` "Structure" cover the tree — every listed path present on disk, every
  root on disk listed; wired into `lint` and a hook. Both set comparisons are a few lines against a
  directory listing, and the import one has no fallback: a file added to `docs/ai/` without an import
  is unloaded for every session until someone runs a sweep, and nothing says so. Two constraints the
  lint must respect:
  - **`docs/decisions/` is history and is exempt from the resolve check.** A record may cite a path
    deliberately in the past tense, as the state before a migration, and cannot be edited to satisfy
    a linter.
  - **A quoted phrase beside a filename is not always a section pointer.** `docs/ai/code.md` reads
    ``application.md "Correctness invariants" owns what "safe" means``, where the first quote is a real
    section and the second is ordinary emphasis. The rule needs the heading to exist *or* the quote to
    be recognisable as prose, which is why this cannot be a one-line grep.

  It had no host when this was written: `crates/popcircles/` is a library about spherical geometry, there
  was no Python package, and a shell script would be a fourth place hooks are configured.

### FU-03 - Nothing couples a wire-format change to a version bump

- **Status** — `closed` (2026-08-15): the `version-bumps` prek hook, `python/src/repo_tools/lint_version_bumps.py`, is
  the tripwire the Fix below asks for, and it carries `FU-06`'s two further pairs in the one hook that
  entry prescribes. Closed while still dormant — the sweep is clean, four commits touch the directory and
  every one is an addition — with three departures from the Fix as written, each measured; see the note
  beneath it.
- **Condition** — a commit **modifies** an existing file under `crates/popcircles/src/snapshots/`
  without changing `SCHEMA_VERSION` in `crates/popcircles/src/report.rs`. The sweep is
  `git log --diff-filter=M --format=%H -- crates/popcircles/src/snapshots/`, and for each commit it
  names, `git show <sha> -- crates/popcircles/src/report.rs | rg SCHEMA_VERSION` coming back empty.
  Modification and not addition is the filter, because a new payload type is additive and owes no bump;
  a *changed* shape under an unchanged number is what leaves an old document unreadable without saying
  so.
- **Fix** — a `repo: local` hook beside `geo-data-lfs`, `files: ^crates/popcircles/src/snapshots/` and
  `pass_filenames: false`, failing when `git diff --cached --diff-filter=M --name-only` over that
  directory is non-empty while `git diff --cached -U0 -- crates/popcircles/src/report.rs` carries no
  `SCHEMA_VERSION` line. Both halves were run against this tree on 2026-08-13: the sole commit touching
  the directory adds the snapshots, so the sweep is clean, and a staged edit to one of them fires the
  check. It cannot judge whether a shape change was breaking, which makes it a tripwire of the same
  kind as `geo-data-lfs` rather than a lint of its own.

  **Three departures from that Fix, all measured on 2026-08-15.** What fires the snapshot half is a JSON
  key HEAD's snapshot published and the staged one does not, rather than any modification: `report.rs`
  rules the format additive, so a new field rewrites an existing snapshot while owing no bump, and a
  tripwire firing on that would send an author with nothing to bump to `--no-verify` — the hole `FU-05`
  closed. A renamed or removed field always drops a key, and a deleted snapshot drops all of them. Second,
  `always_run: true` rather than `files:`: on prek 0.4.13 a `files:`-gated hook is skipped when the only
  staged change is a deletion, so a withdrawn payload type would pass unseen. Third, the bump is read as
  the constant's value in HEAD against its value in the index rather than as a diff naming the constant,
  because editing the comment above it names it too.

  Two limits, since neither is visible from a green commit. The escape for a change no reader can misread
  is `SKIP=version-bumps`, which prek honours per-hook, rather than `--no-verify`. And there is no
  `mise run lint:` half: the check reads the index against HEAD, so on a CI checkout nothing is staged and
  an `--all-files` run would report a gate that had looked at nothing. The hook is the whole of it, bar the
  trigger names, which `python/tests/test_lint_version_bumps.py` pins against the tree — so a watched block
  renamed out from under the check fails in CI even though the check itself never runs there.

### FU-04 - Diagnostics have no facade

- **Status** — `closed` (2026-08-14): the library emits through the `log` facade and the CLI is its own
  subscriber, which discharges the Fix below in the cheaper of the two shapes it weighed. Two departures
  from that Fix as written, both on the measurements beneath it: `log` on the emitting side rather than
  `tracing`, and a hand-written `log::Log` in the CLI rather than `tracing-subscriber`. The cost table is
  left as it was measured.
- **Condition** — either signal that ad-hoc printing has outgrown itself. `rg -n 'print!|println!|eprintln!' crates/popcircles/src`
  matches anything: the library is printing, which is an `application.md` "Architecture" violation
  before it is a logging question. Or the CLI grows the flag shape that hand-rolls level filtering out of
  two booleans — a `--verbose` or `--quiet` long name, a field of either name in an args struct, or
  `short = 'v'` or `short = 'q'` aliasing one:
  `rg -n "^\s*(verbose|quiet)\s*:|--verbose|--quiet|short = '[vq]'" crates/popcircles-cli/src`. The two
  letters are spelled out rather than matching `short =` at large, because a condition firing on any short
  alias the CLI ever grows would ban a mechanism instead of a flag. Progress reporting is **not** this
  condition — `application.md` "Architecture" routes progress through a sink the caller supplies, and a
  sink is not a log.
- **Fix** — a record settling how diagnostics leave the library, since the facade is a project-wide
  dependency choice rather than something a PR changes in passing. Then `tracing` on the emitting side
  and `tracing-subscriber` with `json` and `env-filter`
  on the consuming side, which is what makes it the analogue of structured logging in Python: fields on
  events, and spans carrying context down a nesting the search already has — per radius, per latitude
  band, per candidate.

  The cost is what the record has to argue, measured 2026-08-13 with `cargo tree -e normal` in a scratch
  project outside this tree:

  | Addition | Crates |
  | --- | --- |
  | `log` facade in a library | 1, and it has no dependencies of its own |
  | `tracing` facade in a library | 13 |
  | `log` + `env_logger` in a binary | 25 |
  | `tracing` + `tracing-subscriber` with `json`, `env-filter` | 41 |

  Forty-one is nearly triple the trimmed clap tree the CLI accepted, and the 13 lands on the library
  whose dependency surface is held at serde. So the cheaper shape has to be ruled out
  rather than skipped: `log` in the library, bridged into the binary's subscriber by `tracing-log`, costs
  one crate and buys no spans. If the nesting turns out shallow, that is the better answer and the record
  should say so.

### FU-05 - Formatting is enforced by hooks and by nothing else

- **Status** — `closed` (2026-08-14): `lint:format` runs `prek run --all-files cargo-fmt taplo-fmt
  ruff-format` and `lint` now depends on it, per the fix below.
- **Condition** — `mise run ci` passes on a tree a formatter would rewrite. Both halves are checkable:
  `rg -n -- '--check' mise.toml` names no formatter task, which is what makes this `due` the day it is
  written, and on such a checkout `cargo fmt --all --check`, `taplo fmt --check` or
  `uv run ruff format --check .` exits non-zero while `mise run lint` stays green, which is how a sweep
  shows it has bitten. A commit made with `--no-verify`, or from a clone where `prek install` never ran,
  is the way it happens: `lint` runs clippy, ruff's linter, actionlint and the three LFS hooks, and no
  formatter at all.
- **Fix** — a `lint:format` task selecting the formatting hooks by id, `prek run --all-files cargo-fmt
  taplo-fmt ruff-format`, with `lint` depending on it — the shape `lint:lfs` already uses, which is what
  puts hooks in CI without a second copy of the rule. Those hooks rewrite rather than check, so a CI
  failure reads "files were modified by this hook" rather than naming the diff; that is the same report
  `lint:lfs` gives and is enough to stop the merge. Measured on the tree that added the taplo hook: all
  three come back clean, and `ruff check --show-files` names only `pyproject.toml`, so the Python half
  gates nothing yet but needs none of the deferral `typecheck:python` carries.

### FU-06 - Nothing couples a cache header change to a format version bump

- **Status** — `closed` (2026-08-15): the `version-bumps` hook is `FU-03`'s hook carrying this entry's two
  further pairs, which is the one hook the Fix below asks for. Closed while still dormant: the sweep is
  clean, one commit touches each cache file and each adds it with its constant. `FU-03` holds the
  three departures common to both halves; two more are this entry's, in the note beneath the Fix.
- **Condition** — a commit changes the fields of `Header` in `crates/popcircles/src/table/cache.rs`
  without changing `FORMAT_VERSION` in the same file. The sweep is
  `git log --format=%H -- crates/popcircles/src/table/cache.rs`, and for each commit it names,
  `git show <sha> -- crates/popcircles/src/table/cache.rs` carrying an added or removed field line inside
  the `struct Header` block while no line of that diff names `FORMAT_VERSION`. An **added** field fires it
  too, which is where this differs from `FU-03`: serde ignores keys it does not know, so a build reading a
  header from a later one accepts the document and then maps a payload whose layout it has no reason to
  doubt. The wire format may grow a field additively; a cache header cannot.
- **Fix** — `FU-03`'s hook with two more pairs, so one `repo: local` hook discharges all of them: a table of
  trigger and constant — files under `crates/popcircles/src/snapshots/` with `SCHEMA_VERSION`, the
  `struct Header` block in `crates/popcircles/src/table/cache.rs` with `FORMAT_VERSION`, and the
  `struct Document` and `struct Probe` blocks in `crates/popcircles/src/smallest/cache.rs` with that file's
  own `FORMAT_VERSION` — failing when `git diff --cached` touches a trigger while carrying no line naming
  that trigger's constant. Run against this tree on 2026-08-14: two commits touch `table/cache.rs`, the
  first adding the file with the constant in the same diff and the second touching neither the block nor the
  constant, and one commit adds `smallest/cache.rs` with its constant in the same diff, so the sweep is
  clean. Like `FU-03` it cannot judge whether a change was breaking, which makes it a tripwire of the same
  kind as `geo-data-lfs` rather than a lint of its own.

  The radius ledger is the same kind of file as the table header rather than the same kind as a snapshot,
  which is why it joins this entry: a run that resumes reads the document *back*, so a field added to it
  without a bump leaves an older build resuming from radii it has half understood.

  **Two departures beyond `FU-03`'s three, measured the same day.** The trigger is the whole field set of
  each named block in HEAD against its field set in the index, rather than a field line read out of a diff
  hunk — the same answer where a diff is readable, and no `struct Header {` block boundary to find inside
  one. And a watched block that is not there under that name at all fires the check too, for
  `single-unsafe-allow`'s reason: a rename would otherwise leave the tripwire watching nothing and saying so
  nowhere.

### FU-07 - A radius in kilometres is a bare f64 in more than one signature

- **Status** — `closed` (2026-08-14): `RadiusKm` in `crates/popcircles/src/geodesy.rs` is the fix below,
  and `Kernel::new` takes one, so the sweep now names no signature at all rather than one. The second
  caller was #6's search over candidate centres, not #7's binary search this entry expected: the search
  builds a kernel per candidate row and a second per widened bound radius, so it takes a radius by value
  one issue earlier than predicted.
- **Condition** — more than one **public** library signature takes a radius in kilometres as a bare
  `f64`. The sweep is `rg -n 'pub fn [a-z_]+\([^)]*radius_km: f64' crates/popcircles/src`, which names
  exactly one on 2026-08-14: `Kernel::new`. Three near misses the wording excludes deliberately —
  `Cap::over` beside it is private, `Kernel::radius_km` returns a radius rather than taking one, and
  `report.rs`'s `great_circle_km` parameter is a distance and not a radius. #5's circle evaluation adds
  none of its own: it takes a kernel, which carries the radius it was built for, so the radius reaches it
  through a type rather than through a second parameter. The second signature is expected to be #7's
  binary search over radius, which is where this fires rather than on a refactor.
- **Fix** — a `RadiusKm` newtype in `geodesy`, beside the radius and the conversion it would wrap, whose
  constructor holds what `Kernel::new` checks inline today — finite, not negative — so
  `KernelError::RadiusNotFinite` and `KernelError::RadiusNegative` move into it and no later caller
  revalidates. That is `application.md` "Architecture": prefer a type whose invalid states do not
  construct over a check repeated at every use. It waits for the second caller because introducing it for
  one trades one inconsistency for another — no scalar in this crate is wrapped, and `great_circle_km`,
  `cell_area_km2` and the CLI's `distance` all pass kilometres as `f64` — and with two callers the
  newtype is the cheaper side of that trade rather than merely the more principled one.

### FU-09 - The predicate slack is reported and nothing acts on it

- **Status** — `closed` (2026-08-14):
  [ADR 0007](decisions/0007-a-result-states-what-it-could-not-separate.md) is the record and the PR
  carrying it the implementation. Closed
  with one departure from the Fix as written, measured rather than preferred — see the note beneath it. The
  condition keeps standing and stays checkable: the field is still published, and now something acts on it.
- **Condition** — a surface publishes `predicate_slack_persons` while the search still answers an ambiguous
  comparison with a single radius. The sweep is `rg -n 'predicate_slack_persons'
  crates/popcircles/src/report.rs crates/popcircles-cli/src scripts` — widened to `report.rs` because that
  is where the field is published from, the CLI reaching it through `SmallestReport::new` rather than by
  naming it. Empty on 2026-08-14 before #8 and matching after it. What makes it a real obligation rather
  than a tidy-up is the two pieces of work that can land on a target inside it: #10 validates against the
  published 3300 km result, where the interesting shares sit on a plateau of ocean, and #18 sweeps every
  country, where a small country's own total is close to one.
- **Fix** — report the two radii around an ambiguous comparison rather than one: where a probe's population is
  within the slack of the target, the honest answer is the bracket `[short, reaching]` and a statement that
  the arithmetic cannot separate them, which is a wider bracket and not a tolerance. `smallest` already
  carries the pieces — the slack, the answer and the radius below it — so the change is a comparison and a
  field rather than a new search. It cannot be discharged by shrinking the slack: `mise run test:fold`
  measures the fold's real error at a world's magnitude as **exactly zero** against a bound of 0.0218 persons
  on 2026-08-14, so the bound is conservative by orders of magnitude and tightening it would be a claim about
  cancellation rather than about the arithmetic. Nor by a compensated fold in `circle::population`: that
  changes the answer's bits, which `search`'s determinism tests pin, and is a record's call.

  The two surfaces are the ones #8 added, and the figure has to be on them: #9 puts it on a map and #10
  validates against a share sitting on an ocean plateau. So publishing it was right and the bracket is what
  is owed — its own PR, because it changes `smallest`'s result shape and every document carrying it.

  **What landed reports a wider span than `[short, reaching]`, because that pair is not where the ambiguity
  is.** Measured against the 5 arcmin table at a share of one on 2026-08-14: nine of the run's 28 probed
  radii sit inside the slack, spanning 14 960 to 16 384 km, where the pair this Fix names is 2 km wide. So
  the field is accumulated over every radius the search probed rather than derived from the final pair, and
  it is published as a floor on the ambiguity — the climb doubles, so the radii between two probes were
  never measured and the true interval runs past both ends. ADR 0007's Context holds the measurement; that
  the span is accumulated over the visit rather than read back from the ledger is the PR's.

### FU-10 - Nothing checks rustdoc

- **Status** — `closed` (2026-08-14): `mise run lint:rustdoc` implements the fix below and `lint` depends on
  it, so CI runs it. Closed with two departures from the Fix as written, both measured rather than
  preferred — see the note beneath it.
- **Condition** — no quality gate runs rustdoc, so a doc comment naming an item that has gone survives the
  whole of `mise run ci`. The sweep is `rg -n 'cargo doc' mise.toml .github/workflows/` coming back empty,
  which is what makes this `due` the day it is written. The evidence that it bites rather than merely could:
  on the tree before #8, `cargo doc -p popcircles --no-deps` emitted seven warnings while `mise run ci` was
  green, and one of them was `search.rs`'s `# Errors` line promising a `SearchError::Radius` that went away
  when `RadiusKm::widened_by` became total under `FU-07`. It was found by a human reading the classifier
  beside it, not by a check. The other six were public docs linking to private items, and one redundant
  explicit link target. #8 cleared all seven, so the sweep above is the condition rather than a warning
  count — a clean tree with no gate is exactly the state that lets the next one through.
- **Fix** — `cargo doc -p popcircles --no-deps` as a `lint:rustdoc` task with `lint` depending on it, the
  shape `lint:rust` already uses. It needs `RUSTDOCFLAGS="-D warnings"` or an equivalent, because rustdoc
  warns and exits 0: measured on the tree before #8, the command exited 0 with seven warnings, so a task
  reading only the exit status would gate nothing. Measured again after #8: zero warnings, so the task comes
  back clean the day it is added and gates from then on.

  **What landed takes two more flags than that, because the command as written gates less than it looks
  like it does.** Both were measured on 2026-08-14 by injecting a broken link and checking the exit status.
  `--document-private-items`: without it rustdoc never reads a private item's doc comment, so a broken link
  inside `report.rs`'s own helpers exits 0 and reports nothing — and this crate keeps most of its reasoning
  on private items. `--workspace` rather than `-p popcircles`: the CLI crate has doc comments too, and its
  own intra-doc links were outside the prescribed command. The flag also retires the lint that produced six
  of the seven warnings above — "public documentation links to private item" cannot fire once private items
  are documented — which is the right trade: that lint is about doc visibility, while the defect this entry
  exists to catch is a link naming something gone. The regression test is the original defect itself,
  `search.rs`'s `[`SearchError::Radius`]`, which the task reports as "the enum `SearchError` has no variant
  or associated item named `Radius`".

### FU-11 - The cache binds no grid geometry

- **Status** — `closed` (2026-08-15):
  [ADR 0005](decisions/0005-derived-artefacts-are-keyed-and-refused.md) is the record the Fix below asks
  for, and issue #45 carried it into the tree. Both documents now embed one
  flattened attestation over the digest, the dimensions, the factor and the whole geometry, at
  `FORMAT_VERSION = 2` each. One departure from the Fix as written and one addition to it, plus a correction
  to the Condition's own illustration — all three in the note beneath the Fix.
- **Condition** — `struct Header` in `crates/popcircles/src/table/cache.rs` carries no origin and no step
  field while a command resolves a coordinate against a grid taken from flags. Two greps: `rg -n 'origin|step'
  crates/popcircles/src/table/cache.rs` naming nothing inside the `struct Header` block, and
  `rg -n 'origin_lat' crates/popcircles-cli/src/main.rs` matching. The header binds `format_version`,
  `digest`, `width`, `height`, `decimation` and `byte_order`, and `Identity` is the digest and the
  decimation — so the origin and the two steps are six numbers nothing checks against the table. Build over
  the registry raster's grid, query the same digest with `--origin-lat 0`, and the cache opens cleanly while
  every coordinate resolves to the wrong cell. The digest cannot catch it: it is over cells, and it is itself
  a flag the caller copies across. #8's documents publish the **declared** grid beside the **attested**
  digest and say which is which, which makes the gap visible rather than closed.
- **Fix** — grid geometry in the header and in `Identity`, so opening a cache compares the whole geometry
  rather than three of its numbers. It **takes a record**: what sits in the header as against inside the
  digest was a settled ruling, and this reopens it. It also bumps `FORMAT_VERSION` and invalidates
  every existing cache, which is a cost the record has to weigh rather than something a fix decides in
  passing — at full resolution a rebuild is the raster read again. The entry names the record it needs and
  stops there deliberately: prescribing the change here would settle in a register what `docs/decisions/`
  owns.

  **The geometry went into the header alone.** `Identity` holds a `Decimation`, which holds both grids, so
  half of "in the header and in `Identity`" was already there and what narrowed the geometry to three
  numbers was the header's field list and its `check` body. No caller's side moved.

  **The radius ledger was in scope, which this entry does not say and issue #45 excluded.**
  `crates/popcircles/src/smallest/cache.rs` spelled the same three numbers itself rather than embedding
  `Identity`, so it inherited nothing from the header's fix, and it mints each probe's row and column back
  onto the caller's declared grid: with the dimensions agreeing the mint succeeded and a resumed run
  published a centre whose population was measured somewhere else. Its own `FORMAT_VERSION` moved too, and
  the `version-bumps` hook gained what it could not express — a watched block whose shape governs two
  constants.

  **The Condition's illustration does not reproduce, and the count is four rather than six.** Issue #45
  measures it: a grid 21600 rows deep at 1/120° spans exactly 180°, which pins its origin latitude
  to the pole within the boundary tolerance, so `--origin-lat 0` is refused by `Grid::new` before any cache
  is opened. Six is the count of `GridArgs`' flags; the header bound two of them and four went unchecked.
  What was reachable was the origin's longitude, freely, and each step downward — and the reachable case is
  not a typo but a half-turn shift of every column over identical width, height and steps.

### FU-12 - No gate compiles this for Apple silicon

- **Status** — `closed` (2026-08-15): not by the Fix, which is declined rather than pending, but by
  `mise run release:smoke` putting the evidence it would have produced one command from anyone about to cut
  a tag. The Condition below still reads true and is meant to — the gap stands, and was taken knowingly when
  the release shape was settled. What closed the entry is that nothing here is anyone's to discharge.
- **Condition** — a release job builds a macOS artifact while no gate ever compiles for that target. Two
  greps: `rg -n 'macos' .github/workflows/release.yml` matching, and `rg -n 'runs-on|macos'
  .github/workflows/ci.yml` naming `ubuntu-latest` and nothing else. So the first time this code is compiled
  for `aarch64-apple-darwin` is on a pushed tag, including the `unsafe` mmap site
  [ADR 0006](decisions/0006-one-gated-unsafe.md) reviewed and the `#[allow(unsafe_code)]` the
  `single-unsafe-allow` hook guards. A macOS-only break
  therefore surfaces when the tag already exists, which is the half of the release's cost that CONTRIBUTING's
  Releasing section has to give a recovery for rather than prevent.
- **Fix** — `macos-latest` in `ci.yml`'s job as a matrix beside `ubuntu-latest`. It was weighed when the
  release shape was settled and not taken: the cost is a second runner on every pull request rather than on every tag. The
  repository owner's ruling (2026-08-15) declines it rather than deferring it, so a later reader proposing
  `macos-latest` is reopening a decision rather than discharging an obligation.

  What stands in its place is `mise run release:smoke`, which dispatches
  `.github/workflows/release-smoke.yml` — the same build a tag calls, with none of the jobs a tag owns. It
  is a command a person runs and not a gate, which is why it closes this entry without touching the
  Condition, and why `CONTRIBUTING.md`'s Releasing section has to say when to run it: a dispatch nobody
  runs proves nothing.

### FU-15 - cartopy is built from source because no cp314 wheel exists

- **Status** — `closed` (2026-08-15): issue #69 took cartopy out of the tree, and the Fix below landed
  with it — `.github/workflows/ci.yml` carries no `~/.cache/uv` step and no comment about one. Closed
  rather than retired because what was owed was removing that step, and it was removed for the reason it
  named; the Condition can no longer fire, since nothing here depends on cartopy's wheels either way.
  Measured on the tree that closed it: `uv pip install --only-binary :all: --python-version 3.14
  matplotlib pydantic pyproj pyright pytest ruff shapely` installs the whole dev group into a clean
  target, and `uv sync --locked --reinstall` against an empty `UV_CACHE_DIR` prints no `Building` line,
  leaves no built wheel in the cache, and takes **6.15 s** where the 25.55 s below was mostly one
  compile.
- **Condition** — that same command resolves. It is a one-line sweep with a yes-or-no answer, and it fires
  the day cartopy publishes a cp314 wheel: nothing about this repository has to change for it to become
  true, which is why the entry exists rather than a task. `uv pip install --only-binary :all:
  --python-version 3.14 pydantic` is the contrast — that one resolves today, so the cost is cartopy's
  alone.
- **Fix** — drop the `~/.cache/uv` cache step from `.github/workflows/ci.yml` and the comment beside it.
  The cache is there for exactly one reason, stated in that comment, and it is the kind of step that
  outlives its reason silently: a reader a year from now finds a cache keyed on `uv.lock` and no way to
  tell whether removing it costs 25 seconds a job or nothing at all. The 25.55 s in the Status above is
  that figure, so the entry firing is what licenses removing the step rather than guessing at it.

### FU-16 - A figure names the dataset it credits rather than reading it from the document

- **Status** — `closed` (2026-08-19, issue #94): the citation is keyed by the document. `report`'s
  `provenance` carries the registry key of the dataset a table was built from, read out of the cache
  header, and `python/src/population_circles/render_map.py` resolves that key rather than a constant of its
  own — `rg -n 'POPULATION_KEY' python/` returns nothing. Closed while still dormant: the registry carries
  one attributed row, so the Condition never fired, and what the entry guarded against is now impossible
  rather than merely absent. A document that names no dataset is refused rather than credited from a
  fallback; the two-attributed-dataset case is a fixture in `python/tests/test_render_map.py` because the
  registry cannot supply it, and one case there renders through `main` over that fixture, so putting a
  constant back where the key is read fails the suite rather than passing it.

  **The registry's second row landed and does not fire this** (2026-08-15, issue #69).
  `boundaries/coastline-1to110m.geojson` is a basemap in the public domain whose entry records that its
  terms ask for no attribution — so the count the Condition sweeps for is two rows against one selectable
  dataset while the failure the entry guards against is still impossible. What the Condition means is a
  second row a figure would have to *credit*. The note is here rather than a silent edit for `FU-08`'s
  reason: a reader should be able to tell "the row arrived and the entry held" from "nobody looked".

  **Half of this closed** (2026-08-16, issue #57). The citation was a Python constant, which the original
  title named; it is now read from `data/registry.toml`, so the text a figure carries is the text the
  registry says is owed and `rg -n 'CIESIN' python/src/` returns nothing. What remains — and what the
  entry is now titled for — is the selecting: `render_map.py` names `POPULATION_KEY`, so a figure credits
  the dataset the renderer was written against rather than the one its document was answered from.
- **Condition** — [`data/registry.toml`](../data/registry.toml) carries a second dataset **whose licence
  requires attribution**. The sweep is the count of rows with a non-empty `attribution` against the number
  of datasets `python/src/population_circles/render_map.py` can select between: one and one today. A second
  such dataset makes the credited dataset whichever one `POPULATION_KEY` happens to name, and a figure
  rendered from the other credits the wrong source while `python/tests/test_render_map.py` still passes —
  the test compares the drawn text against that same key's row, so it stays true of the wrong entry.
- **Fix** — key the citation by the document's own dataset, which means publishing enough in the document
  to choose with. That is the reason this is an entry and not a task: `report`'s `provenance` names the
  table a document was answered from by digest and grid, not the dataset the raster came from, so there is
  nothing in the wire format a renderer could select on today. Whoever fires this adds that field first,
  additively, and #56 is the change most likely to want it — naming a dataset on the command line is where
  the value to publish first exists.

### FU-28 - `Circle` is the transport for facts a figure needs and a circle does not

- **Status** — `closed` (2026-10-21, issue #124): `circle_of` returns a frozen answer with separate
  geometry, caption and provenance. `Circle.model_fields` is exactly `centre`, `radius_km` and
  `earth_radius_km`; `render_map` uses the answer for titles and dataset credit while geometry receives
  only the circle. Synthetic measured and nested documents preserve their numeric values and title text.
- **Condition** — a second field on `Circle` in
  [`python/src/population_circles/circle_document.py`](../python/src/population_circles/circle_document.py)
  is read by no caller that draws or measures. The sweep is the field list of `Circle` against what
  `circle_geometry.py` and `map_frame.py` reference: `centre`, `radius_km` and `earth_radius_km` are the
  geometry's, `population` and `share` are the caption's, and `dataset` is the credit's. A digest, a
  decimation or a table path arriving for a caption is the event.
- **Fix** — `circle_of` returns a frozen answer carrying separate geometry, caption facts and provenance,
  so drawing receives only the geometric circle and title and attribution code reads the answer.
