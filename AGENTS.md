# Codex instructions

Read every file below. None is optional and none is per-task.

- [`docs/ai-instructions.md`](docs/ai-instructions.md) — the invariants, the non-negotiables, the layering rule
- [`docs/ai/platform.md`](docs/ai/platform.md) — tooling, data, quality gates, records, CI
- [`docs/ai/code.md`](docs/ai/code.md) — Rust and Python conventions
- [`docs/ai/application.md`](docs/ai/application.md) — the problem, the approach, the correctness invariants, the copying rule
- [`docs/ai/frontend.md`](docs/ai/frontend.md) — Web page architecture

For reused skills and personas, follow [the harness guidance](docs/ai/platform.md#ai-harnesses).

## Codex-specific access

- When needed for the requested task, request elevated access for writes to this repository's `.git` metadata, GitHub CLI operations, and access to the configured Mise, Cargo, Prek, UV, and RTK cache directories. Discover cache paths from the tools' configuration instead of guessing.
- When a configured tool fails because it cannot access its cache, follow [cache access recovery](docs/ai/platform.md#cache-access) before treating it as blocked.
- Keep each elevated command limited to the requested operation and relevant paths. These instructions authorize requesting access; sandbox and approval controls still decide whether it is granted.
- If access is denied, continue with work that does not need it and report the specific blocker.
