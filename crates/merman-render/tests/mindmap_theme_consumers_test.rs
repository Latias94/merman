#![cfg(feature = "diagram-mindmap")]

use merman_core::presentation::SourcePresentationConfig;
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::OperationControl;
use merman_model::presentation::PresentationDiagram;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack,
    ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    ThemeTextStyle, TypographySpec,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{DiagramFamilyId, LayoutOptions};
use serde_json::{Value, json};

fn font_theme() -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    DiagramFamilyId::MINDMAP,
                    ThemeTextStyle::default()
                        .with_font_stack(FontStack::new(["Arial", "sans-serif"]).unwrap()),
                ),
            ),
        )
        .unwrap()
}

fn fill_theme() -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::NodeLabel,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    )
                    .for_family(DiagramFamilyId::MINDMAP),
                ),
            ),
        )
        .unwrap()
}

fn parsed(label: &str, config: Value, theme: Option<&DiagramTheme>) -> PresentationDiagram {
    let mut engine =
        MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(config));
    if let Some(theme) = theme {
        let plan = merman_core::presentation::ThemeCompatibilityPlan::from_recipe(
            theme.compatibility_recipe().clone(),
        );
        engine = engine.with_theme_plan(&plan);
    }
    engine
        .parse_diagram_for_render_model_sync(
            &format!("mindmap\n  root[\"{label}\"]\n"),
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation()
}

fn render(label: &str, config: Value, theme: &DiagramTheme) -> family::RenderedFamilySvg {
    let parsed = parsed(label, config, Some(theme));
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::BestEffort)
        .begin_session_with_theme(theme)
        .unwrap();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
}

fn assert_outcome(rendered: family::RenderedFamilySvg, expected: (usize, usize, usize)) {
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1, "{evidence:?}");
    assert_eq!(
        (
            evidence.applied_count(),
            evidence.not_applicable_count(),
            evidence.theme_residual_count(),
        ),
        expected,
        "{evidence:?}"
    );
}

#[test]
fn mindmap_font_stack_evidence_requires_visible_inheriting_text() {
    let theme = font_theme();
    for (label, expected) in [
        ("Root", (1, 0, 0)),
        ("<span></span>", (0, 1, 0)),
        ("<span style='font-family:monospace'>Root</span>", (0, 1, 0)),
        ("<span style='font-family:inherit'>Root</span>", (1, 0, 0)),
        (
            "Inherited <span style='font-family:monospace'>Local</span>",
            (0, 0, 1),
        ),
    ] {
        let rendered = render(label, json!({"layout": "tidy-tree"}), &theme);
        if label.contains("font-family:monospace") {
            assert!(rendered.svg().contains("font-family:monospace"));
        }
        assert_outcome(rendered, expected);
    }
}

#[test]
fn mindmap_intrinsic_code_fonts_are_local_until_explicitly_inherited() {
    let theme = font_theme();
    for tag in ["code", "kbd", "samp", "tt"] {
        for (label, expected) in [
            (format!("<{tag}>Root</{tag}>"), (0, 1, 0)),
            (format!("Inherited <{tag}>Local</{tag}>"), (0, 0, 1)),
            (
                format!("<{tag} style='font-family:inherit'>Root</{tag}>"),
                (1, 0, 0),
            ),
            (format!("<{tag} class='host-font'>Root</{tag}>"), (0, 0, 1)),
        ] {
            assert_outcome(
                render(&label, json!({"layout": "tidy-tree"}), &theme),
                expected,
            );
        }
    }
}

#[test]
fn mindmap_commented_local_colors_keep_conservative_cascade_evidence() {
    let theme = fill_theme();
    for (declarations, expected) in [
        ("color:/* local */#ff0000", (0, 0, 1)),
        ("color:#ff0000", (0, 1, 0)),
        ("color:/* local */red;color:inherit", (1, 0, 0)),
        ("color:/* local */red!important;color:inherit", (0, 0, 1)),
        (
            "color:/* local */red!important;color:inherit!important",
            (1, 0, 0),
        ),
        ("color:/* local */red;color:#ff0000", (0, 1, 0)),
        (r"color:r\65 d!important;color:inherit", (0, 0, 1)),
        (r"color:r\65 d!important;color:inherit!important", (1, 0, 0)),
        (
            "color:color-mix(in srgb,red 100%,blue)!important;color:inherit",
            (0, 0, 1),
        ),
        ("all:initial", (0, 0, 1)),
        ("all:/* local */initial", (0, 0, 1)),
        ("all:inherit", (1, 0, 0)),
    ] {
        let label = format!("<span style='{declarations}'>Root</span>");
        let rendered = render(&label, json!({"layout": "tidy-tree"}), &theme);
        assert!(
            rendered.svg().contains(declarations),
            "retained source style: {declarations}"
        );
        assert_outcome(rendered, expected);
    }
}

#[test]
fn mindmap_font_stack_keeps_unknown_family_dependencies_residual() {
    let theme = font_theme();
    for (label, expected) in [
        ("<span class='host-font'>Root</span>", (0, 0, 1)),
        (
            "<span style='font-family:var(--host-font)'>Root</span>",
            (0, 0, 1),
        ),
        ("<span style='font-family:initial'>Root</span>", (0, 0, 1)),
        ("<span class='host-font'></span>", (0, 1, 0)),
        ("<span style='font-family:monospace'>Root</span>", (0, 1, 0)),
        (
            "<em style='font-family:monospace'><span style='font-family:inherit'>Root</span></em>",
            (0, 1, 0),
        ),
        (
            "<span class='host-font' style='font-family:monospace'>Root</span>",
            (0, 0, 1),
        ),
        (
            "<span class='host-font' style='font-family:monospace !important'>Root</span>",
            (0, 1, 0),
        ),
        (
            "<span class='host-font' style='font-family:inherit !important'>Root</span>",
            (1, 0, 0),
        ),
        ("Root <span class='host-font'>Unknown</span>", (0, 0, 1)),
        (
            "<span style='font-family:monospace'>Local</span> <span style='font-family:var(--host-font)'>Unknown</span>",
            (0, 0, 1),
        ),
    ] {
        assert_outcome(
            render(label, json!({"layout": "tidy-tree"}), &theme),
            expected,
        );
    }
}

#[test]
fn mindmap_label_paint_evidence_tracks_visible_source_ownership_and_unknown_runs() {
    let theme = fill_theme();
    for (label, expected) in [
        ("Root", (1, 0, 0)),
        ("<span></span>", (0, 1, 0)),
        ("<span style='color:#ff0000'>Root</span>", (0, 1, 0)),
        ("<em style='color:#ff0000'>Root</em>", (0, 1, 0)),
        ("Inherited <span class='host-ink'>Unknown</span>", (0, 0, 1)),
        (
            "Inherited <span style='color:#ff0000'>Local</span>",
            (0, 0, 1),
        ),
    ] {
        let rendered = render(label, json!({"layout": "tidy-tree"}), &theme);
        if label.contains("color:#ff0000") {
            assert!(rendered.svg().contains("color:#ff0000"));
        }
        assert_outcome(rendered, expected);
    }
}

#[test]
fn mindmap_label_paint_observes_direct_span_rules_before_source_color_inheritance() {
    let theme = fill_theme();
    for (label, expected, emitted) in [
        (
            "<em style='color:#ff0000'><span>Root</span></em>",
            (1, 0, 0),
            true,
        ),
        (
            "<span style='color:#ff0000'><span><b>Root</b></span></span>",
            (1, 0, 0),
            true,
        ),
        (
            "<em style='color:#ff0000'>Local <span>Root</span></em>",
            (0, 0, 1),
            true,
        ),
        (
            "<em style='color:#ff0000'><span style='color:inherit'>Root</span></em>",
            (0, 1, 0),
            false,
        ),
        (
            "<em style='color:#ff0000'><span style='color:currentColor'>Root</span></em>",
            (0, 1, 0),
            false,
        ),
        (
            "<em style='color:#ff0000'><span style='color:unset'>Root</span></em>",
            (0, 1, 0),
            false,
        ),
        (
            "<em style='color:red'><span style='color:inherit !important;color:blue'>Root</span></em>",
            (0, 1, 0),
            false,
        ),
        // Mermaid's encodeEntities preprocessing removes semicolons from style/#...; matches.
        // The retained inner declaration is invalid, so the terminal span uses its role color.
        (
            "<em style='color:#ff0000'><span style='color:inherit !important;color:#0000ff'>Root</span></em>",
            (1, 0, 0),
            true,
        ),
        (
            "<em style='color:#ff0000'><span style='color:#0000ff'>Root</span></em>",
            (0, 1, 0),
            false,
        ),
        ("<em style='color:#ff0000'>Root</em>", (0, 1, 0), false),
        (
            "<em style='color:#ff0000'><span class='host-ink'>Root</span></em>",
            (0, 0, 1),
            true,
        ),
    ] {
        let rendered = render(label, json!({"layout": "tidy-tree"}), &theme);
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let retained_styles = document
            .descendants()
            .filter(|node| matches!(node.tag_name().name(), "em" | "span"))
            .filter_map(|node| node.attribute("style"))
            .collect::<Vec<_>>();
        assert_eq!(
            rendered
                .svg()
                .contains("> .label span{fill:#123456;color:#123456;}"),
            emitted,
            "the actual typed span winner must follow prepared ownership: {label}; retained styles: {retained_styles:?}"
        );
        let parent_style = if label.contains("color:red") {
            "color:red"
        } else {
            "color:#ff0000"
        };
        assert!(
            retained_styles.contains(&parent_style),
            "source parent color is retained: {label}"
        );
        assert_outcome(rendered, expected);
    }
}

#[test]
fn mindmap_visible_ownership_uses_sanitized_fragments_instead_of_raw_markup() {
    for theme in [font_theme(), fill_theme()] {
        for keep_content in [true, false] {
            let rendered = render(
                "<span style='font-family:monospace;color:#ff0000'>Root</span>",
                json!({
                    "layout": "tidy-tree", "securityLevel": "loose",
                    "dompurifyConfig": {"FORBID_TAGS": ["span"], "KEEP_CONTENT": keep_content}
                }),
                &theme,
            );
            assert!(!rendered.svg().contains("font-family:monospace"));
            assert!(!rendered.svg().contains("color:#ff0000"));
            assert_outcome(rendered, if keep_content { (1, 0, 0) } else { (0, 1, 0) });
        }
    }
}

#[test]
fn mindmap_svg_text_mode_keeps_html_style_spellings_literal() {
    for theme in [font_theme(), fill_theme()] {
        let rendered = render(
            "<span style='font-family:monospace;color:#ff0000'>Root</span>",
            json!({"layout": "tidy-tree", "htmlLabels": false}),
            &theme,
        );
        assert!(!rendered.svg().contains("<foreignObject"));
        assert_outcome(rendered, (1, 0, 0));
    }
}

#[test]
fn mindmap_native_admission_rejects_unmeasured_html_geometry() {
    for (style, property) in [
        ("letter-spacing:30px", "letter-spacing"),
        ("word-spacing:30px", "word-spacing"),
        ("text-transform:uppercase", "text-transform"),
        ("font:italic 30px Cousine", "font"),
        ("line-height:4", "line-height"),
        ("LETTER-SPACING:30px !important", "letter-spacing"),
        (r"letter\2d spacing:30px", "letter-spacing"),
    ] {
        let parsed = parsed(
            &format!("<span style='{style}'>AAAA</span>"),
            json!({}),
            None,
        );
        assert_eq!(
            merman_render::__private::unsupported_native_html_style(
                &parsed,
                &OperationControl::new(),
            )
            .unwrap()
            .as_deref(),
            Some(property),
            "{style}"
        );
    }
}

#[test]
fn mindmap_native_admission_reads_property_syntax_independently_of_emission_safety() {
    for (style, expected) in [
        ("letter-spacing:/* comment */30px", Some("letter-spacing")),
        ("word-spacing:/* comment */30px", Some("word-spacing")),
        (
            "text-transform:/* comment */uppercase",
            Some("text-transform"),
        ),
        (
            r"/* leading */LETTER\2d SPACING/* property */:/* value */30px !important;",
            Some("letter-spacing"),
        ),
        ("color:red;line-height:/* value */4;", Some("line-height")),
        ("letter-spacing:/* comment */inherit;", None),
        ("letter-spacing:/* comment */unset !important;", None),
        (
            r"letter-spacing:/* comment */\69nherit /**/ !important;",
            None,
        ),
        ("letter-spacing:/* comment only */;color:red", None),
        ("color:/* letter-spacing:30px; */red;", None),
        ("--probe:letter-spacing:30px;color:red", None),
        ("all:initial", Some("all")),
        ("font-size:64px;all:inherit", Some("all")),
        ("font-size:64px;all:unset", Some("all")),
        ("font-size:64px;font:inherit", Some("font")),
        ("font-size:64px;font:unset", Some("font")),
        (
            "font-size:calc(48px)",
            Some("font-size (unresolved CSS value)"),
        ),
        ("font-size:calc(48px);font-size:24px", None),
        (
            r"font-size:64px;\61ll:/* value */\75nset !important",
            Some("all"),
        ),
    ] {
        let label = format!("<span style='{style}'>AAAA</span>");
        let source = parsed(&label, json!({}), None);
        assert_eq!(
            merman_render::__private::unsupported_native_html_style(
                &source,
                &OperationControl::new()
            )
            .unwrap()
            .as_deref(),
            expected,
            "{style}"
        );
        if expected.is_some() {
            for config in [
                json!({"htmlLabels": false}),
                json!({"securityLevel": "loose", "dompurifyConfig": {"FORBID_ATTR": ["style"]}}),
            ] {
                assert_eq!(
                    merman_render::__private::unsupported_native_html_style(
                        &parsed(&label, config.clone(), None),
                        &OperationControl::new(),
                    )
                    .unwrap(),
                    None,
                    "non-HTML or removed declarations must not reject: {style}, {config}"
                );
            }
        }
    }
}

#[test]
fn mindmap_native_admission_keeps_supported_font_longhands_and_literal_text() {
    for (label, config) in [
        (
            "<span style='font-family:Cousine;font-size:2em;font-weight:700;font-style:italic'>AAAA</span>",
            json!({}),
        ),
        (
            "<span style='letter-spacing:inherit'>AAAA</span>",
            json!({}),
        ),
        (
            "<span style='letter-spacing:30px'>AAAA</span>",
            json!({"htmlLabels": false}),
        ),
        (
            "&lt;span style='letter-spacing:30px'&gt;AAAA&lt;/span&gt;",
            json!({}),
        ),
        ("$$x^2$$", json!({})),
    ] {
        assert_eq!(
            merman_render::__private::unsupported_native_html_style(
                &parsed(label, config, None),
                &OperationControl::new(),
            )
            .unwrap(),
            None,
            "{label}"
        );
    }
}

#[test]
fn mindmap_native_admission_observes_sanitizer_removals_and_rem_ownership() {
    let label = "<span style='letter-spacing:30px'>AAAA</span>";
    let config = json!({
        "securityLevel": "loose",
        "dompurifyConfig": {"FORBID_ATTR": ["style"]}
    });
    assert_eq!(
        merman_render::__private::unsupported_native_html_style(
            &parsed(label, config, None),
            &OperationControl::new(),
        )
        .unwrap(),
        None,
    );
    for (style, rejected) in [
        ("font-size:2rem", true),
        ("font-size:2rem;font-size:24px", false),
        ("font-size:2rem !important;font-size:24px", true),
    ] {
        let source = parsed(
            &format!("<span style='{style}'>AAAA</span>"),
            json!({}),
            None,
        );
        assert_eq!(
            merman_render::__private::unsupported_native_html_style(
                &source,
                &OperationControl::new(),
            )
            .unwrap()
            .is_some(),
            rejected,
            "{style}"
        );
    }
}

#[test]
fn mindmap_native_admission_preserves_cancellation_during_label_preparation() {
    let label = format!(
        "{}<span style='letter-spacing:30px'>AAAA</span>",
        "word ".repeat(8192)
    );
    let source = parsed(&label, json!({}), None);
    let control = OperationControl::new();
    control.cancel_after_checkpoints(8);
    let error = merman_render::__private::unsupported_native_html_style(&source, &control)
        .expect_err("native admission must remain cancellable within a large label");
    assert_eq!(error.reason, merman_model::CancelReason::Requested);
}
