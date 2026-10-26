"""Validate a version tag, attach verified Debian assets to its draft release, and publish it."""

import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib


def validated_version():
    """Return the root package version when the release tag matches it."""
    with Path("Cargo.toml").open("rb") as manifest:
        version = tomllib.load(manifest)["package"]["version"]
    # Keep tags, output records, and prerelease detection tied to a SemVer version.
    if not re.fullmatch(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?", version):
        raise ValueError("Root package version must be a SemVer version")
    tag = os.environ.get("RELEASE_TAG", "")
    if tag not in (f"v{version}", f"rahmen-v{version}"):
        raise ValueError("Release tag does not match the root package version")
    return tag, version


def publication_tag():
    """Return the validated tag when this run may change its release."""
    tag, version = validated_version()
    event = os.environ.get("GITHUB_EVENT_NAME", "")
    if event.startswith("pull_request"):
        raise ValueError("Pull requests cannot publish release assets")
    ref = os.environ.get("GITHUB_REF", "")
    requested = os.environ.get("REQUESTED_RELEASE_TAG", "")
    # A reusable call from release-plz runs on master with an explicit tag input.
    if ref != f"refs/tags/{tag}" and requested != tag:
        raise ValueError("Publication requires a tag ref or an explicit release tag")
    return tag, version


def find_release(tag):
    """Return the release for a tag, including drafts, or None when none exists."""
    repository = os.environ["GITHUB_REPOSITORY"]
    # Release commands must select the repository even without a Git remote.
    os.environ["GH_REPO"] = repository
    # The tag endpoint hides drafts, so search the listing. Any failure stops here.
    listing = subprocess.run(
        ["gh", "api", "--paginate", f"repos/{repository}/releases",
         "--jq", f".[] | select(.tag_name == {json.dumps(tag)}) | {{draft, immutable}}"],
        capture_output=True, text=True,
    )
    if listing.returncode != 0:
        raise RuntimeError(f"Cannot list releases: {listing.stderr}")
    releases = [json.loads(line) for line in listing.stdout.splitlines() if line.strip()]
    if len(releases) > 1:
        raise RuntimeError(f"Multiple releases use tag {tag}")
    return releases[0] if releases else None


def publish(assets):
    """Upload the checked Debian packages and checksum file to a draft release."""
    tag, version = publication_tag()
    variant = os.environ.get("PACKAGE_VARIANT", "trixie-armhf")
    if not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", variant):
        raise ValueError("Invalid Debian package variant")
    packages = sorted(assets.glob("*.deb"))
    if not packages:
        raise ValueError("No Debian packages to publish")
    records = []
    for package in packages:
        with package.open("rb") as contents:
            digest = hashlib.file_digest(contents, "sha256").hexdigest()
        records.append(f"{digest}  {package.name}\n")
    expected = "".join(records)
    checksum = assets / f"SHA256SUMS-{variant}"
    if checksum.read_text() != expected:
        raise ValueError("Debian package checksums do not match")

    release = find_release(tag)
    if release is None:
        # Stay a draft until every variant is attached; immutable releases reject later uploads.
        command = ["gh", "release", "create", tag, "--verify-tag", "--generate-notes", "--draft"]
        if "-" in version.split("+", 1)[0]:
            command.append("--prerelease")
        subprocess.run(command, check=True)
    elif not release["draft"] and release["immutable"]:
        raise RuntimeError(f"Release {tag} is already published and immutable; assets cannot be added")
    # Replace only generated filenames on retries, preserving notes and other assets.
    subprocess.run(
        ["gh", "release", "upload", tag, *map(str, packages), str(checksum), "--clobber"],
        check=True,
    )


def finalize():
    """Publish the draft release once all Debian assets are attached."""
    tag, _ = publication_tag()
    release = find_release(tag)
    if release is None:
        raise RuntimeError(f"No release exists for tag {tag}")
    if release["draft"]:
        subprocess.run(["gh", "release", "edit", tag, "--draft=false"], check=True)


def main():
    command = sys.argv[1]
    if command == "validate":
        tag, version = validated_version()
        with Path(os.environ["GITHUB_OUTPUT"]).open("a") as output:
            output.write(f"release_tag={tag}\nversion={version}\n")
    elif command == "publish":
        publish(Path(sys.argv[2]))
    elif command == "finalize":
        finalize()
    else:
        raise ValueError(f"Unknown command: {command}")


if __name__ == "__main__":
    main()
