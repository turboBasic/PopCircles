from pathlib import Path

import pytest

from repo_tools import lint_docs


def not_ignored(_path: str) -> bool:
    return False


def test_slugify_matches_github_anchor_style() -> None:
    assert lint_docs.slugify("Correctness invariants") == "correctness-invariants"
    assert lint_docs.slugify("Fetching") == "fetching"


def test_strip_fenced_code_blanks_content_but_keeps_line_count() -> None:
    text = "before\n```text\ndocs/ai/*.md\n```\nafter\n"
    stripped = lint_docs.strip_fenced_code(text)
    assert stripped.splitlines() == ["before", "", "", "", "after"]


def test_headings_of_ignores_fenced_code(tmp_path: Path) -> None:
    doc = tmp_path / "doc.md"
    doc.write_text("# Title\n\n```text\n# Not a heading\n```\n\n## Real section\n")
    assert lint_docs.headings_of(doc) == ["Title", "Real section"]


def test_heading_exists_is_case_insensitive(tmp_path: Path) -> None:
    doc = tmp_path / "doc.md"
    doc.write_text("## Structure\n")
    assert lint_docs.heading_exists(doc, "Structure")
    assert lint_docs.heading_exists(doc, "structure")
    assert not lint_docs.heading_exists(doc, "Nope")


def test_fragment_exists_uses_slug(tmp_path: Path) -> None:
    doc = tmp_path / "doc.md"
    doc.write_text("## Correctness invariants\n")
    assert lint_docs.fragment_exists(doc, "correctness-invariants")
    assert not lint_docs.fragment_exists(doc, "nope")


def test_resolve_relative_prefers_file_relative(tmp_path: Path) -> None:
    (tmp_path / "sibling.md").write_text("# Sibling\n")
    source = tmp_path / "source.md"
    source.write_text("# Source\n")
    resolved = lint_docs.resolve_relative(source, "sibling.md")
    assert resolved == (tmp_path / "sibling.md").resolve()


def test_resolve_relative_falls_back_to_repo_root(tmp_path: Path) -> None:
    source = tmp_path / "source.md"
    source.write_text("# Source\n")
    resolved = lint_docs.resolve_relative(source, "mise.toml")
    assert resolved == (lint_docs.REPO_ROOT / "mise.toml").resolve()


def test_resolve_relative_returns_none_when_nothing_matches(tmp_path: Path) -> None:
    source = tmp_path / "source.md"
    source.write_text("# Source\n")
    assert lint_docs.resolve_relative(source, "does/not/exist.md") is None


def test_check_links_flags_missing_target(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(lint_docs, "is_ignored", not_ignored)
    source = tmp_path / "source.md"
    findings = lint_docs.check_links(source, 1, "[missing](missing.md)")
    assert len(findings) == 1
    assert "does not resolve" in findings[0].message


def test_check_links_flags_missing_fragment(tmp_path: Path) -> None:
    (tmp_path / "target.md").write_text("## Real heading\n")
    source = tmp_path / "source.md"
    findings = lint_docs.check_links(source, 1, "[t](target.md#missing-heading)")
    assert len(findings) == 1
    assert "fragment" in findings[0].message


def test_check_links_passes_on_matching_fragment(tmp_path: Path) -> None:
    (tmp_path / "target.md").write_text("## Real heading\n")
    source = tmp_path / "source.md"
    findings = lint_docs.check_links(source, 1, "[t](target.md#real-heading)")
    assert findings == []


def test_check_links_skips_external_urls(tmp_path: Path) -> None:
    source = tmp_path / "source.md"
    findings = lint_docs.check_links(source, 1, "[ext](https://example.com/x)")
    assert findings == []


def test_check_links_validates_adjacent_quoted_heading(tmp_path: Path) -> None:
    (tmp_path / "target.md").write_text("## Real section\n")
    source = tmp_path / "source.md"
    ok = lint_docs.check_links(source, 1, '[t](target.md) "Real section"')
    bad = lint_docs.check_links(source, 1, '[t](target.md) "Missing section"')
    assert ok == []
    assert len(bad) == 1
    assert "heading" in bad[0].message


def test_check_backtick_paths_ignores_illustrative_placeholder() -> None:
    source = lint_docs.REPO_ROOT / "docs" / "ai" / "code.md"
    findings = lint_docs.check_backtick_paths(source, 1, "a module is `foo.rs` plus `foo/`")
    assert findings == []


def test_check_backtick_paths_ignores_external_repo_reference() -> None:
    source = lint_docs.REPO_ROOT / "docs" / "ai" / "platform.md"
    findings = lint_docs.check_backtick_paths(source, 1, "reuse `turboBasic/github-actions`")
    assert findings == []


def test_check_backtick_paths_flags_real_missing_path() -> None:
    source = lint_docs.REPO_ROOT / "README.md"
    findings = lint_docs.check_backtick_paths(source, 1, "see `docs/does-not-exist.md`")
    assert len(findings) == 1
    assert "does not resolve" in findings[0].message


def test_check_backtick_paths_accepts_relative_navigation() -> None:
    source = lint_docs.REPO_ROOT / "docs" / "ai" / "platform.md"
    findings = lint_docs.check_backtick_paths(source, 1, "see `../follow-ups.md`")
    assert findings == []


def test_check_backtick_paths_validates_adjacent_quoted_heading() -> None:
    source = lint_docs.REPO_ROOT / "docs" / "ai" / "code.md"
    ok = lint_docs.check_backtick_paths(source, 1, '`docs/ai/platform.md` "Structure"')
    bad = lint_docs.check_backtick_paths(source, 1, '`docs/ai/platform.md` "Not a real section"')
    assert ok == []
    assert len(bad) == 1


def test_check_adr_refs_flags_unknown_number() -> None:
    source = lint_docs.REPO_ROOT / "docs" / "ai" / "platform.md"
    findings = lint_docs.check_adr_refs(source, 1, "see ADR 9999 for context")
    assert len(findings) == 1
    assert "9999" in findings[0].message


def test_check_adr_refs_passes_for_known_record() -> None:
    source = lint_docs.REPO_ROOT / "docs" / "ai" / "platform.md"
    findings = lint_docs.check_adr_refs(source, 1, "see ADR 0001 for context")
    assert findings == []


def test_check_claude_imports_is_clean_on_real_repo() -> None:
    assert lint_docs.check_claude_imports() == []


def test_check_structure_tree_is_clean_on_real_repo() -> None:
    assert lint_docs.check_structure_tree() == []


def test_scope_files_are_all_real_files() -> None:
    files = lint_docs.scope_files()
    assert len(files) > 0
    assert all(f.is_file() for f in files)


def test_main_is_clean_on_real_repo() -> None:
    assert lint_docs.main() == 0


def test_agent_files_are_in_scope_and_checked(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    agent = tmp_path / ".claude" / "agents" / "bad.md"
    agent.parent.mkdir(parents=True)
    agent.write_text("# Bad\n\n[gone](docs/gone.md)\n")
    # Both are @functools.cache and read REPO_ROOT when called, so a warm entry from an earlier
    # test answers about the real repo here — and one left warm by this test answers about tmp_path
    # in whatever runs next.
    lint_docs.top_level_roots.cache_clear()
    lint_docs.is_ignored.cache_clear()
    monkeypatch.setattr(lint_docs, "REPO_ROOT", tmp_path)
    monkeypatch.setattr(lint_docs, "is_ignored", not_ignored)
    try:
        assert agent in lint_docs.scope_files()
        findings = lint_docs.check_pointers(agent)
        assert len(findings) == 1
        assert "does not resolve" in findings[0].message
    finally:
        lint_docs.top_level_roots.cache_clear()
