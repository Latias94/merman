//! Native export must retain bounded Flowchart edge effects and their application evidence.

#![cfg(all(feature = "svg", feature = "png"))]

use merman::SourcePresentationConfig;
use merman::svg::{
    CanvasPaint, DiagramEffectSet, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec,
    EffectBinding, EffectColorSpace, EffectGraph, EffectInput, EffectPrimitive, OrdinalSelector,
    Specified, ThemeColorValue, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman::{MermaidEngine, OperationControl, RenderOutput, RenderRequest, Renderer};

fn hard_shadow_theme(targets: &[ThemeTarget], rules: ThemeRuleSet) -> DiagramTheme {
    let mut effects = DiagramEffectSet::default()
        .with_graph(
            EffectGraph::new(
                "hard-shadow",
                [EffectPrimitive::DropShadow {
                    input: EffectInput::SourceGraphic,
                    offset_x: -16.0,
                    offset_y: 8.0,
                    blur_radius: 0.0,
                    spread: 0.0,
                    color: ThemeColorValue::parse("#ff2080").unwrap(),
                }],
            )
            .unwrap()
            .with_color_space(EffectColorSpace::Srgb),
        )
        .unwrap();
    for &target in targets {
        effects = effects
            .with_binding(EffectBinding::new(target, "hard-shadow").unwrap())
            .unwrap();
    }
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_effects(effects)
                .with_styles(rules),
        )
        .unwrap()
}

fn typed_filter_reference<'a>(node: roxmltree::Node<'a, '_>) -> Option<&'a str> {
    node.attribute("filter")
        .or_else(|| {
            node.attribute("style")?.split(';').find_map(|declaration| {
                let (property, value) = declaration.split_once(':')?;
                (property.trim() == "filter").then_some(value.trim())
            })
        })
        .filter(|value| value.contains("-theme-effect-"))
}

fn label_instance<'a, 'input>(
    xml: &'a roxmltree::Document<'input>,
    text: &str,
) -> roxmltree::Node<'a, 'input> {
    let label = xml
        .descendants()
        .find(|node| node.is_text() && node.text() == Some(text))
        .unwrap();
    if !label.ancestors().any(|node| node.has_tag_name("defs")) {
        return label;
    }
    // Native HTML fallback stores glyphs once and paints them through a direct use.
    let id = label
        .ancestors()
        .find_map(|node| node.attribute("id"))
        .unwrap();
    let reference = format!("#{id}");
    let instances: Vec<_> = xml
        .descendants()
        .filter(|node| node.has_tag_name("use") && node.attribute("href") == Some(&reference))
        .collect();
    assert_eq!(instances.len(), 1, "{text}: one painted label instance");
    instances[0]
}

fn label_paint_content<'a, 'input>(
    terminal: roxmltree::Node<'a, 'input>,
) -> roxmltree::Node<'a, 'input> {
    let Some(instance) = terminal.descendants().find(|node| node.has_tag_name("use")) else {
        return terminal;
    };
    let id = instance
        .attribute("href")
        .unwrap()
        .strip_prefix('#')
        .unwrap();
    terminal
        .document()
        .descendants()
        .find(|node| node.attribute("id") == Some(id))
        .unwrap()
}

fn pink_pixel_halves(bytes: &[u8]) -> [usize; 2] {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    let mut counts = [0; 2];
    for (index, pixel) in pixels[..info.buffer_size()]
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
    {
        if pixel[3] > 0 && pixel[0] > 200 && pixel[1] < 80 && (80..180).contains(&pixel[2]) {
            let x = index % info.width as usize;
            counts[usize::from(x >= info.width as usize / 2)] += 1;
        }
    }
    counts
}

fn assert_native_effect_admission(admission: &merman::TargetAdmissionReceipt) {
    for reason in [
        merman::TargetAdmissionReason::ThemeEvidenceIncomplete,
        merman::TargetAdmissionReason::NativeFilterReceiptMismatch,
        merman::TargetAdmissionReason::PdfNativeFilterNotLocalized,
    ] {
        assert!(!admission.reasons().contains(&reason), "{admission:?}");
    }
}

#[test]
fn neo_node_hard_shadow_and_clear_change_native_pixels_and_application_counts() {
    let rules = ThemeRuleSet::default();
    let mut clear = ThemeStylePatch::default();
    clear.effects.effect = Specified::Clear;
    let themes = [
        (
            "omitted",
            hard_shadow_theme(&[], rules.clone()),
            0,
            [false, false],
        ),
        (
            "shadow",
            hard_shadow_theme(&[ThemeTarget::Node], rules.clone()),
            2,
            [true, true],
        ),
        (
            "clear-first",
            hard_shadow_theme(
                &[ThemeTarget::Node],
                rules.clone().with_rule(
                    ThemeRule::new(ThemeTarget::Node, clear.clone())
                        .with_ordinal(OrdinalSelector::Exact(1)),
                ),
            ),
            1,
            [false, true],
        ),
        (
            "clear-all",
            hard_shadow_theme(
                &[ThemeTarget::Node],
                rules.with_rule(ThemeRule::new(ThemeTarget::Node, clear)),
            ),
            0,
            [false, false],
        ),
    ];
    // Equal-sized bodies leave the midpoint inside the edge gap, even when the
    // negative-x shadow changes the viewport. Pink then identifies each body's shadow.
    for (html_labels, look_filter) in [(false, "none"), (true, "url(#drop-shadow)")] {
        let source = format!(
            "---\nconfig:\n  look: neo\n  htmlLabels: {html_labels}\n  themeVariables:\n    dropShadow: '{look_filter}'\n---\nflowchart LR\nA(((Double))) --> B(((Double)))"
        );
        for (case, theme, expected, visible_halves) in &themes {
            let RenderOutput::Document(Some(document)) = Renderer::new()
                .render(
                    RenderRequest::document(&source, OperationControl::new(), Default::default())
                        .with_theme(theme.clone()),
                )
                .unwrap_or_else(|error| panic!("{case}, html={html_labels}: {error:?}"))
            else {
                panic!("document required")
            };
            let xml = roxmltree::Document::parse(document.svg()).unwrap();
            assert_eq!(
                xml.descendants()
                    .filter(|node| typed_filter_reference(*node).is_some())
                    .count(),
                *expected,
                "{case}, html={html_labels}"
            );
            let nodes: Vec<_> = xml
                .descendants()
                .filter(|node| {
                    node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "node")
                    })
                })
                .collect();
            assert_eq!(nodes.len(), 2);
            for node in nodes {
                let body = node
                    .children()
                    .find(|child| child.attribute("class") == Some("basic label-container"))
                    .unwrap();
                assert!(body.has_tag_name("g"));
                let circles: Vec<_> = body
                    .children()
                    .filter(|child| child.has_tag_name("circle"))
                    .collect();
                assert_eq!(circles.len(), 2);
                assert!(!body.descendants().any(|child| {
                    child.has_tag_name("text") || child.has_tag_name("foreignObject")
                }));
                if *case != "omitted" {
                    assert!(
                        circles.iter().all(|circle| {
                            circle
                                .attribute("style")
                                .unwrap()
                                .contains("filter:none !important")
                        }),
                        "child look filters must not compose with body effects: {case}"
                    );
                    let cleared = *case == "clear-all"
                        || (*case == "clear-first" && node.attribute("data-id") == Some("A"));
                    if cleared {
                        assert!(
                            body.attribute("style")
                                .unwrap()
                                .contains("filter:none !important")
                        );
                        assert!(typed_filter_reference(body).is_none());
                    } else {
                        assert!(typed_filter_reference(body).is_some());
                        assert!(body.attribute("style").unwrap().contains("!important"));
                    }
                }
            }
            let png = document
                .export_png(&Default::default(), OperationControl::new())
                .unwrap();
            assert_native_effect_admission(png.admission());
            let receipt = png.export_report().native_filter_receipt();
            assert_eq!(
                receipt.map_or(0, |receipt| receipt.filter_count() as usize),
                *expected
            );
            assert_eq!(
                receipt.map_or(0, |receipt| receipt.reference_count() as usize),
                *expected
            );
            let counts = pink_pixel_halves(png.bytes());
            assert_eq!(
                counts.map(|count| count > 0),
                *visible_halves,
                "Clear must remove the selected body's actual pink shadow: {case}, html={html_labels}, pixels={counts:?}"
            );
            #[cfg(feature = "pdf")]
            {
                let pdf = document
                    .export_pdf(&Default::default(), OperationControl::new())
                    .unwrap();
                assert_native_effect_admission(pdf.admission());
                assert_eq!(pdf.export_report().native_filter_receipt(), receipt);
            }
        }
    }
}

#[test]
fn neo_edge_hard_shadow_retains_final_paths_markers_and_masks() {
    let theme = hard_shadow_theme(&[ThemeTarget::Edge], ThemeRuleSet::default());
    for layout in ["dagre", "elk"] {
        if layout == "elk" && !cfg!(feature = "layout-elk") {
            continue;
        }
        let source = format!(
            "---\nconfig:\n  look: neo\n  layout: {layout}\n  htmlLabels: false\n---\nflowchart LR\nsubgraph Outer\nA[Alpha] o--o B[Beta]\nB dotted@-.-> C[Gamma]\nC animated@--> D[Delta]\nanimated@{{ animate: true }}\nend"
        );
        let RenderOutput::Document(Some(baseline)) = Renderer::new()
            .render(RenderRequest::document(
                &source,
                OperationControl::new(),
                Default::default(),
            ))
            .unwrap()
        else {
            panic!("document required")
        };
        let RenderOutput::Document(Some(document)) = Renderer::new()
            .render(
                RenderRequest::document(&source, OperationControl::new(), Default::default())
                    .with_theme(theme.clone()),
            )
            .unwrap()
        else {
            panic!("document required")
        };
        let before = roxmltree::Document::parse(baseline.svg()).unwrap();
        let after = roxmltree::Document::parse(document.svg()).unwrap();
        let edges = |xml: &roxmltree::Document<'_>| {
            xml.descendants()
                .filter(|node| node.attribute("data-edge") == Some("true"))
                .map(|node| {
                    [
                        "d",
                        "data-points",
                        "class",
                        "style",
                        "marker-start",
                        "marker-end",
                    ]
                    .map(|attribute| node.attribute(attribute).map(str::to_owned))
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(edges(&before), edges(&after), "{layout}");
        let paths: Vec<_> = after
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .collect();
        assert_eq!(paths.len(), 3);
        assert!(
            paths
                .iter()
                .any(|path| path.attribute("data-id") == Some("animated"))
        );
        for path in &paths {
            let reference = typed_filter_reference(*path).unwrap();
            let id = reference
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let filter = after
                .descendants()
                .find(|node| node.attribute("id") == Some(id))
                .unwrap();
            assert_eq!(filter.attribute("filterUnits"), Some("userSpaceOnUse"));
            assert!(filter.attribute("width").unwrap().parse::<f64>().unwrap() > 0.0);
            assert!(filter.attribute("height").unwrap().parse::<f64>().unwrap() > 0.0);
        }
        let png = document
            .export_png(&Default::default(), OperationControl::new())
            .unwrap();
        assert_native_effect_admission(png.admission());
        assert_eq!(
            png.export_report()
                .native_filter_receipt()
                .unwrap()
                .filter_count(),
            3
        );
        assert!(pink_pixel_halves(png.bytes()).into_iter().sum::<usize>() > 0);
    }
}

#[test]
fn neo_clear_keeps_edge_and_label_terminals_independent() {
    let mut clear = ThemeStylePatch::default();
    clear.effects.effect = Specified::Clear;
    let all_targets = [
        ThemeTarget::Edge,
        ThemeTarget::NodeLabel,
        ThemeTarget::EdgeLabel,
    ];
    for (cleared_target, expected) in [
        (None, 4),
        (Some(ThemeTarget::Edge), 3),
        (Some(ThemeTarget::NodeLabel), 3),
        (Some(ThemeTarget::EdgeLabel), 3),
    ] {
        let mut rules = ThemeRuleSet::default();
        if let Some(target) = cleared_target {
            let rule = ThemeRule::new(target, clear.clone());
            rules = rules.with_rule(if target == ThemeTarget::NodeLabel {
                rule.with_ordinal(OrdinalSelector::Exact(1))
            } else {
                rule
            });
        }
        let theme = hard_shadow_theme(&all_targets, rules);
        for html_labels in [false, true] {
            let source = format!(
                "---\nconfig:\n  look: neo\n  htmlLabels: {html_labels}\n---\nflowchart LR\nsubgraph Outer\nA[Alpha] -->|Advance| B[Beta]\nend"
            );
            let RenderOutput::Document(Some(document)) = Renderer::new()
                .render(
                    RenderRequest::document(&source, OperationControl::new(), Default::default())
                        .with_theme(theme.clone()),
                )
                .unwrap()
            else {
                panic!("document required")
            };
            let xml = roxmltree::Document::parse(document.svg()).unwrap();
            assert_eq!(
                xml.descendants()
                    .filter(|node| typed_filter_reference(*node).is_some())
                    .count(),
                expected
            );
            let edge = xml
                .descendants()
                .find(|node| node.attribute("data-edge") == Some("true"))
                .unwrap();
            assert_eq!(
                typed_filter_reference(edge).is_some(),
                cleared_target != Some(ThemeTarget::Edge)
            );
            for (text, has_shadow) in [
                ("Alpha", cleared_target != Some(ThemeTarget::NodeLabel)),
                ("Beta", true),
                ("Advance", cleared_target != Some(ThemeTarget::EdgeLabel)),
            ] {
                let label = label_instance(&xml, text);
                assert_eq!(
                    label
                        .ancestors()
                        .any(|node| typed_filter_reference(node).is_some()),
                    has_shadow,
                    "{text}, cleared={cleared_target:?}, html={html_labels}: {}",
                    &document.svg()[label.parent().unwrap().range()]
                );
            }
            let png = document
                .export_png(&Default::default(), OperationControl::new())
                .unwrap();
            assert_native_effect_admission(png.admission());
            let receipt = png.export_report().native_filter_receipt().unwrap();
            assert_eq!(receipt.filter_count() as usize, expected);
            assert_eq!(receipt.reference_count() as usize, expected);
            assert!(pink_pixel_halves(png.bytes()).into_iter().sum::<usize>() > 0);
            #[cfg(feature = "pdf")]
            {
                let pdf = document
                    .export_pdf(&Default::default(), OperationControl::new())
                    .unwrap();
                assert_native_effect_admission(pdf.admission());
                assert_eq!(pdf.export_report().native_filter_receipt(), Some(receipt));
            }
        }
    }
}

#[test]
fn neo_effects_keep_source_filter_and_ordinary_content_guards() {
    for (target, body) in [
        (
            ThemeTarget::Edge,
            "flowchart LR\nA --> B\nlinkStyle 0 filter:none",
        ),
        (
            ThemeTarget::Edge,
            "flowchart LR\nA --> B\nlinkStyle 0 filter:blur(2px)",
        ),
        (
            ThemeTarget::Edge,
            "flowchart LR\nA --> B\nlinkStyle 0 stroke-width:2em",
        ),
        (
            ThemeTarget::Edge,
            "flowchart LR\nA --> B\nclassDef edgePaths transform:scale(2)",
        ),
        (ThemeTarget::NodeLabel, "flowchart LR\nA[\"<b>Alpha</b>\"]"),
        (
            ThemeTarget::NodeLabel,
            "flowchart LR\nA[Alpha]:::filtered\nclassDef filtered filter:blur(20px)",
        ),
        (
            ThemeTarget::EdgeLabel,
            "flowchart LR\nA -->|<span style='background:red'>Advance</span>| B",
        ),
        (
            ThemeTarget::EdgeLabel,
            "flowchart LR\nA -->|Advance| B\nclassDef label filter:blur(20px)",
        ),
    ] {
        let theme = hard_shadow_theme(&[target], ThemeRuleSet::default());
        let source = format!("---\nconfig:\n  look: neo\n  htmlLabels: true\n---\n{body}");
        let RenderOutput::Document(Some(document)) = Renderer::new()
            .render(
                RenderRequest::document(&source, OperationControl::new(), Default::default())
                    .with_theme(theme.clone()),
            )
            .unwrap()
        else {
            panic!("document required")
        };
        let xml = roxmltree::Document::parse(document.svg()).unwrap();
        assert!(
            !xml.descendants()
                .any(|node| typed_filter_reference(node).is_some()),
            "{body}"
        );
        if target == ThemeTarget::Edge && body.contains("filter:") {
            let edge = xml
                .descendants()
                .find(|node| node.attribute("data-edge") == Some("true"))
                .unwrap();
            let source_filter = if body.contains("filter:none") {
                "filter:none"
            } else {
                "filter:blur(2px)"
            };
            assert!(
                edge.attribute("style").unwrap().contains(source_filter),
                "{body}"
            );
        }
        let png = document
            .export_png(&Default::default(), OperationControl::new())
            .unwrap();
        assert!(
            png.admission()
                .reasons()
                .contains(&merman::TargetAdmissionReason::ThemeEvidenceIncomplete),
            "an unconsumed effect must remain residual: {body}"
        );
        let strict = merman::SvgRequest {
            environment: merman::SvgEnvironment::deterministic()
                .with_theme_portability_requirement(
                    merman::svg::ThemePortabilityRequirement::RequirePortable,
                ),
            ..Default::default()
        };
        assert!(
            Renderer::new()
                .render(
                    RenderRequest::document(&source, OperationControl::new(), strict)
                        .with_theme(theme)
                )
                .is_err(),
            "{body}"
        );
    }
}

#[test]
fn neo_admission_does_not_expand_swimlane_or_hand_drawn_effects() {
    let theme = hard_shadow_theme(
        &[
            ThemeTarget::Edge,
            ThemeTarget::NodeLabel,
            ThemeTarget::EdgeLabel,
        ],
        ThemeRuleSet::default(),
    );
    for source in [
        "---\nconfig:\n  look: neo\n  layout: swimlane\n---\nflowchart LR\nA[Alpha] -->|Advance| B[Beta]",
        "---\nconfig:\n  look: neo\n---\nswimlane-beta LR\nA[Alpha] -->|Advance| B[Beta]",
        "---\nconfig:\n  look: handDrawn\n---\nflowchart LR\nA[Alpha] -->|Advance| B[Beta]",
    ] {
        let RenderOutput::Document(Some(document)) = Renderer::new()
            .render(
                RenderRequest::document(source, OperationControl::new(), Default::default())
                    .with_theme(theme.clone()),
            )
            .unwrap()
        else {
            panic!("document required")
        };
        let xml = roxmltree::Document::parse(document.svg()).unwrap();
        assert!(
            !xml.descendants()
                .any(|node| typed_filter_reference(node).is_some()),
            "{source}"
        );
        let png = document
            .export_png(&Default::default(), OperationControl::new())
            .unwrap();
        assert!(
            png.admission()
                .reasons()
                .contains(&merman::TargetAdmissionReason::ThemeEvidenceIncomplete),
            "{source}"
        );
    }
}

#[test]
fn edge_glow_survives_native_export_for_line_paths_and_nested_roots() {
    for (look, body) in ["classic", "neo"].into_iter().flat_map(|look| {
        [
            "flowchart LR\nA[Alpha] --> B[Beta]",
            "flowchart TB\nA[Alpha] --> B[Beta]",
            "flowchart LR\nsubgraph Outer\nsubgraph Inner\nA[Alpha] --> B[Beta]\nend\nend",
            "flowchart LR\nA[Alpha] o--o B[Beta] x--x C[Gamma]",
        ]
        .map(move |body| (look, body))
    }) {
        let source = format!("---\nconfig:\n  look: {look}\n  htmlLabels: false\n---\n{body}");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_effects(
                        DiagramEffectSet::default()
                            .with_graph(
                                EffectGraph::new(
                                    "edge-glow",
                                    [EffectPrimitive::DropShadow {
                                        input: EffectInput::SourceGraphic,
                                        offset_x: 0.0,
                                        offset_y: 0.0,
                                        blur_radius: 6.0,
                                        spread: 0.0,
                                        color: ThemeColorValue::parse("#00f2ff").unwrap(),
                                    }],
                                )
                                .unwrap()
                                .with_color_space(EffectColorSpace::Srgb),
                            )
                            .unwrap()
                            .with_binding(
                                EffectBinding::new(ThemeTarget::Edge, "edge-glow").unwrap(),
                            )
                            .unwrap(),
                    )
                    .with_styles(
                        ThemeRuleSet::default()
                            .with_rule(ThemeRule::new(
                                ThemeTarget::Node,
                                ThemeStylePatch::default()
                                    .with_fill(CanvasPaint::solid("#ffffff").unwrap())
                                    .with_stroke(CanvasPaint::solid("#000000").unwrap()),
                            ))
                            .with_rule(ThemeRule::new(
                                ThemeTarget::Edge,
                                ThemeStylePatch::default()
                                    .with_stroke(CanvasPaint::solid("#000000").unwrap()),
                            )),
                    ),
            )
            .unwrap();
        let RenderOutput::Document(Some(document)) = Renderer::new()
            .render(
                RenderRequest::document(&source, OperationControl::new(), Default::default())
                    .with_theme(theme),
            )
            .unwrap()
        else {
            panic!("document required")
        };
        let xml = roxmltree::Document::parse(document.svg()).unwrap();
        let edges: Vec<_> = xml
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .collect();
        for edge in &edges {
            assert!(typed_filter_reference(*edge).is_some(), "{source}");
        }
        let output = document
            .export_png(
                &merman::svg::export::RasterOptions::default(),
                OperationControl::new(),
            )
            .unwrap();
        assert_native_effect_admission(output.admission());
        let receipt = output.export_report().native_filter_receipt().unwrap();
        assert_eq!(receipt.filter_count() as usize, edges.len());
        assert_eq!(receipt.reference_count(), receipt.filter_count());
        let mut reader = png::Decoder::new(std::io::Cursor::new(output.bytes()))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgba);
        // All source paint is achromatic. Cyan identifies the shadow without requiring
        // an arbitrary contrast from a thin blurred path composited over white.
        assert!(
            pixels[..info.buffer_size()]
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] > 0 && pixel[1] > pixel[0] && pixel[2] > pixel[0]),
            "cyan glow must reach native pixels: {source}"
        );
        #[cfg(feature = "pdf")]
        {
            let pdf = document
                .export_pdf(
                    &merman::svg::export::PdfOptions::default(),
                    OperationControl::new(),
                )
                .unwrap();
            assert_native_effect_admission(pdf.admission());
            assert_eq!(
                pdf.export_report().native_filter_receipt(),
                output.export_report().native_filter_receipt()
            );
            assert!(pdf.bytes().starts_with(b"%PDF"));
        }
    }
}

#[test]
fn text_only_glow_survives_native_png_and_pdf() {
    for (look, html_labels) in [
        ("classic", false),
        ("classic", true),
        ("neo", false),
        ("neo", true),
    ] {
        let spec = DiagramThemeSpec::new();
        let effects = DiagramEffectSet::default()
            .with_graph(
                EffectGraph::new(
                    "text-glow",
                    [EffectPrimitive::DropShadow {
                        input: EffectInput::SourceGraphic,
                        offset_x: 0.0,
                        offset_y: 0.0,
                        blur_radius: 6.0,
                        spread: 0.0,
                        color: ThemeColorValue::parse("#ff2080").unwrap(),
                    }],
                )
                .unwrap()
                .with_color_space(EffectColorSpace::Srgb),
            )
            .unwrap()
            .with_binding(EffectBinding::new(ThemeTarget::NodeLabel, "text-glow").unwrap())
            .unwrap();
        let effects = effects
            .with_binding(EffectBinding::new(ThemeTarget::EdgeLabel, "text-glow").unwrap())
            .unwrap();
        let theme = DiagramThemeCompiler::new()
            .compile(
                spec.with_effects(effects).with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ffffff").unwrap())
                                .with_stroke(CanvasPaint::solid("#000000").unwrap()),
                        ))
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Edge,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#000000").unwrap()),
                        )),
                ),
            )
            .unwrap();
        for body in [
            "flowchart LR\nA[Alpha] -->|Advance| B[Beta]",
            "flowchart TB\nsubgraph Outer\nA[Alpha<br/>Gyp] -->|Advance<br/>Next| B[Beta]\nend",
            "flowchart TB\nsubgraph Outer\nA[Alpha<br/><br/>Gyp] -->|Advance<br/>Next| B[Beta]\nend",
        ] {
            let source =
                format!("---\nconfig:\n  look: {look}\n  htmlLabels: {html_labels}\n---\n{body}");
            let RenderOutput::Document(Some(document)) = Renderer::new()
                .render(
                    RenderRequest::document(&source, OperationControl::new(), Default::default())
                        .with_theme(theme.clone()),
                )
                .unwrap_or_else(|error| {
                    panic!("look={look}, html_labels={html_labels}, body={body}: {error:?}")
                })
            else {
                panic!("document required")
            };
            let xml = roxmltree::Document::parse(document.svg()).unwrap();
            let filtered: Vec<_> = xml
                .descendants()
                .filter(|node| typed_filter_reference(*node).is_some())
                .collect();
            assert_eq!(filtered.len(), 3, "{source}");
            for terminal in filtered {
                let content = label_paint_content(terminal);
                assert!(
                    !content.descendants().any(|node| {
                        match node.tag_name().name() {
                            // The fallback viewport keeps an invisible geometric extent.
                            "rect" => {
                                !(node.attribute("fill") == Some("none")
                                    && node.attribute("style").is_some_and(|style| {
                                        style.contains("stroke:none")
                                            && style.contains("opacity:0;")
                                    }))
                            }
                            "path" | "polygon" | "circle" => true,
                            _ => false,
                        }
                    }),
                    "glyph filters must exclude backgrounds and bodies: {source}: {}",
                    &document.svg()[content.range()]
                );
                assert!(
                    content.descendants().any(|node| node.is_text()),
                    "{source}: {}",
                    &document.svg()[terminal.range()]
                );
                if terminal.has_tag_name("p") {
                    assert!(
                        terminal
                            .attribute("style")
                            .unwrap()
                            .contains("background:transparent")
                    );
                    assert!(
                        terminal
                            .ancestors()
                            .any(|node| node.attribute("class") == Some("labelBkg"))
                    );
                }
            }
            let png = document
                .export_png(&Default::default(), OperationControl::new())
                .unwrap();
            assert_native_effect_admission(png.admission());
            assert_eq!(
                png.export_report()
                    .native_filter_receipt()
                    .unwrap()
                    .filter_count(),
                3
            );
            let mut reader = png::Decoder::new(std::io::Cursor::new(png.bytes()))
                .read_info()
                .unwrap();
            let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
            let info = reader.next_frame(&mut pixels).unwrap();
            assert_eq!(info.color_type, png::ColorType::Rgba);
            // Only the text shadow has chromatic paint; definitions alone cannot satisfy this.
            assert!(
                pixels[..info.buffer_size()]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[3] > 0 && p[0] > p[2] && p[2] > p[1]),
                "look={look}, html_labels={html_labels}: text glow must reach PNG pixels"
            );
            #[cfg(feature = "pdf")]
            {
                let pdf = document
                    .export_pdf(&Default::default(), OperationControl::new())
                    .unwrap();
                assert_native_effect_admission(pdf.admission());
                assert_eq!(
                    pdf.export_report().native_filter_receipt(),
                    png.export_report().native_filter_receipt()
                );
                assert!(pdf.bytes().starts_with(b"%PDF"));
            }
        }
    }
}

#[test]
fn underestimated_host_label_geometry_is_rejected_by_native_export() {
    use merman::svg::{
        MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity, TextMeasurer, TextMetrics, TextStyle,
    };
    #[derive(Debug)]
    struct Underestimated;
    impl TextMeasurer for Underestimated {
        fn measure(&self, _: &str, style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 0.01,
                height: style.font_size,
                line_count: 1,
            }
        }
    }
    let environment = merman::SvgEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.flowchart-small-host").unwrap(),
                "1",
            )
            .unwrap(),
            std::sync::Arc::new(Underestimated),
        )),
    );
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_effects(
                DiagramEffectSet::default()
                    .with_graph(
                        EffectGraph::new(
                            "text",
                            [EffectPrimitive::DropShadow {
                                input: EffectInput::SourceGraphic,
                                offset_x: 0.0,
                                offset_y: 0.0,
                                blur_radius: 4.0,
                                spread: 0.0,
                                color: ThemeColorValue::parse("#ff2080").unwrap(),
                            }],
                        )
                        .unwrap(),
                    )
                    .unwrap()
                    .with_binding(EffectBinding::new(ThemeTarget::NodeLabel, "text").unwrap())
                    .unwrap(),
            ),
        )
        .unwrap();
    for (look, html_labels) in [
        ("classic", false),
        ("classic", true),
        ("neo", false),
        ("neo", true),
    ] {
        let source = format!(
            "---\nconfig:\n  look: {look}\n  htmlLabels: {html_labels}\n---\nflowchart LR\nA[{}]",
            "W".repeat(128)
        );
        let RenderOutput::Document(Some(document)) = Renderer::new()
            .render(
                RenderRequest::document(
                    &source,
                    OperationControl::new(),
                    merman::SvgRequest {
                        environment: environment.clone(),
                        ..Default::default()
                    },
                )
                .with_theme(theme.clone()),
            )
            .unwrap()
        else {
            panic!("document required")
        };
        // Host allocation may draw the requested effect without becoming a cache identity.
        // The actual native glyph observer must still reject underestimated paint bounds.
        assert!(document.svg().contains("theme-effect-label"));
        let png = document
            .export_png(&Default::default(), OperationControl::new())
            .unwrap();
        assert!(
            png.admission()
                .reasons()
                .contains(&merman::TargetAdmissionReason::NativeFilterReceiptMismatch),
            "underestimated host metrics must fail final glyph containment: {:?}",
            png.admission()
        );
        #[cfg(feature = "pdf")]
        {
            let pdf = document
                .export_pdf(&Default::default(), OperationControl::new())
                .unwrap();
            assert!(
                pdf.admission()
                    .reasons()
                    .contains(&merman::TargetAdmissionReason::NativeFilterReceiptMismatch)
            );
        }
    }
}

#[test]
fn typed_edge_background_preserves_alpha_in_native_pixels() {
    let renderer = Renderer::new().with_presentation_engine(MermaidEngine::new().with_site_config(
        SourcePresentationConfig::from_value(serde_json::json!({
            "theme": "redux-color",
            "themeVariables": {
                "clusterBkg": "transparent",
                "bkgColorArray": ["transparent"]
            }
        })),
    ));
    for swimlane in [false, true] {
        for html in [false, true] {
            for padding in [0.0, 6.0] {
                // Redux container palettes own Swimlane fills independently of clusterBkg.
                // Initialize Redux and its transparent palette together so source array merging
                // cannot retain an opaque container underneath the edge background under test.
                // Keep the underlay transparent so this test isolates the label's alpha.
                let source = format!(
                    "---\nconfig:\n  htmlLabels: {html}\n{}---\nflowchart LR\nA[Alpha] -->|Advance| B[Beta]",
                    if swimlane { "  layout: swimlane\n" } else { "" },
                );
                let theme = DiagramThemeCompiler::new()
                    .compile(
                        DiagramThemeSpec::new()
                            .with_canvas(merman::svg::CanvasSpec::transparent())
                            .with_styles(
                                ThemeRuleSet::default()
                                    .with_rule(ThemeRule::new(
                                        ThemeTarget::EdgeLabelBackground,
                                        ThemeStylePatch::default().with_fill(
                                            CanvasPaint::solid("rgba(255,0,0,0.4)").unwrap(),
                                        ),
                                    ))
                                    .with_rule(ThemeRule::new(
                                        ThemeTarget::EdgeLabel,
                                        ThemeStylePatch::default()
                                            .with_padding(merman::svg::InsetsPx::all(padding)),
                                    )),
                            ),
                    )
                    .unwrap();
                let RenderOutput::Document(Some(document)) = renderer
                    .render(
                        RenderRequest::document(
                            &source,
                            OperationControl::new(),
                            Default::default(),
                        )
                        .with_theme(theme),
                    )
                    .unwrap()
                else {
                    panic!("document required")
                };
                let output = document
                    .export_png(&Default::default(), OperationControl::new())
                    .unwrap();
                assert_eq!(
                    output
                        .admission()
                        .reasons()
                        .contains(&merman::TargetAdmissionReason::ThemeEvidenceIncomplete),
                    !swimlane && html && padding > 0.0,
                    "only the unmeasured padded HTML background retains a residual: swimlane={swimlane}, html={html}, padding={padding}; diagnostics={:?}",
                    document.evidence().theme_diagnostics(),
                );
                let mut reader = png::Decoder::new(std::io::Cursor::new(output.bytes()))
                    .read_info()
                    .unwrap();
                let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
                let info = reader.next_frame(&mut pixels).unwrap();
                assert_eq!(info.color_type, png::ColorType::Rgba);
                // Only the requested background is red. Its interior must retain the
                // requested alpha, independent of HTML projection or a padding rectangle.
                let max_alpha = pixels[..info.buffer_size()]
                    .as_chunks::<4>().0.iter()
                    .filter(|pixel| pixel[0] >= 250 && pixel[1] <= 2 && pixel[2] <= 2)
                    .map(|pixel| pixel[3])
                    .max()
                    .unwrap_or_else(|| panic!("red background pixels: swimlane={swimlane}, html={html}, padding={padding}"));
                assert!(
                    (101..=103).contains(&max_alpha),
                    "swimlane={swimlane}, html={html}, padding={padding}: alpha={max_alpha}",
                );
            }
        }
    }
}

#[test]
fn stateful_host_label_offsets_are_not_replayed_as_verified_geometry() {
    use merman::svg::{
        MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity, TextMeasurer, TextMetrics, TextStyle, ThemePreset,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct ChangingOffset(Arc<AtomicUsize>);
    impl TextMeasurer for ChangingOffset {
        fn measure(&self, _: &str, style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 80.0,
                height: style.font_size,
                line_count: 1,
            }
        }
        fn measure_svg_create_text_bbox_y_offset_px(&self, _: &str, _: &TextStyle) -> f64 {
            self.0.fetch_add(1, Ordering::SeqCst) as f64 * 0.125
        }
    }
    let theme = DiagramThemeCompiler::new()
        .compile_preset(ThemePreset::Cyberpunk)
        .unwrap();
    for look in ["classic", "neo"] {
        let calls = Arc::new(AtomicUsize::new(0));
        let environment = merman::SvgEnvironment::deterministic().with_text_measurement_policy(
            TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
                TextMeasurementProfileIdentity::new(
                    MeasurementProfileId::new("test.flowchart-changing-offset").unwrap(),
                    "1",
                )
                .unwrap(),
                Arc::new(ChangingOffset(calls.clone())),
            )),
        );
        let source = format!(
            "---\nconfig:\n  look: {look}\n  htmlLabels: false\n---\nflowchart LR\nA[Alpha] -->|Advance| B[Beta]"
        );
        let RenderOutput::Document(Some(document)) = Renderer::new()
            .render(
                RenderRequest::document(
                    &source,
                    OperationControl::new(),
                    merman::SvgRequest {
                        environment,
                        ..Default::default()
                    },
                )
                .with_theme(theme.clone()),
            )
            .unwrap()
        else {
            panic!("document required")
        };
        assert!(
            calls.load(Ordering::SeqCst) >= 2,
            "the host must observe both geometry requests"
        );
        let xml = roxmltree::Document::parse(document.svg()).unwrap();
        assert!(
            xml.descendants()
                .any(|node| node.is_text() && node.text() == Some("Advance"))
        );
        assert!(
            !xml.descendants()
                .filter(|node| node.attribute("class").is_some_and(|classes| {
                    classes.split_whitespace().any(|class| class == "edgeLabel")
                }))
                .any(|label| label
                    .descendants()
                    .any(|node| node.attribute("filter").is_some())),
            "a changed terminal translation cannot consume the prepared edge-label filter"
        );
        let png = document
            .export_png(&Default::default(), OperationControl::new())
            .unwrap();
        assert!(
            png.admission()
                .reasons()
                .contains(&merman::TargetAdmissionReason::ThemeEvidenceIncomplete)
        );
    }
}
