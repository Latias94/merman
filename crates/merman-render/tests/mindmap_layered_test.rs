#![cfg(all(feature = "diagram-mindmap", feature = "layout-layered"))]

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_render::{LayoutOptions, RenderCapability, environment::RenderEnvironment, family};
use serde_json::json;

#[test]
fn mindmap_layered_honors_the_root_layout_key_and_preserves_axis_routes() {
    const BODY: &str = "mindmap\n  root((Release))\n    Plan\n    Build\n      Test\n";
    let default_backend = if cfg!(feature = "layout-cytoscape") {
        RenderCapability::LayoutCytoscape
    } else {
        RenderCapability::LayoutDagre
    };
    for (site, frontmatter, backend) in [
        (json!({"mindmap":{"layout":"layered"}}), "", default_backend),
        (
            json!({}),
            "---\nconfig:\n  mindmap:\n    layout: layered\n---\n",
            default_backend,
        ),
        (
            json!({"layout":"layered"}),
            "",
            RenderCapability::LayoutLayered,
        ),
        (
            json!({"layout":"dagre"}),
            "---\nconfig:\n  layout: layered\n---\n",
            RenderCapability::LayoutLayered,
        ),
        (
            json!({}),
            "---\nconfig:\n  layout: dagre\n  mindmap:\n    layout: layered\n---\n",
            RenderCapability::LayoutLayered,
        ),
    ] {
        let engine =
            MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(site));
        let source = format!("{frontmatter}{BODY}");
        let parsed = engine
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let plan = family::plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.required_capabilities(), [backend]);
        if backend != RenderCapability::LayoutLayered {
            continue;
        }
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
        let svg = artifact
            .render_svg(&Default::default(), &Default::default())
            .unwrap();
        let doc = roxmltree::Document::parse(svg.svg()).unwrap();
        assert_eq!(doc.root_element().attribute("data-layout-fallback"), None);
        let paths = doc
            .descendants()
            .filter(|node| node.has_tag_name("path") && node.attribute("data-points").is_some());
        let mut count = 0;
        for path in paths {
            use base64::Engine as _;
            let points = base64::engine::general_purpose::STANDARD
                .decode(path.attribute("data-points").unwrap())
                .unwrap();
            let points: Vec<serde_json::Value> = serde_json::from_slice(&points).unwrap();
            assert!(points.len() >= 2);
            assert!(
                points
                    .iter()
                    .all(|point| matches!(point["axis"].as_str(), Some("h" | "v")))
            );
            count += 1;
        }
        assert_eq!(count, 3);
    }
}
