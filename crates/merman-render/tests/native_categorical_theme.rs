#![cfg(any(feature = "diagram-mindmap", feature = "diagram-journey"))]

mod common;

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_render::diagram_theme::{
    DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, ThemePortabilityRequirement, ThemePreset,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::{Value, json};

const ID: &str = "native-categorical";
const PRESETS: [ThemePreset; 11] = [
    ThemePreset::EditorLight,
    ThemePreset::EditorDark,
    ThemePreset::OneDark,
    ThemePreset::GruvboxLight,
    ThemePreset::GruvboxDark,
    ThemePreset::AyuLight,
    ThemePreset::AyuDark,
    ThemePreset::Brutalist,
    ThemePreset::Spotless,
    ThemePreset::Cyberpunk,
    ThemePreset::Ghibli,
];

fn render(source: &str, theme: Option<&DiagramTheme>, config: Value) -> family::RenderedFamilySvg {
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
    let environment = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::BestEffort);
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
    )
    .unwrap()
    .render_svg(
        &SvgRenderOptions {
            diagram_id: Some(ID.into()),
            ..Default::default()
        },
        &SvgDebugOptions::default(),
    )
    .unwrap()
}

fn recipe(rules: Value) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile_recipe(
            serde_json::from_value(json!({
                "schema_version": 1, "kind": "design", "design": {"rules": rules}
            }))
            .unwrap(),
        )
        .unwrap()
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

// Read only the exact selectors emitted by these consumers. Each caller also checks the
// corresponding live DOM shape/label; this is not a general CSS cascade approximation.
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
        .last()
        .map(str::to_owned)
}

fn assert_readable(ink: &str, fill: &str, context: &str) {
    let luminance = |css: &str| {
        let color = merman_markup::theme_color::ThemeColor::parse(css)
            .unwrap()
            .rgba_channels();
        assert_eq!(color.alpha, 1.0, "opaque terminal: {context}");
        let linear = |channel: f64| {
            let value = channel / 255.0;
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
    assert!(ratio >= 4.5, "{context}: {ratio:.2}:1 ({ink} on {fill})");
}

fn assert_fully_accounted(rendered: family::RenderedFamilySvg, applied: usize, absent: usize) {
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), applied + absent, "{evidence:?}");
    assert_eq!(evidence.applied_count(), applied, "{evidence:?}");
    assert_eq!(evidence.not_applicable_count(), absent, "{evidence:?}");
    assert_eq!(evidence.theme_residual_count(), 0, "{evidence:?}");
}

#[cfg(feature = "diagram-mindmap")]
mod mindmap {
    use super::*;
    use merman_render::diagram_theme::{
        CanvasPaint, OrdinalSelector, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
        ThemeVariant,
    };

    fn source() -> String {
        let mut source = "mindmap\n  root[Root]\n".to_owned();
        for branch in 0..12 {
            source.push_str(&format!("    b{branch}[Branch {branch}]\n"));
            if branch == 0 {
                source.push_str("      nested[Nested]\n        deep[Deep]\n");
            }
        }
        source
    }

    fn node_with_label<'a>(
        document: &'a roxmltree::Document<'_>,
        label: &str,
    ) -> roxmltree::Node<'a, 'a> {
        document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && has_class(*node, "node")
                    && node
                        .descendants()
                        .filter(|child| child.is_text())
                        .filter_map(|child| child.text())
                        .collect::<Vec<_>>()
                        .join(" ")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        == label
            })
            .unwrap_or_else(|| panic!("live node {label}"))
    }

    fn ink(
        document: &roxmltree::Document<'_>,
        node: roxmltree::Node<'_, '_>,
        html: bool,
    ) -> Option<String> {
        let tag = if html { "span" } else { "text" };
        assert!(
            node.descendants().any(|child| child.has_tag_name(tag)),
            "live {tag} label"
        );
        css_property(
            document,
            &format!("#{ID} g#{} > .label {tag}", node.attribute("id").unwrap()),
            if html { "color" } else { "fill" },
        )
    }

    fn fill(document: &roxmltree::Document<'_>, node: roxmltree::Node<'_, '_>) -> String {
        let shape = node
            .children()
            .find(|child| {
                child.is_element()
                    && matches!(
                        child.tag_name().name(),
                        "rect" | "path" | "circle" | "polygon"
                    )
            })
            .expect("live node body");
        let selector = if let Some(section) = node.attribute("data-mindmap-section") {
            format!(
                "#{ID} [data-mindmap-section=\"{section}\"] {}",
                shape.tag_name().name()
            )
        } else {
            assert!(has_class(node, "section-root"));
            format!(
                "#{ID} g#{} > {}",
                node.attribute("id").unwrap(),
                shape.tag_name().name()
            )
        };
        css_property(document, &selector, "fill").expect("terminal body rule")
    }

    #[test]
    fn native_presets_pair_actual_root_and_nested_branch_terminals_across_the_eleven_slot_ring() {
        for preset in PRESETS {
            let theme = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
            for html in [false, true] {
                let rendered = render(
                    &source(),
                    Some(&theme),
                    json!({"layout": "tidy-tree", "htmlLabels": html}),
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let nodes = document
                    .descendants()
                    .filter(|node| node.has_tag_name("g") && has_class(*node, "node"))
                    .collect::<Vec<_>>();
                assert_eq!(nodes.len(), 15);
                for node in nodes {
                    assert_readable(
                        &ink(&document, node, html).expect("terminal label rule"),
                        &fill(&document, node),
                        &format!(
                            "{} html={html} {}",
                            preset.id(),
                            node.attribute("id").unwrap()
                        ),
                    );
                }
                let first = node_with_label(&document, "Branch 0");
                for label in ["Nested", "Deep", "Branch 11"] {
                    let node = node_with_label(&document, label);
                    assert_eq!(
                        node.attribute("data-mindmap-section"),
                        first.attribute("data-mindmap-section"),
                        "{label}"
                    );
                    assert_eq!(fill(&document, node), fill(&document, first), "{label}");
                    assert_eq!(
                        ink(&document, node, html),
                        ink(&document, first, html),
                        "{label}"
                    );
                }
                assert!(
                    node_with_label(&document, "Root")
                        .attribute("data-mindmap-section")
                        .is_none()
                );
            }
        }
    }

    #[test]
    fn primary_root_pair_reaches_classic_and_neo_without_suppressing_branches() {
        let theme = recipe(json!([
            {"family": "mindmap", "target": "node", "variant": "primary", "style": {"fill": "#123456"}},
            {"family": "mindmap", "target": "node-label", "style": {"fill": "#654321"}},
            {"family": "mindmap", "target": "node-label", "variant": "primary", "style": {"fill": "#abcdef"}}
        ]));
        for look in ["classic", "neo"] {
            for html in [false, true] {
                let rendered = render(
                    "mindmap\n  root[Root]\n    branch[Branch]\n",
                    Some(&theme),
                    json!({"layout": "tidy-tree", "look": look, "htmlLabels": html}),
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let root = node_with_label(&document, "Root");
                assert_eq!(fill(&document, root), "#123456");
                assert_eq!(ink(&document, root, html).as_deref(), Some("#abcdef"));
                assert_eq!(
                    ink(&document, node_with_label(&document, "Branch"), html).as_deref(),
                    Some("#654321")
                );
                assert_fully_accounted(rendered, 3, 0);
            }
        }
    }

    #[test]
    fn source_root_and_branch_paint_keep_their_property_owners() {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(ThemePreset::EditorDark)
            .unwrap();
        for html in [false, true] {
            let rendered = render(
                "mindmap\n  root[Root]\n    branch[Branch]\n",
                Some(&theme),
                json!({
                    "layout": "tidy-tree", "htmlLabels": html,
                    "themeVariables": {"git0": "#123456", "gitBranchLabel0": "#abcdef",
                        "cScale1": "#654321", "cScaleLabel1": "#fedcba"}
                }),
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            for label in ["Root", "Branch"] {
                let node = node_with_label(&document, label);
                assert!(node.attribute("data-mindmap-section").is_none());
                assert!(
                    ink(&document, node, html).is_none(),
                    "{label} remains source owned"
                );
            }
            assert_eq!(
                css_property(&document, &format!("#{ID} .section-root rect"), "fill").as_deref(),
                Some("#123456")
            );
            assert_eq!(
                css_property(&document, &format!("#{ID} .section-0 rect"), "fill").as_deref(),
                Some("#654321")
            );
            assert_eq!(
                css_property(&document, &format!("#{ID} .section-0 text"), "fill").as_deref(),
                Some("#fedcba")
            );
            assert!(
                css_property(
                    &document,
                    &format!(
                        "#{ID} g#{} > rect",
                        node_with_label(&document, "Root").attribute("id").unwrap()
                    ),
                    "fill"
                )
                .is_none()
            );
        }
    }

    #[test]
    fn later_authored_text_still_wins_over_categorical_label_roles() {
        let theme = recipe(json!([
            {"family": "mindmap", "target": "node-label", "style": {"fill": "#123456"}},
            {"family": "mindmap", "target": "text", "style": {"fill": "#654321"}}
        ]));
        let rendered = render(
            "mindmap\n  root[Root]\n    branch[Branch]\n",
            Some(&theme),
            json!({"layout": "tidy-tree"}),
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for label in ["Root", "Branch"] {
            assert_eq!(
                ink(&document, node_with_label(&document, label), true).as_deref(),
                Some("#654321")
            );
        }
        assert_fully_accounted(rendered, 1, 1);
    }

    #[test]
    fn primary_body_fill_does_not_certify_unsupported_siblings_or_ordinal_selectors() {
        let fill = || ThemeStylePatch::default().with_fill(CanvasPaint::solid("#abcdef").unwrap());
        for rule in [
            ThemeRule::new(
                ThemeTarget::Node,
                fill().with_stroke(CanvasPaint::solid("#123456").unwrap()),
            )
            .with_variant(ThemeVariant::Primary),
            ThemeRule::new(ThemeTarget::Node, fill())
                .with_variant(ThemeVariant::Primary)
                .with_ordinal(OrdinalSelector::exact(1).unwrap()),
        ] {
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)),
                )
                .unwrap();
            let rendered = render(
                "mindmap\n  root[Root]\n",
                Some(&theme),
                json!({"layout": "tidy-tree"}),
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.applied_count(), 0);
            assert_eq!(evidence.not_applicable_count(), 0);
            assert_eq!(evidence.theme_residual_count(), 1, "{evidence:?}");
        }
    }

    #[test]
    fn primary_overlay_does_not_certify_the_replaced_neo_main_background() {
        let theme = recipe(json!([
            {"family": "mindmap", "target": "node", "style": {"fill": "#112233"}},
            {"family": "mindmap", "target": "node", "variant": "primary", "style": {"fill": "#abcdef"}}
        ]));
        for html in [false, true] {
            let rendered = render(
                "mindmap\n  root[Root]\n",
                Some(&theme),
                json!({"layout": "tidy-tree", "look": "neo", "htmlLabels": html, "themeVariables": {"useGradient": true}}),
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert_eq!(
                fill(&document, node_with_label(&document, "Root")),
                "#abcdef"
            );
            assert_fully_accounted(rendered, 1, 1);
        }
    }

    #[test]
    fn empty_theme_preserves_mermaid_mindmap_output_in_both_label_modes() {
        let empty = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new())
            .unwrap();
        for html in [false, true] {
            let config = json!({"layout": "tidy-tree", "htmlLabels": html});
            assert_eq!(
                render(&source(), None, config.clone()).svg(),
                render(&source(), Some(&empty), config).svg()
            );
        }
    }
}

#[cfg(feature = "diagram-journey")]
mod journey {
    use super::*;

    fn source() -> String {
        let mut source = "journey\n  title Native categorical\n".to_owned();
        for section in 0..9 {
            source.push_str(&format!("  section Phase {section}\n    Task {section} A: 5: Actor\n    Task {section} B: 3: Actor\n"));
        }
        source
    }

    fn labels<'a, 'input>(
        rect: roxmltree::Node<'a, 'input>,
        placement: &str,
    ) -> Vec<roxmltree::Node<'a, 'input>> {
        rect.parent()
            .unwrap()
            .descendants()
            .filter(|node| {
                if placement == "fo" {
                    (node.has_tag_name("div") && has_class(*node, "label"))
                        || node.has_tag_name("text")
                } else {
                    node.has_tag_name("text")
                }
            })
            .collect()
    }

    fn surface_ink<'a, 'input>(node: roxmltree::Node<'a, 'input>) -> Option<&'a str> {
        declaration(
            node.attribute("style").unwrap_or_default(),
            if node.has_tag_name("div") {
                "color"
            } else {
                "fill"
            },
        )
    }

    #[test]
    fn native_presets_pair_real_section_and_task_labels_in_every_text_placement() {
        for preset in PRESETS {
            let theme = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
            for placement in ["fo", "old", "tspan"] {
                let rendered = render(
                    &source(),
                    Some(&theme),
                    json!({"journey": {"textPlacement": placement}}),
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let rects = document
                    .descendants()
                    .filter(|node| {
                        node.has_tag_name("rect")
                            && (has_class(*node, "task") || has_class(*node, "journey-section"))
                    })
                    .collect::<Vec<_>>();
                assert_eq!(rects.len(), 27);
                for rect in &rects {
                    let fill = declaration(rect.attribute("style").unwrap(), "fill")
                        .expect("actual shell fill");
                    let terminals = labels(*rect, placement);
                    assert_eq!(terminals.len(), if placement == "fo" { 2 } else { 1 });
                    for terminal in terminals {
                        assert_readable(
                            surface_ink(terminal).expect("actual categorical label ink"),
                            fill,
                            &format!(
                                "{} {placement} {}",
                                preset.id(),
                                rect.attribute("class").unwrap()
                            ),
                        );
                    }
                }
                let sections = rects
                    .iter()
                    .copied()
                    .filter(|node| has_class(*node, "journey-section"))
                    .collect::<Vec<_>>();
                assert_eq!(
                    sections[0].attribute("style"),
                    sections[7].attribute("style"),
                    "seven-slot section ring"
                );
                if preset == ThemePreset::EditorDark {
                    assert_eq!(
                        css_property(&document, &format!("#{ID} .legend"), "fill").as_deref(),
                        Some("#e5e7eb")
                    );
                    let title = document
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("text") && node.text() == Some("Native categorical")
                        })
                        .unwrap();
                    assert_eq!(title.attribute("fill"), Some("#e5e7eb"));
                }
            }
        }
    }

    #[test]
    fn authored_task_label_reaches_all_terminal_paths_and_preserves_outside_ink() {
        let theme = recipe(json!([
            {"family": "journey", "target": "text", "style": {"fill": "#abcdef"}},
            {"family": "journey", "target": "task-label", "style": {"fill": "#123456"}}
        ]));
        for placement in ["fo", "old", "tspan"] {
            let rendered = render(
                "journey\nsection Phase\nTask: 5: Actor\nsection Next\nNext task: 4: Actor\n",
                Some(&theme),
                json!({"journey": {"textPlacement": placement}}),
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            for rect in document.descendants().filter(|node| {
                node.has_tag_name("rect")
                    && (has_class(*node, "task") || has_class(*node, "journey-section"))
            }) {
                for terminal in labels(rect, placement) {
                    assert_eq!(surface_ink(terminal), Some("#123456"));
                }
            }
            assert_eq!(
                css_property(&document, &format!("#{ID} .legend"), "fill").as_deref(),
                Some("#abcdef")
            );
            assert_fully_accounted(rendered, 2, 0);
        }
    }

    #[test]
    fn explicit_fill_text_and_tspan_section_colours_keep_source_ownership() {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(ThemePreset::EditorDark)
            .unwrap();
        for placement in ["fo", "old", "tspan"] {
            let rendered = render(
                "journey\nsection Phase\nTask: 5: Actor\nsection Next\nNext task: 4: Actor\n",
                Some(&theme),
                json!({
                    "journey": {"textPlacement": placement, "sectionColours": ["#fedcba"]},
                    "themeVariables": {"textColor": "#abcdef", "fillType0": "#123456", "fillType1": "#123456"}
                }),
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            for rect in document.descendants().filter(|node| {
                node.has_tag_name("rect")
                    && (has_class(*node, "task") || has_class(*node, "journey-section"))
            }) {
                assert_eq!(
                    declaration(rect.attribute("style").unwrap(), "fill"),
                    Some("#123456")
                );
                for terminal in labels(rect, placement) {
                    if placement == "tspan" {
                        // Mermaid appends authored array entries after the default #fff.
                        let first =
                            has_class(rect, "task-type-0") || has_class(rect, "section-type-0");
                        assert_eq!(
                            terminal.attribute("fill"),
                            Some(if first { "#fff" } else { "#fedcba" })
                        );
                    } else if terminal.has_tag_name("text") && placement == "fo" {
                        assert_eq!(surface_ink(terminal), Some("#abcdef"));
                    } else {
                        assert!(surface_ink(terminal).is_none());
                    }
                }
            }
            assert_eq!(
                css_property(&document, &format!("#{ID} .legend"), "fill").as_deref(),
                Some("#abcdef")
            );
        }
    }

    #[test]
    fn later_authored_text_keeps_rule_order_over_task_labels() {
        let theme = recipe(json!([
            {"family": "journey", "target": "task-label", "style": {"fill": "#123456"}},
            {"family": "journey", "target": "text", "style": {"fill": "#654321"}}
        ]));
        let rendered = render(
            "journey\nTask: 5: Actor\n",
            Some(&theme),
            json!({"journey": {"textPlacement": "old"}}),
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let label = document
            .descendants()
            .find(|node| node.has_tag_name("text") && has_class(*node, "task"))
            .unwrap();
        assert_eq!(surface_ink(label), Some("#654321"));
        assert_fully_accounted(rendered, 1, 1);
    }

    #[test]
    fn section_colours_own_only_the_tspan_path() {
        let theme = recipe(json!([
            {"family": "journey", "target": "task-label", "style": {"fill": "#123456"}}
        ]));
        for placement in ["fo", "old", "tspan"] {
            let rendered = render(
                "journey\nsection Phase\nTask: 5: Actor\nsection Next\nNext task: 4: Actor\n",
                Some(&theme),
                json!({
                    "journey": {"textPlacement": placement, "sectionColours": ["#abcdef"]}
                }),
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            for rect in document.descendants().filter(|node| {
                node.has_tag_name("rect")
                    && (has_class(*node, "task") || has_class(*node, "journey-section"))
            }) {
                for terminal in labels(rect, placement) {
                    if placement == "tspan" {
                        assert!(surface_ink(terminal).is_none());
                        let first =
                            has_class(rect, "task-type-0") || has_class(rect, "section-type-0");
                        assert_eq!(
                            terminal.attribute("fill"),
                            Some(if first { "#fff" } else { "#abcdef" })
                        );
                    } else {
                        assert_eq!(surface_ink(terminal), Some("#123456"));
                    }
                }
            }
            assert_fully_accounted(
                rendered,
                usize::from(placement != "tspan"),
                usize::from(placement == "tspan"),
            );
        }
    }

    #[test]
    fn empty_theme_preserves_mermaid_journey_output_for_all_text_placements() {
        let empty = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new())
            .unwrap();
        for placement in ["fo", "old", "tspan"] {
            let config = json!({"journey": {"textPlacement": placement}});
            assert_eq!(
                render(&source(), None, config.clone()).svg(),
                render(&source(), Some(&empty), config).svg()
            );
        }
    }
}
