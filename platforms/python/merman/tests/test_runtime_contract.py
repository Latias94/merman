import copy
import json
import unittest

import merman


def valid_contract():
    return {
        "schema_version": 4,
        "abi_version": 2,
        "package_version": "test",
        "options_schema_version": 1,
        "payload_schemas": {"binding_result": 1},
        "features": {
            "render": False,
            "analysis": False,
            "ascii": False,
            "system_adapter_ids": [],
            "cytoscape_layout": False,
            "elk_layout": False,
            "ratex_math": False,
            "editor_language": False,
            "text_measurement": {
                "vendored": False,
                "deterministic": False,
                "host_callback": False,
                "font_assets": False,
            },
        },
        "registry": {"diagram_family_count": 35},
        "resources": None,
    }


class FakeEngine:
    def __init__(self, contract):
        self.contract = contract

    def runtime_contract_json(self):
        return json.dumps(self.contract)

    def abi_version(self):
        return 2

    def package_version(self):
        return "test"


class RuntimeContractTest(unittest.TestCase):
    def test_accepts_current_contract(self):
        contract = valid_contract()
        self.assertEqual(merman.get_runtime_contract(FakeEngine(contract)), contract)

    def test_rejects_old_schema_and_removed_host_projection(self):
        old_schema = valid_contract()
        old_schema["schema_version"] = 3
        with self.assertRaises(merman.MermanRuntimeContractError):
            merman.get_runtime_contract(FakeEngine(old_schema))

        removed_projection = valid_contract()
        removed_projection["features"]["core_host"] = True
        with self.assertRaises(merman.MermanRuntimeContractError):
            merman.get_runtime_contract(FakeEngine(removed_projection))

    def test_rejects_unknown_and_duplicate_system_adapters(self):
        unknown = valid_contract()
        unknown["features"]["system_adapter_ids"] = ["browser-time"]
        with self.assertRaises(merman.MermanRuntimeContractError):
            merman.get_runtime_contract(FakeEngine(unknown))

        duplicate = valid_contract()
        duplicate["features"]["system_adapter_ids"] = [
            "system-clock",
            "system-clock",
        ]
        with self.assertRaises(merman.MermanRuntimeContractError):
            merman.get_runtime_contract(FakeEngine(duplicate))

    def test_rejects_missing_fields_and_engine_identity_mismatch(self):
        missing = valid_contract()
        del missing["registry"]
        with self.assertRaises(merman.MermanRuntimeContractError):
            merman.get_runtime_contract(FakeEngine(missing))

        wrong_identity = copy.deepcopy(valid_contract())
        wrong_identity["package_version"] = "other"
        with self.assertRaises(merman.MermanRuntimeContractError):
            merman.get_runtime_contract(FakeEngine(wrong_identity))


if __name__ == "__main__":
    unittest.main()
