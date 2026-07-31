import json
import unittest

import merman


def valid_catalog():
    return {
        "schema_version": 1,
        "transport_api_version": 3,
        "package_version": "test",
        "capabilities": {
            "capability_ids": ["analysis", "ascii", "svg", "system-clock"],
            "output_ids": ["ascii", "svg"],
            "operation_ids": ["analysis-json", "ascii", "semantic-json", "svg"],
            "system_adapter_ids": ["system-clock"],
            "text_measurement": {
                "protocol_version": merman.TEXT_MEASUREMENT_PROTOCOL_VERSION,
                "provider_ids": ["host-callback", "vendored"],
            },
        },
        "output_contracts": [
            {
                "id": "ascii",
                "media_type": "text/plain; charset=utf-8",
                "system_fonts": None,
                "embedded_images": None,
            },
            {
                "id": "svg",
                "media_type": "image/svg+xml",
                "system_fonts": None,
                "embedded_images": None,
            },
        ],
        "registry": {"diagram_family_count": 35},
        "resources": {
            "general_binding_default_profile": "interactive",
            "cli_default_profile": "trusted-native",
            "limits": [
                {
                    "id": "max-source-bytes",
                    "phase": "source",
                    "description": "Maximum source size.",
                    "overridable": True,
                    "hard_cap": False,
                }
            ],
            "profiles": [
                {
                    "id": "interactive",
                    "purpose": "Interactive rendering.",
                    "trust_assumption": "Untrusted input.",
                    "recommended_binding_default": True,
                    "limits": {"max_source_bytes": 1048576},
                }
            ],
        },
    }


class FakeEngine:
    def __init__(self, catalog):
        self.catalog = catalog
        self.catalog_calls = 0

    def runtime_catalog_json(self):
        self.catalog_calls += 1
        return json.dumps(self.catalog)

    def binding_api_version(self):
        return 3

    def package_version(self):
        return "test"


class RuntimeCatalogTest(unittest.TestCase):
    def test_accepts_current_catalog_with_one_atomic_read(self):
        catalog = valid_catalog()
        engine = FakeEngine(catalog)

        self.assertEqual(merman.get_runtime_catalog(engine), catalog)
        self.assertEqual(engine.catalog_calls, 1)
        self.assertFalse(hasattr(merman, "get_runtime_contract"))
        self.assertFalse(hasattr(merman, "get_runtime_capability_vocabulary"))
        self.assertFalse(hasattr(merman, "MermanRuntimeContractError"))

    def test_accepts_unknown_future_ids_and_additive_fields(self):
        catalog = valid_catalog()
        catalog["future_root_metadata"] = {}
        catalog["registry"]["future_registry_metadata"] = True
        catalog["resources"]["future_resource_metadata"] = True
        capabilities = catalog["capabilities"]
        capabilities["future_capability_metadata"] = {}
        capabilities["capability_ids"] = [
            "analysis",
            "ascii",
            "future-capability",
            "svg",
            "system-clock",
        ]
        capabilities["output_ids"] = ["ascii", "future-output", "svg"]
        catalog["output_contracts"].insert(
            1,
            {
                "id": "future-output",
                "media_type": "application/octet-stream",
                "system_fonts": None,
                "embedded_images": None,
                "future_output_metadata": True,
            },
        )
        capabilities["operation_ids"] = [
            "analysis-json",
            "ascii",
            "future-operation",
            "future-output",
            "semantic-json",
            "svg",
        ]
        capabilities["text_measurement"]["provider_ids"] = [
            "future-provider",
            "host-callback",
            "vendored",
        ]
        capabilities["text_measurement"]["future_measurement_metadata"] = True
        catalog["resources"]["limits"][0]["future_limit_metadata"] = True
        catalog["resources"]["profiles"][0]["future_profile_metadata"] = True

        self.assertEqual(merman.get_runtime_catalog(FakeEngine(catalog)), catalog)

    def test_public_star_export_has_no_removed_runtime_names(self):
        exported = {}
        exec("from merman import *", exported)
        self.assertIn("get_runtime_catalog", exported)
        self.assertNotIn("get_runtime_contract", exported)
        self.assertNotIn("get_runtime_capability_vocabulary", exported)

    def test_public_star_export_includes_resource_options_api(self):
        exported = {}
        exec("from merman import *", exported)

        self.assertIs(exported["ResourceProfile"], merman.ResourceProfile)
        self.assertIs(exported["ResourceLimitId"], merman.ResourceLimitId)
        options = (
            exported["ResourceOptionsBuilder"]()
            .profile(exported["ResourceProfile"].CONSTRAINED)
            .limit(exported["ResourceLimitId"].MAX_SOURCE_BYTES, 4096)
            .build()
        )
        self.assertEqual(
            json.loads(options.to_options_json()),
            {
                "resources": {
                    "limits": {"max_source_bytes": 4096},
                    "profile": "constrained",
                },
                "version": 1,
            },
        )

    def test_rejects_missing_or_wrong_catalog_identity(self):
        cases = []
        missing = valid_catalog()
        del missing["resources"]
        cases.append(missing)

        missing_output_contracts = valid_catalog()
        del missing_output_contracts["output_contracts"]
        cases.append(missing_output_contracts)

        wrong_schema = valid_catalog()
        wrong_schema["schema_version"] = 2
        cases.append(wrong_schema)

        wrong_transport = valid_catalog()
        wrong_transport["transport_api_version"] = 2
        cases.append(wrong_transport)

        wrong_package = valid_catalog()
        wrong_package["package_version"] = "other"
        cases.append(wrong_package)

        for catalog in cases:
            with self.subTest(catalog=catalog):
                with self.assertRaises(merman.MermanRuntimeCatalogError):
                    merman.get_runtime_catalog(FakeEngine(catalog))

    def test_rejects_duplicate_unsorted_or_invalid_ids(self):
        duplicate = valid_catalog()
        duplicate["capabilities"]["operation_ids"] = ["svg", "svg"]

        unsorted = valid_catalog()
        unsorted["capabilities"]["capability_ids"] = ["svg", "analysis"]

        invalid = valid_catalog()
        invalid["capabilities"]["capability_ids"] = ["analysis", "Bad ID"]

        for catalog in [duplicate, unsorted, invalid]:
            with self.subTest(catalog=catalog):
                with self.assertRaises(merman.MermanRuntimeCatalogError):
                    merman.get_runtime_catalog(FakeEngine(catalog))

    def test_rejects_invalid_local_capability_relations(self):
        output_without_operation = valid_catalog()
        output_without_operation["capabilities"]["operation_ids"].remove("svg")

        adapter_without_capability = valid_catalog()
        adapter_without_capability["capabilities"]["capability_ids"].remove(
            "system-clock"
        )

        for catalog in [output_without_operation, adapter_without_capability]:
            with self.subTest(catalog=catalog):
                with self.assertRaises(merman.MermanRuntimeCatalogError):
                    merman.get_runtime_catalog(FakeEngine(catalog))

    def test_rejects_invalid_output_contracts(self):
        mismatched_ids = valid_catalog()
        mismatched_ids["output_contracts"].pop()

        invalid_fonts = valid_catalog()
        invalid_fonts["output_contracts"][1]["system_fonts"] = {
            "source_id": "host-system",
            "discovery": "first-use",
            "cache_scope": "process-global",
            "host_dependent": "true",
            "caller_configurable": False,
            "resource_bounded": False,
        }

        invalid_images = valid_catalog()
        invalid_images["output_contracts"][1]["embedded_images"] = {
            "source_ids": ["data-url"],
            "filesystem_access": False,
            "network_access": False,
            "caller_configurable": False,
            "limits": {
                "max_bytes_per_image": 0,
                "max_total_bytes": None,
                "max_pixels_per_image": None,
                "max_total_pixels": None,
            },
        }

        for catalog in [mismatched_ids, invalid_fonts, invalid_images]:
            with self.subTest(catalog=catalog):
                with self.assertRaises(merman.MermanRuntimeCatalogError):
                    merman.get_runtime_catalog(FakeEngine(catalog))

    def test_rejects_invalid_text_measurement_boundary(self):
        missing = valid_catalog()
        missing["capabilities"]["text_measurement"] = None

        without_svg = valid_catalog()
        without_svg["capabilities"]["capability_ids"].remove("svg")
        without_svg["capabilities"]["output_ids"].remove("svg")
        without_svg["capabilities"]["operation_ids"].remove("svg")
        without_svg["output_contracts"].pop()

        wrong_protocol = valid_catalog()
        wrong_protocol["capabilities"]["text_measurement"]["protocol_version"] += 1

        missing_vendored = valid_catalog()
        missing_vendored["capabilities"]["text_measurement"]["provider_ids"] = [
            "host-callback"
        ]

        for catalog in [missing, without_svg, wrong_protocol, missing_vendored]:
            with self.subTest(catalog=catalog):
                with self.assertRaises(merman.MermanRuntimeCatalogError):
                    merman.get_runtime_catalog(FakeEngine(catalog))

    def test_rejects_invalid_registry_and_resource_shapes(self):
        negative_registry = valid_catalog()
        negative_registry["registry"]["diagram_family_count"] = -1

        invalid_resource_limit = valid_catalog()
        invalid_resource_limit["resources"]["limits"][0]["hard_cap"] = "false"

        invalid_profile_limit = valid_catalog()
        invalid_profile_limit["resources"]["profiles"][0]["limits"][
            "max_source_bytes"
        ] = -1

        for catalog in [negative_registry, invalid_resource_limit, invalid_profile_limit]:
            with self.subTest(catalog=catalog):
                with self.assertRaises(merman.MermanRuntimeCatalogError):
                    merman.get_runtime_catalog(FakeEngine(catalog))

    def test_rejects_invalid_json(self):
        engine = FakeEngine(valid_catalog())
        engine.runtime_catalog_json = lambda: "{"
        with self.assertRaises(merman.MermanRuntimeCatalogError):
            merman.get_runtime_catalog(engine)


if __name__ == "__main__":
    unittest.main()
