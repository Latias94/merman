use merman_core::{Engine, ParseOptions, ParsedDiagramRender};
use merman_render::LayoutOptions;
use merman_render::environment::{RenderEnvironment, RenderSession};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

#[cfg(not(feature = "layout-cytoscape"))]
use merman_render::diagram_theme::{
    DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue,
    ThemePortabilityRequirement, ThemeRuleSet, ThemeTarget,
};

fn parse_for_render(source: &str) -> ParsedDiagramRender {
    Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
}

fn render_session() -> RenderSession {
    RenderEnvironment::deterministic().begin_session().unwrap()
}

#[test]
fn c4_exposes_layout_only_through_the_prepared_family_artifact() {
    let parsed = parse_for_render("C4Context\nSystem(api, \"API\")\n");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), render_session())
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
    let artifact = family::prepare(parsed, &LayoutOptions::default(), render_session())
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
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
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
    let parsed = match Engine::new().parse_diagram_for_render_model_with_type_sync(
        "architecture",
        "architecture-beta\n  service api(server)[API]\n",
        ParseOptions::strict(),
    ) {
        Ok(Some(parsed)) => parsed,
        Err(merman_core::Error::UnsupportedDiagram { .. }) => return,
        result => panic!("unexpected Architecture parse result: {result:?}"),
    };
    let error = match family::prepare(parsed, &LayoutOptions::default(), render_session()) {
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
    let parsed = match merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_with_type_sync(
            "mindmap",
            "---\nconfig:\n  layout: tidy-tree\n---\nmindmap\n  Root\n    Child\n",
            ParseOptions::strict(),
        ) {
        Ok(Some(parsed)) => parsed,
        Err(merman_core::Error::UnsupportedDiagram { .. }) => return,
        result => panic!("unexpected Mindmap parse result: {result:?}"),
    };
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin default-feature Mindmap theme session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
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

#[cfg(not(feature = "layout-elk"))]
#[test]
fn elk_flowchart_reports_the_missing_layout_capability() {
    let parsed = parse_for_render(
        "---\nconfig:\n  layout: elk\n---\nflowchart TD\n  start[Start] --> finish[Finish]\n",
    );
    let error = match family::prepare(parsed, &LayoutOptions::default(), render_session()) {
        Err(error) => error,
        Ok(_) => panic!("ELK Flowchart must be rejected without layout-elk"),
    };

    assert_eq!(
        error.to_string(),
        "render session lacks capability `layout-elk` required by diagram `flowchart-v2`"
    );
}

#[cfg(not(feature = "layout-elk"))]
#[test]
fn elk_er_reports_the_missing_layout_capability() {
    let parsed =
        parse_for_render("---\nconfig:\n  layout: elk\n---\nerDiagram\n  A ||--o{ B : contains\n");
    let error = match family::prepare(parsed, &LayoutOptions::default(), render_session()) {
        Err(error) => error,
        Ok(_) => panic!("ELK ER must be rejected without layout-elk"),
    };

    assert_eq!(
        error.to_string(),
        "render session lacks capability `layout-elk` required by diagram `er`"
    );
}

#[test]
fn state_prepared_artifact_renders_the_typed_family() {
    let parsed = parse_for_render("stateDiagram-v2\n[*] --> Active\n");
    let session = render_session();
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
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

#[cfg(feature = "internal-theme-acceptance")]
#[test]
fn completion_receipts_bind_both_finalized_outputs_and_reject_other_renders() {
    use merman_render::DiagramFamilyId;
    use merman_render::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeColorValue, ThemeRule,
        ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    };
    use merman_render::svg::{StandaloneSvgArtifact, SvgPipeline};
    use sha2::{Digest as _, Sha256};

    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default().with_fill(CanvasPaint::Solid(
                            ThemeColorValue::parse("#125abc").unwrap(),
                        )),
                    )
                    .for_family(DiagramFamilyId::BLOCK),
                ),
            ),
        )
        .unwrap();
    let render = |source: &str| {
        let session = RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .unwrap();
        let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        family::prepare(parsed, &LayoutOptions::default(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap()
            .finalize_resvg(&SvgPipeline::resvg_safe())
            .unwrap()
            .into_completion()
    };
    let first = render("block-beta\n  a[\"First\"]\n");
    let second = render("block-beta\n  b[\"Second\"]\n");
    let receipts = first.theme_route_cutover_receipts();
    assert!(
        !receipts.is_empty(),
        "the typed Node.fill route must be witnessed"
    );
    let (first_svg, _) = first.into_output_and_report();
    let first_svg = StandaloneSvgArtifact::from(first_svg);
    let second_svg = StandaloneSvgArtifact::from(second.into_output_and_report().0);
    let public_digest = Sha256::digest(first_svg.as_str().as_bytes()).into();
    let native_digest = Sha256::digest(first_svg.native_export_svg().as_bytes()).into();
    let other_public = Sha256::digest(second_svg.as_str().as_bytes()).into();
    let other_native = Sha256::digest(second_svg.native_export_svg().as_bytes()).into();
    assert_ne!(public_digest, other_public);
    assert_ne!(native_digest, other_native);
    for receipt in receipts {
        assert!(receipt.proves_artifacts(public_digest, native_digest));
        assert!(!receipt.proves_artifacts(other_public, native_digest));
        assert!(!receipt.proves_artifacts(public_digest, other_native));
    }
}
