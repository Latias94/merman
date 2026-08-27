"""Versioned agent-facing ASCII capability contract shared by release verifiers."""

from __future__ import annotations


ASCII_CAPABILITY_FIELDS = {
    "schema_version",
    "output_schema_version",
    "report",
    "families",
    "detected_type_mappings",
}
ASCII_FAMILY_FIELDS = {
    "family",
    "display_name",
    "semantic_coverage",
    "primary_projection",
    "structured_text_fallback",
    "support_level",
    "layout_profiles",
    "width_profiles",
    "encodings",
    "fallback_encodings",
}
ASCII_MAPPING_FIELDS = {"detected_type", "family"}
ASCII_REPORT = {
    "success_schema_version": 2,
    "error_schema_version": 1,
    "encoding": "plain",
    "styled_output": False,
    "success_stream": "output",
    "error_stream": "stderr",
}
ASCII_ENCODINGS = ["plain", "ansi16", "ansi256", "truecolor", "html"]
ASCII_WIDTH_PROFILES = ["unicode", "cjk"]

ASCII_DIAGRAMMATIC_FAMILIES = {
    "class",
    "er",
    "flowchart",
    "sequence",
    "state",
    "xychart",
}
ASCII_SUMMARY_FAMILIES = {
    "gantt",
    "gitgraph",
    "journey",
    "kanban",
    "mindmap",
    "packet",
    "timeline",
    "treeView",
}
ASCII_COMPACT_FAMILIES = {"flowchart", "sequence"}
ASCII_FAMILIES = (
    "architecture",
    "block",
    "c4",
    "class",
    "cynefin",
    "er",
    "eventmodeling",
    "flowchart",
    "gantt",
    "gitgraph",
    "info",
    "ishikawa",
    "journey",
    "kanban",
    "mindmap",
    "packet",
    "pie",
    "quadrantchart",
    "radar",
    "railroad",
    "requirement",
    "sankey",
    "sequence",
    "state",
    "timeline",
    "treeView",
    "treemap",
    "venn",
    "wardley",
    "xychart",
    "zenuml",
)
ASCII_DETECTED_TYPE_MAPPINGS = (
    ("architecture", "architecture"),
    ("block", "block"),
    ("c4", "c4"),
    ("class", "class"),
    ("classDiagram", "class"),
    ("cynefin", "cynefin"),
    ("er", "er"),
    ("eventmodeling", "eventmodeling"),
    ("flowchart", "flowchart"),
    ("flowchart-elk", "flowchart"),
    ("flowchart-v2", "flowchart"),
    ("gantt", "gantt"),
    ("gitGraph", "gitgraph"),
    ("info", "info"),
    ("ishikawa", "ishikawa"),
    ("journey", "journey"),
    ("kanban", "kanban"),
    ("mindmap", "mindmap"),
    ("packet", "packet"),
    ("pie", "pie"),
    ("quadrantChart", "quadrantchart"),
    ("radar", "radar"),
    ("railroad", "railroad"),
    ("railroadAbnf", "railroad"),
    ("railroadEbnf", "railroad"),
    ("railroadPeg", "railroad"),
    ("requirement", "requirement"),
    ("sankey", "sankey"),
    ("sequence", "sequence"),
    ("state", "state"),
    ("stateDiagram", "state"),
    ("swimlane", "flowchart"),
    ("timeline", "timeline"),
    ("treeView", "treeView"),
    ("treemap", "treemap"),
    ("venn", "venn"),
    ("wardley", "wardley"),
    ("xychart", "xychart"),
    ("zenuml", "zenuml"),
)


class AsciiCapabilityContractError(ValueError):
    """Raised when a runtime ASCII capability document drifts from schema 1."""


def _expected_family_contract(family: str) -> dict[str, object]:
    if family in ASCII_DIAGRAMMATIC_FAMILIES:
        projection = "diagrammatic"
        support_level = "partial"
    elif family in ASCII_SUMMARY_FAMILIES:
        projection = "structured_text"
        support_level = "summary"
    else:
        return {
            "semantic_coverage": None,
            "primary_projection": "none",
            "structured_text_fallback": False,
            "support_level": "unsupported",
            "layout_profiles": [],
            "width_profiles": [],
            "encodings": [],
            "fallback_encodings": [],
        }
    layouts = ["canonical"]
    if family in ASCII_COMPACT_FAMILIES:
        layouts.append("compact")
    return {
        "semantic_coverage": "partial",
        "primary_projection": projection,
        "structured_text_fallback": True,
        "support_level": support_level,
        "layout_profiles": layouts,
        "width_profiles": ASCII_WIDTH_PROFILES,
        "encodings": ASCII_ENCODINGS,
        "fallback_encodings": ["plain"],
    }


def canonical_ascii_capabilities() -> dict[str, object]:
    """Build a valid fixture without duplicating the public contract in tests."""

    families = []
    for family in ASCII_FAMILIES:
        families.append(
            {
                "family": family,
                "display_name": family,
                **_expected_family_contract(family),
            }
        )
    return {
        "schema_version": 1,
        "output_schema_version": 2,
        "report": dict(ASCII_REPORT),
        "families": families,
        "detected_type_mappings": [
            {"detected_type": detected_type, "family": family}
            for detected_type, family in ASCII_DETECTED_TYPE_MAPPINGS
        ],
    }


def validate_ascii_capabilities(value: object) -> None:
    if not isinstance(value, dict):
        raise AsciiCapabilityContractError("ASCII subcontract is missing")
    if set(value) != ASCII_CAPABILITY_FIELDS:
        raise AsciiCapabilityContractError("ASCII subcontract fields drifted")
    if type(value["schema_version"]) is not int or value["schema_version"] != 1:
        raise AsciiCapabilityContractError("ASCII schema version drifted")
    if (
        type(value["output_schema_version"]) is not int
        or value["output_schema_version"] != 2
    ):
        raise AsciiCapabilityContractError("ASCII output schema version drifted")
    if value["report"] != ASCII_REPORT:
        raise AsciiCapabilityContractError("ASCII report contract drifted")

    families = value["families"]
    if not isinstance(families, list) or not all(
        isinstance(family, dict) for family in families
    ):
        raise AsciiCapabilityContractError("ASCII families must be objects")
    family_ids = [family.get("family") for family in families]
    if family_ids != list(ASCII_FAMILIES):
        raise AsciiCapabilityContractError("ASCII family ids or ordering drifted")
    for family in families:
        family_id = family["family"]
        if set(family) != ASCII_FAMILY_FIELDS:
            raise AsciiCapabilityContractError(
                f"ASCII {family_id} capability fields drifted"
            )
        display_name = family["display_name"]
        if not isinstance(display_name, str) or not display_name:
            raise AsciiCapabilityContractError(
                f"ASCII {family_id} display name is invalid"
            )
        expected = _expected_family_contract(family_id)
        observed = {key: family[key] for key in expected}
        if observed != expected:
            raise AsciiCapabilityContractError(
                f"ASCII {family_id} capability contract drifted"
            )

    mappings = value["detected_type_mappings"]
    if not isinstance(mappings, list) or not all(
        isinstance(mapping, dict) for mapping in mappings
    ):
        raise AsciiCapabilityContractError("ASCII detector mappings must be objects")
    if any(set(mapping) != ASCII_MAPPING_FIELDS for mapping in mappings):
        raise AsciiCapabilityContractError("ASCII detector mapping fields drifted")
    observed_mappings = [
        (mapping["detected_type"], mapping["family"]) for mapping in mappings
    ]
    if observed_mappings != list(ASCII_DETECTED_TYPE_MAPPINGS):
        raise AsciiCapabilityContractError(
            "ASCII detector mappings or ordering drifted"
        )
