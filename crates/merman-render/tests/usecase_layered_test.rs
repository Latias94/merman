#![cfg(feature = "diagram-usecase")]

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_model::presentation::PresentationDiagram;
use merman_render::{Error, environment::RenderEnvironment, family};
use serde_json::{Value, json};

const CASE: &str = r#"usecase-beta
direction LR
systemBoundary Auth(Authentication)@{type:package}
  Login("Sign & go")@{business:true}
  Verify[Check]
end
actor User("Customer")@{type:hollow} <<Human>>
actor Guest("Guest")
User review@-- "**literal** account review" --> Login
Guest join@--> Login
Login verify@..> : include Verify
Login loop@-- "return" --> Login
note for Login "Remember"
json Payload@{"z":1,"a":2}
Payload data@-- Login
"#;

fn parse(body: &str, layout: Option<&str>, site: Value) -> PresentationDiagram {
    let source = layout.map_or_else(
        || body.to_owned(),
        |layout| format!("---\nconfig:\n  layout: {layout}\n---\n{body}"),
    );
    MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(site))
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation()
}

#[cfg(not(feature = "layout-layered"))]
#[test]
fn explicit_usecase_layered_requires_a_compiled_provider() {
    let parsed = parse(CASE, Some("layered"), json!({}));
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    assert!(matches!(family::plan_render(&parsed, &session),
        Err(Error::UnsupportedLayoutBackend {backend:"layered", diagram_type}) if diagram_type == "usecase"));
}

#[cfg(feature = "layout-layered")]
mod layered {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use merman_render::{
        LayoutOptions, RenderCapability, RenderCapabilityPolicy, family::FamilyRenderArtifact,
        svg::SvgRenderOptions, usecase::UsecaseDiagramLayout,
    };

    fn prepare(parsed: &PresentationDiagram) -> FamilyRenderArtifact<'_> {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        family::prepare(parsed, &LayoutOptions::default(), session).unwrap()
    }

    fn svg(artifact: FamilyRenderArtifact<'_>) -> String {
        artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("usecase-layered".into()),
                    ..Default::default()
                },
                &Default::default(),
            )
            .unwrap()
            .svg()
            .to_owned()
    }

    #[test]
    fn usecase_layered_is_explicit_and_preserves_source_layout_precedence() {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let default = if cfg!(feature = "layout-elk") || !cfg!(feature = "layout-dagre") {
            RenderCapability::LayoutElk
        } else {
            RenderCapability::LayoutDagre
        };
        for (source, site, expected) in [
            (None, json!({}), default),
            (
                None,
                json!({"layout":"layered"}),
                RenderCapability::LayoutLayered,
            ),
            (
                Some("layered"),
                json!({"layout":"dagre"}),
                RenderCapability::LayoutLayered,
            ),
            (
                Some("dagre"),
                json!({"layout":"layered"}),
                RenderCapability::LayoutDagre,
            ),
        ] {
            let parsed = parse(CASE, source, site);
            assert_eq!(
                family::plan_render(&parsed, &session)
                    .unwrap()
                    .required_capabilities(),
                [expected]
            );
        }
    }

    #[test]
    fn usecase_layered_keeps_route_order_shapes_and_shifted_transparent_labels() {
        for html in [false, true] {
            let parsed = parse(
                CASE,
                Some("layered"),
                json!({
                    "look": "classic", "htmlLabels": html,
                    "flowchart": {"subGraphTitleMargin": {"top": 7, "bottom": 3}},
                    "layered": {"edgeRouting":"straight", "lineHops":false},
                    "themeVariables": {"edgeLabelBackground": "transparent"},
                }),
            );
            let artifact = prepare(&parsed);
            let layout: UsecaseDiagramLayout = serde_json::from_value(
                artifact.layout_geometry_json().unwrap()["layout"]["UsecaseDiagram"].clone(),
            )
            .unwrap();
            let svg = svg(artifact);
            let doc = roxmltree::Document::parse(&svg).unwrap();
            let root = doc.root_element();
            assert!(root.attribute("data-layout-fallback").is_none());
            let group = |class| {
                doc.descendants()
                    .find(|node| node.attribute("class") == Some(class))
                    .unwrap()
            };
            assert_eq!(
                group("root")
                    .children()
                    .filter_map(|node| node.attribute("class"))
                    .collect::<Vec<_>>(),
                ["clusters", "edges edgePaths", "edgeLabels", "nodes"]
            );
            let sha = group("edges edgePaths");
            assert_eq!(
                sha.attribute("mask"),
                Some("url(#usecase-layered_usecase-edge-label-mask)")
            );
            let edges: Vec<_> = sha
                .children()
                .filter(|node| node.has_tag_name("path"))
                .collect();
            let ids = ["review", "join", "verify", "loop", "data", "note-0-edge"];
            assert_eq!(
                layout
                    .edges
                    .iter()
                    .map(|edge| edge.id.as_str())
                    .collect::<Vec<_>>(),
                ids
            );
            assert_eq!(
                edges
                    .iter()
                    .filter_map(|node| node.attribute("data-id"))
                    .collect::<Vec<_>>(),
                ids
            );
            for path in &edges {
                let points: Vec<Value> = serde_json::from_slice(
                    &STANDARD
                        .decode(path.attribute("data-points").unwrap())
                        .unwrap(),
                )
                .unwrap();
                assert!(
                    points.len() >= 2
                        && points
                            .iter()
                            .all(|p| matches!(p["axis"].as_str(), Some("h" | "v")))
                );
                assert!(!path.attribute("d").unwrap().contains(['A', 'C', 'Q']));
            }
            assert_eq!(
                edges[0].attribute("marker-end"),
                Some("url(#usecase-layered_usecase-pointEnd)")
            );
            assert_eq!(edges.last().unwrap().attribute("aria-hidden"), Some("true"));
            assert_eq!(
                group("usecase-actor-outline").attribute("opacity"),
                Some("0")
            );
            let review = group("edgeLabels")
                .children()
                .find(|node| node.is_element())
                .unwrap();
            assert!(
                review
                    .descendants()
                    .any(|node| node.is_text() && node.text().unwrap().contains("**literal**"))
            );
            assert_eq!(
                review
                    .descendants()
                    .any(|node| node.has_tag_name("foreignObject")),
                html
            );
            let center = layout.edges[0].label.as_ref().unwrap();
            let transform = review
                .attribute("transform")
                .unwrap()
                .trim_start_matches("translate(")
                .trim_end_matches(')');
            let (x, y) = transform.split_once(',').unwrap();
            assert!((x.parse::<f64>().unwrap() - center.x).abs() < 0.001);
            assert!((y.parse::<f64>().unwrap() - center.y - 5.0).abs() < 0.001);
            let cutout = doc
                .descendants()
                .find(|node| {
                    node.has_tag_name("rect")
                        && node.attribute("fill") == Some("black")
                        && node
                            .parent_element()
                            .is_some_and(|parent| parent.has_tag_name("mask"))
                })
                .unwrap();
            let top = cutout.attribute("y").unwrap().parse::<f64>().unwrap();
            assert!((top + center.height / 2.0 - center.y - 5.0).abs() < 0.001);
        }
    }

    #[test]
    fn usecase_layered_clips_to_the_painted_oval_and_deduplicates_authored_markers() {
        let parsed = parse(
            "usecase-beta\ndirection LR\nactor A\nactor B\nOval(\"Browse the products\")\nA first@--> Oval\nB second@--> Oval\nstyle first stroke:#2aa198\nstyle second stroke:#2aa198\n",
            Some("layered"),
            json!({"look":"classic", "htmlLabels":false, "layered":{"edgeRouting":"straight"}}),
        );
        let artifact = prepare(&parsed);
        let layout: UsecaseDiagramLayout = serde_json::from_value(
            artifact.layout_geometry_json().unwrap()["layout"]["UsecaseDiagram"].clone(),
        )
        .unwrap();
        let oval = layout.nodes.iter().find(|node| node.id == "Oval").unwrap();
        let svg = svg(artifact);
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let ellipse = doc
            .descendants()
            .find(|node| node.has_tag_name("ellipse"))
            .unwrap();
        let radius = |name| ellipse.attribute(name).unwrap().parse::<f64>().unwrap();
        let (rx, ry) = (radius("rx"), radius("ry"));
        assert!(
            oval.width > 2.0 * rx,
            "the SVG text can extend past the ellipse"
        );
        let id = "usecase-layered_usecase-pointEnd__2aa198";
        let mut defs = doc
            .descendants()
            .filter(|node| node.attribute("id") == Some(id));
        let marker = defs.next().unwrap();
        assert!(defs.next().is_none());
        assert_eq!(
            marker.first_element_child().unwrap().attribute("stroke"),
            Some("#2aa198")
        );
        for edge in doc
            .descendants()
            .filter(|node| node.attribute("data-et") == Some("edge"))
        {
            assert_eq!(edge.attribute("marker-end").unwrap(), format!("url(#{id})"));
            let points: Vec<Value> = serde_json::from_slice(
                &STANDARD
                    .decode(edge.attribute("data-points").unwrap())
                    .unwrap(),
            )
            .unwrap();
            let point = points.last().unwrap();
            let x = point["x"].as_f64().unwrap() - oval.x;
            let y = point["y"].as_f64().unwrap() - oval.y;
            assert!(((x / rx).powi(2) + (y / ry).powi(2) - 1.0).abs() < 1e-8);
            let raw = layout
                .edges
                .iter()
                .find(|route| edge.attribute("data-id") == Some(&route.id))
                .unwrap()
                .points
                .last()
                .unwrap();
            assert!((raw.x - oval.x).abs() > x.abs());
            assert_eq!(raw.y, point["y"].as_f64().unwrap());
        }
    }

    #[test]
    fn usecase_layered_retry_checks_dagre_capability_and_restarts_the_attempt() {
        // Layered ignores the negative Usecase rank spacing and uses the large Flowchart
        // value. Dagre's JS truthiness rule consumes the finite nonzero Usecase value.
        let site = json!({"htmlLabels": false, "usecase":{"rankSpacing": -1}, "flowchart":{"rankSpacing": 1.7e308}});
        let body = "usecase-beta\ndirection LR\nactor A\nB(Beta)\nC(Third)\nD(Fourth)\nA --> B\nB --> C\nC --> D\n";
        let parsed = parse(body, Some("layered"), site.clone());
        let environment = RenderEnvironment::deterministic();
        let denied = environment
            .clone()
            .with_capability_policy(
                RenderCapabilityPolicy::unrestricted().with_denied(RenderCapability::LayoutDagre),
            )
            .begin_session()
            .unwrap();
        assert!(
            matches!(family::prepare(&parsed, &LayoutOptions::default(), denied),
            Err(Error::MissingCapability{capability: RenderCapability::LayoutDagre, diagram_type}) if diagram_type == "usecase")
        );
        if cfg!(feature = "layout-dagre") {
            let render = |diagram| {
                prepare(diagram)
                    .render_svg(&Default::default(), &Default::default())
                    .unwrap()
                    .into_completion()
            };
            let fallback = render(&parsed);
            assert!(fallback.output().contains("data-layout-fallback=\"dagre\""));
            let direct = render(&parse(body, Some("dagre"), site));
            assert_eq!(
                fallback
                    .output()
                    .replacen(" data-layout-fallback=\"dagre\"", "", 1),
                *direct.output()
            );
            let direct_work = direct.report().session_report().layout_work_units();
            assert!(fallback.report().session_report().layout_work_units() > direct_work);
            let limits = merman_render::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(
                    merman_render::ResourceLimitId::MaxLayoutWorkUnits,
                    direct_work,
                )
                .unwrap();
            let session = environment
                .clone()
                .with_resource_policy(limits)
                .begin_session()
                .unwrap();
            let capped = family::prepare(&parsed, &LayoutOptions::default(), session)
                .and_then(|artifact| artifact.render_svg(&Default::default(), &Default::default()));
            assert!(
                matches!(capped, Err(Error::ResourceLimitExceeded(limit)) if limit.limit == "max_layout_work_units")
            );
        } else {
            let session = environment.begin_session().unwrap();
            assert!(
                matches!(family::prepare(&parsed, &LayoutOptions::default(), session),
                Err(Error::MissingCapability{capability: RenderCapability::LayoutDagre, diagram_type}) if diagram_type == "usecase")
            );
        }
    }
}
