"""Exercise release publication with real files and a recording gh substitute."""

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("publish_debian_release.py")
DRAFT = '{"draft":true,"immutable":false}'
PUBLISHED = '{"draft":false,"immutable":true}'


class PublicationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.assets = self.root / "assets"
        self.assets.mkdir()
        self.package = self.assets / "rahmen_0.2.0_armhf.deb"
        self.package.write_bytes(b"Debian package fixture")
        self.checksum = self.assets / "SHA256SUMS-trixie-armhf"
        self.write_checksum()
        (self.root / "Cargo.toml").write_text('[package]\nversion = "0.2.0"\n')
        self.log = self.root / "gh.jsonl"
        gh = self.root / "gh"
        gh.write_text(
            '#!' + sys.executable + '\n'
            'import json, os, sys\n'
            'assert os.environ.get("GH_REPO") == os.environ["GITHUB_REPOSITORY"]\n'
            'with open(os.environ["GH_LOG"], "a") as f:\n'
            '    f.write(json.dumps(sys.argv[1:]) + "\\n")\n'
            'if sys.argv[1] == "api":\n'
            '    print(os.environ.get("GH_RELEASES", ""))\n'
            '    sys.exit(int(os.environ.get("GH_API_EXIT", "0")))\n'
            'if sys.argv[1:3] == os.environ.get("GH_FAIL_COMMAND", "").split():\n'
            '    sys.exit(1)\n'
        )
        gh.chmod(0o755)
        self.env = dict(
            os.environ,
            PATH=str(self.root) + os.pathsep + os.environ["PATH"],
            GH_LOG=str(self.log),
            GITHUB_REPOSITORY="owner/rahmen",
            GITHUB_EVENT_NAME="push",
            GITHUB_REF="refs/tags/rahmen-v0.2.0",
            RELEASE_TAG="rahmen-v0.2.0",
            REQUESTED_RELEASE_TAG="",
            PACKAGE_VARIANT="trixie-armhf",
            GH_RELEASES=DRAFT,
        )

    def write_checksum(self):
        digest = hashlib.sha256(self.package.read_bytes()).hexdigest()
        self.checksum.write_text(f"{digest}  {self.package.name}\n")

    def run_helper(self, command="publish"):
        return subprocess.run(
            [sys.executable, str(SCRIPT), command, str(self.assets)],
            cwd=self.root, env=self.env, text=True, capture_output=True,
        )

    def calls(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []

    def test_existing_release_only_uploads_expected_assets(self):
        result = self.run_helper()
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.calls()
        self.assertEqual(len(calls), 2)
        self.assertEqual(calls[1][:3], ["release", "upload", "rahmen-v0.2.0"])
        self.assertEqual(set(calls[1][3:-1]), {str(self.package), str(self.checksum)})
        self.assertEqual(calls[1][-1], "--clobber")

    def test_alternate_variant_selects_its_own_checksum_file(self):
        original_checksum = self.checksum
        self.checksum = self.assets / "SHA256SUMS-bookworm-armhf"
        original_checksum.rename(self.checksum)
        original_checksum.write_text("unrelated checksum file")
        self.env["PACKAGE_VARIANT"] = "bookworm-armhf"
        result = self.run_helper()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(str(self.checksum), self.calls()[-1])
        self.assertNotIn(str(original_checksum), self.calls()[-1])

    def test_variant_requires_its_own_checksum_file(self):
        self.env["PACKAGE_VARIANT"] = "bookworm-armhf"
        result = self.run_helper()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.calls(), [])

    def test_invalid_variant_has_no_github_side_effects(self):
        for variant in ("", "../trixie-armhf", "trixie/armhf", "trixie-armhf\ninjected"):
            with self.subTest(variant=variant):
                self.env["PACKAGE_VARIANT"] = variant
                result = self.run_helper()
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.calls(), [])

    def test_release_lookup_includes_drafts_for_the_tag(self):
        result = self.run_helper()
        self.assertEqual(result.returncode, 0, result.stderr)
        lookup = self.calls()[0]
        self.assertEqual(lookup[:3], ["api", "--paginate", "repos/owner/rahmen/releases"])
        self.assertIn('select(.tag_name == "rahmen-v0.2.0")', lookup[-1])

    def test_new_release_is_a_draft_with_verified_tag_and_notes(self):
        self.env["GH_RELEASES"] = ""
        result = self.run_helper()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.calls()[1],
            ["release", "create", "rahmen-v0.2.0", "--verify-tag", "--generate-notes", "--draft"],
        )
        self.assertEqual(self.calls()[2][:2], ["release", "upload"])

    def test_published_immutable_release_is_not_uploaded_to(self):
        self.env["GH_RELEASES"] = PUBLISHED
        result = self.run_helper()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("immutable", result.stderr)
        self.assertEqual(len(self.calls()), 1)

    def test_published_mutable_release_still_accepts_assets(self):
        self.env["GH_RELEASES"] = '{"draft":false,"immutable":false}'
        result = self.run_helper()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls()[-1][:2], ["release", "upload"])

    def test_duplicate_releases_for_a_tag_fail(self):
        self.env["GH_RELEASES"] = DRAFT + "\n" + DRAFT
        result = self.run_helper()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(self.calls()), 1)

    def test_finalize_publishes_draft_release(self):
        result = self.run_helper("finalize")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls()[-1], ["release", "edit", "rahmen-v0.2.0", "--draft=false"])

    def test_finalize_leaves_published_release_alone(self):
        self.env["GH_RELEASES"] = PUBLISHED
        result = self.run_helper("finalize")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(self.calls()), 1)

    def test_finalize_requires_existing_release(self):
        self.env["GH_RELEASES"] = ""
        result = self.run_helper("finalize")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(self.calls()), 1)

    def test_finalize_applies_publication_guards(self):
        for changes in ({"GITHUB_EVENT_NAME": "pull_request"}, {"GITHUB_REF": "refs/heads/master"}):
            with self.subTest(changes=changes):
                original = self.env.copy()
                self.env.update(changes)
                result = self.run_helper("finalize")
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.calls(), [])
                self.env = original

    def test_prerelease_flag_and_manual_v_tag(self):
        (self.root / "Cargo.toml").write_text('[package]\nversion = "0.2.0-rc.1"\n')
        self.env.update(GH_RELEASES="", RELEASE_TAG="v0.2.0-rc.1", GITHUB_REF="refs/tags/v0.2.0-rc.1")
        result = self.run_helper()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("--prerelease", self.calls()[1])

    def test_release_plz_explicit_tag_on_master(self):
        self.env.update(GITHUB_REF="refs/heads/master", REQUESTED_RELEASE_TAG="rahmen-v0.2.0")
        result = self.run_helper()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_unrelated_extra_asset_is_not_uploaded(self):
        (self.assets / "unrelated.txt").write_text("other asset")
        result = self.run_helper()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn(str(self.assets / "unrelated.txt"), self.calls()[-1])

    def test_invalid_inputs_have_no_github_side_effects(self):
        cases = [
            {"GITHUB_EVENT_NAME": "pull_request"},
            {"GITHUB_EVENT_NAME": "pull_request_target", "REQUESTED_RELEASE_TAG": "rahmen-v0.2.0"},
            {"GITHUB_REF": "refs/heads/master"},
            {"RELEASE_TAG": "rahmen-exiv2-v0.2.0"},
            {"RELEASE_TAG": "rahmen-v0.3.0"},
            {"RELEASE_TAG": ""},
            {"REQUESTED_RELEASE_TAG": "rahmen-v0.3.0", "GITHUB_REF": "refs/heads/master"},
        ]
        for changes in cases:
            with self.subTest(changes=changes):
                original = self.env.copy()
                self.env.update(changes)
                result = self.run_helper()
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.calls(), [])
                self.env = original

    def test_invalid_assets_have_no_github_side_effects(self):
        for invalid in ("corrupt", "missing package", "missing checksum", "extra checksum entry"):
            with self.subTest(invalid=invalid):
                self.package.write_bytes(b"Debian package fixture")
                self.write_checksum()
                if invalid == "corrupt":
                    self.package.write_bytes(b"changed")
                elif invalid == "missing package":
                    self.package.unlink()
                elif invalid == "missing checksum":
                    self.checksum.unlink()
                else:
                    with self.checksum.open("a") as output:
                        output.write("0" * 64 + "  unexpected.deb\n")
                result = self.run_helper()
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.calls(), [])

    def test_retry_reuses_release_and_replaces_only_generated_names(self):
        for _ in range(2):
            result = self.run_helper()
            self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.calls()
        self.assertEqual(calls[:2], calls[2:])
        self.assertEqual(calls[-1][-1], "--clobber")

    def test_api_error_does_not_create_release(self):
        self.env["GH_API_EXIT"] = "1"
        result = self.run_helper()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(self.calls()), 1)

    def test_failed_tag_verification_does_not_upload(self):
        self.env.update(GH_RELEASES="", GH_FAIL_COMMAND="release create")
        result = self.run_helper()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(self.calls()), 2)
        self.assertIn("--verify-tag", self.calls()[-1])

    def test_failed_upload_fails_publication(self):
        self.env["GH_FAIL_COMMAND"] = "release upload"
        result = self.run_helper()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(self.calls()), 2)

    def test_validate_outputs_manifest_version(self):
        output = self.root / "outputs"
        self.env["GITHUB_OUTPUT"] = str(output)
        self.package.unlink()
        self.checksum.unlink()
        self.assets.rmdir()
        result = self.run_helper("validate")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(output.read_text(), "release_tag=rahmen-v0.2.0\nversion=0.2.0\n")


if __name__ == "__main__":
    unittest.main()
