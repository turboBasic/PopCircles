import re
from pathlib import Path

import pytest

from repo_tools import release_smoke as rs
from repo_tools import release_trust as rt

WORKFLOWS = Path(__file__).resolve().parents[2] / ".github/workflows"


def workflow(name: str) -> str:
    return (WORKFLOWS / name).read_text(encoding="utf-8")


def subjects(found: list[str]) -> list[str]:
    return [finding.split(":")[0] for finding in found]


def test_the_expected_artifacts_are_the_build_matrix() -> None:
    triples = re.findall(r"^\s*triple:\s*(\S+)$", workflow("build-binaries.yml"), re.MULTILINE)
    assert set(rs.ARTIFACTS) == {f"popcircles-{triple}" for triple in triples}


def test_the_smoke_dispatches_the_workflow_this_script_watches() -> None:
    text = workflow(rs.WORKFLOW)
    assert re.search(r"^\s*workflow_dispatch:$", text, re.MULTILINE)
    assert "uses: ./.github/workflows/build-binaries.yml" in text


# A smoke publishes nothing because its workflow declares no job that could,
# and neither wrapper tells its jobs apart by the event. Both are properties of the files, so this
# is where they are checked — at run time there is nothing left to look at.
def test_the_smoke_workflow_can_neither_publish_nor_write() -> None:
    text = workflow(rs.WORKFLOW)
    assert "contents: write" not in text
    assert "gh release" not in text


def test_neither_wrapper_forks_on_the_event() -> None:
    for name in (rs.WORKFLOW, "release.yml"):
        assert "github.event_name" not in workflow(name)


def test_a_skipped_job_is_not_a_finding() -> None:
    skipped = [
        rs.Job("skipped", "Gate", "https://example.invalid/1"),
        rs.Job("success", "build / Build aarch64-apple-darwin", "https://example.invalid/2"),
    ]
    assert rs.findings(skipped, frozenset(rs.ARTIFACTS)) == []


def test_a_failed_leg_and_a_missing_artifact_are_each_named() -> None:
    failed = [rs.Job("failure", "build / Build aarch64-apple-darwin", "https://example.invalid/2")]
    found = rs.findings(failed, frozenset({rs.ARTIFACTS[1]}))
    assert subjects(found) == ["build / Build aarch64-apple-darwin", rs.ARTIFACTS[0]]
    assert "failure" in found[0]


def jobs_of(text: str) -> dict[str, str]:
    return dict(
        re.findall(r"(?ms)^  ([\w-]+):\n(.*?)(?=^  [\w-]+:|\Z)", text.split("\njobs:\n")[1])
    )


def steps_of(job: str) -> list[str]:
    return re.split(r"(?m)^      - ", job.split("    steps:\n")[1])[1:]


def action_step(job: str, action: str) -> str:
    matches = [
        step
        for step in steps_of(job)
        if step.startswith(f"uses: {action}@") or f"uses: {action}@" in step
    ]
    assert len(matches) == 1
    return matches[0]


def paths_of(step: str) -> set[str]:
    return set(re.findall(r"^            (dist/\S+)$", step, re.MULTILINE))


def test_the_release_chain_gates_publish_behind_attestation() -> None:
    text = workflow("release.yml")
    jobs = jobs_of(text)
    assert set(jobs) == {"authorize", "gate", "build", "attest", "publish"}
    for job, needs in (
        ("gate", "authorize"),
        ("build", "gate"),
        ("attest", "build"),
        ("publish", "attest"),
    ):
        assert re.findall(r"^    needs: (.+)$", jobs[job], re.MULTILINE) == [needs]
    assert not re.search(r"^\s*(?:if|continue-on-error):", text, re.MULTILINE)
    assert "|| true" not in text
    assert "group: release-${{ github.ref }}" in text
    assert "cancel-in-progress: false" in text


def test_only_attest_and_publish_receive_write_identity() -> None:
    text = workflow("release.yml")
    assert re.findall(r"^permissions:\n  (.+)$", text, re.MULTILINE) == ["contents: read"]
    for name, job in jobs_of(text).items():
        writes = re.findall(r"^\s*([\w-]+): write$", job, re.MULTILINE)
        assert set(writes) == {"attest": {"id-token", "attestations"}, "publish": {"contents"}}.get(
            name, set()
        )
        assert "write-all" not in job
        if name == "attest":
            assert "      contents: read\n" in job


def test_the_attest_job_names_each_payload_subject() -> None:
    text = workflow("release.yml")
    attest = jobs_of(text)["attest"]
    step = action_step(attest, "actions/attest")
    assert "actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6 # v4" in step
    assert "id: attest\n" in step
    assert "          subject-path: |\n" in step
    expected = {f"dist/{name}" for name in rt.payload_names()}
    assert paths_of(step) == expected
    assert "predicate" not in step
    assert "sbom" not in step
    steps = steps_of(attest)
    inventory = next(
        i
        for i, s in enumerate(steps)
        if "run: mise run release:trust -- assets --directory dist\n" in s
    )
    signing = steps.index(step)
    verification = next(
        i
        for i, s in enumerate(steps)
        if 'cp "${{ steps.attest.outputs.bundle-path }}" dist/provenance.sigstore.json' in s
    )
    upload = action_step(attest, "actions/upload-artifact")
    assert inventory < signing < verification < steps.index(upload)
    assert "name: release-assets\n" in upload
    assert paths_of(upload) == expected | {"dist/provenance.sigstore.json"}
    assert "if-no-files-found: error\n" in upload


def test_attest_downloads_only_the_current_runs_build_artifacts() -> None:
    download = action_step(jobs_of(workflow("release.yml"))["attest"], "actions/download-artifact")
    assert "          pattern: popcircles-*\n" in download
    assert "          merge-multiple: true\n" in download
    assert "          path: dist\n" in download
    assert not re.search(
        r"^          (?:name|run-id|repository|github-token):", download, re.MULTILINE
    )


def test_publish_downloads_only_the_attested_handoff() -> None:
    publish = jobs_of(workflow("release.yml"))["publish"]
    download = action_step(publish, "actions/download-artifact")
    assert "          name: release-assets\n" in download
    assert "          path: dist\n" in download
    assert not re.search(
        r"^          (?:pattern|merge-multiple|run-id|repository|github-token):",
        download,
        re.MULTILINE,
    )
    steps = steps_of(publish)
    assert steps.index(download) < next(
        i for i, s in enumerate(steps) if "release:trust -- publish" in s
    )


@pytest.mark.parametrize(("job", "command"), [("attest", "assets"), ("publish", "publish")])
def test_both_jobs_prepare_the_event_commit_and_verify_the_same_policy(
    job: str, command: str
) -> None:
    body = jobs_of(workflow("release.yml"))[job]
    checkout = action_step(body, "actions/checkout")
    assert "          ref: ${{ github.sha }}\n" in checkout
    assert "          persist-credentials: false\n" in checkout
    action_step(body, "jdx/mise-action")
    steps = steps_of(body)
    sync = next(i for i, s in enumerate(steps) if "run: uv sync --locked\n" in s)
    verify = next(s for s in steps if f"release:trust -- {command}" in s and "--bundle" in s)
    assert steps.index(checkout) < sync < steps.index(verify)
    assert f"mise run release:trust -- {command} --directory dist" in verify
    assert '--bundle dist/provenance.sigstore.json --tag "$TAG" --commit "$COMMIT"' in verify
    for key, value in (
        ("GH_TOKEN", "github.token"),
        ("GH_REPO", "github.repository"),
        ("TAG", "github.ref_name"),
        ("COMMIT", "github.sha"),
    ):
        assert f"          {key}: ${{{{ {value} }}}}\n" in verify


def test_no_existing_release_mutation_remains() -> None:
    text = workflow("release.yml")
    assert "--clobber" not in text
    assert "gh release upload" not in text
    assert "gh release view" not in text


def test_the_wrappers_and_shared_build_cannot_delegate_write_permissions() -> None:
    for name in ("build-binaries.yml", "release-smoke.yml"):
        text = workflow(name)
        assert not re.search(r"^\s*[\w-]+: (?:write|write-all)$", text, re.MULTILINE)
        assert "permissions:\n  contents: read\n" in text
        assert "gh release" not in text
        assert "actions/attest" not in text
    for name in ("release.yml", "release-smoke.yml"):
        calls = re.findall(r"^    uses: (.+)$", workflow(name), re.MULTILINE)
        assert calls == ["./.github/workflows/build-binaries.yml"]
    assert not re.search(r"^    uses:", workflow("build-binaries.yml"), re.MULTILINE)


def test_the_attester_is_the_release_workflow_itself() -> None:
    assert (
        rt.signer("turboBasic/PopCircles") == "turboBasic/PopCircles/.github/workflows/release.yml"
    )
    attest = jobs_of(workflow("release.yml"))["attest"]
    assert "uses:" not in attest.split("    steps:\n")[0]
