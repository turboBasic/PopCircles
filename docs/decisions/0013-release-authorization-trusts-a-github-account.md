---
id: 0013
status: accepted
date: 2026-10-18
scope: build
tags: [adr, popcircles]
---

# ADR 0013 — Release authorization trusts a GitHub account

## Decision

A release source is authorized by the current SSH signing identity of the designated GitHub account,
`turboBasic`. Repository-pinned maintainer keys do not establish that authority. Both cryptographic
validity and GitHub's verification of the signed tag are required.

## Context

Signed release tags need an answer to who may authorize the source, beyond whether a signature is
mathematically valid. The repository already publishes through GitHub, and the owner chose the account
identity as the authority for future releases in issue #63.

Pinning keys in the repository would make rotation a repository change and leave a removed account key
trusted until that change landed. Account authority instead makes the provider's current identity
state part of every release decision.

## Options

### Option 1 (SELECTED): the designated account's current signing identity

- Adopted because: key rotation and removal take effect through the identity the owner already manages.
- Adopted despite: provider availability and account security become release authorization dependencies.

### Option 2: repository-pinned maintainer keys

- Rejected because: repository review would become a second authority for key rotation and revocation.
- Rejected despite: a pinned trust root is reviewable locally and independent of account changes.

## Consequences

The release gate refuses a tag when either signature check fails or the account authority cannot be
established. A valid signature from another account is insufficient. Removing a signing key can
therefore prevent a later authorization check of an older tag; this policy governs release admission,
not a promise that account state preserves historical trust forever.

Future changes must preserve account authority unless a new record supersedes this ruling. Reopen it
if releases must be authorized independently of GitHub's identity service.

## Links

- [Issue #63](https://github.com/turboBasic/PopulationCircles2026/issues/63) — policy and release proof.
