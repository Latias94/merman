#![cfg(feature = "diagram-class")]

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_model::presentation::PresentationDiagram;
use merman_render::{Error, environment::RenderEnvironment, family};
use serde_json::Value;

const FAN: &str = "classDiagram\nA --> B\nA --> C";

fn parse(body: &str, source_layout: Option<&str>, site: Value) -> PresentationDiagram {
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
fn class_layered_requires_a_compiled_provider() {
    let parsed = parse(FAN, Some("layered"), Value::Null);
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    assert!(matches!(family::plan_render(&parsed, &session),
        Err(Error::UnsupportedLayoutBackend { backend: "layered", diagram_type }) if diagram_type == "classDiagram"));
}

#[cfg(feature = "layout-layered")]
mod layered {
    use super::*;
    use merman_render::{
        DiagramFamilyId, LayoutOptions, RenderCapability, RenderCapabilityPolicy,
        family::FamilyRenderArtifact, model::ClassDiagramLayout, svg::SvgRenderOptions,
    };
    use serde_json::json;

    fn prepare(parsed: &PresentationDiagram) -> merman_render::Result<FamilyRenderArtifact<'_>> {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        family::prepare(parsed, &LayoutOptions::default(), session)
    }

    fn svg(artifact: FamilyRenderArtifact<'_>) -> String {
        artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("layered-class".into()),
                    ..Default::default()
                },
                &Default::default(),
            )
            .unwrap()
            .svg()
            .to_owned()
    }

    #[test]
    fn explicit_class_layered_selects_source_or_site_and_preserves_family_dto() {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        for (source, site) in [(Some("layered"), "dagre"), (None, "layered")] {
            let parsed = parse(
                FAN,
                source,
                json!({"layout":site,"look":"classic","htmlLabels":false}),
            );
            let plan = family::plan_render(&parsed, &session).unwrap();
            assert_eq!(plan.family_id(), DiagramFamilyId::CLASS);
            assert_eq!(
                plan.required_capabilities(),
                [RenderCapability::LayoutLayered]
            );
            let artifact = prepare(&parsed).unwrap();
            let geometry = artifact.layout_geometry_json().unwrap();
            assert_eq!(
                geometry["layout"],
                artifact.layout_json().unwrap()["layout"]
            );
            let layout: ClassDiagramLayout =
                serde_json::from_value(geometry["layout"]["ClassDiagramV2"].clone()).unwrap();
            let [a, b, c] = &layout.nodes[..] else {
                panic!("three Class nodes")
            };
            assert_eq!(
                (a.id.as_str(), b.id.as_str(), c.id.as_str()),
                ("A", "B", "C")
            );
            assert!(b.x < a.x && a.x < c.x && a.y < b.y && (b.y - c.y).abs() < 1e-6);
            let rendered = svg(artifact);
            let doc = roxmltree::Document::parse(&rendered).unwrap();
            assert_eq!(doc.root_element().attribute("data-layout-fallback"), None);
            let paths = doc
                .descendants()
                .filter(|path| path.attribute("data-edge") == Some("true"))
                .collect::<Vec<_>>();
            assert_eq!(paths[0].attribute("data-id"), Some("id_A_B_1"));
            assert_eq!(paths[1].attribute("data-id"), Some("id_A_C_2"));
            assert!(paths[0].attribute("d").unwrap().contains('A'));
        }

        let default = parse(FAN, None, json!({}));
        let plan = family::plan_render(&default, &session).unwrap();
        let classic =
            if merman_render::layout_elk_available() || !merman_render::layout_dagre_available() {
                RenderCapability::LayoutElk
            } else {
                RenderCapability::LayoutDagre
            };
        assert_eq!(plan.required_capabilities(), [classic]);
    }

    #[test]
    fn class_layered_requires_explicit_host_admission() {
        let parsed = parse(FAN, Some("layered"), json!({}));
        let denied = RenderEnvironment::deterministic()
            .with_capability_policy(
                RenderCapabilityPolicy::unrestricted().with_denied(RenderCapability::LayoutLayered),
            )
            .begin_session()
            .unwrap();
        assert!(
            matches!(family::prepare(&parsed, &LayoutOptions::default(), denied),
            Err(Error::MissingCapability {capability: RenderCapability::LayoutLayered, diagram_type}) if diagram_type == "classDiagram")
        );
    }

    #[test]
    fn class_layered_retry_requires_dagre_and_keeps_fresh_dagre_paint() {
        let body = "classDiagram\nnamespace Shop.Core {\nclass Item\n}\nOutside --> Item";
        // Each Layered title band reserves its bottom margin. Nesting two overflows the layout
        // geometry, while the fresh Dagre path has its usual finite title coordinates.
        let site = json!({"look":"classic","htmlLabels":false,
            "flowchart":{"subGraphTitleMargin":{"bottom":1.1e308}}});
        let parsed = parse(body, Some("layered"), site.clone());
        let denied = RenderEnvironment::deterministic()
            .with_capability_policy(
                RenderCapabilityPolicy::unrestricted().with_denied(RenderCapability::LayoutDagre),
            )
            .begin_session()
            .unwrap();
        let is_missing_dagre = |error| {
            matches!(error, Error::MissingCapability {
            capability: RenderCapability::LayoutDagre, diagram_type } if diagram_type == "classDiagram")
        };
        assert!(
            family::prepare(&parsed, &LayoutOptions::default(), denied)
                .is_err_and(is_missing_dagre)
        );

        if cfg!(feature = "layout-dagre") {
            let direct = parse(body, Some("dagre"), site);
            let fallback = svg(prepare(&parsed).unwrap());
            assert_eq!(
                roxmltree::Document::parse(&fallback)
                    .unwrap()
                    .root_element()
                    .attribute("data-layout-fallback"),
                Some("dagre")
            );
            assert_eq!(
                fallback.replacen(" data-layout-fallback=\"dagre\"", "", 1),
                svg(prepare(&direct).unwrap())
            );
        } else {
            assert!(prepare(&parsed).is_err_and(is_missing_dagre));
        }
    }
}
