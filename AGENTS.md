# Codex instructions

Read [CLAUDE.md](CLAUDE.md) and every file it imports with `@`, resolving imports relative to the
file containing them. All imported instructions apply to every task.

For reused skills and personas, follow [the harness guidance](docs/ai/platform.md#ai-harnesses).

## Codex-specific access

- When needed for the requested task, request elevated access for writes to this repository's `.git` metadata, GitHub CLI operations, and the configured Mise, Cargo, Prek, and UV cache directories. Discover cache paths from the tools' configuration instead of guessing.
- When a configured tool fails because it cannot write its cache, follow [cache access recovery](docs/ai/platform.md#cache-access) before treating it as blocked.
- Keep each elevated command limited to the requested operation and relevant paths. These instructions authorize requesting access; sandbox and approval controls still decide whether it is granted.
- If access is denied, continue with work that does not need it and report the specific blocker.
