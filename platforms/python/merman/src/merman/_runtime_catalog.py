"""Typed, fail-closed access to the artifact-owned Merman runtime catalog."""

import json
import re
from typing import Any, Dict, List, Optional, Protocol, TypedDict, cast

try:
    from ._text_measurement_protocol import TEXT_MEASUREMENT_PROTOCOL_VERSION
except ModuleNotFoundError as exc:
    if exc.name != f"{__package__}._text_measurement_protocol":
        raise
    TEXT_MEASUREMENT_PROTOCOL_VERSION = None


RUNTIME_CATALOG_SCHEMA_VERSION = 1
_IDENTIFIER = re.compile(r"^[a-z0-9][a-z0-9-]*$")


class MermanRuntimeCatalogError(ValueError):
    """Raised when the loaded native library returns an incompatible catalog."""


class MermanTextMeasurementCapabilities(TypedDict):
    protocol_version: int
    provider_ids: List[str]


class MermanRuntimeCapabilities(TypedDict):
    capability_ids: List[str]
    output_ids: List[str]
    operation_ids: List[str]
    system_adapter_ids: List[str]
    text_measurement: Optional[MermanTextMeasurementCapabilities]


class MermanRuntimeRegistry(TypedDict):
    diagram_family_count: int


class MermanRuntimeResources(TypedDict):
    general_binding_default_profile: str
    cli_default_profile: str
    limits: List[Dict[str, Any]]
    profiles: List[Dict[str, Any]]


class MermanRuntimeCatalog(TypedDict):
    schema_version: int
    transport_api_version: int
    package_version: str
    capabilities: MermanRuntimeCapabilities
    registry: MermanRuntimeRegistry
    resources: MermanRuntimeResources


class _RuntimeCatalogEngine(Protocol):
    def runtime_catalog_json(self) -> str:
        ...

    def binding_api_version(self) -> int:
        ...

    def package_version(self) -> str:
        ...


def get_runtime_catalog(engine: _RuntimeCatalogEngine) -> MermanRuntimeCatalog:
    """Read and validate one artifact's current runtime fact set."""

    try:
        decoded = json.loads(engine.runtime_catalog_json())
    except (AttributeError, TypeError, json.JSONDecodeError) as error:
        raise MermanRuntimeCatalogError(
            f"Merman runtime catalog is not valid JSON: {error}"
        ) from error

    catalog = _expect_object(decoded, "runtime catalog")
    _require_required_keys(
        catalog,
        {
            "schema_version",
            "transport_api_version",
            "package_version",
            "capabilities",
            "registry",
            "resources",
        },
        "runtime catalog",
    )
    if catalog["schema_version"] != RUNTIME_CATALOG_SCHEMA_VERSION:
        raise MermanRuntimeCatalogError(
            "runtime catalog schema_version is unsupported"
        )
    transport_api_version = _expect_positive_integer(
        catalog["transport_api_version"], "runtime catalog transport_api_version"
    )
    if transport_api_version != engine.binding_api_version():
        raise MermanRuntimeCatalogError(
            "runtime catalog transport_api_version does not match the loaded library"
        )
    package_version = catalog["package_version"]
    if not isinstance(package_version, str) or not package_version:
        raise MermanRuntimeCatalogError(
            "runtime catalog package_version must be a non-empty string"
        )
    if package_version != engine.package_version():
        raise MermanRuntimeCatalogError(
            "runtime catalog package_version does not match the loaded library"
        )

    _validate_capabilities(catalog["capabilities"])
    _validate_registry(catalog["registry"])
    _validate_resources(catalog["resources"])
    return cast(MermanRuntimeCatalog, catalog)


def _validate_capabilities(value: Any) -> None:
    capabilities = _expect_object(value, "runtime catalog capabilities")
    _require_required_keys(
        capabilities,
        {
            "capability_ids",
            "output_ids",
            "operation_ids",
            "system_adapter_ids",
            "text_measurement",
        },
        "runtime catalog capabilities",
    )
    capability_ids = _validate_identifier_list(
        capabilities["capability_ids"], "runtime capability IDs"
    )
    output_ids = _validate_identifier_list(
        capabilities["output_ids"], "runtime output IDs"
    )
    operation_ids = _validate_identifier_list(
        capabilities["operation_ids"], "runtime operation IDs"
    )
    system_adapter_ids = _validate_identifier_list(
        capabilities["system_adapter_ids"], "runtime system adapter IDs"
    )
    if not set(output_ids).issubset(operation_ids):
        raise MermanRuntimeCatalogError(
            "runtime output IDs must also be runtime operation IDs"
        )
    if not set(system_adapter_ids).issubset(capability_ids):
        raise MermanRuntimeCatalogError(
            "runtime system adapter IDs must also be runtime capability IDs"
        )

    text_measurement = capabilities["text_measurement"]
    if text_measurement is None:
        if "svg" in capability_ids:
            raise MermanRuntimeCatalogError(
                "runtime SVG capability requires text measurement metadata"
            )
        return
    if "svg" not in capability_ids:
        raise MermanRuntimeCatalogError(
            "runtime text measurement metadata requires the SVG capability"
        )
    measurement = _expect_object(text_measurement, "runtime text measurement")
    _require_required_keys(
        measurement,
        {"protocol_version", "provider_ids"},
        "runtime text measurement",
    )
    protocol_version = _expect_positive_integer(
        measurement["protocol_version"], "runtime text measurement protocol_version"
    )
    if (
        TEXT_MEASUREMENT_PROTOCOL_VERSION is None
        or protocol_version != TEXT_MEASUREMENT_PROTOCOL_VERSION
    ):
        raise MermanRuntimeCatalogError(
            "runtime text measurement protocol is incompatible with this package"
        )
    provider_ids = _validate_identifier_list(
        measurement["provider_ids"], "runtime text measurement provider IDs"
    )
    if "vendored" not in provider_ids:
        raise MermanRuntimeCatalogError(
            "runtime text measurement providers must include vendored"
        )


def _validate_registry(value: Any) -> None:
    registry = _expect_object(value, "runtime registry")
    _require_required_keys(registry, {"diagram_family_count"}, "runtime registry")
    count = registry["diagram_family_count"]
    if not _is_integer(count) or count < 0:
        raise MermanRuntimeCatalogError(
            "runtime registry diagram_family_count must be a non-negative integer"
        )


def _validate_resources(value: Any) -> None:
    resources = _expect_object(value, "runtime resources")
    _require_required_keys(
        resources,
        {
            "general_binding_default_profile",
            "cli_default_profile",
            "limits",
            "profiles",
        },
        "runtime resources",
    )
    for field in ["general_binding_default_profile", "cli_default_profile"]:
        if not isinstance(resources[field], str) or not resources[field]:
            raise MermanRuntimeCatalogError(
                f"runtime resources {field} must be a non-empty string"
            )
    if not isinstance(resources["limits"], list):
        raise MermanRuntimeCatalogError("runtime resources limits must be an array")
    for limit in resources["limits"]:
        item = _expect_object(limit, "runtime resource limit")
        _require_required_keys(
            item,
            {"id", "phase", "description", "overridable", "hard_cap"},
            "runtime resource limit",
        )
        _expect_non_empty_string(item["id"], "runtime resource limit ID")
        for field in ["phase", "description"]:
            if not isinstance(item[field], str):
                raise MermanRuntimeCatalogError(
                    f"runtime resource limit {field} must be a string"
                )
        for field in ["overridable", "hard_cap"]:
            if type(item[field]) is not bool:
                raise MermanRuntimeCatalogError(
                    f"runtime resource limit {field} must be a boolean"
                )
    if not isinstance(resources["profiles"], list):
        raise MermanRuntimeCatalogError("runtime resources profiles must be an array")
    for profile in resources["profiles"]:
        item = _expect_object(profile, "runtime resource profile")
        _require_required_keys(
            item,
            {
                "id",
                "purpose",
                "trust_assumption",
                "recommended_binding_default",
                "limits",
            },
            "runtime resource profile",
        )
        _expect_non_empty_string(item["id"], "runtime resource profile ID")
        for field in ["purpose", "trust_assumption"]:
            if not isinstance(item[field], str):
                raise MermanRuntimeCatalogError(
                    f"runtime resource profile {field} must be a string"
                )
        if type(item["recommended_binding_default"]) is not bool:
            raise MermanRuntimeCatalogError(
                "runtime resource profile recommended_binding_default must be a boolean"
            )
        limits = _expect_object(item["limits"], "runtime resource profile limits")
        if any(
            value is not None and (not _is_integer(value) or value < 0)
            for value in limits.values()
        ):
            raise MermanRuntimeCatalogError(
                "runtime resource profile limits must be non-negative integers or null"
            )


def _validate_identifier_list(value: Any, label: str) -> List[str]:
    if not isinstance(value, list):
        raise MermanRuntimeCatalogError(f"{label} must be a string array")
    identifiers = [_expect_identifier(item, label) for item in value]
    if identifiers != sorted(set(identifiers)):
        raise MermanRuntimeCatalogError(f"{label} must be sorted and unique")
    return identifiers


def _expect_identifier(value: Any, label: str) -> str:
    if not isinstance(value, str) or _IDENTIFIER.fullmatch(value) is None:
        raise MermanRuntimeCatalogError(f"{label} contains an invalid identifier")
    return value


def _expect_non_empty_string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise MermanRuntimeCatalogError(f"{label} must be a non-empty string")
    return value


def _expect_object(value: Any, label: str) -> Dict[str, Any]:
    if not isinstance(value, dict) or not all(isinstance(key, str) for key in value):
        raise MermanRuntimeCatalogError(f"{label} must be a JSON object")
    return value


def _require_required_keys(value: Dict[str, Any], keys: set, label: str) -> None:
    missing = keys - value.keys()
    if missing:
        raise MermanRuntimeCatalogError(
            f"{label} is missing required fields: {', '.join(sorted(missing))}"
        )


def _is_integer(value: Any) -> bool:
    return type(value) is int


def _expect_positive_integer(value: Any, label: str) -> int:
    if not _is_integer(value) or value <= 0:
        raise MermanRuntimeCatalogError(f"{label} must be a positive integer")
    return cast(int, value)
