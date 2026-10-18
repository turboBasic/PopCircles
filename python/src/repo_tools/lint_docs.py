import functools
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]


@functools.cache
def top_level_roots() -> frozenset[str]:
    tracked = subprocess.run(
        ["git", "ls-files"],  # noqa: S607 — trusted, repo-local, no user input
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.splitlines()
    return frozenset(p.split("/")[0] for p in tracked)


# The scope of the housekeeping sweep's "Duplication" check (.claude/skills/housekeeping/SKILL.md):
# the instruction layer, .claude/skills/, .claude/agents/, the two documents in .github/, and the
# human layer.
# docs/decisions/ is a valid *target* for a pointer but is never itself scanned: a record is frozen
# once accepted, so a pointer it contains cannot be fixed to satisfy this lint.
def scope_files() -> list[Path]:
    fixed = [
        REPO_ROOT / "CLAUDE.md",
        REPO_ROOT / "AGENTS.md",
        REPO_ROOT / "docs" / "ai-instructions.md",
        REPO_ROOT / ".github" / "copilot-instructions.md",
        REPO_ROOT / ".github" / "PULL_REQUEST_TEMPLATE.md",
        REPO_ROOT / "README.md",
        REPO_ROOT / "USAGE.md",
        REPO_ROOT / "CONTRIBUTING.md",
    ]
    globbed = [
        *sorted((REPO_ROOT / "docs" / "ai").glob("*.md")),
        *sorted((REPO_ROOT / ".claude" / "skills").glob("*/SKILL.md")),
        *sorted((REPO_ROOT / ".claude" / "agents").glob("*.md")),
    ]
    return [*fixed, *globbed]


# Root-level files with no path separator that prose still points to by bare name.
ROOT_FILE_ALLOWLIST = frozenset(
    {
        "mise.toml",
        "Cargo.toml",
        "pyproject.toml",
        "README.md",
        "USAGE.md",
        "CONTRIBUTING.md",
        "CLAUDE.md",
        "AGENTS.md",
        "LICENSE",
    }
)

# docs/ai/platform.md's own rule: "Committed configuration sits at the repository root and
# documents itself" (its opening paragraph names mise.toml, Cargo.toml, pyproject.toml and
# .pre-commit-config.yaml outright); the rest here are the same kind of thing, plus
# the human layer docs/ai-instructions.md "Layering" names, which are documents and not roots.
STRUCTURE_EXEMPT_ROOTS = frozenset(
    {
        "mise.toml",
        "Cargo.toml",
        "Cargo.lock",
        "pyproject.toml",
        "uv.lock",
        ".pre-commit-config.yaml",
        ".editorconfig",
        ".gitattributes",
        ".gitignore",
        ".taplo.toml",
        ".markdownlint-cli2.jsonc",
        "cspell.config.yaml",
        ".cargo",
        ".cspell",
        ".vscode",
        "README.md",
        "USAGE.md",
        "CONTRIBUTING.md",
        "LICENSE",
    }
)

_LINK_RE = re.compile(r"\[[^\]]*\]\(([^)]+)\)")
_BACKTICK_RE = re.compile(r"`([^`]+)`")
_ADR_RE = re.compile(r"\bADR\s+(\d{4})\b")
_PATH_CANDIDATE_RE = re.compile(r"[\w.\-]+(?:/[\w.\-]+)*/?")
_SCHEME_RE = re.compile(r"^[a-zA-Z][a-zA-Z0-9+.\-]*:")
_HEADING_RE = re.compile(r"^(#{1,6})[ \t]+(.+?)[ \t]*#*[ \t]*$", re.MULTILINE)
_QUOTE_ADJACENT_RE = re.compile(r'^[ \t]?"([^"]+)"')


def display_path(path: Path) -> str:
    return str(path.relative_to(REPO_ROOT)) if path.is_relative_to(REPO_ROOT) else str(path)


@dataclass(frozen=True)
class Finding:
    path: Path
    line: int
    message: str

    def __str__(self) -> str:
        return f"{display_path(self.path)}:{self.line}: {self.message}"


def strip_fenced_code(text: str) -> str:
    lines = text.splitlines(keepends=True)
    out: list[str] = []
    in_fence = False
    for line in lines:
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
            out.append("\n")
            continue
        out.append("\n" if in_fence else line)
    return "".join(out)


def headings_of(path: Path) -> list[str]:
    if not path.exists() or path.suffix != ".md":
        return []
    text = strip_fenced_code(path.read_text(encoding="utf-8"))
    return [m.group(2).strip() for m in _HEADING_RE.finditer(text)]


def slugify(heading: str) -> str:
    # Approximates GitHub's heading-anchor algorithm; close enough for this repo's plain headings.
    text = re.sub(r"[^\w\- ]", "", heading.strip().lower())
    return text.replace(" ", "-")


def heading_exists(target: Path, claimed: str) -> bool:
    heads = headings_of(target)
    claimed_norm = claimed.strip()
    return any(h == claimed_norm for h in heads) or any(
        h.lower() == claimed_norm.lower() for h in heads
    )


def fragment_exists(target: Path, fragment: str) -> bool:
    return any(slugify(h) == fragment.lower() for h in headings_of(target))


@functools.cache
def is_ignored(rel: str) -> bool:
    # Unlike the call above, this one passes a path read out of a scanned document, which is what
    # S603 is about. It reaches git as one argv element and never a shell, so the worst a crafted
    # value can do is make check-ignore answer about some other path.
    return (
        subprocess.run(  # noqa: S603 — argv, not a shell
            ["git", "check-ignore", "-q", rel],  # noqa: S607 — trusted, repo-local
            cwd=REPO_ROOT,
            check=False,
        ).returncode
        == 0
    )


# A document may name a file no clone has: `.claude/settings.local.json` is per-developer by
# design, and the housekeeping sweep's stale-allowlist check has to name it to be readable.
# Resolving against the filesystem alone made this lint answer differently per machine — present
# for whoever created the file, absent in CI — so a path a committed ignore rule covers counts as
# a deliberate absence. That rule has to be in .gitignore rather than in a contributor's global
# excludes for the two answers to agree. Residual: a file untracked, unignored and present only
# locally still resolves here and fails in CI.
def deliberately_absent(source: Path, target: str) -> bool:
    for base in (source.parent, REPO_ROOT):
        candidate = (base / target).resolve()
        if candidate.is_relative_to(REPO_ROOT) and is_ignored(
            str(candidate.relative_to(REPO_ROOT))
        ):
            return True
    return False


def resolve_relative(source: Path, target: str) -> Path | None:
    # Convention in this repo is mixed: usually file-relative (standard Markdown), occasionally
    # a bare repo-root-relative path in prose (e.g. `docs/ai/platform.md` cited from
    # .claude/skills/, which is nowhere near a docs/ subdirectory of its own).
    for base in (source.parent, REPO_ROOT):
        candidate = (base / target).resolve()
        if candidate.exists():
            return candidate
    return None


def check_quote_adjacency(
    line: str, end: int, source: Path, target: Path, lineno: int
) -> Finding | None:
    m = _QUOTE_ADJACENT_RE.match(line[end:])
    if not m:
        return None
    claimed = m.group(1)
    if target.is_dir() or target.suffix != ".md":
        return None
    if not heading_exists(target, claimed):
        return Finding(source, lineno, f'heading "{claimed}" not found in {display_path(target)}')
    return None


def check_links(source: Path, lineno: int, line: str) -> list[Finding]:
    findings: list[Finding] = []
    for m in _LINK_RE.finditer(line):
        href = m.group(1)
        if _SCHEME_RE.match(href):
            continue
        link_path, _, fragment = href.partition("#")
        target = source if link_path == "" else (source.parent / link_path).resolve()
        if not target.exists():
            if not deliberately_absent(source, link_path):
                findings.append(Finding(source, lineno, f"link target does not resolve: {href}"))
            continue
        if fragment and not fragment_exists(target, fragment):
            findings.append(
                Finding(source, lineno, f'link fragment "#{fragment}" not found in {href}')
            )
            continue
        adjacency = check_quote_adjacency(line, m.end(), source, target, lineno)
        if adjacency:
            findings.append(adjacency)
    return findings


def check_backtick_paths(source: Path, lineno: int, line: str) -> list[Finding]:
    findings: list[Finding] = []
    for m in _BACKTICK_RE.finditer(line):
        content = m.group(1)
        # A slash alone isn't enough: `foo/` in code.md's module-naming example and
        # `turboBasic/github-actions` (a GitHub owner/repo, not a local path) both contain one.
        # Requiring the first segment to be a real root of this tree — or `../` navigation, as
        # in platform.md's `../follow-ups.md` — rules out both without a denylist.
        rooted = "/" in content and (
            content.startswith("../") or content.split("/")[0] in top_level_roots()
        )
        is_candidate = (rooted and _PATH_CANDIDATE_RE.fullmatch(content)) or (
            content in ROOT_FILE_ALLOWLIST
        )
        if not is_candidate:
            continue
        target = resolve_relative(source, content)
        if target is None:
            if not deliberately_absent(source, content):
                findings.append(Finding(source, lineno, f"path does not resolve: `{content}`"))
            continue
        adjacency = check_quote_adjacency(line, m.end(), source, target, lineno)
        if adjacency:
            findings.append(adjacency)
    return findings


def check_adr_refs(source: Path, lineno: int, line: str) -> list[Finding]:
    findings: list[Finding] = []
    for m in _ADR_RE.finditer(line):
        number = m.group(1)
        matches = sorted((REPO_ROOT / "docs" / "decisions").glob(f"{number}-*.md"))
        if len(matches) != 1:
            findings.append(Finding(source, lineno, f"ADR {number} names no single record"))
            continue
        adjacency = check_quote_adjacency(line, m.end(), source, matches[0], lineno)
        if adjacency:
            findings.append(adjacency)
    return findings


def check_pointers(path: Path) -> list[Finding]:
    text = strip_fenced_code(path.read_text(encoding="utf-8"))
    findings: list[Finding] = []
    for lineno, line in enumerate(text.splitlines(), start=1):
        findings.extend(check_links(path, lineno, line))
        findings.extend(check_backtick_paths(path, lineno, line))
        findings.extend(check_adr_refs(path, lineno, line))
    return findings


def check_claude_imports() -> list[Finding]:
    claude_md = REPO_ROOT / "CLAUDE.md"
    findings: list[Finding] = []
    imports: list[tuple[int, str]] = []
    for lineno, line in enumerate(claude_md.read_text(encoding="utf-8").splitlines(), start=1):
        m = re.match(r"^@(\S+)$", line.strip())
        if not m:
            continue
        target = m.group(1)
        imports.append((lineno, target))
        if not (REPO_ROOT / target).exists():
            findings.append(Finding(claude_md, lineno, f"@ import target does not exist: {target}"))

    imported_ai = sorted(t for _, t in imports if t.startswith("docs/ai/"))
    actual_ai = sorted(f"docs/ai/{p.name}" for p in (REPO_ROOT / "docs" / "ai").glob("*.md"))
    findings.extend(
        Finding(claude_md, 1, f"docs/ai/ file has no @ import: {missing}")
        for missing in sorted(set(actual_ai) - set(imported_ai))
    )
    findings.extend(
        Finding(claude_md, 1, f"@ import names no file under docs/ai/: {extra}")
        for extra in sorted(set(imported_ai) - set(actual_ai))
    )
    return findings


def check_structure_tree() -> list[Finding]:
    platform_md = REPO_ROOT / "docs" / "ai" / "platform.md"
    text = platform_md.read_text(encoding="utf-8")
    block_match = re.search(r"^## Structure\b.*?```text\n(.*?)```", text, re.MULTILINE | re.DOTALL)
    if not block_match:
        return [Finding(platform_md, 1, "no Structure code block found")]

    start_line = text.count("\n", 0, block_match.start(1)) + 1
    findings: list[Finding] = []
    listed_paths: list[str] = []
    for offset, block_line in enumerate(block_match.group(1).splitlines()):
        if not block_line.strip():
            continue
        listed_path = block_line.split()[0]
        listed_paths.append(listed_path)
        if not (REPO_ROOT / listed_path).exists():
            message = f"listed root absent on disk: {listed_path}"
            findings.append(Finding(platform_md, start_line + offset, message))

    listed_roots = {p.split("/")[0] for p in listed_paths}
    disk_roots = top_level_roots()

    findings.extend(
        Finding(platform_md, start_line, f"new root not listed in Structure: {missing}")
        for missing in sorted(disk_roots - listed_roots - STRUCTURE_EXEMPT_ROOTS)
    )
    findings.extend(
        Finding(platform_md, start_line, f"Structure lists a root gone from disk: {stale}")
        for stale in sorted(listed_roots - disk_roots)
    )
    return findings


def main() -> int:
    findings: list[Finding] = []
    for path in scope_files():
        findings.extend(check_pointers(path))
    findings.extend(check_claude_imports())
    findings.extend(check_structure_tree())

    for finding in findings:
        print(finding)
    if findings:
        print(f"\n{len(findings)} finding(s).")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
