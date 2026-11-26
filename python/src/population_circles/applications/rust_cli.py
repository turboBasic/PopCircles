import subprocess
from collections.abc import Callable
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
Invoke = Callable[[list[str]], subprocess.CompletedProcess[bytes]]


class WorkflowError(Exception):
    pass


def invoke_cli(arguments: list[str]) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(  # noqa: S603 — argument vector, never a shell
        ["mise", "run", "popcircles-cli", "--", *arguments],  # noqa: S607 — checkout's mise task
        cwd=REPO_ROOT,
        stdout=subprocess.PIPE,
        check=False,
    )


def child(stage: str, arguments: list[str], invoke: Invoke) -> subprocess.CompletedProcess[bytes]:
    try:
        return invoke(arguments)
    except OSError as error:
        message = f"{stage}: could not launch Rust CLI: {error}"
        raise WorkflowError(message) from error


def require_success(stage: str, result: subprocess.CompletedProcess[bytes]) -> None:
    if result.returncode != 0:
        message = f"{stage}: Rust CLI exited {result.returncode}; see child diagnostic above"
        raise WorkflowError(message)
