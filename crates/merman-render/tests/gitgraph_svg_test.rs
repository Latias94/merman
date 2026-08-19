use merman_core::{DiagramFamilyId, Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, GradientStop,
    LinearGradient, OrdinalPalette, OrdinalSelector, PatternKind, PatternSpec, Specified,
    ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    ThemeTarget, ThemeVariant,
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
    gitgraph_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default().with_stroke(stroke),
    )
    .for_family(DiagramFamilyId::GIT_GRAPH)])
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

fn stylesheet(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid GitGraph SVG");
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .collect()
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
    let source_owned = format!("%%{{init: {source_config}}}%%\n{source}");
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
    for (case, stroke, expected_stroke) in [
        (
            "solid",
            CanvasPaint::solid("#123456").expect("valid GitGraph Edge stroke"),
            "#123456",
        ),
        ("transparent", CanvasPaint::Transparent, "transparent"),
    ] {
        let theme = gitgraph_edge_stroke_theme(stroke);
        let rendered = render_gitgraph_with_theme_and_engine(
            TWO_BRANCHES,
            &theme,
            Engine::new(),
            &format!("git-edge-{case}"),
        );
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid GitGraph Edge stroke SVG");
        let branches = branch_lines(&document);

        assert_eq!(branches.len(), 2, "{case}");
        for branch in branches {
            assert_eq!(
                branch.attribute("style"),
                Some(format!("stroke:{expected_stroke};").as_str()),
                "{case}: every actual .branch line must carry the direct terminal stroke",
            );
        }
        assert!(
            document
                .descendants()
                .filter(|node| node.has_tag_name("path") && has_class(node, "arrow"))
                .all(|arrow| arrow
                    .attribute("style")
                    .is_none_or(|style| !style.contains(expected_stroke))),
            "{case}: .arrowN belongs to the Node palette and must not count as Edge.stroke",
        );

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "{case}");
        assert_eq!(evidence.applied_count(), 1, "{case}");
        assert_eq!(evidence.not_applicable_count(), 0, "{case}");
        assert_eq!(evidence.theme_residual_count(), 0, "{case}");
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
    let css = stylesheet(rendered.svg());

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
fn gitgraph_edge_stroke_respects_commit_line_and_line_color_owners() {
    let theme = gitgraph_edge_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid GitGraph Edge stroke"),
    );

    let assert_owned = |owner: &str, rendered: family::RenderedFamilySvg| {
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid owned GitGraph Edge SVG");
        let branches = branch_lines(&document);
        let css = stylesheet(rendered.svg());

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
    assert!(!stylesheet(rendered.svg()).contains("#fedcba"));

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

    let legacy_theme = gitgraph_edge_rules_theme([ThemeRule::new(ThemeTarget::Edge, solid())
        .with_variant(ThemeVariant::Default)
        .for_family(DiagramFamilyId::GIT_GRAPH)]);
    let legacy_error = match try_render_gitgraph_with_theme_and_engine(
        TWO_BRANCHES,
        &legacy_theme,
        Engine::new(),
        "git-edge-qualified-legacy",
    ) {
        Ok(_) => panic!("the qualified GitGraph edge route must remain a compatibility residual"),
        Err(error) => error,
    };
    match legacy_error {
        merman_render::Error::LegacyFamilyThemeCompatibility {
            family_id,
            residual_count,
        } => {
            assert_eq!(family_id, DiagramFamilyId::GIT_GRAPH);
            assert_eq!(residual_count, 2);
        }
        other => panic!("expected GitGraph legacy compatibility residual, got {other}"),
    }

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
fn gitgraph_node_palette_reaches_terminal_commit_arrow_and_branch_label_surfaces() {
    let theme = gitgraph_node_palette_theme(&["#123456", "transparent"]);
    let rendered =
        render_gitgraph_with_theme_and_engine(TWO_BRANCHES, &theme, Engine::new(), "git-palette");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid GitGraph palette SVG");
    let css = stylesheet(rendered.svg());

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
    let css = stylesheet(rendered.svg());

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
        let css = stylesheet(rendered.svg());
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
    let css = stylesheet(rendered.svg());

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
            let css = stylesheet(rendered.svg());
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
    let css = stylesheet(rendered.svg());
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
            let css = stylesheet(rendered.svg());
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
