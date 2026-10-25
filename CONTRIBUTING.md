# Contributing

## Setup

Everything is pinned in `mise.toml`; nothing is installed globally.

```sh
mise trust          # first time in a fresh clone
mise run setup      # toolchains, dependencies, git hooks
```

## The loop

```sh
mise run fmt        # format
mise run lint       # lint
mise run typecheck  # type-check
mise run test       # test
mise run ci         # all of the above — what CI runs
mise run build      # the release binary, at build/target/release/popcircles
```

Deselected from `test` and `ci`, because each needs the raster, time, or both — `mise.toml`'s comment on
each says which:

```sh
mise run test:validate      # the real raster, end to end, against the published result
mise run test:python-raster # the registry's fetched rows against the files on disk
mise run bench              # kernel construction and circle evaluation
mise run bench:table        # the table build — writes 7.5 GB, which is why it is separate
```

A benchmark asserts nothing and no gate compares it against a baseline, so its figures are read rather
than checked, and a figure worth keeping belongs to the issue or the PR that measured it.

Each task's description in `mise.toml` names what it actually runs; `mise run ci` is the check to
run before opening a PR. Hooks run through
[prek](https://github.com/j178/prek), not pre-commit; the config keeps pre-commit's filename and
format because prek reads it.

## Questions and ideas

Use [Discussions](https://github.com/turboBasic/PopulationCircles2026/discussions) for usage questions,
feedback on results, and ideas that are still being explored. Open an [issue](https://github.com/turboBasic/PopulationCircles2026/issues)
for a reproducible bug or work ready to be scoped; link the discussion when an idea moves into an issue.
Use a pull request to discuss a proposed code change.

## Data

Input datasets live in [`data/`](data/README.md), described for a machine in
[`data/registry.toml`](data/registry.toml) and for a person in [`data/README.md`](data/README.md). A
large one is published rather than carried, so a clone is cheap and fetching is a separate step:

```sh
mise run data:get   # fetch every registered dataset not already here, and verify it
```

`data:get` needs no account anywhere: it reads the registry, verifies each file against the recorded
checksum before putting it in place, and prints the attribution each licence requires
([`data/README.md`](data/README.md#getting-it)).

A new input dataset goes in `data/<kind>/` with a registry entry. Never make a test depend on raster
content: a CI checkout has none.

### Publishing a dataset

A dataset too large to carry in the repository is published as an asset on a **data-only tag**. Cutting
one is rare enough that its body is written from this list rather than from whatever the person cutting
it remembered.

The tag and its assets:

1. Name it `data-vN`. It must **not** match `v*`, which is what `.github/workflows/release.yml` triggers
   on — a data tag matching that pattern publishes a binary release nobody asked for.
2. Name each asset for the dataset's key plus its extension, so the asset, the file on disk and the
   registry row are one string. Attach the `.sha256` beside it.

The body is read by someone who has never seen this repository, and the asset's name is this project's
own description rather than the publisher's, so it lets them assume nothing. Each item names where its
text already exists, and items 1 and 3 to 6 all come from the dataset's own heading in
[`data/README.md`](data/README.md), which is named for its key:

1. **What the file is** — its nodata sentinel, from the dataset's row in
   [`data/registry.toml`](data/registry.toml); its extent and pixel type from that heading, which is
   where those two are recorded.
2. **Its grid** — dimensions and cell size, from the registry row.
3. **Which variant**, where a dataset is published in several that differ in values — the sentence naming
   the variant and the figure that distinguishes it. The asset's name will not carry it and the numbers
   change, so the body is the only place a fetcher can learn it.
4. **Provenance** — the format, the published name, the publisher and the DOI or tag identifying it.
5. **The licence** and its URL.
6. **The citation** verbatim, where the licence requires one. A fetcher acquires the obligation with the
   bytes, so it travels with them.
7. **The `sha256` and `bytes`**, from the row, so the download can be verified by hand and not only by
   `mise run data:get`.

A figure the body needs and no row holds is a finding, not something to measure into the release notes:
measure it into [`data/registry.toml`](data/registry.toml) where a machine reads it, or into
[`data/README.md`](data/README.md) where a person does, then quote it.

### Verifying a published dataset

`mise run data:get` verifies what it fetches against the registry, so this is not that. It is the other
route: obtaining an **independent** copy from the publisher, which is what makes the recorded `sha256`
something more than a record of one download. Nobody needs it to work on this repository; someone
checking that a republished asset is what it claims does.

For the population raster it needs a free [NASA Earthdata Login][urs-new], because the archive is
behind URS OAuth and answers an anonymous request with a 401 and a redirect rather than the file. A
browser download from the [dataset's granules in Earthdata Search][gpw-search] is simplest — pick the
2020, 30 arc-second GeoTIFF granule. For `curl` or `wget`, NASA documents the [cookie and netrc
setup][urs-curl] the redirect needs.

The granule is a ~405 MB zip. Extract just the raster and rename it to what the registry expects:

```sh
unzip -j <granule>.zip '*.tif' -d data/population/
mv data/population/gpw_v4_population_count_*_2020_30_sec.tif \
   data/population/population-count-2020-30arcsec.tif
shasum -a 256 data/population/population-count-2020-30arcsec.tif
```

The zip carries one `.tif` per year, so the glob is what selects 2020 rather than an assumption about
the name inside.

**Check the last line against the `sha256` in [`data/registry.toml`](data/registry.toml)**, which holds
it and the `bytes` beside it. A match means the copy every figure was measured from is the copy the
archive serves. A mismatch is a finding rather than a broken download: it means the two differ, and the
registry — measured from ours — is what would then need re-measuring. Say so rather than working around
it. [`data/README.md`](data/README.md) is where the provenance that identifies the dataset lives.

[gpw-search]: https://search.earthdata.nasa.gov/search/granules?p=C3540909447-ESDIS
[urs-curl]: https://urs.earthdata.nasa.gov/documentation/for_users/data_access/curl_and_wget
[urs-new]: https://urs.earthdata.nasa.gov/users/new

## Sending a change

- Conventional Commits, commitizen's default types. The PR title follows the same format; the
  commit-msg hook enforces it locally and CI checks both the title and the commits.
- Branch off `main`; do not push to `main` directly. The ruleset refuses it, and the one exception is the
  repository owner, whose `Repository admin` role bypasses the required checks — used for changes that
  touch no code, with the `guard-direct-push` hook running `mise run ci` in their place.
- Rebase, never merge. `main` rejects a merge commit, and PRs land squashed or rebased; when your
  branch falls behind, `git rebase main` rather than merging it in. `mise run setup` also points this
  clone at the same rule, so an accidental `git pull` fails early instead of after a push.
- **Do not port code from the upstream C++ project.** It carries no licence. Implementations
  written from a description of the algorithm are welcome; transliterations are not.
- Documentation moves with the change. Stale framing is a defect, not a follow-up.

## Asking the agent

Comment `/agent <what you want done>` **on an issue**. A bare `/agent` takes the issue's own body as the
instruction. On a pull request it does not do the useful thing — it would branch from `main` rather than
from that pull request's head and open a second, unrelated one — so it is an issue or nothing until
something resolves the head ref. The agent works on a branch, runs `mise run ci`, and opens a **draft**
pull request whose body reports whether that gate passed — read that line before the diff, because a red gate
there is the agent saying so rather than hiding it. It cannot push to `main`, and its pull request needs
the same three green checks as anyone else's.

Two things to know before using it:

- **Only the repository owner can.** A `/agent` comment from anyone else starts nothing and says nothing —
  there is no reply, deliberately. The run would hold a metered API key and a write-scoped token, which is
  not a combination a public repository hands to whoever asks.
- **Every run spends real money**, billed per token rather than by subscription
  ([ADR 0010](docs/decisions/0010-automation-brings-its-own-provider.md)). The cost follows what the agent
  chooses to read, not the length of what you asked, so a vague instruction over a large tree is the
  expensive shape. Nothing in this repository caps it; a ceiling belongs on the provider's side.

`.github/workflows/agent-run.yml` declares the `model` input and its default, and can also be run on its
own with `gh workflow run agent-run.yml -f instruction='…'` when there is no issue to comment on.

## Releasing

A release is a tag and the two binaries it attaches, from one workspace version and no registry; this is
the sequence for cutting one.

The `v*` release and `data-v*` dataset tag namespaces are protected: creating, retargeting or deleting
one requires the Repository Admin bypass. Future users granted that role inherit the same permission.
The ruleset does not replace the signed-tag authorization below; release tags must still be signed by
a trusted `turboBasic` SSH signing key.

```sh
mise run release:smoke  # both build legs on demand, macOS included, publishing nothing
```

Run it before cutting a tag, and after bumping a pinned toolchain. Nothing else in this repository ever
compiles for `aarch64-apple-darwin` — `ci.yml` runs only on Linux — so
without it the first macOS compile of a tree is the tag you are trying to ship. It builds the branch as
`origin` has it, takes about as long as a release, and leaves a run page and its two artifacts behind:
no tag, no Release. It runs the same build a tag runs — `.github/workflows/release-smoke.yml` and
`release.yml` call one `build-binaries.yml` — and neither the gate nor the publish job, which only a tag
declares.

1. Bump `version` in `[workspace.package]`, which is the only place it lives, and land it on `main` like
   any other change. Every report snapshot moves with it, because every document carries `tool_version`.
2. Sign the merged commit's tag `vX.Y.Z` for that same version and push the tag. The gate compares the
   two and refuses a mismatched tag, so a mismatch costs a run rather than a wrong binary.
   Register the public key once as a **signing key** on `turboBasic`, and use an email verified on that
   account as the tagger email. Supply the agent-backed public key and Git settings per invocation; the
   private key stays in 1Password:

   ```sh
   SSH_AUTH_SOCK="$signing_agent_socket" git -c gpg.format=ssh \
     -c user.signingkey="$signing_public_key" -c user.email="$verified_email" \
     tag -s vX.Y.Z -m 'Release vX.Y.Z' "$merged_commit"
   git push origin refs/tags/vX.Y.Z
   ```

   The authority is `turboBasic`'s current GitHub SSH signing keys, including rotations and removals,
   rather than repository-pinned keys. Both SSH verification and GitHub's verified-tag result must
   succeed. The source gate refuses lightweight, nested, unsigned, untrusted or mismatched tags,
   existing releases (drafts included) and lookup errors before building.
   Historical unsigned tags remain unchanged and do not satisfy this prospective policy.
3. Write the notes by hand, then publish. The workflow attaches five assets to a **draft** with an
   empty body — the two binaries, their two checksums, and one `provenance.sigstore.json` bundle
   attesting those four payloads — so nothing is visible to a fetcher until you say so. The notes have
   to say that `schema_version` is a contract across releases while a cache or a ledger may be
   invalidated by this one and rebuilt, and nothing can generate that from a commit range. Publish
   only after you have downloaded all five draft assets and verified them against their bundle, with
   the expected tag
   and source commit. Run `mise run release:trust -- assets --directory DIR --bundle
   DIR/provenance.sigstore.json --tag vX.Y.Z --commit SHA` with
   `GH_REPO=turboBasic/PopulationCircles2026`. Publish only a draft whose run passed `authorize`
   and attestation, then `gh release edit vX.Y.Z --draft=false`.

When a run fails, inspect release state before retrying; a lookup error does not establish absence.

- **No release exists.** The tag is still retractable and the version is not spent. Re-run the
  workflow if the cause was the runner rather than the commit; otherwise explicitly delete the
  unpublished tag, fix the cause, and sign the same version again.
- **An incomplete draft exists.** Explicitly delete it as a maintainer before any rerun, including
  after runner failures; the workflow refuses existing releases. If the commit must change, explicitly
  delete the unpublished tag too before signing its replacement.
- **It is published.** That version is spent, so the next attempt is a version bump rather than a moved
  tag — a tag that moves lies to everyone who already fetched it. Publishing the draft is therefore the
  one step in this sequence that cannot be taken back, which is why it is yours and not the workflow's.

### Verifying a downloaded release

Each release includes a `.sha256` checksum beside each binary and a `provenance.sigstore.json` bundle.
Verify a downloaded binary against the bundle to confirm that GitHub's release workflow produced it
from the expected source commit.

Set `TAG` to the release tag and `COMMIT` to its source commit. To find the commit, inspect the tag's
commit target, for example with `git ls-remote` and the annotated tag's peeled ref:

```sh
git ls-remote https://github.com/turboBasic/PopulationCircles2026.git \
  "refs/tags/$TAG" "refs/tags/$TAG^{}"
```

The peeled `^{}` line is the commit. Download the matching binary and `provenance.sigstore.json`
from the release, then verify them:

```sh
gh attestation verify ./popcircles-aarch64-apple-darwin \
  --repo turboBasic/PopulationCircles2026 \
  --signer-workflow turboBasic/PopulationCircles2026/.github/workflows/release.yml \
  --source-ref "refs/tags/$TAG" \
  --source-digest "$COMMIT" \
  --bundle ./provenance.sigstore.json
```

Use the Linux binary when applicable. The same command verifies either `.sha256` file; pass the
downloaded checksum file as the first argument.

## The conventions

The conventions AI tools follow are the conventions humans follow here.
[`docs/ai-instructions.md`](docs/ai-instructions.md) holds the project invariants and maps the rest;
[`docs/ai/`](docs/ai/) holds the per-subject detail, and every file in it applies to every change —
the split is so each subject can be corrected on its own, not so any of them is optional. Read those
before a first change. [`docs/decisions/`](docs/decisions/) says why a constraint exists, and
[`docs/follow-ups.md`](docs/follow-ups.md) what is still owed; open those when you need them.

A decision about how the repository is built is recorded as an ADR rather than argued again — but only
where reversing it would cost more than a PR, and then on one page. Most choices are explained by the PR
that makes them, and the work itself is decomposed in GitHub issues.
