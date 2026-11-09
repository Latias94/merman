// Parser/editor-only fields remain in the shared core characterization cases.
#[allow(dead_code)]
#[path = "../../merman-core/src/tests/registry_cases.rs"]
mod catalog_cases;

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::PresentationDiagram;
use merman_render::LayoutOptions;
use merman_render::environment::{RenderEnvironment, RenderSession};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

#[cfg(not(feature = "layout-cytoscape"))]
use merman_render::diagram_theme::{
    DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue,
    ThemePortabilityRequirement, ThemeRuleSet, ThemeTarget,
};

fn parse_for_render(source: &str) -> PresentationDiagram {
    MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation()
}

fn render_session() -> RenderSession {
    RenderEnvironment::deterministic().begin_session().unwrap()
}

#[test]
fn public_native_finalization_still_enforces_strict_portability() {
    use merman_render::diagram_theme::ThemePortabilityRequirement;
    use merman_render::svg::{RootBackgroundPostprocessor, SvgPipeline};

    let parsed = parse_for_render("info");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session()
        .unwrap();
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let error = rendered
        .finalize_resvg(
            &SvgPipeline::resvg_safe()
                .with_postprocessor(RootBackgroundPostprocessor::new("#111827")),
        )
        .err()
        .expect("only facade-owned target admission may defer strict enforcement");
    assert!(matches!(
        error,
        merman_render::Error::UnverifiedFamilyOutputMutation { .. }
    ));
}

#[test]
fn c4_exposes_layout_only_through_the_prepared_family_artifact() {
    let parsed = parse_for_render("C4Context\nSystem(api, \"API\")\n");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), render_session())
        .expect("prepare C4 artifact");
    let projection = artifact.layout_json().expect("project C4 layout");

    assert_eq!(projection["meta"]["diagram_type"], "c4");
    assert_eq!(
        projection["layout"]["C4Diagram"]["shapes"][0]["alias"],
        "api"
    );
}

#[test]
fn xychart_exposes_layout_only_through_the_prepared_family_artifact() {
    let parsed = parse_for_render("xychart-beta\n  x-axis [A]\n  y-axis 0 --> 10\n  bar [7]\n");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), render_session())
        .expect("prepare XYChart artifact");
    let projection = artifact.layout_json().expect("project XYChart layout");
    let layout = &projection["layout"]["XyChartDiagram"];

    assert!(layout["width"].as_f64().is_some_and(|width| width > 0.0));
    assert!(
        layout["drawables"]
            .as_array()
            .is_some_and(|drawables| !drawables.is_empty())
    );
}

#[cfg(feature = "layout-cytoscape")]
#[test]
fn architecture_prepared_artifact_renders_the_typed_family() {
    let parsed = parse_for_render("architecture-beta\n  service api(server)[API]\n");
    let session = render_session();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Architecture artifact");
    let options = SvgRenderOptions {
        diagram_id: Some("typed-architecture".to_string()),
        ..Default::default()
    };

    let svg = artifact
        .render_svg(&options, &SvgDebugOptions::default())
        .expect("render Architecture artifact");

    assert!(svg.svg().contains(r#"id="typed-architecture-service-api""#));
}

#[cfg(not(feature = "layout-cytoscape"))]
#[test]
fn architecture_reports_the_missing_cytoscape_layout_capability() {
    let parsed = match MermaidEngine::new().parse_diagram_for_render_model_with_type_sync(
        "architecture",
        "architecture-beta\n  service api(server)[API]\n",
        ParseOptions::strict(),
    ) {
        Ok(Some(parsed)) => parsed.into_presentation(),
        Err(merman_core::Error::UnsupportedDiagram { .. }) => return,
        result => panic!("unexpected Architecture parse result: {result:?}"),
    };
    let error = match family::prepare(&parsed, &LayoutOptions::default(), render_session()) {
        Err(error) => error,
        Ok(_) => panic!("Architecture must be rejected without layout-cytoscape"),
    };

    assert_eq!(
        error.to_string(),
        "render session lacks capability `layout-cytoscape` required by diagram `architecture`"
    );
}

#[cfg(not(feature = "layout-cytoscape"))]
#[test]
fn mindmap_tidy_tree_renders_without_cytoscape_layout() {
    let palette = OrdinalPalette::new([ThemeColorValue::parse("#123456").unwrap()]).unwrap();
    let theme =
        DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Node, palette),
            ))
            .expect("compile default-feature Mindmap palette");
    let parsed = match common::install_parse_compatibility(&theme, MermaidEngine::new())
        .parse_diagram_for_render_model_with_type_sync(
            "mindmap",
            "---\nconfig:\n  layout: tidy-tree\n---\nmindmap\n  Root\n    Child\n",
            ParseOptions::strict(),
        ) {
        Ok(Some(parsed)) => parsed.into_presentation(),
        Err(merman_core::Error::UnsupportedDiagram { .. }) => return,
        result => panic!("unexpected Mindmap parse result: {result:?}"),
    };
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin default-feature Mindmap theme session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("tidy-tree Mindmap must not require layout-cytoscape");
    assert_eq!(artifact.family_id().as_str(), "mindmap");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("typed-mindmap-tidy-tree".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render strict default-feature Mindmap palette");

    assert!(rendered.svg().contains(r#"data-mindmap-section="0""#));
    assert!(rendered.svg().contains("fill:#123456"));
    let completion = rendered.into_completion();
    assert_eq!(
        merman_render::__private::family_evidence(completion.report()).applied_count(),
        1
    );
}

#[test]
fn state_prepared_artifact_renders_the_typed_family() {
    let parsed = parse_for_render("stateDiagram-v2\n[*] --> Active\n");
    let session = render_session();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare State artifact");
    let options = SvgRenderOptions {
        diagram_id: Some("typed-state".to_string()),
        ..Default::default()
    };

    let svg = artifact
        .render_svg(&options, &SvgDebugOptions::default())
        .expect("render State artifact");

    assert!(
        svg.svg().contains(r#"id="typed-state-state-Active-0""#),
        "{}",
        svg.svg()
    );
}

#[test]
fn every_catalog_variant_prepares_its_declared_family_or_exact_missing_capability() {
    use merman_model::family::{
        DiagramFamilyId, diagram_type_family_id, model_family_capabilities,
    };
    use merman_render::{Error, RenderCapability};
    use std::collections::BTreeSet;

    let catalog = model_family_capabilities();
    let cases = catalog_cases::FAMILY_CHARACTERIZATION_MATRIX;
    assert_eq!(
        cases
            .iter()
            .map(|case| case.variant_id)
            .collect::<BTreeSet<_>>(),
        catalog
            .iter()
            .map(|fact| fact.diagram_type)
            .collect::<BTreeSet<_>>()
    );
    let engine = MermaidEngine::new();
    let mut prepared_families = BTreeSet::new();
    let mut missing_families = BTreeSet::new();
    for case in cases {
        let expected = DiagramFamilyId::from_id(case.logical_family).unwrap();
        assert_eq!(diagram_type_family_id(case.variant_id), Some(expected));
        let parsed = engine
            .parse_diagram_for_render_model_with_type_sync(
                case.variant_id,
                case.representative_source,
                ParseOptions::strict(),
            )
            .unwrap_or_else(|error| panic!("{}: {error}", case.variant_id))
            .unwrap()
            .into_presentation();
        assert_eq!(
            parsed.family_id(),
            Some(expected),
            "{} parsed family",
            case.variant_id
        );
        let expected_missing = match case.variant_id {
            "architecture" if !cfg!(feature = "layout-cytoscape") => {
                Some(RenderCapability::LayoutCytoscape)
            }
            _ => None,
        };
        match (
            family::prepare(
                &parsed,
                &LayoutOptions::headless_svg_defaults(),
                render_session(),
            ),
            expected_missing,
        ) {
            (Err(Error::MissingCapability { capability, .. }), Some(expected_capability)) => {
                assert_eq!(
                    capability, expected_capability,
                    "{} capability",
                    case.variant_id
                );
                missing_families.insert(expected);
            }
            (Ok(artifact), None) => {
                assert_eq!(
                    artifact.family_id(),
                    expected,
                    "{} artifact family",
                    case.variant_id
                );
                let svg = artifact
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .unwrap_or_else(|error| panic!("{} writer: {error}", case.variant_id));
                assert!(
                    svg.svg().contains("<svg"),
                    "{} writer returned no SVG",
                    case.variant_id
                );
                prepared_families.insert(expected);
            }
            (Err(error), _) => panic!("{} preparation: {error}", case.variant_id),
            (Ok(_), Some(capability)) => {
                panic!("{} unexpectedly has {capability:?}", case.variant_id)
            }
        }
    }
    assert_eq!(
        prepared_families
            .union(&missing_families)
            .copied()
            .collect::<BTreeSet<_>>(),
        DiagramFamilyId::all().iter().copied().collect()
    );
    if cfg!(all(feature = "layout-elk", feature = "layout-cytoscape")) {
        assert!(missing_families.is_empty());
    }
}

#[allow(dead_code)]
mod common;
