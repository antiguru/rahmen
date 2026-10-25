"""Validate a version tag and publish verified Debian assets to its release."""

import hashlib
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib
from urllib.parse import quote


CHECKSUM = "SHA256SUMS-trixie-armhf"


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


def publish(assets):
    """Upload the checked Debian packages and checksum file to an existing tag."""
    tag, version = validated_version()
    event = os.environ.get("GITHUB_EVENT_NAME", "")
    if event.startswith("pull_request"):
        raise ValueError("Pull requests cannot publish release assets")
    ref = os.environ.get("GITHUB_REF", "")
    requested = os.environ.get("REQUESTED_RELEASE_TAG", "")
    # A reusable call from release-plz runs on master with an explicit tag input.
    if ref != f"refs/tags/{tag}" and requested != tag:
        raise ValueError("Publication requires a tag ref or an explicit release tag")
    packages = sorted(assets.glob("*.deb"))
    if not packages:
        raise ValueError("No Debian packages to publish")
    expected = "".join(
        f"{hashlib.file_digest(package.open('rb'), 'sha256').hexdigest()}  {package.name}\n"
        for package in packages
    )
    checksum = assets / CHECKSUM
    if checksum.read_text() != expected:
        raise ValueError("Debian package checksums do not match")

    repository = os.environ["GITHUB_REPOSITORY"]
    # Only a confirmed 404 permits creation. Auth and transport failures stop here.
    release = subprocess.run(
        ["gh", "api", "--include", f"repos/{repository}/releases/tags/{quote(tag, safe='')}"],
        capture_output=True, text=True,
    )
    status = release.stdout.splitlines()[0].split() if release.stdout else []
    if len(status) < 2 or status[0] not in ("HTTP/1.1", "HTTP/2.0", "HTTP/2"):
        raise RuntimeError(f"Cannot determine release status: {release.stderr}")
    if status[1] == "404":
        command = ["gh", "release", "create", tag, "--verify-tag", "--generate-notes"]
        if "-" in version.split("+", 1)[0]:
            command.append("--prerelease")
        subprocess.run(command, check=True)
    elif status[1] != "200" or release.returncode != 0:
        raise RuntimeError(f"Cannot inspect release: {release.stderr}")
    # Replace only generated filenames on retries, preserving notes and other assets.
    subprocess.run(
        ["gh", "release", "upload", tag, *map(str, packages), str(checksum), "--clobber"],
        check=True,
    )


def main():
    command = sys.argv[1]
    if command == "validate":
        tag, version = validated_version()
        with Path(os.environ["GITHUB_OUTPUT"]).open("a") as output:
            output.write(f"release_tag={tag}\nversion={version}\n")
    elif command == "publish":
        publish(Path(sys.argv[2]))
    else:
        raise ValueError(f"Unknown command: {command}")


if __name__ == "__main__":
    main()
