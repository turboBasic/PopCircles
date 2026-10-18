import argparse
import hashlib
import os
import re
import subprocess
import sys
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Never
from urllib.parse import quote

from pydantic import BaseModel, ConfigDict, TypeAdapter, ValidationError

AUTHORITY = "turboBasic"
SSH_BEGIN = "-----BEGIN SSH SIGNATURE-----\n"
SSH_END = "-----END SSH SIGNATURE-----\n"
TRIPLES = ("aarch64-apple-darwin", "x86_64-unknown-linux-gnu")
BUNDLE = "provenance.sigstore.json"
SIGNER_SUFFIX = "/.github/workflows/release.yml"


def payload_names() -> tuple[str, ...]:
    return tuple(
        name
        for triple in TRIPLES
        for name in (f"popcircles-{triple}", f"popcircles-{triple}.sha256")
    )


class TrustError(Exception):
    pass


class GitObject(BaseModel):
    model_config = ConfigDict(strict=True)
    type: str
    sha: str


class TagRef(BaseModel):
    model_config = ConfigDict(strict=True)
    ref: str
    object: GitObject


class Verification(BaseModel):
    model_config = ConfigDict(strict=True)
    verified: bool
    reason: str
    signature: str
    payload: str


class TagObject(BaseModel):
    model_config = ConfigDict(strict=True)
    sha: str
    tag: str
    object: GitObject
    verification: Verification


class SigningKey(BaseModel):
    model_config = ConfigDict(strict=True)
    key: str


def refuse(message: str) -> Never:
    raise TrustError(message)


def run(*args: str, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(  # noqa: S603 — argv only, with validated refs and repository
        args, cwd=cwd, capture_output=True, text=True, check=False
    )


def checked(*args: str, cwd: Path | None = None) -> str:
    result = run(*args, cwd=cwd)
    if result.returncode:
        refuse(f"{args[0]} failed: {result.stderr.strip()}")
    return result.stdout


def api(endpoint: str, *options: str) -> str:
    return checked("gh", "api", endpoint, *options)


def require_absent(repo: str, tag: str) -> None:
    owner, name = repo.split("/")
    query = (
        "query($owner: String!, $name: String!, $tag: String!) {"
        "repository(owner: $owner, name: $name) {"
        "name: nameWithOwner release(tagName: $tag) { id } } }"
    )
    mapping = TypeAdapter(dict[str, object])
    result = mapping.validate_json(
        api(
            "graphql",
            "-f",
            f"query={query}",
            "-f",
            f"owner={owner}",
            "-f",
            f"name={name}",
            "-f",
            f"tag={tag}",
        )
    )
    if result.get("errors", []) != []:
        refuse("release lookup returned GraphQL errors")
    data = mapping.validate_python(result.get("data"), strict=True)
    repository = mapping.validate_python(data.get("repository"), strict=True)
    if repository.get("name") != repo or "release" not in repository:
        refuse("release lookup did not establish the requested repository's release state")
    if repository["release"] is not None:
        refuse(
            f"release {tag} already exists, including drafts; explicit maintainer cleanup required"
        )


def allowed_signers(directory: Path) -> Path:
    pages = TypeAdapter(list[list[SigningKey]]).validate_json(
        api(f"users/{AUTHORITY}/ssh_signing_keys?per_page=100", "--paginate", "--slurp")
    )
    keys = [key.key for page in pages for key in page]
    if not keys:
        refuse(f"{AUTHORITY} has no SSH signing keys")
    signers = directory / "allowed-signers"
    lines: list[str] = []
    for key in keys:
        if not re.fullmatch(r"[\w@.+-]+ [A-Za-z0-9+/]+={0,3}(?: [^\r\n]*)?", key):
            refuse("malformed SSH signing key")
        public = " ".join(key.split(" ", 2)[:2])
        key_file = directory / "public-key"
        key_file.write_text(f"{public}\n", encoding="utf-8")
        checked("ssh-keygen", "-l", "-f", str(key_file))
        lines.append(f'{AUTHORITY} namespaces="git" {public}\n')
    signers.write_text("".join(lines), encoding="utf-8")
    return signers


def authorize_tag(repo: str, tag: str, commit: str) -> None:
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo):
        refuse("GH_REPO must name owner/repository")
    if not tag.startswith("v") or run("git", "check-ref-format", f"refs/tags/{tag}").returncode:
        refuse("release tag must be a valid v* tag")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        refuse("commit must be a full commit SHA")
    ref_name = f"refs/tags/{tag}"
    ref = TagRef.model_validate_json(api(f"repos/{repo}/git/ref/tags/{quote(tag, safe='')}"))
    if ref.ref != ref_name or ref.object.type != "tag":
        refuse("remote ref must name the requested annotated tag")
    oid = ref.object.sha
    if not re.fullmatch(r"[0-9a-f]{40}", oid):
        refuse("remote tag object has an invalid SHA")
    obj = TagObject.model_validate_json(api(f"repos/{repo}/git/tags/{oid}"))
    if obj.sha != oid or obj.tag != tag or obj.object != GitObject(type="commit", sha=commit):
        refuse("tag must directly name the expected commit with the expected signed name")
    verification = obj.verification
    if (
        not verification.verified
        or verification.reason != "valid"
        or not verification.payload
        or not verification.signature.startswith(SSH_BEGIN)
        or not verification.signature.endswith(SSH_END)
    ):
        refuse("GitHub must verify a valid SSH signature with a nonempty payload")
    with TemporaryDirectory(prefix="release-trust-") as temporary:
        directory = Path(temporary)
        signers = allowed_signers(directory)
        git_dir = directory / "repository"
        checked("git", "init", "--bare", str(git_dir))
        checked("git", "fetch", "--no-tags", f"https://github.com/{repo}.git", oid, cwd=git_dir)
        raw = checked("git", "cat-file", "tag", oid, cwd=git_dir)
        payload, separator, signature = raw.partition(SSH_BEGIN)
        if (
            not separator
            or payload != verification.payload
            or SSH_BEGIN + signature != verification.signature
            or payload.split("\n\n", 1)[0].splitlines()[:3]
            != [f"object {commit}", "type commit", f"tag {tag}"]
        ):
            refuse("fetched tag does not match GitHub's signed payload and signature")
        checked(
            "git",
            "-c",
            "gpg.format=ssh",
            "-c",
            f"gpg.ssh.allowedSignersFile={signers}",
            "-c",
            "gpg.ssh.revocationFile=",
            "-c",
            "gpg.ssh.program=ssh-keygen",
            "verify-tag",
            oid,
            cwd=git_dir,
        )
    require_absent(repo, tag)


def read_payloads(directory: Path, allow: frozenset[str] = frozenset()) -> dict[str, Path]:
    files = {path.name: path for path in directory.iterdir()}
    expected = {*payload_names(), *allow}
    missing = [name for name in expected if name not in files]
    extra = sorted(set(files) - expected)
    if missing or extra:
        refuse(f"payload inventory mismatch: missing={missing or 'none'} extra={extra or 'none'}")
    for name, path in files.items():
        if path.is_symlink() or not path.is_file():
            refuse(f"payload {name} must be a regular file")
        if path.stat().st_size == 0:
            refuse(f"payload {name} is empty")
    return {name: files[name] for name in payload_names()}


def require_checksums(files: dict[str, Path]) -> None:
    for triple in TRIPLES:
        binary = files[f"popcircles-{triple}"]
        checksum_path = files[f"popcircles-{triple}.sha256"]
        digest = hashlib.sha256(binary.read_bytes()).hexdigest()
        line = checksum_path.read_text(encoding="utf-8").strip()
        if line != digest and line.split(" ", 1)[0].split("\t", 1)[0] != digest:
            refuse(f"checksum mismatch for popcircles-{triple}")


def signer(repo: str) -> str:
    return f"{repo}{SIGNER_SUFFIX}"


def verify_bundle(repo: str, directory: Path, bundle: Path, tag: str, commit: str) -> None:
    for name in payload_names():
        checked(
            "gh",
            "attestation",
            "verify",
            str(directory / name),
            "--repo",
            repo,
            "--signer-workflow",
            signer(repo),
            "--source-ref",
            f"refs/tags/{tag}",
            "--source-digest",
            commit,
            "--bundle",
            str(bundle),
        )


def verify_assets(repo: str, directory: Path, bundle: Path, tag: str, commit: str) -> None:
    if not bundle.is_file():
        refuse(f"missing bundle {bundle.name}")
    if bundle.name != BUNDLE or bundle.resolve() != (directory / BUNDLE).resolve():
        refuse(f"bundle must be {BUNDLE} in the payload directory")
    files = read_payloads(directory, allow=frozenset({BUNDLE}))
    require_checksums(files)
    verify_bundle(repo, directory, bundle, tag, commit)


def publish(repo: str, directory: Path, bundle: Path, tag: str, commit: str) -> None:
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo):
        refuse("GH_REPO must name owner/repository")
    if bundle.name != BUNDLE:
        refuse(f"the bundle must be named {BUNDLE}")
    authorize_tag(repo, tag, commit)
    verify_assets(repo, directory, bundle, tag, commit)
    require_absent(repo, tag)
    checked(
        "gh",
        "release",
        "create",
        tag,
        "--repo",
        repo,
        "--title",
        tag,
        "--notes",
        "",
        "--draft",
        "--verify-tag",
        *[str(directory / name) for name in payload_names()],
        str(bundle),
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Authorize release source and publication")
    commands = parser.add_subparsers(dest="command", required=True)
    tag_parser = commands.add_parser(
        "tag", help="Verify tag authority and refuse an existing release"
    )
    tag_parser.add_argument("--tag", required=True)
    tag_parser.add_argument("--commit", required=True)
    assets_parser = commands.add_parser(
        "assets", help="Validate payloads, with optional provenance"
    )
    assets_parser.add_argument("--directory", required=True)
    assets_parser.add_argument("--bundle")
    assets_parser.add_argument("--tag")
    assets_parser.add_argument("--commit")
    publish_parser = commands.add_parser(
        "publish", help="Authorize, verify and create a draft release"
    )
    publish_parser.add_argument("--directory", required=True)
    publish_parser.add_argument("--bundle", required=True)
    publish_parser.add_argument("--tag", required=True)
    publish_parser.add_argument("--commit", required=True)
    args = parser.parse_args()
    repo = os.environ.get("GH_REPO", "")
    try:
        if args.command == "tag":
            authorize_tag(repo, str(args.tag), str(args.commit))
            print(f"authorized {args.tag} at {args.commit}")
        elif args.command == "assets":
            directory = Path(args.directory)
            if args.bundle or args.tag or args.commit:
                if not (args.bundle and args.tag and args.commit):
                    refuse("--bundle, --tag and --commit are supplied together")
                if Path(args.bundle).name != BUNDLE:
                    refuse(f"the bundle must be named {BUNDLE}")
                verify_assets(repo, directory, Path(args.bundle), str(args.tag), str(args.commit))
            else:
                require_checksums(read_payloads(directory))
            print("payload inventory validated")
        else:
            publish(
                repo,
                Path(args.directory),
                Path(args.bundle),
                str(args.tag),
                str(args.commit),
            )
            print(f"published draft {args.tag}")
    except (TrustError, ValidationError, OSError) as error:
        print(f"release trust refused: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
