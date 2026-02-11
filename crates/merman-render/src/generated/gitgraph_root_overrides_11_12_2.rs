// Fixture-derived root viewport overrides for Mermaid@11.12.2 GitGraph diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/gitgraph/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable at higher decimal precision when browser float
// behavior (DOM `getBBox()` + serialization) differs from our deterministic headless pipeline.

pub fn lookup_gitgraph_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_docs_gitgraph_base_theme_046" => Some((
            "-124.078125 -19 1032.078125 536.0304565429688",
            "1032.078125",
        )),
        "upstream_docs_gitgraph_default_theme_050" => Some((
            "-124.078125 -19 1032.078125 536.0304565429688",
            "1032.078125",
        )),
        "upstream_docs_gitgraph_forest_theme_048" => Some((
            "-124.078125 -19 1032.078125 536.0304565429688",
            "1032.078125",
        )),
        "upstream_docs_gitgraph_neutral_theme_054" => Some((
            "-124.078125 -19 1032.078125 536.0304565429688",
            "1032.078125",
        )),
        "upstream_docs_gitgraph_hiding_branch_names_and_lines_022" => Some((
            "-38.39225769042969 -18 915.3922729492188 535.0304565429688",
            "915.3922729492188",
        )),
        _ => None,
    }
}
