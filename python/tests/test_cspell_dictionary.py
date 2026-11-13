import sys
from pathlib import Path


def ordering_problem(text: str) -> str | None:
    group = "before the first group"
    words: list[str] = []
    for line in text.splitlines():
        if line.startswith("# ---") and line.endswith("---"):
            if words != sorted(words, key=str.casefold):
                return f"{group}: words are not sorted case-insensitively"
            group = line[5:-3].strip()
            words = []
        elif line and not line.startswith("#"):
            words.append(line)
    if words != sorted(words, key=str.casefold):
        return f"{group}: words are not sorted case-insensitively"
    return None


def test_ordering_problem_rejects_an_unsorted_group() -> None:
    assert ordering_problem("# --- Group ---\nzebra\nAlpha\n") == (
        "Group: words are not sorted case-insensitively"
    )


def main() -> int:
    path = Path(".cspell/project.txt")
    problem = ordering_problem(path.read_text(encoding="utf-8"))
    if problem:
        sys.stderr.write(f"{path}: {problem}\n")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
