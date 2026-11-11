#![cfg(any(feature = "diagram-er", feature = "diagram-requirement"))]

mod common;

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::DiagramFamilyId;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, GradientStop,
    LinearGradient, OrdinalSelector, Specified, ThemeColorValue, ThemePortabilityRequirement,
    ThemePreset, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use roxmltree::{Document, Node};
use serde_json::{Value, json};

const BACKGROUND: &str = "#102537";
const CYBERPUNK_BACKGROUND: &str = "#0B1020";

struct Case {
    family: DiagramFamilyId,
    source: &'static str,
    without_relationship: &'static str,
}

const CASES: &[Case] = &[
    #[cfg(feature = "diagram-er")]
    Case {
        family: DiagramFamilyId::ER,
        source: "erDiagram\nA ||--o{ B : \"**owns** records\"\n",
        without_relationship: "erDiagram\nA\nB\n",
    },
    #[cfg(feature = "diagram-requirement")]
    Case {
        family: DiagramFamilyId::REQUIREMENT,
        source: "requirementDiagram\nrequirement req1 {\nid: R-1\ntext: Required behavior\n}\nelement elem1 {\ntype: simulation\n}\nreq1 - satisfies -> elem1\n",
        without_relationship: "requirementDiagram\nrequirement req1 {\nid: R-1\ntext: Required behavior\n}\n",
    },
];

fn config(html: bool, layout: &str) -> Value {
    json!({"look": "classic", "layout": layout, "htmlLabels": html,
        "flowchart": {"htmlLabels": html}})
}

fn render(
    source: &str,
    theme: Option<&DiagramTheme>,
    config: Value,
    requirement: ThemePortabilityRequirement,
    debug: &SvgDebugOptions,
) -> merman_render::Result<family::RenderedFamilySvg> {
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
    let environment =
        RenderEnvironment::deterministic().with_theme_portability_requirement(requirement);
    let session = match theme {
        Some(theme) => environment.begin_session_with_theme(theme).unwrap(),
        None => environment.begin_session().unwrap(),
    };
    family::prepare(
        &parsed,
        &merman_render::LayoutOptions::headless_svg_defaults(),
        session,
    )
    .unwrap()
    .render_svg(&SvgRenderOptions::default(), debug)
}

fn compile(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                rules
                    .into_iter()
                    .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule),
            ),
        )
        .unwrap()
}

fn rule(paint: CanvasPaint) -> ThemeRule {
    ThemeRule::new(
        ThemeTarget::EdgeLabelBackground,
        ThemeStylePatch::default().with_fill(paint),
    )
}

fn has_class(node: Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|value| value.split_ascii_whitespace().any(|value| value == class))
}

fn style_property<'a>(node: Node<'a, '_>, property: &str) -> Option<&'a str> {
    node.attribute("style")?
        .split(';')
        .rev()
        .find_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            (name.trim() == property).then(|| value.trim().trim_end_matches("!important").trim())
        })
}

fn assert_backgrounds(
    svg: &str,
    html: bool,
    outer: Option<&str>,
    inner: Option<&str>,
    count: usize,
) {
    let document = Document::parse(svg).unwrap();
    let groups: Vec<_> = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && has_class(*node, "label")
                && (node.attribute("data-id").is_some()
                    || node
                        .attribute("id")
                        .is_some_and(|id| id.starts_with("edge-label-")))
        })
        .filter(|node| {
            node.descendants().any(|child| {
                (child.has_tag_name("foreignObject")
                    || (child.has_tag_name("rect") && has_class(child, "background")))
                    && child
                        .attribute("width")
                        .and_then(|value| value.parse::<f64>().ok())
                        .is_some_and(|value| value > 0.0)
                    && child
                        .attribute("height")
                        .and_then(|value| value.parse::<f64>().ok())
                        .is_some_and(|value| value > 0.0)
            })
        })
        .collect();
    assert_eq!(groups.len(), count, "{svg}");
    for group in groups {
        if html {
            let div = group
                .descendants()
                .find(|node| node.has_tag_name("div"))
                .unwrap();
            let span = div
                .descendants()
                .find(|node| {
                    node.has_tag_name("span")
                        && (has_class(*node, "edgeLabel") || has_class(*node, "nodeLabel"))
                })
                .unwrap();
            assert_eq!(
                style_property(div, "background-color"),
                outer,
                "outer HTML backing: {svg}"
            );
            assert_eq!(
                style_property(span, "background-color"),
                inner,
                "nested HTML backing: {svg}"
            );
        } else {
            let rect = group
                .descendants()
                .find(|node| node.has_tag_name("rect") && has_class(*node, "background"))
                .unwrap();
            assert_eq!(style_property(rect, "fill"), inner, "SVG backing: {svg}");
        }
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

fn opaque_rgb(css: &str) -> [f64; 3] {
    assert_eq!(
        css.len(),
        7,
        "expected an opaque six-digit background: {css}"
    );
    assert!(css.starts_with('#'));
    [1, 3, 5]
        .map(|index| f64::from(u8::from_str_radix(&css[index..index + 2], 16).unwrap()) / 255.0)
}

fn luminance(rgb: [f64; 3]) -> f64 {
    let [r, g, b] = rgb.map(|channel| {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    });
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn assert_native_html_layer_contrast(svg: &str) {
    let document = Document::parse(svg).unwrap();
    let mut surface = opaque_rgb(CYBERPUNK_BACKGROUND);
    let div = document
        .descendants()
        .find(|node| node.has_tag_name("div") && has_class(*node, "labelBkg"))
        .unwrap();
    let span = div
        .descendants()
        .find(|node| node.has_tag_name("span") && has_class(*node, "edgeLabel"))
        .unwrap();
    for layer in [div, span] {
        // Read each real background in paint order. A six-digit static color has alpha 1,
        // so neither inherited translucent compatibility backing survives composition.
        let front = opaque_rgb(style_property(layer, "background-color").unwrap());
        let alpha = 1.0;
        surface = std::array::from_fn(|channel| {
            front[channel] * alpha + surface[channel] * (1.0 - alpha)
        });
    }
    let background = luminance(surface);
    let foreground = luminance(opaque_rgb("#E2EFF7"));
    assert!((foreground.max(background) + 0.05) / (foreground.min(background) + 0.05) >= 4.5);
}

#[test]
fn direct_recipe_paints_both_html_layers_and_svg_backgrounds() {
    for (fill, css) in [
        (json!(BACKGROUND), BACKGROUND),
        (json!("transparent"), "transparent"),
    ] {
        for variant in [None, Some("default")] {
            let mut recipe = json!({
                "styles": [{"kind": "rule", "target": "edge-label-background", "style": {"fill": fill}}]
            });
            if let Some(variant) = variant {
                recipe["styles"][0]["variant"] = json!(variant);
            }
            let theme = DiagramThemeCompiler::new()
                .compile_spec_wire(serde_json::from_value(recipe).unwrap())
                .unwrap();
            for case in CASES {
                for html in [false, true] {
                    for layout in ["dagre", "elk", "swimlane"] {
                        if (layout == "elk" && !cfg!(feature = "layout-elk"))
                            || (layout == "swimlane" && case.family != DiagramFamilyId::ER)
                        {
                            continue;
                        }
                        let rendered = render(
                            case.source,
                            Some(&theme),
                            config(html, layout),
                            ThemePortabilityRequirement::RequirePortable,
                            &SvgDebugOptions::default(),
                        )
                        .unwrap();
                        assert_backgrounds(rendered.svg(), html, Some(css), Some(css), 1);
                        assert_evidence(rendered, 1, 0, 0);
                    }
                }
            }
        }
    }
}

#[test]
fn builtin_cyberpunk_reaches_actual_relationship_backings() {
    let theme = DiagramThemeCompiler::new()
        .compile_preset(ThemePreset::Cyberpunk)
        .unwrap();
    for case in CASES {
        for html in [false, true] {
            let rendered = render(
                case.source,
                Some(&theme),
                config(html, "dagre"),
                ThemePortabilityRequirement::BestEffort,
                &SvgDebugOptions::default(),
            )
            .unwrap();
            assert_backgrounds(
                rendered.svg(),
                html,
                Some(CYBERPUNK_BACKGROUND),
                Some(CYBERPUNK_BACKGROUND),
                1,
            );
            assert!(
                rendered.svg().contains("fill:#E2EFF7"),
                "{}",
                rendered.svg()
            );
            if html {
                assert_native_html_layer_contrast(rendered.svg());
            }
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.accounted_count(), evidence.required_count());
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn source_backgrounds_keep_each_layers_existing_property_owner() {
    let theme = compile([rule(CanvasPaint::solid(BACKGROUND).unwrap())]);
    for case in CASES {
        for palette in ["default", "redux-color", "redux-dark-color"] {
            let redux = palette != "default";
            let fields = if case.family == DiagramFamilyId::ER {
                [
                    ("tertiaryColor", !redux, false),
                    ("edgeLabelBackground", false, !redux),
                    ("erEdgeLabelBackground", redux, redux),
                ]
            } else {
                [
                    ("edgeLabelBackground", !redux, true),
                    ("requirementEdgeLabelBackground", true, false),
                    ("relationLabelBackground", false, false),
                ]
            };
            for (field, owns_outer, owns_inner) in fields {
                for html in [false, true] {
                    // Frontmatter owns only the authored property. Initializing the
                    // palette keeps the other materialized fields available to typed
                    // paint, while preserving the family's effective CSS suppliers.
                    let mut source_config = json!({"themeVariables": {}});
                    source_config["themeVariables"][field] = json!("#ab3571");
                    let source = format!("---\nconfig: {source_config}\n---\n{}", case.source);
                    let mut cfg = config(html, "dagre");
                    cfg["theme"] = json!(palette);
                    let rendered = render(
                        &source,
                        Some(&theme),
                        cfg,
                        ThemePortabilityRequirement::BestEffort,
                        &SvgDebugOptions::default(),
                    )
                    .unwrap();
                    assert_backgrounds(
                        rendered.svg(),
                        html,
                        (!owns_outer).then_some(BACKGROUND),
                        (!owns_inner).then_some(BACKGROUND),
                        1,
                    );
                    if owns_outer {
                        let css = if field == "tertiaryColor" {
                            "rgba(171, 53, 113, 0.5)"
                        } else {
                            "#ab3571"
                        };
                        assert!(
                            rendered
                                .svg()
                                .contains(&format!(".labelBkg{{background-color:{css};}}")),
                            "{}",
                            rendered.svg()
                        );
                    }
                    if owns_inner {
                        assert!(
                            rendered
                                .svg()
                                .contains(".edgeLabel{background-color:#ab3571;}"),
                            "{}",
                            rendered.svg()
                        );
                        assert!(
                            rendered
                                .svg()
                                .contains(".edgeLabel .label rect{fill:#ab3571;}"),
                            "{}",
                            rendered.svg()
                        );
                    }
                    if field == "relationLabelBackground" {
                        assert!(
                            rendered.svg().contains(".reqLabelBox{fill:#ab3571;"),
                            "{}",
                            rendered.svg()
                        );
                    }
                    let inherited = !owns_inner || (html && !owns_outer);
                    assert_evidence(rendered, usize::from(inherited), usize::from(!inherited), 0);
                }
            }
        }
    }
}

#[cfg(feature = "diagram-er")]
#[test]
fn er_redux_family_background_keeps_precedence_over_generic_and_tertiary_values() {
    let case = CASES
        .iter()
        .find(|case| case.family == DiagramFamilyId::ER)
        .unwrap();
    let theme = compile([rule(CanvasPaint::solid(BACKGROUND).unwrap())]);
    for (palette, source_background) in
        [("redux-color", "#FFFFFF"), ("redux-dark-color", "#16141F")]
    {
        for family_override in [false, true] {
            for html in [false, true] {
                let mut cfg = config(html, "dagre");
                cfg["theme"] = json!(palette);
                cfg["themeVariables"] = json!({
                    "edgeLabelBackground": "#45ab71", "tertiaryColor": "#ab3571"
                });
                if family_override {
                    cfg["themeVariables"]["erEdgeLabelBackground"] = json!("#7035ab");
                }
                let rendered = render(
                    case.source,
                    Some(&theme),
                    cfg,
                    ThemePortabilityRequirement::BestEffort,
                    &SvgDebugOptions::default(),
                )
                .unwrap();
                let typed = (!family_override).then_some(BACKGROUND);
                assert_backgrounds(rendered.svg(), html, typed, typed, 1);
                let source_background = if family_override {
                    "#7035ab"
                } else {
                    source_background
                };
                assert!(rendered.svg().contains(&format!(
                    ".labelBkg{{background-color:{source_background};}}"
                )));
                assert!(rendered.svg().contains(&format!(
                    ".edgeLabel{{background-color:{source_background};}}"
                )));
                assert!(rendered.svg().contains(&format!(
                    ".edgeLabel .label rect{{fill:{source_background};}}"
                )));
                assert_evidence(
                    rendered,
                    usize::from(!family_override),
                    usize::from(family_override),
                    0,
                );
            }
        }
    }
}

#[test]
fn independently_owned_layers_do_not_restore_typed_paint_under_source_colors() {
    let theme = compile([rule(CanvasPaint::solid(BACKGROUND).unwrap())]);
    for case in CASES {
        for html in [false, true] {
            let mut cfg = config(html, "dagre");
            cfg["theme"] = json!("default");
            cfg["themeVariables"] = if case.family == DiagramFamilyId::ER {
                json!({"tertiaryColor": "#ab3571", "edgeLabelBackground": "#45ab71"})
            } else {
                json!({"requirementEdgeLabelBackground": "#ab3571", "edgeLabelBackground": "#45ab71"})
            };
            let rendered = render(
                case.source,
                Some(&theme),
                cfg,
                ThemePortabilityRequirement::BestEffort,
                &SvgDebugOptions::default(),
            )
            .unwrap();
            assert_backgrounds(rendered.svg(), html, None, None, 1);
            assert_evidence(rendered, 0, 1, 0);
        }
    }
}

#[test]
fn absent_and_nondefault_background_domains_are_not_applicable() {
    for case in CASES {
        for html in [false, true] {
            for (source, variant) in [
                (case.without_relationship, ThemeVariant::Default),
                (case.source, ThemeVariant::Primary),
            ] {
                let theme =
                    compile([rule(CanvasPaint::solid(BACKGROUND).unwrap()).with_variant(variant)]);
                let rendered = render(
                    source,
                    Some(&theme),
                    config(html, "dagre"),
                    ThemePortabilityRequirement::RequirePortable,
                    &SvgDebugOptions::default(),
                )
                .unwrap();
                assert_backgrounds(
                    rendered.svg(),
                    html,
                    None,
                    None,
                    usize::from(variant == ThemeVariant::Primary),
                );
                assert_evidence(rendered, 0, 1, 0);
            }
        }
    }
}

#[test]
fn unsupported_gradient_clear_ordinal_and_sibling_facets_remain_residual() {
    let gradient = CanvasPaint::LinearGradient(
        LinearGradient::new(
            90.0,
            [
                GradientStop::new(0.0, ThemeColorValue::parse("#123456").unwrap()).unwrap(),
                GradientStop::new(1.0, ThemeColorValue::parse("#abcdef").unwrap()).unwrap(),
            ],
        )
        .unwrap(),
    );
    let mut clear = ThemeStylePatch::default();
    clear.paint.fill = Specified::Clear;
    let mixed = ThemeStylePatch::default()
        .with_fill(CanvasPaint::solid(BACKGROUND).unwrap())
        .with_stroke(CanvasPaint::solid("#123456").unwrap());
    for case in CASES {
        for html in [false, true] {
            for (candidate, paints) in [
                (rule(gradient.clone()), false),
                (
                    ThemeRule::new(ThemeTarget::EdgeLabelBackground, clear.clone()),
                    false,
                ),
                (
                    rule(CanvasPaint::Transparent).with_ordinal(OrdinalSelector::exact(1).unwrap()),
                    false,
                ),
                (
                    ThemeRule::new(ThemeTarget::EdgeLabelBackground, mixed.clone()),
                    true,
                ),
            ] {
                let theme = compile([candidate]);
                let rendered = render(
                    case.source,
                    Some(&theme),
                    config(html, "dagre"),
                    ThemePortabilityRequirement::BestEffort,
                    &SvgDebugOptions::default(),
                )
                .unwrap();
                assert_backgrounds(
                    rendered.svg(),
                    html,
                    paints.then_some(BACKGROUND),
                    paints.then_some(BACKGROUND),
                    1,
                );
                assert_evidence(rendered, 0, 0, 1);
                assert!(
                    render(
                        case.source,
                        Some(&theme),
                        config(html, "dagre"),
                        ThemePortabilityRequirement::RequirePortable,
                        &SvgDebugOptions::default()
                    )
                    .is_err()
                );
            }
        }
    }
}

#[cfg(feature = "diagram-requirement")]
#[test]
fn requirement_background_ordinals_follow_relationships_across_self_loop_segments() {
    const LOOP: &str = "requirementDiagram\nrequirement req1 {\nid: R-1\ntext: Required behavior\n}\nreq1 - traces -> req1\n";
    let followed =
        format!("{LOOP}element elem1 {{\ntype: simulation\n}}\nreq1 - satisfies -> elem1\n");
    for (source, count) in [(LOOP, 1), (followed.as_str(), 2)] {
        for ordinal in 1..=3 {
            let theme = compile([rule(CanvasPaint::Transparent)
                .with_ordinal(OrdinalSelector::exact(ordinal).unwrap())]);
            for html in [false, true] {
                for layout in ["dagre", "elk"] {
                    if layout == "elk" && !cfg!(feature = "layout-elk") {
                        continue;
                    }
                    let rendered = render(
                        source,
                        Some(&theme),
                        config(html, layout),
                        ThemePortabilityRequirement::BestEffort,
                        &SvgDebugOptions::default(),
                    )
                    .unwrap();
                    let applicable = ordinal <= count;
                    assert_evidence(
                        rendered,
                        0,
                        usize::from(!applicable),
                        usize::from(applicable),
                    );
                    assert_eq!(
                        render(
                            source,
                            Some(&theme),
                            config(html, layout),
                            ThemePortabilityRequirement::RequirePortable,
                            &SvgDebugOptions::default()
                        )
                        .is_err(),
                        applicable,
                        "{layout} html={html} ordinal={ordinal} relationships={count}"
                    );
                }
            }
        }
    }
}

#[cfg(feature = "diagram-er")]
#[test]
fn er_hidden_labels_preserve_relationship_ordinals_and_debug_visibility() {
    let source = "erDiagram\nA ||--|| B : \"\"\nB ||--|| C : visible\n";
    for html in [false, true] {
        for ordinal in [1, 2, 3] {
            let theme = compile([rule(CanvasPaint::Transparent)
                .with_ordinal(OrdinalSelector::exact(ordinal).unwrap())]);
            let rendered = render(
                source,
                Some(&theme),
                config(html, "dagre"),
                ThemePortabilityRequirement::BestEffort,
                &SvgDebugOptions::default(),
            )
            .unwrap();
            assert_evidence(
                rendered,
                0,
                usize::from(ordinal != 2),
                usize::from(ordinal == 2),
            );
        }
        let theme = compile([rule(CanvasPaint::solid(BACKGROUND).unwrap())]);
        let debug = SvgDebugOptions {
            include_edges: false,
            ..Default::default()
        };
        let rendered = render(
            source,
            Some(&theme),
            config(html, "dagre"),
            ThemePortabilityRequirement::BestEffort,
            &debug,
        )
        .unwrap();
        assert_backgrounds(rendered.svg(), html, None, None, 0);
        assert_evidence(rendered, 0, 1, 0);
    }
}

#[test]
fn default_and_empty_typed_theme_keep_the_same_svg_output() {
    let empty = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    for case in CASES {
        for html in [false, true] {
            let baseline = render(
                case.source,
                None,
                config(html, "dagre"),
                ThemePortabilityRequirement::BestEffort,
                &SvgDebugOptions::default(),
            )
            .unwrap();
            let empty_themed = render(
                case.source,
                Some(&empty),
                config(html, "dagre"),
                ThemePortabilityRequirement::BestEffort,
                &SvgDebugOptions::default(),
            )
            .unwrap();
            assert_eq!(empty_themed.svg(), baseline.svg());
            assert_backgrounds(baseline.svg(), html, None, None, 1);
        }
    }
}
