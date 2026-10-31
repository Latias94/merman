#![cfg(any(feature = "diagram-class", feature = "diagram-flowchart"))]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{DiagramTheme, ThemePortabilityRequirement};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

fn render(
    source: &str,
    requirement: ThemePortabilityRequirement,
    theme: Option<&DiagramTheme>,
) -> merman_render::Result<String> {
    render_in_environment(
        source,
        RenderEnvironment::deterministic().with_theme_portability_requirement(requirement),
        theme,
    )
}

fn render_in_environment(
    source: &str,
    environment: RenderEnvironment,
    theme: Option<&DiagramTheme>,
) -> merman_render::Result<String> {
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(json!({
            "htmlLabels": true,
        })),
    );
    let engine = match theme {
        Some(theme) => common::install_parse_compatibility(theme, engine),
        None => engine,
    };
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = match theme {
        Some(theme) => environment.begin_session_with_theme(theme),
        None => environment.begin_session(),
    }
    .unwrap();
    Ok(
        family::prepare(&parsed, &LayoutOptions::default(), session)?
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("root-font-admission".to_owned()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )?
            .svg()
            .to_owned(),
    )
}

#[test]
fn default_and_best_effort_preserve_svg_while_portable_plain_diagrams_still_succeed() {
    let sources = [
        #[cfg(feature = "diagram-class")]
        "classDiagram\nclass A\n",
        #[cfg(feature = "diagram-flowchart")]
        "flowchart LR\nA[Measured label]\n",
        #[cfg(feature = "diagram-sequence")]
        "sequenceDiagram\nAlice->>Bob: Hello\n",
        #[cfg(feature = "diagram-pie")]
        "pie\n\"A\": 1\n\"B\": 2\n",
        #[cfg(feature = "diagram-info")]
        "info\n",
    ];
    for source in sources {
        let default = render_in_environment(source, RenderEnvironment::default(), None).unwrap();
        let best_effort = render(source, ThemePortabilityRequirement::BestEffort, None).unwrap();
        let portable = render(source, ThemePortabilityRequirement::RequirePortable, None).unwrap();
        assert_eq!(
            default, best_effort,
            "default must retain BestEffort behavior: {source}"
        );
        assert_eq!(
            default, portable,
            "successful strict admission must preserve SVG: {source}"
        );
    }
}

#[test]
fn default_rem_output_matches_explicit_best_effort_without_a_theme() {
    let sources = [
        #[cfg(feature = "diagram-class")]
        "classDiagram\nclass A\nstyle A font-size:2rem\n",
        #[cfg(feature = "diagram-flowchart")]
        "flowchart LR\nA[Measured label]\nstyle A font-size:2rem\n",
    ];
    for source in sources {
        let default = render_in_environment(source, RenderEnvironment::default(), None).unwrap();
        let best_effort = render(source, ThemePortabilityRequirement::BestEffort, None).unwrap();
        assert_eq!(
            default, best_effort,
            "default rem compatibility must remain enabled"
        );
        assert!(default.contains("font-size:2rem"));
    }
}

fn assert_source_sizes(family: &str) {
    for (style, root_dependent) in [
        ("font-size:2rem", true),
        ("font-size:2rem,font-size:24px", false),
        ("font-size:2rem !important,font-size:24px", true),
        ("font-size:2rem !important,font-size:24px !important", false),
        ("font-size:24px !important,font-size:2rem", false),
    ] {
        let source = format!("{family}style A {style}\n");
        let compatible = render(&source, ThemePortabilityRequirement::BestEffort, None)
            .expect("compatibility rendering retains root-dependent CSS");
        if style == "font-size:2rem" {
            assert!(compatible.contains("font-size:2rem"), "{compatible}");
        }
        let strict = render(&source, ThemePortabilityRequirement::RequirePortable, None);
        if root_dependent {
            assert!(
                matches!(
                    strict,
                    Err(merman_render::Error::UnverifiedFamilyStyle { .. })
                ),
                "source-owned rem needs a residual without a typed theme: {source}: {strict:?}"
            );
        } else {
            let px_only = render(
                &format!("{family}style A font-size:24px\n"),
                ThemePortabilityRequirement::RequirePortable,
                None,
            );
            // A px winner still needs its existing terminal writer receipt. Replacing
            // its shadowed history must not add a new root-dependency residual.
            match (strict, px_only) {
                (Ok(_), Ok(_)) => {}
                (
                    Err(merman_render::Error::UnverifiedFamilyStyle {
                        family_id,
                        residual_count,
                    }),
                    Err(merman_render::Error::UnverifiedFamilyStyle {
                        family_id: expected_family,
                        residual_count: expected_count,
                    }),
                ) => {
                    assert_eq!(family_id, expected_family, "{source}");
                    assert_eq!(residual_count, expected_count, "{source}");
                }
                (actual, expected) => panic!("{source}: {actual:?}; px-only: {expected:?}"),
            }
        }
    }
}

#[cfg(feature = "diagram-class")]
#[test]
fn class_source_font_size_is_fail_closed_without_a_typed_theme() {
    assert_source_sizes("classDiagram\nclass A\n");
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn flowchart_source_font_size_is_fail_closed_without_a_typed_theme() {
    assert_source_sizes("flowchart LR\nA[Measured label]\n");
}

#[test]
fn root_dependent_font_sizes_remain_residual_with_typed_typography() {
    use merman_render::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, ThemeTextStyle, TypographySpec,
    };

    let families = [
        #[cfg(feature = "diagram-class")]
        (
            merman_model::family::DiagramFamilyId::CLASS,
            "classDiagram\nclass A\n",
        ),
        #[cfg(feature = "diagram-flowchart")]
        (
            merman_model::family::DiagramFamilyId::FLOWCHART,
            "flowchart LR\nA[Measured label]\n",
        ),
    ];
    for (id, family) in families {
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    id,
                    ThemeTextStyle::default().with_font_size_px(23.0).unwrap(),
                ),
            ))
            .unwrap();
        let source = format!("{family}style A font-size:2rem\n");
        render(
            &source,
            ThemePortabilityRequirement::BestEffort,
            Some(&theme),
        )
        .unwrap();
        assert!(
            render(
                &source,
                ThemePortabilityRequirement::RequirePortable,
                Some(&theme)
            )
            .is_err()
        );
    }
}

#[allow(dead_code)]
mod common;
