"""Record tested PR merge trees and select immutable native packages for a tag."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib
from urllib.parse import urlencode
import zipfile


ROOT = Path(__file__).resolve().parents[2]
PLATFORMS = {"linux_id": "linux-x86_64", "macos_id": "macos-arm64", "windows_id": "windows-x86_64"}
RELEASE = ".github/workflows/release.yml"
CHECKS = ".github/workflows/check.yml"


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True, timeout=15).strip()


def version():
    with (ROOT / "Cargo.toml").open("rb") as manifest:
        return tomllib.load(manifest)["workspace"]["package"]["version"]


def api(repo, endpoint="", *, binary=False):
    result = subprocess.run(["gh", "api", "--method", "GET", f"repos/{repo}{endpoint}"],
                            capture_output=True, check=True, timeout=40)
    return result.stdout if binary else json.loads(result.stdout)


def artifacts(repo, run_id):
    listing = api(repo, f"/actions/runs/{run_id}/artifacts?per_page=100")
    items = listing["artifacts"]
    require(len(items) == listing["total_count"], "Incomplete artifact listing")
    result = {}
    for item in items:
        name = item["name"]
        require(name not in result, "Ambiguous artifact name")
        if not item["expired"] and item["size_in_bytes"] > 0:
            result[name] = item
    return result


def package_ids(items):
    packages = {}
    for platform in PLATFORMS.values():
        item = items[f"package-{platform}"]
        require(type(item["id"]) is int and item["id"] > 0, "Missing package ID")
        require(re.fullmatch(r"sha256:[0-9a-f]{64}", item["digest"]), "Missing package digest")
        packages[platform] = {"id": item["id"], "digest": item["digest"]}
    return packages


def record(out, packages):
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text(encoding="utf-8"))
    pr = event["pull_request"]
    repo = event["repository"]
    head = pr["head"]
    base = pr["base"]
    require(os.environ["GITHUB_EVENT_NAME"] == "pull_request"
            and head["repo"]["id"] == base["repo"]["id"] == repo["id"]
            and os.environ["GITHUB_REPOSITORY"] == repo["full_name"]
            and head["ref"] == "release/" + version() and base["ref"] == repo["default_branch"],
            "A release candidate must be a same-repository release PR into the default branch")
    commit = git("rev-parse", "HEAD")
    require(commit == os.environ["GITHUB_SHA"], "Checkout differs from the PR merge commit")
    info = {"repository_id": repo["id"], "pr": pr["number"], "head": head["sha"],
            "base": base["sha"], "commit": commit, "tree": git("rev-parse", "HEAD^{tree}"),
            "version": version(), "run_id": int(os.environ["GITHUB_RUN_ID"]),
            "attempt": int(os.environ["GITHUB_RUN_ATTEMPT"])}
    if packages:
        info["packages"] = package_ids(artifacts(repo["full_name"], info["run_id"]))
    out.write_text(json.dumps(info, sort_keys=True) + "\n", encoding="utf-8")


def read_record(repo, item):
    # Parse the sole named member in memory. No files from a PR are extracted or executed.
    require(0 < item["size_in_bytes"] <= 65536, "Oversized release record")
    data = api(repo, f"/actions/artifacts/{int(item['id'])}/zip", binary=True)
    require("sha256:" + hashlib.sha256(data).hexdigest() == item["digest"], "Record digest changed")
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        require(archive.namelist() == ["candidate.json"]
                and archive.getinfo("candidate.json").file_size <= 8192, "Invalid release record")
        return json.loads(archive.read("candidate.json"))


def matching_run(repo, pr, workflow, record_name, tree, expected_version, expected_jobs):
    repo_id = pr["base"]["repo"]["id"]
    head = pr["head"]
    query = urlencode({"head_sha": head["sha"], "event": "pull_request", "status": "success", "per_page": 100})
    runs = api(repo, f"/actions/workflows/{workflow.rsplit('/', 1)[1]}/runs?{query}")["workflow_runs"]
    for run in runs:
        if not (run["path"] == workflow and run["event"] == "pull_request"
                and run["status"] == "completed" and run["conclusion"] == "success"
                and run["repository"]["id"] == run["head_repository"]["id"] == repo_id
                and run["head_sha"] == head["sha"] and run["head_branch"] == head["ref"]):
            continue
        association = next((link for link in run["pull_requests"] if link["number"] == pr["number"]
                            and link["base"]["repo"]["id"] == link["head"]["repo"]["id"] == repo_id
                            and link["head"]["sha"] == head["sha"]
                            and link["head"]["ref"] == head["ref"]
                            and link["base"]["ref"] == pr["base"]["ref"]), None)
        if not association:
            continue
        run_id = run["id"]
        items = artifacts(repo, run_id)
        if record_name not in items:
            continue
        info = read_record(repo, items[record_name])
        # The recorded first parent identifies the base at checkout, even if
        # the PR association moved forward when GitHub merged it.
        expected = {"repository_id": repo_id, "pr": pr["number"], "head": head["sha"],
                    "tree": tree, "version": expected_version,
                    "run_id": run_id, "attempt": run["run_attempt"]}
        if any(info.get(key) != value for key, value in expected.items()):
            continue
        commit_sha = info["commit"]
        require(re.fullmatch(r"[0-9a-f]{40}", commit_sha), "Invalid recorded checkout")
        merge = api(repo, f"/git/commits/{commit_sha}")
        if not (merge["sha"] == commit_sha and merge["tree"]["sha"] == tree
                and [p["sha"] for p in merge["parents"]] == [info["base"], info["head"]]):
            continue
        listing = api(repo, f"/actions/runs/{run_id}/jobs?filter=latest&per_page=100")
        successful = {job["name"] for job in listing["jobs"]
                      if job["run_id"] == run_id and job["status"] == "completed" and job["conclusion"] == "success"}
        if not expected_jobs <= successful:
            continue
        if record_name == "release-candidate" and info.get("packages") != package_ids(items):
            continue
        return info
    return None


def find(repo, tag):
    require(re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo)
            and os.environ.get("GITHUB_REPOSITORY", repo) == repo, "Repository mismatch")
    expected_version = version()
    require(tag == "v" + expected_version, "Tag differs from the workspace version")
    target = git("rev-parse", "HEAD")
    tree = git("rev-parse", "HEAD^{tree}")
    remote = api(repo)
    # Resolve the tag explicitly, including annotated tags, instead of treating a branch as a tag.
    ref = api(repo, f"/git/ref/tags/{tag}")["object"]
    while ref["type"] == "tag":
        ref = api(repo, f"/git/tags/{ref['sha']}")["object"]
    require(ref["type"] == "commit" and ref["sha"] == target, "Checkout differs from the release tag")
    candidates = api(repo, f"/commits/{target}/pulls?per_page=100")
    for brief in candidates:
        pr = api(repo, f"/pulls/{int(brief['number'])}")
        if not (pr["merged"] and pr["merge_commit_sha"] == target
                and pr["head"]["repo"] and pr["head"]["repo"]["id"] == pr["base"]["repo"]["id"] == remote["id"]
                and pr["head"]["ref"] == "release/" + expected_version
                and pr["base"]["ref"] == remote["default_branch"]):
            continue
        jobs = {f"{platform} / acceptance" for platform in PLATFORMS.values()}
        packages = matching_run(repo, pr, RELEASE, "release-candidate", tree, expected_version, jobs)
        if packages and matching_run(repo, pr, CHECKS, "workspace-check-candidate", tree, expected_version, {"check"}):
            return packages
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    write = sub.add_parser("record", help="Record the actual PR checkout after a successful job")
    write.add_argument("--out", type=Path, required=True)
    write.add_argument("--packages", action="store_true")
    read = sub.add_parser("find", help="Emit GitHub step outputs; empty outputs select a native tag build")
    read.add_argument("--repo", required=True)
    read.add_argument("--tag", required=True)
    args = parser.parse_args()
    if args.command == "record":
        record(args.out, args.packages)
        return
    try:
        candidate = find(args.repo, args.tag)
    except (ValueError, KeyError, TypeError, OSError, subprocess.SubprocessError, zipfile.BadZipFile) as error:
        print(f"Candidate verification unavailable; building the tag: {error}", file=sys.stderr)
        candidate = None
    ids = {key: str(candidate["packages"][platform]["id"]) if candidate else ""
           for key, platform in PLATFORMS.items()}
    print("run_id=" + (str(candidate["run_id"]) if candidate else ""))
    print("artifact_ids=" + (",".join(ids.values()) if candidate else ""))
    for key, value in ids.items():
        print(f"{key}={value}")


if __name__ == "__main__":
    main()
