"""Regression checks for the supported license-export policy, not a general SPDX solver."""
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

from export_licenses import validate_license_expression, validate_license_inventory


class LicensePolicyTests(unittest.TestCase):
    def test_unknown_additional_obligation_requires_manual_review(self):
        with self.assertRaises(RuntimeError):
            validate_license_expression("MIT AND LicenseRef-Proprietary")

    def test_stale_license_directories_are_not_silently_distributed(self):
        with self.assertRaises(RuntimeError):
            validate_license_inventory({"windows-0.58.0", "removed-0.1.0"}, {"windows-0.58.0"})
        validate_license_inventory(set(), {"windows-0.58.0"})
        validate_license_inventory({"windows-0.58.0"}, {"windows-0.58.0"})

    def test_current_dependency_expressions_are_supported(self):
        for expression in ("MIT OR Apache-2.0", "(MIT OR Apache-2.0) AND Unicode-3.0"):
            validate_license_expression(expression)


class LicenseCheckoutTests(unittest.TestCase):
    def test_export_bytes_survive_windows_checkout_and_reexport(self):
        """Exercise real Git conversion, not a mocked license consistency check."""
        root = Path(__file__).resolve().parents[1]
        # Explicitly artificial fixtures; no dependency license is fabricated.
        payloads = {
            "THIRD_PARTY_LICENSES/fixture-1.0/LICENSE-MIT": b"License regression fixture\nSecond line\n",
            "THIRD_PARTY_LICENSES/fixture-1.0/LICENSE-UNICODE": b"Additional license fixture\n",
            "THIRD_PARTY_NOTICES.md": b"# Notice regression fixture\n",
        }
        with tempfile.TemporaryDirectory(prefix="audiobridge-license-test-") as directory:
            origin = Path(directory) / "origin"
            checkout = Path(directory) / "checkout"
            origin.mkdir()

            def git(directory, *args):
                return subprocess.run(
                    ["git", "-C", str(directory), *args], check=True,
                    capture_output=True, text=True, encoding="utf-8",
                ).stdout.strip()

            git(origin, "init", "--quiet")
            for key, value in (
                ("core.autocrlf", "true"),
                ("user.name", "AudioBridge regression tests"),
                ("user.email", "tests@example.invalid"),
                ("commit.gpgsign", "false"),
                ("core.hooksPath", str(Path(directory) / "no-hooks")),
            ):
                git(origin, "config", "--local", key, value)
            attributes = root / ".gitattributes"
            if attributes.is_file():
                shutil.copyfile(attributes, origin / ".gitattributes")
            for relative, payload in payloads.items():
                path = origin / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(payload)
            git(origin, "add", ".")
            git(origin, "commit", "--quiet", "-m", "Create EOL regression fixtures")
            git(origin, "clone", "--quiet", "-c", "core.autocrlf=true", str(origin), str(checkout))
            for relative, payload in payloads.items():
                self.assertEqual((checkout / relative).read_bytes(), payload, relative)
                # Mirrors the exporter overwriting a checked-out license with original bytes.
                (checkout / relative).write_bytes(payload)
            self.assertEqual(
                git(checkout, "status", "--porcelain", "--", "THIRD_PARTY_LICENSES", "THIRD_PARTY_NOTICES.md"),
                "",
            )
            # The consistency guard must still catch an actual content change.
            relative = next(iter(payloads))
            (checkout / relative).write_bytes(payloads[relative] + b"Changed fixture content\n")
            self.assertNotEqual(
                git(checkout, "status", "--porcelain", "--", relative), "",
            )


if __name__ == "__main__":
    unittest.main()
