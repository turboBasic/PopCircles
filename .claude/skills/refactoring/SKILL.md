---
name: refactoring
description: Guide a behavior-preserving refactor through investigation, scope and design decisions, incremental implementation, and independent review. Use when changing code structure while preserving existing behavior.
---

# Refactoring workflow and handoff contracts

Resolve every repository path in this skill from the repository root.

Use the sequence **Evidence → Behavior → Dependencies → Responsibilities → Boundaries → Refactoring → Validation**. Keep investigation, design authority, implementation, and independent review in separate turns or contexts.

Treat the templates as handoff formats. Keep task scope and evidence in the GitHub issue and pull request; record an architecture decision in `docs/decisions/` only when it meets the bar in `docs/ai/platform.md`.

## 1. Establish the decision and baseline

The requester and existing Product Owner agent define preserved behavior, scope, constraints, and acceptance checks in the GitHub issue. The Investigator records the current state before anyone proposes a design. Label uncertain behavior as a question; do not resolve it silently by changing code.

## 2. Investigate (read-only)

Give the Legacy Code Investigator a target and ask for the investigation report using `.claude/skills/refactoring/templates/investigation.md`. It traces entry points and callers, tests, side effects, dependencies, shared state, and relevant accepted decisions or history artifacts with read-only source tools. When function complexity or size would inform the investigation, it may run the pinned Lizard tool with `mise exec -- lizard <target files>` and report the metric, command, scope, and limits. Treat those results as leads to inspect in source, not verdicts or pass/fail gates; Lizard does not establish coupling. Report other measurements only when their method is available to the Investigator. Prioritize demonstrated change risk and avoid proposing a rewrite.

**Handoff to Software Architect and Product Owner:** provide the investigation report plus unresolved behavior questions. The Architect owns structural options and trade-offs; the Product Owner owns behavior, scope, and acceptance. They agree the smallest useful structural change. Record the decision, rejected alternatives, compatibility constraints, and success signals in the issue or pull request using `.claude/skills/refactoring/templates/design-decision.md`.

## 3. Implement incrementally

The Refactoring Engineer receives the approved decision, touched files, preserved behavior, and validation commands in an implementation brief using `.claude/skills/refactoring/templates/implementation-brief.md`. It changes one structural seam at a time, preserves externally observable behavior, updates affected callers/tests/fixtures/config/exports, and reports each validation result. It does not broaden scope or add speculative abstractions. If evidence contradicts the decision, stop at a safe point and return the discrepancy to the Architect and Product Owner.

## 4. Review independently

Start the Architectural Reviewer in a fresh context after implementation. Supply the original investigation and decision artifacts, the change/diff, and validation results. Ask it to find concrete behavior regressions, missed callers, weakened boundaries, unnecessary indirection, compatibility breaks, and evidence gaps. The Reviewer does not edit files. Findings cite paths and symbols and distinguish confirmed defects from questions. If findings require a design change, route it back to the Architect and Product Owner before further implementation.

## 5. Close out

The Engineer resolves accepted findings, and the Reviewer rechecks affected areas. The Product Owner confirms behavior and acceptance; the Architect confirms the boundary and intended coupling change. Keep the closeout in the issue or pull request; capture durable rationale in an ADR only when it meets the project bar.

## Handoff payloads

- **Investigator → Architect/Product Owner:** scope, observed behavior, evidence and source locations, callers/dependencies, risks, unknowns, candidate seams (not a prescribed design).
- **Architect/Product Owner → Engineer:** approved boundary and rationale, explicit in/out of scope, invariants, compatibility requirements, acceptance criteria, validation commands.
- **Engineer → Reviewer:** exact change or diff, touched callers and exports, validation evidence, deviations, and known limits.
- **Reviewer → decision owners:** prioritized findings with evidence, impact, and an accept/fix/question recommendation. No edits.

## Agent independence

Do not pass the Engineer's self-review as the reviewer's only context. Include the initial evidence and design decision so the Reviewer can compare intent with implementation. Source evidence remains authoritative over earlier speculative options.
