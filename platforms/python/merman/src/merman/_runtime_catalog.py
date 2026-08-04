"""Typed, fail-closed access to the artifact-owned Merman runtime catalog."""

import json
import re
from typing import Any, Dict, List, Literal, Optional, Protocol, TypedDict, cast

from ._binding_contract import (
    BINDING_OPTION_GROUP_SPECS,
    BINDING_OPERATION_RELATION_SPECS,
    BINDING_TRANSPORT_EXPOSURE_SPECS,
    CAPABILITY_SPECS,
    CONSTRUCTOR_SERVICE_SPECS,
    METADATA_SPECS,
    REQUIRED_PAYLOAD_SCHEMA_VERSIONS,
    RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN,
    RUNTIME_CATALOG_IDENTIFIER_PATTERN,
    RUNTIME_CATALOG_MAX_SAFE_INTEGER,
    RUNTIME_CATALOG_SCHEMA_VERSION,
    TEXT_MEASUREMENT_PROVIDER_SPECS,
)
from ._resource_options import BINDING_OPTIONS_SCHEMA_VERSION, ResourceLimitId

try:
    from ._text_measurement_protocol import TEXT_MEASUREMENT_PROTOCOL_VERSION
except ModuleNotFoundError as exc:
    if exc.name != f"{__package__}._text_measurement_protocol":
        raise
    TEXT_MEASUREMENT_PROTOCOL_VERSION = None


_IDENTIFIER = re.compile(RUNTIME_CATALOG_IDENTIFIER_PATTERN)
_FIELD_IDENTIFIER = re.compile(RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN)
_CAPABILITY_SPEC_BY_ID = {spec["id"]: spec for spec in CAPABILITY_SPECS}
_METADATA_SPEC_BY_ID = {spec["id"]: spec for spec in METADATA_SPECS}
_KNOWN_OPTION_GROUP_IDS = {spec["id"] for spec in BINDING_OPTION_GROUP_SPECS}
_TRANSPORT_EXPOSURE_SPEC_BY_ID = {
    spec["id"]: spec for spec in BINDING_TRANSPORT_EXPOSURE_SPECS
}
_UNIFFI_TRANSPORT_EXPOSURE = _TRANSPORT_EXPOSURE_SPEC_BY_ID["uniffi"]
_REQUIRED_TRANSPORT_PAYLOAD_SCHEMA_VERSIONS = {
    schema_id: REQUIRED_PAYLOAD_SCHEMA_VERSIONS[schema_id]
    for schema_id in _UNIFFI_TRANSPORT_EXPOSURE["payload_schema_ids"]
}
_OPERATION_RELATION_SPEC_BY_ID = {
    spec["operation_id"]: spec for spec in BINDING_OPERATION_RELATION_SPECS
}
_CONSTRUCTOR_SERVICE_SPEC_BY_ID = {
    spec["id"]: spec for spec in CONSTRUCTOR_SERVICE_SPECS
}
_CONSTRUCTOR_SERVICE_CANDIDATE_IDS = set(
    _UNIFFI_TRANSPORT_EXPOSURE["constructor_service_candidate_ids"]
)
_TEXT_MEASUREMENT_PROVIDER_SPEC_BY_ID = {
    spec["id"]: spec for spec in TEXT_MEASUREMENT_PROVIDER_SPECS
}


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
    id: ResourceLimitId
    phase: str
    description: str
    overridable: bool
    hard_cap: bool
    minimum_value: int
    operation_ids: List[str]


class MermanRuntimePayloadSchema(TypedDict):
    id: str
    version: int


class MermanRuntimeConstructorResourceLimit(TypedDict):
    id: str
    phase: str
    unit: str
    description: str
    value: int


class MermanRuntimeConstructorServiceContract(TypedDict):
    id: str
    provided_text_measurement_provider_ids: List[str]
    resource_limits: List[MermanRuntimeConstructorResourceLimit]


class MermanRuntimeResourceProfile(TypedDict):
    id: str
    purpose: str
    trust_assumption: str
    recommended_binding_default: bool
    limits: Dict[ResourceLimitId, Optional[int]]


class MermanRuntimeResources(TypedDict):
    general_binding_default_profile: str
    cli_default_profile: str
    limits: List[MermanRuntimeResourceLimit]
    profiles: List[MermanRuntimeResourceProfile]


class _MermanRuntimeCatalogRequired(TypedDict):
    schema_version: Literal[1]
    transport_api_version: int
    package_version: str
    options_schema_versions: List[int]
    payload_schemas: List[MermanRuntimePayloadSchema]
    metadata_ids: List[str]
    option_group_ids: List[str]
    constructor_service_ids: List[str]
    constructor_service_contracts: List[MermanRuntimeConstructorServiceContract]
    capabilities: MermanRuntimeCapabilities
    output_contracts: List[MermanOutputContract]
    registry: MermanRuntimeRegistry
    resources: MermanRuntimeResources


class MermanRuntimeCatalog(_MermanRuntimeCatalogRequired, total=False):
    pass


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
    if (
        not _is_integer(catalog["schema_version"])
        or catalog["schema_version"] != RUNTIME_CATALOG_SCHEMA_VERSION
    ):
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

    capability_ids, output_ids, operation_ids, provider_ids = _validate_capabilities(
        catalog["capabilities"]
    )
    _validate_operation_relations(operation_ids, capability_ids, output_ids)
    options_schema_versions = _validate_options_schema_versions(
        catalog["options_schema_versions"]
    )
    if BINDING_OPTIONS_SCHEMA_VERSION not in options_schema_versions:
        raise MermanRuntimeCatalogError(
            "runtime catalog does not advertise the current Options JSON schema"
        )
    _validate_payload_schemas(catalog["payload_schemas"])
    _validate_metadata_ids(catalog["metadata_ids"], capability_ids)
    if "option_group_ids" in catalog:
        _validate_option_group_ids(
            catalog["option_group_ids"],
            capability_ids,
            uses_svg_pipeline=catalog["capabilities"]["text_measurement"] is not None,
        )
    else:
        catalog["option_group_ids"] = []
    _validate_constructor_services(
        catalog,
        provider_ids,
    )
    _validate_output_contracts(catalog["output_contracts"], output_ids)
    _validate_registry(catalog["registry"])
    _validate_resources(catalog["resources"], operation_ids)
    return cast(MermanRuntimeCatalog, catalog)


def _validate_capabilities(
    value: Any,
) -> tuple[List[str], List[str], List[str], List[str]]:
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
    _validate_capability_implications(capability_ids)

    requires_svg_pipeline = "svg" in capability_ids or any(
        "svg"
        in _OPERATION_RELATION_SPEC_BY_ID.get(operation_id, {}).get(
            "compiled_prerequisite_ids", ()
        )
        for operation_id in operation_ids
    )

    text_measurement = capabilities["text_measurement"]
    if text_measurement is None:
        if requires_svg_pipeline:
            raise MermanRuntimeCatalogError(
                "runtime SVG pipeline requires text measurement metadata"
            )
        return capability_ids, output_ids, operation_ids, []
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
    return capability_ids, output_ids, operation_ids, provider_ids


def _validate_capability_implications(capability_ids: List[str]) -> None:
    capability_id_set = set(capability_ids)
    for capability_id in capability_ids:
        spec = _CAPABILITY_SPEC_BY_ID.get(capability_id)
        if spec is None:
            continue
        for implication_id in spec["implication_ids"]:
            if implication_id not in capability_id_set:
                raise MermanRuntimeCatalogError(
                    f"runtime capability {capability_id} is missing implied capability "
                    f"{implication_id}"
                )


def _validate_metadata_ids(value: Any, capability_ids: List[str]) -> List[str]:
    ids = _validate_identifier_list(value, "runtime metadata IDs")
    capability_id_set = set(capability_ids)
    for identifier in ids:
        spec = _METADATA_SPEC_BY_ID.get(identifier)
        if (
            spec is not None
            and spec["required_capability_id"] is not None
            and spec["required_capability_id"] not in capability_id_set
        ):
            raise MermanRuntimeCatalogError(
                f"runtime metadata {identifier} requires capability "
                f"{spec['required_capability_id']}"
            )
    return ids


def _validate_operation_relations(
    operation_ids: List[str], capability_ids: List[str], output_ids: List[str]
) -> None:
    capability_id_set = set(capability_ids)
    output_id_set = set(output_ids)
    for operation_id in operation_ids:
        spec = _OPERATION_RELATION_SPEC_BY_ID.get(operation_id)
        if spec is None:
            continue
        required_capability_id = spec["availability_capability_id"]
        if (
            required_capability_id is not None
            and required_capability_id not in capability_id_set
        ):
            raise MermanRuntimeCatalogError(
                f"runtime operation {operation_id} is missing capability "
                f"{required_capability_id}"
            )
        output_id = spec["output_id"]
        if output_id is not None and output_id not in output_id_set:
            raise MermanRuntimeCatalogError(
                f"runtime operation {operation_id} is missing output {output_id}"
            )


def _validate_option_group_ids(
    value: Any,
    capability_ids: List[str],
    *,
    uses_svg_pipeline: bool,
) -> List[str]:
    ids = _validate_sorted_string_list(value, "runtime option group IDs")
    for identifier in ids:
        _expect_field_identifier(identifier, "runtime option group IDs")
    capability_id_set = set(capability_ids)
    expected_known_ids = sorted(
        spec["id"]
        for spec in BINDING_OPTION_GROUP_SPECS
        if spec["always_available"]
        or (spec["requires_svg_pipeline"] and uses_svg_pipeline)
        or any(
            capability_id in capability_id_set
            for capability_id in spec["any_capability_ids"]
        )
    )
    actual_known_ids = [
        identifier for identifier in ids if identifier in _KNOWN_OPTION_GROUP_IDS
    ]
    if actual_known_ids != expected_known_ids:
        raise MermanRuntimeCatalogError(
            "runtime option group IDs do not match the artifact capability closure"
        )
    return ids


def _validate_constructor_services(
    catalog: Dict[str, Any], provider_ids: List[str]
) -> None:
    has_ids = "constructor_service_ids" in catalog
    has_contracts = "constructor_service_contracts" in catalog
    if has_ids != has_contracts:
        raise MermanRuntimeCatalogError(
            "runtime constructor service IDs and contracts must appear together"
        )
    if not has_ids:
        catalog["constructor_service_ids"] = []
        catalog["constructor_service_contracts"] = []
        return

    service_ids = _validate_identifier_list(
        catalog["constructor_service_ids"],
        "runtime constructor service IDs",
    )
    uses_svg_pipeline = bool(provider_ids)
    expected_known_ids = sorted(
        spec["id"]
        for spec in CONSTRUCTOR_SERVICE_SPECS
        if spec["id"] in _CONSTRUCTOR_SERVICE_CANDIDATE_IDS
        and (not spec["requires_svg_pipeline"] or uses_svg_pipeline)
    )
    actual_known_ids = [
        service_id
        for service_id in service_ids
        if service_id in _CONSTRUCTOR_SERVICE_SPEC_BY_ID
    ]
    if actual_known_ids != expected_known_ids:
        raise MermanRuntimeCatalogError(
            "runtime constructor service IDs do not match the transport exposure"
        )
    for service_id in service_ids:
        service_spec = _CONSTRUCTOR_SERVICE_SPEC_BY_ID.get(service_id)
        if service_spec is None:
            continue
        if service_id not in _CONSTRUCTOR_SERVICE_CANDIDATE_IDS:
            raise MermanRuntimeCatalogError(
                f"runtime constructor service {service_id} is unavailable "
                "through this Python facade"
            )
        if service_spec["requires_svg_pipeline"] and not uses_svg_pipeline:
            raise MermanRuntimeCatalogError(
                f"runtime constructor service {service_id} requires an SVG pipeline"
            )
    contracts = _validate_constructor_service_contracts(
        catalog["constructor_service_contracts"]
    )
    contract_ids = [contract["id"] for contract in contracts]
    if contract_ids != service_ids:
        raise MermanRuntimeCatalogError(
            "runtime constructor service contracts must exactly match "
            "constructor service IDs"
        )
    _validate_constructor_service_providers(contracts, provider_ids)


def _validate_constructor_service_contracts(
    value: Any,
) -> List[MermanRuntimeConstructorServiceContract]:
    if not isinstance(value, list):
        raise MermanRuntimeCatalogError(
            "runtime constructor service contracts must be an array"
        )
    contracts: List[MermanRuntimeConstructorServiceContract] = []
    previous_id = None
    for value_item in value:
        item = _expect_object(value_item, "runtime constructor service contract")
        _require_required_keys(
            item,
            {"id", "provided_text_measurement_provider_ids", "resource_limits"},
            "runtime constructor service contract",
        )
        service_id = _expect_identifier(
            item["id"], "runtime constructor service contract ID"
        )
        if previous_id is not None and previous_id >= service_id:
            raise MermanRuntimeCatalogError(
                "runtime constructor service contracts must be sorted and unique by ID"
            )
        _validate_identifier_list(
            item["provided_text_measurement_provider_ids"],
            f"runtime constructor service {service_id} text measurement provider IDs",
        )
        _validate_constructor_service_resource_limits(
            item["resource_limits"], service_id
        )
        previous_id = service_id
        contracts.append(cast(MermanRuntimeConstructorServiceContract, item))
    return contracts


def _validate_constructor_service_resource_limits(
    value: Any, service_id: str
) -> None:
    if not isinstance(value, list):
        raise MermanRuntimeCatalogError(
            f"runtime constructor service {service_id} resource limits must be an array"
        )
    limit_ids: List[str] = []
    for value_item in value:
        item = _expect_object(
            value_item,
            f"runtime constructor service {service_id} resource limit",
        )
        _require_required_keys(
            item,
            {"id", "phase", "unit", "description", "value"},
            f"runtime constructor service {service_id} resource limit",
        )
        limit_ids.append(
            _expect_field_identifier(
                item["id"],
                f"runtime constructor service {service_id} resource limit ID",
            )
        )
        for field in ["phase", "unit", "description"]:
            _expect_non_empty_string(
                item[field],
                f"runtime constructor service {service_id} resource limit {field}",
            )
        if not _is_safe_integer(item["value"]) or item["value"] < 0:
            raise MermanRuntimeCatalogError(
                f"runtime constructor service {service_id} resource limit value "
                "must be a non-negative JSON-safe integer"
            )
    if limit_ids != sorted(set(limit_ids)):
        raise MermanRuntimeCatalogError(
            f"runtime constructor service {service_id} resource limits must be "
            "sorted and unique by ID"
        )


def _validate_constructor_service_providers(
    contracts: List[MermanRuntimeConstructorServiceContract],
    provider_ids: List[str],
) -> None:
    available_provider_ids = set(provider_ids)
    contracts_by_id = {contract["id"]: contract for contract in contracts}
    provider_owner_by_id: Dict[str, str] = {}

    for contract in contracts:
        service_id = contract["id"]
        contract_provider_ids = contract["provided_text_measurement_provider_ids"]
        service_spec = _CONSTRUCTOR_SERVICE_SPEC_BY_ID.get(service_id)
        if service_spec is not None:
            actual_known_provider_ids = [
                provider_id
                for provider_id in contract_provider_ids
                if provider_id in _TEXT_MEASUREMENT_PROVIDER_SPEC_BY_ID
            ]
            expected_known_provider_ids = list(
                service_spec["provided_text_measurement_provider_ids"]
            )
            if actual_known_provider_ids != expected_known_provider_ids:
                raise MermanRuntimeCatalogError(
                    f"runtime constructor service {service_id} does not match "
                    "its known provider contract"
                )

        for provider_id in contract_provider_ids:
            if provider_id not in available_provider_ids:
                raise MermanRuntimeCatalogError(
                    f"runtime constructor service {service_id} names an "
                    "unavailable text measurement provider"
                )
            if provider_id in provider_owner_by_id:
                raise MermanRuntimeCatalogError(
                    f"runtime text measurement provider {provider_id} has "
                    "multiple constructor service owners"
                )
            provider_owner_by_id[provider_id] = service_id

            provider_spec = _TEXT_MEASUREMENT_PROVIDER_SPEC_BY_ID.get(provider_id)
            if provider_spec is not None and (
                provider_spec["source"] != "constructor-service"
                or provider_spec["constructor_service_id"] != service_id
            ):
                raise MermanRuntimeCatalogError(
                    f"runtime text measurement provider {provider_id} has the "
                    "wrong constructor service owner"
                )

    for provider_id in provider_ids:
        provider_spec = _TEXT_MEASUREMENT_PROVIDER_SPEC_BY_ID.get(provider_id)
        if provider_spec is None or provider_spec["source"] != "constructor-service":
            continue
        owner_id = provider_spec["constructor_service_id"]
        owner_contract = contracts_by_id.get(owner_id)
        if (
            owner_contract is None
            or provider_id
            not in owner_contract["provided_text_measurement_provider_ids"]
        ):
            raise MermanRuntimeCatalogError(
                f"runtime text measurement provider {provider_id} is missing "
                "its constructor service owner"
            )


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
        if limit is not None and (not _is_safe_integer(limit) or limit <= 0):
            raise MermanRuntimeCatalogError(
                f"runtime embedded image limit {field} must be a positive integer or null"
            )


def _validate_registry(value: Any) -> None:
    registry = _expect_object(value, "runtime registry")
    _require_required_keys(registry, {"diagram_family_count"}, "runtime registry")
    count = registry["diagram_family_count"]
    if not _is_safe_integer(count) or count < 0:
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
        _expect_identifier(resources[field], f"runtime resources {field}")
    if not isinstance(resources["limits"], list):
        raise MermanRuntimeCatalogError("runtime resources limits must be an array")
    minimums: Dict[ResourceLimitId, int] = {}
    hard_cap_ids: set[ResourceLimitId] = set()
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
        limit_id = ResourceLimitId.from_id(
            _expect_field_identifier(item["id"], "runtime resource limit ID")
        )
        item["id"] = limit_id
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
        if not _is_safe_integer(item["minimum_value"]) or item["minimum_value"] < 0:
            raise MermanRuntimeCatalogError(
                "runtime resource limit minimum_value must be a non-negative integer"
            )
        if limit_id in minimums:
            raise MermanRuntimeCatalogError(
                "runtime resource limit IDs must be unique"
            )
        if item["hard_cap"] and item["overridable"]:
            raise MermanRuntimeCatalogError(
                "runtime hard resource limits cannot be overridable"
            )
        minimums[limit_id] = item["minimum_value"]
        if item["hard_cap"]:
            hard_cap_ids.add(limit_id)
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
        _expect_identifier(item["id"], "runtime resource profile ID")
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
        raw_limits = _expect_object(item["limits"], "runtime resource profile limits")
        if set(raw_limits) != set(minimums):
            raise MermanRuntimeCatalogError(
                "runtime resource profile limits must cover the declared limits"
            )
        limits: Dict[ResourceLimitId, Optional[int]] = {}
        for raw_limit_id, value in raw_limits.items():
            limit_id = ResourceLimitId.from_id(raw_limit_id)
            if value is None:
                if limit_id in hard_cap_ids:
                    raise MermanRuntimeCatalogError(
                        "runtime resource profile removed a finite hard cap"
                    )
                limits[limit_id] = None
                continue
            if not _is_safe_integer(value) or value < minimums[limit_id]:
                raise MermanRuntimeCatalogError(
                    "runtime resource profile limits must meet the declared minimum or be null"
                )
            limits[limit_id] = value
        item["limits"] = limits
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
    if any(not _is_safe_integer(version) or version <= 0 for version in value):
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
        if not _is_safe_integer(schema["version"]) or schema["version"] <= 0:
            raise MermanRuntimeCatalogError(
                "runtime payload schema version must be a positive integer"
            )
        previous = identifier
        schemas.append(cast(MermanRuntimePayloadSchema, schema))
    versions_by_id = {schema["id"]: schema["version"] for schema in schemas}
    known_schema_ids = {
        schema["id"]
        for schema in schemas
        if schema["id"] in REQUIRED_PAYLOAD_SCHEMA_VERSIONS
    }
    if known_schema_ids != set(_REQUIRED_TRANSPORT_PAYLOAD_SCHEMA_VERSIONS):
        raise MermanRuntimeCatalogError(
            "runtime payload schemas do not match the Python transport exposure"
        )
    for identifier, version in _REQUIRED_TRANSPORT_PAYLOAD_SCHEMA_VERSIONS.items():
        if versions_by_id.get(identifier) != version:
            raise MermanRuntimeCatalogError(
                f"runtime payload schema {identifier} must have version {version}"
            )
    return schemas


def _expect_identifier(value: Any, label: str) -> str:
    if not isinstance(value, str) or _IDENTIFIER.fullmatch(value) is None:
        raise MermanRuntimeCatalogError(f"{label} contains an invalid identifier")
    return value


def _expect_field_identifier(value: Any, label: str) -> str:
    if not isinstance(value, str) or _FIELD_IDENTIFIER.fullmatch(value) is None:
        raise MermanRuntimeCatalogError(f"{label} contains an invalid field identifier")
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


def _is_safe_integer(value: Any) -> bool:
    return _is_integer(value) and abs(value) <= RUNTIME_CATALOG_MAX_SAFE_INTEGER


def _expect_positive_integer(value: Any, label: str) -> int:
    if not _is_safe_integer(value) or value <= 0:
        raise MermanRuntimeCatalogError(f"{label} must be a positive integer")
    return cast(int, value)
