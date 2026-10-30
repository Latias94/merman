"""Direct contracts for the unqualified production theme preset catalog."""

import copy
import unittest

from scripts.theme_preset_catalog_contract import validate_unqualified_catalog
from scripts.theme_preset_catalog_test_fixture import canonical_theme_preset_catalog


class ThemePresetCatalogContractTests(unittest.TestCase):
    def setUp(self):
        self.catalog = canonical_theme_preset_catalog()

    def test_accepts_unqualified_alpha_catalog(self):
        validate_unqualified_catalog(self.catalog)

    def test_rejects_missing_application_guidance_fields(self):
        for field in ("preview_colors", "application_notes"):
            changed = copy.deepcopy(self.catalog)
            del changed["presets"][0][field]
            with self.subTest(field=field), self.assertRaisesRegex(RuntimeError, "entry fields"):
                validate_unqualified_catalog(changed)

    def test_rejects_malformed_preview_colors(self):
        for colors in (None, {}, "#123456", [""], [1]):
            changed = copy.deepcopy(self.catalog)
            changed["presets"][0]["preview_colors"] = colors
            with self.subTest(colors=colors), self.assertRaisesRegex(RuntimeError, "preview colors"):
                validate_unqualified_catalog(changed)

    def test_family_designs_preserve_open_ids_without_granting_qualification(self):
        self.catalog["presets"][0]["family_designs"] = [
            {"family_id": "future-family", "treatment": "future-treatment"},
        ]
        validate_unqualified_catalog(self.catalog)

    def test_rejects_malformed_or_duplicate_family_designs(self):
        for designs in [
            None, {}, [{"family_id": "flowchart"}],
            [{"family_id": "flowchart", "treatment": 1}],
            [{"family_id": "", "treatment": "base_only"}],
            [{"family_id": "flowchart", "treatment": "base_only"}] * 2,
        ]:
            changed = copy.deepcopy(self.catalog)
            changed["presets"][0]["family_designs"] = designs
            with self.subTest(designs=designs), self.assertRaisesRegex(RuntimeError, "family"):
                validate_unqualified_catalog(changed)

    def test_application_guidance_preserves_open_ids_and_configuration(self):
        note = {
            "id": "future-guidance", "family_id": "future-family", "message": "Review settings.",
            "recommended_config": {"future": {"look": "future-look"}},
        }
        self.catalog["presets"][0]["application_notes"] = [note]
        validate_unqualified_catalog(self.catalog)
        self.assertEqual(note["recommended_config"], {"future": {"look": "future-look"}})
        for notes in [None, [dict(note, recommended_config=[])], [dict(note, message="")], [note, note]]:
            changed = copy.deepcopy(self.catalog)
            changed["presets"][0]["application_notes"] = notes
            with self.subTest(notes=notes), self.assertRaisesRegex(RuntimeError, "application note"):
                validate_unqualified_catalog(changed)

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
