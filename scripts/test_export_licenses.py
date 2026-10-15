"""Regression checks for the supported license-export policy, not a general SPDX solver."""
import unittest

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


if __name__ == "__main__":
    unittest.main()
