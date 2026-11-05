#![cfg(feature = "diagram-flowchart")]

//! Typed BPMN paints must reach real terminals without rewriting Mermaid source styles.

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette,
    OrdinalSelector, Specified, ThemeColorValue, ThemePortabilityRequirement, ThemeRule,
    ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family::{self, RenderedFamilySvg};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::{Value, json};

mod common;

const EVENTS: [(&str, &str, usize, &str); 4] = [
    ("Start", "bpmn-start", 1, "1.6px"),
    ("Intermediate", "bpmn-intermediate", 2, "1.4px"),
    ("Boundary", "bpmn-boundary", 2, "1.4px"),
    ("End", "bpmn-end", 1, "3.4px"),
];

fn event_source() -> String {
    let mut source = String::from("flowchart LR\n");
    for (id, shape, _, _) in EVENTS {
        source.push_str(&format!("{id}@{{ shape: {shape}, label: \"{id}\" }}\n"));
    }
    source
}

fn config(look: &str) -> Value {
    json!({"layout": "dagre", "look": look, "htmlLabels": false, "handDrawnSeed": 1})
}

fn compile(styles: ThemeRuleSet) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .unwrap()
}

fn render(source: &str, theme: Option<&DiagramTheme>, config: Value) -> RenderedFamilySvg {
    let engine = MermaidEngine::new()
        .with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(config));
    let engine = match theme {
        Some(theme) => common::install_parse_compatibility(theme, engine),
        None => engine,
    };
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let environment = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::BestEffort);
    let session = match theme {
        Some(theme) => environment.begin_session_with_theme(theme),
        None => environment.begin_session(),
    }
    .unwrap();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("bpmn-theme".to_owned()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap()
}

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|value| value.split_ascii_whitespace().any(|part| part == class))
}

fn node<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    id: &str,
) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| {
            node.attribute("data-id") == Some(id) && node.attribute("data-et") == Some("node")
        })
        .unwrap()
}

fn terminals<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    id: &str,
) -> Vec<roxmltree::Node<'a, 'input>> {
    let terminals = node(document, id)
        .descendants()
        .filter(|node| {
            has_class(*node, "bpmn-event-ring") || has_class(*node, "bpmn-annotation-bracket")
        })
        .collect::<Vec<_>>();
    assert!(!terminals.is_empty(), "missing BPMN terminal for {id}");
    terminals
}

fn property<'a>(node: roxmltree::Node<'a, '_>, property: &str) -> Option<&'a str> {
    node.attribute("style")?
        .rsplit(';')
        .find_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            (name.trim() == property).then(|| {
                let value = value.trim();
                value.strip_suffix("!important").unwrap_or(value).trim()
            })
        })
}

fn assert_unpainted_parts_unchanged(baseline: &str, themed: &str, id: &str) {
    let baseline_document = roxmltree::Document::parse(baseline).unwrap();
    let themed_document = roxmltree::Document::parse(themed).unwrap();
    for class in ["label", "bpmn-bounds"] {
        let baseline_part = node(&baseline_document, id)
            .children()
            .find(|node| has_class(*node, class))
            .unwrap();
        let themed_part = node(&themed_document, id)
            .children()
            .find(|node| has_class(*node, class))
            .unwrap();
        assert_eq!(
            &baseline[baseline_part.range()],
            &themed[themed_part.range()],
            "{id}: {class} must retain its exact source output"
        );
    }
}

fn assert_evidence(rendered: RenderedFamilySvg, applied: usize, absent: usize, residual: usize) {
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), applied + absent + residual);
    assert_eq!(evidence.applied_count(), applied);
    assert_eq!(evidence.not_applicable_count(), absent);
    assert_eq!(evidence.theme_residual_count(), residual);
}

#[test]
fn bpmn_empty_theme_preserves_source_and_config_output_bytes() {
    let source = format!(
        "{}Annotation@{{ shape: bpmn-annotation, label: \"Note\" }}\n\
         classDef owned fill:#ddeeff,stroke:#aabbcc\nclass Intermediate owned\n\
         style Start fill:#123456,stroke:#654321,stroke-width:9px\n\
         style Annotation stroke:#2468ac,stroke-width:7px\n\
         classDef bpmn-event fill:#abcdef\n\
         classDef bpmn-event-body stroke:#fedcba !important\n\
         classDef bpmn-annotation-body stroke:#aabbcc,stroke-width:6px\n",
        event_source()
    );
    let empty = compile(ThemeRuleSet::default());
    for look in ["classic", "neo", "handDrawn"] {
        for html in [false, true] {
            let mut config = config(look);
            config["htmlLabels"] = json!(html);
            config["themeVariables"] = json!({"mainBkg": "#fafafa", "nodeBorder": "#334455"});
            let baseline = render(&source, None, config.clone());
            let themed = render(&source, Some(&empty), config);
            assert_eq!(themed.svg(), baseline.svg(), "{look}, html={html}");
            let document = roxmltree::Document::parse(themed.svg()).unwrap();
            for (id, _, count, width) in EVENTS {
                let rings = terminals(&document, id);
                assert_eq!(rings.len(), count);
                for ring in rings {
                    assert_eq!(
                        ring.attribute("style"),
                        Some(format!("stroke-width: {width};").as_str())
                    );
                }
            }
            assert_eq!(
                terminals(&document, "Annotation")[0].attribute("style"),
                None
            );
            let start_body = node(&document, "Start")
                .children()
                .find(|node| has_class(*node, "bpmn-event-body"))
                .unwrap();
            assert_eq!(property(start_body, "fill"), Some("#123456"));
            assert_eq!(property(start_body, "stroke-width"), Some("9px"));
        }
    }
}

#[test]
fn bpmn_event_typed_paints_reach_every_ring_without_repainting_captions_or_bounds() {
    let source = event_source();
    for look in ["classic", "neo", "handDrawn"] {
        let baseline = render(&source, None, config(look));
        for (paint, expected) in [
            (CanvasPaint::solid("#123456").unwrap(), "#123456"),
            (CanvasPaint::Transparent, "none"),
        ] {
            let theme = compile(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default()
                        .with_fill(paint.clone())
                        .with_stroke(paint),
                )),
            );
            let rendered = render(&source, Some(&theme), config(look));
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            for (id, _, count, width) in EVENTS {
                let rings = terminals(&document, id);
                assert_eq!(rings.len(), count);
                for ring in rings {
                    assert_eq!(property(ring, "fill"), Some(expected), "{look}, {id}");
                    assert_eq!(property(ring, "stroke"), Some(expected), "{look}, {id}");
                    assert_eq!(property(ring, "stroke-width"), Some(width));
                }
                assert_unpainted_parts_unchanged(baseline.svg(), rendered.svg(), id);
            }
            assert_evidence(rendered, 1, 0, 0);
        }
    }
}

#[test]
fn bpmn_annotation_typed_stroke_width_and_dash_reach_the_bracket() {
    let source = "flowchart LR\nA@{ shape: bpmn-annotation, label: \"Note\" }\n";
    let theme = compile(
        ThemeRuleSet::default().with_rule(
            ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default()
                    .with_stroke(CanvasPaint::solid("#654321").unwrap())
                    .with_stroke_width(4.0)
                    .unwrap()
                    .with_stroke_dasharray([2.0, 5.0])
                    .unwrap(),
            )
            .with_variant(ThemeVariant::Default),
        ),
    );
    for look in ["classic", "neo", "handDrawn"] {
        let baseline = render(source, None, config(look));
        let rendered = render(source, Some(&theme), config(look));
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let bracket = terminals(&document, "A")[0];
        assert_eq!(property(bracket, "stroke"), Some("#654321"));
        assert_eq!(property(bracket, "stroke-width"), Some("4px"));
        assert_eq!(property(bracket, "stroke-dasharray"), Some("2 5"));
        assert_eq!(property(bracket, "fill"), None);
        assert_unpainted_parts_unchanged(baseline.svg(), rendered.svg(), "A");
        assert_evidence(rendered, 1, 0, 0);
    }
}

#[test]
fn bpmn_typed_terminals_preserve_class_and_inline_ownership_per_property() {
    for (shape, owned_property, source_value, typed_value, other_property, other_value) in [
        (
            "bpmn-intermediate",
            "fill",
            "#aabbcc",
            "#123456",
            "stroke",
            "#654321",
        ),
        (
            "bpmn-intermediate",
            "stroke",
            "#aabbcc",
            "#654321",
            "fill",
            "#123456",
        ),
        (
            "bpmn-intermediate",
            "stroke-dasharray",
            "3 7",
            "2 5",
            "stroke",
            "#654321",
        ),
        (
            "bpmn-annotation",
            "stroke",
            "#aabbcc",
            "#654321",
            "stroke-width",
            "4px",
        ),
        (
            "bpmn-annotation",
            "stroke-width",
            "7px",
            "4px",
            "stroke",
            "#654321",
        ),
        (
            "bpmn-annotation",
            "stroke-dasharray",
            "3 7",
            "2 5",
            "stroke",
            "#654321",
        ),
    ] {
        let patch = if shape == "bpmn-annotation" {
            ThemeStylePatch::default().with_stroke_width(4.0).unwrap()
        } else {
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap())
        }
        .with_stroke(CanvasPaint::solid("#654321").unwrap())
        .with_stroke_dasharray([2.0, 5.0])
        .unwrap();
        let theme =
            compile(ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Node, patch)));
        for value in [source_value, "var(--source-property)"] {
            for declaration in [
                format!("classDef owned {owned_property}:{value}\nclass A owned\n"),
                format!("style A {owned_property}:{value}\n"),
            ] {
                let source = format!(
                    "flowchart LR\nA@{{ shape: {shape}, label: \"Owned\" }}\n\
                     B@{{ shape: {shape}, label: \"Free\" }}\n{declaration}"
                );
                for look in ["classic", "neo", "handDrawn"] {
                    let rendered = render(&source, Some(&theme), config(look));
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    for terminal in terminals(&document, "A") {
                        assert_eq!(
                            property(terminal, owned_property),
                            None,
                            "{look}: {declaration}"
                        );
                        assert_eq!(property(terminal, other_property), Some(other_value));
                    }
                    for terminal in terminals(&document, "B") {
                        assert_eq!(property(terminal, other_property), Some(other_value));
                        assert_eq!(property(terminal, owned_property), Some(typed_value));
                    }
                }
            }
        }
    }
}

#[test]
fn bpmn_structural_class_ownership_follows_the_emitted_terminal_selectors() {
    for (shape, class, svg_matches, html_matches) in [
        ("bpmn-start", "bpmn-event", true, false),
        ("bpmn-start", "bpmn-event-start", true, false),
        ("bpmn-start", "bpmn-event-body", true, true),
        ("bpmn-start", "bpmn-event-end", false, false),
        ("bpmn-start", "bpmn-event-ring", false, false),
        ("bpmn-annotation", "bpmn-annotation", true, false),
        ("bpmn-annotation", "bpmn-annotation-body", true, true),
        ("bpmn-annotation", "bpmn-annotation-bracket", false, false),
        ("bpmn-annotation", "bpmn-event-body", false, false),
    ] {
        let annotation = shape == "bpmn-annotation";
        let (paint, element, patch) = if annotation {
            (
                "stroke",
                "path",
                ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#654321").unwrap()),
            )
        } else {
            (
                "fill",
                "circle",
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#654321").unwrap()),
            )
        };
        let theme =
            compile(ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Node, patch)));
        let source = format!(
            "flowchart LR\nA@{{ shape: {shape}, label: \"Step\" }}\n\
             classDef {class} {paint}:#aabbcc !important\n"
        );
        for (html, matches) in [(false, svg_matches), (true, html_matches)] {
            let mut config = config("classic");
            config["htmlLabels"] = json!(html);
            let rendered = render(&source, Some(&theme), config);
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let suffix = if html {
                ">*".to_owned()
            } else {
                format!(" {element}")
            };
            assert!(rendered.svg().contains(&format!(
                " .{class}{suffix}{{{paint}:rgb(170, 187, 204)!important;}}"
            )));
            for terminal in terminals(&document, "A") {
                assert_eq!(
                    property(terminal, paint),
                    (!matches).then_some("#654321"),
                    "{shape}, {class}, html={html}"
                );
            }
            assert_evidence(rendered, usize::from(!matches), usize::from(matches), 0);
        }
    }
}

#[test]
fn bpmn_structural_owners_block_only_their_property_even_for_unadmitted_values() {
    for (shape, class, owned_property, source_value) in [
        ("bpmn-intermediate", "bpmn-event-body", "fill", "#aabbcc"),
        ("bpmn-intermediate", "bpmn-event-body", "stroke", "#aabbcc"),
        (
            "bpmn-intermediate",
            "bpmn-event-body",
            "stroke-dasharray",
            "3 7",
        ),
        (
            "bpmn-annotation",
            "bpmn-annotation-body",
            "stroke",
            "#aabbcc",
        ),
        (
            "bpmn-annotation",
            "bpmn-annotation-body",
            "stroke-width",
            "7px",
        ),
        (
            "bpmn-annotation",
            "bpmn-annotation-body",
            "stroke-dasharray",
            "3 7",
        ),
    ] {
        let annotation = shape == "bpmn-annotation";
        let (first_property, first_value, first_patch) = if annotation {
            (
                "stroke-width",
                "4px",
                ThemeStylePatch::default().with_stroke_width(4.0).unwrap(),
            )
        } else {
            (
                "fill",
                "#123456",
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            )
        };
        let theme = compile(
            ThemeRuleSet::default()
                .with_rule(ThemeRule::new(ThemeTarget::Node, first_patch))
                .with_rule(ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#654321").unwrap()),
                ))
                .with_rule(ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default()
                        .with_stroke_dasharray([2.0, 5.0])
                        .unwrap(),
                )),
        );
        for value in [source_value, "var(--source-paint) !important"] {
            let source = format!(
                "flowchart LR\nA@{{ shape: {shape}, label: \"Step\" }}\n\
                 classDef {class} {owned_property}:{value}\n"
            );
            for html in [false, true] {
                let mut config = config("classic");
                config["htmlLabels"] = json!(html);
                let rendered = render(&source, Some(&theme), config);
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                for terminal in terminals(&document, "A") {
                    for (paint, expected) in [
                        (first_property, first_value),
                        ("stroke", "#654321"),
                        ("stroke-dasharray", "2 5"),
                    ] {
                        assert_eq!(
                            property(terminal, paint),
                            (paint != owned_property).then_some(expected),
                            "{shape}, {owned_property}:{value}, html={html}, {paint}"
                        );
                    }
                    if !annotation {
                        assert_eq!(property(terminal, "stroke-width"), Some("1.4px"));
                    }
                }
                assert_evidence(rendered, 2, 1, 0);
            }
        }
    }
}

#[test]
fn bpmn_structural_owner_does_not_certify_the_independent_inline_source_transport() {
    let source = "flowchart LR\nA@{ shape: bpmn-annotation, label: \"Step\" }\n\
                  classDef bpmn-annotation-body stroke:var(--source-paint) !important\n\
                  style A stroke:#aabbcc\n";
    let theme = compile(
        ThemeRuleSet::default()
            .with_rule(ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#654321").unwrap()),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_stroke_width(4.0).unwrap(),
            )),
    );
    for html in [false, true] {
        let mut config = config("classic");
        config["htmlLabels"] = json!(html);
        let baseline = render(source, None, config.clone()).into_completion();
        let baseline_evidence = merman_render::__private::family_evidence(baseline.report());
        assert!(baseline_evidence.source_residual_count() > 0);
        let rendered = render(source, Some(&theme), config);
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let bracket = terminals(&document, "A")[0];
        assert_eq!(property(bracket, "stroke"), None);
        assert_eq!(property(bracket, "stroke-width"), Some("4px"));
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
        assert_eq!(
            evidence.source_residual_count(),
            baseline_evidence.source_residual_count()
        );
    }
}

#[test]
fn bpmn_typed_terminals_respect_site_and_source_config_paint_owners() {
    for shape in ["bpmn-intermediate", "bpmn-annotation"] {
        let mut patch =
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#654321").unwrap());
        if shape != "bpmn-annotation" {
            patch = patch.with_fill(CanvasPaint::solid("#123456").unwrap());
        }
        let theme =
            compile(ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Node, patch)));
        for (key, owned_property, other_property, other_value) in [
            ("mainBkg", "fill", "stroke", "#654321"),
            ("nodeBorder", "stroke", "fill", "#123456"),
        ] {
            if shape == "bpmn-annotation" && key == "mainBkg" {
                continue;
            }
            for in_source in [false, true] {
                let mut config = config("classic");
                let diagram = format!("flowchart LR\nA@{{ shape: {shape}, label: \"Step\" }}\n");
                let source = if in_source {
                    format!("---\nconfig:\n  themeVariables:\n    {key}: '#aabbcc'\n---\n{diagram}")
                } else {
                    config["themeVariables"] = json!({});
                    config["themeVariables"][key] = json!("#aabbcc");
                    diagram
                };
                let baseline = render(&source, None, config.clone());
                let rendered = render(&source, Some(&theme), config);
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                for terminal in terminals(&document, "A") {
                    assert_eq!(
                        property(terminal, owned_property),
                        None,
                        "{key}, source={in_source}"
                    );
                    if shape != "bpmn-annotation" {
                        assert_eq!(property(terminal, other_property), Some(other_value));
                    }
                }
                if shape == "bpmn-annotation" {
                    assert_eq!(rendered.svg(), baseline.svg());
                    assert_evidence(rendered, 0, 1, 0);
                }
            }
        }
    }
}

#[test]
fn bpmn_event_terminals_use_existing_ordinal_palette_and_variant_winners() {
    let fill = |color| ThemeStylePatch::default().with_fill(CanvasPaint::solid(color).unwrap());
    let theme = compile(
        ThemeRuleSet::default()
            .with_rule(
                ThemeRule::new(ThemeTarget::Node, fill("#fedcba"))
                    .with_variant(ThemeVariant::Warning),
            )
            .with_rule(
                ThemeRule::new(ThemeTarget::Node, fill("#abcdef"))
                    .with_variant(ThemeVariant::Default)
                    .with_ordinal(OrdinalSelector::exact(2).unwrap()),
            )
            .with_ordinal_palette(
                ThemeTarget::Node,
                OrdinalPalette::new([
                    ThemeColorValue::parse("#102030").unwrap(),
                    ThemeColorValue::parse("#405060").unwrap(),
                ])
                .unwrap(),
            ),
    );
    let rendered = render(&event_source(), Some(&theme), config("classic"));
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    for (id, expected) in [
        ("Start", "#102030"),
        ("Intermediate", "#abcdef"),
        ("Boundary", "#102030"),
        ("End", "#405060"),
    ] {
        for terminal in terminals(&document, id) {
            assert_eq!(property(terminal, "fill"), Some(expected), "{id}");
        }
    }
    assert_evidence(rendered, 2, 1, 0);
}

#[test]
fn bpmn_event_clear_does_not_resurrect_a_shadowed_color_or_claim_support() {
    let mut clear = ThemeStylePatch::default();
    clear.paint.fill = Specified::Clear;
    let theme = compile(
        ThemeRuleSet::default()
            .with_rule(ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            ))
            .with_rule(ThemeRule::new(ThemeTarget::Node, clear)),
    );
    let source = event_source();
    let baseline = render(&source, None, config("classic"));
    let rendered = render(&source, Some(&theme), config("classic"));
    assert_eq!(rendered.svg(), baseline.svg());
    assert_evidence(rendered, 0, 1, 1);
}

#[test]
fn bpmn_event_ring_width_remains_fixed_and_keeps_a_mixed_rule_residual() {
    let theme = compile(
        ThemeRuleSet::default().with_rule(ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default()
                .with_stroke(CanvasPaint::solid("#654321").unwrap())
                .with_stroke_width(9.0)
                .unwrap()
                .with_stroke_dasharray([2.0, 5.0])
                .unwrap(),
        )),
    );
    let rendered = render(&event_source(), Some(&theme), config("classic"));
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    for (id, _, _, width) in EVENTS {
        for terminal in terminals(&document, id) {
            assert_eq!(property(terminal, "stroke"), Some("#654321"));
            assert_eq!(property(terminal, "stroke-dasharray"), Some("2 5"));
            assert_eq!(property(terminal, "stroke-width"), Some(width));
        }
    }
    assert_evidence(rendered, 0, 0, 1);
}

#[test]
fn bpmn_typed_terminal_proof_does_not_upgrade_independent_source_evidence() {
    for (shape, source_style, patch) in [
        (
            "bpmn-intermediate",
            "fill:#aabbcc",
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#654321").unwrap()),
        ),
        (
            "bpmn-annotation",
            "stroke:#aabbcc",
            ThemeStylePatch::default().with_stroke_width(4.0).unwrap(),
        ),
    ] {
        let theme =
            compile(ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Node, patch)));
        let source = format!(
            "flowchart LR\nA@{{ shape: {shape}, label: \"Step\" }}\nstyle A {source_style}\n"
        );
        for look in ["classic", "neo", "handDrawn"] {
            let baseline = render(&source, None, config(look)).into_completion();
            let baseline_evidence = merman_render::__private::family_evidence(baseline.report());
            assert!(baseline_evidence.source_residual_count() > 0);
            let rendered = render(&source, Some(&theme), config(look)).into_completion();
            let evidence = merman_render::__private::family_evidence(rendered.report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
            assert_eq!(
                evidence.source_residual_count(),
                baseline_evidence.source_residual_count(),
                "{look}, {shape}: a typed terminal must not certify source paint"
            );
        }
    }
}
