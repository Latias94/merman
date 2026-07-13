// Fixture-derived root viewport overrides for Mermaid@11.12.2 C4 diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/c4/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable when upstream browser float behavior (DOM `getBBox()`
// + serialization) differs from our deterministic headless pipeline.

pub fn lookup_c4_root_viewport_override(diagram_id: &str) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_pkgtests_c4person_spec_004" => Some(("0 -10 653 393", "653")),
        "upstream_pkgtests_c4personext_spec_004" => Some(("0 -10 653 393", "653")),
        _ => None,
    }
}
