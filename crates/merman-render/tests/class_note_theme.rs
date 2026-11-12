#![cfg(feature = "diagram-class")]

mod common;

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette,
    OrdinalSelector, Specified, ThemeAppearance, ThemeColorValue, ThemePortabilityRequirement,
    ThemePreset, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::{Value, json};

const SOURCE: &str = "classDiagram\nclass Job\nnote for Job \"Retry on failure\"\n";
const FILL: &str = "#152637";
const STROKE: &str = "#7b4ba8";
const INK: &str = "#b4236b";
const AUTHORED: &str = "#246801";

fn recipe(rules: Value) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile_recipe(
            serde_json::from_value(json!({
                "schema_version": 1,
                "kind": "design",
                "design": {"rules": rules}
            }))
            .unwrap(),
        )
        .unwrap()
}

fn note_recipe() -> DiagramTheme {
    recipe(json!([
        {"family": "class", "target": "note", "style": {"fill": FILL, "stroke": {"paint": STROKE}}},
        {"family": "class", "target": "note-label", "style": {"fill": INK}}
    ]))
}

fn compile(rule: ThemeRule) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
        .unwrap()
}

fn fill_rule(target: ThemeTarget, color: &str) -> ThemeRule {
    ThemeRule::new(
        target,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid(color).unwrap()),
    )
}

fn config(html: bool) -> Value {
    json!({"htmlLabels": html, "class": {"htmlLabels": html}})
}

fn render(
    source: &str,
    theme: Option<&DiagramTheme>,
    config: Value,
    requirement: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let engine =
        MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(config));
    let engine = if let Some(theme) = theme {
        common::install_parse_compatibility(theme, engine)
    } else {
        engine
    };
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let environment =
        RenderEnvironment::deterministic().with_theme_portability_requirement(requirement);
    let session = if let Some(theme) = theme {
        environment.begin_session_with_theme(theme)
    } else {
        environment.begin_session()
    }
    .unwrap();
    family::prepare(
        &parsed,
        &merman_render::LayoutOptions::headless_svg_defaults(),
        session,
    )?
    .render_svg(
        &SvgRenderOptions {
            diagram_id: Some("class-note-theme".into()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )
}

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|value| value == class))
}

fn note_label<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| has_class(*node, "noteLabel"))
        .expect("actual note label")
}

fn assert_note_surface(svg: &str, fill: &str, stroke: &str) {
    let document = roxmltree::Document::parse(svg).unwrap();
    let note = note_label(&document).parent().unwrap();
    let paths = note
        .children()
        .find(|node| has_class(*node, "outer-path"))
        .unwrap()
        .children()
        .filter(|node| node.has_tag_name("path"))
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 2, "{svg}");
    assert_eq!(paths[0].attribute("fill"), Some(fill), "{svg}");
    assert_eq!(paths[1].attribute("stroke"), Some(stroke), "{svg}");
    for path in paths {
        let style = path.attribute("style").unwrap();
        assert!(style.contains(&format!("fill:{fill} !important")), "{svg}");
        assert!(
            style.contains(&format!("stroke:{stroke} !important")),
            "{svg}"
        );
    }
}

fn assert_note_ink(svg: &str, html: bool, ink: &str) {
    let document = roxmltree::Document::parse(svg).unwrap();
    let label = note_label(&document);
    assert_eq!(
        label
            .descendants()
            .any(|node| node.has_tag_name("foreignObject")),
        html
    );
    let terminals = label
        .descendants()
        .filter(|node| {
            if html {
                node.has_tag_name("span") && has_class(*node, "nodeLabel")
            } else {
                node.has_tag_name("text")
            }
        })
        .filter(|node| {
            node.descendants().any(|child| {
                child.is_text() && child.text().is_some_and(|text| !text.trim().is_empty())
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(terminals.len(), 1, "{svg}");
    for terminal in terminals {
        let style = terminal.attribute("style").unwrap_or_default();
        assert!(style.contains(&format!("fill:{ink} !important")), "{svg}");
        assert!(style.contains(&format!("color:{ink} !important")), "{svg}");
    }
}

fn assert_evidence(
    rendered: family::RenderedFamilySvg,
    applied: usize,
    absent: usize,
    residual: usize,
) {
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), applied + absent + residual);
    assert_eq!(evidence.accounted_count(), evidence.required_count());
    assert_eq!(evidence.applied_count(), applied);
    assert_eq!(evidence.not_applicable_count(), absent);
    assert_eq!(evidence.theme_residual_count(), residual);
}

#[test]
fn native_cyberpunk_pairs_class_note_surface_and_ink_in_both_label_modes() {
    let theme = DiagramThemeCompiler::new()
        .compile_preset(ThemePreset::Cyberpunk)
        .unwrap();
    for html in [false, true] {
        let rendered = render(
            SOURCE,
            Some(&theme),
            config(html),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        assert_note_surface(rendered.svg(), "#351C29", "#FF4564");
        assert_note_ink(rendered.svg(), html, "#FFE9EF");
    }
}

#[test]
fn library_recipes_pair_class_note_paints_in_both_label_modes() {
    let reading = include_str!("../../../playground/src/themes/recipes/reading.theme.json");
    let prism = include_str!("../../../playground/src/themes/recipes/prism.theme.json");
    let cases = [
        (
            "aurora",
            include_str!("../../../playground/src/themes/recipes/aurora.theme.json"),
            None,
            ["#302849", "#BB9CFF", "#EBF5FF"],
        ),
        (
            "blueprint",
            include_str!("../../../playground/src/themes/recipes/blueprint.theme.json"),
            None,
            ["#CBE6EE", "#E3F3FA", "#173C5C"],
        ),
        (
            "chalk",
            include_str!("../../../playground/src/themes/recipes/chalk.theme.json"),
            None,
            ["#3B4032", "#F0D88B", "#F0D88B"],
        ),
        (
            "lacquer",
            include_str!("../../../playground/src/themes/recipes/lacquer.theme.json"),
            None,
            ["#291B23", "#D8BC82", "#D8BC82"],
        ),
        (
            "metro",
            include_str!("../../../playground/src/themes/recipes/metro.theme.json"),
            None,
            ["#63DCE5", "#63DCE5", "#071B2C"],
        ),
        (
            "nightfall",
            include_str!("../../../playground/src/themes/recipes/nightfall.theme.json"),
            None,
            ["#31394A", "#72839B", "#E8D4AC"],
        ),
        (
            "phosphor",
            include_str!("../../../playground/src/themes/recipes/phosphor.theme.json"),
            None,
            ["#332C18", "#F1C577", "#F1C577"],
        ),
        (
            "monograph",
            include_str!("../../../playground/src/themes/recipes/monograph.theme.json"),
            None,
            ["#F7F7F7", "#737373", "#161616"],
        ),
        (
            "reading-dark",
            reading,
            Some(ThemeAppearance::Dark),
            ["#403623", "#71839B", "#E9CF92"],
        ),
        (
            "prism-dark",
            prism,
            Some(ThemeAppearance::Dark),
            ["#30334A", "#6C819C", "#D2CEED"],
        ),
        // The new appearance-local rules must leave the existing light notes unchanged.
        (
            "reading-light",
            reading,
            Some(ThemeAppearance::Light),
            ["#fff5ad", "#FACC15", "#243246"],
        ),
        (
            "prism-light",
            prism,
            Some(ThemeAppearance::Light),
            ["#fff5ad", "#FACC15", "#223047"],
        ),
    ];
    let compiler = DiagramThemeCompiler::new();
    for (name, document, appearance, [fill, stroke, ink]) in cases {
        let theme = compiler
            .compile_document(serde_json::from_str(document).unwrap(), appearance)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        for html in [false, true] {
            let rendered = render(
                SOURCE,
                Some(&theme),
                config(html),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap_or_else(|error| panic!("{name} (HTML labels: {html}): {error}"));
            assert_note_surface(rendered.svg(), fill, stroke);
            assert_note_ink(rendered.svg(), html, ink);
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(
                evidence.accounted_count(),
                evidence.required_count(),
                "{name} (HTML labels: {html}): {evidence:?}"
            );
            assert_eq!(
                evidence.theme_residual_count(),
                0,
                "{name} (HTML labels: {html}): {evidence:?}"
            );
        }
    }
}

#[test]
fn direct_note_recipe_paints_real_terminals_and_accounts_for_both_rules() {
    let theme = note_recipe();
    for html in [false, true] {
        for source in [SOURCE, "classDiagram\nnote \"Retry on failure\"\n"] {
            let rendered = render(
                source,
                Some(&theme),
                config(html),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            assert_note_surface(rendered.svg(), FILL, STROKE);
            assert_note_ink(rendered.svg(), html, INK);
            assert_evidence(rendered, 2, 0, 0);
        }
    }
}

#[test]
fn note_label_uses_global_text_without_a_local_rule_and_keeps_author_order() {
    for rules in [
        json!([{"target": "text", "style": {"fill": AUTHORED}}]),
        json!([
            {"target": "note-label", "style": {"fill": INK}},
            {"target": "text", "style": {"fill": AUTHORED}}
        ]),
        json!([
            {"target": "text", "style": {"fill": AUTHORED}},
            {"target": "note-label", "style": {"fill": INK}}
        ]),
    ] {
        let local_last = rules.as_array().unwrap().last().unwrap()["target"] == "note-label";
        let theme = recipe(rules.clone());
        for html in [false, true] {
            let rendered = render(
                SOURCE,
                Some(&theme),
                config(html),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            assert_note_ink(
                rendered.svg(),
                html,
                if local_last { INK } else { AUTHORED },
            );
            let absent = usize::from(rules.as_array().unwrap().len() == 2 && !local_last);
            assert_evidence(
                rendered,
                rules.as_array().unwrap().len() - absent,
                absent,
                0,
            );
        }
    }
}

#[test]
fn source_and_site_colors_keep_each_note_property_owner() {
    let theme = note_recipe();
    for html in [false, true] {
        let label_owner = if html { "noteTextColor" } else { "classText" };
        for fields in [
            vec!["noteBkgColor"],
            vec!["noteBorderColor"],
            vec![label_owner],
            vec!["noteBkgColor", "noteBorderColor", label_owner],
        ] {
            for source_override in [false, true] {
                let mut config = config(html);
                let mut variables = json!({});
                for field in &fields {
                    variables[*field] = json!(AUTHORED);
                }
                let source = if source_override {
                    format!("---\nconfig:\n  themeVariables: {variables}\n---\n{SOURCE}")
                } else {
                    config["themeVariables"] = variables;
                    SOURCE.to_owned()
                };
                let rendered = render(
                    &source,
                    Some(&theme),
                    config,
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                assert_note_surface(
                    rendered.svg(),
                    if fields.contains(&"noteBkgColor") {
                        AUTHORED
                    } else {
                        FILL
                    },
                    if fields.contains(&"noteBorderColor") {
                        AUTHORED
                    } else {
                        STROKE
                    },
                );
                if fields.contains(&label_owner) {
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    assert!(
                        note_label(&document)
                            .descendants()
                            .all(|node| !node.attribute("style").unwrap_or_default().contains(INK))
                    );
                    let selector = if html {
                        ".noteLabel .edgeLabel{color:"
                    } else {
                        ".label text{fill:"
                    };
                    assert!(
                        rendered.svg().contains(&format!("{selector}{AUTHORED};}}")),
                        "{}",
                        rendered.svg()
                    );
                } else {
                    assert_note_ink(rendered.svg(), html, INK);
                }
                let absent = usize::from(
                    fields.contains(&"noteBkgColor") && fields.contains(&"noteBorderColor"),
                ) + usize::from(fields.contains(&label_owner));
                assert_evidence(rendered, 2 - absent, absent, 0);
            }
        }
    }
}

#[test]
fn absent_and_unsupported_note_requests_do_not_claim_applied_terminals() {
    let theme = note_recipe();
    for html in [false, true] {
        let rendered = render(
            "classDiagram\nclass Job\n",
            Some(&theme),
            config(html),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        assert_evidence(rendered, 0, 2, 0);
    }
    for target in [ThemeTarget::Note, ThemeTarget::NoteLabel] {
        for rule in [
            fill_rule(target, INK).with_variant(ThemeVariant::Default),
            fill_rule(target, INK).with_variant(ThemeVariant::Primary),
            fill_rule(target, INK).with_ordinal(OrdinalSelector::exact(1).unwrap()),
        ] {
            let ordinal = rule.ordinal().is_some();
            let inactive = rule.variant() == Some(ThemeVariant::Primary);
            let theme = compile(rule);
            for html in [false, true] {
                for present in [false, true] {
                    let rendered = render(
                        if present {
                            SOURCE
                        } else {
                            "classDiagram\nclass Job\n"
                        },
                        Some(&theme),
                        config(html),
                        ThemePortabilityRequirement::BestEffort,
                    )
                    .unwrap();
                    let applicable = present && !inactive;
                    assert_evidence(
                        rendered,
                        usize::from(applicable && !ordinal),
                        usize::from(!applicable),
                        usize::from(applicable && ordinal),
                    );
                }
            }
        }
    }
}

#[test]
fn transparent_note_paints_reach_terminals_with_applied_evidence() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(ThemeRule::new(
                        ThemeTarget::Note,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::Transparent)
                            .with_stroke(CanvasPaint::Transparent),
                    ))
                    .with_rule(ThemeRule::new(
                        ThemeTarget::NoteLabel,
                        ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
                    )),
            ),
        )
        .unwrap();
    for html in [false, true] {
        let rendered = render(
            SOURCE,
            Some(&theme),
            config(html),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        assert_note_surface(rendered.svg(), "transparent", "transparent");
        assert_note_ink(rendered.svg(), html, "transparent");
        assert_evidence(rendered, 2, 0, 0);
    }
}

#[test]
fn emitted_note_fill_does_not_certify_unsupported_sibling_facets() {
    for target in [ThemeTarget::Note, ThemeTarget::NoteLabel] {
        let mut patch = ThemeStylePatch::default().with_fill(CanvasPaint::solid(INK).unwrap());
        patch.typography.font_weight = Specified::Value(600);
        let theme = compile(ThemeRule::new(target, patch));
        for html in [false, true] {
            let rendered = render(
                SOURCE,
                Some(&theme),
                config(html),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            if target == ThemeTarget::NoteLabel {
                assert_note_ink(rendered.svg(), html, INK);
            } else {
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                assert!(
                    note_label(&document)
                        .parent()
                        .unwrap()
                        .descendants()
                        .any(
                            |node| node.has_tag_name("path") && node.attribute("fill") == Some(INK)
                        )
                );
            }
            assert_evidence(rendered, 0, 0, 1);
        }
    }
}

#[test]
fn equal_note_colors_keep_distinct_rule_and_source_origins_for_unsupported_siblings() {
    let theme = recipe(json!([
        {"target":"note","style":{"fill":FILL,"typography":{"font_weight":600}}},
        {"target":"note","style":{"fill":FILL,"stroke":{"paint":STROKE}}}
    ]));
    for html in [false, true] {
        for source_override in [false, true] {
            let source = if source_override {
                format!(
                    "---\nconfig:\n  themeVariables: {{noteBkgColor: '{FILL}', noteBorderColor: '{STROKE}'}}\n---\n{SOURCE}"
                )
            } else {
                SOURCE.to_owned()
            };
            let rendered = render(
                &source,
                Some(&theme),
                config(html),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            assert_note_surface(rendered.svg(), FILL, STROKE);
            // Equal CSS must still suppress the later paint rule when source-owned,
            // while the earlier rule's font weight remains an unsupported winner.
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 2);
            assert_eq!(evidence.accounted_count(), 2);
            assert_eq!(evidence.applied_count(), usize::from(!source_override));
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(source_override)
            );
            assert_eq!(evidence.theme_residual_count(), 1);
            let diagnostics = completion.report().theme_diagnostics();
            assert_eq!(diagnostics.len(), 1);
            let diagnostic = &diagnostics[0];
            assert_eq!(diagnostic.code(), "unsupported-typography");
            assert_eq!(diagnostic.subject(), "rule");
            assert_eq!(diagnostic.target(), Some(ThemeTarget::Note));
            assert_eq!(diagnostic.property(), None);
            assert_eq!(diagnostic.source_document(), Some("design"));
            assert_eq!(diagnostic.source_paths(), ["/design/rules/0/style"]);
            assert!(!diagnostic.generated());
        }
    }
}

#[test]
fn note_clear_retains_its_origin_and_blocks_palette_fallback_until_shadowed() {
    let mut clear = ThemeStylePatch::default();
    clear.paint.fill = Specified::Clear;
    clear.stroke.paint = Specified::Clear;
    let clear_rule = ThemeRule::new(ThemeTarget::Note, clear);
    let set_rule = ThemeRule::new(
        ThemeTarget::Note,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid(FILL).unwrap())
            .with_stroke(CanvasPaint::solid(STROKE).unwrap()),
    );
    for clear_last in [false, true] {
        let rules = if clear_last {
            [set_rule.clone(), clear_rule.clone()]
        } else {
            [clear_rule.clone(), set_rule.clone()]
        };
        let styles = rules.into_iter().fold(
            ThemeRuleSet::default().with_ordinal_palette(
                ThemeTarget::Note,
                OrdinalPalette::new([ThemeColorValue::parse(AUTHORED).unwrap()]).unwrap(),
            ),
            |set, rule| set.with_rule(rule),
        );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .unwrap();
        for html in [false, true] {
            let mut config = config(html);
            config["look"] = json!("classic");
            let rendered = render(
                SOURCE,
                Some(&theme),
                config.clone(),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            assert_note_surface(
                rendered.svg(),
                if clear_last { "#fff5ad" } else { FILL },
                // Class inherits Mermaid's scoped redux-color compatibility defaults.
                if clear_last { "#FACC15" } else { STROKE },
            );
            // Neither Clear nor an explicit paint permits the ordinal palette fallback.
            assert_evidence(
                rendered,
                usize::from(!clear_last),
                2,
                usize::from(clear_last),
            );
            assert_eq!(
                render(
                    SOURCE,
                    Some(&theme),
                    config,
                    ThemePortabilityRequirement::RequirePortable,
                )
                .is_err(),
                clear_last,
            );
        }
    }
}

#[test]
fn note_inline_html_color_owns_ink_without_suppressing_the_body_rule() {
    let theme = note_recipe();
    let source = format!(
        "classDiagram\nclass Job\nnote for Job \"<span style='color:{AUTHORED}'>Retry on failure</span>\"\n"
    );
    let rendered = render(
        &source,
        Some(&theme),
        config(true),
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap();
    assert_note_surface(rendered.svg(), FILL, STROKE);
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(note_label(&document).descendants().any(|node| {
        node.attribute("style")
            .is_some_and(|style| style.contains(&format!("color:{AUTHORED}")))
    }));
    assert_evidence(rendered, 1, 1, 0);
}

#[test]
fn untyped_and_empty_theme_keep_classic_note_paints_and_markup() {
    let empty = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    for html in [false, true] {
        let mut config = config(html);
        config["theme"] = json!("default");
        config["look"] = json!("classic");
        let baseline = render(
            SOURCE,
            None,
            config.clone(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let rendered = render(
            SOURCE,
            Some(&empty),
            config,
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        assert_note_surface(baseline.svg(), "#fff5ad", "#aaaa33");
        assert_eq!(rendered.svg(), baseline.svg());
        assert_evidence(rendered, 0, 0, 0);
    }
}
