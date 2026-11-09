use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette,
    OrdinalSelector, Specified, ThemeGeometryPatch, ThemePortabilityRequirement, ThemeRule,
    ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn journey_task_paint_theme() -> merman_render::diagram_theme::DiagramTheme {
    journey_task_paint_theme_with_variant(
        None,
        CanvasPaint::solid("#123456").expect("valid Journey task fill"),
        CanvasPaint::solid("#654321").expect("valid Journey task stroke"),
    )
}

fn journey_task_paint_theme_with_variant(
    variant: Option<ThemeVariant>,
    fill: CanvasPaint,
    stroke: CanvasPaint,
) -> merman_render::diagram_theme::DiagramTheme {
    let mut fill_rule = ThemeRule::new(
        ThemeTarget::JourneyTask,
        ThemeStylePatch::default().with_fill(fill),
    );
    let mut stroke_rule = ThemeRule::new(
        ThemeTarget::JourneyTask,
        ThemeStylePatch::default().with_stroke(stroke),
    );
    if let Some(variant) = variant {
        fill_rule = fill_rule.with_variant(variant);
        stroke_rule = stroke_rule.with_variant(variant);
    }
    let styles = ThemeRuleSet::default()
        .with_rule(fill_rule)
        .with_rule(stroke_rule);
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile Journey task paint theme")
}

fn journey_typography_theme(font_size: f32) -> merman_render::diagram_theme::DiagramTheme {
    let typography = ThemeTextStyle::default()
        .with_font_stack(
            merman_render::diagram_theme::FontStack::single("Journey Typed")
                .expect("valid Journey font stack"),
        )
        .with_font_size_px(font_size)
        .expect("valid Journey font size");
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default()
                    .with_family_style(merman_render::DiagramFamilyId::JOURNEY, typography),
            ),
        )
        .expect("compile Journey typography theme")
}

#[test]
fn journey_ignored_base_sizes_do_not_own_typed_typography() {
    let theme = journey_typography_theme(27.0);
    let diagram =
        "journey\n  title Release audit\n  section Delivery\n    Ship release: 5: Maintainer\n";
    for source_owned in [false, true] {
        for (size, expected, applied) in [
            (serde_json::json!("  "), "27px", 2),
            (serde_json::json!("bogus"), "27px", 2),
            (serde_json::json!([32]), "27px", 2),
            (serde_json::json!(-2), "27px", 2),
            (serde_json::json!("-2px"), "27px", 2),
            (serde_json::json!("32px"), "32px", 1),
            (serde_json::json!(32), "32px", 1),
        ] {
            let config = serde_json::json!({"themeVariables": {"fontSize": size}});
            let (source, engine) = if source_owned {
                (
                    format!("---\nconfig: {config}\n---\n{diagram}"),
                    MermaidEngine::new(),
                )
            } else {
                (
                    diagram.to_owned(),
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(config),
                    ),
                )
            };
            let parsed = common::install_parse_compatibility(&theme, engine)
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let session = RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .unwrap();
            let rendered =
                family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
                    .unwrap()
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .unwrap();
            assert!(rendered.svg().contains(&format!("font-size:{expected}")));
            assert!(rendered.svg().contains("font-family:\"Journey Typed\""));
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 2);
            assert_eq!(evidence.applied_count(), applied);
            assert_eq!(evidence.not_applicable_count(), 2 - applied);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn journey_zero_sizes_keep_hidden_runs_without_certifying_typography() {
    let theme = journey_typography_theme(24.0);
    for placement in ["fo", "old", "tspan"] {
        for task_size in [0.0, 0.5] {
            let rendered = render_journey_with_theme_requirement(
                "journey\nsection Work\nShip: 5: Reader\n",
                &theme,
                serde_json::json!({
                    "themeVariables": {"fontSize": "0px"},
                    "journey": {"textPlacement": placement, "taskFontSize": task_size,
                                "taskFontFamily": "inherit"}
                }),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            assert!(rendered.svg().contains("font-size:0px;"));
            assert!(rendered.svg().contains("Reader"));
            assert!(rendered.svg().contains("Ship"));
            if placement != "old" {
                assert!(
                    rendered
                        .svg()
                        .contains(&format!("font-size: {task_size}px;"))
                );
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            let applied = usize::from(placement == "tspan" && task_size > 0.0);
            assert_eq!(evidence.applied_count(), applied, "{placement}/{task_size}");
            assert_eq!(evidence.not_applicable_count(), 2 - applied);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn journey_subpixel_typed_size_reaches_css_and_terminal_receipt() {
    let theme = journey_typography_theme(0.5);
    let rendered = render_journey_with_theme_requirement(
        "journey\nsection Work\nShip: 5: Reader\n",
        &theme,
        serde_json::json!({}),
        ThemePortabilityRequirement::RequirePortable,
    )
    .unwrap();
    assert!(rendered.svg().contains("font-size:0.5px;"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn journey_invalid_negative_task_size_inherits_instead_of_hiding_text() {
    let theme = journey_typography_theme(27.0);
    for task_size in [-2.0, -0.5] {
        let rendered = render_journey_with_theme_requirement(
            "journey\nsection Work\nShip: 5\n",
            &theme,
            serde_json::json!({"journey": {"textPlacement": "tspan",
                "taskFontSize": task_size, "taskFontFamily": "inherit"}}),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for text in document
            .descendants()
            .filter(|node| node.has_tag_name("text"))
        {
            assert!(!text.attribute("style").unwrap_or("").contains("font-size:"));
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 2);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn journey_hidden_title_does_not_consume_title_rules() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Title,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            ))),
        )
        .unwrap();
    let mut unsupported_style = ThemeStylePatch::default();
    unsupported_style.typography.font_size_px = Specified::Value(31.0);
    let unsupported = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(ThemeRule::new(ThemeTarget::Title, unsupported_style)),
            ),
        )
        .unwrap();
    for (base, title_size, applied) in [
        ("0px", "4ex", 0),
        ("0px", "24px", 1),
        ("16px", "0px", 0),
        ("0px", "var(--title-size)", 1),
        ("16px", "0px !important", 1),
        ("16px", "0px;", 1),
        ("16px", "-5e-324%", 1),
    ] {
        let rendered = render_journey_with_theme_requirement(
            "journey\ntitle Release\nsection Work\nShip: 5: Reader\n",
            &theme,
            serde_json::json!({"themeVariables": {"fontSize": base},
                "journey": {"titleFontSize": title_size}}),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(
            document
                .descendants()
                .any(|node| node.has_tag_name("text") && node.text() == Some("Release"))
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), applied, "{base}/{title_size}");
        assert_eq!(evidence.not_applicable_count(), 1 - applied);
        assert_eq!(evidence.theme_residual_count(), 0);
        let result = render_journey_with_theme_requirement(
            "journey\ntitle Release\nsection Work\nShip: 5: Reader\n",
            &unsupported,
            serde_json::json!({"themeVariables": {"fontSize": base},
                "journey": {"titleFontSize": title_size}}),
            ThemePortabilityRequirement::RequirePortable,
        );
        assert_eq!(result.is_err(), applied != 0, "{base}/{title_size}");
    }
}

#[test]
fn journey_text_fill_has_portable_terminal_evidence() {
    for variant in [None, Some(ThemeVariant::Default)] {
        for paint in [
            CanvasPaint::solid("#123456").unwrap(),
            CanvasPaint::Transparent,
        ] {
            let mut rule = ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default().with_fill(paint),
            );
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)),
                )
                .expect("compile Journey Text.fill");
            let parsed = common::install_parse_compatibility(
                &theme,
                MermaidEngine::new(),
            )
            .parse_diagram_for_render_model_sync(
                "journey\n  title Release\n  section Delivery\n    Ship release: 5: Maintainer\n",
                ParseOptions::strict(),
            )
            .expect("parse Journey")
            .expect("detect Journey").into_presentation();
            let session = RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("start portable Journey session");
            let artifact =
                family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
                    .expect("prepare Journey");
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .expect("Journey Text.fill has direct portable evidence");
            let document = roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG");
            let title = document
                .descendants()
                .find(|node| node.has_tag_name("text") && node.text() == Some("Release"))
                .unwrap();
            assert_eq!(
                title.attribute("fill"),
                Some(""),
                "Text paint remains inherited"
            );
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.accounted_count(), 1);
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

fn render_journey_text_rules(
    source: &str,
    styles: ThemeRuleSet,
    config: serde_json::Value,
) -> merman_render::Result<family::RenderedFamilySvg> {
    render_journey_rules_with_requirement(
        source,
        styles,
        config,
        ThemePortabilityRequirement::RequirePortable,
    )
}

fn render_journey_rules_with_requirement(
    source: &str,
    styles: ThemeRuleSet,
    config: serde_json::Value,
    requirement: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile Journey rules");
    render_journey_with_theme_requirement(source, &theme, config, requirement)
}

fn render_journey_with_theme_requirement(
    source: &str,
    theme: &DiagramTheme,
    config: serde_json::Value,
    requirement: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = common::install_parse_compatibility(
        theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(config),
        ),
    )
    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
    .expect("parse Journey")
    .expect("detect Journey")
    .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(requirement)
        .begin_session_with_theme(theme)
        .expect("begin Journey session");
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
}

#[test]
fn journey_numeric_actor_legend_preserves_identity_and_authored_task_people() {
    for (people, expected_legend, expected_legend_classes) in [
        ("10, 2, 10", ["2", "10"], ["actor-1", "actor-0"]),
        ("A10, A2, A10", ["A10", "A2"], ["actor-0", "actor-1"]),
    ] {
        let source = format!("journey\nPay: 4: {people}\n");
        let rendered =
            render_journey_text_rules(&source, ThemeRuleSet::default(), serde_json::json!({}))
                .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let legend_text = document
            .descendants()
            .filter(|node| node.has_tag_name("text") && node.attribute("class") == Some("legend"))
            .map(|node| {
                node.descendants()
                    .find(|span| span.has_tag_name("tspan"))
                    .and_then(|span| span.text())
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(legend_text, expected_legend, "{people}");

        let legend_circles = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("circle")
                    && node
                        .attribute("class")
                        .is_some_and(|class| class.starts_with("actor-"))
                    && !node.children().any(|child| child.has_tag_name("title"))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            legend_circles
                .iter()
                .map(|node| node.attribute("class").unwrap())
                .collect::<Vec<_>>(),
            expected_legend_classes,
            "{people}"
        );
        assert_eq!(
            legend_circles
                .iter()
                .map(|node| node.attribute("cy").unwrap())
                .collect::<Vec<_>>(),
            ["60", "80"]
        );

        let task_circles = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("circle")
                    && node.children().any(|child| child.has_tag_name("title"))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            task_circles
                .iter()
                .map(|node| {
                    node.children()
                        .find(|child| child.has_tag_name("title"))
                        .and_then(|title| title.text())
                        .unwrap()
                })
                .collect::<Vec<_>>(),
            people.split(", ").collect::<Vec<_>>()
        );
        assert_eq!(
            task_circles
                .iter()
                .map(|node| node.attribute("class").unwrap())
                .collect::<Vec<_>>(),
            ["actor-0", "actor-1", "actor-0"]
        );
        assert_eq!(
            task_circles
                .iter()
                .map(|node| node.attribute("fill").unwrap())
                .collect::<Vec<_>>(),
            ["#8FBC8F", "#7CFC00", "#8FBC8F"]
        );
    }
}

#[test]
fn journey_unicode_actor_order_matches_utf16_sort_and_preserves_source_order() {
    for (people, expected, task_classes, task_colors) in [
        (
            "😀, \u{e000}",
            ["😀", "\u{e000}"],
            ["actor-0", "actor-1"],
            ["#8FBC8F", "#7CFC00"],
        ),
        (
            "\u{e000}, 😀",
            ["😀", "\u{e000}"],
            ["actor-1", "actor-0"],
            ["#7CFC00", "#8FBC8F"],
        ),
        (
            "A, Z",
            ["A", "Z"],
            ["actor-0", "actor-1"],
            ["#8FBC8F", "#7CFC00"],
        ),
    ] {
        let source = format!("journey\nPay: 4: {people}\n");
        let rendered =
            render_journey_text_rules(&source, ThemeRuleSet::default(), serde_json::json!({}))
                .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let legend = document
            .descendants()
            .filter(|node| node.has_tag_name("text") && node.attribute("class") == Some("legend"))
            .map(|node| {
                node.descendants()
                    .find(|span| span.has_tag_name("tspan"))
                    .and_then(|span| span.text())
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(legend, expected, "{people}");

        let legend_circles = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("circle")
                    && node
                        .attribute("class")
                        .is_some_and(|class| class.starts_with("actor-"))
                    && !node.children().any(|child| child.has_tag_name("title"))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            legend_circles
                .iter()
                .map(|node| node.attribute("class").unwrap())
                .collect::<Vec<_>>(),
            ["actor-0", "actor-1"]
        );
        assert_eq!(
            legend_circles
                .iter()
                .map(|node| node.attribute("cy").unwrap())
                .collect::<Vec<_>>(),
            ["60", "80"]
        );

        let task_circles = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("circle")
                    && node.children().any(|child| child.has_tag_name("title"))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            task_circles
                .iter()
                .map(|node| {
                    node.children()
                        .find(|child| child.has_tag_name("title"))
                        .and_then(|title| title.text())
                        .unwrap()
                })
                .collect::<Vec<_>>(),
            people.split(", ").collect::<Vec<_>>()
        );
        assert_eq!(
            task_circles
                .iter()
                .map(|node| node.attribute("class").unwrap())
                .collect::<Vec<_>>(),
            task_classes
        );
        assert_eq!(
            task_circles
                .iter()
                .map(|node| node.attribute("fill").unwrap())
                .collect::<Vec<_>>(),
            task_colors
        );
    }
}

#[test]
fn journey_frontmatter_zero_cell_dimensions_reach_terminal_svg_geometry() {
    for (width, height, task_y, face_x, activity_y) in [
        (0, 0, "10", "150", "0"),
        (0, 50, "110", "150", "200"),
        (150, 0, "10", "225", "0"),
        (1, 1, "12", "150.5", "4"),
    ] {
        let source = format!(
            "---\nconfig:\n  journey:\n    width: {width}\n    height: {height}\n---\njourney\nsection Checkout\nPay: 4\n"
        );
        let rendered =
            render_journey_text_rules(&source, ThemeRuleSet::default(), serde_json::json!({}))
                .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let task = document
            .descendants()
            .find(|node| {
                node.has_tag_name("rect")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "task")
                    })
            })
            .unwrap();
        let section = document
            .descendants()
            .find(|node| {
                node.has_tag_name("rect")
                    && node.attribute("class").is_some_and(|classes| {
                        classes
                            .split_whitespace()
                            .any(|class| class == "journey-section")
                    })
            })
            .unwrap();
        let expected_width = width.to_string();
        let expected_height = height.to_string();
        for rect in [task, section] {
            assert_eq!(rect.attribute("width"), Some(expected_width.as_str()));
            assert_eq!(rect.attribute("height"), Some(expected_height.as_str()));
        }
        assert_eq!(task.attribute("y"), Some(task_y));
        assert_eq!(section.attribute("y"), Some("50"));
        let face = document
            .descendants()
            .find(|node| node.has_tag_name("circle") && node.attribute("class") == Some("face"))
            .unwrap();
        assert_eq!(face.attribute("cx"), Some(face_x));
        let task_line = document
            .descendants()
            .find(|node| node.has_tag_name("line") && node.attribute("class") == Some("task-line"))
            .unwrap();
        assert_eq!(task_line.attribute("x1"), Some(face_x));
        assert_eq!(task_line.attribute("x2"), Some(face_x));
        assert_eq!(task_line.attribute("y1"), Some(task_y));
        assert_eq!(task_line.attribute("y2"), Some("450"));
        let activity_line = document
            .descendants()
            .find(|node| node.has_tag_name("line") && node.attribute("marker-end").is_some())
            .unwrap();
        assert_eq!(activity_line.attribute("y1"), Some(activity_y));
        assert_eq!(activity_line.attribute("y2"), Some(activity_y));
    }
}

#[test]
fn journey_scores_select_fractional_faces_and_preserve_integer_and_nan_controls() {
    for (literal, expected_y, mouth_tag, mouth_y) in [
        ("3.5", "345", "path", Some("347")),
        ("3.01", "359.7", "path", Some("361.7")),
        ("2.75", "367.5", "path", Some("374.5")),
        ("3", "360", "line", None),
        ("4", "330", "path", Some("332")),
        ("bad", "NaN", "line", None),
        ("NaN", "NaN", "line", None),
        ("Infinity", "NaN", "line", None),
    ] {
        let source = format!("journey\nPay: {literal}: Customer\n");
        let rendered =
            render_journey_text_rules(&source, ThemeRuleSet::default(), serde_json::json!({}))
                .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let face = document
            .descendants()
            .find(|node| node.has_tag_name("circle") && node.attribute("class") == Some("face"))
            .unwrap();
        assert_eq!(face.attribute("cy"), Some(expected_y), "{literal}");
        let mouth = document
            .descendants()
            .find(|node| node.attribute("class") == Some("mouth"))
            .unwrap();
        assert_eq!(mouth.tag_name().name(), mouth_tag, "{literal}");
        let transform =
            mouth_y.map(|y| format!("translate({},{y})", face.attribute("cx").unwrap()));
        assert_eq!(
            mouth.attribute("transform"),
            transform.as_deref(),
            "{literal}"
        );
        if expected_y == "NaN" {
            assert_eq!(mouth.attribute("y1"), Some("NaN"));
            assert_eq!(mouth.attribute("y2"), Some("NaN"));
        }
    }
}

#[test]
fn journey_tspan_breaks_keep_the_opening_tag_rule() {
    let theme = journey_typography_theme(21.0);
    for (separator, expected) in [
        ("<BR />", vec!["Pay", "now"]),
        ("<br\u{00a0}/>", vec!["Pay", "now"]),
        ("</br>", vec!["Pay</br>now"]),
    ] {
        let source = format!("journey\nPay{separator}now: 4\n");
        let rendered = render_journey_with_theme_requirement(
            &source,
            &theme,
            serde_json::json!({"journey": {"textPlacement": "tspan"}}),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let rows: Vec<_> = document
            .descendants()
            .filter(|node| node.has_tag_name("text") && node.attribute("class") == Some("task"))
            .map(|node| {
                node.descendants()
                    .find(|child| child.is_text())
                    .unwrap()
                    .text()
                    .unwrap()
            })
            .collect();
        assert_eq!(rows, expected, "{separator:?}");
    }
}

#[test]
fn journey_text_placement_owns_label_dom_fonts_and_typography_evidence() {
    let theme = journey_typography_theme(21.0);
    let source = "journey\nsection Order<br/>review\nPay<br>now: 4\n";
    for placement in [None, Some("fo"), Some("tspan"), Some("old")] {
        let mut config = serde_json::json!({
            "journey": {"taskFontSize": 20, "taskFontFamily": "Task Choice"}
        });
        if let Some(placement) = placement {
            config["journey"]["textPlacement"] = serde_json::json!(placement);
        }
        let rendered = render_journey_with_theme_requirement(
            source,
            &theme,
            config,
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("each selected label candidate must have complete terminal evidence");
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let uses_fo = placement.is_none() || placement == Some("fo");
        for tag in ["switch", "foreignObject"] {
            assert_eq!(
                document
                    .descendants()
                    .filter(|node| node.has_tag_name(tag))
                    .count(),
                if uses_fo { 2 } else { 0 },
                "placement {placement:?}, element {tag}"
            );
        }
        let labels = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("text")
                    && node.attribute("class").is_some_and(|class| {
                        class
                            .split_whitespace()
                            .any(|token| token == "task" || token == "journey-section")
                    })
            })
            .collect::<Vec<_>>();
        if placement == Some("old") {
            assert_eq!(labels.len(), 2);
            assert_eq!(labels[0].text(), Some("Order<br/>review"));
            assert_eq!(labels[1].text(), Some("Pay<br>now"));
            assert_eq!(labels[0].attribute("y"), Some("80"));
            assert_eq!(labels[1].attribute("y"), Some("140"));
            assert!(labels.iter().all(|node| {
                node.children().all(|child| !child.has_tag_name("tspan"))
                    && node.attribute("style") == Some("text-anchor: middle;")
            }));
        } else {
            assert_eq!(labels.len(), 4);
            for (index, (node, text)) in labels
                .iter()
                .zip(["Order", "review", "Pay", "now"])
                .enumerate()
            {
                assert_eq!(node.attribute("x"), Some("225"));
                assert_eq!(
                    node.attribute("y"),
                    Some(if index < 2 { "75" } else { "135" })
                );
                assert_eq!(node.attribute("dominant-baseline"), Some("central"));
                let style = node.attribute("style").unwrap();
                assert!(style.contains("font-size: 20px;"), "{style}");
                assert!(style.contains("font-family: Task Choice;"), "{style}");
                let tspan = node
                    .children()
                    .find(|child| child.has_tag_name("tspan"))
                    .unwrap();
                assert_eq!(
                    tspan.attribute("dy"),
                    Some(if index % 2 == 0 { "-10" } else { "10" })
                );
                assert_eq!(tspan.text(), Some(text));
                if placement == Some("tspan") {
                    assert_eq!(node.attribute("fill"), Some("#fff"));
                    assert!(!style.contains("fill:"));
                }
            }
        }
        // With no actors, tspan has no inherited text candidate. Fo and old really consume
        // the base font; the task-specific SVG font must not stand in for that evidence.
        let inherited = placement != Some("tspan");
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.accounted_count(), 2);
        assert_eq!(evidence.applied_count(), if inherited { 2 } else { 0 });
        assert_eq!(
            evidence.not_applicable_count(),
            if inherited { 0 } else { 2 }
        );
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn journey_tspan_inherited_font_stack_keeps_task_font_size_independent() {
    let size_only = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(
                merman_render::DiagramFamilyId::JOURNEY,
                ThemeTextStyle::default().with_font_size_px(21.0).unwrap(),
            ),
        ))
        .unwrap();
    for (theme, required, inherited_applied) in
        [(journey_typography_theme(21.0), 2, 1), (size_only, 1, 0)]
    {
        for (font_family, inherits) in [
            ("inherit", true),
            ("unset", true),
            (r#""inherit""#, false),
            ("Task Choice", false),
        ] {
            for source in ["journey\nPay: 4\n", "journey\nsection Checkout\nPay: 4\n"] {
                let rendered = render_journey_with_theme_requirement(
                    source,
                    &theme,
                    serde_json::json!({"journey": {
                        "textPlacement": "tspan",
                        "taskFontFamily": font_family,
                        "taskFontSize": 14
                    }}),
                    ThemePortabilityRequirement::RequirePortable,
                )
                .unwrap();
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                assert!(!document.descendants().any(|node| {
                    node.has_tag_name("foreignObject") || node.attribute("class") == Some("legend")
                }));
                let labels = document
                    .descendants()
                    .filter(|node| node.has_tag_name("text"))
                    .collect::<Vec<_>>();
                assert!(!labels.is_empty());
                for label in labels {
                    let style = label.attribute("style").unwrap();
                    assert!(style.contains(&format!("font-family: {font_family};")));
                    assert!(style.contains("font-size: 14px;"));
                }
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                let applied = if inherits { inherited_applied } else { 0 };
                assert_eq!(evidence.required_count(), required);
                assert_eq!(evidence.accounted_count(), required);
                assert_eq!(evidence.applied_count(), applied, "{font_family}");
                assert_eq!(evidence.not_applicable_count(), required - applied);
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn journey_tspan_empty_rows_do_not_certify_inherited_fonts() {
    let rendered = render_journey_with_theme_requirement(
        "journey\n<br/><br>: 4\n",
        &journey_typography_theme(21.0),
        serde_json::json!({"journey": {
            "textPlacement": "tspan",
            "taskFontFamily": "inherit",
            "taskFontSize": 14
        }}),
        ThemePortabilityRequirement::RequirePortable,
    )
    .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let rows = document
        .descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|row| row.text().is_none()));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn journey_tspan_decoded_whitespace_is_not_a_visible_font_consumer() {
    for text in ["#32;", "#9;", "#32;<br/>#32;"] {
        let rendered = render_journey_with_theme_requirement(
            &format!("journey\n{text}: 4\n"),
            &journey_typography_theme(21.0),
            serde_json::json!({"journey": {
                "textPlacement": "tspan", "taskFontFamily": "inherit", "taskFontSize": 14
            }}),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let rows = document
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .collect::<Vec<_>>();
        assert!(!rows.is_empty());
        assert!(
            rows.iter()
                .all(|row| row.text().unwrap_or("").trim().is_empty())
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0, "{text}");
        assert_eq!(evidence.not_applicable_count(), 2, "{text}");
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn journey_tspan_visible_weight_and_style_requests_remain_unsupported() {
    for style in [
        ThemeTextStyle::default().with_font_weight(700).unwrap(),
        ThemeTextStyle::default().with_font_style(merman_render::diagram_theme::FontStyle::Italic),
    ] {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_typography(
                    TypographySpec::default()
                        .with_family_style(merman_render::DiagramFamilyId::JOURNEY, style),
                ),
            )
            .unwrap();
        for (text, visible) in [("Pay", true), ("#32;", false)] {
            let result = render_journey_with_theme_requirement(
                &format!("journey\n{text}: 4\n"),
                &theme,
                serde_json::json!({"journey": {
                    "textPlacement": "tspan", "taskFontFamily": "Task Choice", "taskFontSize": 14
                }}),
                ThemePortabilityRequirement::RequirePortable,
            );
            if visible {
                assert!(matches!(
                    result,
                    Err(merman_render::Error::UnverifiedFamilyTheme { .. })
                ));
            } else {
                let rendered = result.unwrap();
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.not_applicable_count(), 1);
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn journey_frontmatter_tspan_retains_inherited_legend_typography() {
    let source = concat!(
        "---\nconfig:\n  journey:\n",
        "    textPlacement: tspan\n",
        "    taskFontSize: 18\n",
        "    taskFontFamily: Task Specific\n",
        "---\njourney\nsection Checkout\nPay: 3.5: A\n",
    );
    let rendered = render_journey_with_theme_requirement(
        source,
        &journey_typography_theme(21.0),
        serde_json::json!({}),
        ThemePortabilityRequirement::RequirePortable,
    )
    .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(
        !document
            .descendants()
            .any(|node| node.has_tag_name("switch"))
    );
    let task = document
        .descendants()
        .find(|node| node.has_tag_name("text") && node.attribute("class") == Some("task"))
        .unwrap();
    assert!(
        task.attribute("style")
            .unwrap()
            .contains("font-family: Task Specific;")
    );
    assert!(
        task.attribute("style")
            .unwrap()
            .contains("font-size: 18px;")
    );
    let legend = document
        .descendants()
        .find(|node| node.has_tag_name("text") && node.attribute("class") == Some("legend"))
        .unwrap();
    assert_eq!(legend.attribute("style"), None);
    assert!(rendered.svg().contains("font-family:\"Journey Typed\""));
    assert!(rendered.svg().contains("font-size:21px"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn journey_tspan_retains_section_colours_separately_from_inherited_text_paint() {
    let source = concat!(
        "journey\n",
        "Before section: 3.5\n",
        "section First\n",
        "Pay: 4\n",
        "section Second\n",
        "Confirm: 3\n",
        "section Third\n",
        "Complete: 4\n",
    );
    let styles = ThemeRuleSet::default().with_rule(ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#abcdef").unwrap()),
    ));
    let rendered = render_journey_text_rules(
        source,
        styles,
        serde_json::json!({"journey": {
            "textPlacement": "tspan",
            "sectionColours": ["#123456", "#654321"]
        }}),
    )
    .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let labels = document
        .descendants()
        .filter(|node| node.has_tag_name("text") && node.attribute("class") == Some("task"))
        .collect::<Vec<_>>();
    assert_eq!(labels.len(), 4);
    // Mermaid's config merge retains the default colour before appending authored colours.
    for (label, colour) in labels.iter().zip(["black", "#fff", "#123456", "#654321"]) {
        assert_eq!(label.attribute("fill"), Some(colour));
        assert!(!label.attribute("style").unwrap().contains("fill:"));
    }
    assert!(rendered.svg().contains("line{stroke:#abcdef;}"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn journey_accessibility_dom_links_complete_punctuation_payloads() {
    for (title, description) in [
        ("Checkout 1", "Confirm then pay"),
        (
            "Checkout #1 ready; confirm & pay",
            "Confirm; then pay #ready",
        ),
    ] {
        let source =
            format!("journey\naccTitle: {title}\naccDescr: {description}\nPay: 4: Customer\n");
        let rendered =
            render_journey_text_rules(&source, ThemeRuleSet::default(), serde_json::json!({}))
                .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let root = document.root_element();
        for (aria, tag, expected) in [
            ("aria-labelledby", "title", title),
            ("aria-describedby", "desc", description),
        ] {
            let target = root.attribute(aria).unwrap();
            let node = root
                .children()
                .find(|node| node.attribute("id") == Some(target))
                .unwrap();
            assert!(node.has_tag_name(tag));
            assert_eq!(node.text(), Some(expected));
        }
    }
}

#[test]
fn journey_title_fill_consumes_real_titles_and_accounts_source_ownership() {
    for variant in [None, Some(ThemeVariant::Default)] {
        for (paint, expected_fill) in [
            (CanvasPaint::solid("#987654").unwrap(), "#987654"),
            (CanvasPaint::Transparent, "transparent"),
        ] {
            let mut rule = ThemeRule::new(
                ThemeTarget::Title,
                ThemeStylePatch::default().with_fill(paint),
            );
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            let styles = ThemeRuleSet::default().with_rule(rule);
            for (source, has_title) in [
                ("journey\n  section Delivery\n  Ship: 5: Actor\n", false),
                (
                    "journey\n  title Release\n  section Delivery\n  Ship: 5: Actor\n",
                    true,
                ),
                (
                    "---\ntitle: Release\n---\njourney\n  section Delivery\n  Ship: 5: Actor\n",
                    true,
                ),
                (
                    "---\ntitle: Release\n---\njourney\n  title  \n  section Delivery\n  Ship: 5: Actor\n",
                    true,
                ),
            ] {
                for explicit_title in [false, true] {
                    let config = if explicit_title {
                        serde_json::json!({"journey": {"titleColor": "#fedcba"}})
                    } else {
                        serde_json::json!({})
                    };
                    let baseline = render_journey_rules_with_requirement(
                        source,
                        ThemeRuleSet::default(),
                        config.clone(),
                        ThemePortabilityRequirement::BestEffort,
                    )
                    .unwrap();
                    let rendered = render_journey_rules_with_requirement(
                        source,
                        styles.clone(),
                        config.clone(),
                        ThemePortabilityRequirement::BestEffort,
                    )
                    .expect("Title fill has a real terminal consumer");
                    let active = has_title && !explicit_title;
                    assert_eq!(rendered.svg() == baseline.svg(), !active);
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    assert!(!document.descendants().any(|node| {
                        node.attribute("class").is_some_and(|classes| {
                            classes.split_whitespace().any(|class| class == "cluster")
                        })
                    }));
                    let title = document
                        .descendants()
                        .find(|node| node.has_tag_name("text") && node.text() == Some("Release"));
                    assert_eq!(title.is_some(), has_title);
                    if let Some(title) = title {
                        assert_eq!(
                            title.attribute("fill"),
                            Some(if explicit_title {
                                "#fedcba"
                            } else {
                                expected_fill
                            })
                        );
                    }
                    let completion = rendered.into_completion();
                    let evidence = merman_render::__private::family_evidence(completion.report());
                    assert_eq!(evidence.required_count(), 1);
                    assert_eq!(evidence.accounted_count(), 1);
                    assert_eq!(evidence.applied_count(), usize::from(active));
                    assert_eq!(evidence.theme_residual_count(), 0);
                    assert_eq!(evidence.not_applicable_count(), usize::from(!active));
                    let strict = render_journey_rules_with_requirement(
                        source,
                        styles.clone(),
                        config,
                        ThemePortabilityRequirement::RequirePortable,
                    );
                    strict.expect("Title fill is applied or not applicable");
                }
            }
        }
    }
}

#[test]
fn journey_historical_title_color_has_no_visible_terminal_consumer() {
    let source = "journey\n  title Release\n  section Delivery\n  Ship: 5: Actor\n";
    let visible_dom = |color: &str| {
        let rendered = render_journey_rules_with_requirement(
            source,
            ThemeRuleSet::default(),
            serde_json::json!({"themeVariables": {"titleColor": color}}),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let stylesheet = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap();
        assert!(
            stylesheet
                .text()
                .unwrap()
                .contains(&format!(".cluster text{{fill:{color};}}"))
        );
        // Compare the complete emitted element/text tree outside the stylesheet. This checks
        // a real legacy input path independently of the matrix and the retired bridge probe.
        document
            .descendants()
            .filter(|node| !node.ancestors().any(|parent| parent.has_tag_name("style")))
            .map(|node| {
                format!(
                    "{:?}:{:?}:{:?}",
                    node.tag_name().name(),
                    node.attributes().collect::<Vec<_>>(),
                    if node.is_text() { node.text() } else { None }
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(visible_dom("#123456"), visible_dom("#987654"));
}

#[test]
fn journey_title_fill_respects_inherited_source_text_color() {
    let diagram = "journey\n  title Release\n  section Delivery\n  Ship: 5: Actor\n";
    for (owner, prefix, site, inherits_source) in [
        ("default", "", serde_json::json!({}), false),
        (
            "site-palette",
            "",
            serde_json::json!({"theme": "dark"}),
            false,
        ),
        (
            "source-palette",
            "%%{init: {\"theme\": \"dark\"}}%%\n",
            serde_json::json!({}),
            true,
        ),
        (
            "site-variable",
            "",
            serde_json::json!({"themeVariables": {"textColor": "#125634"}}),
            true,
        ),
        (
            "source-variable",
            "%%{init: {\"themeVariables\": {\"textColor\": \"#125634\"}}}%%\n",
            serde_json::json!({}),
            true,
        ),
    ] {
        let source = format!("{prefix}{diagram}");
        for title_color in [
            None,
            Some(serde_json::Value::Null),
            Some(serde_json::json!("")),
            Some(serde_json::json!("#223344")),
        ] {
            let explicit_title = title_color
                .as_ref()
                .and_then(serde_json::Value::as_str)
                .filter(|color| !color.is_empty());
            let mut config = site.clone();
            if let Some(color) = &title_color {
                config["journey"] = serde_json::json!({"titleColor": color});
            }
            let baseline = render_journey_rules_with_requirement(
                &source,
                ThemeRuleSet::default(),
                config.clone(),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            for paint in [
                CanvasPaint::solid("#7a3c9a").unwrap(),
                CanvasPaint::Transparent,
            ] {
                let styles = ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::Title,
                    ThemeStylePatch::default().with_fill(paint.clone()),
                ));
                let rendered = render_journey_rules_with_requirement(
                    &source,
                    styles,
                    config.clone(),
                    ThemePortabilityRequirement::RequirePortable,
                )
                .unwrap();
                let owned = inherits_source || explicit_title.is_some();
                assert_eq!(
                    rendered.svg() == baseline.svg(),
                    owned,
                    "{owner}: {title_color:?}"
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let title = document
                    .descendants()
                    .find(|node| node.has_tag_name("text") && node.text() == Some("Release"))
                    .unwrap();
                assert_eq!(
                    title.attribute("fill"),
                    Some(if owned {
                        explicit_title.unwrap_or("")
                    } else if matches!(paint, CanvasPaint::Transparent) {
                        "transparent"
                    } else {
                        "#7a3c9a"
                    }),
                    "{owner}: {title_color:?}"
                );
                let completion = rendered.into_completion();
                let evidence = merman_render::__private::family_evidence(completion.report());
                assert_eq!(evidence.required_count(), 1, "{owner}: {title_color:?}");
                assert_eq!(evidence.accounted_count(), 1, "{owner}: {title_color:?}");
                assert_eq!(
                    evidence.applied_count(),
                    usize::from(!owned),
                    "{owner}: {title_color:?}"
                );
                assert_eq!(
                    evidence.not_applicable_count(),
                    usize::from(owned),
                    "{owner}: {title_color:?}"
                );
                assert_eq!(
                    evidence.theme_residual_count(),
                    0,
                    "{owner}: {title_color:?}"
                );
            }
        }
    }
}

#[test]
fn journey_title_fill_preserves_unsupported_stroke_under_source_and_site_ownership() {
    for channel in [
        "none",
        "site",
        "source",
        "inherited-site",
        "inherited-source",
    ] {
        let source = format!(
            "{}journey\n  title Release\n  section Delivery\n  Ship: 5: Actor\n",
            if channel == "source" {
                "---\nconfig:\n  journey:\n    titleColor: '#fedcba'\n---\n"
            } else if channel == "inherited-source" {
                "---\nconfig:\n  themeVariables:\n    textColor: '#fedcba'\n---\n"
            } else {
                ""
            }
        );
        let config = if channel == "site" {
            serde_json::json!({"journey": {"titleColor": "#fedcba"}})
        } else if channel == "inherited-site" {
            serde_json::json!({"themeVariables": {"textColor": "#fedcba"}})
        } else {
            serde_json::json!({})
        };
        let styles = ThemeRuleSet::default().with_rule(ThemeRule::new(
            ThemeTarget::Title,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#987654").unwrap())
                .with_stroke(CanvasPaint::solid("#123456").unwrap()),
        ));
        let rendered = render_journey_rules_with_requirement(
            &source,
            styles.clone(),
            config.clone(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let title = document
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some("Release"))
            .unwrap();
        assert_eq!(
            title.attribute("fill"),
            Some(if channel == "none" {
                "#987654"
            } else if channel.starts_with("inherited-") {
                ""
            } else {
                "#fedcba"
            })
        );
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 1);
        let strict = render_journey_rules_with_requirement(
            &source,
            styles,
            config,
            ThemePortabilityRequirement::RequirePortable,
        );
        assert!(matches!(
            strict,
            Err(merman_render::Error::UnverifiedFamilyTheme { .. })
        ));
    }
}

#[test]
fn journey_title_selectors_account_visibility_and_property_winners() {
    for (variant, ordinal, shadowed, residual) in [
        (None, Some(OrdinalSelector::Exact(1)), false, true),
        (None, Some(OrdinalSelector::Exact(2)), false, false),
        (Some(ThemeVariant::Error), None, false, false),
        (None, Some(OrdinalSelector::Exact(1)), true, false),
    ] {
        let mut rule = ThemeRule::new(
            ThemeTarget::Title,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        );
        if let Some(variant) = variant {
            rule = rule.with_variant(variant);
        }
        if let Some(ordinal) = ordinal {
            rule = rule.with_ordinal(ordinal);
        }
        let mut styles = ThemeRuleSet::default().with_rule(rule);
        if shadowed {
            styles = styles.with_rule(ThemeRule::new(
                ThemeTarget::Title,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#987654").unwrap()),
            ));
        }
        let rendered = render_journey_rules_with_requirement(
            "journey\n  title Release\n  section Delivery\n  Ship: 5: Actor\n",
            styles,
            serde_json::json!({}),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let title = document
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some("Release"))
            .unwrap();
        assert_eq!(
            title.attribute("fill"),
            Some(if shadowed { "#987654" } else { "" })
        );
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1 + usize::from(shadowed));
        assert_eq!(evidence.accounted_count(), evidence.required_count());
        assert_eq!(evidence.applied_count(), usize::from(shadowed));
        assert_eq!(evidence.not_applicable_count(), usize::from(!residual));
        assert_eq!(evidence.theme_residual_count(), usize::from(residual));
    }
}

#[test]
fn journey_all_presets_consume_title_fill_without_unsupported_title_diagnostics() {
    let presets = merman_render::diagram_theme::theme_preset_descriptors();
    assert_eq!(presets.len(), 11);
    for preset in presets {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(preset.preset())
            .unwrap();
        let rendered = render_journey_with_theme_requirement(
            "journey\n  title Release\n  section Delivery\n  Ship: 5: Actor\n",
            &theme,
            serde_json::json!({}),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let title = document
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some("Release"))
            .unwrap();
        assert!(
            !title.attribute("fill").unwrap().is_empty(),
            "{}",
            preset.id()
        );
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert!(evidence.applied_count() > 0, "{}", preset.id());
        assert_eq!(
            evidence.accounted_count(),
            evidence.required_count(),
            "{}",
            preset.id()
        );
        assert!(
            completion
                .report()
                .theme_diagnostics()
                .iter()
                .all(|diagnostic| {
                    diagnostic.target() != Some(ThemeTarget::Title)
                        && diagnostic.code() != "evidence-incomplete"
                }),
            "{}",
            preset.id()
        );
    }
}

#[test]
fn journey_text_fill_preserves_unsectioned_tasks_and_empty_diagram_lines() {
    for (source, tasks) in [
        ("journey\n", 0),
        ("journey\n  title Release\n", 0),
        ("journey\n  Unsectioned: 5: Actor\n", 1),
        (
            "journey\n  Unsectioned: 5: Actor\n  section Delivery\n  Named: 3: Actor\n",
            2,
        ),
    ] {
        let styles = ThemeRuleSet::default().with_rule(ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        ));
        let rendered = render_journey_text_rules(source, styles, serde_json::json!({}))
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG");
        let task_count = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("text")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "task")
                    })
            })
            .count();
        assert_eq!(task_count, tasks, "{source}");
        let lines = document
            .descendants()
            .filter(|node| node.has_tag_name("line") && node.attribute("class") != Some("mouth"))
            .count();
        assert_eq!(lines, tasks + 1, "task lines and persistent activity line");
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 1);
    }
}

#[test]
fn journey_text_fill_does_not_certify_unsupported_siblings() {
    for config in [
        serde_json::json!({}),
        serde_json::json!({"themeVariables": {"textColor": "#abcdef"}}),
    ] {
        for font_sibling in [false, true] {
            let mut patch =
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
            if font_sibling {
                patch.typography.font_size_px = Specified::Value(40.0);
            } else {
                patch = patch.with_stroke(CanvasPaint::solid("#654321").unwrap());
            }
            let styles =
                ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Text, patch));
            let error = render_journey_text_rules(
                "journey\n  section Delivery\n  Ship: 5: Actor\n",
                styles,
                config.clone(),
            )
            .err()
            .expect("typed fill must not certify an unsupported sibling");
            assert!(
                matches!(error, merman_render::Error::UnverifiedFamilyTheme { .. }),
                "{error}"
            );
        }
    }
}

#[test]
fn journey_text_fill_config_owner_and_shadowed_palette_are_accounted() {
    for config_owned in [false, true] {
        let styles = ThemeRuleSet::default()
            .with_ordinal_palette(
                ThemeTarget::Text,
                OrdinalPalette::new([merman_render::diagram_theme::ThemeColorValue::parse(
                    "#fedcba",
                )
                .unwrap()])
                .unwrap(),
            )
            .with_rule(ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            ));
        let config = if config_owned {
            serde_json::json!({"themeVariables": {"textColor": "#abcdef"}})
        } else {
            serde_json::json!({})
        };
        let rendered = render_journey_text_rules(
            "journey\n  section Delivery\n  Ship: 5: Actor\n",
            styles,
            config,
        )
        .expect("source owner and inactive palette are portable");
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.accounted_count(), 2);
        assert_eq!(evidence.applied_count(), usize::from(!config_owned));
        assert_eq!(
            evidence.not_applicable_count(),
            1 + usize::from(config_owned)
        );
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn journey_text_paint_preserves_labels_lines_and_title_local_ownership() {
    for variant in [None, Some(ThemeVariant::Default)] {
        for (paint, expected_paint) in [("#123456", "#123456"), ("transparent", "#00000000")] {
            let mut rule = ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid(paint).expect("valid text paint")),
            );
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            let theme = DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(rule).with_rule(
                        ThemeRule::new(
                            ThemeTarget::Title,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#987654").expect("valid title paint"),
                            ),
                        ),
                    ),
                ))
                .expect("compile Journey text paint");
            for (config, expected_text, expected_title) in [
                (serde_json::json!({}), expected_paint, "#987654"),
                (
                    serde_json::json!({"themeVariables": {"textColor": "#abcdef"}}),
                    "#abcdef",
                    "",
                ),
                (
                    serde_json::json!({"journey": {"titleColor": "#fedcba"}}),
                    expected_paint,
                    "#fedcba",
                ),
            ] {
                let parsed = common::install_parse_compatibility(
                    &theme,
                    MermaidEngine::new().with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(config)),
                )
                .parse_diagram_for_render_model_sync(
                    "journey\n  title Release\n  section Delivery\n    Ship release: 5: Maintainer\n",
                    ParseOptions::strict(),
                )
                .expect("parse Journey text paint fixture")
                .expect("detect Journey").into_presentation();
                let session = RenderEnvironment::deterministic()
                    .begin_session_with_theme(&theme)
                    .expect("begin Journey session");
                let artifact =
                    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
                        .expect("prepare Journey text paint fixture");
                let rendered = artifact
                    .render_svg(
                        &SvgRenderOptions {
                            diagram_id: Some("journey-text-paint".to_owned()),
                            ..SvgRenderOptions::default()
                        },
                        &SvgDebugOptions::default(),
                    )
                    .expect("render Journey text paint fixture");
                let document =
                    roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG");
                let css = document
                    .descendants()
                    .find(|node| node.has_tag_name("style"))
                    .and_then(|node| node.text())
                    .expect("Journey stylesheet");
                let root_rule = css.split_once('}').expect("complete root CSS rule").0;
                assert!(root_rule.starts_with("#journey-text-paint{"));
                assert!(
                    root_rule.ends_with(&format!(";fill:{expected_text};")),
                    "root rule: {root_rule}"
                );
                assert!(css.contains(&format!(
                    "#journey-text-paint line{{stroke:{expected_text};}}"
                )));
                assert!(css.contains(&format!(
                    "#journey-text-paint .legend{{fill:{expected_text};"
                )));
                assert!(css.contains(&format!(
                    "#journey-text-paint .label{{color:{expected_text};}}"
                )));
                // Title paint belongs to its visible text terminal, not the legacy cluster rule.
                assert!(!css.contains("#987654"));
                assert!(!document.descendants().any(|node| {
                    node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "cluster")
                    })
                }));

                for (class, expected_label) in [
                    ("task", "    Ship release"),
                    ("journey-section", "Delivery"),
                ] {
                    let text = document
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("text")
                                && node.attribute("class").is_some_and(|classes| {
                                    classes.split_whitespace().any(|item| item == class)
                                })
                        })
                        .expect("native Journey text terminal");
                    assert_eq!(
                        text.descendants()
                            .find(|node| node.is_text())
                            .and_then(|node| node.text()),
                        Some(expected_label)
                    );
                    assert!(
                        text.attribute("style")
                            .expect("native text style")
                            .ends_with(&format!("fill: {expected_text};"))
                    );
                    let html = document
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("div")
                                && node.attribute("class").is_some_and(|classes| {
                                    classes.split_whitespace().any(|item| item == class)
                                })
                        })
                        .expect("HTML Journey text terminal");
                    let label = html
                        .children()
                        .find(|node| node.attribute("class") == Some("label"))
                        .expect("HTML label uses the color rule");
                    assert_eq!(label.text(), Some(expected_label));
                }
                let legend = document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("text") && node.attribute("class") == Some("legend")
                    })
                    .expect("actor legend terminal");
                assert_eq!(
                    legend
                        .descendants()
                        .find(|node| node.is_text())
                        .and_then(|node| node.text()),
                    Some("Maintainer")
                );
                let title = document
                    .descendants()
                    .find(|node| node.has_tag_name("text") && node.text() == Some("Release"))
                    .expect("diagram title terminal");
                assert_eq!(title.attribute("fill"), Some(expected_title));
                assert_eq!(
                    document
                        .descendants()
                        .filter(|node| node.has_tag_name("line"))
                        .count(),
                    2,
                    "task and activity lines consume the line rule"
                );
            }
        }
    }
}

#[test]
fn journey_task_radius_reaches_terminal_svg_without_changing_sections() {
    for (radius, expected_task_radius) in [(Specified::Value(9.0), "9"), (Specified::Clear, "3")] {
        let styles = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(
                ThemeTarget::JourneyTask,
                ThemeStylePatch {
                    geometry: ThemeGeometryPatch { radius },
                    ..ThemeStylePatch::default()
                },
            ))
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::JourneyTask,
                    ThemeStylePatch {
                        geometry: ThemeGeometryPatch {
                            radius: Specified::Value(99.0),
                        },
                        ..ThemeStylePatch::default()
                    },
                )
                .with_variant(ThemeVariant::Warning)
                .with_ordinal(
                    OrdinalSelector::exact(999).expect("valid out-of-range Journey ordinal"),
                ),
            );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .expect("compile Journey task radius theme");
        let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
            .parse_diagram_for_render_model_sync(
                "journey\n  section Delivery\n    Ship release: 5: Maintainer\n",
                ParseOptions::strict(),
            )
            .expect("parse themed Journey")
            .expect("detect themed Journey")
            .into_presentation();
        let session = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable Journey session");
        let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .expect("prepare themed Journey");
        let svg = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("journey-task-radius".to_string()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render themed Journey SVG")
            .svg()
            .to_owned();
        let document = roxmltree::Document::parse(&svg).expect("valid themed Journey SVG XML");
        let task = document
            .descendants()
            .find(|node| {
                node.has_tag_name("rect")
                    && node.attribute("class").is_some_and(|class| {
                        class.split_ascii_whitespace().any(|token| token == "task")
                    })
            })
            .expect("Journey task rect");
        let section = document
            .descendants()
            .find(|node| {
                node.has_tag_name("rect")
                    && node.attribute("class").is_some_and(|class| {
                        class
                            .split_ascii_whitespace()
                            .any(|token| token == "journey-section")
                    })
            })
            .expect("Journey section rect");

        assert_eq!(task.attribute("rx"), Some(expected_task_radius));
        assert_eq!(task.attribute("ry"), Some(expected_task_radius));
        assert_eq!(section.attribute("rx"), Some("3"));
        assert_eq!(section.attribute("ry"), Some("3"));
    }
}

#[test]
fn journey_task_static_paint_reaches_terminal_rect_and_is_fully_accounted() {
    let theme = journey_task_paint_theme();
    let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
        .parse_diagram_for_render_model_sync(
            "journey\n  section Delivery\n    Ship release: 5: Maintainer\n",
            ParseOptions::strict(),
        )
        .expect("parse themed Journey")
        .expect("detect themed Journey")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable Journey session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed Journey");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("journey-task-paint".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Journey SVG");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG XML");
    let task = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class.split_ascii_whitespace().any(|token| token == "task")
                })
        })
        .expect("Journey task rect");
    let style = task.attribute("style").expect("typed task paint style");
    assert!(style.contains("fill:#123456;"), "task style: {style}");
    assert!(style.contains("stroke:#654321;"), "task style: {style}");

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn journey_task_explicit_default_paint_reaches_terminal_rect_and_is_fully_accounted() {
    for (fill, stroke, expected_fill, expected_stroke) in [
        (
            CanvasPaint::solid("#123456").expect("valid Journey task fill"),
            CanvasPaint::solid("#654321").expect("valid Journey task stroke"),
            "#123456",
            "#654321",
        ),
        (
            CanvasPaint::Transparent,
            CanvasPaint::Transparent,
            "transparent",
            "transparent",
        ),
    ] {
        let theme =
            journey_task_paint_theme_with_variant(Some(ThemeVariant::Default), fill, stroke);
        let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
            .parse_diagram_for_render_model_sync(
                "journey\n  section Delivery\n    Ship release: 5: Maintainer\n",
                ParseOptions::strict(),
            )
            .expect("parse explicitly default-themed Journey")
            .expect("detect explicitly default-themed Journey")
            .into_presentation();
        let session = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable Journey session");
        let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .expect("prepare explicitly default-themed Journey");
        let rendered = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("journey-task-default-paint".to_string()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render explicitly default-themed Journey SVG");
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG XML");
        let task = document
            .descendants()
            .find(|node| {
                node.has_tag_name("rect")
                    && node.attribute("class").is_some_and(|class| {
                        class.split_ascii_whitespace().any(|token| token == "task")
                    })
            })
            .expect("Journey task rect");
        let style = task.attribute("style").expect("typed task paint style");
        assert!(
            style.contains(&format!("fill:{expected_fill};")),
            "task style: {style}"
        );
        assert!(
            style.contains(&format!("stroke:{expected_stroke};")),
            "task style: {style}"
        );

        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.accounted_count(), 2);
        assert_eq!(evidence.applied_count(), 2);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn journey_task_explicit_default_paint_is_not_applicable_without_tasks() {
    let theme = journey_task_paint_theme_with_variant(
        Some(ThemeVariant::Default),
        CanvasPaint::solid("#123456").expect("valid Journey task fill"),
        CanvasPaint::solid("#654321").expect("valid Journey task stroke"),
    );
    let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
        .parse_diagram_for_render_model_sync("journey\n", ParseOptions::strict())
        .expect("parse empty Journey")
        .expect("detect empty Journey")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable empty Journey session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare empty Journey");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("journey-task-default-paint-empty".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render empty Journey SVG");
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn journey_base_typography_drives_scoped_css_and_is_fully_accounted() {
    let theme = journey_typography_theme(21.0);
    let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
        .parse_diagram_for_render_model_sync(
            "journey\n  section Delivery\n    Ship release: 5: Maintainer\n",
            ParseOptions::strict(),
        )
        .expect("parse themed Journey")
        .expect("detect themed Journey")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable Journey session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed Journey");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("journey-base-typography".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Journey SVG");

    let svg = rendered.svg();
    assert!(
        svg.contains("#journey-base-typography .legend"),
        "Journey actor legend selector missing from SVG CSS"
    );
    assert!(
        svg.contains("font-family:\"Journey Typed\""),
        "typed Journey font family missing from SVG CSS: {svg}"
    );
    assert!(
        svg.contains("font-size:21px"),
        "typed Journey font size missing from SVG CSS: {svg}"
    );

    let document = roxmltree::Document::parse(svg).expect("valid Journey SVG");
    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("text")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|token| token == "legend")
                })
        }),
        "typed Journey actor legend terminal missing"
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn journey_explicit_theme_font_size_owns_only_the_size_route() {
    let theme = journey_typography_theme(21.0);
    let parsed = common::install_parse_compatibility(
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "themeVariables": { "fontSize": "24px" }
            })),
        ),
    )
    .parse_diagram_for_render_model_sync(
        "journey\n  section Delivery\n    Ship release: 5: Maintainer\n",
        ParseOptions::strict(),
    )
    .expect("parse configured Journey")
    .expect("detect configured Journey")
    .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable Journey session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare configured Journey");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("journey-configured-font-size".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render configured Journey SVG");

    assert!(rendered.svg().contains("font-family:\"Journey Typed\""));
    assert!(rendered.svg().contains("font-size:24px"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn journey_task_palette_reaches_section_and_task_terminals_without_legacy_projection() {
    let palette = OrdinalPalette::new([
        merman_render::diagram_theme::ThemeColorValue::parse("#123456")
            .expect("valid first Journey palette color"),
        merman_render::diagram_theme::ThemeColorValue::parse("#654321")
            .expect("valid second Journey palette color"),
    ])
    .expect("valid Journey task palette");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::JourneyTask, palette),
        ))
        .expect("compile Journey task palette theme");
    let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
        .parse_diagram_for_render_model_sync(
            "journey\n  section Delivery\n    Ship release: 5: Maintainer\n  section Verify\n    Verify release: 4: Maintainer\n",
            ParseOptions::strict(),
        )
        .expect("parse themed Journey")
        .expect("detect themed Journey").into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable Journey session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed Journey");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("journey-task-palette".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Journey SVG");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG XML");
    let mut sections = document.descendants().filter(|node| {
        node.has_tag_name("rect")
            && node.attribute("class").is_some_and(|class| {
                class
                    .split_ascii_whitespace()
                    .any(|token| token == "journey-section")
            })
    });
    let mut tasks = document.descendants().filter(|node| {
        node.has_tag_name("rect")
            && node
                .attribute("class")
                .is_some_and(|class| class.split_ascii_whitespace().any(|token| token == "task"))
    });
    for expected in ["#123456", "#654321"] {
        let section = sections.next().expect("Journey section rect");
        let task = tasks.next().expect("Journey task rect");
        let expected_style = format!("fill:{expected};");
        assert_eq!(section.attribute("style"), Some(expected_style.as_str()));
        assert_eq!(
            task.attribute("style")
                .map(|style| style.split("stroke:").next().unwrap_or(style)),
            Some(expected_style.as_str())
        );
    }

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn journey_task_palette_preserves_source_owned_slots_and_counts_typed_slots() {
    let palette = OrdinalPalette::new([
        merman_render::diagram_theme::ThemeColorValue::parse("#123456")
            .expect("valid first Journey palette color"),
        merman_render::diagram_theme::ThemeColorValue::parse("#654321")
            .expect("valid second Journey palette color"),
    ])
    .expect("valid Journey task palette");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::JourneyTask, palette),
        ))
        .expect("compile Journey task palette theme");
    let parsed = common::install_parse_compatibility(
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "secure": []
            })),
        ),
    )
    .parse_diagram_for_render_model_sync(
        r##"%%{init: {"themeVariables": {"fillType0": "#f59e0b"}}}%%
journey
  section Delivery
    Ship release: 5: Maintainer
  section Verify
    Verify release: 4: Maintainer
"##,
        ParseOptions::strict(),
    )
    .expect("parse themed Journey")
    .expect("detect themed Journey")
    .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable Journey session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed Journey");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("journey-task-palette-source".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Journey SVG");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG XML");
    let section_fills = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|token| token == "journey-section")
                })
        })
        .map(|node| node.attribute("style").unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    let task_fills = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class.split_ascii_whitespace().any(|token| token == "task")
                })
        })
        .map(|node| node.attribute("style").unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    assert_eq!(section_fills, ["fill:#f59e0b;", "fill:#654321;"]);
    assert_eq!(task_fills, ["fill:#f59e0b;", "fill:#654321;"]);
    assert!(
        !rendered
            .svg()
            .contains("#journey-task-palette-source .task-type-0")
            && !rendered
                .svg()
                .contains("#journey-task-palette-source .section-type-0")
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn journey_task_transparent_palette_reaches_both_terminals() {
    let palette = OrdinalPalette::new([
        merman_render::diagram_theme::ThemeColorValue::parse("transparent")
            .expect("valid transparent Journey palette color"),
        merman_render::diagram_theme::ThemeColorValue::parse("#654321")
            .expect("valid solid Journey palette color"),
    ])
    .expect("valid Journey task palette");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::JourneyTask, palette),
        ))
        .expect("compile Journey task palette theme");
    let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
        .parse_diagram_for_render_model_sync(
            "journey\n  section Delivery\n    Ship release: 5: Maintainer\n  section Verify\n    Verify release: 4: Maintainer\n",
            ParseOptions::strict(),
        )
        .expect("parse themed Journey")
        .expect("detect themed Journey").into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable Journey session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed Journey");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("journey-task-palette-transparent".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Journey SVG");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG XML");
    let section = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|token| token == "journey-section")
                })
        })
        .expect("first Journey section rect");
    let task = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class.split_ascii_whitespace().any(|token| token == "task")
                })
        })
        .expect("first Journey task rect");
    assert_eq!(section.attribute("style"), Some("fill:#00000000;"));
    assert_eq!(
        task.attribute("style")
            .and_then(|style| style.split("stroke:").next()),
        Some("fill:#00000000;")
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn journey_task_theme_receipts_only_require_actual_section_headers() {
    for named in [false, true] {
        for paint in ["radius", "paint", "palette"] {
            let mut styles = ThemeRuleSet::default();
            if paint == "radius" {
                styles = styles.with_rule(ThemeRule::new(
                    ThemeTarget::JourneyTask,
                    ThemeStylePatch {
                        geometry: ThemeGeometryPatch {
                            radius: Specified::Value(9.0),
                        },
                        ..Default::default()
                    },
                ));
            } else if paint == "paint" {
                styles = styles.with_rule(ThemeRule::new(
                    ThemeTarget::JourneyTask,
                    ThemeStylePatch::default()
                        .with_fill(CanvasPaint::solid("#123456").unwrap())
                        .with_stroke(CanvasPaint::solid("#654321").unwrap()),
                ));
            } else {
                let palette = OrdinalPalette::new([
                    merman_render::diagram_theme::ThemeColorValue::parse("#123456").unwrap(),
                    merman_render::diagram_theme::ThemeColorValue::parse("#654321").unwrap(),
                ])
                .unwrap();
                styles = styles.with_ordinal_palette(ThemeTarget::JourneyTask, palette);
            }
            let theme = DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new().with_styles(styles))
                .unwrap();
            let source = if named {
                "journey\n  Unsectioned: 5: Actor\n  section Delivery\n    Named: 4: Actor\n"
            } else {
                "journey\n  Unsectioned: 5: Actor\n"
            };
            let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let session = RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .unwrap();
            let rendered =
                family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
                    .unwrap()
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .expect("valid unsectioned tasks must complete their actual receipts");
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let tasks = document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("rect")
                        && node.attribute("class").is_some_and(|classes| {
                            classes.split_whitespace().any(|class| class == "task")
                        })
                })
                .collect::<Vec<_>>();
            let sections = document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("rect")
                        && node.attribute("class").is_some_and(|classes| {
                            classes
                                .split_whitespace()
                                .any(|class| class == "journey-section")
                        })
                })
                .collect::<Vec<_>>();
            assert_eq!(tasks.len(), if named { 2 } else { 1 });
            assert_eq!(sections.len(), usize::from(named));
            for task in tasks {
                if paint == "radius" {
                    assert_eq!(task.attribute("rx"), Some("9"));
                    assert_eq!(task.attribute("ry"), Some("9"));
                } else if paint == "paint" {
                    assert_eq!(
                        task.attribute("style"),
                        Some("fill:#123456;stroke:#654321;")
                    );
                } else {
                    assert_eq!(task.attribute("style"), Some("fill:#123456;"));
                }
            }
            if paint == "palette" && named {
                assert_eq!(sections[0].attribute("style"), Some("fill:#123456;"));
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[allow(dead_code)]
mod common;
