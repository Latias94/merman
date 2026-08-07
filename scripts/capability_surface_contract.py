"""Shared strict reader for the canonical capability-surface contract."""

from __future__ import annotations

from collections.abc import Callable
import hashlib
import json
from typing import TypeAlias


ErrorFactory: TypeAlias = Callable[[str], Exception]

CAPABILITY_KINDS = {"adapter", "api", "engine", "output", "tool"}
SURFACE_FIELDS = {
    "schema_version",
    "descriptor_id",
    "targets",
    "capabilities",
    "outputs",
    "binding_operations",
}
OPERATION_FIELDS = {
    "id",
    "capability",
    "output",
    "compiled_prerequisites",
    "description",
    "media_type",
    "requires_uri",
    "targets",
}


def canonical_capability_surface(
    value: object,
    *,
    error_factory: ErrorFactory,
    context: str = "capability surface",
    expected_schema_version: int | None = None,
    require_sorted_compiled_prerequisites: bool = False,
) -> dict[str, object]:
    """Validate and canonicalize the semantic capability descriptor."""

    surface = _object(value, context, error_factory, fields=SURFACE_FIELDS)
    schema_version = _integer(
        surface["schema_version"],
        f"{context} schema_version",
        error_factory,
    )
    if (
        expected_schema_version is not None
        and schema_version != expected_schema_version
    ):
        raise error_factory(
            f"{context} schema_version must be {expected_schema_version}"
        )
    descriptor_id = _string(
        surface["descriptor_id"],
        f"{context} descriptor_id",
        error_factory,
    )

    targets: list[dict[str, object]] = []
    for index, item in enumerate(
        _array(surface["targets"], f"{context} targets", error_factory)
    ):
        label = f"{context} targets[{index}]"
        target = _object(
            item,
            label,
            error_factory,
            fields={"id", "description"},
        )
        targets.append(
            {
                "id": _string(target["id"], f"{label}.id", error_factory),
                "description": _string(
                    target["description"],
                    f"{label}.description",
                    error_factory,
                ),
            }
        )
    _unique_ids(targets, f"{context} targets", error_factory)

    capabilities: list[dict[str, object]] = []
    for index, item in enumerate(
        _array(
            surface["capabilities"],
            f"{context} capabilities",
            error_factory,
        )
    ):
        label = f"{context} capabilities[{index}]"
        capability = _object(
            item,
            label,
            error_factory,
            fields={
                "id",
                "kind",
                "description",
                "targets",
                "implications",
                "absence",
            },
        )
        kind = _string(capability["kind"], f"{label}.kind", error_factory)
        if kind not in CAPABILITY_KINDS:
            raise error_factory(f"{label}.kind is unknown: {kind!r}")
        absence = _object(
            capability["absence"],
            f"{label}.absence",
            error_factory,
            fields={"error_id", "contract"},
        )
        capabilities.append(
            {
                "id": _string(capability["id"], f"{label}.id", error_factory),
                "kind": kind,
                "description": _string(
                    capability["description"],
                    f"{label}.description",
                    error_factory,
                ),
                "targets": _string_array(
                    capability["targets"],
                    f"{label}.targets",
                    error_factory,
                ),
                "implications": _string_array(
                    capability["implications"],
                    f"{label}.implications",
                    error_factory,
                ),
                "absence": {
                    "error_id": _string(
                        absence["error_id"],
                        f"{label}.absence.error_id",
                        error_factory,
                    ),
                    "contract": _string(
                        absence["contract"],
                        f"{label}.absence.contract",
                        error_factory,
                    ),
                },
            }
        )
    _unique_ids(capabilities, f"{context} capabilities", error_factory)

    outputs: list[dict[str, object]] = []
    for index, item in enumerate(
        _array(surface["outputs"], f"{context} outputs", error_factory)
    ):
        label = f"{context} outputs[{index}]"
        output = _object(
            item,
            label,
            error_factory,
            fields={"id", "capability", "description", "media_type", "targets"},
        )
        outputs.append(
            {
                "id": _string(output["id"], f"{label}.id", error_factory),
                "capability": _string(
                    output["capability"],
                    f"{label}.capability",
                    error_factory,
                ),
                "description": _string(
                    output["description"],
                    f"{label}.description",
                    error_factory,
                ),
                "media_type": _string(
                    output["media_type"],
                    f"{label}.media_type",
                    error_factory,
                ),
                "targets": _string_array(
                    output["targets"],
                    f"{label}.targets",
                    error_factory,
                ),
            }
        )
    _unique_ids(outputs, f"{context} outputs", error_factory)

    operations: list[dict[str, object]] = []
    for index, item in enumerate(
        _array(
            surface["binding_operations"],
            f"{context} binding_operations",
            error_factory,
        )
    ):
        label = f"{context} binding_operations[{index}]"
        operation = _object(
            item,
            label,
            error_factory,
            fields=OPERATION_FIELDS,
        )
        prerequisites = _string_array(
            operation["compiled_prerequisites"],
            f"{label}.compiled_prerequisites",
            error_factory,
            require_sorted=require_sorted_compiled_prerequisites,
        )
        operations.append(
            {
                "id": _string(operation["id"], f"{label}.id", error_factory),
                "capability": _nullable_string(
                    operation["capability"],
                    f"{label}.capability",
                    error_factory,
                ),
                "output": _nullable_string(
                    operation["output"],
                    f"{label}.output",
                    error_factory,
                ),
                "compiled_prerequisites": prerequisites,
                "description": _string(
                    operation["description"],
                    f"{label}.description",
                    error_factory,
                ),
                "media_type": _string(
                    operation["media_type"],
                    f"{label}.media_type",
                    error_factory,
                ),
                "requires_uri": _boolean(
                    operation["requires_uri"],
                    f"{label}.requires_uri",
                    error_factory,
                ),
                "targets": _string_array(
                    operation["targets"],
                    f"{label}.targets",
                    error_factory,
                ),
            }
        )
    _unique_ids(operations, f"{context} binding_operations", error_factory)

    return {
        "schema_version": schema_version,
        "descriptor_id": descriptor_id,
        "targets": sorted(targets, key=lambda target: str(target["id"])),
        "capabilities": sorted(
            capabilities,
            key=lambda capability: str(capability["id"]),
        ),
        "outputs": sorted(outputs, key=lambda output: str(output["id"])),
        "binding_operations": sorted(
            operations,
            key=lambda operation: str(operation["id"]),
        ),
    }


def capability_surface_digest(surface: dict[str, object]) -> str:
    """Return the descriptor-owned semantic digest."""

    encoded = json.dumps(
        surface,
        ensure_ascii=False,
        separators=(",", ":"),
    ).encode("utf-8")
    return f"sha256:{hashlib.sha256(encoded).hexdigest()}"


def validate_capability_authority(
    profiles_descriptor: dict[str, object],
    capability_descriptor: object,
    *,
    expected_path: str,
    error_factory: ErrorFactory,
    profiles_context: str,
    capability_context: str,
    expected_schema_version: int | None = None,
    require_sorted_compiled_prerequisites: bool = False,
) -> dict[str, object]:
    """Validate a profile descriptor's exact authority reference and return its surface."""

    authority_label = f"{profiles_context} capability_authority"
    authority = _object(
        profiles_descriptor.get("capability_authority"),
        authority_label,
        error_factory,
        fields={"path", "schema_version", "digest"},
    )
    observed_path = _string(
        authority["path"],
        f"{authority_label}.path",
        error_factory,
    )
    if observed_path != expected_path:
        raise error_factory(
            f"{authority_label}.path must match the consumed descriptor path "
            f"{expected_path!r}; found {observed_path!r}"
        )

    canonical = canonical_capability_surface(
        capability_descriptor,
        error_factory=error_factory,
        context=capability_context,
        expected_schema_version=expected_schema_version,
        require_sorted_compiled_prerequisites=(
            require_sorted_compiled_prerequisites
        ),
    )
    observed_schema = _integer(
        authority["schema_version"],
        f"{authority_label}.schema_version",
        error_factory,
    )
    if observed_schema != canonical["schema_version"]:
        raise error_factory(
            f"{authority_label}.schema_version must match the consumed descriptor; "
            f"expected {canonical['schema_version']}, found {observed_schema}"
        )

    observed_digest = _string(
        authority["digest"],
        f"{authority_label}.digest",
        error_factory,
    )
    expected_digest = capability_surface_digest(canonical)
    if observed_digest != expected_digest:
        raise error_factory(
            f"{authority_label}.digest must match the consumed descriptor; "
            f"expected {expected_digest}, found {observed_digest}"
        )
    return canonical


def _object(
    value: object,
    label: str,
    error_factory: ErrorFactory,
    *,
    fields: set[str] | None = None,
) -> dict[str, object]:
    if type(value) is not dict:
        raise error_factory(f"{label} must be a JSON object")
    result = value
    if fields is not None:
        observed = set(result)
        if observed != fields:
            missing = sorted(fields - observed)
            extra = sorted(observed - fields)
            details = []
            if missing:
                details.append("missing fields " + ", ".join(missing))
            if extra:
                details.append("extra fields " + ", ".join(extra))
            raise error_factory(f"{label} has " + "; ".join(details))
    return result


def _array(
    value: object,
    label: str,
    error_factory: ErrorFactory,
) -> list[object]:
    if type(value) is not list:
        raise error_factory(f"{label} must be a JSON array")
    return value


def _string(value: object, label: str, error_factory: ErrorFactory) -> str:
    if type(value) is not str or not value or value.strip() != value:
        raise error_factory(f"{label} must be a non-empty, trimmed JSON string")
    return value


def _nullable_string(
    value: object,
    label: str,
    error_factory: ErrorFactory,
) -> str | None:
    if value is None:
        return None
    return _string(value, label, error_factory)


def _integer(value: object, label: str, error_factory: ErrorFactory) -> int:
    if type(value) is not int:
        raise error_factory(f"{label} must be a JSON integer")
    return value


def _boolean(value: object, label: str, error_factory: ErrorFactory) -> bool:
    if type(value) is not bool:
        raise error_factory(f"{label} must be a JSON boolean")
    return value


def _string_array(
    value: object,
    label: str,
    error_factory: ErrorFactory,
    *,
    require_sorted: bool = False,
) -> list[str]:
    values = _array(value, label, error_factory)
    result = [
        _string(item, f"{label}[{index}]", error_factory)
        for index, item in enumerate(values)
    ]
    if len(set(result)) != len(result):
        raise error_factory(f"{label} must contain unique strings")
    if require_sorted and result != sorted(result):
        raise error_factory(f"{label} must be sorted unique strings")
    return sorted(result)


def _unique_ids(
    values: list[dict[str, object]],
    label: str,
    error_factory: ErrorFactory,
) -> None:
    ids = [str(value["id"]) for value in values]
    if len(set(ids)) != len(ids):
        raise error_factory(f"{label} must contain unique IDs")
