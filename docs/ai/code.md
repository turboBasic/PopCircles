# Code

The Rust and Python sections cover language-specific rules. The comments and documentation rules
apply throughout the repository. A `.toml`, `.yaml`, or `.jsonc` file that configures a build, hook, or
workflow is code, and its comments need the same care as comments in a function. `Cargo.toml` and
`pyproject.toml` define the lint levels. This file covers judgments those lints cannot express. Do not
loosen a lint just to clear an error.

## Rust

- **Raise `unsafe` as a design question.** When a change appears to need `unsafe`, discuss the design
  before using it.
- **Use `unwrap()` and `expect()` only in tests or in `main` to document an invariant.** In library
  code, return `Result` and propagate errors with `?`.
- **Exempt only lints that the code actually triggers.** For example, a test module that uses
  `expect_err` needs only `clippy::expect_used`; do not copy exemptions from a neighbouring module.
  Unneeded exemptions hide which rules matter. The number of exemptions is not a goal: add one when
  new tests in a module require it.
- **Use concrete errors at crate boundaries.** Define an error enum for each boundary and use
  `thiserror` when it helps. Add `anyhow` context only at the binary edge. Return errors for expected
  failures; do not `panic!`.
- **Make numeric conversions explicit.** Add a comment when the reason a conversion is safe is not
  obvious. [`application.md`](application.md) "Correctness invariants" defines the safety requirements
  for summation and geodesy.
- Prefer iterators and slices over index arithmetic. Where index arithmetic is the clearer
  expression of a raster traversal, keep it local and named.
- Keep modules in `foo.rs` with child modules in `foo/`; do not create `mod.rs` files.

## Python

Use Python 3.14. Do not add compatibility shims or version guards for earlier releases.

- `X | None`, not `typing.Optional`. Built-in `dict`/`list`/`tuple`, not `typing.Dict`.
- No `from __future__ import annotations`.
- No `if TYPE_CHECKING:` guard except to break an import cycle.
- Full type hints on every signature, tests included.

## Comments and docs

- Do not add Python docstrings. In Rust, use `///` only to explain a non-obvious reason.
- Comment on non-obvious reasoning; do not restate what the code does.
- Keep comments to one line.
- Match the surrounding comment density, names, and style.
- Put comments about task, hook, or lint configuration beside that configuration.
- For each change, check the affected instruction and user documentation, plus any docs naming a
  changed file, task, or convention. Update those docs in the same change.
