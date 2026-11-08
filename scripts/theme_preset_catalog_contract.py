"""Validate unqualified production preset discovery without issuing qualification."""

from __future__ import annotations


def validate_unqualified_catalog(catalog: object) -> None:
    """Check preset discovery; the producer owns unrelated catalog envelope metadata."""
    if not isinstance(catalog, dict) or not {"schema_version", "presets"} <= set(catalog):
        raise RuntimeError("Invalid theme preset catalog fields")
    if type(catalog["schema_version"]) is not int or catalog["schema_version"] != 1:
        raise RuntimeError("Unsupported theme preset catalog schema")
    entries = catalog["presets"]
    if not isinstance(entries, list) or not entries:
        raise RuntimeError("Missing theme preset catalog entries")
    strings = {"id", "display_name", "maturity", "license_expression", "export_kind"}
    fields = strings | {
        "supported_appearances", "default_appearance", "appearances",
        "available", "availability_reason_ids", "required_attribution",
    }
    appearance_fields = {
        "appearance", "concrete_preset_id", "available", "availability_reason_ids",
        "family_designs", "preview_colors", "application_notes", "qualified_cells",
    }
    seen = set()
    concrete_ids = set()
    for entry in entries:
        if not isinstance(entry, dict) or set(entry) != fields:
            raise RuntimeError("Invalid theme preset catalog entry fields")
        if any(not isinstance(entry[key], str) or not entry[key] for key in strings):
            raise RuntimeError("Invalid theme preset catalog identifiers")
        _validate_availability(entry)
        if entry["required_attribution"] is not None and not isinstance(entry["required_attribution"], str):
            raise RuntimeError("Invalid theme preset catalog attribution")
        if entry["maturity"] != "alpha":
            raise RuntimeError("Unqualified catalog cannot advertise stable maturity or qualified cells")
        if entry["id"] in seen:
            raise RuntimeError("Duplicate preset in theme catalog")
        seen.add(entry["id"])
        supported = entry["supported_appearances"]
        if (not isinstance(supported, list) or not supported
                or any(appearance not in ("light", "dark") for appearance in supported)
                or len(set(supported)) != len(supported)
                or entry["default_appearance"] not in supported):
            raise RuntimeError("Invalid theme preset supported/default appearances")
        appearances = entry["appearances"]
        if not isinstance(appearances, list) or len(appearances) != len(supported):
            raise RuntimeError("Invalid theme preset appearance entries")
        for appearance, expected in zip(appearances, supported):
            if not isinstance(appearance, dict) or set(appearance) != appearance_fields:
                raise RuntimeError("Invalid theme preset appearance entry fields")
            if appearance["appearance"] != expected:
                raise RuntimeError("Inconsistent theme preset appearance entries")
            concrete_id = appearance["concrete_preset_id"]
            if not isinstance(concrete_id, str) or not concrete_id:
                raise RuntimeError("Invalid concrete theme preset identifier")
            if concrete_id in concrete_ids:
                raise RuntimeError("Duplicate concrete preset in theme catalog")
            concrete_ids.add(concrete_id)
            _validate_availability(appearance)
            if (appearance["available"] != entry["available"]
                    or appearance["availability_reason_ids"] != entry["availability_reason_ids"]):
                raise RuntimeError("Inconsistent whole-theme appearance availability")
            designs = appearance["family_designs"]
            if (not isinstance(designs, list)
                    or any(not isinstance(design, dict)
                           or set(design) != {"family_id", "treatment"}
                           or any(not isinstance(value, str) or not value for value in design.values())
                           for design in designs)):
                raise RuntimeError("Invalid theme preset family designs")
            if len({design["family_id"] for design in designs}) != len(designs):
                raise RuntimeError("Duplicate family in theme preset designs")
            colors = appearance["preview_colors"]
            if not isinstance(colors, list) or any(not isinstance(color, str) or not color for color in colors):
                raise RuntimeError("Invalid theme preset preview colors")
            notes = appearance["application_notes"]
            if (not isinstance(notes, list)
                    or any(not isinstance(note, dict)
                           or set(note) != {"id", "family_id", "message", "recommended_config"}
                           or any(not isinstance(note[key], str) or not note[key] for key in ("id", "family_id", "message"))
                           or (note["recommended_config"] is not None and not isinstance(note["recommended_config"], dict))
                           for note in notes)):
                raise RuntimeError("Invalid theme preset application notes")
            if len({(note["family_id"], note["id"]) for note in notes}) != len(notes):
                raise RuntimeError("Duplicate theme preset application note")
            if appearance["qualified_cells"] != []:
                raise RuntimeError("Unqualified catalog cannot advertise stable maturity or qualified cells")


def _validate_availability(entry: dict) -> None:
    reasons = entry["availability_reason_ids"]
    if (type(entry["available"]) is not bool or not isinstance(reasons, list)
            or any(not isinstance(reason, str) or not reason for reason in reasons)
            or entry["available"] != (not reasons)):
        raise RuntimeError("Invalid theme preset catalog availability")
