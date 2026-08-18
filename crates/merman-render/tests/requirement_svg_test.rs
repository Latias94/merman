use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, ThemePortabilityRequirement,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{DiagramFamilyId, LayoutOptions};

fn requirement_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Requirement,
                        ThemeStylePatch::default().with_fill(fill),
                    )
                    .for_family(DiagramFamilyId::REQUIREMENT),
                ),
            ),
        )
        .expect("compile Requirement fill theme")
}

fn render_requirement_with_theme(source: &str, theme: &DiagramTheme) -> family::RenderedFamilySvg {
    render_requirement_with_theme_and_engine(source, theme, Engine::new())
}

fn render_requirement_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> family::RenderedFamilySvg {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Requirement")
        .expect("detect themed Requirement");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Requirement session");

    family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare themed Requirement")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("requirement-theme".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Requirement")
}

fn render_requirement_without_theme(source: &str) -> family::RenderedFamilySvg {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Requirement")
        .expect("detect Requirement");
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin Requirement session");

    family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare Requirement")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("requirement-theme".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render Requirement")
}

fn terminal_fill_for(document: &roxmltree::Document<'_>, node_id: &str) -> String {
    let requirement = document
        .descendants()
        .find(|node| node.attribute("id") == Some(node_id))
        .unwrap_or_else(|| panic!("missing Requirement node group {node_id}"));
    requirement
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("stroke") == Some("none")
                && node.attribute("stroke-width") == Some("0")
        })
        .and_then(|path| path.attribute("fill"))
        .unwrap_or_else(|| panic!("missing terminal fill path for {node_id}"))
        .to_string()
}

fn requirement_source() -> &'static str {
    r#"requirementDiagram
  requirement req1 {
    id: 1
    text: Themed requirement
    risk: high
    verifymethod: analysis
  }
"#
}

fn same_named_requirement_and_element_source() -> &'static str {
    r#"requirementDiagram
  requirement shared {
    id: R-1
    text: Requirement owner
    risk: low
    verifymethod: analysis
  }
  element shared {
    type: simulation
    docref: Element owner
  }
"#
}

#[test]
fn same_named_requirement_and_element_render_one_default_fill_terminal() {
    let rendered = render_requirement_without_theme(same_named_requirement_and_element_source());
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid default Requirement SVG");
    let node_id = "requirement-theme-shared";

    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("id") == Some(node_id))
            .count(),
        1
    );
    assert_eq!(terminal_fill_for(&document, node_id), "#ECECFF");
}

#[test]
fn same_named_requirement_and_element_share_one_typed_fill_receipt() {
    let theme = requirement_fill_theme(
        CanvasPaint::solid("#123456").expect("valid same-name Requirement fill"),
    );
    let rendered =
        render_requirement_with_theme(same_named_requirement_and_element_source(), &theme);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid themed Requirement SVG");
    let node_id = "requirement-theme-shared";

    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("id") == Some(node_id))
            .count(),
        1
    );
    assert_eq!(terminal_fill_for(&document, node_id), "#123456");

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_fill_reaches_terminal_path_and_family_evidence() {
    let cases = [
        (
            CanvasPaint::solid("#123456").expect("valid Requirement fill"),
            "#123456",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ];

    for (fill, expected_fill) in cases {
        let theme = requirement_fill_theme(fill);
        let rendered = render_requirement_with_theme(requirement_source(), &theme);
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid themed Requirement SVG");
        assert_eq!(
            terminal_fill_for(&document, "requirement-theme-req1"),
            expected_fill
        );

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn requirement_source_class_and_inline_fills_outrank_typed_fill_per_node() {
    let source = r#"requirementDiagram
  requirement bare {
    id: 1
    text: Typed owner
    risk: low
    verifymethod: analysis
  }
  requirement assigned {
    id: 2
    text: Class owner
    risk: medium
    verifymethod: test
  }
  element inline {
    type: simulation
    docref: inline-owner
  }
  classDef accent fill:#00aa00
  class assigned,inline accent
  style inline fill:#0000aa
"#;
    let theme = requirement_fill_theme(
        CanvasPaint::solid("#123456").expect("valid mixed Requirement fill"),
    );
    let rendered = render_requirement_with_theme(source, &theme);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid mixed-owner Requirement SVG");

    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-bare"),
        "#123456"
    );
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-assigned"),
        "#00aa00"
    );
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-inline"),
        "#0000aa"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_fill_is_not_applicable_when_every_node_has_a_source_owner() {
    let source = r#"requirementDiagram
  requirement assigned {
    id: 1
    text: Class owner
    risk: medium
    verifymethod: test
  }
  element inline {
    type: simulation
    docref: inline-owner
  }
  classDef accent fill:#00aa00
  class assigned accent
  style inline fill:#0000aa
"#;
    let theme = requirement_fill_theme(
        CanvasPaint::solid("#123456").expect("valid suppressed Requirement fill"),
    );
    let rendered = render_requirement_with_theme(source, &theme);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid source-owned Requirement SVG");
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-assigned"),
        "#00aa00"
    );
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-inline"),
        "#0000aa"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_explicit_background_outranks_typed_fill() {
    let theme = requirement_fill_theme(
        CanvasPaint::solid("#123456").expect("valid config-owned Requirement fill"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": { "requirementBackground": "#fedcba" }
    })));
    let rendered = render_requirement_with_theme_and_engine(requirement_source(), &theme, engine);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid config-owned Requirement SVG");
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-req1"),
        "#fedcba"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn extended_dark_primary_color_derived_background_outranks_typed_fill() {
    let theme = requirement_fill_theme(
        CanvasPaint::solid("#123456").expect("valid config-derived Requirement fill"),
    );
    let source_owned = format!(
        "%%{{init: {{\"themeVariables\": {{\"primaryColor\": \"#fedcba\"}}}}}}%%\n{}",
        requirement_source()
    );
    let cases = [
        (
            "site-owned neo-dark",
            requirement_source().to_string(),
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "theme": "neo-dark",
                "themeVariables": { "primaryColor": "#fedcba" }
            }))),
        ),
        (
            "source-owned redux-dark",
            source_owned,
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "theme": "redux-dark",
                "secure": []
            }))),
        ),
    ];

    for (owner, source, engine) in cases {
        let rendered = render_requirement_with_theme_and_engine(&source, &theme, engine);
        let document = roxmltree::Document::parse(rendered.svg())
            .unwrap_or_else(|error| panic!("valid {owner} Requirement SVG: {error}"));
        assert_eq!(
            terminal_fill_for(&document, "requirement-theme-req1"),
            "#fedcba",
            "{owner} primaryColor must retain the terminal fill"
        );

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "{owner}");
        assert_eq!(evidence.applied_count(), 0, "{owner}");
        assert_eq!(evidence.not_applicable_count(), 1, "{owner}");
        assert_eq!(evidence.theme_residual_count(), 0, "{owner}");
    }
}
