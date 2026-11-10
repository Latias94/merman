#![cfg(any(feature = "diagram-timeline", feature = "diagram-git-graph"))]

mod common;

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_render::DiagramFamilyId;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, MermaidThemeCompatibility,
    OrdinalPalette, OrdinalSelector, ThemeColorValue, ThemePortabilityRequirement, ThemePreset,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::{Value, json};

const ID: &str = "native-category-label";

fn render(source: &str, theme: Option<&DiagramTheme>, config: Value) -> family::RenderedFamilySvg {
    try_render_with_requirement(
        source,
        theme,
        config,
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap()
}

fn try_render_with_requirement(
    source: &str,
    theme: Option<&DiagramTheme>,
    config: Value,
    requirement: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let mut engine =
        MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(config));
    if let Some(theme) = theme {
        engine = common::install_parse_compatibility(theme, engine);
    }
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
            diagram_id: Some(ID.into()),
            ..Default::default()
        },
        &SvgDebugOptions::default(),
    )
}

fn theme(
    palette: Option<(ThemeTarget, &[&str])>,
    rules: impl IntoIterator<Item = ThemeRule>,
    baseline: &str,
) -> DiagramTheme {
    let mut styles = rules
        .into_iter()
        .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule);
    if let Some((target, colors)) = palette {
        styles = styles.with_ordinal_palette(
            target,
            OrdinalPalette::new(
                colors
                    .iter()
                    .map(|color| ThemeColorValue::parse(color).unwrap()),
            )
            .unwrap(),
        );
    }
    let spec = DiagramThemeSpec::new().with_styles(styles);
    let spec = if baseline == "default" {
        spec
    } else {
        spec.with_mermaid_compatibility(
            MermaidThemeCompatibility::default()
                .with_theme(baseline)
                .unwrap(),
        )
    };
    DiagramThemeCompiler::new().compile(spec).unwrap()
}

fn fill_rule(target: ThemeTarget, css: &str) -> ThemeRule {
    ThemeRule::new(
        target,
        ThemeStylePatch::default().with_fill(if css == "transparent" {
            CanvasPaint::Transparent
        } else {
            CanvasPaint::solid(css).unwrap()
        }),
    )
}

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_whitespace().any(|value| value == class))
}

fn declaration<'a>(style: &'a str, property: &str) -> Option<&'a str> {
    style
        .split(';')
        .filter_map(|entry| entry.split_once(':'))
        .filter(|(name, _)| name.trim() == property)
        .map(|(_, value)| value.trim())
        .next_back()
}

// These are exact family-emitted selectors, checked together with the live SVG element.
fn css_property(
    document: &roxmltree::Document<'_>,
    selector: &str,
    property: &str,
) -> Option<String> {
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .flat_map(|css| css.split('}'))
        .filter_map(|rule| rule.split_once('{'))
        .filter(|(selectors, _)| selectors.split(',').any(|value| value.trim() == selector))
        .filter_map(|(_, body)| declaration(body, property))
        .next_back()
        .map(str::to_owned)
}

fn inline_fill<'a>(node: roxmltree::Node<'a, '_>) -> Option<&'a str> {
    node.attribute("style")
        .and_then(|style| declaration(style, "fill"))
}

fn assert_readable(ink: &str, fill: &str, brightness: f64, context: &str) {
    let luminance = |css: &str| {
        let color = merman_markup::theme_color::ThemeColor::parse(css)
            .unwrap()
            .rgba_channels();
        assert_eq!(color.alpha, 1.0, "opaque category pair: {context}");
        let linear = |channel: f64| {
            let value = (channel * brightness).min(255.0) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.red) + 0.7152 * linear(color.green) + 0.0722 * linear(color.blue)
    };
    let foreground = luminance(ink);
    let background = luminance(fill);
    let ratio = (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05);
    assert!(
        ratio >= 4.5,
        "{context}: {ratio:.2}:1 ({ink} on {fill}, brightness={brightness})"
    );
}

fn assert_evidence(
    rendered: family::RenderedFamilySvg,
    applied: usize,
    absent: usize,
    residual: usize,
) {
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(
        evidence.required_count(),
        applied + absent + residual,
        "{evidence:?}"
    );
    assert_eq!(evidence.applied_count(), applied, "{evidence:?}");
    assert_eq!(evidence.not_applicable_count(), absent, "{evidence:?}");
    assert_eq!(evidence.theme_residual_count(), residual, "{evidence:?}");
}

fn label_cases() -> Vec<(DiagramFamilyId, ThemeTarget, &'static str)> {
    vec![
        #[cfg(feature = "diagram-timeline")]
        (
            DiagramFamilyId::TIMELINE,
            ThemeTarget::NodeLabel,
            "timeline\n  Period : Event\n",
        ),
        #[cfg(feature = "diagram-git-graph")]
        (
            DiagramFamilyId::GIT_GRAPH,
            ThemeTarget::ClusterLabel,
            "gitGraph\n  commit id: \"A\" tag: \"v1\"\n",
        ),
    ]
}

#[test]
fn specialized_labels_admit_only_static_default_solid_or_transparent_fill_in_rule_order() {
    for (family, target, source) in label_cases() {
        for (variant, ink) in [
            (None, "#123456"),
            (Some(ThemeVariant::Default), "transparent"),
        ] {
            let mut rule = fill_rule(target, ink).for_family(family);
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            let themed = theme(None, [rule], "default");
            let rendered = render(source, Some(&themed), json!({}));
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert!(
                document
                    .descendants()
                    .any(|node| node.has_tag_name("text") && inline_fill(node) == Some(ink))
            );
            assert_evidence(rendered, 1, 0, 0);
        }
        let ordinal = fill_rule(target, "#654321")
            .for_family(family)
            .with_ordinal(OrdinalSelector::exact(1).unwrap());
        let static_rule = fill_rule(target, "#123456").for_family(family);
        for (rules, applied, absent, residual) in [
            (vec![ordinal.clone()], 0, 0, 1),
            (vec![ordinal.clone(), static_rule.clone()], 1, 1, 0),
            (vec![static_rule, ordinal], 1, 0, 1),
        ] {
            assert_evidence(
                render(source, Some(&theme(None, rules, "default")), json!({})),
                applied,
                absent,
                residual,
            );
        }
        let mixed = ThemeRule::new(
            target,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#123456").unwrap())
                .with_stroke(CanvasPaint::solid("#654321").unwrap()),
        )
        .for_family(family);
        assert_evidence(
            render(source, Some(&theme(None, [mixed], "default")), json!({})),
            0,
            0,
            1,
        );
        let primary = fill_rule(target, "#123456")
            .for_family(family)
            .with_variant(ThemeVariant::Primary);
        assert_evidence(
            render(source, Some(&theme(None, [primary], "default")), json!({})),
            0,
            1,
            0,
        );
    }
}

#[test]
fn empty_theme_preserves_default_mermaid_svg_bytes() {
    let empty = theme(None, [], "default");
    for (_, _, source) in label_cases() {
        assert_eq!(
            render(source, None, json!({})).svg(),
            render(source, Some(&empty), json!({})).svg()
        );
    }
}

#[cfg(feature = "diagram-timeline")]
mod timeline {
    use super::*;

    fn source(direction: &str, sectioned: bool) -> String {
        let mut source = format!("timeline {direction}\n");
        for index in 0..13 {
            if sectioned {
                source.push_str(&format!("  section S{index}\n"));
            }
            source.push_str(&format!("  P{index} : E{index}\n"));
        }
        source
    }

    fn nodes<'a>(document: &'a roxmltree::Document<'_>) -> Vec<roxmltree::Node<'a, 'a>> {
        document
            .descendants()
            .filter(|node| node.has_tag_name("g") && has_class(*node, "timeline-node"))
            .collect()
    }

    fn label<'a>(node: roxmltree::Node<'a, 'a>) -> roxmltree::Node<'a, 'a> {
        node.descendants()
            .find(|child| child.has_tag_name("text"))
            .unwrap()
    }

    fn body<'a>(node: roxmltree::Node<'a, 'a>) -> roxmltree::Node<'a, 'a> {
        node.descendants()
            .find(|child| child.has_tag_name("path") && has_class(*child, "node-bkg"))
            .unwrap()
    }

    #[test]
    fn native_pairs_follow_sections_and_unsectioned_tasks_through_the_twelve_slot_ring() {
        for preset in [
            ThemePreset::EditorLight,
            ThemePreset::EditorDark,
            ThemePreset::Cyberpunk,
        ] {
            let themed = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
            for direction in ["LR", "TD"] {
                for sectioned in [false, true] {
                    let rendered = render(&source(direction, sectioned), Some(&themed), json!({}));
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    assert_eq!(
                        css_property(&document, &format!("#{ID} .eventWrapper"), "filter")
                            .as_deref(),
                        Some("brightness(120%)")
                    );
                    let nodes = nodes(&document);
                    assert_eq!(nodes.len(), if sectioned { 39 } else { 26 });
                    for node in &nodes {
                        let event = node
                            .ancestors()
                            .any(|ancestor| has_class(ancestor, "eventWrapper"));
                        assert_readable(
                            inline_fill(label(*node)).expect("typed category label"),
                            inline_fill(body(*node)).expect("typed category fill"),
                            if event { 1.2 } else { 1.0 },
                            preset.id(),
                        );
                    }
                    let first = nodes[0];
                    let repeated = nodes[if sectioned { 36 } else { 24 }];
                    assert_eq!(first.attribute("class"), repeated.attribute("class"));
                    assert_eq!(inline_fill(body(first)), inline_fill(body(repeated)));
                    assert_eq!(inline_fill(label(first)), inline_fill(label(repeated)));
                }
            }
        }
    }

    #[test]
    fn explicit_category_pair_is_independent_of_canvas_and_source_owned_label_slots() {
        let themed = theme(
            Some((
                ThemeTarget::TimelineEvent,
                &["#fef08a", "#a7f3d0", "#bae6fd"],
            )),
            [
                fill_rule(ThemeTarget::Text, "#112233"),
                fill_rule(ThemeTarget::Title, "#334455"),
                fill_rule(ThemeTarget::NodeLabel, "#000000"),
            ],
            "default",
        );
        let rendered = render(
            &format!(
                "timeline\n  title Canvas\n{}",
                source("LR", true).split_once('\n').unwrap().1
            ),
            Some(&themed),
            json!({"themeVariables": {"cScaleLabel0": "#13579b"}}),
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            css_property(&document, &format!("#{ID}"), "fill").as_deref(),
            Some("#112233")
        );
        assert!(document.descendants().any(|node| node.has_tag_name("text")
            && node.text() == Some("Canvas")
            && node.attribute("fill") == Some("#334455")));
        for node in nodes(&document) {
            if has_class(node, "section--1") {
                assert_eq!(inline_fill(label(node)), None);
                assert_eq!(
                    css_property(&document, &format!("#{ID} .section--1 text"), "fill").as_deref(),
                    Some("#13579b")
                );
            } else {
                assert_eq!(inline_fill(label(node)), Some("#000000"));
            }
        }
        assert_evidence(rendered, 4, 0, 0);

        let label_theme = theme(
            None,
            [fill_rule(ThemeTarget::NodeLabel, "#123456")],
            "default",
        );
        assert_evidence(
            render(
                "timeline\n  P : E\n",
                Some(&label_theme),
                json!({"themeVariables": {"THEME_COLOR_LIMIT": 1, "cScaleLabel0": "#13579b"}}),
            ),
            0,
            1,
            0,
        );
    }

    #[test]
    fn unrequested_and_empty_label_domains_keep_mermaid_ownership() {
        let palette = theme(
            Some((ThemeTarget::TimelineEvent, &["#fef08a"])),
            [],
            "default",
        );
        let rendered = render("timeline\n  P : E\n", Some(&palette), json!({}));
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(
            nodes(&document)
                .iter()
                .all(|node| inline_fill(label(*node)).is_none())
        );
        assert_evidence(rendered, 1, 0, 0);
        let themed = theme(
            None,
            [fill_rule(ThemeTarget::NodeLabel, "#123456")],
            "default",
        );
        for source in ["timeline\n", "timeline\n  title Canvas only\n"] {
            assert_evidence(render(source, Some(&themed), json!({})), 0, 1, 0);
        }
        assert_evidence(
            render(
                "timeline\n  P : E\n",
                Some(&themed),
                json!({"themeVariables": {"fontSize": "0px"}}),
            ),
            0,
            1,
            0,
        );
    }

    #[test]
    fn category_label_support_matches_the_bounded_timeline_surface_path() {
        for (baseline, look, gradient, applied) in [
            ("default", "classic", false, true),
            ("default", "neo", false, true),
            ("default", "neo", true, false),
            ("redux", "classic", false, false),
            ("redux-color", "neo", false, false),
        ] {
            let themed = theme(
                None,
                [fill_rule(ThemeTarget::NodeLabel, "#123456")],
                baseline,
            );
            let rendered = render(
                "timeline\n  P : E\n",
                Some(&themed),
                json!({"look": look, "themeVariables": {"useGradient": gradient}}),
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert!(
                nodes(&document)
                    .iter()
                    .all(|node| inline_fill(label(*node)) == applied.then_some("#123456")),
                "{baseline}/{look}/{gradient}"
            );
            assert_evidence(rendered, usize::from(applied), 0, usize::from(!applied));
        }
    }

    #[test]
    fn redux_label_fill_ownership_follows_node_border_in_both_portability_modes() {
        for baseline in ["redux", "redux-dark"] {
            let themed = theme(
                None,
                [fill_rule(ThemeTarget::NodeLabel, "#123456")],
                "default",
            );
            for (variables, owned) in [
                (json!({"cScaleLabel0": "#abcdef"}), false),
                (json!({"nodeBorder": "#13579b"}), true),
                (
                    json!({"cScaleLabel0": "#abcdef", "nodeBorder": "#13579b"}),
                    true,
                ),
            ] {
                for requirement in [
                    ThemePortabilityRequirement::BestEffort,
                    ThemePortabilityRequirement::RequirePortable,
                ] {
                    let result = try_render_with_requirement(
                        "timeline\n  P : E\n",
                        Some(&themed),
                        json!({"theme": baseline, "themeVariables": variables.clone()}),
                        requirement,
                    );
                    if requirement == ThemePortabilityRequirement::RequirePortable && !owned {
                        let error = result
                            .err()
                            .expect("unused slot ink must not hide the Redux label residual");
                        assert_eq!(
                            error.unverified_family_theme(),
                            Some((DiagramFamilyId::TIMELINE, 1)),
                            "{baseline}: {error:?}"
                        );
                        continue;
                    }
                    let rendered = result.unwrap();
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    let terminals = nodes(&document);
                    assert_eq!(terminals.len(), 2);
                    assert!(
                        terminals
                            .iter()
                            .all(|node| inline_fill(label(*node)).is_none())
                    );
                    let ink = css_property(&document, &format!("#{ID} .section--1 text"), "fill")
                        .expect("Redux section text uses its nodeBorder CSS");
                    if owned {
                        assert_eq!(ink, "#13579b", "{baseline}");
                    } else {
                        assert_ne!(ink, "#abcdef", "{baseline}");
                    }
                    assert_evidence(rendered, 0, usize::from(owned), usize::from(!owned));
                }
            }
        }
    }
}

#[cfg(feature = "diagram-git-graph")]
mod gitgraph {
    use super::*;

    fn source() -> String {
        let mut source = "gitGraph\n  commit id: \"C0\" tag: \"v1\"\n".to_owned();
        for index in 1..9 {
            source.push_str(&format!("  branch B{index}\n  commit id: \"C{index}\"\n"));
        }
        source
    }

    fn labels<'a>(document: &'a roxmltree::Document<'_>) -> Vec<roxmltree::Node<'a, 'a>> {
        document
            .descendants()
            .filter(|node| node.has_tag_name("g") && has_class(*node, "branchLabel"))
            .collect()
    }

    fn text<'a>(node: roxmltree::Node<'a, 'a>) -> roxmltree::Node<'a, 'a> {
        node.descendants()
            .find(|child| child.has_tag_name("text"))
            .unwrap()
    }

    fn slot(node: roxmltree::Node<'_, '_>) -> usize {
        text(node)
            .parent()
            .unwrap()
            .attribute("class")
            .unwrap()
            .split_whitespace()
            .find_map(|class| class.strip_prefix("branch-label"))
            .unwrap()
            .parse()
            .unwrap()
    }

    #[test]
    fn native_branch_pairs_repeat_the_eight_slot_ring_without_recoloring_tag_or_commit_roles() {
        for preset in [
            ThemePreset::EditorLight,
            ThemePreset::EditorDark,
            ThemePreset::Cyberpunk,
        ] {
            let themed = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
            let rendered = render(&source(), Some(&themed), json!({}));
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let labels = labels(&document);
            assert_eq!(labels.len(), 9);
            for label in &labels {
                let index = slot(*label);
                assert!(
                    document.descendants().any(|node| node.has_tag_name("rect")
                        && has_class(node, &format!("label{index}")))
                );
                let fill = css_property(&document, &format!("#{ID} .label{index}"), "fill")
                    .expect("real branch backing CSS");
                assert_readable(
                    inline_fill(text(*label)).expect("category text"),
                    &fill,
                    1.0,
                    preset.id(),
                );
            }
            assert_eq!(slot(labels[0]), slot(labels[8]));
            assert_eq!(inline_fill(text(labels[0])), inline_fill(text(labels[8])));
        }
    }

    #[test]
    fn source_slots_and_global_text_order_keep_branch_tag_commit_and_title_owners_separate() {
        let themed = theme(
            Some((ThemeTarget::Node, &["#fef08a", "#a7f3d0", "#bae6fd"])),
            [
                fill_rule(ThemeTarget::Text, "#112233"),
                fill_rule(ThemeTarget::NodeLabel, "#223344"),
                fill_rule(ThemeTarget::EdgeLabel, "#334455"),
                fill_rule(ThemeTarget::ClusterLabel, "#000000"),
                fill_rule(ThemeTarget::Title, "#445566"),
            ],
            "default",
        );
        let rendered = render(
            &format!("---\ntitle: Canvas\n---\n{}", source()),
            Some(&themed),
            json!({"themeVariables": {"gitBranchLabel0": "#13579b"}}),
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for label in labels(&document) {
            assert_eq!(
                inline_fill(text(label)),
                if slot(label) == 0 {
                    None
                } else {
                    Some("#000000")
                }
            );
        }
        assert_eq!(
            css_property(&document, &format!("#{ID} .branch-label0"), "fill").as_deref(),
            Some("#13579b")
        );
        for (class, fill) in [("tag-label", "#223344"), ("commit-label", "#334455")] {
            assert!(
                document
                    .descendants()
                    .any(|node| node.has_tag_name("text") && has_class(node, class))
            );
            assert_eq!(
                css_property(&document, &format!("#{ID} .{class}"), "fill").as_deref(),
                Some(fill)
            );
        }
        assert_eq!(
            css_property(&document, &format!("#{ID}"), "fill").as_deref(),
            Some("#112233")
        );
        assert!(document.descendants().any(|node| has_class(node, "gitTitleText") && inline_fill(node) == Some("#445566")));
        assert_evidence(rendered, 6, 0, 0);

        let label_theme = theme(
            None,
            [fill_rule(ThemeTarget::ClusterLabel, "#123456")],
            "default",
        );
        assert_evidence(
            render(
                "gitGraph\n",
                Some(&label_theme),
                json!({"themeVariables": {"gitBranchLabel0": "#13579b"}}),
            ),
            0,
            1,
            0,
        );
        // A later root Text rule still competes for tags/commits, never branch category text.
        let later_text = theme(
            None,
            [
                fill_rule(ThemeTarget::ClusterLabel, "#123456"),
                fill_rule(ThemeTarget::Text, "#654321"),
            ],
            "default",
        );
        let rendered = render("gitGraph\n", Some(&later_text), json!({}));
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_eq!(inline_fill(text(labels(&document)[0])), Some("#123456"));
    }

    #[test]
    fn unrequested_hidden_and_empty_branch_labels_do_not_claim_application() {
        let palette = theme(Some((ThemeTarget::Node, &["#fef08a"])), [], "default");
        let rendered = render(&source(), Some(&palette), json!({}));
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(
            labels(&document)
                .iter()
                .all(|node| inline_fill(text(*node)).is_none())
        );
        assert_evidence(rendered, 1, 0, 0);
        let themed = theme(
            None,
            [fill_rule(ThemeTarget::ClusterLabel, "#123456")],
            "default",
        );
        for config in [
            json!({"gitGraph": {"showBranches": false}}),
            json!({"gitGraph": {"mainBranchName": "\\n"}}),
            json!({"themeVariables": {"fontSize": "0px"}}),
        ] {
            assert_evidence(render("gitGraph\n", Some(&themed), config), 0, 1, 0);
        }
        assert_evidence(render("gitGraph\n", Some(&themed), json!({})), 1, 0, 0);
    }

    #[test]
    fn generated_color_looks_keep_a_visible_branch_request_residual() {
        for (baseline, look, applied) in [
            ("default", "classic", true),
            ("default", "neo", true),
            ("neo", "classic", false),
            ("redux", "classic", false),
            ("redux-color", "neo", false),
        ] {
            let themed = theme(
                None,
                [fill_rule(ThemeTarget::ClusterLabel, "#123456")],
                baseline,
            );
            let rendered = render("gitGraph\n", Some(&themed), json!({"look": look}));
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert!(
                labels(&document)
                    .iter()
                    .all(|node| inline_fill(text(*node)) == applied.then_some("#123456")),
                "{baseline}/{look}"
            );
            assert_evidence(rendered, usize::from(applied), 0, usize::from(!applied));
        }
    }

    #[test]
    fn redux_branch_fill_ownership_follows_node_border_in_both_portability_modes() {
        let multi_branch = source();
        for baseline in ["redux", "redux-dark"] {
            let themed = theme(
                None,
                [fill_rule(ThemeTarget::ClusterLabel, "#123456")],
                "default",
            );
            for (graph, variables, owned) in [
                ("gitGraph\n", json!({"gitBranchLabel0": "#abcdef"}), false),
                (
                    multi_branch.as_str(),
                    json!({"nodeBorder": "#13579b"}),
                    true,
                ),
                (
                    multi_branch.as_str(),
                    json!({"gitBranchLabel0": "#abcdef", "nodeBorder": "#13579b"}),
                    true,
                ),
            ] {
                for requirement in [
                    ThemePortabilityRequirement::BestEffort,
                    ThemePortabilityRequirement::RequirePortable,
                ] {
                    let result = try_render_with_requirement(
                        graph,
                        Some(&themed),
                        json!({"theme": baseline, "themeVariables": variables.clone()}),
                        requirement,
                    );
                    if requirement == ThemePortabilityRequirement::RequirePortable && !owned {
                        let error = result
                            .err()
                            .expect("unused slot ink must not hide the Redux branch residual");
                        assert_eq!(
                            error.unverified_family_theme(),
                            Some((DiagramFamilyId::GIT_GRAPH, 1)),
                            "{baseline}: {error:?}"
                        );
                        continue;
                    }
                    let rendered = result.unwrap();
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    let terminals = labels(&document);
                    assert_eq!(terminals.len(), if owned { 9 } else { 1 });
                    if owned {
                        assert!(terminals.iter().any(|node| slot(*node) == 8));
                    }
                    assert!(
                        terminals
                            .iter()
                            .all(|node| inline_fill(text(*node)).is_none())
                    );
                    for terminal in terminals {
                        let ink = css_property(
                            &document,
                            &format!("#{ID} .branch-label{}", slot(terminal)),
                            "fill",
                        )
                        .expect("Redux branch text uses its nodeBorder CSS");
                        if owned {
                            assert_eq!(ink, "#13579b", "{baseline}");
                        } else {
                            assert_ne!(ink, "#abcdef", "{baseline}");
                        }
                    }
                    assert_evidence(rendered, 0, usize::from(owned), usize::from(!owned));
                }
            }
        }
    }
}
