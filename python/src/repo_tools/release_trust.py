import argparse
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
    result = run("gh", "api", f"repos/{repo}/releases/tags/{quote(tag, safe='')}", "--include")
    headers, separator, body = result.stdout.replace("\r\n", "\n").partition("\n\n")
    status = re.fullmatch(r"HTTP/[\d.]+ (\d{3})(?: [^\n]*)?", headers.split("\n")[0])
    if separator and status and status[1] == "404" and result.returncode:
        # The status must come from HTTP, not a string in a transport error or arbitrary JSON.
        error = TypeAdapter(dict[str, object]).validate_json(body)
        if error.get("message") == "Not Found":
            return
    if separator and status and status[1] == "200" and result.returncode == 0:
        refuse(
            f"release {tag} already exists, including drafts; explicit maintainer cleanup required"
        )
    refuse(f"release lookup did not establish absence: {result.stderr.strip()}")


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


def main() -> int:
    parser = argparse.ArgumentParser(description="Authorize release source and publication")
    commands = parser.add_subparsers(dest="command", required=True)
    tag_parser = commands.add_parser(
        "tag", help="Verify tag authority and refuse an existing release"
    )
    tag_parser.add_argument("--tag", required=True)
    tag_parser.add_argument("--commit", required=True)
    args = parser.parse_args()
    try:
        authorize_tag(os.environ.get("GH_REPO", ""), str(args.tag), str(args.commit))
    except (TrustError, ValidationError, OSError) as error:
        print(f"release trust refused: {error}", file=sys.stderr)
        return 1
    print(f"authorized {args.tag} at {args.commit}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
