use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue,
    ThemePortabilityRequirement, ThemeRuleSet, ThemeTarget,
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

fn render_gitgraph_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    diagram_id: &str,
) -> family::RenderedFamilySvg {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed GitGraph")
        .expect("detect themed GitGraph");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable GitGraph session");
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed GitGraph")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
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
                !css.contains(&format!(
                    "#{diagram_id} .commit0{{stroke:#123456;fill:#123456;}}"
                )),
                "the typed palette must not overwrite the {owner}-owned commit surface"
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
fn gitgraph_neo_palette_is_not_applicable_when_source_or_site_owns_every_surface() {
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
                css.contains(&format!(
                    "#{diagram_id} .label0{{fill:#abcdef;stroke:url(#{diagram_id}-gradient);stroke-width:"
                )),
                "the {owner}-owned mainBkg must remain the terminal label owner"
            );
            assert!(
                !css.contains(&format!(
                    "#{diagram_id} .commit0{{stroke:#123456;fill:#123456;}}"
                )),
                "the typed palette must not overwrite the {owner}-owned commit surface"
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
