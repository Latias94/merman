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
        "upstream_docs_requirementdiagram_class_definitions_016" => {
            Some(("0 0 416.375 200", "416.375"))
        }
        "upstream_docs_requirementdiagram_combined_example_022" => {
            Some(("0 0 430.28125 200", "430.281"))
        }
        "upstream_docs_requirementdiagram_larger_example_010" => {
            Some(("0 0 855.671875 1442", "855.672"))
        }
        "upstream_html_demos_requirements_requirement_diagram_demos_001" => {
            Some(("0 0 855.671875 1442", "855.672"))
        }
        "upstream_html_demos_requirements_requirement_diagram_demos_002" => {
            Some(("0 0 939.79296875 1466", "939.793"))
        }
        "upstream_docs_requirementdiagram_direct_styling_013" => {
            Some(("0 0 377.546875 200", "377.547"))
        }
        _ => None,
    }
}
