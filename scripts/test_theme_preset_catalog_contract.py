"""Direct contracts for the unqualified production theme preset catalog."""

import copy
import unittest

from scripts.theme_preset_catalog_contract import validate_unqualified_catalog


class ThemePresetCatalogContractTests(unittest.TestCase):
    def setUp(self):
        self.catalog = {
            "schema_version": 1,
            "presets": [{
                "id": "brutalist",
                "display_name": "Brutalist",
                "appearance": "light",
                "maturity": "alpha",
                "available": True,
                "availability_reason_ids": [],
                "qualified_cells": [],
                "license_expression": "MIT OR Apache-2.0",
                "required_attribution": None,
                "export_kind": "complete_spec",
            }],
        }

    def test_accepts_unqualified_alpha_catalog(self):
        validate_unqualified_catalog(self.catalog)

    def test_rejects_qualification_or_maturity_in_shared_catalog(self):
        for mutate in [
            lambda entry: entry.__setitem__("qualified_cells", [{"family_id": "state"}]),
            lambda entry: entry.__setitem__("maturity", "stable"),
        ]:
            changed = copy.deepcopy(self.catalog)
            mutate(changed["presets"][0])
            with self.subTest(changed=changed), self.assertRaisesRegex(RuntimeError, "Unqualified"):
                validate_unqualified_catalog(changed)

    def test_rejects_inconsistent_availability_and_duplicate_ids(self):
        changed = copy.deepcopy(self.catalog)
        changed["presets"][0]["availability_reason_ids"] = ["missing-font"]
        with self.assertRaisesRegex(RuntimeError, "availability"):
            validate_unqualified_catalog(changed)

        duplicate = copy.deepcopy(self.catalog)
        duplicate["presets"].append(copy.deepcopy(duplicate["presets"][0]))
        with self.assertRaisesRegex(RuntimeError, "Duplicate"):
            validate_unqualified_catalog(duplicate)


if __name__ == "__main__":
    unittest.main()
