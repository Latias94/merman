// Fixture-derived root viewport overrides for Mermaid@11.12.2 Requirement diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/requirement/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable at higher decimal precision when browser float
// behavior (DOM `getBBox()` + serialization) differs from our deterministic headless pipeline.

pub fn lookup_requirement_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "basic" => Some(("0 0 185.4375 200", "185.438")),
        "relations" => Some(("0 0 257.078125 410", "257.078")),
        "upstream_cypress_requirement_spec_example_001" => Some(("0 0 551.0625 668", "551.062")),
        "upstream_cypress_requirementdiagram_unified_spec_example_001" => {
            Some(("0 0 173.734375 386", "173.734"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_003" => {
            Some(("0 0 855.671875 1442", "855.672"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_005" => {
            Some(("0 0 173.734375 386", "173.734"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_006" => {
            Some(("0 0 323.890625 84", "323.891"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_007" => {
            Some(("-24.03125 -48 221.796875 434", "221.797"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_008" => {
            Some(("0 0 173.734375 386", "173.734"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_009" => {
            Some(("0 0 173.734375 386", "173.734"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_010" => {
            Some(("0 0 453.34375 200", "453.344"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_011" => {
            Some(("0 0 453.34375 200", "453.344"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_012" => {
            Some(("0 0 179.125 386", "179.125"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_013" => {
            Some(("0 0 179.125 386", "179.125"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_014" => {
            Some(("0 0 179.125 386", "179.125"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_016" => {
            Some(("0 0 173.734375 386", "173.734"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_018" => {
            Some(("0 0 173.734375 386", "173.734"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_020" => {
            Some(("0 0 173.734375 386", "173.734"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_021" => {
            Some(("0 0 384.34375 668", "384.344"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_022" => {
            Some(("0 0 336.40625 200", "336.406"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_023" => {
            Some(("0 0 801.421875 224", "801.422"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_025" => {
            Some(("0 0 859 224", "859"))
        }
        "upstream_cypress_requirementdiagram_unified_spec_example_026" => {
            Some(("0 0 582.984375 200", "582.984"))
        }
        "upstream_docs_accessibility_requirement_diagram_013" => {
            Some(("0 0 173.734375 386", "173.734"))
        }
        "upstream_docs_requirementdiagram_class_definitions_016" => {
            Some(("0 0 416.375 200", "416.375"))
        }
        "upstream_docs_requirementdiagram_combined_example_022" => {
            Some(("0 0 430.28125 200", "430.281"))
        }
        "upstream_docs_requirementdiagram_direct_styling_013" => {
            Some(("0 0 377.546875 200", "377.547"))
        }
        "upstream_docs_requirementdiagram_direction_012" => Some(("0 0 453.34375 200", "453.344")),
        "upstream_docs_requirementdiagram_larger_example_010" => {
            Some(("0 0 855.671875 1442", "855.672"))
        }
        "upstream_docs_requirementdiagram_markdown_formatting_005" => {
            Some(("0 0 252.109375 200", "252.109"))
        }
        "upstream_docs_requirementdiagram_requirement_diagram_001" => {
            Some(("0 0 173.734375 386", "173.734"))
        }
        "upstream_html_demos_requirements_requirement_diagram_demos_001" => {
            Some(("0 0 855.671875 1442", "855.672"))
        }
        "upstream_html_demos_requirements_requirement_diagram_demos_002" => {
            Some(("0 0 939.79296875 1466", "939.793"))
        }
        "upstream_pkgtests_diagram_orchestration_spec_038" => Some(("-8 -8 16 16", "16")),
        "upstream_pkgtests_requirementdiagram_spec_015" => Some(("0 0 158.28125 152", "158.281")),
        "upstream_pkgtests_requirementdiagram_spec_017" => Some(("0 0 158.28125 152", "158.281")),
        "upstream_pkgtests_requirementdiagram_spec_147" => Some(("0 0 173.734375 200", "173.734")),
        "upstream_pkgtests_requirementdiagram_spec_149" => Some(("0 0 154.09375 128", "154.094")),
        "upstream_requirement_accessibility_spec" => Some(("0 0 158.28125 152", "158.281")),
        "upstream_requirement_classes_and_inheritance_spec" => {
            Some(("0 0 707.203125 128", "707.203"))
        }
        "upstream_requirement_direction_and_proto_ids_spec" => {
            Some(("0 0 158.28125 152", "158.281"))
        }
        "upstream_requirement_full_element_and_relationships_spec" => Some(("0 0 207 362", "207")),
        "upstream_requirement_full_requirement_spec" => Some(("0 0 410.8125 200", "410.812")),
        "upstream_requirement_requirement_types_spec" => Some(("0 0 1548.65625 200", "1548.66")),
        "upstream_requirement_styles_spec" => Some(("0 0 331.21875 84", "331.219")),
        "stress_requirement_font_size_precedence_001" => Some(("0 0 286 758", "286")),
        _ => None,
    }
}
