#![cfg(feature = "diagram-mindmap")]

use merman_core::{Engine, MermaidConfig, ParseOptions};
use serde_json::json;

#[test]
fn mindmap_compatibility_projection_retains_the_final_layout_selection() {
    for (site, prefix, expected) in [
        (json!({}), "", json!("cose-bilkent")),
        (json!({"layout": "dagre"}), "", json!("dagre")),
        (json!({"mindmap": {"layout": "elk"}}), "", json!("elk")),
        (
            json!({"layout": "elk"}),
            "---\nconfig:\n  layout: dagre\n---\n",
            json!("dagre"),
        ),
        (
            json!({"layout": "elk"}),
            "---\nconfig:\n  layout: null\n---\n",
            json!("elk"),
        ),
    ] {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(site));
        let source = format!("{prefix}mindmap\nroot\n  child");
        let parsed = engine
            .parse_diagram_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.meta.effective_config.as_value()["layout"], expected);
        assert_eq!(parsed.model["config"]["layout"], expected);
    }
}
