use merman_core::{MermaidEngine, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, GradientStop,
    LinearGradient, OrdinalSelector, Specified, ThemeColorValue, ThemePortabilityRequirement,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::{Value, json};

const MINIMAL: &str = "classDiagram\nDocument *-- Section : contains\n";
const INK: &str = "#b4236b";

fn rule(target: ThemeTarget, paint: CanvasPaint) -> ThemeRule {
    ThemeRule::new(target, ThemeStylePatch::default().with_fill(paint))
}

fn compile(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                rules
                    .into_iter()
                    .fold(ThemeRuleSet::default(), |set, rule| set.with_rule(rule)),
            ),
        )
        .unwrap()
}

fn config(html: bool, layout: &str) -> Value {
    json!({"htmlLabels": html, "class": {"htmlLabels": html}, "layout": layout})
}

fn render(
    source: &str,
    theme: &DiagramTheme,
    config: Value,
    requirement: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = common::install_parse_compatibility(
        theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(config),
        ),
    )
    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
    .unwrap()
    .unwrap()
    .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(requirement)
        .begin_session_with_theme(theme)
        .unwrap();
    family::prepare(
        &parsed,
        &merman_render::LayoutOptions::headless_svg_defaults(),
        session,
    )
    .unwrap()
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
}

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|value| value == class))
}

fn assert_relation_ink(svg: &str, css: &str, expected_labels: usize) {
    let document = roxmltree::Document::parse(svg).unwrap();
    let terminals: Vec<_> = document
        .descendants()
        .filter(|node| {
            (node.has_tag_name("text")
                || (node.has_tag_name("span") && has_class(*node, "edgeLabel")))
                && node.ancestors().any(|ancestor| {
                    ancestor.has_tag_name("g")
                        && (has_class(ancestor, "edgeLabel")
                            || has_class(ancestor, "edgeTerminals"))
                })
                && node.descendants().any(|child| {
                    child.is_text() && child.text().is_some_and(|text| !text.trim().is_empty())
                })
        })
        .collect();
    assert_eq!(terminals.len(), expected_labels, "{svg}");
    for terminal in terminals {
        assert!(
            terminal.ancestors().any(|node| {
                node.attribute("style").is_some_and(|style| {
                    style.contains(&format!("fill:{css} !important"))
                        && style.contains(&format!("color:{css} !important"))
                })
            }),
            "{svg}"
        );
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

fn gradient() -> CanvasPaint {
    CanvasPaint::LinearGradient(
        LinearGradient::new(
            90.0,
            [
                GradientStop::new(0.0, ThemeColorValue::parse("#123456").unwrap()).unwrap(),
                GradientStop::new(1.0, ThemeColorValue::parse("#abcdef").unwrap()).unwrap(),
            ],
        )
        .unwrap(),
    )
}

#[test]
fn single_local_edge_label_rule_paints_the_minimal_reading_probe() {
    for transparent in [false, true] {
        let paint = if transparent {
            CanvasPaint::Transparent
        } else {
            CanvasPaint::solid(INK).unwrap()
        };
        let theme = compile([rule(ThemeTarget::EdgeLabel, paint)]);
        for html in [false, true] {
            for layout in ["dagre", "elk"] {
                if layout == "elk" && !cfg!(feature = "layout-elk") {
                    continue;
                }
                let rendered = render(
                    MINIMAL,
                    &theme,
                    config(html, layout),
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                assert_relation_ink(
                    rendered.svg(),
                    if transparent { "transparent" } else { INK },
                    1,
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                assert!(
                    document
                        .descendants()
                        .filter(|node| {
                            node.ancestors().any(|ancestor| has_class(ancestor, "node"))
                        })
                        .all(|node| !node.attribute("style").unwrap_or("").contains(INK))
                );
                assert_evidence(rendered, 1, 0, 0);
            }
        }
    }
}

#[test]
fn local_edge_label_scope_includes_cardinalities_without_painting_notes_or_nodes() {
    let theme = compile([rule(
        ThemeTarget::EdgeLabel,
        CanvasPaint::solid(INK).unwrap(),
    )]);
    for html in [false, true] {
        for central in ["", " : contains"] {
            let source = format!(
                "classDiagram\nDocument \"1\" *-- \"many\" Section{central}\nnote for Document \"Reader note\"\n"
            );
            let rendered = render(
                &source,
                &theme,
                config(html, "dagre"),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            assert_relation_ink(rendered.svg(), INK, if central.is_empty() { 2 } else { 3 });
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert!(
                document
                    .descendants()
                    .filter(|node| {
                        node.ancestors().any(|ancestor| {
                            has_class(ancestor, "node") || has_class(ancestor, "note")
                        })
                    })
                    .all(|node| !node.attribute("style").unwrap_or("").contains(INK))
            );
            assert_evidence(rendered, 1, 0, 0);
        }
    }
}

#[test]
fn edge_label_and_shared_text_fill_keep_authored_order() {
    for local_last in [false, true] {
        let local = rule(ThemeTarget::EdgeLabel, CanvasPaint::solid(INK).unwrap());
        let shared = rule(ThemeTarget::Text, CanvasPaint::solid("#246801").unwrap());
        let theme = compile(if local_last {
            [shared, local]
        } else {
            [local, shared]
        });
        for html in [false, true] {
            let rendered = render(
                MINIMAL,
                &theme,
                config(html, "dagre"),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            assert_relation_ink(rendered.svg(), if local_last { INK } else { "#246801" }, 1);
            assert_evidence(
                rendered,
                if local_last { 2 } else { 1 },
                usize::from(!local_last),
                0,
            );
        }
    }
}

#[test]
fn unsupported_or_absent_edge_label_requests_are_fully_accounted() {
    let mut mixed = ThemeStylePatch::default().with_fill(CanvasPaint::solid(INK).unwrap());
    mixed.typography.font_weight = Specified::Value(600);
    for (label_rule, residual) in [
        (rule(ThemeTarget::EdgeLabel, gradient()), true),
        (ThemeRule::new(ThemeTarget::EdgeLabel, mixed), true),
        (
            rule(ThemeTarget::EdgeLabel, CanvasPaint::solid(INK).unwrap())
                .with_variant(ThemeVariant::Primary),
            false,
        ),
    ] {
        let theme = compile([label_rule]);
        for html in [false, true] {
            for (source, visible) in [
                (MINIMAL, true),
                ("classDiagram\nDocument *-- Section\n", false),
            ] {
                let rendered = render(
                    source,
                    &theme,
                    config(html, "dagre"),
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                assert_evidence(
                    rendered,
                    0,
                    usize::from(!visible || !residual),
                    usize::from(visible && residual),
                );
                if visible && residual {
                    assert!(
                        render(
                            source,
                            &theme,
                            config(html, "dagre"),
                            ThemePortabilityRequirement::RequirePortable
                        )
                        .is_err()
                    );
                }
            }
        }
    }
}

#[test]
fn edge_label_ordinal_uses_relation_identity_and_never_certifies_a_static_fallback() {
    let source = "classDiagram\nA --> B\nB --> C : visible\n";
    for ordinal in [1, 2, 3] {
        for fallback in [false, true] {
            let mut rules = Vec::new();
            if fallback {
                rules.push(rule(
                    ThemeTarget::EdgeLabel,
                    CanvasPaint::solid(INK).unwrap(),
                ));
            }
            rules.push(
                rule(ThemeTarget::EdgeLabel, CanvasPaint::Transparent)
                    .with_ordinal(OrdinalSelector::exact(ordinal).unwrap()),
            );
            let theme = compile(rules);
            for html in [false, true] {
                let rendered = render(
                    source,
                    &theme,
                    config(html, "dagre"),
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                let matched = ordinal == 2;
                assert_evidence(
                    rendered,
                    usize::from(fallback && !matched),
                    usize::from(!matched) + usize::from(fallback && matched),
                    usize::from(matched),
                );
                if matched {
                    assert!(
                        render(
                            source,
                            &theme,
                            config(html, "dagre"),
                            ThemePortabilityRequirement::RequirePortable
                        )
                        .is_err()
                    );
                }
            }
        }
    }
}

#[test]
fn edge_label_config_colors_keep_property_ownership_for_supported_and_unsupported_paint() {
    for paint in [CanvasPaint::solid(INK).unwrap(), gradient()] {
        let theme = compile([rule(ThemeTarget::EdgeLabel, paint)]);
        for html in [false, true] {
            for field in ["classText", "primaryTextColor", "textColor"] {
                let mut config = config(html, "dagre");
                config["themeVariables"] = json!({});
                config["themeVariables"][field] = json!("#246801");
                let rendered = render(
                    MINIMAL,
                    &theme,
                    config,
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                assert!(!rendered.svg().contains(&format!("color:{INK} !important")));
                assert_evidence(rendered, 0, 1, 0);
            }
        }
    }
}

#[test]
fn edge_label_html_colors_suppress_only_fully_owned_runs_and_keep_unknowns_residual() {
    for ordinal in [false, true] {
        let mut label_rule = rule(ThemeTarget::EdgeLabel, CanvasPaint::solid(INK).unwrap());
        if ordinal {
            label_rule = label_rule.with_ordinal(OrdinalSelector::exact(1).unwrap());
        }
        let theme = compile([label_rule]);
        for (label, owned) in [
            ("<span style='color:#246801'>owned</span>", true),
            ("<span style='color:var(--unknown)'>unknown</span>", false),
            ("<span class='external'>unknown</span>", false),
            ("inherited <span style='color:#246801'>owned</span>", false),
        ] {
            let source = format!("classDiagram\nDocument *-- Section : {label}\n");
            let rendered = render(
                &source,
                &theme,
                config(true, "dagre"),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            assert!(rendered.svg().contains(if label.contains("var(") {
                "color:var(--unknown)"
            } else if label.contains("class=") {
                "class=\"external\""
            } else {
                "color:#246801"
            }));
            assert_evidence(rendered, 0, usize::from(owned), usize::from(!owned));
        }
    }
}

mod common;
