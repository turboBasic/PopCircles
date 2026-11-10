---
id: 0018
status: accepted
date: 2026-11-10
scope: output
tags: [adr, popcircles]
---

# ADR 0018 — A local gallery build is handed to Pages

## Decision

Keep the gallery at its stable Pages URL. A maintainer builds it locally and uploads the finished site
as a uniquely named ZIP to one draft Release. A manually dispatched workflow deploys that exact asset.

## Context

The gallery build needs prepared country data and can take substantial time. Running it on every merge
repeats that work on a hosted runner and couples publication to unrelated changes. Local publishing
keeps the existing reader-facing site while making the maintainer choose when to publish.

## Options

### Option 1 (SELECTED): locally build, then hand off the site to Pages

- Adopted because: the runner only stages and deploys a finished site, while the maintainer controls
  when expensive gallery generation occurs.
- Adopted despite: local credentials and a draft Release handoff add steps to publishing.

### Option 2: rebuild the gallery on every default-branch push

- Rejected because: each merge repeats the expensive country sweep and rendering, even when no gallery
  publication is intended.
- Rejected despite: it keeps publication automatic and requires no local publishing command.

### Option 3: attach gallery files to a release instead of Pages

- Rejected because: readers would lose the stable gallery URL and have to download files to view maps.
- Rejected despite: a release already provides versioned asset storage.

## Consequences

- The Pages URL remains stable, and each deployment uses the exact ZIP selected by its workflow input.
- A failed deployment leaves uploaded assets available for retry; successful cleanup retains the
  deployed asset and removes older gallery ZIPs.

## Links

- Issue #263 — local gallery publishing through a release handoff.
- Supersedes [ADR 0012](0012-a-figure-is-deployed-to-a-site-not-attached-to-a-tag.md).
