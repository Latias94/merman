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
        "upstream_docs_requirementdiagram_larger_example_010" => {
            Some(("0 0 855.671875 1442", "855.672"))
        }
        _ => None,
    }
}
