import json
import unittest

import merman


def valid_catalog():
    return {
        "schema_version": 1,
        "transport_api_version": 3,
        "package_version": "test",
        "options_schema_versions": [2],
        "payload_schemas": [
            {"id": "binding-result", "version": 1},
            {"id": "operation-metadata", "version": 1},
        ],
        "metadata_ids": [
            "ascii-capabilities",
            "diagram-family-capabilities",
            "lint-rule-catalog",
            "supported-diagrams",
            "supported-host-theme-presets",
            "supported-themes",
        ],
        "option_group_ids": ["ascii", "environment", "host_theme", "lint", "svg"],
        "constructor_service_ids": ["host-text-measurement"],
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
                    "id": "max_source_bytes",
                    "phase": "source",
                    "description": "Maximum source size.",
                    "overridable": True,
                    "hard_cap": False,
                    "minimum_value": 1,
                    "operation_ids": [
                        "analysis-json",
                        "ascii",
                        "semantic-json",
                        "svg",
                    ],
                }
            ],
            "profiles": [
                {
                    "id": "interactive",
                    "purpose": "Interactive rendering.",
                    "trust_assumption": "Untrusted input.",
                    "recommended_binding_default": True,
                    "limits": {"max_source_bytes": 1048576},
                },
                {
                    "id": "trusted-native",
                    "purpose": "Trusted native rendering.",
                    "trust_assumption": "Trusted input.",
                    "recommended_binding_default": False,
                    "limits": {"max_source_bytes": None},
                },
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
        self.assertEqual(catalog["options_schema_versions"], [2])
        self.assertEqual(
            catalog["resources"]["limits"][0]["operation_ids"],
            ["analysis-json", "ascii", "semantic-json", "svg"],
        )

    def test_accepts_catalog_without_svg_or_text_measurement(self):
        catalog = valid_catalog()
        catalog["capabilities"]["capability_ids"].remove("svg")
        catalog["capabilities"]["output_ids"].remove("svg")
        catalog["capabilities"]["operation_ids"].remove("svg")
        catalog["capabilities"]["text_measurement"] = None
        catalog["constructor_service_ids"] = []
        catalog["output_contracts"] = [
            contract for contract in catalog["output_contracts"] if contract["id"] != "svg"
        ]
        catalog["resources"]["limits"][0]["operation_ids"].remove("svg")

        parsed = merman.get_runtime_catalog(FakeEngine(catalog))

        self.assertNotIn("svg", parsed["capabilities"]["capability_ids"])
        self.assertIsNone(parsed["capabilities"]["text_measurement"])

    def test_accepts_legacy_catalog_without_additive_discovery_sections(self):
        catalog = valid_catalog()
        del catalog["option_group_ids"]
        del catalog["constructor_service_ids"]

        parsed = merman.get_runtime_catalog(FakeEngine(catalog))

        self.assertEqual(parsed["option_group_ids"], [])
        self.assertEqual(parsed["constructor_service_ids"], [])

    def test_accepts_output_backed_by_an_internal_svg_pipeline(self):
        catalog = valid_catalog()
        catalog["capabilities"] = {
            "capability_ids": ["png"],
            "output_ids": ["png"],
            "operation_ids": ["png", "semantic-json"],
            "system_adapter_ids": [],
            "text_measurement": {
                "protocol_version": merman.TEXT_MEASUREMENT_PROTOCOL_VERSION,
                "provider_ids": ["vendored"],
            },
        }
        catalog["constructor_service_ids"] = []
        catalog["output_contracts"] = [
            {
                "id": "png",
                "media_type": "image/png",
                "system_fonts": None,
                "embedded_images": None,
            }
        ]
        catalog["resources"]["limits"][0]["operation_ids"] = [
            "png",
            "semantic-json",
        ]

        parsed = merman.get_runtime_catalog(FakeEngine(catalog))

        self.assertEqual(parsed["capabilities"]["capability_ids"], ["png"])
        self.assertEqual(
            parsed["capabilities"]["text_measurement"]["provider_ids"],
            ["vendored"],
        )

    def test_rejects_constructor_service_without_its_provider(self):
        catalog = valid_catalog()
        catalog["capabilities"]["text_measurement"]["provider_ids"] = ["vendored"]

        with self.assertRaisesRegex(
            merman.MermanRuntimeCatalogError,
            "requires the host-callback provider",
        ):
            merman.get_runtime_catalog(FakeEngine(catalog))

    def test_rejects_catalog_without_current_options_schema(self):
        catalog = valid_catalog()
        catalog["options_schema_versions"] = [1]

        with self.assertRaisesRegex(
            merman.MermanRuntimeCatalogError,
            "current Options JSON schema",
        ):
            merman.get_runtime_catalog(FakeEngine(catalog))

    def test_rejects_missing_or_wrong_version_known_payload_schema(self):
        missing = valid_catalog()
        missing["payload_schemas"] = [{"id": "binding-result", "version": 1}]

        wrong_version = valid_catalog()
        wrong_version["payload_schemas"][1]["version"] = 2

        for catalog in [missing, wrong_version]:
            with self.subTest(catalog=catalog):
                with self.assertRaisesRegex(
                    merman.MermanRuntimeCatalogError,
                    "runtime payload schema",
                ):
                    merman.get_runtime_catalog(FakeEngine(catalog))

    def test_accepts_unknown_future_ids_and_additive_fields(self):
        catalog = valid_catalog()
        catalog["payload_schemas"].insert(
            1, {"id": "future-payload", "version": 9}
        )
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
        self.assertIs(exported["ResourceOverrideId"], merman.ResourceOverrideId)
        options = (
            exported["ResourceOptionsBuilder"]()
            .profile(exported["ResourceProfile"].CONSTRAINED)
            .limit(exported["ResourceOverrideId"].MAX_SOURCE_BYTES, 4096)
            .build()
        )
        self.assertEqual(
            json.loads(options.to_options_json()),
            {
                "resources": {
                    "limits": {"max_source_bytes": 4096},
                    "profile": "constrained",
                },
                "version": 2,
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

        missing_options_schemas = valid_catalog()
        del missing_options_schemas["options_schema_versions"]
        cases.append(missing_options_schemas)

        missing_payload_schemas = valid_catalog()
        del missing_payload_schemas["payload_schemas"]
        cases.append(missing_payload_schemas)

        missing_metadata_ids = valid_catalog()
        del missing_metadata_ids["metadata_ids"]
        cases.append(missing_metadata_ids)

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

        missing_limit_operations = valid_catalog()
        del missing_limit_operations["resources"]["limits"][0]["operation_ids"]

        hard_cap_is_overridable = valid_catalog()
        hard_cap_is_overridable["resources"]["limits"][0]["hard_cap"] = True

        hard_cap_is_unbounded = valid_catalog()
        hard_cap_is_unbounded["resources"]["limits"][0]["hard_cap"] = True
        hard_cap_is_unbounded["resources"]["limits"][0]["overridable"] = False
        hard_cap_is_unbounded["resources"]["profiles"][1]["limits"][
            "max_source_bytes"
        ] = None

        unknown_default_profile = valid_catalog()
        unknown_default_profile["resources"]["cli_default_profile"] = "missing"

        nonrecommended_binding_default = valid_catalog()
        nonrecommended_binding_default["resources"][
            "general_binding_default_profile"
        ] = "trusted-native"

        multiple_recommended_profiles = valid_catalog()
        multiple_recommended_profiles["resources"]["profiles"][1][
            "recommended_binding_default"
        ] = True

        duplicate_profile = valid_catalog()
        duplicate_profile["resources"]["profiles"].append(
            duplicate_profile["resources"]["profiles"][0].copy()
        )

        for catalog in [
            negative_registry,
            invalid_resource_limit,
            invalid_profile_limit,
            missing_limit_operations,
            hard_cap_is_overridable,
            hard_cap_is_unbounded,
            unknown_default_profile,
            nonrecommended_binding_default,
            multiple_recommended_profiles,
            duplicate_profile,
        ]:
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
