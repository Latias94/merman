use merman_core::{DiagramFamilyId, Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, GradientStop,
    LinearGradient, OrdinalPalette, OrdinalSelector, PatternKind, PatternSpec, Specified,
    ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

const TWO_BRANCHES: &str = r#"gitGraph
  commit id: "1"
  branch develop
  checkout develop
  commit id: "2"
  checkout main
  commit id: "3"
"#;

const SOURCE_OWNED_TWO_BRANCHES: &str = r##"%%{init: {"themeVariables": {"git0": "#fedcba", "gitInv0": "#abcdef", "gitBranchLabel0": "#112233"}}}%%
gitGraph
  commit id: "1"
  branch develop
  checkout develop
  commit id: "2"
  checkout main
  commit id: "3"
"##;

fn gitgraph_node_palette_theme(colors: &[&str]) -> DiagramTheme {
    let palette = OrdinalPalette::new(
        colors
            .iter()
            .map(|color| ThemeColorValue::parse(*color).expect("valid GitGraph palette color")),
    )
    .expect("non-empty GitGraph Node palette");
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Node, palette),
            ),
        )
        .expect("compile GitGraph Node palette")
}

fn gitgraph_node_palette_with_static_stroke_theme(
    colors: &[&str],
    stroke: CanvasPaint,
) -> DiagramTheme {
    let palette = OrdinalPalette::new(
        colors
            .iter()
            .map(|color| ThemeColorValue::parse(*color).expect("valid GitGraph palette color")),
    )
    .expect("non-empty GitGraph Node palette");
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_ordinal_palette(ThemeTarget::Node, palette)
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default().with_stroke(stroke),
                        )
                        .for_family(DiagramFamilyId::GIT_GRAPH),
                    ),
            ),
        )
        .expect("compile GitGraph Node palette with static stroke")
}

fn gitgraph_edge_rules_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    let styles = rules
        .into_iter()
        .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule);
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile GitGraph Edge rules")
}

fn gitgraph_edge_stroke_theme(stroke: CanvasPaint) -> DiagramTheme {
    gitgraph_edge_stroke_theme_variant(stroke, None)
}

fn gitgraph_edge_stroke_theme_variant(
    stroke: CanvasPaint,
    variant: Option<ThemeVariant>,
) -> DiagramTheme {
    let rule = ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default().with_stroke(stroke),
    )
    .for_family(DiagramFamilyId::GIT_GRAPH);
    let rule = match variant {
        Some(variant) => rule.with_variant(variant),
        None => rule,
    };
    gitgraph_edge_rules_theme([rule])
}

fn gitgraph_typography_theme(typography: ThemeTextStyle) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::GIT_GRAPH, typography),
        ))
        .expect("compile GitGraph typography theme")
}

fn gitgraph_stylesheet(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid GitGraph SVG");
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .collect()
}

fn gitgraph_css_rule(stylesheet: &str, selector: &str) -> String {
    let marker = format!("{selector}{{");
    let start = stylesheet
        .find(&marker)
        .unwrap_or_else(|| panic!("missing GitGraph CSS selector `{selector}`: {stylesheet}"))
        + marker.len();
    let end = stylesheet[start..]
        .find('}')
        .unwrap_or_else(|| panic!("unterminated GitGraph CSS selector `{selector}`: {stylesheet}"));
    stylesheet[start..start + end].to_string()
}

fn css_property<'a>(declarations: &'a str, property: &str) -> Option<&'a str> {
    declarations.split(';').find_map(|declaration| {
        let (name, value) = declaration.split_once(':')?;
        (name == property).then_some(value)
    })
}

fn css_property_winner<'a>(declarations: &'a str, property: &str) -> Option<&'a str> {
    declarations
        .split(';')
        .filter_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            (name == property).then_some(value)
        })
        .last()
}

fn gitgraph_legacy_paint_theme() -> DiagramTheme {
    let solid = |color| CanvasPaint::solid(color).expect("valid GitGraph compatibility color");
    let rules = [
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default()
                .with_fill(solid("#101112"))
                .with_stroke(solid("#131415")),
        ),
        ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(solid("#191a1b")),
        ),
        ThemeRule::new(
            ThemeTarget::NodeLabel,
            ThemeStylePatch::default().with_fill(solid("#161718")),
        ),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_fill(solid("#1c1d1e")),
        ),
        ThemeRule::new(
            ThemeTarget::EdgeLabel,
            ThemeStylePatch::default().with_fill(solid("#1f2021")),
        ),
        ThemeRule::new(
            ThemeTarget::EdgeLabelBackground,
            ThemeStylePatch::default().with_fill(solid("#222324")),
        ),
    ]
    .map(|rule| rule.for_family(DiagramFamilyId::GIT_GRAPH));

    gitgraph_edge_rules_theme(rules)
}

fn try_render_gitgraph_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    diagram_id: &str,
) -> merman_render::Result<family::RenderedFamilySvg> {
    try_render_gitgraph_with_theme_engine_and_requirement(
        source,
        theme,
        engine,
        diagram_id,
        ThemePortabilityRequirement::RequirePortable,
    )
}

fn try_render_gitgraph_with_theme_engine_and_requirement(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    diagram_id: &str,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed GitGraph")
        .expect("detect themed GitGraph");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed GitGraph session");
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )
}

fn render_gitgraph_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    diagram_id: &str,
) -> family::RenderedFamilySvg {
    try_render_gitgraph_with_theme_and_engine(source, theme, engine, diagram_id)
        .expect("render themed GitGraph")
}

#[test]
fn gitgraph_mixed_base_typography_is_direct_and_portable() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(
            FontStack::new(["GitGraph Typed", "monospace"])
                .expect("valid GitGraph typed font stack"),
        )
        .with_font_size_px(23.0)
        .expect("valid GitGraph typed font size");
    let theme = gitgraph_typography_theme(typography);
    let rendered = try_render_gitgraph_with_theme_and_engine(
        r#"---
title: GitGraph typography
---
gitGraph
  commit id: "1" tag: "v1"
  branch develop
  checkout develop
  commit id: "2"
"#,
        &theme,
        Engine::new(),
        "git-typography",
    )
    .expect("render strict portable GitGraph typography");

    for path in [
        "fontFamily",
        "themeVariables.fontFamily",
        "themeVariables.fontSize",
    ] {
        assert!(
            !merman_core::__private::fallback_overlay_owns_path(
                &rendered.metadata().effective_config,
                path,
            ),
            "GitGraph direct typography must retire fallback ownership of {path}"
        );
    }

    let stylesheet = gitgraph_stylesheet(rendered.svg());
    assert_eq!(
        gitgraph_css_rule(&stylesheet, "#git-typography"),
        "font-family:\"GitGraph Typed\", monospace;font-size:23px;fill:#333;"
    );
    assert_eq!(
        gitgraph_css_rule(&stylesheet, "#git-typography svg"),
        "font-family:\"GitGraph Typed\", monospace;font-size:23px;"
    );
    assert_eq!(
        css_property_winner(
            &gitgraph_css_rule(
                &stylesheet,
                "#git-typography .commit-id,#git-typography .commit-msg,#git-typography .branch-label"
            ),
            "font-family"
        ),
        Some("\"GitGraph Typed\", monospace")
    );
    assert!(!stylesheet.contains("font-family:var(--mermaid-font-family)"));

    let document = roxmltree::Document::parse(rendered.svg()).expect("valid GitGraph SVG");
    assert!(document.descendants().any(|node| {
        node.is_element()
            && node.tag_name().name() == "tspan"
            && node.attribute("class") == Some("row")
            && node.text() == Some("develop")
    }));
    assert!(document.descendants().any(|node| {
        node.is_element()
            && node.tag_name().name() == "text"
            && node.attribute("class") == Some("commit-label")
            && node.text() == Some("1")
    }));
    assert!(document.descendants().any(|node| {
        node.is_element()
            && node.tag_name().name() == "text"
            && node.attribute("class") == Some("tag-label")
            && node.text() == Some("v1")
    }));
    assert!(document.descendants().any(|node| {
        node.is_element()
            && node.tag_name().name() == "text"
            && node.attribute("class") == Some("gitTitleText")
            && node.text() == Some("GitGraph typography")
    }));

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn gitgraph_font_stack_only_is_proven_by_fixed_size_visual_roles() {
    let theme = gitgraph_typography_theme(
        ThemeTextStyle::default().with_font_stack(
            FontStack::new(["GitGraphStackOnly", "monospace"])
                .expect("valid GitGraph stack-only theme"),
        ),
    );
    let rendered = render_gitgraph_with_theme_and_engine(
        r#"---
title: GitGraph fixed roles
---
gitGraph
  commit id: "1" tag: "v1"
"#,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "gitGraph": { "showBranches": false }
        }))),
        "git-stack-fixed-roles",
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid GitGraph SVG");
    let root_rule = gitgraph_css_rule(
        &gitgraph_stylesheet(rendered.svg()),
        "#git-stack-fixed-roles",
    );

    assert_eq!(
        css_property(&root_rule, "font-family"),
        Some("GitGraphStackOnly, monospace")
    );
    assert_eq!(css_property(&root_rule, "font-size"), Some("16px"));
    assert!(
        !document
            .descendants()
            .any(|node| has_class(&node, "branch-label0"))
    );
    for (class, text) in [
        ("commit-label", "1"),
        ("tag-label", "v1"),
        ("gitTitleText", "GitGraph fixed roles"),
    ] {
        assert!(document.descendants().any(|node| {
            node.is_element() && has_class(&node, class) && node.text() == Some(text)
        }));
    }

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gitgraph_font_size_requires_a_visible_nonempty_branch_label() {
    let theme = gitgraph_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(29.0)
            .expect("valid GitGraph font size"),
    );
    let visible = render_gitgraph_with_theme_and_engine(
        TWO_BRANCHES,
        &theme,
        Engine::new(),
        "git-size-visible-branch",
    );
    let visible_rule = gitgraph_css_rule(
        &gitgraph_stylesheet(visible.svg()),
        "#git-size-visible-branch",
    );
    assert_eq!(css_property(&visible_rule, "font-size"), Some("29px"));
    let visible_evidence =
        merman_render::__private::family_evidence(visible.into_completion().report());
    assert_eq!(visible_evidence.applied_count(), 1);
    assert_eq!(visible_evidence.not_applicable_count(), 0);

    let fixed_roles_only = render_gitgraph_with_theme_and_engine(
        r#"---
title: Fixed size roles only
---
gitGraph
  commit id: "1" tag: "v1"
"#,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "gitGraph": { "showBranches": false }
        }))),
        "git-size-fixed-roles",
    );
    let fixed_document =
        roxmltree::Document::parse(fixed_roles_only.svg()).expect("valid GitGraph SVG");
    assert!(
        !fixed_document
            .descendants()
            .any(|node| has_class(&node, "branch-label0"))
    );
    assert!(
        fixed_document
            .descendants()
            .any(|node| has_class(&node, "commit-label"))
    );
    assert!(
        fixed_document
            .descendants()
            .any(|node| has_class(&node, "tag-label"))
    );
    assert!(
        fixed_document
            .descendants()
            .any(|node| has_class(&node, "gitTitleText"))
    );
    let fixed_evidence =
        merman_render::__private::family_evidence(fixed_roles_only.into_completion().report());
    assert_eq!(fixed_evidence.applied_count(), 0);
    assert_eq!(fixed_evidence.not_applicable_count(), 1);
    assert_eq!(fixed_evidence.theme_residual_count(), 0);
}

#[test]
fn gitgraph_base_dependent_role_sizes_use_the_typed_font_size() {
    let theme = gitgraph_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(29.0)
            .expect("valid GitGraph font size"),
    );
    let rendered = render_gitgraph_with_theme_and_engine(
        r#"gitGraph
  commit id: "1" tag: "v1"
"#,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "gitGraph": { "showBranches": false },
            "themeVariables": {
                "commitLabelFontSize": "inherit",
                "tagLabelFontSize": "150%"
            }
        }))),
        "git-relative-role-size",
    );
    let stylesheet = gitgraph_stylesheet(rendered.svg());
    assert_eq!(
        css_property(
            &gitgraph_css_rule(&stylesheet, "#git-relative-role-size .commit-label"),
            "font-size"
        ),
        Some("inherit")
    );
    assert_eq!(
        css_property(
            &gitgraph_css_rule(&stylesheet, "#git-relative-role-size .tag-label"),
            "font-size"
        ),
        Some("150%")
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gitgraph_unmeasurable_role_size_fails_closed() {
    let theme = gitgraph_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(29.0)
            .expect("valid GitGraph font size"),
    );
    let engine = || {
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "gitGraph": { "showBranches": false },
            "themeVariables": {
                "commitLabelFontSize": "calc(10px + 1em)"
            }
        })))
    };
    let source = r#"gitGraph
  commit id: "1"
"#;
    let error = match try_render_gitgraph_with_theme_and_engine(
        source,
        &theme,
        engine(),
        "git-unmeasurable-role-size",
    ) {
        Ok(_) => panic!("unmeasurable GitGraph role size must fail closed"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::GIT_GRAPH, 1))
    );

    let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
        source,
        &theme,
        engine(),
        "git-unmeasurable-role-size-best-effort",
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("best-effort GitGraph keeps the Mermaid CSS value");
    let stylesheet = gitgraph_stylesheet(rendered.svg());
    assert_eq!(
        css_property(
            &gitgraph_css_rule(
                &stylesheet,
                "#git-unmeasurable-role-size-best-effort .commit-label"
            ),
            "font-size"
        ),
        Some("calc(10px + 1em)")
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn gitgraph_config_owned_size_does_not_inherit_unmeasurable_role_residuals() {
    let theme = gitgraph_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("GitGraphTypedOwner").unwrap())
            .with_font_size_px(29.0)
            .unwrap(),
    );
    for (property, class) in [
        ("commitLabelFontSize", "commit-label"),
        ("tagLabelFontSize", "tag-label"),
    ] {
        let mut config = json!({"themeVariables": {"fontSize": "31px"}});
        config["themeVariables"][property] = json!("calc(1em)");
        render_source_and_site_config_cases(
            "gitGraph\n  commit id: \"1\" tag: \"v1\"\n",
            config,
            &theme,
            "git-config-owned-role-size",
            |owner, rendered| {
                let stylesheet = gitgraph_stylesheet(rendered.svg());
                let selector = format!("#git-config-owned-role-size-{owner}");
                let root = gitgraph_css_rule(&stylesheet, &selector);
                assert_eq!(css_property(&root, "font-size"), Some("31px"));
                assert_eq!(
                    css_property(&root, "font-family"),
                    Some("GitGraphTypedOwner")
                );
                let role = gitgraph_css_rule(&stylesheet, &format!("{selector} .{class}"));
                assert_eq!(
                    css_property(&role, "font-size"),
                    Some("calc(1em)"),
                    "{owner}/{property}: {role}"
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                assert!(document.descendants().any(|node| has_class(&node, class)));
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.applied_count(), 1, "{owner}/{property}");
                assert_eq!(evidence.not_applicable_count(), 1, "{owner}/{property}");
                assert_eq!(evidence.theme_residual_count(), 0, "{owner}/{property}");
            },
        );
    }
}

#[test]
fn gitgraph_accessibility_metadata_does_not_prove_visual_typography() {
    let theme = gitgraph_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("GitGraphAccessibleOnly").expect("valid GitGraph font stack"),
    ));
    let rendered = render_gitgraph_with_theme_and_engine(
        r#"gitGraph:
accTitle: Accessibility only
accDescr: This text is not a visual terminal
commit id:"1"
"#,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "gitGraph": {
                "showBranches": false,
                "showCommitLabel": false
            }
        }))),
        "git-accessibility-only",
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid GitGraph SVG");
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("title") && node.text() == Some("Accessibility only"))
    );
    assert!(
        !document
            .descendants()
            .any(|node| has_class(&node, "gitTitleText"))
    );
    assert!(
        !document
            .descendants()
            .any(|node| has_class(&node, "branch-label0"))
    );
    assert!(
        !document
            .descendants()
            .any(|node| has_class(&node, "commit-label"))
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gitgraph_typography_config_ownership_is_property_local() {
    let theme = gitgraph_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(
                FontStack::new(["GitGraphTypedOwner", "monospace"])
                    .expect("valid GitGraph typed font stack"),
            )
            .with_font_size_px(23.0)
            .expect("valid GitGraph typed font size"),
    );

    render_source_and_site_config_cases(
        TWO_BRANCHES,
        json!({
            "fontFamily": "RootFallback",
            "fontSize": 41
        }),
        &theme,
        "git-root-family-owner",
        |owner, rendered| {
            let selector = format!("#git-root-family-owner-{owner}");
            let root = gitgraph_css_rule(&gitgraph_stylesheet(rendered.svg()), &selector);
            assert_eq!(css_property(&root, "font-family"), Some("RootFallback"));
            assert_eq!(
                css_property(&root, "font-size"),
                Some("23px"),
                "root fontSize must not own or suppress GitGraph FontSize"
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 2, "owner={owner}");
            assert_eq!(evidence.accounted_count(), 2, "owner={owner}");
            assert_eq!(evidence.applied_count(), 1, "owner={owner}");
            assert_eq!(evidence.not_applicable_count(), 1, "owner={owner}");
            assert_eq!(evidence.theme_residual_count(), 0, "owner={owner}");
        },
    );

    render_source_and_site_config_cases(
        TWO_BRANCHES,
        json!({
            "fontFamily": "RootOwner",
            "fontSize": 41,
            "themeVariables": { "fontFamily": "NestedOwner" }
        }),
        &theme,
        "git-family-owner",
        |owner, rendered| {
            let selector = format!("#git-family-owner-{owner}");
            let root = gitgraph_css_rule(&gitgraph_stylesheet(rendered.svg()), &selector);
            assert_eq!(css_property(&root, "font-family"), Some("NestedOwner"));
            assert_eq!(
                css_property(&root, "font-size"),
                Some("23px"),
                "root fontSize must not own or suppress GitGraph FontSize"
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 2, "owner={owner}");
            assert_eq!(evidence.accounted_count(), 2, "owner={owner}");
            assert_eq!(evidence.applied_count(), 1, "owner={owner}");
            assert_eq!(evidence.not_applicable_count(), 1, "owner={owner}");
            assert_eq!(evidence.theme_residual_count(), 0, "owner={owner}");
        },
    );

    render_source_and_site_config_cases(
        TWO_BRANCHES,
        json!({
            "fontSize": 41,
            "themeVariables": { "fontSize": "31px" }
        }),
        &theme,
        "git-size-owner",
        |owner, rendered| {
            let selector = format!("#git-size-owner-{owner}");
            let root = gitgraph_css_rule(&gitgraph_stylesheet(rendered.svg()), &selector);
            assert_eq!(
                css_property(&root, "font-family"),
                Some("GitGraphTypedOwner, monospace")
            );
            assert_eq!(css_property(&root, "font-size"), Some("31px"));
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 2, "owner={owner}");
            assert_eq!(evidence.accounted_count(), 2, "owner={owner}");
            assert_eq!(evidence.applied_count(), 1, "owner={owner}");
            assert_eq!(evidence.not_applicable_count(), 1, "owner={owner}");
            assert_eq!(evidence.theme_residual_count(), 0, "owner={owner}");
        },
    );
}

#[test]
fn gitgraph_unsupported_typography_sibling_keeps_direct_properties() {
    let theme = gitgraph_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(
                FontStack::single("GitGraphSuppressed").expect("valid GitGraph font stack"),
            )
            .with_font_size_px(23.0)
            .expect("valid GitGraph font size")
            .with_font_weight(700)
            .expect("valid unsupported GitGraph font weight"),
    );
    let error = match try_render_gitgraph_with_theme_and_engine(
        TWO_BRANCHES,
        &theme,
        Engine::new(),
        "git-unsupported-typography",
    ) {
        Ok(_) => panic!("unsupported GitGraph typography sibling must fail closed"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::GIT_GRAPH, 1))
    );

    let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
        TWO_BRANCHES,
        &theme,
        Engine::new(),
        "git-unsupported-typography-best-effort",
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("best-effort GitGraph must retain baseline typography");
    let root = gitgraph_css_rule(
        &gitgraph_stylesheet(rendered.svg()),
        "#git-unsupported-typography-best-effort",
    );
    assert_eq!(
        css_property(&root, "font-family"),
        Some("GitGraphSuppressed")
    );
    assert_eq!(css_property(&root, "font-size"), Some("23px"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 3);
    assert_eq!(evidence.accounted_count(), 3);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn gitgraph_direct_font_stack_is_not_bounded_by_the_legacy_assignment_ceiling() {
    let stack =
        FontStack::new((0..32).map(|index| format!("GitGraphFont{index}{}", "x".repeat(180))))
            .expect("valid large GitGraph font stack");
    let expected_css = stack.as_css();
    assert!(expected_css.len() > 4 * 1024);
    let theme = gitgraph_typography_theme(ThemeTextStyle::default().with_font_stack(stack));
    let rendered = render_gitgraph_with_theme_and_engine(
        TWO_BRANCHES,
        &theme,
        Engine::new(),
        "git-large-stack",
    );
    let root = gitgraph_css_rule(&gitgraph_stylesheet(rendered.svg()), "#git-large-stack");
    assert_eq!(
        css_property(&root, "font-family"),
        Some(expected_css.as_str())
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

fn has_class(node: &roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|value| value == class))
}

fn branch_lines<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
) -> Vec<roxmltree::Node<'a, 'input>> {
    document
        .descendants()
        .filter(|node| node.has_tag_name("line") && has_class(node, "branch"))
        .collect()
}

fn render_source_and_site_config_cases(
    source: &str,
    config: serde_json::Value,
    theme: &DiagramTheme,
    diagram_id_prefix: &str,
    mut assert_case: impl FnMut(&str, family::RenderedFamilySvg),
) {
    let source_config = serde_json::to_string(&config).expect("serialize GitGraph source config");
    let (frontmatter, body) = if let Some(rest) = source.strip_prefix("---\n") {
        let end = rest.find("\n---\n").expect("closed test frontmatter");
        source.split_at(4 + end + 5)
    } else {
        ("", source)
    };
    let source_owned = format!("{frontmatter}%%{{init: {source_config}}}%%\n{body}");
    let cases = [
        (
            "site",
            source.to_string(),
            Engine::new().with_site_config(MermaidConfig::from_value(config)),
        ),
        (
            "source",
            source_owned,
            Engine::new().with_site_config(MermaidConfig::from_value(json!({ "secure": [] }))),
        ),
    ];

    for (owner, source, engine) in cases {
        let diagram_id = format!("{diagram_id_prefix}-{owner}");
        let rendered = render_gitgraph_with_theme_and_engine(&source, theme, engine, &diagram_id);
        assert_case(owner, rendered);
    }
}

#[test]
fn gitgraph_edge_stroke_reaches_every_visible_branch_line_without_touching_arrows() {
    for (selector, variant) in [
        ("unqualified", None),
        ("default", Some(ThemeVariant::Default)),
    ] {
        for (case, stroke, expected_stroke) in [
            (
                "solid",
                CanvasPaint::solid("#123456").expect("valid GitGraph Edge stroke"),
                "#123456",
            ),
            ("transparent", CanvasPaint::Transparent, "transparent"),
        ] {
            let theme = gitgraph_edge_stroke_theme_variant(stroke, variant);
            let rendered = render_gitgraph_with_theme_and_engine(
                TWO_BRANCHES,
                &theme,
                Engine::new(),
                &format!("git-edge-{selector}-{case}"),
            );
            let document =
                roxmltree::Document::parse(rendered.svg()).expect("valid GitGraph Edge stroke SVG");
            let branches = branch_lines(&document);

            assert_eq!(branches.len(), 2, "{selector}/{case}");
            for branch in branches {
                assert_eq!(
                    branch.attribute("style"),
                    Some(format!("stroke:{expected_stroke};").as_str()),
                    "{selector}/{case}: every actual .branch line must carry the direct terminal stroke",
                );
            }
            assert!(
                document
                    .descendants()
                    .filter(|node| node.has_tag_name("path") && has_class(node, "arrow"))
                    .all(|arrow| arrow
                        .attribute("style")
                        .is_none_or(|style| !style.contains(expected_stroke))),
                "{selector}/{case}: .arrowN belongs to the Node palette and must not count as Edge.stroke",
            );

            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1, "{selector}/{case}");
            assert_eq!(evidence.applied_count(), 1, "{selector}/{case}");
            assert_eq!(evidence.not_applicable_count(), 0, "{selector}/{case}");
            assert_eq!(evidence.theme_residual_count(), 0, "{selector}/{case}");
        }
    }
}

#[test]
fn gitgraph_edge_stroke_and_node_palette_keep_independent_terminal_receipts() {
    let palette = OrdinalPalette::new([
        ThemeColorValue::parse("#abcdef").expect("valid GitGraph palette color"),
        ThemeColorValue::parse("#fedcba").expect("valid GitGraph palette color"),
    ])
    .expect("non-empty GitGraph Node palette");
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_ordinal_palette(ThemeTarget::Node, palette)
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Edge,
                            ThemeStylePatch::default().with_stroke(
                                CanvasPaint::solid("#123456").expect("valid GitGraph Edge stroke"),
                            ),
                        )
                        .for_family(DiagramFamilyId::GIT_GRAPH),
                    ),
            ),
        )
        .expect("compile combined GitGraph theme");
    let rendered = render_gitgraph_with_theme_and_engine(
        TWO_BRANCHES,
        &theme,
        Engine::new(),
        "git-palette-and-edge",
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid combined GitGraph SVG");
    let css = gitgraph_stylesheet(rendered.svg());

    assert!(
        branch_lines(&document)
            .iter()
            .all(|branch| branch.attribute("style") == Some("stroke:#123456;"))
    );
    assert!(css.contains("#git-palette-and-edge .commit0{stroke:#abcdef;fill:#abcdef;}"));

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gitgraph_retained_compatibility_routes_reach_writer_consumed_terminals() {
    let source = r#"---
title: GitGraph compatibility surfaces
---
gitGraph
  commit id: "1" tag: "v1"
"#;
    let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
        source,
        &gitgraph_legacy_paint_theme(),
        Engine::new(),
        "git-legacy-paint",
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("render GitGraph compatibility surfaces");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid GitGraph SVG");
    let css = gitgraph_stylesheet(rendered.svg());

    for expected in [
        "#git-legacy-paint .branch{stroke-width:1;stroke:#1c1d1e;",
        "#git-legacy-paint .commit-label{font-size:10px;fill:#1f2021;",
        "#git-legacy-paint .commit-label-bkg{font-size:10px;fill:#222324;",
        "#git-legacy-paint .tag-label{font-size:10px;fill:#161718;}",
        "#git-legacy-paint .tag-label-bkg{fill:#101112;stroke:#131415;",
        "#git-legacy-paint .gitTitleText{text-anchor:middle;font-size:18px;fill:#191a1b;}",
    ] {
        assert!(css.contains(expected), "missing terminal CSS: {expected}");
    }
    for class in [
        "branch",
        "commit-label",
        "commit-label-bkg",
        "tag-label",
        "tag-label-bkg",
        "gitTitleText",
    ] {
        assert!(
            document.descendants().any(|node| has_class(&node, class)),
            "GitGraph must emit a terminal occurrence for {class}"
        );
    }
}

#[test]
fn gitgraph_edge_label_background_fill_is_typed_and_reaches_commit_label_rects() {
    let solid = CanvasPaint::solid("#123456").expect("valid GitGraph label background");
    for (variant, diagram_id) in [
        (None, "git-edge-label-background"),
        (
            Some(ThemeVariant::Default),
            "git-edge-label-background-default",
        ),
    ] {
        let mut rule = ThemeRule::new(
            ThemeTarget::EdgeLabelBackground,
            ThemeStylePatch::default().with_fill(solid.clone()),
        )
        .for_family(DiagramFamilyId::GIT_GRAPH);
        if let Some(variant) = variant {
            rule = rule.with_variant(variant);
        }
        let theme = gitgraph_edge_rules_theme([rule]);
        let rendered =
            render_gitgraph_with_theme_and_engine(TWO_BRANCHES, &theme, Engine::new(), diagram_id);
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid GitGraph SVG");
        let stylesheet = gitgraph_stylesheet(rendered.svg());
        let commit_label_rule =
            gitgraph_css_rule(&stylesheet, &format!("#{diagram_id} .commit-label-bkg"));

        assert_eq!(
            css_property_winner(&commit_label_rule, "fill"),
            Some("#123456")
        );
        assert!(
            document
                .descendants()
                .any(|node| has_class(&node, "commit-label-bkg"))
        );

        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn gitgraph_commit_background_accounts_for_each_winning_property() {
    let fill = || ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
    let font = |mut patch: ThemeStylePatch| {
        patch.typography.font_stack =
            Specified::Value(FontStack::single("UnsupportedCommitBackgroundFont").unwrap());
        patch
    };
    let rule = |patch| {
        ThemeRule::new(ThemeTarget::EdgeLabelBackground, patch)
            .for_family(DiagramFamilyId::GIT_GRAPH)
    };
    let clear = || {
        let mut patch = ThemeStylePatch::default();
        patch.paint.fill = Specified::Clear;
        rule(patch)
    };
    let ordinal = || rule(fill()).with_ordinal(OrdinalSelector::exact(1).unwrap());
    for (case, rules, config, applied, not_applicable, residuals) in [
        (
            "font-only",
            vec![rule(font(ThemeStylePatch::default()))],
            json!({}),
            0,
            0,
            1,
        ),
        (
            "stroke-only",
            vec![rule(
                ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#654321").unwrap()),
            )],
            json!({}),
            0,
            0,
            1,
        ),
        ("mixed", vec![rule(font(fill()))], json!({}), 0, 0, 1),
        (
            "config-owned-fill",
            vec![rule(font(fill()))],
            json!({"themeVariables": {"commitLabelBackground": "#abcdef"}}),
            0,
            0,
            1,
        ),
        (
            "generated-fill",
            vec![rule(font(fill()))],
            json!({"theme": "neo"}),
            0,
            0,
            1,
        ),
        (
            "later-fill",
            vec![rule(font(fill())), rule(fill())],
            json!({}),
            1,
            0,
            1,
        ),
        (
            "later-font",
            vec![
                rule(font(ThemeStylePatch::default())),
                rule(font(ThemeStylePatch::default())),
            ],
            json!({}),
            0,
            1,
            1,
        ),
        (
            "other-variant",
            vec![rule(font(fill())).with_variant(ThemeVariant::Active)],
            json!({}),
            0,
            1,
            0,
        ),
        ("ordinal", vec![ordinal()], json!({}), 0, 0, 1),
        (
            "shadowed-ordinal",
            vec![ordinal(), rule(fill())],
            json!({}),
            1,
            1,
            0,
        ),
        ("clear", vec![rule(fill()), clear()], json!({}), 0, 1, 1),
        (
            "config-owned-clear",
            vec![rule(fill()), clear()],
            json!({"themeVariables": {"commitLabelBackground": "#abcdef"}}),
            0,
            2,
            0,
        ),
        (
            "hidden",
            vec![rule(font(fill()))],
            json!({"gitGraph": {"showCommitLabel": false}}),
            0,
            1,
            0,
        ),
    ] {
        let theme = gitgraph_edge_rules_theme(rules);
        let engine = || Engine::new().with_site_config(MermaidConfig::from_value(config.clone()));
        let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
            TWO_BRANCHES,
            &theme,
            engine(),
            &format!("git-background-{case}"),
            ThemePortabilityRequirement::BestEffort,
        )
        .expect("unsupported background properties must preserve best-effort rendering");
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid GitGraph SVG");
        assert_eq!(
            document
                .descendants()
                .any(|node| has_class(&node, "commit-label-bkg")),
            case != "hidden",
            "{case}"
        );
        if matches!(case, "mixed" | "later-fill" | "shadowed-ordinal") {
            let css = gitgraph_stylesheet(rendered.svg());
            let background =
                gitgraph_css_rule(&css, &format!("#git-background-{case} .commit-label-bkg"));
            assert_eq!(
                css_property_winner(&background, "fill"),
                Some("#123456"),
                "{case}: best effort preserves the supported fill"
            );
        }
        if case == "clear" {
            let css = gitgraph_stylesheet(rendered.svg());
            let background = gitgraph_css_rule(&css, "#git-background-clear .commit-label-bkg");
            assert_ne!(
                css_property_winner(&background, "fill"),
                Some("#123456"),
                "Clear must not revive the earlier assignment"
            );
        }
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), applied, "{case}");
        assert_eq!(evidence.not_applicable_count(), not_applicable, "{case}");
        assert_eq!(evidence.theme_residual_count(), residuals, "{case}");
        let strict = try_render_gitgraph_with_theme_engine_and_requirement(
            TWO_BRANCHES,
            &theme,
            engine(),
            &format!("git-background-strict-{case}"),
            ThemePortabilityRequirement::RequirePortable,
        );
        if residuals == 0 {
            strict.expect("inactive or fully supported rules have no unsupported obligation");
        } else {
            let error = match strict {
                Ok(_) => {
                    panic!("{case}: winning unsupported siblings must fail strict portability")
                }
                Err(error) => error,
            };
            assert_eq!(
                error.unverified_family_theme(),
                Some((DiagramFamilyId::GIT_GRAPH, residuals)),
                "{case}"
            );
        }
    }
}

#[test]
fn gitgraph_edge_label_background_is_not_applicable_without_visible_commit_labels() {
    let theme = gitgraph_edge_rules_theme([ThemeRule::new(
        ThemeTarget::EdgeLabelBackground,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").expect("valid GitGraph label background")),
    )
    .for_family(DiagramFamilyId::GIT_GRAPH)]);

    let rendered = render_gitgraph_with_theme_and_engine(
        r#"gitGraph
  commit id: "1"
"#,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "gitGraph": { "showCommitLabel": false }
        }))),
        "git-edge-label-background-hidden",
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid GitGraph SVG");
    assert!(
        !document
            .descendants()
            .any(|node| has_class(&node, "commit-label-bkg")),
        "a hidden commit label must not emit a background terminal"
    );
    assert!(!rendered.svg().contains("#123456"));

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gitgraph_edge_label_background_yields_to_color_generated_theme_ownership() {
    let theme = gitgraph_edge_rules_theme([ThemeRule::new(
        ThemeTarget::EdgeLabelBackground,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").expect("valid GitGraph label background")),
    )
    .for_family(DiagramFamilyId::GIT_GRAPH)]);

    render_source_and_site_config_cases(
        TWO_BRANCHES,
        json!({ "theme": "neo" }),
        &theme,
        "git-edge-label-background-neo",
        |owner, rendered| {
            let diagram_id = format!("git-edge-label-background-neo-{owner}");
            let document = roxmltree::Document::parse(rendered.svg()).expect("valid GitGraph SVG");
            let stylesheet = gitgraph_stylesheet(rendered.svg());
            let commit_label_rule =
                gitgraph_css_rule(&stylesheet, &format!("#{diagram_id} .commit-label-bkg"));

            assert_eq!(
                css_property_winner(&commit_label_rule, "fill"),
                Some("transparent"),
                "{owner}: Mermaid's color-generated theme owns the commit label background"
            );
            assert!(
                document
                    .descendants()
                    .any(|node| has_class(&node, "commit-label-bkg")),
                "{owner}: the not-applicable route must still have a real source-owned terminal"
            );

            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1, "owner={owner}");
            assert_eq!(evidence.applied_count(), 0, "owner={owner}");
            assert_eq!(evidence.not_applicable_count(), 1, "owner={owner}");
            assert_eq!(evidence.theme_residual_count(), 0, "owner={owner}");
        },
    );
}

#[test]
fn gitgraph_edge_label_background_yields_to_explicit_config_ownership() {
    let theme = gitgraph_edge_rules_theme([ThemeRule::new(
        ThemeTarget::EdgeLabelBackground,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").expect("valid GitGraph label background")),
    )
    .for_family(DiagramFamilyId::GIT_GRAPH)]);

    render_source_and_site_config_cases(
        TWO_BRANCHES,
        json!({ "themeVariables": { "commitLabelBackground": "#abcdef" } }),
        &theme,
        "git-edge-label-background-config",
        |owner, rendered| {
            let diagram_id = format!("git-edge-label-background-config-{owner}");
            let stylesheet = gitgraph_stylesheet(rendered.svg());
            let commit_label_rule =
                gitgraph_css_rule(&stylesheet, &format!("#{diagram_id} .commit-label-bkg"));

            assert_eq!(
                css_property_winner(&commit_label_rule, "fill"),
                Some("#abcdef"),
                "{owner}: an explicit Mermaid config owns this property"
            );

            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1, "owner={owner}");
            assert_eq!(evidence.applied_count(), 0, "owner={owner}");
            assert_eq!(evidence.not_applicable_count(), 1, "owner={owner}");
            assert_eq!(evidence.theme_residual_count(), 0, "owner={owner}");
        },
    );
}

#[test]
fn gitgraph_unsupported_routes_follow_real_terminal_occurrences() {
    let source = r#"---
title: GitGraph unsupported surfaces
---
gitGraph
  commit id: "1" tag: "v1"
"#;
    let solid = || CanvasPaint::solid("#123456").expect("valid GitGraph unsupported paint");
    let visible_title = gitgraph_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Title,
        ThemeStylePatch::default().with_fill(solid()),
    )
    .for_family(DiagramFamilyId::GIT_GRAPH)]);
    let portable_error = match try_render_gitgraph_with_theme_engine_and_requirement(
        source,
        &visible_title,
        Engine::new(),
        "git-unsupported-title-fill",
        ThemePortabilityRequirement::RequirePortable,
    ) {
        Ok(_) => panic!("visible unsupported GitGraph Title must fail closed"),
        Err(error) => error,
    };
    assert_eq!(
        portable_error.unverified_family_theme(),
        Some((DiagramFamilyId::GIT_GRAPH, 1))
    );

    let absent_cases = [
        (
            "marker-fill",
            ThemeRule::new(
                ThemeTarget::Marker,
                ThemeStylePatch::default().with_fill(solid()),
            ),
        ),
        (
            "marker-stroke",
            ThemeRule::new(
                ThemeTarget::Marker,
                ThemeStylePatch::default().with_stroke(solid()),
            ),
        ),
        (
            "cluster-fill",
            ThemeRule::new(
                ThemeTarget::Cluster,
                ThemeStylePatch::default().with_fill(solid()),
            ),
        ),
        (
            "cluster-stroke",
            ThemeRule::new(
                ThemeTarget::Cluster,
                ThemeStylePatch::default().with_stroke(solid()),
            ),
        ),
        (
            "cluster-label-fill",
            ThemeRule::new(
                ThemeTarget::ClusterLabel,
                ThemeStylePatch::default().with_fill(solid()),
            ),
        ),
    ];

    for (case, rule) in absent_cases {
        let theme = gitgraph_edge_rules_theme([rule.for_family(DiagramFamilyId::GIT_GRAPH)]);
        let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
            source,
            &theme,
            Engine::new(),
            &format!("git-unsupported-{case}"),
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("absent GitGraph terminals must make unsupported rules not applicable");
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "case={case}");
        assert_eq!(evidence.applied_count(), 0, "case={case}");
        assert_eq!(evidence.not_applicable_count(), 1, "case={case}");
        assert_eq!(evidence.theme_residual_count(), 0, "case={case}");
    }
}

#[test]
fn gitgraph_edge_stroke_respects_commit_line_and_line_color_owners() {
    let theme = gitgraph_edge_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid GitGraph Edge stroke"),
    );

    let assert_owned = |owner: &str, rendered: family::RenderedFamilySvg| {
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid owned GitGraph Edge SVG");
        let branches = branch_lines(&document);
        let css = gitgraph_stylesheet(rendered.svg());

        assert_eq!(branches.len(), 2, "{owner}");
        assert!(
            branches.iter().all(|branch| branch
                .attribute("style")
                .is_none_or(|style| !style.contains("#123456"))),
            "{owner}: the typed stroke must not overwrite the Mermaid owner",
        );
        assert!(
            css.contains("stroke:#fedcba;stroke-dasharray:"),
            "{owner}: the Mermaid branch source must remain terminal",
        );

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "{owner}");
        assert_eq!(evidence.applied_count(), 0, "{owner}");
        assert_eq!(evidence.not_applicable_count(), 1, "{owner}");
        assert_eq!(evidence.theme_residual_count(), 0, "{owner}");
    };

    let site_commit_line = render_gitgraph_with_theme_and_engine(
        TWO_BRANCHES,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "themeVariables": { "commitLineColor": "#fedcba" }
        }))),
        "git-edge-commit-line-site-owner",
    );
    assert_owned("commit-line/site", site_commit_line);

    render_source_and_site_config_cases(
        TWO_BRANCHES,
        json!({ "themeVariables": { "lineColor": "#fedcba" } }),
        &theme,
        "git-edge-line-fallback-owner",
        |owner, rendered| assert_owned(&format!("line-fallback/{owner}"), rendered),
    );
}

#[test]
fn gitgraph_source_commit_line_color_is_sanitized_before_terminal_ownership() {
    let theme = gitgraph_edge_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid GitGraph Edge stroke"),
    );
    let rendered = render_gitgraph_with_theme_and_engine(
        &format!(
            "%%{{init: {}}}%%\n{TWO_BRANCHES}",
            serde_json::to_string(&json!({ "themeVariables": { "commitLineColor": "#fedcba" } }))
                .expect("serialize GitGraph source config")
        ),
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({ "secure": [] }))),
        "git-edge-source-commit-line-sanitized",
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid GitGraph Edge SVG");

    assert!(
        branch_lines(&document)
            .iter()
            .all(|branch| branch.attribute("style") == Some("stroke:#123456;"))
    );
    assert!(!gitgraph_stylesheet(rendered.svg()).contains("#fedcba"));

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gitgraph_edge_stroke_ignores_line_color_when_commit_line_color_is_the_terminal_source() {
    let theme = gitgraph_edge_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid GitGraph Edge stroke"),
    );
    render_source_and_site_config_cases(
        TWO_BRANCHES,
        json!({
            "theme": "redux",
            "themeVariables": { "lineColor": "#fedcba" }
        }),
        &theme,
        "git-edge-redux-line-owner",
        |owner, rendered| {
            let document =
                roxmltree::Document::parse(rendered.svg()).expect("valid Redux GitGraph Edge SVG");
            let branches = branch_lines(&document);

            assert_eq!(branches.len(), 2, "{owner}");
            assert!(
                branches
                    .iter()
                    .all(|branch| branch.attribute("style") == Some("stroke:#123456;")),
                "{owner}: lineColor must not suppress the direct stroke when commitLineColor is the selected Mermaid source",
            );

            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1, "{owner}");
            assert_eq!(evidence.applied_count(), 1, "{owner}");
            assert_eq!(evidence.not_applicable_count(), 0, "{owner}");
            assert_eq!(evidence.theme_residual_count(), 0, "{owner}");
        },
    );
}

#[test]
fn gitgraph_edge_stroke_is_not_applicable_when_branches_are_hidden() {
    let theme = gitgraph_edge_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid GitGraph Edge stroke"),
    );
    let rendered = render_gitgraph_with_theme_and_engine(
        TWO_BRANCHES,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "gitGraph": { "showBranches": false }
        }))),
        "git-edge-hidden-branches",
    );
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid hidden-branch GitGraph SVG");

    assert!(branch_lines(&document).is_empty());
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gitgraph_edge_stroke_rejects_ordinal_variant_clear_gradient_and_pattern_routes() {
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(
                0.0,
                ThemeColorValue::parse("#123456").expect("valid GitGraph gradient start"),
            )
            .expect("valid GitGraph gradient stop"),
            GradientStop::new(
                1.0,
                ThemeColorValue::parse("#abcdef").expect("valid GitGraph gradient end"),
            )
            .expect("valid GitGraph gradient stop"),
        ],
    )
    .expect("valid GitGraph gradient");
    let pattern = PatternSpec::new(
        PatternKind::Grid,
        8.0,
        8.0,
        ThemeColorValue::parse("#123456").expect("valid GitGraph pattern color"),
    )
    .expect("valid GitGraph pattern");
    let mut clear = ThemeStylePatch::default();
    clear.stroke.paint = Specified::Clear;
    let solid = || {
        ThemeStylePatch::default()
            .with_stroke(CanvasPaint::solid("#123456").expect("valid GitGraph stroke"))
    };
    let unsupported_cases = [
        ThemeRule::new(ThemeTarget::Edge, solid())
            .with_ordinal(OrdinalSelector::exact(1).expect("valid GitGraph edge ordinal")),
        ThemeRule::new(ThemeTarget::Edge, clear),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::LinearGradient(gradient)),
        ),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::Pattern(pattern)),
        ),
    ];

    let default_theme = gitgraph_edge_stroke_theme_variant(
        CanvasPaint::solid("#123456").expect("valid GitGraph default Edge stroke"),
        Some(ThemeVariant::Default),
    );
    let default_rendered = try_render_gitgraph_with_theme_and_engine(
        TWO_BRANCHES,
        &default_theme,
        Engine::new(),
        "git-edge-qualified-default",
    )
    .expect("explicit Default GitGraph Edge stroke must use the typed terminal");
    let default_document =
        roxmltree::Document::parse(default_rendered.svg()).expect("valid GitGraph default SVG");
    assert!(
        branch_lines(&default_document)
            .iter()
            .all(|branch| { branch.attribute("style") == Some("stroke:#123456;") })
    );
    let default_evidence =
        merman_render::__private::family_evidence(default_rendered.into_completion().report());
    assert_eq!(default_evidence.applied_count(), 1);
    assert_eq!(default_evidence.not_applicable_count(), 0);
    assert_eq!(default_evidence.theme_residual_count(), 0);

    for rule in unsupported_cases {
        let theme = gitgraph_edge_rules_theme([rule.for_family(DiagramFamilyId::GIT_GRAPH)]);
        let error = match try_render_gitgraph_with_theme_and_engine(
            TWO_BRANCHES,
            &theme,
            Engine::new(),
            "git-edge-unsupported",
        ) {
            Ok(_) => panic!("unsupported GitGraph Edge stroke route must fail closed"),
            Err(error) => error,
        };
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::GIT_GRAPH, 1))
        );
    }
}

#[test]
fn gitgraph_qualified_edge_stroke_accounts_only_real_default_occurrence_winners() {
    let solid = |color| {
        ThemeStylePatch::default()
            .with_stroke(CanvasPaint::solid(color).expect("valid GitGraph stroke"))
    };
    let theme = gitgraph_edge_rules_theme([
        ThemeRule::new(ThemeTarget::Edge, solid("#111111"))
            .with_variant(ThemeVariant::Default)
            .for_family(DiagramFamilyId::GIT_GRAPH),
        ThemeRule::new(ThemeTarget::Edge, solid("#222222"))
            .with_variant(ThemeVariant::Default)
            .for_family(DiagramFamilyId::GIT_GRAPH),
        ThemeRule::new(ThemeTarget::Edge, solid("#333333"))
            .with_variant(ThemeVariant::Odd)
            .with_ordinal(OrdinalSelector::exact(1).expect("valid GitGraph edge ordinal"))
            .for_family(DiagramFamilyId::GIT_GRAPH),
    ]);
    let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
        TWO_BRANCHES,
        &theme,
        Engine::new(),
        "git-edge-qualified-winners",
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("render qualified GitGraph edge winners");

    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid qualified GitGraph Edge SVG");
    assert!(
        branch_lines(&document)
            .iter()
            .all(|branch| branch.attribute("style") == Some("stroke:#222222;")),
        "the final Default winner must own every typed branch stroke",
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 3);
    assert_eq!(evidence.accounted_count(), 3);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn gitgraph_node_palette_reaches_terminal_commit_arrow_and_branch_label_surfaces() {
    let theme = gitgraph_node_palette_theme(&["#123456", "transparent"]);
    let rendered =
        render_gitgraph_with_theme_and_engine(TWO_BRANCHES, &theme, Engine::new(), "git-palette");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid GitGraph palette SVG");
    let css = gitgraph_stylesheet(rendered.svg());

    let solid_rule = concat!(
        "#git-palette .commit0{stroke:#123456;fill:#123456;}",
        "#git-palette .arrow0{stroke:#123456;}",
        "#git-palette .label0{fill:#123456;}"
    );
    let transparent_css = ThemeColorValue::parse("transparent")
        .expect("valid transparent GitGraph color")
        .as_css();
    let transparent_rule = format!(
        "#git-palette .commit1{{stroke:{transparent_css};fill:{transparent_css};}}#git-palette .arrow1{{stroke:{transparent_css};}}#git-palette .label1{{fill:{transparent_css};}}"
    );
    let baseline_merge_rule = css
        .find("#git-palette .commit-merge{")
        .expect("Mermaid GitGraph merge baseline rule");
    let direct_solid_rule = css
        .rfind(solid_rule)
        .expect("direct GitGraph solid palette rule");
    let direct_transparent_rule = css
        .rfind(&transparent_rule)
        .unwrap_or_else(|| panic!("direct GitGraph transparent palette rule missing from: {css}"));
    assert!(baseline_merge_rule < direct_solid_rule && direct_solid_rule < direct_transparent_rule);

    for class in ["commit0", "commit1", "arrow0", "arrow1", "label0", "label1"] {
        assert!(
            document.descendants().any(|node| has_class(&node, class)),
            "GitGraph must emit a terminal occurrence for {class}"
        );
    }

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gitgraph_node_palette_strokes_survive_static_node_stroke_on_its_actual_tag_surface() {
    let theme = gitgraph_node_palette_with_static_stroke_theme(
        &["#123456", "#abcdef"],
        CanvasPaint::solid("#654321").expect("valid GitGraph static Node stroke"),
    );
    let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
        r#"gitGraph
  commit id: "1" tag: "v1"
  branch develop
  checkout develop
  commit id: "2"
  checkout main
  commit id: "3"
"#,
        &theme,
        Engine::new(),
        "git-palette-static-stroke",
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("best-effort GitGraph should retain the direct palette beside compatibility stroke");
    let document = roxmltree::Document::parse(rendered.svg())
        .expect("valid GitGraph palette and static stroke SVG");
    let css = gitgraph_stylesheet(rendered.svg());

    for (slot, color) in [(0, "#123456"), (1, "#abcdef")] {
        assert!(
            css.contains(&format!(
                "#git-palette-static-stroke .commit{slot}{{stroke:{color};fill:{color};}}"
            )),
            "the palette must retain both commit properties for slot {slot}: {css}"
        );
        assert!(
            css.contains(&format!(
                "#git-palette-static-stroke .arrow{slot}{{stroke:{color};}}"
            )),
            "the palette must retain the arrow stroke for slot {slot}: {css}"
        );
        assert!(
            css.contains(&format!(
                "#git-palette-static-stroke .label{slot}{{fill:{color};}}"
            )),
            "the palette must retain the branch-label fill for slot {slot}: {css}"
        );
        assert!(
            !css.contains(&format!(
                "#git-palette-static-stroke .commit{slot}{{stroke:#654321;"
            )),
            "the static Node stroke must not claim commit slot {slot}: {css}"
        );
    }

    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("polygon") && has_class(&node, "tag-label-bkg")),
        "the static Node stroke fixture must contain its terminal tag surface"
    );
    let tag_rule = css
        .split_once("#git-palette-static-stroke .tag-label-bkg{")
        .and_then(|(_, remaining)| remaining.split_once('}'))
        .map(|(declarations, _)| declarations)
        .expect("GitGraph tag-label background rule");
    assert!(
        tag_rule.contains("stroke:#654321;"),
        "the compatibility-owned static Node stroke must remain on its tag surface: {tag_rule}"
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gitgraph_node_palette_respects_source_and_site_git_slot_owners() {
    let theme = gitgraph_node_palette_theme(&["#123456", "#654321"]);
    let cases = [
        (
            "site",
            TWO_BRANCHES,
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "themeVariables": {
                    "git0": "#fedcba",
                    "gitInv0": "#abcdef",
                    "gitBranchLabel0": "#112233"
                }
            }))),
        ),
        (
            "source",
            SOURCE_OWNED_TWO_BRANCHES,
            Engine::new().with_site_config(MermaidConfig::from_value(json!({ "secure": [] }))),
        ),
    ];

    for (owner, source, engine) in cases {
        let diagram_id = format!("git-owner-{owner}");
        let rendered = render_gitgraph_with_theme_and_engine(source, &theme, engine, &diagram_id);
        let css = gitgraph_stylesheet(rendered.svg());
        let direct_slot_zero = format!(
            "#{diagram_id} .commit0{{stroke:#123456;fill:#123456;}}#{diagram_id} .arrow0{{stroke:#123456;}}#{diagram_id} .label0{{fill:#123456;}}"
        );
        let direct_slot_one = format!(
            "#{diagram_id} .commit1{{stroke:#654321;fill:#654321;}}#{diagram_id} .arrow1{{stroke:#654321;}}#{diagram_id} .label1{{fill:#654321;}}"
        );

        assert!(
            !css.contains(&direct_slot_zero),
            "the {owner}-owned git0 value must suppress the typed slot-zero rule"
        );
        assert!(
            css.contains(&direct_slot_one),
            "the unowned slot one must still use the typed palette"
        );
        assert!(
            css.contains(&format!(
                "#{diagram_id} .commit0{{stroke:#fedcba;fill:#fedcba;}}"
            )),
            "the {owner}-owned git0 value must remain the terminal commit owner"
        );
        assert!(
            css.contains(&format!(
                "#{diagram_id} .commit-highlight0{{stroke:#abcdef;fill:#abcdef;}}"
            )),
            "the adapter must not derive or replace gitInv0"
        );
        assert!(
            css.contains(&format!("#{diagram_id} .branch-label0{{fill:#112233;}}")),
            "the adapter must not derive or replace gitBranchLabel0"
        );

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "{owner}");
        assert_eq!(evidence.applied_count(), 1, "{owner}");
        assert_eq!(evidence.not_applicable_count(), 0, "{owner}");
        assert_eq!(evidence.theme_residual_count(), 0, "{owner}");
    }
}

#[test]
fn gitgraph_node_palette_is_not_applicable_when_every_visible_slot_is_owned() {
    let theme = gitgraph_node_palette_theme(&["#123456"]);
    let rendered = render_gitgraph_with_theme_and_engine(
        "gitGraph\n  commit id: \"1\"\n",
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "themeVariables": { "git0": "#fedcba" }
        }))),
        "git-owned-only",
    );
    let css = gitgraph_stylesheet(rendered.svg());

    assert!(css.contains("#git-owned-only .commit0{stroke:#fedcba;fill:#fedcba;}"));
    assert!(!css.contains("#git-owned-only .commit0{stroke:#123456;fill:#123456;}"));

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gitgraph_redux_palette_preserves_source_and_site_node_border_per_terminal_surface() {
    let theme = gitgraph_node_palette_theme(&["#123456", "#654321"]);
    render_source_and_site_config_cases(
        TWO_BRANCHES,
        json!({
            "theme": "redux",
            "themeVariables": { "nodeBorder": "#fedcba" }
        }),
        &theme,
        "git-redux-node-border",
        |owner, rendered| {
            let document =
                roxmltree::Document::parse(rendered.svg()).expect("valid Redux GitGraph SVG");
            for class in ["commit0", "arrow0", "label0"] {
                assert!(
                    document.descendants().any(|node| has_class(&node, class)),
                    "the Redux fixture must contain the terminal {class} surface"
                );
            }
            let css = gitgraph_stylesheet(rendered.svg());
            let diagram_id = format!("git-redux-node-border-{owner}");

            assert!(
                css.contains(&format!("#{diagram_id} .commit0{{stroke:#fedcba;}}")),
                "the {owner}-owned nodeBorder must remain the terminal commit owner"
            );
            assert!(
                css.contains(&format!("#{diagram_id} .arrow0{{stroke:#fedcba;}}")),
                "the {owner}-owned nodeBorder must remain the terminal arrow owner"
            );
            assert!(
                css.contains(&format!("#{diagram_id} .commit-bullets{{fill:#fedcba;}}")),
                "the {owner}-owned nodeBorder must remain the inherited commit fill owner"
            );
            assert!(
                !css.contains(&format!("#{diagram_id} .commit0{{stroke:#123456;"))
                    && !css.contains(&format!("#{diagram_id} .commit0{{fill:#123456;")),
                "the typed palette must not overwrite the {owner}-owned commit properties"
            );
            assert!(
                !css.contains(&format!("#{diagram_id} .arrow0{{stroke:#123456;}}")),
                "the typed palette must not overwrite the {owner}-owned arrow surface"
            );
            assert!(
                css.contains(&format!("#{diagram_id} .label0{{fill:#123456;}}")),
                "the unowned branch-label background must still consume the typed palette"
            );

            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1, "{owner}");
            assert_eq!(evidence.applied_count(), 1, "{owner}");
            assert_eq!(evidence.not_applicable_count(), 0, "{owner}");
            assert_eq!(evidence.theme_residual_count(), 0, "{owner}");
        },
    );
}

#[test]
fn gitgraph_redux_color_palette_preserves_site_border_color_array_surfaces() {
    let theme = gitgraph_node_palette_theme(&["#123456", "#654321"]);
    let config = json!({
        "theme": "redux-color",
        "themeVariables": {
            "borderColorArray": ["#aabbcc", "#fedcba"]
        }
    });
    let rendered = render_gitgraph_with_theme_and_engine(
        TWO_BRANCHES,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(config)),
        "git-redux-color-array-site",
    );
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid Redux-color GitGraph SVG");
    for class in ["commit0", "commit1", "arrow1", "label1"] {
        assert!(
            document.descendants().any(|node| has_class(&node, class)),
            "the Redux-color fixture must contain the terminal {class} surface"
        );
    }
    let css = gitgraph_stylesheet(rendered.svg());
    let diagram_id = "git-redux-color-array-site";

    for rule in [
        format!("#{diagram_id} .commit1{{stroke:#fedcba;fill:#fedcba;}}"),
        format!("#{diagram_id} .arrow1{{stroke:#fedcba;}}"),
        format!("#{diagram_id} .label1{{fill:#fedcba;stroke:#fedcba;stroke-width:"),
    ] {
        assert!(
            css.contains(&rule),
            "the site-owned borderColorArray must remain the terminal owner: {rule}"
        );
    }
    for rule in [
        format!("#{diagram_id} .commit1{{stroke:#654321;fill:#654321;}}"),
        format!("#{diagram_id} .arrow1{{stroke:#654321;}}"),
        format!("#{diagram_id} .label1{{fill:#654321;}}"),
    ] {
        assert!(
            !css.contains(&rule),
            "the typed palette must not overwrite the site-owned slot-one surface: {rule}"
        );
    }
    assert!(
        css.contains(&format!(
            "#{diagram_id} .commit0{{stroke:#123456;fill:#123456;}}"
        )),
        "the unowned slot-zero commit must still consume the typed palette"
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gitgraph_neo_palette_yields_when_every_visible_surface_is_owned() {
    let theme = gitgraph_node_palette_theme(&["#123456"]);
    render_source_and_site_config_cases(
        "gitGraph\n  commit id: \"1\"\n",
        json!({
            "theme": "neo",
            "themeVariables": {
                "nodeBorder": "#fedcba",
                "mainBkg": "#abcdef",
                "useGradient": true
            }
        }),
        &theme,
        "git-neo-owned",
        |owner, rendered| {
            let document =
                roxmltree::Document::parse(rendered.svg()).expect("valid Neo GitGraph SVG");
            for class in ["commit0", "label0"] {
                assert!(
                    document.descendants().any(|node| has_class(&node, class)),
                    "the Neo fixture must contain the terminal {class} surface"
                );
            }
            let css = gitgraph_stylesheet(rendered.svg());
            let diagram_id = format!("git-neo-owned-{owner}");

            assert!(
                css.contains(&format!("#{diagram_id} .commit0{{stroke:#fedcba;}}")),
                "the {owner}-owned nodeBorder must remain the terminal commit owner"
            );
            assert!(
                css.contains(&format!("#{diagram_id} .commit-bullets{{fill:#fedcba;}}")),
                "the {owner}-owned nodeBorder must remain the inherited commit fill owner"
            );
            assert!(
                css.contains(&format!(
                    "#{diagram_id} .label0{{fill:#abcdef;stroke:url(#{diagram_id}-gradient);stroke-width:"
                )),
                "the {owner}-owned mainBkg must remain the terminal label owner"
            );
            assert!(
                !css.contains(&format!("#{diagram_id} .commit0{{fill:#123456;}}")),
                "the typed palette must not overwrite the {owner}-owned inherited commit fill"
            );
            assert!(
                !css.contains(&format!("#{diagram_id} .commit0{{stroke:#123456;")),
                "the typed palette must not overwrite the {owner}-owned commit stroke"
            );
            assert!(
                !css.contains(&format!("#{diagram_id} .label0{{fill:#123456;}}")),
                "the typed palette must not overwrite the {owner}-owned label surface"
            );

            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1, "{owner}");
            assert_eq!(evidence.applied_count(), 0, "{owner}");
            assert_eq!(evidence.not_applicable_count(), 1, "{owner}");
            assert_eq!(evidence.theme_residual_count(), 0, "{owner}");
        },
    );
}

#[test]
fn gitgraph_text_fill_replaces_all_three_legacy_color_projections() {
    let source = "---\ntitle: Release & tags\n---\ngitGraph\n  commit id: \"A\" tag: \"v1\"\n  branch next\n  commit id: \"B\" tag: \"v2\"\n";
    let empty_theme = gitgraph_edge_rules_theme([]);
    for look in ["classic", "neo", "handDrawn"] {
        for theme_name in ["default", "neo", "neo-dark", "redux", "redux-color"] {
            for variant in [None, Some(ThemeVariant::Default)] {
                for (paint, color) in [
                    (CanvasPaint::solid("#123456").unwrap(), "#123456"),
                    (CanvasPaint::Transparent, "transparent"),
                ] {
                    let mut rule = ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(paint),
                    )
                    .for_family(DiagramFamilyId::GIT_GRAPH);
                    if let Some(variant) = variant {
                        rule = rule.with_variant(variant);
                    }
                    let theme = gitgraph_edge_rules_theme([rule]);
                    let config = json!({"look": look, "theme": theme_name, "themeVariables": {"useGradient": false}});
                    let rendered = render_gitgraph_with_theme_and_engine(
                        source,
                        &theme,
                        Engine::new().with_site_config(MermaidConfig::from_value(config.clone())),
                        "git-text",
                    );
                    let mut legacy_config = config;
                    for path in ["textColor", "tagLabelColor", "commitLabelColor"] {
                        legacy_config["themeVariables"][path] = json!(color);
                    }
                    let legacy = render_gitgraph_with_theme_and_engine(
                        source,
                        &empty_theme,
                        Engine::new().with_site_config(MermaidConfig::from_value(legacy_config)),
                        "git-text",
                    );
                    assert_eq!(
                        rendered.svg(),
                        legacy.svg(),
                        "look={look}, theme={theme_name}, variant={variant:?}, color={color}"
                    );
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    for class in ["tag-hole", "tag-label", "commit-label", "gitTitleText"] {
                        assert!(
                            document.descendants().any(|node| has_class(&node, class)),
                            "missing {class}"
                        );
                    }
                    let title = document
                        .descendants()
                        .find(|node| has_class(node, "gitTitleText"))
                        .unwrap();
                    assert_eq!(title.text(), Some("Release & tags"));
                    assert_eq!(
                        document
                            .descendants()
                            .filter(|node| node.has_tag_name("circle")
                                && has_class(node, "tag-hole")
                                && node.attribute("r") == Some("1.5"))
                            .count(),
                        2
                    );
                    let css = gitgraph_stylesheet(rendered.svg());
                    for selector in [
                        "#git-text",
                        "#git-text .tag-hole",
                        "#git-text .gitTitleText",
                        "#git-text .tag-label",
                    ] {
                        assert_eq!(
                            css_property_winner(&gitgraph_css_rule(&css, selector), "fill"),
                            Some(color),
                            "{selector}"
                        );
                    }
                    if theme_name == "default" {
                        assert_eq!(
                            css_property_winner(
                                &gitgraph_css_rule(&css, "#git-text .commit-label"),
                                "fill"
                            ),
                            Some(color)
                        );
                    }
                    let completion = rendered.into_completion();
                    let evidence = merman_render::__private::family_evidence(completion.report());
                    assert_eq!(evidence.applied_count(), 1);
                    assert_eq!(evidence.not_applicable_count(), 0);
                    assert_eq!(evidence.theme_residual_count(), 0);
                }
            }
        }
    }
}

#[test]
fn gitgraph_text_fill_preserves_each_source_color_owner() {
    let source = "---\ntitle: Owners\n---\ngitGraph\n  commit id: \"A\" tag: \"v1\"\n";
    let theme = gitgraph_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
    )]);
    for (path, owned_selectors) in [
        ("textColor", vec!["", " .tag-hole", " .gitTitleText"]),
        ("tagLabelColor", vec![" .tag-label"]),
        ("commitLabelColor", vec![" .commit-label"]),
    ] {
        let config = json!({"themeVariables": {path: "#abcdef"}});
        render_source_and_site_config_cases(
            source,
            config,
            &theme,
            &format!("git-text-{path}"),
            |owner, rendered| {
                let css = gitgraph_stylesheet(rendered.svg());
                for suffix in [
                    "",
                    " .tag-hole",
                    " .gitTitleText",
                    " .tag-label",
                    " .commit-label",
                ] {
                    let selector = format!("#git-text-{path}-{owner}{suffix}");
                    assert_eq!(
                        css_property_winner(&gitgraph_css_rule(&css, &selector), "fill"),
                        Some(if owned_selectors.contains(&suffix) {
                            "#abcdef"
                        } else {
                            "#123456"
                        }),
                        "{selector}"
                    );
                }
                let completion = rendered.into_completion();
                let evidence = merman_render::__private::family_evidence(completion.report());
                assert_eq!(
                    evidence.applied_count(),
                    1,
                    "the remaining unowned roles still consume Text.fill"
                );
                assert_eq!(evidence.theme_residual_count(), 0);
            },
        );
    }
}

#[test]
fn gitgraph_text_and_specific_labels_preserve_source_order() {
    let source = "---\ntitle: Specific labels\n---\ngitGraph\n  commit id: \"A\" tag: \"v1\"\n";
    let rule = |target, color| {
        ThemeRule::new(
            target,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid(color).unwrap()),
        )
    };
    for text_first in [false, true] {
        let text = rule(ThemeTarget::Text, "#123456");
        let mut rules = vec![
            rule(ThemeTarget::NodeLabel, "#abcdef"),
            rule(ThemeTarget::EdgeLabel, "#654321"),
        ];
        if text_first {
            rules.insert(0, text);
        } else {
            rules.push(text);
        }
        let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
            source,
            &gitgraph_edge_rules_theme(rules),
            Engine::new(),
            "git-text-specific",
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let css = gitgraph_stylesheet(rendered.svg());
        for (class, color) in [
            ("tag-label", if text_first { "#abcdef" } else { "#123456" }),
            (
                "commit-label",
                if text_first { "#654321" } else { "#123456" },
            ),
            ("tag-hole", "#123456"),
            ("gitTitleText", "#123456"),
        ] {
            assert!(document.descendants().any(|node| has_class(&node, class)));
            assert_eq!(
                css_property_winner(
                    &gitgraph_css_rule(&css, &format!("#git-text-specific .{class}")),
                    "fill"
                ),
                Some(color)
            );
        }
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 3);
        assert_eq!(evidence.applied_count(), if text_first { 3 } else { 1 });
        assert_eq!(
            evidence.not_applicable_count(),
            if text_first { 0 } else { 2 }
        );
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn gitgraph_text_applicability_follows_visible_inherited_surfaces() {
    let theme = gitgraph_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
    )]);
    let hidden = json!({"gitGraph": {"showCommitLabel": false, "showBranches": false}});
    let cherry = "gitGraph\n  commit id: \"A\"\n  branch next\n  commit id: \"B\"\n  checkout main\n  cherry-pick id: \"B\" tag: \"\"\n";
    for (case, source, config, applied) in [
        ("empty", "gitGraph\n", hidden.clone(), 0),
        (
            "accessibility-only",
            "gitGraph\n  accTitle: Accessible only\n  commit id: \"A\"\n",
            hidden.clone(),
            0,
        ),
        ("hidden-label", TWO_BRANCHES, hidden.clone(), 0),
        (
            "title-only",
            "---\ntitle: Visible title\n---\ngitGraph\n",
            hidden.clone(),
            1,
        ),
        ("cherry-root", cherry, hidden.clone(), 1),
        (
            "neo-background",
            TWO_BRANCHES,
            json!({"theme": "neo", "gitGraph": {"showCommitLabel": false}, "themeVariables": {"useGradient": false}}),
            1,
        ),
        (
            "neo-local-background",
            TWO_BRANCHES,
            json!({"theme": "neo", "gitGraph": {"showCommitLabel": false}, "themeVariables": {"useGradient": true}}),
            0,
        ),
        (
            "cherry-owned",
            cherry,
            json!({"gitGraph": {"showCommitLabel": false, "showBranches": false}, "themeVariables": {"textColor": "#abcdef"}}),
            0,
        ),
        (
            "missing-slot",
            TWO_BRANCHES,
            json!({"gitGraph": {"showCommitLabel": false, "showBranches": false}, "themeVariables": {"THEME_COLOR_LIMIT": 1}}),
            1,
        ),
        (
            "all-owned",
            "---\ntitle: Owned title\n---\ngitGraph\n  commit id: \"A\" tag: \"v1\"\n",
            json!({"themeVariables": {"textColor": "#abcdef", "tagLabelColor": "#654321", "commitLabelColor": "#456789"}}),
            0,
        ),
    ] {
        let rendered = render_gitgraph_with_theme_and_engine(
            source,
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(config)),
            &format!("git-text-{case}"),
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        if case == "cherry-root" {
            assert!(
                !document
                    .descendants()
                    .any(|node| has_class(&node, "tag-hole")),
                "the cherry case must not be proven by a tag hole"
            );
            assert!(document.descendants().any(|node| {
                node.has_tag_name("circle")
                    && node.attribute("r") == Some("10")
                    && node.attribute("fill").is_none()
                    && node
                        .attribute("class")
                        .is_some_and(|value| value.contains("cherry"))
            }));
        }
        if case == "neo-background" {
            assert!(document.descendants().any(|node| node.has_tag_name("rect")
                && has_class(&node, "label0")
                && node.attribute("fill").is_none()));
            assert!(!gitgraph_stylesheet(rendered.svg()).contains(" .label0{"));
        }
        if case == "missing-slot" {
            assert!(
                document
                    .descendants()
                    .any(|node| node.has_tag_name("circle")
                        && has_class(&node, "commit1")
                        && node.attribute("fill").is_none())
            );
            assert!(!gitgraph_stylesheet(rendered.svg()).contains(" .commit1{"));
        }
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), applied, "{case}");
        assert_eq!(evidence.not_applicable_count(), 1 - applied, "{case}");
        assert_eq!(evidence.theme_residual_count(), 0, "{case}");
    }
}

#[test]
fn gitgraph_text_fill_cannot_certify_unsupported_sibling_properties() {
    let source = "---\ntitle: Mixed text\n---\ngitGraph\n  commit id: \"A\" tag: \"v1\"\n";
    let fill = || ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
    let font = |mut patch: ThemeStylePatch| {
        patch.typography.font_stack =
            Specified::Value(FontStack::single("UnsupportedRuleFont").unwrap());
        patch
    };
    let rule = |patch| ThemeRule::new(ThemeTarget::Text, patch);
    for (case, rules, config, applied, not_applicable, residuals) in [
        ("mixed", vec![rule(font(fill()))], json!({}), 0, 0, 1),
        (
            "font-only",
            vec![rule(font(ThemeStylePatch::default()))],
            json!({}),
            0,
            0,
            1,
        ),
        (
            "later-fill",
            vec![rule(font(fill())), rule(fill())],
            json!({}),
            1,
            0,
            1,
        ),
        (
            "owned-fill",
            vec![rule(font(fill()))],
            json!({"themeVariables": {"textColor": "#abcdef", "tagLabelColor": "#abcdef", "commitLabelColor": "#abcdef"}}),
            0,
            0,
            1,
        ),
        (
            "inactive-variant",
            vec![rule(font(fill())).with_variant(ThemeVariant::Active)],
            json!({}),
            0,
            1,
            0,
        ),
        (
            "ordinal",
            vec![rule(fill()).with_ordinal(OrdinalSelector::exact(1).unwrap())],
            json!({}),
            0,
            0,
            1,
        ),
        (
            "shadowed-ordinal",
            vec![
                rule(fill()).with_ordinal(OrdinalSelector::exact(1).unwrap()),
                rule(fill()),
            ],
            json!({}),
            1,
            1,
            0,
        ),
    ] {
        let theme = gitgraph_edge_rules_theme(rules);
        let engine = || Engine::new().with_site_config(MermaidConfig::from_value(config.clone()));
        let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
            source,
            &theme,
            engine(),
            &format!("git-text-mixed-{case}"),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        roxmltree::Document::parse(rendered.svg()).expect("valid best-effort GitGraph SVG");
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), applied, "{case}");
        assert_eq!(evidence.not_applicable_count(), not_applicable, "{case}");
        assert_eq!(evidence.theme_residual_count(), residuals, "{case}");
        let strict = try_render_gitgraph_with_theme_engine_and_requirement(
            source,
            &theme,
            engine(),
            &format!("git-text-strict-{case}"),
            ThemePortabilityRequirement::RequirePortable,
        );
        if residuals == 0 {
            strict.expect("only supported or inactive text properties");
        } else {
            let error = match strict {
                Ok(_) => panic!("{case}: unsupported text property passed strict portability"),
                Err(error) => error,
            };
            assert_eq!(
                error.unverified_family_theme(),
                Some((DiagramFamilyId::GIT_GRAPH, residuals)),
                "{case}"
            );
        }
    }
}

#[test]
fn gitgraph_node_palette_can_own_neo_backgrounds_without_certifying_text_fill() {
    let palette = OrdinalPalette::new([ThemeColorValue::parse("#abcdef").unwrap()]).unwrap();
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_ordinal_palette(ThemeTarget::Node, palette)
                    .with_rule(ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    )),
            ),
        )
        .unwrap();
    let rendered = render_gitgraph_with_theme_and_engine(
        TWO_BRANCHES,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "theme": "neo", "gitGraph": {"showCommitLabel": false},
            "themeVariables": {"useGradient": false}
        }))),
        "git-text-palette-owned",
    );
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(
        !document
            .descendants()
            .any(|node| has_class(&node, "commit-label"))
    );
    assert!(
        !document
            .descendants()
            .any(|node| has_class(&node, "tag-hole"))
    );
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(
        evidence.applied_count(),
        1,
        "only the Node palette has an active terminal"
    );
    assert_eq!(
        evidence.not_applicable_count(),
        1,
        "Text.fill has no inherited background left"
    );
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gitgraph_specific_label_clear_does_not_restore_generic_text_fill() {
    for (target, class, source) in [
        (
            ThemeTarget::NodeLabel,
            "tag-label",
            "gitGraph\n  commit id: \"A\" tag: \"v1\"\n",
        ),
        (
            ThemeTarget::EdgeLabel,
            "commit-label",
            "gitGraph\n  commit id: \"A\"\n",
        ),
    ] {
        let mut cleared = ThemeStylePatch::default();
        cleared.paint.fill = Specified::Clear;
        let theme = gitgraph_edge_rules_theme([
            ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            ),
            ThemeRule::new(target, cleared),
        ]);
        let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
            source,
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "themeVariables": {"textColor": "#abcdef"},
                "gitGraph": {"showCommitLabel": target == ThemeTarget::EdgeLabel}
            }))),
            "git-text-clear",
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(document.descendants().any(|node| has_class(&node, class)));
        let css = gitgraph_stylesheet(rendered.svg());
        assert_ne!(
            css_property_winner(
                &gitgraph_css_rule(&css, &format!("#git-text-clear .{class}")),
                "fill"
            ),
            Some("#123456")
        );
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(
            evidence.applied_count(),
            0,
            "a specific Clear must block the generic fill"
        );
        assert!(
            evidence.not_applicable_count() >= 1,
            "the generic Text rule has no remaining terminal"
        );
    }
}

#[test]
fn gitgraph_later_text_ordinal_is_not_masked_by_an_earlier_specific_label_rule() {
    let theme = gitgraph_edge_rules_theme([
        ThemeRule::new(
            ThemeTarget::NodeLabel,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#abcdef").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        )
        .with_ordinal(OrdinalSelector::exact(1).unwrap()),
    ]);
    let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
        "gitGraph\n  commit id: \"A\" tag: \"v1\"\n",
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "themeVariables": {"textColor": "#fedcba", "commitLabelColor": "#fedcba"}
        }))),
        "git-text-later-ordinal",
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(
        document
            .descendants()
            .any(|node| has_class(&node, "tag-label"))
    );
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(
        evidence.theme_residual_count(),
        1,
        "the later ordinal still needs a modeled terminal identity"
    );
}

#[test]
fn gitgraph_text_typography_yields_to_the_winning_label_property() {
    for (target, class, source) in [
        (
            ThemeTarget::EdgeLabel,
            "commit-label",
            "gitGraph\n  commit id: \"A\"\n",
        ),
        (
            ThemeTarget::NodeLabel,
            "tag-label",
            "gitGraph\n  commit id: \"A\" tag: \"v1\"\n",
        ),
    ] {
        let font_rule = |target, name| {
            let mut patch = ThemeStylePatch::default();
            patch.typography.font_stack = Specified::Value(FontStack::single(name).unwrap());
            ThemeRule::new(target, patch)
        };
        let theme = gitgraph_edge_rules_theme([
            font_rule(ThemeTarget::Text, "EarlierGenericFont"),
            font_rule(target, "LaterLabelFont"),
        ]);
        let rendered = try_render_gitgraph_with_theme_engine_and_requirement(source, &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "gitGraph": {"showBranches": false, "showCommitLabel": target == ThemeTarget::EdgeLabel}
            }))), "git-text-font-shadowed", ThemePortabilityRequirement::BestEffort).unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(document.descendants().any(|node| has_class(&node, class)));
        assert!(
            !document
                .descendants()
                .any(|node| has_class(&node, "gitTitleText"))
        );
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(
            evidence.not_applicable_count(),
            1,
            "the earlier Text font has no winning text terminal"
        );
    }
}

#[test]
fn gitgraph_specific_label_fill_replaces_its_exact_legacy_color() {
    let source = "---\ntitle: Independent labels\n---\ngitGraph\n  commit id: \"A\" tag: \"v1\"\n  branch next\n  commit id: \"B\" tag: \"v2\"\n";
    for (target, key, class) in [
        (ThemeTarget::NodeLabel, "tagLabelColor", "tag-label"),
        (ThemeTarget::EdgeLabel, "commitLabelColor", "commit-label"),
    ] {
        for theme_name in ["default", "neo", "neo-dark", "redux", "redux-color"] {
            for look in ["classic", "neo", "handDrawn"] {
                for variant in [None, Some(ThemeVariant::Default)] {
                    for (paint, color) in [
                        (CanvasPaint::solid("#123456").unwrap(), "#123456"),
                        (CanvasPaint::Transparent, "transparent"),
                    ] {
                        let mut rule =
                            ThemeRule::new(target, ThemeStylePatch::default().with_fill(paint));
                        if let Some(variant) = variant {
                            rule = rule.with_variant(variant);
                        }
                        let config = json!({"theme": theme_name, "look": look});
                        let rendered = render_gitgraph_with_theme_and_engine(
                            source,
                            &gitgraph_edge_rules_theme([rule]),
                            Engine::new()
                                .with_site_config(MermaidConfig::from_value(config.clone())),
                            "git-specific-label",
                        );
                        let mut legacy_config = config;
                        legacy_config["themeVariables"] = json!({key: color});
                        let legacy = render_gitgraph_with_theme_and_engine(
                            source,
                            &gitgraph_edge_rules_theme([]),
                            Engine::new()
                                .with_site_config(MermaidConfig::from_value(legacy_config)),
                            "git-specific-label",
                        );
                        assert_eq!(
                            rendered.svg(),
                            legacy.svg(),
                            "{target:?}/{theme_name}/{look}/{variant:?}/{color}"
                        );
                        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                        assert_eq!(
                            document
                                .descendants()
                                .filter(|node| node.has_tag_name("text") && has_class(node, class))
                                .count(),
                            2
                        );
                        let source_owned =
                            target == ThemeTarget::EdgeLabel && theme_name != "default";
                        if !source_owned {
                            let css = gitgraph_stylesheet(rendered.svg());
                            assert_eq!(
                                css_property_winner(
                                    &gitgraph_css_rule(
                                        &css,
                                        &format!("#git-specific-label .{class}")
                                    ),
                                    "fill"
                                ),
                                Some(color)
                            );
                        }
                        let completion = rendered.into_completion();
                        let evidence =
                            merman_render::__private::family_evidence(completion.report());
                        assert_eq!(evidence.required_count(), 1);
                        assert_eq!(evidence.applied_count(), usize::from(!source_owned));
                        assert_eq!(evidence.not_applicable_count(), usize::from(source_owned));
                        assert_eq!(evidence.theme_residual_count(), 0);
                    }
                }
            }
        }
    }
}

#[test]
fn gitgraph_label_color_ownership_is_property_local_and_requires_visible_text() {
    for (target, own_key, other_key, class) in [
        (
            ThemeTarget::NodeLabel,
            "tagLabelColor",
            "commitLabelColor",
            "tag-label",
        ),
        (
            ThemeTarget::EdgeLabel,
            "commitLabelColor",
            "tagLabelColor",
            "commit-label",
        ),
    ] {
        let theme = gitgraph_edge_rules_theme([ThemeRule::new(
            target,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        )]);
        for key in [own_key, other_key, "textColor"] {
            render_source_and_site_config_cases(
                "gitGraph\n  commit id: \"A\" tag: \"v1\"\n",
                json!({"themeVariables": {key: "#abcdef"}}),
                &theme,
                "git-label-owner",
                |owner, rendered| {
                    let css = gitgraph_stylesheet(rendered.svg());
                    let selector = format!("#git-label-owner-{owner} .{class}");
                    assert_eq!(
                        css_property_winner(&gitgraph_css_rule(&css, &selector), "fill"),
                        Some(if key == own_key { "#abcdef" } else { "#123456" })
                    );
                    let completion = rendered.into_completion();
                    let evidence = merman_render::__private::family_evidence(completion.report());
                    assert_eq!(evidence.applied_count(), usize::from(key != own_key));
                    assert_eq!(evidence.not_applicable_count(), usize::from(key == own_key));
                    assert_eq!(evidence.theme_residual_count(), 0);
                },
            );
        }
        for source in [
            "gitGraph\n",
            "---\ntitle: Only root title\n---\ngitGraph\n  commit id: \"A\"\n",
        ] {
            let rendered = render_gitgraph_with_theme_and_engine(
                source,
                &theme,
                Engine::new().with_site_config(MermaidConfig::from_value(
                    json!({"gitGraph": {"showCommitLabel": false}}),
                )),
                "git-label-absent",
            );
            let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert!(!doc.descendants().any(|node| has_class(&node, class)));
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.applied_count(), 0);
            assert_eq!(
                evidence.not_applicable_count(),
                1,
                "an unrelated root title is not a label terminal"
            );
        }
    }
}

#[test]
fn gitgraph_specific_label_mixed_properties_and_ordinals_fail_closed() {
    for target in [ThemeTarget::NodeLabel, ThemeTarget::EdgeLabel] {
        let fill = || ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
        let mut mixed = fill();
        mixed.typography.font_stack =
            Specified::Value(FontStack::single("UnsupportedLabelFont").unwrap());
        let own_key = if target == ThemeTarget::NodeLabel {
            "tagLabelColor"
        } else {
            "commitLabelColor"
        };
        for (case, rule, config, residuals) in [
            ("mixed", ThemeRule::new(target, mixed.clone()), json!({}), 1),
            (
                "owned-mixed",
                ThemeRule::new(target, mixed),
                json!({"themeVariables": {own_key: "#abcdef"}}),
                1,
            ),
            (
                "ordinal",
                ThemeRule::new(target, fill()).with_ordinal(OrdinalSelector::exact(1).unwrap()),
                json!({}),
                1,
            ),
            (
                "inactive",
                ThemeRule::new(target, fill()).with_variant(ThemeVariant::Active),
                json!({}),
                0,
            ),
        ] {
            let theme = gitgraph_edge_rules_theme([rule]);
            let engine =
                || Engine::new().with_site_config(MermaidConfig::from_value(config.clone()));
            let source = "gitGraph\n  commit id: \"A\" tag: \"v1\"\n";
            let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
                source,
                &theme,
                engine(),
                "git-label-mixed",
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            roxmltree::Document::parse(rendered.svg()).unwrap();
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.applied_count(), 0);
            assert_eq!(
                evidence.theme_residual_count(),
                residuals,
                "{target:?}/{case}"
            );
            assert_eq!(evidence.not_applicable_count(), 1 - residuals);
            let strict = try_render_gitgraph_with_theme_engine_and_requirement(
                source,
                &theme,
                engine(),
                "git-label-mixed-strict",
                ThemePortabilityRequirement::RequirePortable,
            );
            if residuals == 0 {
                strict.unwrap();
            } else {
                let error = match strict {
                    Err(error) => error,
                    Ok(_) => panic!("unsupported {target:?}/{case} passed strict"),
                };
                assert_eq!(
                    error.unverified_family_theme(),
                    Some((DiagramFamilyId::GIT_GRAPH, 1))
                );
            }
        }
    }
}

#[test]
fn gitgraph_label_fallbacks_require_their_own_visible_terminal() {
    use merman_render::diagram_theme::{
        DiagramEffectSet, EffectBinding, EffectGraph, EffectInput, EffectPrimitive,
    };
    for target in [ThemeTarget::NodeLabel, ThemeTarget::EdgeLabel] {
        for effect in [false, true] {
            for overridden in [false, true] {
                let mut styles = ThemeRuleSet::default();
                let mut spec = DiagramThemeSpec::new();
                if effect {
                    spec = spec.with_effects(
                        DiagramEffectSet::default()
                            .with_graph(
                                EffectGraph::new(
                                    "blur",
                                    [EffectPrimitive::GaussianBlur {
                                        input: EffectInput::SourceGraphic,
                                        std_deviation: 1.0,
                                    }],
                                )
                                .unwrap(),
                            )
                            .unwrap()
                            .with_binding(EffectBinding::new(target, "blur").unwrap())
                            .unwrap(),
                    );
                } else {
                    styles = styles.with_ordinal_palette(
                        target,
                        OrdinalPalette::new([ThemeColorValue::parse("#123456").unwrap()]).unwrap(),
                    );
                }
                if overridden {
                    let mut patch = ThemeStylePatch::default();
                    if effect {
                        patch.effects.effect = Specified::Clear;
                    } else {
                        patch.paint.fill = Specified::Clear;
                    }
                    styles = styles.with_rule(ThemeRule::new(target, patch));
                }
                let theme = DiagramThemeCompiler::new()
                    .compile(spec.with_styles(styles))
                    .unwrap();
                for (source, visible) in [
                    ("---\ntitle: Root only\n---\ngitGraph\n", false),
                    ("gitGraph\n  commit id: \"A\" tag: \"v1\"\n", true),
                ] {
                    let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
                        source,
                        &theme,
                        Engine::new(),
                        "git-label-fallback",
                        ThemePortabilityRequirement::BestEffort,
                    )
                    .unwrap();
                    roxmltree::Document::parse(rendered.svg()).unwrap();
                    let completion = rendered.into_completion();
                    let evidence = merman_render::__private::family_evidence(completion.report());
                    assert_eq!(evidence.required_count(), 1 + usize::from(overridden));
                    assert_eq!(evidence.applied_count(), 0);
                    assert_eq!(
                        evidence.theme_residual_count(),
                        usize::from(visible),
                        "{target:?}/effect={effect}/overridden={overridden}/visible={visible}"
                    );
                    assert_eq!(
                        evidence.not_applicable_count(),
                        usize::from(overridden) + usize::from(!visible)
                    );
                }
            }
        }
    }
}

#[test]
fn gitgraph_node_fill_reaches_state_and_tag_terminals_for_default_selectors() {
    let source = "gitGraph\n  commit id: \"A\" type: HIGHLIGHT tag: \"v1\"\n  branch next\n  commit id: \"B\" type: REVERSE\n  checkout main\n  merge next id: \"C\"\n";
    for theme_name in ["default", "neo", "redux-color"] {
        for variant in [None, Some(ThemeVariant::Default)] {
            for (paint, color) in [
                (CanvasPaint::solid("#123456").unwrap(), "#123456"),
                (CanvasPaint::Transparent, "transparent"),
            ] {
                let mut rule = ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(paint),
                );
                if let Some(variant) = variant {
                    rule = rule.with_variant(variant);
                }
                let rendered = render_gitgraph_with_theme_and_engine(
                    source,
                    &gitgraph_edge_rules_theme([rule]),
                    Engine::new()
                        .with_site_config(MermaidConfig::from_value(json!({"theme": theme_name}))),
                    "git-node-fill-terminals",
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let css = gitgraph_stylesheet(rendered.svg());
                for class in [
                    "commit-highlight-inner",
                    "commit-reverse",
                    "commit-merge",
                    "tag-label-bkg",
                ] {
                    assert!(document.descendants().any(|node| has_class(&node, class)));
                    assert_eq!(
                        css_property_winner(
                            &gitgraph_css_rule(&css, &format!("#git-node-fill-terminals .{class}")),
                            "fill"
                        ),
                        Some(color),
                        "{theme_name}/{variant:?}/{color}/{class}"
                    );
                }
                let completion = rendered.into_completion();
                let evidence = merman_render::__private::family_evidence(completion.report());
                assert_eq!(evidence.required_count(), 1);
                assert_eq!(evidence.applied_count(), 1);
                assert_eq!(evidence.not_applicable_count(), 0);
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn gitgraph_node_fill_requires_an_unowned_visible_fill_terminal() {
    let theme = gitgraph_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Node,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
    )]);
    for (case, source, config) in [
        ("empty", "gitGraph\n", json!({})),
        ("ordinary", "gitGraph\n  commit id: \"A\"\n", json!({})),
        (
            "owned-state",
            "gitGraph\n  commit id: \"A\" type: REVERSE\n",
            json!({"themeVariables": {"primaryColor": "#abcdef"}}),
        ),
        (
            "owned-tag",
            "gitGraph\n  commit id: \"A\" tag: \"v1\"\n",
            json!({"themeVariables": {"tagLabelBackground": "#abcdef"}}),
        ),
    ] {
        render_source_and_site_config_cases(
            source,
            config,
            &theme,
            "git-node-fill-absent",
            |owner, rendered| {
                roxmltree::Document::parse(rendered.svg()).unwrap();
                let completion = rendered.into_completion();
                let evidence = merman_render::__private::family_evidence(completion.report());
                assert_eq!(evidence.required_count(), 1, "{case}/{owner}");
                assert_eq!(evidence.applied_count(), 0, "{case}/{owner}");
                assert_eq!(evidence.not_applicable_count(), 1, "{case}/{owner}");
                assert_eq!(evidence.theme_residual_count(), 0, "{case}/{owner}");
            },
        );
    }
}

#[test]
fn gitgraph_node_fill_remains_portable_with_owned_legacy_stroke_in_the_same_rule() {
    let theme = gitgraph_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Node,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").unwrap())
            .with_stroke(CanvasPaint::solid("#654321").unwrap()),
    )]);
    let source = "gitGraph\n  commit id: \"A\" type: REVERSE tag: \"v1\"\n";
    for portability in [
        ThemePortabilityRequirement::BestEffort,
        ThemePortabilityRequirement::RequirePortable,
    ] {
        let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
            source,
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "themeVariables": {
                    "primaryBorderColor": "#abcdef",
                    "nodeBorder": "#abcdef",
                    "tagLabelBorder": "#abcdef"
                }
            }))),
            "git-node-fill-owned-stroke",
            portability,
        )
        .expect("owned compatibility stroke must not reject portable Node fill");
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let css = gitgraph_stylesheet(rendered.svg());
        for class in ["commit-reverse", "tag-label-bkg"] {
            assert!(document.descendants().any(|node| has_class(&node, class)));
            assert_eq!(
                css_property_winner(
                    &gitgraph_css_rule(&css, &format!("#git-node-fill-owned-stroke .{class}")),
                    "fill"
                ),
                Some("#123456")
            );
        }
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
        assert_eq!(evidence.compatibility_residual_count(), 0);
    }

    let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
        source,
        &theme,
        Engine::new(),
        "git-node-fill-unowned-stroke",
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap();
    let css = gitgraph_stylesheet(rendered.svg());
    for class in ["commit-reverse", "tag-label-bkg"] {
        assert_eq!(
            css_property_winner(
                &gitgraph_css_rule(&css, &format!("#git-node-fill-unowned-stroke .{class}")),
                "fill"
            ),
            Some("#123456")
        );
    }
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 1);

    let error = match try_render_gitgraph_with_theme_and_engine(
        source,
        &theme,
        Engine::new(),
        "git-node-fill-unowned-stroke-strict",
    ) {
        Ok(_) => panic!("unowned Node stroke must retain its compatibility residual"),
        Err(error) => error,
    };
    match error {
        merman_render::Error::LegacyFamilyThemeCompatibility {
            family_id,
            residual_count,
        } => {
            assert_eq!(family_id, DiagramFamilyId::GIT_GRAPH);
            assert_eq!(residual_count, 1);
        }
        other => panic!("expected GitGraph legacy compatibility residual, got {other}"),
    }
}

#[test]
fn gitgraph_node_fill_does_not_certify_an_unsupported_font_in_the_same_rule() {
    let mut patch = ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
    patch.typography.font_stack =
        Specified::Value(FontStack::single("UnsupportedNodeFont").unwrap());
    let theme = gitgraph_edge_rules_theme([ThemeRule::new(ThemeTarget::Node, patch)]);
    let source = "gitGraph\n  commit id: \"A\" type: REVERSE tag: \"v1\"\n";
    for owned in [false, true] {
        let config = if owned {
            json!({"themeVariables": {
                "primaryColor": "#abcdef", "tagLabelBackground": "#abcdef"
            }})
        } else {
            json!({})
        };
        let engine = || Engine::new().with_site_config(MermaidConfig::from_value(config.clone()));
        let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
            source,
            &theme,
            engine(),
            "git-node-fill-mixed",
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let css = gitgraph_stylesheet(rendered.svg());
        for class in ["commit-reverse", "tag-label-bkg"] {
            assert_eq!(
                css_property_winner(
                    &gitgraph_css_rule(&css, &format!("#git-node-fill-mixed .{class}")),
                    "fill"
                ),
                Some(if owned { "#abcdef" } else { "#123456" })
            );
        }
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 1);
        let strict = try_render_gitgraph_with_theme_and_engine(
            source,
            &theme,
            engine(),
            "git-node-fill-mixed-strict",
        );
        let error = match strict {
            Err(error) => error,
            Ok(_) => panic!("a supported Node fill must not certify its unsupported font"),
        };
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::GIT_GRAPH, 1))
        );
    }
}

#[test]
fn gitgraph_static_node_fill_keeps_palette_strokes_in_either_builder_order() {
    let source = "gitGraph\n  commit id: \"A\" type: REVERSE tag: \"v1\"\n";
    let mut previous_svg = None;
    for palette_first in [true, false] {
        let palette = OrdinalPalette::new([ThemeColorValue::parse("#abcdef").unwrap()]).unwrap();
        let rule = ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        );
        let styles = if palette_first {
            ThemeRuleSet::default()
                .with_ordinal_palette(ThemeTarget::Node, palette)
                .with_rule(rule)
        } else {
            ThemeRuleSet::default()
                .with_rule(rule)
                .with_ordinal_palette(ThemeTarget::Node, palette)
        };
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .unwrap();
        let rendered = render_gitgraph_with_theme_and_engine(
            source,
            &theme,
            Engine::new(),
            "git-node-fill-palette",
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(
            document
                .descendants()
                .any(|node| has_class(&node, "commit0"))
        );
        let css = gitgraph_stylesheet(rendered.svg());
        for class in ["commit-reverse", "tag-label-bkg"] {
            assert_eq!(
                css_property_winner(
                    &gitgraph_css_rule(&css, &format!("#git-node-fill-palette .{class}")),
                    "fill"
                ),
                Some("#123456")
            );
        }
        let (_, palette_rule) = css.rsplit_once("#git-node-fill-palette .commit0{").unwrap();
        let (palette_rule, _) = palette_rule.split_once('}').unwrap();
        assert_eq!(css_property_winner(palette_rule, "stroke"), Some("#abcdef"));
        assert_eq!(css_property_winner(palette_rule, "fill"), None);
        if let Some(previous_svg) = previous_svg.as_ref() {
            assert_eq!(rendered.svg(), previous_svg);
        }
        previous_svg = Some(rendered.svg().to_owned());
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.applied_count(), 2);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn gitgraph_node_fill_preserves_independent_state_and_tag_owners() {
    let source = "gitGraph\n  commit id: \"state\" type: REVERSE tag: \"v1\"\n";
    for theme_name in ["base", "neo", "redux", "redux-color"] {
        for explicit_path in [
            None,
            Some("primaryColor"),
            Some("mainBkg"),
            Some("tagLabelBackground"),
        ] {
            let mut config = json!({"theme": theme_name});
            if let Some(path) = explicit_path {
                config["themeVariables"] = json!({path: "#fedcba"});
            }
            let theme = gitgraph_edge_rules_theme([ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            )
            .for_family(DiagramFamilyId::GIT_GRAPH)]);
            let rendered = render_gitgraph_with_theme_and_engine(
                source,
                &theme,
                Engine::new().with_site_config(MermaidConfig::from_value(config)),
                "git-node-fill",
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert!(
                document
                    .descendants()
                    .any(|node| has_class(&node, "commit-reverse"))
            );
            assert!(
                document
                    .descendants()
                    .any(|node| has_class(&node, "tag-label-bkg"))
            );
            let css = gitgraph_stylesheet(rendered.svg());
            let color_gen = theme_name != "base";
            let state_path = if color_gen { "mainBkg" } else { "primaryColor" };
            let tag_path = if color_gen {
                "mainBkg"
            } else {
                "tagLabelBackground"
            };
            for (selector, path) in [
                ("#git-node-fill .commit-reverse", state_path),
                ("#git-node-fill .tag-label-bkg", tag_path),
            ] {
                assert_eq!(
                    css_property_winner(&gitgraph_css_rule(&css, selector), "fill"),
                    Some(if explicit_path == Some(path) {
                        "#fedcba"
                    } else {
                        "#123456"
                    }),
                    "{theme_name}/{explicit_path:?}/{selector}"
                );
            }
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1);
            let entirely_owned = color_gen && explicit_path == Some("mainBkg");
            assert_eq!(evidence.applied_count(), usize::from(!entirely_owned));
            assert_eq!(evidence.not_applicable_count(), usize::from(entirely_owned));
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn gitgraph_edge_fill_replaces_the_branch_line_config_without_changing_other_surfaces() {
    for theme_name in ["default", "neo", "neo-dark", "redux", "redux-color"] {
        for look in ["classic", "neo", "handDrawn"] {
            for variant in [None, Some(ThemeVariant::Default)] {
                for (paint, color) in [
                    (CanvasPaint::solid("#123456").unwrap(), "#123456"),
                    (CanvasPaint::Transparent, "transparent"),
                ] {
                    let mut rule = ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default().with_fill(paint),
                    );
                    if let Some(variant) = variant {
                        rule = rule.with_variant(variant);
                    }
                    let config = json!({"theme": theme_name, "look": look});
                    let rendered = render_gitgraph_with_theme_and_engine(
                        TWO_BRANCHES,
                        &gitgraph_edge_rules_theme([rule]),
                        Engine::new().with_site_config(MermaidConfig::from_value(config.clone())),
                        "git-edge-fill",
                    );
                    let mut legacy_config = config;
                    legacy_config["themeVariables"] = json!({"commitLineColor": color});
                    let legacy = render_gitgraph_with_theme_and_engine(
                        TWO_BRANCHES,
                        &gitgraph_edge_rules_theme([]),
                        Engine::new().with_site_config(MermaidConfig::from_value(legacy_config)),
                        "git-edge-fill",
                    );
                    assert_eq!(
                        rendered.svg(),
                        legacy.svg(),
                        "{theme_name}/{look}/{variant:?}/{color}"
                    );
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    assert_eq!(branch_lines(&document).len(), 2);
                    let css = gitgraph_stylesheet(rendered.svg());
                    assert_eq!(
                        css_property_winner(
                            &gitgraph_css_rule(&css, "#git-edge-fill .branch"),
                            "stroke"
                        ),
                        Some(color)
                    );
                    let completion = rendered.into_completion();
                    let evidence = merman_render::__private::family_evidence(completion.report());
                    assert_eq!(evidence.required_count(), 1);
                    assert_eq!(evidence.applied_count(), 1);
                    assert_eq!(evidence.not_applicable_count(), 0);
                    assert_eq!(evidence.theme_residual_count(), 0);
                }
            }
        }
    }
}

#[test]
fn gitgraph_edge_fill_preserves_its_config_owner_and_visible_branch_domain() {
    let theme = gitgraph_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
    )]);
    for theme_name in ["default", "redux"] {
        for (key, owned) in [("commitLineColor", true), ("lineColor", false)] {
            let rendered = render_gitgraph_with_theme_and_engine(
                TWO_BRANCHES,
                &theme,
                Engine::new().with_site_config(MermaidConfig::from_value(
                    json!({"theme": theme_name, "themeVariables": {key: "#abcdef"}}),
                )),
                "git-edge-fill-owner",
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert_eq!(branch_lines(&document).len(), 2);
            let css = gitgraph_stylesheet(rendered.svg());
            assert_eq!(
                css_property_winner(
                    &gitgraph_css_rule(&css, "#git-edge-fill-owner .branch"),
                    "stroke"
                ),
                Some(if owned { "#abcdef" } else { "#123456" })
            );
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.applied_count(), usize::from(!owned));
            assert_eq!(evidence.not_applicable_count(), usize::from(owned));
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
    render_source_and_site_config_cases(
        TWO_BRANCHES,
        json!({"themeVariables": {"lineColor": "#abcdef"}}),
        &theme,
        "git-edge-fill-source",
        |owner, rendered| {
            let css = gitgraph_stylesheet(rendered.svg());
            assert_eq!(
                css_property_winner(
                    &gitgraph_css_rule(&css, &format!("#git-edge-fill-source-{owner} .branch")),
                    "stroke"
                ),
                Some("#123456")
            );
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
        },
    );
    let rendered = render_gitgraph_with_theme_and_engine(
        TWO_BRANCHES,
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(
            json!({"gitGraph": {"showBranches": false}}),
        )),
        "git-edge-fill-hidden",
    );
    let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(branch_lines(&doc).is_empty());
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn gitgraph_edge_fill_is_only_a_fallback_for_unspecified_stroke() {
    let fill = || {
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        )
    };
    for reverse in [false, true] {
        for clear in [false, true] {
            let mut stroke = ThemeStylePatch::default();
            stroke.stroke.paint = if clear {
                Specified::Clear
            } else {
                Specified::Value(CanvasPaint::solid("#abcdef").unwrap())
            };
            let stroke = ThemeRule::new(ThemeTarget::Edge, stroke);
            let rules = if reverse {
                vec![stroke, fill()]
            } else {
                vec![fill(), stroke]
            };
            let theme = gitgraph_edge_rules_theme(rules);
            let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
                TWO_BRANCHES,
                &theme,
                Engine::new(),
                "git-edge-fill-fallback",
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
            let branches = branch_lines(&doc);
            assert_eq!(branches.len(), 2);
            assert!(branches.iter().all(|node| {
                node.attribute("style")
                    .is_none_or(|style| !style.contains("#123456"))
            }));
            if !clear {
                assert!(
                    branches
                        .iter()
                        .all(|node| node.attribute("style") == Some("stroke:#abcdef;"))
                );
            }
            let css = gitgraph_stylesheet(rendered.svg());
            assert_ne!(
                css_property_winner(
                    &gitgraph_css_rule(&css, "#git-edge-fill-fallback .branch"),
                    "stroke"
                ),
                Some("#123456"),
                "stroke/Clear must block the stylesheet fallback too"
            );
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 2);
            assert_eq!(evidence.applied_count(), usize::from(!clear));
            assert_eq!(evidence.not_applicable_count(), 1);
            assert_eq!(evidence.theme_residual_count(), usize::from(clear));
            let strict = try_render_gitgraph_with_theme_engine_and_requirement(
                TWO_BRANCHES,
                &theme,
                Engine::new(),
                "git-edge-fill-fallback-strict",
                ThemePortabilityRequirement::RequirePortable,
            );
            if clear {
                assert!(matches!(
                    strict,
                    Err(merman_render::Error::UnverifiedFamilyTheme { .. })
                ));
            } else {
                strict.unwrap();
            }
        }
    }
}

#[test]
fn gitgraph_edge_fill_does_not_certify_unsupported_winners() {
    let fill = || ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
    let mut mixed = fill();
    mixed.typography.font_stack =
        Specified::Value(FontStack::single("UnverifiedBranchFont").unwrap());
    for (rule, config, residuals, expected_color) in [
        (
            ThemeRule::new(ThemeTarget::Edge, mixed.clone()),
            json!({}),
            1,
            Some("#123456"),
        ),
        (
            ThemeRule::new(ThemeTarget::Edge, mixed),
            json!({"themeVariables": {"commitLineColor": "#abcdef"}}),
            1,
            Some("#abcdef"),
        ),
        (
            ThemeRule::new(ThemeTarget::Edge, fill())
                .with_ordinal(OrdinalSelector::exact(1).unwrap()),
            json!({}),
            1,
            None,
        ),
        (
            ThemeRule::new(ThemeTarget::Edge, fill()).with_variant(ThemeVariant::Active),
            json!({}),
            0,
            None,
        ),
    ] {
        let theme = gitgraph_edge_rules_theme([rule]);
        let engine = || Engine::new().with_site_config(MermaidConfig::from_value(config.clone()));
        let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
            TWO_BRANCHES,
            &theme,
            engine(),
            "git-edge-fill-unsupported",
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        roxmltree::Document::parse(rendered.svg()).unwrap();
        if let Some(color) = expected_color {
            let css = gitgraph_stylesheet(rendered.svg());
            assert_eq!(
                css_property_winner(
                    &gitgraph_css_rule(&css, "#git-edge-fill-unsupported .branch"),
                    "stroke"
                ),
                Some(color)
            );
        }
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1 - residuals);
        assert_eq!(evidence.theme_residual_count(), residuals);
        let strict = try_render_gitgraph_with_theme_engine_and_requirement(
            TWO_BRANCHES,
            &theme,
            engine(),
            "git-edge-fill-unsupported-strict",
            ThemePortabilityRequirement::RequirePortable,
        );
        if residuals == 0 {
            strict.unwrap();
        } else {
            let error = match strict {
                Err(error) => error,
                Ok(_) => panic!("unsupported Edge winner passed strict portability"),
            };
            assert_eq!(
                error.unverified_family_theme(),
                Some((DiagramFamilyId::GIT_GRAPH, 1))
            );
        }
    }
}

#[test]
fn gitgraph_edge_fill_partial_ordinals_preserve_best_effort_without_false_evidence() {
    let paint = || CanvasPaint::solid("#123456").unwrap();
    let static_fill = || {
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_fill(paint()),
        )
    };
    for stroke in [false, true] {
        let mut patch = ThemeStylePatch::default();
        if stroke {
            patch.stroke.paint = Specified::Value(paint());
        } else {
            patch.paint.fill = Specified::Value(paint());
        }
        let ordinal = ThemeRule::new(ThemeTarget::Edge, patch)
            .with_ordinal(OrdinalSelector::exact(1).unwrap());
        let theme = gitgraph_edge_rules_theme([static_fill(), ordinal]);
        let rendered = try_render_gitgraph_with_theme_engine_and_requirement(
            TWO_BRANCHES,
            &theme,
            Engine::new(),
            "git-partial-ordinal",
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let previous = render_gitgraph_with_theme_and_engine(
            TWO_BRANCHES,
            &gitgraph_edge_rules_theme([]),
            Engine::new().with_site_config(MermaidConfig::from_value(
                json!({"themeVariables": {"commitLineColor": "#123456"}}),
            )),
            "git-partial-ordinal",
        );
        assert_eq!(
            rendered.svg(),
            previous.svg(),
            "legacy fallback ignored unsupported ordinal routes; stroke={stroke}"
        );
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(
            evidence.theme_residual_count(),
            2,
            "uniform CSS cannot certify partial ordinal ownership"
        );
        let strict = try_render_gitgraph_with_theme_engine_and_requirement(
            TWO_BRANCHES,
            &theme,
            Engine::new(),
            "git-partial-ordinal-strict",
            ThemePortabilityRequirement::RequirePortable,
        );
        assert!(matches!(
            strict,
            Err(merman_render::Error::UnverifiedFamilyTheme { .. })
        ));
    }
    let theme = gitgraph_edge_rules_theme([
        static_fill().with_ordinal(OrdinalSelector::exact(1).unwrap()),
        static_fill(),
    ]);
    let rendered = render_gitgraph_with_theme_and_engine(
        TWO_BRANCHES,
        &theme,
        Engine::new(),
        "git-shadowed-ordinal",
    );
    let css = gitgraph_stylesheet(rendered.svg());
    assert_eq!(
        css_property_winner(
            &gitgraph_css_rule(&css, "#git-shadowed-ordinal .branch"),
            "stroke"
        ),
        Some("#123456")
    );
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}
