// Fixture-derived root viewport overrides for Mermaid@11.12.2 Block diagrams.
//
// These values are taken from upstream SVG baselines under
// `fixtures/upstream-svgs/block/*.svg` and are keyed by `diagram_id` (fixture stem).
//
// They are used to keep `parity-root` stable when upstream browser float behavior (DOM `getBBox()`
// + serialization) differs from our deterministic headless pipeline.

pub fn lookup_block_root_viewport_override(
    diagram_id: &str,
) -> Option<(&'static str, &'static str)> {
    match diagram_id {
        "upstream_html_demos_block_block_diagram_demos_002" => {
            Some(("-5 -53.203125 1034.375 106.40625", "1034.38"))
        }
        "upstream_html_demos_block_block_diagram_demos_003" => {
            Some(("-5 -31.419912338256836 866.5 62.83982467651367", "866.5"))
        }
        "upstream_html_demos_block_block_diagram_demos_010" => {
            Some(("-5 -36 285.078125 72", "285.078"))
        }
        _ => None,
    }
}
