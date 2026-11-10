#![cfg(feature = "diagram-journey")]

mod common;

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_render::DiagramFamilyId;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, Specified, ThemeRule,
    ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::{Value, json};

const ID: &str = "journey-task-label-visibility";
const SOURCE: &str = "journey\n  section Phase\n  Task: 5: Actor\n";
const INK: &str = "#123456";

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
    let environment = RenderEnvironment::deterministic();
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

fn theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    let styles = rules
        .into_iter()
        .fold(ThemeRuleSet::default(), |styles, rule| {
            styles.with_rule(rule.for_family(DiagramFamilyId::JOURNEY))
        });
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .unwrap()
}

fn fill_rule(target: ThemeTarget, css: &str) -> ThemeRule {
    ThemeRule::new(
        target,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid(css).unwrap()),
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

fn surface_labels<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
) -> Vec<roxmltree::Node<'a, 'input>> {
    document
        .descendants()
        .filter(|node| {
            (node.has_tag_name("text")
                && (has_class(*node, "task") || has_class(*node, "journey-section")))
                || (node.has_tag_name("div") && has_class(*node, "label"))
        })
        .collect()
}

fn assert_surface_fill(svg: &str, expected: &str, count: usize) {
    let document = roxmltree::Document::parse(svg).unwrap();
    let labels = surface_labels(&document);
    assert_eq!(labels.len(), count, "actual section/task writers");
    for label in labels {
        let property = if label.has_tag_name("div") {
            "color"
        } else {
            "fill"
        };
        assert_eq!(
            declaration(label.attribute("style").unwrap_or_default(), property),
            Some(expected),
            "{label:?}"
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
    assert_eq!(
        evidence.required_count(),
        applied + absent + residual,
        "{evidence:?}"
    );
    assert_eq!(evidence.applied_count(), applied, "{evidence:?}");
    assert_eq!(evidence.not_applicable_count(), absent, "{evidence:?}");
    assert_eq!(evidence.theme_residual_count(), residual, "{evidence:?}");
}

#[test]
fn task_label_visibility_uses_the_existing_placement_specific_font_size_owner() {
    // There is no typed typography request: paint evidence must still observe source font sizes.
    let theme = theme([fill_rule(ThemeTarget::TaskLabel, INK)]);
    for (placement, base_size, task_size, visible) in [
        ("old", "0px", 14.0, false),
        ("fo", "0px", 14.0, false),
        ("tspan", "16px", 0.0, false),
        ("tspan", "0px", 0.0, false),
        ("tspan", "0px", -1.0, false),
        ("old", "16px", 0.0, true),
        ("fo", "16px", 0.0, true),
        ("tspan", "0px", 14.0, true),
        ("tspan", "16px", -1.0, true),
        ("old", "0.5px", 0.0, true),
        ("tspan", "0px", 0.5, true),
    ] {
        let rendered = render(
            SOURCE,
            Some(&theme),
            json!({
                "themeVariables": {"fontSize": base_size},
                "journey": {"textPlacement": placement, "taskFontSize": task_size}
            }),
        );
        // Zero size changes visibility, not writer completeness or paint emission.
        assert_surface_fill(rendered.svg(), INK, if placement == "fo" { 4 } else { 2 });
        assert!(rendered.svg().contains(&format!("font-size:{base_size};")));
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        if placement == "tspan" {
            for label in surface_labels(&document) {
                assert_eq!(
                    declaration(label.attribute("style").unwrap(), "font-size"),
                    (task_size >= 0.0)
                        .then(|| format!("{task_size}px"))
                        .as_deref()
                );
            }
        }
        assert_evidence(rendered, usize::from(visible), usize::from(!visible), 0);
    }
}

#[test]
fn a_source_font_family_named_none_does_not_hide_positive_size_labels() {
    let theme = theme([fill_rule(ThemeTarget::TaskLabel, INK)]);
    for placement in ["fo", "old", "tspan"] {
        let rendered = render(
            SOURCE,
            Some(&theme),
            json!({
                "themeVariables": {"fontFamily": "none", "fontSize": "16px"},
                "journey": {"textPlacement": placement, "taskFontFamily": "none", "taskFontSize": 14}
            }),
        );
        assert!(rendered.svg().contains("font-family:none"));
        assert_surface_fill(rendered.svg(), INK, if placement == "fo" { 4 } else { 2 });
        assert_evidence(rendered, 1, 0, 0);
    }
}

#[test]
fn empty_task_label_domains_do_not_borrow_visibility_from_title_or_actors() {
    let theme = theme([fill_rule(ThemeTarget::TaskLabel, INK)]);
    for placement in ["fo", "old", "tspan"] {
        for source in [
            "journey\n",
            "journey\n  title Canvas only\n",
            "journey\n  section Unused\n",
        ] {
            let rendered = render(
                source,
                Some(&theme),
                json!({"journey": {"textPlacement": placement}}),
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert!(surface_labels(&document).is_empty());
            assert_evidence(rendered, 0, 1, 0);
        }

        let rendered = render(
            "journey\n  title Canvas\n  section <br>\n  <br>: 5: Actor\n",
            Some(&theme),
            json!({"journey": {"textPlacement": placement}}),
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(
            document
                .descendants()
                .any(|node| node.is_text() && node.text() == Some("Actor"))
        );
        assert!(
            document
                .descendants()
                .any(|node| node.is_text() && node.text() == Some("Canvas"))
        );
        // Old/fo write the escaped break tag literally; tspan splits it into empty runs.
        if placement == "tspan" {
            assert_eq!(surface_labels(&document).len(), 4);
            assert!(surface_labels(&document).iter().all(|label| {
                label
                    .descendants()
                    .filter(|node| node.is_text())
                    .all(|node| node.text().unwrap().trim().is_empty())
            }));
        }
        assert_evidence(
            rendered,
            usize::from(placement != "tspan"),
            usize::from(placement == "tspan"),
            0,
        );
    }
}

#[test]
fn either_a_visible_section_or_a_visible_task_can_consume_task_label_fill() {
    let theme = theme([fill_rule(ThemeTarget::TaskLabel, INK)]);
    for source in [
        "journey\n  section Phase\n  <br>: 5: Actor\n",
        "journey\n  section <br>\n  Task: 5: Actor\n",
    ] {
        let rendered = render(
            source,
            Some(&theme),
            json!({"journey": {"textPlacement": "tspan"}}),
        );
        assert_surface_fill(rendered.svg(), INK, 3);
        assert_evidence(rendered, 1, 0, 0);
    }
}

#[test]
fn source_text_color_and_tspan_section_colors_retain_fill_ownership() {
    let theme = theme([fill_rule(ThemeTarget::TaskLabel, INK)]);
    for placement in ["fo", "old", "tspan"] {
        let text_owned = json!({
            "themeVariables": {"textColor": "#abcdef"},
            "journey": {"textPlacement": placement}
        });
        let rendered = render(SOURCE, Some(&theme), text_owned.clone());
        assert_eq!(rendered.svg(), render(SOURCE, None, text_owned).svg());
        assert_evidence(rendered, 0, 1, 0);

        let section_owned =
            json!({"journey": {"textPlacement": placement, "sectionColours": ["#fedcba"]}});
        let rendered = render(SOURCE, Some(&theme), section_owned.clone());
        if placement == "tspan" {
            assert_eq!(rendered.svg(), render(SOURCE, None, section_owned).svg());
            assert_evidence(rendered, 0, 1, 0);
        } else {
            assert_surface_fill(rendered.svg(), INK, if placement == "fo" { 4 } else { 2 });
            assert_evidence(rendered, 1, 0, 0);
        }
    }
}

#[test]
fn mixed_unsupported_facets_need_visible_text_even_when_source_owns_fill() {
    let fill = ThemeStylePatch::default().with_fill(CanvasPaint::solid(INK).unwrap());
    let mut with_font_weight = fill.clone();
    with_font_weight.typography.font_weight = Specified::Value(700);
    for patch in [
        fill.with_stroke(CanvasPaint::solid("#654321").unwrap()),
        with_font_weight,
    ] {
        let theme = theme([ThemeRule::new(ThemeTarget::TaskLabel, patch)]);
        for placement in ["fo", "old", "tspan"] {
            for visible in [false, true] {
                for source_owned in [false, true] {
                    let mut config = json!({
                        "themeVariables": {"fontSize": if visible {"16px"} else {"0px"}},
                        "journey": {"textPlacement": placement, "taskFontSize": if visible {14} else {0}}
                    });
                    if source_owned {
                        config["themeVariables"]["textColor"] = json!("#abcdef");
                    }
                    let rendered = render(SOURCE, Some(&theme), config);
                    if !source_owned {
                        assert_surface_fill(
                            rendered.svg(),
                            INK,
                            if placement == "fo" { 4 } else { 2 },
                        );
                    }
                    assert_evidence(rendered, 0, usize::from(!visible), usize::from(visible));
                }
            }
        }
        assert_evidence(render("journey\n", Some(&theme), json!({})), 0, 1, 0);
    }
}

#[test]
fn visible_task_labels_keep_text_and_task_label_author_order() {
    for placement in ["fo", "old", "tspan"] {
        for later_text in [false, true] {
            let text = fill_rule(ThemeTarget::Text, "#654321");
            let label = fill_rule(ThemeTarget::TaskLabel, INK);
            let theme = theme(if later_text {
                [label, text]
            } else {
                [text, label]
            });
            let rendered = render(
                SOURCE,
                Some(&theme),
                json!({"journey": {"textPlacement": placement}}),
            );
            assert_surface_fill(
                rendered.svg(),
                if later_text { "#654321" } else { INK },
                if placement == "fo" { 4 } else { 2 },
            );
            assert_evidence(
                rendered,
                if later_text { 1 } else { 2 },
                usize::from(later_text),
                0,
            );
        }
    }
}

#[test]
fn empty_theme_keeps_default_svg_bytes_in_every_text_placement() {
    let empty = theme([]);
    for placement in ["fo", "old", "tspan"] {
        for source in [SOURCE, "journey\n", "journey\n  <br>: 5: Actor\n"] {
            for zero_size in [false, true] {
                let config = json!({
                    "themeVariables": {"fontSize": if zero_size {"0px"} else {"16px"}},
                    "journey": {"textPlacement": placement, "taskFontSize": if zero_size {0} else {14}}
                });
                assert_eq!(
                    render(source, None, config.clone()).svg(),
                    render(source, Some(&empty), config).svg(),
                    "{placement}/{zero_size}: {source}"
                );
            }
        }
    }
}
