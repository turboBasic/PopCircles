import hashlib
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


def make_signed_tag(tmp_path: Path) -> tuple[Path, str, str, str, str, rt.TagObject]:
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
    return directory, commit, oid, trusted, untrusted, obj


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
    directory, commit, oid, trusted, untrusted, obj = make_signed_tag(tmp_path)
    responses = {
        REF_ENDPOINT: response(
            rt.TagRef(
                ref=f"refs/tags/{TAG}",
                object=rt.GitObject(type="tag", sha=oid),
            ).model_dump_json()
        ),
        KEY_ENDPOINT: response(json.dumps([[{"key": trusted}]])),
        RELEASE_ENDPOINT: response('HTTP/2.0 404 Not Found\r\n\r\n{"message":"Not Found"}', 1),
        "graphql": response(json.dumps({"data": {"repository": {"name": REPO, "release": None}}})),
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
    assert ("gh", "api", RELEASE_ENDPOINT, "--include") in repository.commands
    # The tag gate stops at published-absence; a read-only token cannot see drafts, so the
    # GraphQL draft lookup belongs to the publish path, not to this authorization.
    assert not any(command[:3] == ("gh", "api", "graphql") for command in repository.commands)
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


def test_existing_published_release_is_refused(repository: Repository) -> None:
    repository.responses[RELEASE_ENDPOINT] = response(
        "HTTP/2.0 200 OK\n\n" + json.dumps({"draft": False, "tag_name": TAG}),
    )
    with pytest.raises(rt.TrustError, match="already exists"):
        authorize(repository)


def test_published_404_with_an_existing_draft_is_refused(repository: Repository) -> None:
    repository.responses["graphql"] = response(
        json.dumps(
            {
                "data": {"repository": {"name": REPO, "release": {"id": "draft-release"}}},
            }
        )
    )
    # The draft check lives in the publish path: a read-only gate token cannot see drafts at all.
    with pytest.raises(rt.TrustError, match="already exists"):
        rt.require_no_draft(REPO, TAG)


@pytest.mark.parametrize(
    ("body", "code"),
    [
        ("", 1),
        ("not json", 0),
        ("{}", 0),
        ('{"data":null}', 0),
        ('{"data":{"repository":null}}', 0),
        ('{"data":{"repository":{"name":"other/repo","release":null}}}', 0),
        ('{"data":{"repository":{"name":"turboBasic/PopulationCircles2026"}}}', 0),
        ('{"errors":[{"message":"Resource not accessible by integration"}]}', 0),
        (
            (
                '{"data":{"repository":{"name":"turboBasic/PopulationCircles2026",'
                '"release":null}},"errors":[{"message":"rate limit"}]}'
            ),
            0,
        ),
    ],
)
def test_draft_lookup_errors_cannot_turn_a_published_404_into_absence(
    repository: Repository,
    body: str,
    code: int,
) -> None:
    repository.responses["graphql"] = response(body, code, "draft lookup failed" if code else "")
    with pytest.raises((rt.TrustError, ValidationError)):
        rt.require_no_draft(REPO, TAG)


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


def payload_bytes(triple: str) -> bytes:
    return f"binary-{triple}".encode()


def write_payloads(directory: Path, *, corrupt: str | None = None) -> None:
    for triple in rt.TRIPLES:
        (directory / f"popcircles-{triple}").write_bytes(payload_bytes(triple))
        digest = hashlib.sha256(payload_bytes(triple)).hexdigest()
        if corrupt == triple:
            digest = "0" * 64
        (directory / f"popcircles-{triple}.sha256").write_text(f"{digest}\n", encoding="utf-8")


@dataclass
class PublishState:
    responses: dict[str, subprocess.CompletedProcess[str]]
    commands: list[tuple[str, ...]] = field(default_factory=list[tuple[str, ...]])

    def released(self) -> list[tuple[str, ...]]:
        return [c for c in self.commands if c[:2] == ("gh", "release")]


@pytest.fixture
def publish_state(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> tuple[PublishState, Path, str, str]:
    monkeypatch.setenv("GIT_CONFIG_NOSYSTEM", "1")
    monkeypatch.setenv("GIT_CONFIG_GLOBAL", "/dev/null")
    monkeypatch.delenv("GIT_CONFIG_COUNT", raising=False)
    directory, commit, oid, trusted, _untrusted, obj = make_signed_tag(tmp_path)
    responses = {
        REF_ENDPOINT: response(
            rt.TagRef(
                ref=f"refs/tags/{TAG}",
                object=rt.GitObject(type="tag", sha=oid),
            ).model_dump_json()
        ),
        KEY_ENDPOINT: response(json.dumps([[{"key": trusted}]])),
        RELEASE_ENDPOINT: response('HTTP/2.0 404 Not Found\r\n\r\n{"message":"Not Found"}', 1),
        "graphql": response(json.dumps({"data": {"repository": {"name": REPO, "release": None}}})),
    }
    responses[f"repos/{REPO}/git/tags/{oid}"] = response(obj.model_dump_json())
    state = PublishState(responses)
    original_run = rt.run

    def fake_run(*args: str, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
        state.commands.append(args)
        if args[:2] == ("gh", "attestation"):
            assert args[2] == "verify", f"unexpected gh attestation mutation: {args}"
            return response("")
        if args[:2] == ("gh", "release"):
            assert args[2] == "create", f"unexpected gh release mutation: {args}"
            return response("")
        if args[:3] == ("git", "fetch", "--no-tags"):
            assert args[3] == f"https://github.com/{REPO}.git"
            return original_run(*args[:3], str(directory), *args[4:], cwd=cwd)
        if args[0] == "gh":
            assert args[1] == "api", f"unexpected GitHub mutation: {args}"
            return state.responses[args[2]]
        return original_run(*args, cwd=cwd)

    monkeypatch.setattr(rt, "run", fake_run)
    assets = tmp_path / "assets"
    assets.mkdir()
    return state, assets, commit, oid


def test_assets_rejects_missing_and_extra_payloads(tmp_path: Path) -> None:
    write_payloads(tmp_path)
    (tmp_path / "popcircles-x86_64-unknown-linux-gnu").unlink()
    with pytest.raises(rt.TrustError, match="missing"):
        rt.read_payloads(tmp_path)
    write_payloads(tmp_path)
    (tmp_path / "stray.txt").write_text("x", encoding="utf-8")
    with pytest.raises(rt.TrustError, match="extra"):
        rt.read_payloads(tmp_path)


def test_assets_requires_nonempty_files(tmp_path: Path) -> None:
    write_payloads(tmp_path)
    (tmp_path / "popcircles-aarch64-apple-darwin").write_bytes(b"")
    with pytest.raises(rt.TrustError, match="empty"):
        rt.read_payloads(tmp_path)


def test_assets_rejects_a_bad_checksum(tmp_path: Path) -> None:
    write_payloads(tmp_path, corrupt="aarch64-apple-darwin")
    with pytest.raises(rt.TrustError, match="checksum mismatch"):
        rt.require_checksums(rt.read_payloads(tmp_path))


def test_assets_rejects_a_missing_bundle(tmp_path: Path) -> None:
    write_payloads(tmp_path)
    with pytest.raises(rt.TrustError, match="missing bundle"):
        rt.verify_assets(REPO, tmp_path, tmp_path / rt.BUNDLE, TAG, "0" * 40)


def test_verify_assets_runs_every_subject_with_the_exact_policy(
    publish_state: tuple[PublishState, Path, str, str],
) -> None:
    state, directory, commit, _oid = publish_state
    write_payloads(directory)
    bundle = directory / rt.BUNDLE
    bundle.write_text("{}", encoding="utf-8")
    rt.verify_assets(REPO, directory, bundle, TAG, commit)
    verifies = [c for c in state.commands if c[:3] == ("gh", "attestation", "verify")]
    assert {c[3] for c in verifies} == {str(directory / name) for name in rt.payload_names()}
    for command in verifies:
        assert command[4:] == (
            "--repo",
            REPO,
            "--signer-workflow",
            f"{REPO}/.github/workflows/release.yml",
            "--source-ref",
            f"refs/tags/{TAG}",
            "--source-digest",
            commit,
            "--bundle",
            str(bundle),
        )
    assert state.released() == []


@pytest.mark.parametrize("with_bundle", [False, True])
def test_assets_cli_accepts_the_workflow_inventory(
    publish_state: tuple[PublishState, Path, str, str],
    monkeypatch: pytest.MonkeyPatch,
    *,
    with_bundle: bool,
) -> None:
    state, directory, commit, _oid = publish_state
    write_payloads(directory)
    argv = ["release-trust", "assets", "--directory", str(directory)]
    if with_bundle:
        bundle = directory / rt.BUNDLE
        bundle.write_text("{}", encoding="utf-8")
        argv.extend(["--bundle", str(bundle), "--tag", TAG, "--commit", commit])
    monkeypatch.setenv("GH_REPO", REPO)
    monkeypatch.setattr("sys.argv", argv)
    assert rt.main() == 0
    verifies = [c for c in state.commands if c[:3] == ("gh", "attestation", "verify")]
    assert len(verifies) == (4 if with_bundle else 0)
    assert state.released() == []


@pytest.mark.parametrize(
    "invalid",
    ["missing", "extra", "empty", "directory", "symlink", "checksum", "bundle", "external-bundle"],
)
def test_publish_inventory_failures_never_mutate_a_release(
    publish_state: tuple[PublishState, Path, str, str],
    tmp_path: Path,
    invalid: str,
) -> None:
    state, directory, commit, _oid = publish_state
    write_payloads(directory)
    bundle = directory / rt.BUNDLE
    bundle.write_text("{}", encoding="utf-8")
    binary = directory / rt.payload_names()[0]
    if invalid in {"missing", "directory", "symlink"}:
        binary.unlink()
        if invalid == "directory":
            binary.mkdir()
        elif invalid == "symlink":
            target = tmp_path / "target"
            target.write_bytes(payload_bytes(rt.TRIPLES[0]))
            binary.symlink_to(target)
    elif invalid == "extra":
        (directory / "extra").mkdir()
    elif invalid == "empty":
        binary.write_bytes(b"")
    elif invalid == "checksum":
        binary.write_bytes(b"changed")
    elif invalid == "bundle":
        bundle.unlink()
    else:
        bundle = tmp_path / rt.BUNDLE
        bundle.write_text("{}", encoding="utf-8")
    with pytest.raises(rt.TrustError):
        rt.publish(REPO, directory, bundle, TAG, commit)
    assert state.released() == []


def test_verify_assets_refuses_a_failed_verification(
    publish_state: tuple[PublishState, Path, str, str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    _state, directory, commit, _oid = publish_state
    write_payloads(directory)
    bundle = directory / rt.BUNDLE
    bundle.write_text("{}", encoding="utf-8")

    def failing(*args: str, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:  # noqa: ARG001
        if args[:3] == ("gh", "attestation", "verify"):
            return response("", 1, "failed to verify")
        message = f"unexpected command before failure: {args}"
        raise AssertionError(message)

    monkeypatch.setattr(rt, "run", failing)
    with pytest.raises(rt.TrustError, match="gh failed"):
        rt.verify_assets(REPO, directory, bundle, TAG, commit)


def test_publish_authorizes_verifies_and_creates_a_draft(
    publish_state: tuple[PublishState, Path, str, str],
) -> None:
    state, directory, commit, _oid = publish_state
    write_payloads(directory)
    bundle = directory / rt.BUNDLE
    bundle.write_text("{}", encoding="utf-8")
    rt.publish(REPO, directory, bundle, TAG, commit)
    assert any(c[:3] == ("gh", "attestation", "verify") for c in state.commands)
    assert any("verify-tag" in c for c in state.commands)
    created = state.released()
    assert len(created) == 1
    create = created[0]
    assert create[:2] == ("gh", "release")
    assert create[2] == "create"
    assert create[3] == TAG
    assert "--draft" in create
    assert "--verify-tag" in create
    assert set(create) >= {
        "--repo",
        REPO,
        "--title",
        TAG,
        "--notes",
        "",
        "--draft",
        "--verify-tag",
    }
    assert {Path(a).name for a in create if str(a).startswith(str(directory))} == {
        *rt.payload_names(),
        rt.BUNDLE,
    }


def test_publish_requires_the_bundle_name(
    publish_state: tuple[PublishState, Path, str, str],
    tmp_path: Path,
) -> None:
    state, directory, commit, _oid = publish_state
    write_payloads(directory)
    other = tmp_path / "other.json"
    other.write_text("{}", encoding="utf-8")
    with pytest.raises(rt.TrustError, match="must be named"):
        rt.publish(REPO, directory, other, TAG, commit)
    assert state.released() == []


def test_publish_records_no_mutation_before_each_precondition(
    publish_state: tuple[PublishState, Path, str, str],
) -> None:
    state, directory, commit, _oid = publish_state
    write_payloads(directory)
    bundle = directory / rt.BUNDLE
    bundle.write_text("{}", encoding="utf-8")

    with pytest.raises(rt.TrustError):
        rt.publish(REPO, directory, bundle, TAG, "0" * 40)
    assert state.released() == []
    state.responses["graphql"] = response(
        json.dumps({"data": {"repository": {"name": REPO, "release": {"id": "draft"}}}})
    )
    with pytest.raises(rt.TrustError):
        rt.publish(REPO, directory, bundle, TAG, commit)
    assert state.released() == []
    state.responses[RELEASE_ENDPOINT] = response(
        "HTTP/2.0 200 OK\n\n" + json.dumps({"tag_name": TAG}), 0
    )
    with pytest.raises(rt.TrustError):
        rt.publish(REPO, directory, bundle, TAG, commit)
    assert state.released() == []


@pytest.mark.parametrize("failure", ["create conflict", "upload failed"])
def test_publish_propagates_create_failure(
    publish_state: tuple[PublishState, Path, str, str],
    monkeypatch: pytest.MonkeyPatch,
    failure: str,
) -> None:
    state, directory, commit, _oid = publish_state
    write_payloads(directory)
    bundle = directory / rt.BUNDLE
    bundle.write_text("{}", encoding="utf-8")
    delegate = rt.run

    def failing_create(*args: str, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
        if args[:3] == ("gh", "release", "create"):
            state.commands.append(args)
            return response("", 1, failure)
        return delegate(*args, cwd=cwd)

    monkeypatch.setattr(rt, "run", failing_create)
    with pytest.raises(rt.TrustError, match="gh failed"):
        rt.publish(REPO, directory, bundle, TAG, commit)
    assert len(state.released()) == 1


@pytest.mark.parametrize("payload", rt.payload_names())
def test_a_failed_subject_verification_prevents_publication(
    publish_state: tuple[PublishState, Path, str, str],
    monkeypatch: pytest.MonkeyPatch,
    payload: str,
) -> None:
    state, directory, commit, _oid = publish_state
    write_payloads(directory)
    bundle = directory / rt.BUNDLE
    bundle.write_text("{}", encoding="utf-8")
    delegate = rt.run

    def fail_subject(*args: str, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
        if args[:4] == ("gh", "attestation", "verify", str(directory / payload)):
            state.commands.append(args)
            return response("", 1, "subject verification failed")
        return delegate(*args, cwd=cwd)

    monkeypatch.setattr(rt, "run", fail_subject)
    with pytest.raises(rt.TrustError, match="subject verification failed"):
        rt.publish(REPO, directory, bundle, TAG, commit)
    assert state.released() == []


@pytest.mark.parametrize("failure", ["published", "draft", "lookup-error"])
def test_release_state_is_checked_again_after_verification(
    publish_state: tuple[PublishState, Path, str, str],
    monkeypatch: pytest.MonkeyPatch,
    failure: str,
) -> None:
    state, directory, commit, _oid = publish_state
    write_payloads(directory)
    bundle = directory / rt.BUNDLE
    bundle.write_text("{}", encoding="utf-8")
    delegate = rt.run

    def change_release(*args: str, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
        result = delegate(*args, cwd=cwd)
        if args[:3] == ("gh", "attestation", "verify"):
            if failure in {"published", "draft"}:
                state.responses["graphql"] = response(
                    json.dumps({"data": {"repository": {"name": REPO, "release": {"id": failure}}}})
                )
            else:
                state.responses["graphql"] = response("", 1, "HTTP 503")
        return result

    monkeypatch.setattr(rt, "run", change_release)
    with pytest.raises(rt.TrustError):
        rt.publish(REPO, directory, bundle, TAG, commit)
    assert state.released() == []
