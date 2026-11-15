---
name: session-handoff
description: Create a session handoff document to allow a new session to resume the current work with minimal context loss and token consumption.
---

# Create session handoff document

Create a session handoff document at `tmp/session-handoff.md` (overwrite if exists) so a new session can resume the current work with minimal context loss and token consumption.

Review the current conversation, project state, relevant files, and Git diff. Produce a concise, accurate, self-contained Markdown document with these sections:

1. **Objective and scope**
   - What we are building or changing, and why.
   - Current task, expected outcome, and acceptance criteria.
   - Explicit requirements, constraints, and user preferences.

2. **Current progress**
   - What has been completed and verified.
   - What is partially implemented or still pending.
   - Current blockers and unresolved issues.

3. **Architecture and decisions**
   - Relevant architecture, components, APIs, and dependencies.
   - Important decisions and their rationale.
   - Rejected approaches only when knowing why those decisions were rejected prevents repeated mistakes.

4. **Modified files and Git state**
   - Relevant file paths and a brief description of changes.
   - Uncommitted modifications, staged changes, and relevant commits.
   - Distinguish changes from this session from pre-existing changes when possible.
   - Do not include full diffs or source code unless essential.

5. **Testing and verification**
   - Tests, builds, and checks executed, including their results.
   - Known failures and suspected causes.
   - Exact commands needed to reproduce or validate the current state.
   - Clearly distinguish verified facts from assumptions.

6. **Next steps**
   - Prioritized, concrete actions to resume implementation.
   - Specific files to inspect first.
   - Any unresolved decisions requiring user input.
   - The recommended immediate next action.

7. **Critical context**
   - Important details from the conversation that cannot be reconstructed from the repository.
   - Non-obvious constraints, pitfalls, and lessons learned.
   - Do not duplicate information already available in persistent project documentation; reference those files instead.

**Rules:**

- Be concise and information-dense. Target 1,000–1,500 words maximum; use substantially less if sufficient.
- Prefer exact file paths, symbols, commands, and actionable facts over narrative summaries.
- Do not invent progress, test results, or decisions.
- Preserve uncertainty explicitly.
- Do not modify application code or execute destructive commands.
- Do not stage or commit any files.
- Do not include secrets, credentials, tokens, or sensitive environment values.
- Treat the working tree as authoritative; do not assume every change belongs to this session.
- Ensure the handoff is sufficient for a new session to continue without reading the previous conversation.

After writing the file, respond only with a brief summary of what was captured, and any critical information that could not be reconstructed.
