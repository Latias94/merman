"""Validate unqualified production preset discovery without issuing qualification."""

from __future__ import annotations


def validate_unqualified_catalog(catalog: object) -> None:
    """Check production discovery shape without granting any execution qualification."""
    if not isinstance(catalog, dict) or set(catalog) != {"schema_version", "presets"}:
        raise RuntimeError("Invalid theme preset catalog fields")
    if type(catalog["schema_version"]) is not int or catalog["schema_version"] != 1:
        raise RuntimeError("Unsupported theme preset catalog schema")
    entries = catalog["presets"]
    if not isinstance(entries, list) or not entries:
        raise RuntimeError("Missing theme preset catalog entries")
    strings = {"id", "display_name", "appearance", "maturity", "license_expression", "export_kind"}
    fields = strings | {"available", "availability_reason_ids", "family_designs", "qualified_cells", "required_attribution"}
    seen = set()
    for entry in entries:
        if not isinstance(entry, dict) or set(entry) != fields:
            raise RuntimeError("Invalid theme preset catalog entry fields")
        if any(not isinstance(entry[key], str) or not entry[key] for key in strings):
            raise RuntimeError("Invalid theme preset catalog identifiers")
        designs = entry["family_designs"]
        if (not isinstance(designs, list)
                or any(not isinstance(design, dict)
                       or set(design) != {"family_id", "treatment"}
                       or any(not isinstance(value, str) or not value for value in design.values())
                       for design in designs)):
            raise RuntimeError("Invalid theme preset family designs")
        if len({design["family_id"] for design in designs}) != len(designs):
            raise RuntimeError("Duplicate family in theme preset designs")
        reasons = entry["availability_reason_ids"]
        if (type(entry["available"]) is not bool or not isinstance(reasons, list)
                or any(not isinstance(reason, str) or not reason for reason in reasons)
                or entry["available"] != (not reasons)):
            raise RuntimeError("Invalid theme preset catalog availability")
        if entry["required_attribution"] is not None and not isinstance(entry["required_attribution"], str):
            raise RuntimeError("Invalid theme preset catalog attribution")
        if entry["maturity"] != "alpha" or entry["qualified_cells"] != []:
            raise RuntimeError("Unqualified catalog cannot advertise stable maturity or qualified cells")
        if entry["id"] in seen:
            raise RuntimeError("Duplicate preset in theme catalog")
        seen.add(entry["id"])
