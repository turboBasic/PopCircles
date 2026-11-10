---
id: 0012
status: accepted
date: 2026-08-19
scope: output
tags: [adr, popcircles]
superseded_by: 0018
---

# ADR 0012 — A figure is deployed to a site, never attached to a tag

Superseded by [ADR 0018](0018-a-local-gallery-build-is-handed-to-pages.md).

## Decision

A rendered figure is published by deploying it to a web page CI rebuilds from the committed documents on
every push to the default branch. It is never attached to a release, and never committed — the invariant
already refuses the third, and this rules between the first two.

The page is a build product with no versioned identity: it shows what the tree currently says and carries
no history of what it said before. Whoever wants the answer as of a tag has the document, which is
committed and does carry one.

## Context

The maps are the point of this project and nobody outside this clone can see one. Once the documents are
in the tree (ADR 0011) and the basemap is committed, a figure needs no raster and no network, so CI can
draw every one of them — which turns publishing from a capability question into a channel question.

The two channels differ in who they serve. A reader arriving from `README.md` wants to look at a map now,
from a link that does not change. A reader downloading a tagged binary wants the artefacts that binary
produced, frozen beside it.

Only the first of those readers is one this project has failed so far, and the second is already served:
a release's documents are the committed corpus at that tag, and a figure is a pure function of one.

## Options

### Option 1 (SELECTED): a site, redeployed from the default branch

- Adopted because: it serves the reader who has nothing today, from one URL that stays true as the answers
  move.
- Adopted because: a figure is then never stale with respect to the tree, since nothing but the tree
  produces it.
- Adopted despite: it is a deploy surface — a public site, a token permission and an environment this
  repository did not have, and one more thing that can be broken by a change to something else.
- Adopted despite: nothing records what a figure looked like before, so a regression is invisible unless
  someone was looking at the time.

### Option 2: figures attached to each release

- Rejected because: nothing is visible between tags, and this project tags rarely — so the gallery would
  be empty for most of its life and wrong for the rest.
- Rejected because: a link to a releases page is not a link to a picture; the reader who needed this has
  to download a file to see whether they wanted it.
- Rejected despite: it is versioned by construction, needs no new surface, and reuses the publish path
  that already exists.

### Option 3: both

- Rejected because: it doubles the wiring for one deliverable and gives the same artefact two homes, one
  of which is then the one nobody checks.
- Rejected despite: it is the only option serving both readers directly rather than by argument.

## Consequences

- **A figure has no history, deliberately.** The document does, and a figure is derivable from it — so
  reconstructing an old map is a checkout and a render rather than an asset nobody fetched.
- The default branch now publishes on merge, so a change breaking the render is visible to readers and not
  only to CI. What bounds that is the render running on a pull request too, where a failure blocks.
- Which documents appear is the reader's business, not the channel's: a kind no renderer draws is stated on
  the page rather than silently absent, and ADR 0011 already refuses to bound the corpus by what can be
  drawn.
- Reopened if the page needs to say what an answer used to be, which is the one thing this channel cannot
  do.

## Links

- Issue #70 — the gallery, its workflow, and the check that keeps a figure out of the tree.
