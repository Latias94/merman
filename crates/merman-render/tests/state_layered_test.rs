#![cfg(feature = "diagram-state")]

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_model::presentation::PresentationDiagram;
use merman_render::{
    DiagramFamilyId, Error, RenderCapability, environment::RenderEnvironment, family,
};
use serde_json::{Value, json};

// Mermaid 12.4.0 e2e/diagrams/state-diagram-v2/pr-check/layout-layered-state.mmd.
const STATE: &str = "stateDiagram-v2\n\
  state Checking <<choice>>\n\
  state Split <<fork>>\n\
  state Merge <<join>>\n\
  [*] --> Idle\n\
  Idle --> Running : start\n\
  state Running {\n\
    [*] --> Working\n\
    Working --> Paused : pause\n\
    Paused --> Working : resume\n\
  }\n\
  Running --> Checking : finish\n\
  Checking --> Split : ok\n\
  Checking --> Idle : retry\n\
  Split --> Archive\n\
  Split --> Notify\n\
  Archive --> Merge\n\
  Notify --> Merge\n\
  Merge --> [*]\n\
  note right of Idle : waiting for input\n";

fn parse(body: &str, source_layout: Option<&str>, site: Value) -> PresentationDiagram {
    let source = source_layout.map_or_else(
        || body.to_owned(),
        |layout| format!("---\nconfig:\n  layout: {layout}\n---\n{body}"),
    );
    let diagram = MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(site))
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    assert_eq!(diagram.family_id(), Some(DiagramFamilyId::STATE));
    assert_eq!(diagram.metadata().diagram_type, "stateDiagram");
    diagram
}

#[test]
fn state_layered_preserves_default_dagre_and_elk_selection() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let default =
        if merman_render::layout_elk_available() || !merman_render::layout_dagre_available() {
            RenderCapability::LayoutElk
        } else {
            RenderCapability::LayoutDagre
        };
    for (layout, capability) in [
        (None, default),
        (Some("elk"), default),
        (Some("dagre"), RenderCapability::LayoutDagre),
    ] {
        let parsed = parse(STATE, layout, json!({}));
        assert_eq!(
            family::plan_render(&parsed, &session)
                .unwrap()
                .required_capabilities(),
            [capability]
        );
    }
}

#[cfg(not(feature = "layout-layered"))]
#[test]
fn explicit_state_layered_requires_the_compiled_provider() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    assert!(
        matches!(family::plan_render(&parse(STATE, Some("layered"), json!({})), &session),
        Err(Error::UnsupportedLayoutBackend { backend: "layered", diagram_type }) if diagram_type == "stateDiagram")
    );
}

#[cfg(feature = "layout-layered")]
mod layered {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use merman_render::{
        LayoutOptions, RenderCapabilityPolicy, family::FamilyRenderArtifact,
        model::StateDiagramLayout, svg::SvgRenderOptions,
    };
    use roxmltree::{Document, Node};

    fn prepare(diagram: &PresentationDiagram) -> FamilyRenderArtifact<'_> {
        family::prepare(
            diagram,
            &LayoutOptions::default(),
            RenderEnvironment::deterministic().begin_session().unwrap(),
        )
        .unwrap()
    }

    fn svg(artifact: FamilyRenderArtifact<'_>) -> String {
        let rendered = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("state-layered".into()),
                    ..Default::default()
                },
                &Default::default(),
            )
            .unwrap();
        assert_eq!(rendered.family_id(), DiagramFamilyId::STATE);
        rendered.svg().into()
    }

    fn layout(artifact: &FamilyRenderArtifact<'_>) -> StateDiagramLayout {
        let output = artifact.layout_geometry_json().unwrap();
        assert_eq!(output["family_id"], "state");
        assert_eq!(output["diagram_type"], "stateDiagram");
        serde_json::from_value(output["layout"]["StateDiagramV2"].clone()).unwrap()
    }

    fn paths<'a, 'input>(doc: &'a Document<'input>) -> impl Iterator<Item = Node<'a, 'input>> {
        doc.descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
    }

    fn points(path: Node<'_, '_>) -> Vec<Value> {
        serde_json::from_slice(
            &STANDARD
                .decode(path.attribute("data-points").unwrap())
                .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn explicit_state_layered_respects_source_site_and_capability_policy() {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        for (source, site) in [(None, "layered"), (Some("layered"), "dagre")] {
            let parsed = parse(STATE, source, json!({"layout":site}));
            let plan = family::plan_render(&parsed, &session).unwrap();
            assert_eq!(
                plan.required_capabilities(),
                [RenderCapability::LayoutLayered]
            );
            assert!(plan.is_ready());
        }
        let parsed = parse(STATE, Some("layered"), json!({}));
        let denied = RenderEnvironment::deterministic()
            .with_capability_policy(
                RenderCapabilityPolicy::unrestricted().with_denied(RenderCapability::LayoutLayered),
            )
            .begin_session()
            .unwrap();
        assert!(
            matches!(family::prepare(&parsed, &LayoutOptions::default(), denied),
            Err(Error::MissingCapability { capability: RenderCapability::LayoutLayered, diagram_type }) if diagram_type == "stateDiagram")
        );
    }

    #[test]
    fn pinned_state_layered_keeps_choice_contacts_source_slots_and_note_markers() {
        let parsed = parse(
            STATE,
            Some("layered"),
            json!({"look":"classic","htmlLabels":false}),
        );
        let artifact = prepare(&parsed);
        let layout = layout(&artifact);
        assert_eq!(layout.edges.len(), 14);
        let node = |id| layout.nodes.iter().find(|node| node.id == id).unwrap();
        assert_eq!((node("Split").width, node("Split").height), (70.0, 10.0));
        let checking = node("Checking");
        let start = node("root_start");
        let result = svg(artifact);
        let doc = Document::parse(&result).unwrap();
        assert_eq!(
            doc.root_element().attribute("aria-roledescription"),
            Some("stateDiagram")
        );
        assert_eq!(doc.root_element().attribute("data-layout-fallback"), None);
        let mut roots = doc
            .descendants()
            .filter(|node| node.attribute("class") == Some("root"));
        let root = roots.next().unwrap();
        assert!(roots.next().is_none());
        let layers = root
            .children()
            .filter_map(|node| node.attribute("class"))
            .collect::<Vec<_>>();
        assert_eq!(
            layers,
            ["clusters", "edges edgePaths", "edgeLabels", "nodes"]
        );
        let paths = paths(&doc).collect::<Vec<_>>();
        for (path, route) in paths.iter().zip(&layout.edges) {
            assert_eq!(path.attribute("data-id"), Some(route.id.as_str()));
            assert!(
                points(*path)
                    .iter()
                    .all(|point| matches!(point["axis"].as_str(), Some("h" | "v")))
            );
        }
        let start_contact = points(paths[0]).remove(0);
        let delta = (
            start_contact["x"].as_f64().unwrap() - start.x,
            start_contact["y"].as_f64().unwrap() - start.y,
        );
        assert!((delta.0.hypot(delta.1) - 7.0).abs() < 1e-6);
        let contact = points(paths[5]).pop().unwrap();
        let distance = (contact["x"].as_f64().unwrap() - checking.x).abs()
            + (contact["y"].as_f64().unwrap() - checking.y).abs();
        assert!((distance - 14.0).abs() < 1e-6);
        assert_eq!(
            paths[5].attribute("marker-end"),
            Some("url(#state-layered_stateDiagram-barbEnd)")
        );
        let note = paths.last().unwrap();
        assert_eq!(note.attribute("marker-end"), None);
        assert!(
            note.attribute("class")
                .unwrap()
                .contains("edge-pattern-dashed")
        );
        assert!(
            doc.descendants()
                .any(|node| node.attribute("class") == Some("note-cluster"))
        );
    }

    const NESTED: &str = r#"stateDiagram-v2
direction RL
state Outer {
    direction LR
    state Branch <<fork>>
    [*] --> Branch
    Branch --> A
    state Inner {
        direction BT
        state Up <<join>>
        Up --> C
    }
    A --> A: **again**
    A --> A: twice
    note right of A: first note
    note left of A: second note
}
Outer --> A: descendant
Outer --> [*]
"#;

    #[test]
    fn state_layered_uses_existing_markdown_labels_and_keeps_both_loop_owners() {
        for html in [false, true] {
            let parsed = parse(
                NESTED,
                Some("layered"),
                json!({"look":"neo","htmlLabels":html,"flowchart":{"subGraphTitleMargin":{"top":7,"bottom":3}}}),
            );
            let artifact = prepare(&parsed);
            let layout = layout(&artifact);
            let group = |id| layout.clusters.iter().find(|group| group.id == id).unwrap();
            assert_eq!(group("Outer").requested_dir.as_deref(), Some("LR"));
            assert_eq!(group("Inner").requested_dir.as_deref(), Some("BT"));
            let dims = |id| {
                let node = layout.nodes.iter().find(|node| node.id == id).unwrap();
                (node.width, node.height)
            };
            assert_eq!(dims("Branch"), (10.0, 70.0));
            assert_eq!(dims("Up"), (70.0, 10.0));
            let loop_ids: Vec<_> = layout
                .edges
                .iter()
                .filter(|edge| edge.from == "A" && edge.to == "A")
                .map(|edge| edge.id.as_str())
                .collect();
            assert_eq!(loop_ids.len(), 2);
            let result = svg(artifact);
            let doc = Document::parse(&result).unwrap();
            let clusters = doc
                .descendants()
                .find(|node| node.attribute("class") == Some("clusters"))
                .unwrap();
            let order = clusters
                .children()
                .filter(Node::is_element)
                .map(|node| node.attribute("data-id").unwrap_or("note"))
                .collect::<Vec<_>>();
            assert_eq!(order, ["Outer", "note", "Inner"]);
            let labels = doc
                .descendants()
                .find(|node| node.attribute("class") == Some("edgeLabels"))
                .unwrap();
            for (id, text) in loop_ids.into_iter().zip(["again", "twice"]) {
                let label = labels
                    .children()
                    .find(|label| {
                        label
                            .descendants()
                            .any(|node| node.attribute("data-id") == Some(id))
                    })
                    .unwrap();
                assert!(label.descendants().any(|node| node.text() == Some(text)));
                assert_eq!(
                    label
                        .descendants()
                        .any(|node| node.has_tag_name("foreignObject")),
                    html
                );
                assert!(
                    !label
                        .descendants()
                        .any(|node| node.text() == Some("**again**"))
                );
                assert_eq!(
                    paths(&doc)
                        .find(|path| path.attribute("data-id") == Some(id))
                        .unwrap()
                        .attribute("marker-end"),
                    Some("url(#state-layered_stateDiagram-barbEnd-margin)")
                );
            }
        }
    }

    #[test]
    fn state_layered_retries_only_with_dagre_capability_and_fresh_state() {
        let site = json!({"look":"classic","htmlLabels":false,"flowchart":{"subGraphTitleMargin":{"top":1.7e308,"bottom":1.7e308}}});
        let parsed = parse(NESTED, Some("layered"), site.clone());
        let denied = RenderEnvironment::deterministic()
            .with_capability_policy(
                RenderCapabilityPolicy::unrestricted().with_denied(RenderCapability::LayoutDagre),
            )
            .begin_session()
            .unwrap();
        assert!(
            matches!(family::prepare(&parsed, &LayoutOptions::default(), denied),
            Err(Error::MissingCapability { capability: RenderCapability::LayoutDagre, diagram_type }) if diagram_type == "stateDiagram")
        );
        if cfg!(feature = "layout-dagre") {
            let direct = parse(NESTED, Some("dagre"), site);
            let fallback = svg(prepare(&parsed));
            assert_eq!(
                Document::parse(&fallback)
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
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            assert!(
                matches!(family::prepare(&parsed, &LayoutOptions::default(), session),
                Err(Error::MissingCapability { capability: RenderCapability::LayoutDagre, diagram_type }) if diagram_type == "stateDiagram")
            );
        }
    }
}
