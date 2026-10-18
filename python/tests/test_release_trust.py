import json
import subprocess
from dataclasses import dataclass, field
from pathlib import Path

import pytest
from pydantic import ValidationError

from repo_tools import release_trust as rt

REPO = "turboBasic/PopulationCircles2026"
TAG = "v1.2.3"
REF_ENDPOINT = f"repos/{REPO}/git/ref/tags/{TAG}"
KEY_ENDPOINT = f"users/{rt.AUTHORITY}/ssh_signing_keys?per_page=100"
RELEASE_ENDPOINT = f"repos/{REPO}/releases/tags/{TAG}"


def git(directory: Path, *args: str, stdin: str | None = None) -> str:
    result = subprocess.run(  # noqa: S603 — fixed commands and ephemeral fixture paths
        ["git", *args],  # noqa: S607 — the installed Git is the subject of these tests
        cwd=directory,
        input=stdin,
        capture_output=True,
        text=True,
        check=True,
    )
    return result.stdout.strip()


def keypair(path: Path) -> str:
    subprocess.run(  # noqa: S603 — a disposable test key, never an account key
        ["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(path)],  # noqa: S607
        capture_output=True,
        text=True,
        check=True,
    )
    return path.with_suffix(".pub").read_text(encoding="utf-8").strip()


def response(stdout: str, code: int = 0, stderr: str = "") -> subprocess.CompletedProcess[str]:
    return subprocess.CompletedProcess(["gh"], code, stdout, stderr)


@dataclass
class Repository:
    directory: Path
    commit: str
    oid: str
    trusted: str
    untrusted: str
    tag_object: rt.TagObject
    responses: dict[str, subprocess.CompletedProcess[str]]
    commands: list[tuple[str, ...]] = field(default_factory=list[tuple[str, ...]])

    def tag_response(self, obj: rt.TagObject) -> None:
        self.responses[f"repos/{REPO}/git/tags/{self.oid}"] = response(obj.model_dump_json())


@pytest.fixture
def repository(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Repository:
    monkeypatch.setenv("GIT_CONFIG_NOSYSTEM", "1")
    monkeypatch.setenv("GIT_CONFIG_GLOBAL", "/dev/null")
    monkeypatch.delenv("GIT_CONFIG_COUNT", raising=False)
    directory = tmp_path / "origin"
    directory.mkdir()
    git(directory, "init")
    git(directory, "config", "user.name", "Release test")
    git(directory, "config", "user.email", "release@example.test")
    git(directory, "-c", "commit.gpgsign=false", "commit", "--allow-empty", "-m", "fixture")
    commit = git(directory, "rev-parse", "HEAD")
    trusted = keypair(tmp_path / "trusted")
    untrusted = keypair(tmp_path / "untrusted")
    git(
        directory,
        "-c",
        "gpg.format=ssh",
        "-c",
        f"user.signingkey={tmp_path / 'trusted'}",
        "tag",
        "-s",
        TAG,
        "-m",
        "Release fixture",
    )
    oid = git(directory, "rev-parse", TAG)
    raw = git(directory, "cat-file", "tag", oid) + "\n"
    payload, _, signature = raw.partition(rt.SSH_BEGIN)
    obj = rt.TagObject(
        sha=oid,
        tag=TAG,
        object=rt.GitObject(type="commit", sha=commit),
        verification=rt.Verification(
            verified=True,
            reason="valid",
            payload=payload,
            signature=rt.SSH_BEGIN + signature,
        ),
    )
    responses = {
        REF_ENDPOINT: response(
            rt.TagRef(
                ref=f"refs/tags/{TAG}",
                object=rt.GitObject(type="tag", sha=oid),
            ).model_dump_json()
        ),
        KEY_ENDPOINT: response(json.dumps([[{"key": trusted}]])),
        RELEASE_ENDPOINT: response('HTTP/2.0 404 Not Found\r\n\r\n{"message":"Not Found"}', 1),
    }
    state = Repository(directory, commit, oid, trusted, untrusted, obj, responses)
    state.tag_response(obj)
    original_run = rt.run

    def fake_run(*args: str, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
        state.commands.append(args)
        if args[0] == "gh":
            assert args[1] == "api", f"unexpected GitHub mutation: {args}"
            return state.responses[args[2]]
        if args[:3] == ("git", "fetch", "--no-tags"):
            assert args[3] == f"https://github.com/{REPO}.git"
            return original_run(*args[:3], str(directory), *args[4:], cwd=cwd)
        return original_run(*args, cwd=cwd)

    monkeypatch.setattr(rt, "run", fake_run)
    return state


def authorize(repository: Repository) -> None:
    rt.authorize_tag(REPO, TAG, repository.commit)


def test_trusted_tag_performs_real_git_ssh_verification(repository: Repository) -> None:
    authorize(repository)
    assert any("verify-tag" in command for command in repository.commands)
    assert repository.commands[-1] == ("gh", "api", RELEASE_ENDPOINT, "--include")
    signers_option = next(
        arg
        for command in repository.commands
        for arg in command
        if arg.startswith("gpg.ssh.allowedSignersFile=")
    )
    assert not Path(signers_option.split("=", 1)[1]).exists()


def test_trusted_key_on_page_two_is_authorized(repository: Repository) -> None:
    repository.responses[KEY_ENDPOINT] = response(
        json.dumps(
            [
                [{"key": repository.untrusted}],
                [{"key": repository.trusted}],
            ]
        )
    )
    authorize(repository)
    assert ("gh", "api", KEY_ENDPOINT, "--paginate", "--slurp") in repository.commands


@pytest.mark.parametrize(
    "body",
    [
        "[]",
        "[[]]",
        "{}",
        "null",
        "not json",
        "[{}]",
        "[[{}]]",
        '[[{"key":null}]]',
        '[[{"key":""}]]',
        '[[{"key":"ssh-ed25519 invalid"}]]',
        '[[{"key":"ssh-ed25519 AAAA\\nother AAAA"}]]',
    ],
)
def test_empty_or_malformed_signing_keys_fail(repository: Repository, body: str) -> None:
    repository.responses[KEY_ENDPOINT] = response(body)
    with pytest.raises((rt.TrustError, ValidationError)):
        authorize(repository)


def test_other_accounts_key_cannot_authorize_even_when_github_says_valid(
    repository: Repository,
) -> None:
    repository.responses[KEY_ENDPOINT] = response(json.dumps([[{"key": repository.untrusted}]]))
    with pytest.raises(rt.TrustError, match="git failed"):
        authorize(repository)
    assert any("verify-tag" in command for command in repository.commands)
    assert not any(
        command[2] == RELEASE_ENDPOINT for command in repository.commands if command[0] == "gh"
    )


@pytest.mark.parametrize(
    ("verified", "reason", "signature", "payload"),
    [
        (False, "valid", None, None),
        (True, "unknown_key", None, None),
        (True, "valid", "-----BEGIN PGP SIGNATURE-----\n", None),
        (True, "valid", "", None),
        (True, "valid", None, ""),
    ],
)
def test_github_verification_is_required(
    repository: Repository,
    *,
    verified: bool,
    reason: str,
    signature: str | None,
    payload: str | None,
) -> None:
    original = repository.tag_object.verification
    verification = rt.Verification(
        verified=verified,
        reason=reason,
        signature=signature if signature is not None else original.signature,
        payload=payload if payload is not None else original.payload,
    )
    repository.tag_response(repository.tag_object.model_copy(update={"verification": verification}))
    with pytest.raises(rt.TrustError, match="GitHub must verify"):
        authorize(repository)


@pytest.mark.parametrize("change", ["name", "commit", "nested", "sha"])
def test_wrong_tag_name_target_or_object_is_rejected(repository: Repository, change: str) -> None:
    obj = repository.tag_object
    changed = {
        "name": obj.model_copy(update={"tag": "v9.9.9"}),
        "commit": obj.model_copy(update={"object": rt.GitObject(type="commit", sha="0" * 40)}),
        "nested": obj.model_copy(update={"object": rt.GitObject(type="tag", sha=repository.oid)}),
        "sha": obj.model_copy(update={"sha": "0" * 40}),
    }
    repository.tag_response(changed[change])
    with pytest.raises(rt.TrustError, match="directly name"):
        authorize(repository)


@pytest.mark.parametrize("kind", ["lightweight", "wrong-ref", "bad-sha"])
def test_remote_ref_must_be_exact_and_annotated(repository: Repository, kind: str) -> None:
    ref = rt.TagRef(ref=f"refs/tags/{TAG}", object=rt.GitObject(type="tag", sha=repository.oid))
    if kind == "lightweight":
        ref = ref.model_copy(update={"object": rt.GitObject(type="commit", sha=repository.commit)})
    elif kind == "wrong-ref":
        ref = ref.model_copy(update={"ref": "refs/tags/v9"})
    else:
        ref = ref.model_copy(update={"object": rt.GitObject(type="tag", sha="--bad")})
    repository.responses[REF_ENDPOINT] = response(ref.model_dump_json())
    with pytest.raises(rt.TrustError):
        authorize(repository)


def test_unsigned_tag_is_refused(repository: Repository) -> None:
    obj = repository.tag_object.model_dump()
    obj["verification"] = {
        "verified": False,
        "reason": "unsigned",
        "signature": None,
        "payload": None,
    }
    repository.responses[f"repos/{REPO}/git/tags/{repository.oid}"] = response(json.dumps(obj))
    with pytest.raises(ValidationError):
        authorize(repository)


def test_api_payload_must_match_the_fetched_object(repository: Repository) -> None:
    original = repository.tag_object
    changed = original.verification.model_copy(
        update={"payload": original.verification.payload + "x"}
    )
    repository.tag_response(original.model_copy(update={"verification": changed}))
    with pytest.raises(rt.TrustError, match="fetched tag"):
        authorize(repository)


def test_invalid_signature_fails_even_with_a_matching_api_response(repository: Repository) -> None:
    original = repository.tag_object.verification
    payload = original.payload.replace("Release fixture", "Changed fixture")
    oid = git(
        repository.directory,
        "hash-object",
        "-w",
        "-t",
        "tag",
        "--stdin",
        stdin=payload + original.signature,
    )
    repository.oid = oid
    obj = repository.tag_object.model_copy(
        update={
            "sha": oid,
            "verification": original.model_copy(update={"payload": payload}),
        }
    )
    repository.responses[REF_ENDPOINT] = response(
        rt.TagRef(
            ref=f"refs/tags/{TAG}",
            object=rt.GitObject(type="tag", sha=oid),
        ).model_dump_json()
    )
    repository.tag_response(obj)
    with pytest.raises(rt.TrustError, match="git failed"):
        authorize(repository)
    assert any("verify-tag" in command for command in repository.commands)


@pytest.mark.parametrize("endpoint", [REF_ENDPOINT, KEY_ENDPOINT, "tag"])
@pytest.mark.parametrize("status", [401, 403, 429, 500])
def test_api_failures_fail_closed(repository: Repository, endpoint: str, status: int) -> None:
    if endpoint == "tag":
        endpoint = f"repos/{REPO}/git/tags/{repository.oid}"
    repository.responses[endpoint] = response("", 1, f"HTTP {status}")
    with pytest.raises(rt.TrustError, match="gh failed"):
        authorize(repository)


@pytest.mark.parametrize("draft", [True, False])
def test_existing_release_is_refused(repository: Repository, *, draft: bool) -> None:
    repository.responses[RELEASE_ENDPOINT] = response(
        "HTTP/2.0 200 OK\n\n" + json.dumps({"draft": draft, "tag_name": TAG}),
    )
    with pytest.raises(rt.TrustError, match="already exists"):
        authorize(repository)


@pytest.mark.parametrize(
    ("stdout", "code", "stderr"),
    [
        ("", 1, "could not resolve host"),
        ('{"message":"Not Found"}', 1, "HTTP 404"),
        ('HTTP/2.0 401 Unauthorized\n\n{"message":"Not Found"}', 1, ""),
        ("HTTP/2.0 403 Forbidden\n\n{}", 1, ""),
        ("HTTP/2.0 429 Too Many Requests\n\n{}", 1, ""),
        ("HTTP/2.0 500 Server Error\n\n{}", 1, ""),
        ("HTTP/2.0 404 Not Found\n\n{}", 1, ""),
        ("HTTP/2.0 404 Not Found\n\nnot json", 1, ""),
        ('HTTP/2.0 404 Not Found\n\n{"message":"Not Found"}', 0, ""),
        ("HTTP/2.0 200 OK\n\nnot json", 0, ""),
    ],
)
def test_release_lookup_only_accepts_definitive_404(
    repository: Repository,
    stdout: str,
    code: int,
    stderr: str,
) -> None:
    repository.responses[RELEASE_ENDPOINT] = response(stdout, code, stderr)
    with pytest.raises((rt.TrustError, ValidationError)):
        authorize(repository)


@pytest.mark.parametrize(
    ("repo", "tag", "commit"),
    [
        ("", TAG, "0" * 40),
        ("bad/path/extra", TAG, "0" * 40),
        (REPO, "other", "0" * 40),
        (REPO, "v..bad", "0" * 40),
        (REPO, TAG, "--option"),
        (REPO, TAG, "abcdef"),
    ],
)
def test_invalid_cli_inputs_fail_before_github(
    repository: Repository,
    repo: str,
    tag: str,
    commit: str,
) -> None:
    with pytest.raises(rt.TrustError):
        rt.authorize_tag(repo, tag, commit)
    assert not any(command[0] == "gh" for command in repository.commands)
