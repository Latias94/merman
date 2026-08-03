"""Typed, fail-closed access to the artifact-owned Merman runtime catalog."""

import json
import re
from typing import Any, Dict, List, Optional, Protocol, TypedDict, cast

from ._binding_contract import REQUIRED_PAYLOAD_SCHEMA_VERSIONS
from ._resource_options import BINDING_OPTIONS_SCHEMA_VERSION

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


class MermanSystemFontContract(TypedDict):
    source_id: str
    discovery: str
    cache_scope: str
    host_dependent: bool
    caller_configurable: bool
    resource_bounded: bool


class MermanEmbeddedImageLimits(TypedDict):
    max_bytes_per_image: Optional[int]
    max_total_bytes: Optional[int]
    max_pixels_per_image: Optional[int]
    max_total_pixels: Optional[int]


class MermanEmbeddedImageContract(TypedDict):
    source_ids: List[str]
    filesystem_access: bool
    network_access: bool
    caller_configurable: bool
    limits: MermanEmbeddedImageLimits


class MermanOutputContract(TypedDict):
    id: str
    media_type: str
    system_fonts: Optional[MermanSystemFontContract]
    embedded_images: Optional[MermanEmbeddedImageContract]


class MermanRuntimeRegistry(TypedDict):
    diagram_family_count: int


class MermanRuntimeResourceLimit(TypedDict):
    id: str
    phase: str
    description: str
    overridable: bool
    hard_cap: bool
    minimum_value: int
    operation_ids: List[str]


class MermanRuntimePayloadSchema(TypedDict):
    id: str
    version: int


class MermanRuntimeResourceProfile(TypedDict):
    id: str
    purpose: str
    trust_assumption: str
    recommended_binding_default: bool
    limits: Dict[str, Optional[int]]


class MermanRuntimeResources(TypedDict):
    general_binding_default_profile: str
    cli_default_profile: str
    limits: List[MermanRuntimeResourceLimit]
    profiles: List[MermanRuntimeResourceProfile]


class _MermanRuntimeCatalogRequired(TypedDict):
    schema_version: int
    transport_api_version: int
    package_version: str
    options_schema_versions: List[int]
    payload_schemas: List[MermanRuntimePayloadSchema]
    metadata_ids: List[str]
    capabilities: MermanRuntimeCapabilities
    output_contracts: List[MermanOutputContract]
    registry: MermanRuntimeRegistry
    resources: MermanRuntimeResources


class MermanRuntimeCatalog(_MermanRuntimeCatalogRequired, total=False):
    option_group_ids: List[str]
    constructor_service_ids: List[str]


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
            "options_schema_versions",
            "payload_schemas",
            "metadata_ids",
            "capabilities",
            "output_contracts",
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

    output_ids, operation_ids, provider_ids = _validate_capabilities(
        catalog["capabilities"]
    )
    options_schema_versions = _validate_options_schema_versions(
        catalog["options_schema_versions"]
    )
    if BINDING_OPTIONS_SCHEMA_VERSION not in options_schema_versions:
        raise MermanRuntimeCatalogError(
            "runtime catalog does not advertise the current Options JSON schema"
        )
    _validate_payload_schemas(catalog["payload_schemas"])
    _validate_identifier_list(
        catalog["metadata_ids"],
        "runtime metadata IDs",
    )
    option_group_ids = catalog.setdefault("option_group_ids", [])
    _validate_option_group_ids(option_group_ids)
    constructor_service_ids = catalog.setdefault("constructor_service_ids", [])
    _validate_identifier_list(
        constructor_service_ids,
        "runtime constructor service IDs",
    )
    if (
        "host-text-measurement" in constructor_service_ids
        and "host-callback" not in provider_ids
    ):
        raise MermanRuntimeCatalogError(
            "runtime host text-measurement service requires the host-callback provider"
        )
    _validate_output_contracts(catalog["output_contracts"], output_ids)
    _validate_registry(catalog["registry"])
    _validate_resources(catalog["resources"], operation_ids)
    return cast(MermanRuntimeCatalog, catalog)


def _validate_capabilities(value: Any) -> tuple[List[str], List[str], List[str]]:
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
        return output_ids, operation_ids, []
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
    return output_ids, operation_ids, provider_ids


def _validate_option_group_ids(value: Any) -> List[str]:
    ids = _validate_sorted_string_list(value, "runtime option group IDs")
    if any(re.fullmatch(r"[a-z][a-z0-9_]*", item) is None for item in ids):
        raise MermanRuntimeCatalogError(
            "runtime option group IDs contain an invalid field identifier"
        )
    return ids


def _validate_output_contracts(value: Any, output_ids: List[str]) -> None:
    if not isinstance(value, list):
        raise MermanRuntimeCatalogError("runtime output contracts must be an array")

    contract_ids: List[str] = []
    for value_item in value:
        item = _expect_object(value_item, "runtime output contract")
        _require_required_keys(
            item,
            {"id", "media_type", "system_fonts", "embedded_images"},
            "runtime output contract",
        )
        contract_ids.append(_expect_identifier(item["id"], "runtime output contract ID"))
        _expect_non_empty_string(
            item["media_type"], "runtime output contract media_type"
        )
        _validate_system_fonts(item["system_fonts"])
        _validate_embedded_images(item["embedded_images"])

    if contract_ids != output_ids:
        raise MermanRuntimeCatalogError(
            "runtime output contract IDs must exactly match runtime output IDs"
        )


def _validate_system_fonts(value: Any) -> None:
    if value is None:
        return
    fonts = _expect_object(value, "runtime system font contract")
    _require_required_keys(
        fonts,
        {
            "source_id",
            "discovery",
            "cache_scope",
            "host_dependent",
            "caller_configurable",
            "resource_bounded",
        },
        "runtime system font contract",
    )
    for field in ["source_id", "discovery", "cache_scope"]:
        _expect_identifier(fonts[field], f"runtime system font contract {field}")
    for field in ["host_dependent", "caller_configurable", "resource_bounded"]:
        if type(fonts[field]) is not bool:
            raise MermanRuntimeCatalogError(
                f"runtime system font contract {field} must be a boolean"
            )


def _validate_embedded_images(value: Any) -> None:
    if value is None:
        return
    images = _expect_object(value, "runtime embedded image contract")
    _require_required_keys(
        images,
        {
            "source_ids",
            "filesystem_access",
            "network_access",
            "caller_configurable",
            "limits",
        },
        "runtime embedded image contract",
    )
    _validate_identifier_list(
        images["source_ids"], "runtime embedded image source IDs"
    )
    for field in ["filesystem_access", "network_access", "caller_configurable"]:
        if type(images[field]) is not bool:
            raise MermanRuntimeCatalogError(
                f"runtime embedded image contract {field} must be a boolean"
            )

    limits = _expect_object(images["limits"], "runtime embedded image limits")
    limit_fields = {
        "max_bytes_per_image",
        "max_total_bytes",
        "max_pixels_per_image",
        "max_total_pixels",
    }
    _require_required_keys(limits, limit_fields, "runtime embedded image limits")
    for field in limit_fields:
        limit = limits[field]
        if limit is not None and (not _is_integer(limit) or limit <= 0):
            raise MermanRuntimeCatalogError(
                f"runtime embedded image limit {field} must be a positive integer or null"
            )


def _validate_registry(value: Any) -> None:
    registry = _expect_object(value, "runtime registry")
    _require_required_keys(registry, {"diagram_family_count"}, "runtime registry")
    count = registry["diagram_family_count"]
    if not _is_integer(count) or count < 0:
        raise MermanRuntimeCatalogError(
            "runtime registry diagram_family_count must be a non-negative integer"
        )


def _validate_resources(value: Any, operation_ids: List[str]) -> None:
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
    minimums: Dict[str, int] = {}
    hard_cap_ids = set()
    for limit in resources["limits"]:
        item = _expect_object(limit, "runtime resource limit")
        _require_required_keys(
            item,
            {
                "id",
                "phase",
                "description",
                "overridable",
                "hard_cap",
                "minimum_value",
                "operation_ids",
            },
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
        if not _is_integer(item["minimum_value"]) or item["minimum_value"] < 0:
            raise MermanRuntimeCatalogError(
                "runtime resource limit minimum_value must be a non-negative integer"
            )
        if item["id"] in minimums:
            raise MermanRuntimeCatalogError(
                "runtime resource limit IDs must be unique"
            )
        if item["hard_cap"] and item["overridable"]:
            raise MermanRuntimeCatalogError(
                "runtime hard resource limits cannot be overridable"
            )
        minimums[item["id"]] = item["minimum_value"]
        if item["hard_cap"]:
            hard_cap_ids.add(item["id"])
        limit_operation_ids = _validate_identifier_list(
            item["operation_ids"], "runtime resource limit operation IDs"
        )
        if not set(limit_operation_ids).issubset(operation_ids):
            raise MermanRuntimeCatalogError(
                "runtime resource limit operation IDs must be declared runtime operations"
            )
    if not isinstance(resources["profiles"], list):
        raise MermanRuntimeCatalogError("runtime resources profiles must be an array")
    profile_ids = set()
    recommended_profile_ids = set()
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
        if item["id"] in profile_ids:
            raise MermanRuntimeCatalogError(
                "runtime resource profile IDs must be unique"
            )
        profile_ids.add(item["id"])
        if item["recommended_binding_default"]:
            recommended_profile_ids.add(item["id"])
        limits = _expect_object(item["limits"], "runtime resource profile limits")
        if set(limits) != set(minimums):
            raise MermanRuntimeCatalogError(
                "runtime resource profile limits must cover the declared limits"
            )
        for limit_id, value in limits.items():
            if value is None:
                if limit_id in hard_cap_ids:
                    raise MermanRuntimeCatalogError(
                        "runtime resource profile removed a finite hard cap"
                    )
                continue
            if not _is_integer(value) or value < minimums[limit_id]:
                raise MermanRuntimeCatalogError(
                    "runtime resource profile limits must meet the declared minimum or be null"
                )
    general_default = resources["general_binding_default_profile"]
    cli_default = resources["cli_default_profile"]
    if general_default not in profile_ids or cli_default not in profile_ids:
        raise MermanRuntimeCatalogError(
            "runtime resource defaults must name declared profiles"
        )
    if recommended_profile_ids != {general_default}:
        raise MermanRuntimeCatalogError(
            "runtime resources must recommend exactly the general binding default"
        )


def _validate_identifier_list(value: Any, label: str) -> List[str]:
    identifiers = _validate_sorted_string_list(value, label)
    for identifier in identifiers:
        _expect_identifier(identifier, label)
    return identifiers


def _validate_sorted_string_list(value: Any, label: str) -> List[str]:
    if not isinstance(value, list):
        raise MermanRuntimeCatalogError(f"{label} must be a string array")
    if any(not isinstance(item, str) for item in value):
        raise MermanRuntimeCatalogError(f"{label} must be a string array")
    if value != sorted(set(value)):
        raise MermanRuntimeCatalogError(f"{label} must be sorted and unique")
    return value


def _validate_options_schema_versions(value: Any) -> List[int]:
    if not isinstance(value, list):
        raise MermanRuntimeCatalogError(
            "runtime options schema versions must be an array"
        )
    if any(not _is_integer(version) or version <= 0 for version in value):
        raise MermanRuntimeCatalogError(
            "runtime options schema versions must contain positive integers"
        )
    if value != sorted(set(value)):
        raise MermanRuntimeCatalogError(
            "runtime options schema versions must be sorted and unique"
        )
    return value


def _validate_payload_schemas(value: Any) -> List[MermanRuntimePayloadSchema]:
    if not isinstance(value, list):
        raise MermanRuntimeCatalogError("runtime payload schemas must be an array")
    schemas: List[MermanRuntimePayloadSchema] = []
    previous = None
    for item in value:
        schema = _expect_object(item, "runtime payload schema")
        _require_required_keys(schema, {"id", "version"}, "runtime payload schema")
        identifier = _expect_identifier(schema["id"], "runtime payload schema ID")
        if previous is not None and previous >= identifier:
            raise MermanRuntimeCatalogError(
                "runtime payload schema IDs must be sorted and unique"
            )
        if not _is_integer(schema["version"]) or schema["version"] <= 0:
            raise MermanRuntimeCatalogError(
                "runtime payload schema version must be a positive integer"
            )
        previous = identifier
        schemas.append(cast(MermanRuntimePayloadSchema, schema))
    versions_by_id = {schema["id"]: schema["version"] for schema in schemas}
    for identifier, version in REQUIRED_PAYLOAD_SCHEMA_VERSIONS.items():
        if versions_by_id.get(identifier) != version:
            raise MermanRuntimeCatalogError(
                f"runtime payload schema {identifier} must have version {version}"
            )
    return schemas


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
