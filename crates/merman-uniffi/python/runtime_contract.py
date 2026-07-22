"""Typed, fail-closed access to the Merman runtime contract."""

import json
from typing import Any, Dict, List, Optional, Protocol, TypedDict, cast


RUNTIME_CONTRACT_SCHEMA_VERSION = 4
OPTIONS_SCHEMA_VERSION = 1
SYSTEM_ADAPTER_IDS = frozenset(
    {
        "system-clock",
        "system-timezone",
        "system-random",
        "system-timing",
    }
)


class MermanRuntimeContractError(ValueError):
    """Raised when the loaded native library returns an incompatible contract."""


class MermanTextMeasurementCapabilities(TypedDict):
    vendored: bool
    deterministic: bool
    host_callback: bool
    font_assets: bool


class MermanRuntimeFeatures(TypedDict):
    render: bool
    analysis: bool
    ascii: bool
    system_adapter_ids: List[str]
    cytoscape_layout: bool
    elk_layout: bool
    ratex_math: bool
    editor_language: bool
    text_measurement: MermanTextMeasurementCapabilities


class MermanRuntimeRegistry(TypedDict):
    diagram_family_count: int


class MermanRuntimeResources(TypedDict):
    schema_version: int
    general_binding_default_profile: str
    cli_default_profile: str
    limits: List[Dict[str, Any]]
    profiles: List[Dict[str, Any]]


class MermanRuntimeContract(TypedDict):
    schema_version: int
    abi_version: int
    package_version: str
    options_schema_version: int
    payload_schemas: Dict[str, int]
    features: MermanRuntimeFeatures
    registry: MermanRuntimeRegistry
    resources: Optional[MermanRuntimeResources]


class _RuntimeContractEngine(Protocol):
    def runtime_contract_json(self) -> str:
        ...

    def abi_version(self) -> int:
        ...

    def package_version(self) -> str:
        ...


def get_runtime_contract(engine: _RuntimeContractEngine) -> MermanRuntimeContract:
    """Return the loaded engine contract after strict schema and identity checks."""

    try:
        decoded = json.loads(engine.runtime_contract_json())
    except (TypeError, json.JSONDecodeError) as error:
        raise MermanRuntimeContractError(
            f"Merman runtime contract is not valid JSON: {error}"
        ) from error

    contract = _expect_object(decoded, "runtime contract")
    _require_keys(
        contract,
        {
            "schema_version",
            "abi_version",
            "package_version",
            "options_schema_version",
            "payload_schemas",
            "features",
            "registry",
            "resources",
        },
        "runtime contract",
    )
    _expect_integer(contract["schema_version"], "runtime contract schema_version")
    if contract["schema_version"] != RUNTIME_CONTRACT_SCHEMA_VERSION:
        raise MermanRuntimeContractError(
            "unsupported Merman runtime contract schema: "
            f"expected {RUNTIME_CONTRACT_SCHEMA_VERSION}, "
            f"got {contract['schema_version']!r}"
        )
    _expect_integer(contract["abi_version"], "runtime contract abi_version")
    if contract["abi_version"] != engine.abi_version():
        raise MermanRuntimeContractError("Merman runtime contract ABI does not match the engine")
    if not isinstance(contract["package_version"], str):
        raise MermanRuntimeContractError("runtime contract package_version must be a string")
    if contract["package_version"] != engine.package_version():
        raise MermanRuntimeContractError(
            "Merman runtime contract package version does not match the engine"
        )
    _expect_integer(
        contract["options_schema_version"], "runtime contract options_schema_version"
    )
    if contract["options_schema_version"] != OPTIONS_SCHEMA_VERSION:
        raise MermanRuntimeContractError(
            "unsupported Merman options schema: "
            f"expected {OPTIONS_SCHEMA_VERSION}, "
            f"got {contract['options_schema_version']!r}"
        )

    payload_schemas = _expect_object(contract["payload_schemas"], "payload_schemas")
    if not payload_schemas or not all(
        isinstance(name, str) and _is_integer(version)
        for name, version in payload_schemas.items()
    ):
        raise MermanRuntimeContractError(
            "runtime contract payload_schemas must be a non-empty string-to-integer object"
        )

    _validate_features(contract["features"])
    _validate_registry(contract["registry"])
    _validate_resources(contract["resources"])
    return cast(MermanRuntimeContract, contract)


def _validate_features(value: Any) -> None:
    features = _expect_object(value, "features")
    boolean_fields = {
        "render",
        "analysis",
        "ascii",
        "cytoscape_layout",
        "elk_layout",
        "ratex_math",
        "editor_language",
    }
    _require_keys(
        features,
        boolean_fields | {"system_adapter_ids", "text_measurement"},
        "features",
    )
    if "core_host" in features:
        raise MermanRuntimeContractError("runtime contract contains removed core_host field")
    for field in boolean_fields:
        if not isinstance(features[field], bool):
            raise MermanRuntimeContractError(f"features.{field} must be a boolean")

    adapter_ids = features["system_adapter_ids"]
    if not isinstance(adapter_ids, list) or not all(
        isinstance(adapter_id, str) for adapter_id in adapter_ids
    ):
        raise MermanRuntimeContractError("features.system_adapter_ids must be a string array")
    if len(adapter_ids) != len(set(adapter_ids)):
        raise MermanRuntimeContractError("features.system_adapter_ids must be unique")
    unknown = set(adapter_ids) - SYSTEM_ADAPTER_IDS
    if unknown:
        raise MermanRuntimeContractError(
            "features.system_adapter_ids contains unknown IDs: " + ", ".join(sorted(unknown))
        )

    text_measurement = _expect_object(features["text_measurement"], "text_measurement")
    text_measurement_fields = {
        "vendored",
        "deterministic",
        "host_callback",
        "font_assets",
    }
    _require_keys(text_measurement, text_measurement_fields, "text_measurement")
    for field in text_measurement_fields:
        if not isinstance(text_measurement[field], bool):
            raise MermanRuntimeContractError(f"text_measurement.{field} must be a boolean")


def _validate_registry(value: Any) -> None:
    registry = _expect_object(value, "registry")
    _require_keys(registry, {"diagram_family_count"}, "registry")
    count = registry["diagram_family_count"]
    if not _is_integer(count) or count < 0:
        raise MermanRuntimeContractError(
            "registry.diagram_family_count must be a non-negative integer"
        )


def _validate_resources(value: Any) -> None:
    if value is None:
        return
    resources = _expect_object(value, "resources")
    _require_keys(
        resources,
        {
            "schema_version",
            "general_binding_default_profile",
            "cli_default_profile",
            "limits",
            "profiles",
        },
        "resources",
    )
    _expect_integer(resources["schema_version"], "resources.schema_version")
    for field in ["general_binding_default_profile", "cli_default_profile"]:
        if not isinstance(resources[field], str):
            raise MermanRuntimeContractError(f"resources.{field} must be a string")
    for field in ["limits", "profiles"]:
        if not isinstance(resources[field], list) or not all(
            isinstance(item, dict) for item in resources[field]
        ):
            raise MermanRuntimeContractError(f"resources.{field} must be an object array")


def _expect_object(value: Any, label: str) -> Dict[str, Any]:
    if not isinstance(value, dict) or not all(isinstance(key, str) for key in value):
        raise MermanRuntimeContractError(f"{label} must be a JSON object")
    return value


def _require_keys(value: Dict[str, Any], keys: set, label: str) -> None:
    missing = keys - value.keys()
    if missing:
        raise MermanRuntimeContractError(
            f"{label} is missing required fields: {', '.join(sorted(missing))}"
        )


def _is_integer(value: Any) -> bool:
    return type(value) is int


def _expect_integer(value: Any, label: str) -> None:
    if not _is_integer(value):
        raise MermanRuntimeContractError(f"{label} must be an integer")
