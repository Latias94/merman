#![cfg(feature = "diagram-gantt")]

mod common;

use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_model::presentation::PresentationConfig;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, OrdinalSelector,
    ThemePortabilityRequirement, ThemePreset, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    ThemeTarget, ThemeVariant,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_theme_contract::{ThemeDesignTokenValueV1, ThemeRecipeV1, ThemeValueV1};
use roxmltree::{Document, Node};
use serde_json::{Value, json};

const ID: &str = "native-gantt-background";
const SOURCE: &str = concat!(
    "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nexcludes weekends\n",
    "section Zero\nAn outside label crossing excluded weekends: outside0, 2024-01-05, 2024-01-06\n",
    "Timeline: span, 2024-01-01, 2024-04-10\n",
    "section One\nAn outside label crossing excluded weekends: outside1, 2024-01-05, 2024-01-06\n",
    "section Two\nAn outside label crossing excluded weekends: outside2, 2024-01-05, 2024-01-06\n",
    "section Three\nAn outside label crossing excluded weekends: outside3, 2024-01-05, 2024-01-06\n",
);
const SINGLE_EXCLUSION_SOURCE: &str = concat!(
    "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nexcludes weekends\n",
    "section Work\nSpan: task, 2024-01-05, 2024-01-09\n",
);
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
fn render(
    source: &str,
    theme: Option<&DiagramTheme>,
    config: Value,
) -> (family::RenderedFamilySvg, PresentationConfig) {
    render_with_requirement(
        source,
        theme,
        config,
        ThemePortabilityRequirement::BestEffort,
    )
}

fn render_with_requirement(
    source: &str,
    theme: Option<&DiagramTheme>,
    config: Value,
    requirement: ThemePortabilityRequirement,
) -> (family::RenderedFamilySvg, PresentationConfig) {
    try_render_with_requirement(source, theme, config, requirement).unwrap()
}

fn try_render_with_requirement(
    source: &str,
    theme: Option<&DiagramTheme>,
    config: Value,
    requirement: ThemePortabilityRequirement,
) -> merman_render::Result<(family::RenderedFamilySvg, PresentationConfig)> {
    let engine =
        MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(config));
    let engine = match theme {
        Some(theme) => common::install_parse_compatibility(theme, engine),
        None => engine,
    };
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let config = parsed.metadata().effective_config.clone();
    let environment =
        RenderEnvironment::deterministic().with_theme_portability_requirement(requirement);
    let session = match theme {
        Some(theme) => environment.begin_session_with_theme(theme),
        None => environment.begin_session(),
    }
    .unwrap();
    let rendered = family::prepare(
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
    )?;
    Ok((rendered, config))
}

fn has_class(node: Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|value| value == class))
}

fn declaration<'a>(style: &'a str, property: &str) -> Option<&'a str> {
    style
        .split(';')
        .filter_map(|entry| entry.split_once(':'))
        .filter(|(name, _)| name.trim() == property)
        .map(|(_, value)| value.trim().trim_end_matches("!important").trim())
        .next_back()
}

// Only the exact Gantt selectors under test are read, with separate live-DOM gates below.
fn css(document: &Document<'_>, selector: &str, property: &str) -> Option<String> {
    let selector = format!("#{ID} {selector}");
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

fn outside_label<'doc, 'input>(
    document: &'doc Document<'input>,
    section: usize,
) -> Node<'doc, 'input> {
    let suffix = format!("-outside{section}-text");
    let label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("id").is_some_and(|id| id.ends_with(&suffix))
        })
        .expect("live outside task label");
    assert!(has_class(label, "taskTextOutsideRight"));
    assert!(has_class(label, &format!("taskTextOutside{section}")));
    label
}

fn outside_ink(document: &Document<'_>, section: usize) -> String {
    outside_label(document, section)
        .attribute("style")
        .and_then(|style| declaration(style, "fill"))
        .map(str::to_owned)
        .or_else(|| css(document, &format!(".taskTextOutside{section}"), "fill"))
        .expect("outside label fill")
}

fn opaque(css: &str) -> [f64; 3] {
    let color = merman_markup::theme_color::ThemeColor::parse(css)
        .unwrap()
        .rgba_channels();
    assert_eq!(color.alpha, 1.0, "opaque paint: {css}");
    [color.red, color.green, color.blue]
}

fn composite(fill: &str, opacity: f64, background: [f64; 3]) -> [f64; 3] {
    let color = merman_markup::theme_color::ThemeColor::parse(fill)
        .unwrap()
        .rgba_channels();
    let channels = [color.red, color.green, color.blue];
    let alpha = color.alpha * opacity;
    std::array::from_fn(|i| channels[i] * alpha + background[i] * (1.0 - alpha))
}

fn contrast(first: [f64; 3], second: [f64; 3]) -> f64 {
    let luminance = |rgb: [f64; 3]| {
        let linear = rgb.map(|channel| {
            let value = channel / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        });
        0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]
    };
    let first = luminance(first);
    let second = luminance(second);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

#[test]
fn native_presets_keep_excluded_periods_distinct_and_ink_readable_under_every_section_overlay() {
    for preset in PRESETS {
        let theme = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
        let (rendered, _) = render(SOURCE, Some(&theme), json!({}));
        let document = Document::parse(rendered.svg()).unwrap();
        assert!(
            document
                .descendants()
                .any(|node| node.has_tag_name("g") && has_class(node, "tick"))
        );
        let canvas = document
            .descendants()
            .find(|node| node.attribute("data-merman-theme-canvas") == Some("base"))
            .expect("actual preset canvas");
        let canvas = opaque(canvas.attribute("fill").unwrap());
        let exclude = opaque(&css(&document, ".exclude-range", "fill").unwrap());
        let excluded = document
            .descendants()
            .find(|node| node.has_tag_name("rect") && has_class(*node, "exclude-range"))
            .expect("live excluded weekend");
        for section in 0..4 {
            let selector = format!(".section{section}");
            let row = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("rect")
                        && has_class(*node, "section")
                        && has_class(*node, &format!("section{section}"))
                })
                .expect("live section row");
            assert!(
                excluded.range().start < row.range().start,
                "sections overlay excludes"
            );
            let title_selector = format!(".sectionTitle{section}");
            assert!(document.descendants().any(|node| {
                node.has_tag_name("text") && has_class(node, &format!("sectionTitle{section}"))
            }));
            let fill = css(&document, &selector, "fill").unwrap();
            let opacity = css(&document, &selector, "opacity")
                .or_else(|| css(&document, ".section", "opacity"))
                .unwrap()
                .parse::<f64>()
                .unwrap();
            assert_eq!(opacity, 0.2);
            let normal_row = composite(&fill, opacity, canvas);
            let excluded_row = composite(&fill, opacity, exclude);
            for ink in [
                outside_ink(&document, section),
                css(&document, &title_selector, "fill").unwrap(),
            ] {
                for background in [exclude, excluded_row] {
                    let ratio = contrast(opaque(&ink), background);
                    assert!(
                        ratio >= 4.5,
                        "{preset:?} section{section}: {ink} on {background:?}, {ratio:.2}:1"
                    );
                }
            }
            if preset.is_dark() {
                // Guard against hiding exclusions when native surface and canvas are equal.
                let difference = contrast(normal_row, excluded_row);
                assert!(
                    difference >= 1.2,
                    "{preset:?} section{section}: exclusions disappeared ({difference:.2}:1)"
                );
            }
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(
            evidence.accounted_count(),
            evidence.required_count(),
            "{preset:?}: {evidence:?}"
        );
    }
}

#[test]
fn native_dark_presets_keep_every_section_composite_readable() {
    for (preset, fallback, exclude) in [
        (ThemePreset::Cyberpunk, None, "#253139"),
        (ThemePreset::EditorDark, Some("dark"), "#334155"),
        (ThemePreset::OneDark, Some("dark"), "#11151c"),
    ] {
        let theme = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
        let authored = theme.compatibility_recipe().compatibility_config();
        assert_eq!(authored.get_str("theme"), fallback);
        for key in ["look", "layout", "flowchart.defaultRenderer"] {
            assert_eq!(authored.get_str(key), None);
        }
        let (rendered, config) = render(SOURCE, Some(&theme), json!({}));
        assert_eq!(config.get_str("theme"), Some(fallback.unwrap_or("default")));
        let document = Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            css(&document, ".exclude-range", "fill").as_deref(),
            Some(exclude)
        );
        let canvas = document
            .descendants()
            .find(|node| node.attribute("data-merman-theme-canvas") == Some("base"))
            .unwrap();
        let canvas = opaque(canvas.attribute("fill").unwrap());
        for section in 0..4 {
            let selector = format!(".section{section}");
            let fill = css(&document, &selector, "fill").unwrap();
            let opacity = css(&document, &selector, "opacity")
                .or_else(|| css(&document, ".section", "opacity"))
                .unwrap()
                .parse::<f64>()
                .unwrap();
            for ink in [
                outside_ink(&document, section),
                css(&document, &format!(".sectionTitle{section}"), "fill").unwrap(),
            ] {
                for background in [
                    canvas,
                    opaque(exclude),
                    composite(&fill, opacity, canvas),
                    composite(&fill, opacity, opaque(exclude)),
                ] {
                    let ratio = contrast(opaque(&ink), background);
                    assert!(
                        ratio >= 4.5,
                        "{preset:?} section{section}: {ink} on {background:?}, {ratio:.2}:1"
                    );
                }
            }
        }
    }
}

#[test]
fn source_background_and_outside_text_overrides_reach_actual_terminals() {
    let overrides = json!({"theme": "forest", "themeVariables": {
        "excludeBkgColor": "#26384a", "sectionBkgColor": "rgba(40, 60, 80, 0.5)",
        "sectionBkgColor2": "#152535", "altSectionBkgColor": "#354555",
        "gridColor": "#657585", "taskTextOutsideColor": "#abcdef"
    }});
    for preset in [ThemePreset::Cyberpunk, ThemePreset::EditorDark] {
        let theme = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
        for frontmatter in [false, true] {
            let (source, site) = if frontmatter {
                (
                    format!("---\nconfig: {overrides}\n---\n{SOURCE}"),
                    json!({}),
                )
            } else {
                (SOURCE.to_owned(), overrides.clone())
            };
            let (rendered, config) = render(&source, Some(&theme), site);
            assert_eq!(config.get_str("theme"), Some("forest"));
            let document = Document::parse(rendered.svg()).unwrap();
            for (key, selector, property) in [
                ("excludeBkgColor", ".exclude-range", "fill"),
                ("sectionBkgColor", ".section0", "fill"),
                ("sectionBkgColor2", ".section2", "fill"),
                ("altSectionBkgColor", ".section1", "fill"),
                ("gridColor", ".grid .tick", "stroke"),
            ] {
                let expected = overrides["themeVariables"][key].as_str().unwrap();
                assert_eq!(
                    config.get_str(&format!("themeVariables.{key}")),
                    Some(expected)
                );
                assert_eq!(
                    css(&document, selector, property).as_deref(),
                    Some(expected)
                );
            }
            assert_eq!(
                css(&document, ".section3", "fill").as_deref(),
                Some("#354555")
            );
            for section in 0..4 {
                assert!(
                    outside_label(&document, section)
                        .attribute("style")
                        .is_none()
                );
                assert_eq!(outside_ink(&document, section), "#abcdef");
            }
        }
    }
}

#[test]
fn exported_excluded_period_token_is_independent_and_edits_reach_the_terminal() {
    let compiler = DiagramThemeCompiler::new();
    let ThemeRecipeV1::Design { mut design } =
        compiler.export_preset(ThemePreset::Cyberpunk).unwrap()
    else {
        panic!("editable preset design");
    };
    for (token, value) in [("canvas", "#000710"), ("neon-cyan", "#c7fdff")] {
        design.tokens.get_mut(token).unwrap().value =
            ThemeDesignTokenValueV1::Color(ThemeValueV1::Literal(value.into()));
    }
    for edited_token in [false, true] {
        if edited_token {
            design
                .tokens
                .get_mut("gantt-excluded-period")
                .unwrap()
                .value = ThemeDesignTokenValueV1::Color(ThemeValueV1::Literal("#1d3049".into()));
        }
        let theme = compiler
            .compile_recipe(ThemeRecipeV1::Design {
                design: design.clone(),
            })
            .unwrap();
        assert!(
            theme
                .compatibility_recipe()
                .compatibility_config()
                .as_value()
                .get("themeVariables")
                .is_none()
        );
        let (rendered, _) = render(SOURCE, Some(&theme), json!({}));
        let document = Document::parse(rendered.svg()).unwrap();
        assert_eq!(outside_ink(&document, 0), "#EAF4F0");
        let canvas = document
            .descendants()
            .find(|node| node.attribute("data-merman-theme-canvas") == Some("base"))
            .unwrap();
        assert_eq!(canvas.attribute("fill"), Some("#000710"));
        let expected = if edited_token { "#1d3049" } else { "#253139" };
        assert_eq!(
            css(&document, ".exclude-range", "fill").as_deref(),
            Some(expected)
        );
    }
}

fn excluded_period_theme(fill: CanvasPaint, variant: Option<ThemeVariant>) -> DiagramTheme {
    let rule = ThemeRule::new(
        ThemeTarget::ExcludedPeriod,
        ThemeStylePatch::default().with_fill(fill),
    );
    let rule = match variant {
        Some(variant) => rule.with_variant(variant),
        None => rule,
    };
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
        .unwrap()
}

#[test]
fn static_solid_and_transparent_excluded_periods_have_portable_terminal_evidence() {
    for (paint, expected) in [
        (CanvasPaint::solid("#14324a").unwrap(), "#14324a"),
        (CanvasPaint::Transparent, "transparent"),
    ] {
        for variant in [None, Some(ThemeVariant::Default)] {
            let theme = excluded_period_theme(paint.clone(), variant);
            let (rendered, _) = render_with_requirement(
                SOURCE,
                Some(&theme),
                json!({}),
                ThemePortabilityRequirement::RequirePortable,
            );
            let document = Document::parse(rendered.svg()).unwrap();
            assert!(
                document
                    .descendants()
                    .any(|node| node.has_tag_name("rect") && has_class(node, "exclude-range"))
            );
            assert_eq!(
                css(&document, ".exclude-range", "fill").as_deref(),
                Some(expected)
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
            assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
        }
    }
}

#[test]
fn excluded_period_fill_receipts_follow_actual_ordinal_winners() {
    for static_last in [false, true] {
        let static_rule = ThemeRule::new(
            ThemeTarget::ExcludedPeriod,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#14324a").unwrap()),
        );
        let ordinal_rule = ThemeRule::new(
            ThemeTarget::ExcludedPeriod,
            ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
        )
        .with_ordinal(OrdinalSelector::exact(1).unwrap());
        let [first, second] = if static_last {
            [ordinal_rule, static_rule]
        } else {
            [static_rule, ordinal_rule]
        };
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_styles(ThemeRuleSet::default().with_rule(first).with_rule(second)),
            )
            .unwrap();
        for source in [SINGLE_EXCLUSION_SOURCE, SOURCE] {
            let single_exclusion = source == SINGLE_EXCLUSION_SOURCE;
            let static_applies = static_last || !single_exclusion;
            let (rendered, _) = render(source, Some(&theme), json!({}));
            let document = Document::parse(rendered.svg()).unwrap();
            let count = document
                .descendants()
                .filter(|node| node.has_tag_name("rect") && has_class(*node, "exclude-range"))
                .count();
            if single_exclusion {
                assert_eq!(count, 1);
            } else {
                assert!(count > 1);
            }
            assert_eq!(
                css(&document, ".exclude-range", "fill").as_deref(),
                Some(if static_applies { "#14324a" } else { "#eeeeee" })
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 2);
            assert_eq!(evidence.accounted_count(), 2);
            assert_eq!(evidence.applied_count(), usize::from(static_applies));
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(static_last || single_exclusion)
            );
            assert_eq!(evidence.theme_residual_count(), usize::from(!static_last));
            assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);

            let strict = try_render_with_requirement(
                source,
                Some(&theme),
                json!({}),
                ThemePortabilityRequirement::RequirePortable,
            );
            if static_last {
                strict.expect("the later static fill shadows the unsupported ordinal fill");
            } else {
                assert!(matches!(
                    strict,
                    Err(merman_render::Error::UnverifiedFamilyTheme {
                        family_id: merman_render::DiagramFamilyId::GANTT,
                        residual_count: 1,
                    })
                ));
            }
        }
    }
}

#[test]
fn excluded_period_mixed_facets_use_the_actual_ordinal_domain() {
    for ordinal_last in [false, true] {
        let mixed_rule = ThemeRule::new(
            ThemeTarget::ExcludedPeriod,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#14324a").unwrap())
                .with_stroke(CanvasPaint::solid("#123456").unwrap()),
        );
        let ordinal_rule = ThemeRule::new(
            ThemeTarget::ExcludedPeriod,
            ThemeStylePatch::default().with_stroke(CanvasPaint::Transparent),
        )
        .with_ordinal(OrdinalSelector::exact(1).unwrap());
        let [first, second] = if ordinal_last {
            [mixed_rule, ordinal_rule]
        } else {
            [ordinal_rule, mixed_rule]
        };
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_styles(ThemeRuleSet::default().with_rule(first).with_rule(second)),
            )
            .unwrap();
        let (rendered, _) = render(SINGLE_EXCLUSION_SOURCE, Some(&theme), json!({}));
        let document = Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            css(&document, ".exclude-range", "fill").as_deref(),
            Some("#14324a")
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.accounted_count(), 2);
        assert_eq!(evidence.applied_count(), usize::from(ordinal_last));
        assert_eq!(evidence.not_applicable_count(), usize::from(!ordinal_last));
        assert_eq!(evidence.theme_residual_count(), 1);
        assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
        assert!(matches!(
            try_render_with_requirement(
                SINGLE_EXCLUSION_SOURCE,
                Some(&theme),
                json!({}),
                ThemePortabilityRequirement::RequirePortable,
            ),
            Err(merman_render::Error::UnverifiedFamilyTheme {
                family_id: merman_render::DiagramFamilyId::GANTT,
                residual_count: 1,
            })
        ));
    }
}

#[test]
fn source_owned_exclusions_are_not_claimed_as_typed_application() {
    let theme = excluded_period_theme(CanvasPaint::solid("#14324a").unwrap(), None);
    let (rendered, _) = render(
        SOURCE,
        Some(&theme),
        json!({"themeVariables": {"excludeBkgColor": "#abcdef"}}),
    );
    let document = Document::parse(rendered.svg()).unwrap();
    assert_eq!(
        css(&document, ".exclude-range", "fill").as_deref(),
        Some("#abcdef")
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
}

#[test]
fn absent_excluded_rectangles_leave_default_bytes_and_mark_the_rule_not_applicable() {
    let theme = excluded_period_theme(CanvasPaint::solid("#14324a").unwrap(), None);
    for source in [
        SOURCE.replace("excludes weekends\n", ""),
        "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nexcludes weekends\nsection Work\nWeekdays: task, 2024-01-01, 2024-01-03\n".into(),
        "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nexcludes weekends\nsection Work\nTrailing exclusion: task, 2024-01-05, 2024-01-07\n".into(),
    ] {
        let (baseline, _) = render(&source, None, json!({}));
        let (rendered, _) = render_with_requirement(&source, Some(&theme), json!({}), ThemePortabilityRequirement::RequirePortable);
        assert_eq!(rendered.svg(), baseline.svg());
        let document = Document::parse(rendered.svg()).unwrap();
        assert!(!document.descendants().any(|node| node.has_tag_name("rect") && has_class(node, "exclude-range")));
        let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
    }
}

#[cfg(feature = "diagram-pie")]
#[test]
fn full_ghibli_pie_keeps_require_portable_without_gantt_compatibility_variables() {
    let theme = DiagramThemeCompiler::new()
        .compile_preset(ThemePreset::Ghibli)
        .unwrap();
    assert!(
        theme
            .compatibility_recipe()
            .compatibility_config()
            .as_value()
            .get("themeVariables")
            .is_none()
    );
    let (rendered, _) = render_with_requirement(
        "pie title Calendar\n\"Work\": 5\n\"Weekend\": 2\n",
        Some(&theme),
        json!({}),
        ThemePortabilityRequirement::RequirePortable,
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
}

#[test]
fn unthemed_mermaid_keeps_its_default_gantt_backgrounds() {
    let (rendered, config) = render(SOURCE, None, json!({}));
    assert_eq!(config.get_str("theme"), Some("default"));
    let document = Document::parse(rendered.svg()).unwrap();
    for (selector, property, expected) in [
        (".exclude-range", "fill", "#eeeeee"),
        (".section0", "fill", "rgba(102, 102, 255, 0.49)"),
        (".section1", "fill", "white"),
        (".section2", "fill", "#fff400"),
        (".section3", "fill", "white"),
        (".section", "opacity", "0.2"),
        (".grid .tick", "stroke", "lightgrey"),
    ] {
        assert_eq!(
            css(&document, selector, property).as_deref(),
            Some(expected)
        );
    }
    assert!(
        !document
            .descendants()
            .any(|node| node.attribute("data-merman-theme-canvas").is_some())
    );
    for section in 0..4 {
        assert_eq!(outside_ink(&document, section), "black");
    }
}
