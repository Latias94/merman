#![cfg(feature = "diagram-er")]

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_model::presentation::PresentationDiagram;
use merman_render::environment::RenderEnvironment;
use merman_render::{Error, family};

const ER: &str = "erDiagram\nA ||--|| A : repeats\nA ||--o{ B : \"**owns**\"\nA ||..o{ B : serves\nA ||--|| C : \"\"\n";

fn parse(body: &str, source_layout: Option<&str>, site: serde_json::Value) -> PresentationDiagram {
    let source = match source_layout {
        Some(layout) => format!("---\nconfig:\n  layout: {layout}\n---\n{body}"),
        None => body.to_owned(),
    };
    MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(site))
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation()
}

#[cfg(not(feature = "layout-layered"))]
#[test]
fn explicit_er_layered_requires_the_provider() {
    let parsed = parse(ER, Some("layered"), serde_json::json!({}));
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    assert!(matches!(family::plan_render(&parsed, &session),
        Err(Error::UnsupportedLayoutBackend { backend: "layered", diagram_type }) if diagram_type == "er"));
}

#[cfg(feature = "layout-layered")]
mod layered {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use merman_render::{
        DiagramFamilyId, LayoutOptions, RenderCapability, RenderCapabilityPolicy,
        family::FamilyRenderArtifact, svg::SvgRenderOptions,
    };

    fn prepare(parsed: &PresentationDiagram) -> FamilyRenderArtifact<'_> {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        family::prepare(parsed, &LayoutOptions::default(), session).unwrap()
    }

    fn svg(artifact: FamilyRenderArtifact<'_>) -> String {
        artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("er-layered".into()),
                    ..Default::default()
                },
                &Default::default(),
            )
            .unwrap()
            .svg()
            .into()
    }

    #[test]
    fn er_layered_is_explicit_and_obeys_source_then_site_selection() {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        for (source, site) in [(None, "layered"), (Some("layered"), "dagre")] {
            let parsed = parse(ER, source, serde_json::json!({"layout":site}));
            let plan = family::plan_render(&parsed, &session).unwrap();
            assert_eq!(plan.family_id(), DiagramFamilyId::ER);
            assert_eq!(
                plan.required_capabilities(),
                [RenderCapability::LayoutLayered]
            );
        }
        let parsed = parse(ER, None, serde_json::json!({}));
        let plan = family::plan_render(&parsed, &session).unwrap();
        let expected = if cfg!(feature = "layout-elk") || !cfg!(feature = "layout-dagre") {
            RenderCapability::LayoutElk
        } else {
            RenderCapability::LayoutDagre
        };
        assert_eq!(plan.required_capabilities(), [expected]);
        let parsed = parse(ER, Some("layered"), serde_json::json!({}));
        let denied = RenderEnvironment::deterministic()
            .with_capability_policy(
                RenderCapabilityPolicy::unrestricted().with_denied(RenderCapability::LayoutLayered),
            )
            .begin_session()
            .unwrap();
        assert!(
            matches!(family::prepare(&parsed, &LayoutOptions::default(), denied),
            Err(Error::MissingCapability { capability: RenderCapability::LayoutLayered, diagram_type }) if diagram_type == "er")
        );
    }

    #[test]
    fn er_layered_retains_source_occurrences_crow_feet_and_markdown_terminals() {
        for html in [false, true] {
            let parsed = parse(
                ER,
                Some("layered"),
                serde_json::json!({"htmlLabels":html,"layered":{"edgeRouting":"rounded"}}),
            );
            let svg = svg(prepare(&parsed));
            let doc = roxmltree::Document::parse(&svg).unwrap();
            assert!(
                doc.root_element()
                    .attribute("data-layout-fallback")
                    .is_none()
            );
            let edges = doc
                .descendants()
                .filter(|node| node.attribute("data-edge") == Some("true"))
                .collect::<Vec<_>>();
            assert_eq!(edges.len(), 4);
            for (slot, (edge, target)) in edges
                .iter()
                .zip(["entity-A-0", "entity-B-1", "entity-B-1", "entity-C-2"])
                .enumerate()
            {
                assert_eq!(
                    edge.attribute("data-id").unwrap(),
                    format!("id_entity-A-0_{target}_{slot}")
                );
                assert_eq!(
                    edge.attribute("marker-start"),
                    Some("url(#er-layered_er-onlyOneStart)")
                );
                let points: Vec<serde_json::Value> = serde_json::from_slice(
                    &STANDARD
                        .decode(edge.attribute("data-points").unwrap())
                        .unwrap(),
                )
                .unwrap();
                assert!(
                    points.len() >= 2
                        && points
                            .iter()
                            .all(|point| matches!(point["axis"].as_str(), Some("h" | "v")))
                );
            }
            assert_eq!(
                edges[1].attribute("marker-end"),
                Some("url(#er-layered_er-zeroOrMoreEnd)")
            );
            assert!(
                edges[2]
                    .attribute("class")
                    .unwrap()
                    .contains("edge-pattern-dashed")
            );
            let groups = doc
                .descendants()
                .find(|node| node.attribute("class") == Some("edgeLabels"))
                .unwrap();
            let labels = groups
                .children()
                .filter(|node| node.is_element())
                .collect::<Vec<_>>();
            assert_eq!(labels.len(), 3);
            assert_eq!(
                labels[1]
                    .descendants()
                    .any(|node| node.has_tag_name("foreignObject")),
                html
            );
            assert!(
                labels[1]
                    .descendants()
                    .any(|node| node.is_text() && node.text() == Some("owns"))
            );
        }
    }

    const NESTED: &str = "erDiagram\ndirection LR\nsubgraph Outer[Accounting desk]\nsubgraph Inner[Ledger team]\nA\nend\nB\nend\nA ||--|| B : owns\n";

    #[test]
    fn er_layered_compound_shells_paint_parent_first() {
        let parsed = parse(
            NESTED,
            Some("layered"),
            serde_json::json!({"htmlLabels":false,"flowchart":{"subGraphTitleMargin":{"top":7,"bottom":3}}}),
        );
        let svg = svg(prepare(&parsed));
        let dom = roxmltree::Document::parse(&svg).unwrap();
        let groups = dom
            .descendants()
            .find(|node| node.attribute("class") == Some("clusters"))
            .unwrap();
        let ids = groups
            .children()
            .filter_map(|node| node.attribute("id"))
            .collect::<Vec<_>>();
        assert_eq!(ids, ["er-layered-Outer", "er-layered-Inner"]);
    }

    #[test]
    fn er_layered_recoverable_failure_obeys_dagre_admission() {
        let site = serde_json::json!({"htmlLabels":false,"flowchart":{"subGraphTitleMargin":{"top":1.7e308,"bottom":1.7e308}}});
        let source = parse(NESTED, Some("layered"), site.clone());
        let environment = RenderEnvironment::deterministic();
        let denied = environment
            .clone()
            .with_capability_policy(
                RenderCapabilityPolicy::unrestricted().with_denied(RenderCapability::LayoutDagre),
            )
            .begin_session()
            .unwrap();
        assert!(
            matches!(family::prepare(&source, &LayoutOptions::default(), denied),
            Err(Error::MissingCapability { capability: RenderCapability::LayoutDagre, diagram_type }) if diagram_type == "er")
        );

        if cfg!(feature = "layout-dagre") {
            let direct = parse(NESTED, Some("dagre"), site);
            let fallback = svg(prepare(&source));
            assert_eq!(
                roxmltree::Document::parse(&fallback)
                    .unwrap()
                    .root_element()
                    .attribute("data-layout-fallback"),
                Some("dagre")
            );
            assert_eq!(
                fallback.replacen(" data-layout-fallback=\"dagre\"", "", 1),
                svg(prepare(&direct))
            );
        } else {
            let session = environment.begin_session().unwrap();
            assert!(
                matches!(family::prepare(&source, &LayoutOptions::default(), session),
                Err(Error::MissingCapability { capability: RenderCapability::LayoutDagre, diagram_type }) if diagram_type == "er")
            );
        }
    }
}
