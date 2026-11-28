#![cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]

use kurbo::{BezPath, Shape};
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::RenderSemanticModel;
use merman_render::environment::RenderEnvironment;
use merman_render::theme::{Appearance, BuiltinPreset, NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, RenderCapability, family};
use std::collections::BTreeSet;

#[test]
fn shoal_dense_flowcharts_paint_every_solved_edge() {
    let engine = MermaidEngine::new();
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Studio, Some(Appearance::Light))
        .unwrap();
    for source in [
        include_str!("fixtures/shoal-stress/dense-compound.mmd"),
        include_str!("fixtures/shoal-stress/dense-cycles.mmd"),
    ] {
        let parsed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let session = RenderEnvironment::deterministic()
            .begin_session_with_native_theme(NativeTheme::new(theme.clone()))
            .unwrap();
        let prepared = family::prepare(
            parsed.presentation(),
            &LayoutOptions::headless_svg_defaults(),
            session,
        )
        .unwrap();
        let svg = prepared
            .render_svg(&Default::default(), &Default::default())
            .unwrap();
        assert!(
            svg.required_capabilities()
                .contains(&RenderCapability::LayoutShoal)
        );
        let doc = roxmltree::Document::parse(svg.svg()).unwrap();
        let model = parsed.presentation().model();
        let RenderSemanticModel::Flowchart(model) = model else {
            panic!("expected a flowchart model");
        };
        let mut painted = BTreeSet::new();
        for edge in doc
            .descendants()
            .filter(|node| node.has_tag_name("path") && node.attribute("data-et") == Some("edge"))
        {
            let id = edge.attribute("data-id").unwrap();
            let d = edge.attribute("d").unwrap();
            let shaft = BezPath::from_svg(d).unwrap();
            assert!(shaft.segments().next().is_some(), "edge {id}: {d}");
            assert!(
                shaft.is_finite() && shaft.bounding_box().is_finite(),
                "edge {id}: {d}"
            );
            painted.insert(id);
        }
        assert_eq!(
            painted,
            model.edges.iter().map(|edge| edge.id.as_str()).collect()
        );
    }
}
