#![cfg(feature = "diagram-flowchart")]

//! Generated class selectors must retain ownership only on the terminals they actually match.

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, Specified,
    ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family::{self, RenderedFamilySvg};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::{Value, json};

mod common;

const SHAPES: [(&str, &str, &str); 3] = [
    ("Process", "A[Box]", "rect"),
    ("RoundedRectangle", "A(Round)", "rect"),
    ("Diamond", "A{Decision}", "polygon"),
];

const CLUSTER_SOURCE: &str = "flowchart TD\nsubgraph Group[Heading]\nA[Inner]\nend\n";

fn config(html: bool) -> Value {
    json!({"layout": "dagre", "look": "classic", "htmlLabels": html,
        "flowchart": {"htmlLabels": html}})
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

fn fill(target: ThemeTarget) -> ThemeRule {
    ThemeRule::new(
        target,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
    )
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
                diagram_id: Some("structural-class-theme".to_owned()),
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

fn node<'a, 'input>(document: &'a roxmltree::Document<'input>) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| {
            node.attribute("data-id") == Some("A") && node.attribute("data-et") == Some("node")
        })
        .unwrap()
}

fn body<'a, 'input>(document: &'a roxmltree::Document<'input>) -> roxmltree::Node<'a, 'input> {
    node(document)
        .children()
        .find(|node| {
            (node.has_tag_name("rect") || node.has_tag_name("polygon"))
                && has_class(*node, "label-container")
        })
        .unwrap()
}

fn cluster<'a, 'input>(document: &'a roxmltree::Document<'input>) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| {
            node.attribute("data-id") == Some("Group")
                && node.attribute("data-et") == Some("cluster")
        })
        .unwrap()
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

fn assert_rule(svg: &str, class: &str, suffix: &str, declarations: &str) {
    assert!(
        svg.contains(&format!(" .{class}{suffix}{{{declarations}}}")),
        "missing generated .{class}{suffix} rule: {declarations}"
    );
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
fn structural_shape_paints_respect_actual_root_and_nodes_selectors() {
    // Repro: SVG .nodes rect owns Box, but typed inline !important used to repaint it.
    let theme = compile([ThemeRule::new(
        ThemeTarget::Node,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").unwrap())
            .with_stroke(CanvasPaint::solid("#654321").unwrap()),
    )]);
    for (name, syntax, element) in SHAPES {
        for class in ["root", "nodes"] {
            let source =
                format!("flowchart LR\n{syntax}\nclassDef {class} fill:#aabbcc,stroke:#ccbbaa\n");
            for html in [false, true] {
                let rendered = render(&source, Some(&theme), config(html));
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let terminal = body(&document);
                assert_eq!(terminal.tag_name().name(), element);
                assert!(
                    terminal
                        .ancestors()
                        .skip(1)
                        .any(|node| has_class(node, class))
                );
                let suffix = if html {
                    ">*".to_owned()
                } else {
                    format!(" {element}")
                };
                assert_rule(
                    rendered.svg(),
                    class,
                    &suffix,
                    "fill:rgb(170, 187, 204)!important;stroke:rgb(204, 187, 170)!important;",
                );
                if html {
                    // >* selects the node wrapper, whose inherited paint cannot own its rect.
                    assert!(!has_class(terminal.parent_element().unwrap(), class));
                    assert_eq!(property(terminal, "fill"), Some("#123456"));
                    assert_eq!(property(terminal, "stroke"), Some("#654321"));
                } else {
                    // The explicit source rule matches the real terminal; no stronger typed
                    // inline declaration may replace either of its paints.
                    assert_ne!(
                        property(terminal, "fill"),
                        Some("#123456"),
                        "{name}, {class}"
                    );
                    assert_ne!(
                        property(terminal, "stroke"),
                        Some("#654321"),
                        "{name}, {class}"
                    );
                }
                assert_evidence(rendered, usize::from(html), usize::from(!html), 0);
            }
        }
    }
}

#[test]
fn structural_leaf_classes_do_not_own_their_own_shape_paint() {
    let theme = compile([fill(ThemeTarget::Node)]);
    for (_, syntax, element) in SHAPES {
        for class in ["basic", "label-container"] {
            let source = format!("flowchart LR\n{syntax}\nclassDef {class} fill:#aabbcc\n");
            for html in [false, true] {
                let rendered = render(&source, Some(&theme), config(html));
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let terminal = body(&document);
                assert!(terminal.children().all(|child| !child.is_element()));
                assert!(
                    !terminal
                        .ancestors()
                        .skip(1)
                        .any(|node| has_class(node, class))
                );
                assert_rule(
                    rendered.svg(),
                    class,
                    &if html {
                        ">*".to_owned()
                    } else {
                        format!(" {element}")
                    },
                    "fill:rgb(170, 187, 204)!important;",
                );
                assert_eq!(property(terminal, "fill"), Some("#123456"));
                assert_evidence(rendered, 1, 0, 0);
            }
        }
    }
}

#[test]
fn structural_shape_ownership_is_per_property_in_a_mixed_rule() {
    let theme = compile([ThemeRule::new(
        ThemeTarget::Node,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").unwrap())
            .with_stroke(CanvasPaint::solid("#654321").unwrap()),
    )]);
    for (_, syntax, _) in SHAPES {
        for class in ["root", "nodes"] {
            let source = format!("flowchart LR\n{syntax}\nclassDef {class} fill:#aabbcc\n");
            for html in [false, true] {
                let rendered = render(&source, Some(&theme), config(html));
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let terminal = body(&document);
                if html {
                    assert_eq!(property(terminal, "fill"), Some("#123456"));
                } else {
                    assert_ne!(property(terminal, "fill"), Some("#123456"));
                }
                assert_eq!(property(terminal, "stroke"), Some("#654321"));
                assert_evidence(rendered, 1, 0, 0);
            }
        }
    }
}

#[test]
fn structural_classdefs_preserve_author_order_and_source_config_priority() {
    let theme = compile([fill(ThemeTarget::Node)]);
    for reverse in [false, true] {
        let mut classes = [
            ("root", "#aabbcc", "rgb(170, 187, 204)"),
            ("nodes", "#ccbbaa", "rgb(204, 187, 170)"),
        ];
        if reverse {
            classes.reverse();
        }
        let diagram = format!(
            "flowchart LR\nA[Box]\nclassDef {} fill:{}\nclassDef {} fill:{}\n",
            classes[0].0, classes[0].1, classes[1].0, classes[1].1,
        );
        for in_source in [false, true] {
            for html in [false, true] {
                let mut config = config(html);
                let source = if in_source {
                    format!(
                        "---\nconfig:\n  themeVariables:\n    mainBkg: '#ddccaa'\n---\n{diagram}"
                    )
                } else {
                    config["themeVariables"] = json!({"mainBkg": "#ddccaa"});
                    diagram.clone()
                };
                let baseline = render(&source, None, config.clone());
                let rendered = render(&source, Some(&theme), config);
                // SVG source paint outranks configuration; HTML parent-only selectors leave
                // configuration as the owner. Neither case permits a typed fill replacement.
                assert_eq!(rendered.svg(), baseline.svg());
                let suffix = if html { ">*" } else { " rect" };
                let rules = classes.map(|(class, _, color)| {
                    format!(" .{class}{suffix}{{fill:{color}!important;}}")
                });
                assert!(
                    rendered.svg().find(&rules[0]).unwrap()
                        < rendered.svg().find(&rules[1]).unwrap()
                );
                assert_evidence(rendered, 0, 1, 0);
            }
        }
    }
}

#[test]
fn structural_clear_and_mixed_rules_count_only_actual_source_owners() {
    // A matched source fill makes Clear inapplicable; a parent-only HTML selector does not.
    for mixed in [false, true] {
        let mut patch = ThemeStylePatch::default();
        patch.paint.fill = Specified::Clear;
        if mixed {
            patch = patch.with_stroke(CanvasPaint::solid("#654321").unwrap());
        }
        let theme = compile([
            fill(ThemeTarget::Node),
            ThemeRule::new(ThemeTarget::Node, patch),
        ]);
        for class in ["root", "nodes"] {
            let source = format!("flowchart LR\nA[Box]\nclassDef {class} fill:#aabbcc\n");
            for html in [false, true] {
                let baseline = render(&source, None, config(html));
                let rendered = render(&source, Some(&theme), config(html));
                let baseline_document = roxmltree::Document::parse(baseline.svg()).unwrap();
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let terminal = body(&document);
                assert_eq!(
                    property(terminal, "fill"),
                    property(body(&baseline_document), "fill")
                );
                assert_ne!(property(terminal, "fill"), Some("#123456"));
                if mixed {
                    assert_eq!(property(terminal, "stroke"), Some("#654321"));
                } else {
                    assert_eq!(rendered.svg(), baseline.svg());
                }
                assert_evidence(
                    rendered,
                    usize::from(mixed && !html),
                    1 + usize::from(!mixed && !html),
                    usize::from(html),
                );
            }
        }
    }
}

#[test]
fn structural_source_rx_or_ry_supersedes_the_whole_typed_radius() {
    // Repro: .root rect { rx:3px!important } was lost to typed rx/ry inline declarations.
    let mut patch = ThemeStylePatch::default();
    patch.geometry.radius = Specified::Value(11.0);
    let theme = compile([ThemeRule::new(ThemeTarget::Node, patch)]);
    for (_, syntax, _) in SHAPES.into_iter().take(2) {
        for class in ["root", "nodes"] {
            for axis in ["rx", "ry"] {
                let source = format!("flowchart LR\n{syntax}\nclassDef {class} {axis}:3px\n");
                for html in [false, true] {
                    let baseline = render(&source, None, config(html));
                    let rendered = render(&source, Some(&theme), config(html));
                    let baseline_document = roxmltree::Document::parse(baseline.svg()).unwrap();
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    let terminal = body(&document);
                    assert_rule(
                        rendered.svg(),
                        class,
                        if html { ">*" } else { " rect" },
                        &format!("{axis}:3px!important;"),
                    );
                    for coordinate in ["rx", "ry"] {
                        if html {
                            assert_eq!(terminal.attribute(coordinate), Some("11"));
                            assert_eq!(property(terminal, coordinate), Some("11px"));
                        } else {
                            assert_eq!(
                                terminal.attribute(coordinate),
                                body(&baseline_document).attribute(coordinate)
                            );
                            assert_eq!(
                                property(terminal, coordinate),
                                property(body(&baseline_document), coordinate)
                            );
                        }
                    }
                    assert_evidence(rendered, usize::from(html), usize::from(!html), 0);
                }
            }
        }
    }
}

#[test]
fn structural_label_color_owns_svg_glyphs_but_shape_fill_does_not() {
    let theme = compile([fill(ThemeTarget::NodeLabel)]);
    for class in ["root", "nodes"] {
        for paint in ["color", "fill"] {
            let source = format!("flowchart LR\nA[Glyph]\nclassDef {class} {paint}:#aabbcc\n");
            let rendered = render(&source, Some(&theme), config(false));
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let label = node(&document)
                .descendants()
                .find(|node| node.has_tag_name("text"))
                .unwrap();
            let glyph = label
                .descendants()
                .find(|node| {
                    node.has_tag_name("tspan") && node.children().any(|child| child.is_text())
                })
                .unwrap();
            assert!(glyph.ancestors().skip(1).any(|node| has_class(node, class)));
            if paint == "color" {
                // createCssStyles translates source color into a direct tspan fill. Its
                // foreground wins even over typed paint inherited from the parent text.
                assert_rule(
                    rendered.svg(),
                    class,
                    " tspan",
                    "fill:rgb(170, 187, 204)!important;",
                );
                assert_ne!(property(label, "fill"), Some("#123456"));
                assert_ne!(property(glyph, "fill"), Some("#123456"));
                assert_evidence(rendered, 0, 1, 0);
            } else {
                assert!(!rendered.svg().contains(&format!(" .{class} tspan{{")));
                assert_eq!(property(label, "fill"), Some("#123456"));
                assert_evidence(rendered, 1, 0, 0);
            }
        }
    }
}

#[test]
fn structural_html_fonts_own_matched_spans_without_owning_native_text() {
    // Repro: .nodes span { font-family:serif!important } directly owns nodeLabel, but
    // the typed font was still appended to that span and reported as Applied.
    let mut stack = ThemeStylePatch::default();
    stack.typography.font_stack = Specified::Value(FontStack::single("monospace").unwrap());
    let mut size = ThemeStylePatch::default();
    size.typography.font_size_px = Specified::Value(29.0);
    let theme = compile([
        ThemeRule::new(ThemeTarget::NodeLabel, stack),
        ThemeRule::new(ThemeTarget::NodeLabel, size),
    ]);
    for class in ["root", "nodes"] {
        for (owned_property, source_value, typed_value, other_property, other_typed) in [
            ("font-family", "serif", "monospace", "font-size", "29px"),
            ("font-size", "31px", "29px", "font-family", "monospace"),
        ] {
            let source = format!(
                "flowchart LR\nA[Glyph]\nclassDef {class} {owned_property}:{source_value}\n"
            );
            for html in [false, true] {
                let rendered = render(&source, Some(&theme), config(html));
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let label = node(&document)
                    .descendants()
                    .find(|node| {
                        if html {
                            node.has_tag_name("span") && has_class(*node, "nodeLabel")
                        } else {
                            node.has_tag_name("text")
                        }
                    })
                    .unwrap();
                assert!(label.ancestors().skip(1).any(|node| has_class(node, class)));
                if html {
                    assert_eq!(property(label, other_property), Some(other_typed));
                    assert_rule(
                        rendered.svg(),
                        class,
                        " span",
                        &format!("{owned_property}:{source_value}!important;"),
                    );
                    let glyph = label
                        .descendants()
                        .find(|node| node.is_text() && node.text() == Some("Glyph"))
                        .unwrap();
                    for descendant in glyph.ancestors().skip(1).take_while(|node| *node != label) {
                        assert_eq!(property(descendant, owned_property), None);
                        assert_eq!(property(descendant, other_property), None);
                    }
                    assert_ne!(property(label, owned_property), Some(typed_value));
                    let div = label.parent_element().unwrap();
                    assert!(div.has_tag_name("div"));
                    assert_ne!(property(div, owned_property), Some(typed_value));
                    assert_evidence(rendered, 1, 1, 0);
                } else {
                    // SVG font declarations select rect/polygon/etc., not the text/tspan.
                    assert_rule(
                        rendered.svg(),
                        class,
                        " rect",
                        &format!("{owned_property}:{source_value}!important;"),
                    );
                    assert!(!rendered.svg().contains(&format!(" .{class} tspan{{")));
                    // The native writer puts target fonts on the label group; text inherits
                    // them. In this fixture source font selectors reach shapes only, and the
                    // fixed stylesheet adds no font override below the label group. Inspect
                    // that known inheritance chain, including SVG presentation attributes.
                    let wrapper = node(&document);
                    let glyph = label
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("tspan")
                                && node
                                    .children()
                                    .any(|child| child.is_text() && child.text() == Some("Glyph"))
                        })
                        .unwrap();
                    for (font_property, expected) in
                        [(owned_property, typed_value), (other_property, other_typed)]
                    {
                        let projected = glyph
                            .ancestors()
                            .take_while(|ancestor| *ancestor != wrapper)
                            .find_map(|ancestor| {
                                property(ancestor, font_property)
                                    .or_else(|| ancestor.attribute(font_property))
                            })
                            .unwrap_or_else(|| panic!("missing native {font_property} owner"));
                        if font_property == "font-size" {
                            // An SVG font-size attribute may spell the same px size as `29`.
                            assert_eq!(
                                projected.strip_suffix("px").unwrap_or(projected),
                                expected.strip_suffix("px").unwrap_or(expected),
                            );
                        } else {
                            assert_eq!(projected, expected);
                        }
                    }
                    assert_evidence(rendered, 2, 0, 0);
                }
            }
        }
    }
}

#[test]
fn structural_cluster_paints_follow_actual_rect_selectors_in_both_label_modes() {
    // Repro: unassigned root/clusters/cluster classDefs reach the real cluster rect,
    // but its typed inline fill/stroke/width previously bypassed those source owners.
    let theme = compile([
        fill(ThemeTarget::Cluster),
        ThemeRule::new(
            ThemeTarget::Cluster,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#654321").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::Cluster,
            ThemeStylePatch::default().with_stroke_width(5.0).unwrap(),
        ),
    ]);
    for class in ["root", "clusters", "cluster"] {
        let source = format!(
            "{CLUSTER_SOURCE}classDef {class} fill:#aabbcc,stroke:#ccbbaa,stroke-width:7px\n"
        );
        for html in [false, true] {
            let rendered = render(&source, Some(&theme), config(html));
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let group = cluster(&document);
            let rect = group
                .children()
                .find(|node| node.has_tag_name("rect"))
                .unwrap();
            assert!(rect.ancestors().skip(1).any(|node| has_class(node, class)));
            let matches = !html || class == "cluster";
            if html {
                assert_eq!(has_class(rect.parent_element().unwrap(), class), matches);
            }
            assert_rule(
                rendered.svg(),
                class,
                if html { ">*" } else { " rect" },
                "fill:rgb(170, 187, 204)!important;stroke:rgb(204, 187, 170)!important;stroke-width:7px!important;",
            );
            for (paint, typed) in [
                ("fill", "#123456"),
                ("stroke", "#654321"),
                ("stroke-width", "5px"),
            ] {
                if matches {
                    assert_ne!(
                        property(rect, paint),
                        Some(typed),
                        "{class}, html={html}, {paint}"
                    );
                } else {
                    assert_eq!(
                        property(rect, paint),
                        Some(typed),
                        "{class}, html={html}, {paint}"
                    );
                }
            }
            assert_evidence(
                rendered,
                if matches { 0 } else { 3 },
                if matches { 3 } else { 0 },
                0,
            );
        }
    }
}

#[test]
fn structural_cluster_source_ownership_blocks_only_the_matching_mixed_field() {
    let theme = compile([ThemeRule::new(
        ThemeTarget::Cluster,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").unwrap())
            .with_stroke(CanvasPaint::solid("#654321").unwrap())
            .with_stroke_width(5.0)
            .unwrap(),
    )]);
    for class in ["root", "clusters", "cluster"] {
        for (owned_property, source_value, source_css) in [
            ("fill", "#aabbcc", "rgb(170, 187, 204)"),
            ("stroke", "#aabbcc", "rgb(170, 187, 204)"),
            ("stroke-width", "7px", "7px"),
        ] {
            let source =
                format!("{CLUSTER_SOURCE}classDef {class} {owned_property}:{source_value}\n");
            for html in [false, true] {
                let rendered = render(&source, Some(&theme), config(html));
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let group = cluster(&document);
                let rect = group
                    .children()
                    .find(|node| node.has_tag_name("rect"))
                    .unwrap();
                let matches = !html || class == "cluster";
                assert!(rect.ancestors().skip(1).any(|node| has_class(node, class)));
                if html {
                    assert_eq!(has_class(rect.parent_element().unwrap(), class), matches);
                }
                assert_rule(
                    rendered.svg(),
                    class,
                    if html { ">*" } else { " rect" },
                    &format!("{owned_property}:{source_css}!important;"),
                );
                for (paint, typed) in [
                    ("fill", "#123456"),
                    ("stroke", "#654321"),
                    ("stroke-width", "5px"),
                ] {
                    if matches && paint == owned_property {
                        assert_ne!(property(rect, paint), Some(typed));
                    } else {
                        assert_eq!(property(rect, paint), Some(typed));
                    }
                }
                // The two unowned fields still apply; one source owner cannot suppress the rule.
                assert_evidence(rendered, 1, 0, 0);
            }
        }
    }
}

#[test]
fn structural_cluster_label_color_owns_native_tspans_but_shape_fill_does_not() {
    let theme = compile([fill(ThemeTarget::ClusterLabel)]);
    for class in ["root", "clusters", "cluster"] {
        for paint in ["color", "fill"] {
            let source = format!("{CLUSTER_SOURCE}classDef {class} {paint}:#aabbcc\n");
            let rendered = render(&source, Some(&theme), config(false));
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let label = cluster(&document)
                .descendants()
                .find(|node| node.has_tag_name("g") && has_class(*node, "cluster-label"))
                .unwrap();
            let glyph = label
                .descendants()
                .find(|node| {
                    node.has_tag_name("tspan")
                        && node
                            .children()
                            .any(|child| child.is_text() && child.text() == Some("Heading"))
                })
                .unwrap();
            assert!(glyph.ancestors().skip(1).any(|node| has_class(node, class)));
            if paint == "color" {
                // Direct tspan fill wins over the typed color inherited from cluster text CSS.
                assert_rule(
                    rendered.svg(),
                    class,
                    " tspan",
                    "fill:rgb(170, 187, 204)!important;",
                );
                assert_ne!(property(glyph, "fill"), Some("#123456"));
                assert_evidence(rendered, 0, 1, 0);
            } else {
                assert!(!rendered.svg().contains(&format!(" .{class} tspan{{")));
                assert_rule(rendered.svg(), "cluster-label", " text", "fill:#123456;");
                assert_evidence(rendered, 1, 0, 0);
            }
        }
    }
}
