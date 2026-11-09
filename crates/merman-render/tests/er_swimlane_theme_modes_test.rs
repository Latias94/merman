#![cfg(feature = "diagram-er")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::OperationControl;
use merman_model::family::DiagramFamilyId;
use merman_model::presentation::{PresentationDiagram, RenderSemanticModel};
use merman_model::resources::InputResourcePolicy;
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemePortabilityRequirement, ThemeTextStyle,
    TypographySpec,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

mod common;

#[test]
fn er_swimlane_source_fonts_follow_each_groups_actual_html_mode() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    DiagramFamilyId::ER,
                    ThemeTextStyle::default()
                        .with_font_stack(FontStack::single("TypedFace").unwrap())
                        .with_font_size_px(24.0)
                        .unwrap(),
                ),
            ),
        )
        .unwrap();
    for (root, flowchart) in [(false, true), (true, false)] {
        for (styled_group, html) in [("Outer", flowchart), ("Inner", root)] {
            let source = format!(
                "---\nconfig: {}\n---\nerDiagram\nsubgraph Outer[Outer title]\nsubgraph Inner[Inner title]\nA\nend\nend\nstyle {styled_group} font-family:SourceFace,font-size:30px\n",
                json!({"layout": "swimlane", "htmlLabels": root,
                    "flowchart": {"htmlLabels": flowchart}}),
            );
            let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            for requirement in [
                ThemePortabilityRequirement::BestEffort,
                ThemePortabilityRequirement::RequirePortable,
            ] {
                let session = RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(requirement)
                    .begin_session_with_theme(&theme)
                    .unwrap();
                let result = family::prepare(&parsed, &LayoutOptions::default(), session)
                    .unwrap()
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
                if html && matches!(requirement, ThemePortabilityRequirement::RequirePortable) {
                    let error = match result {
                        Ok(_) => panic!("expected unverified ER source typography"),
                        Err(error) => error,
                    };
                    assert_eq!(
                        error.unverified_family_theme(),
                        Some((DiagramFamilyId::ER, 2)),
                        "{styled_group}: root={root}, flowchart={flowchart}"
                    );
                    continue;
                }
                let rendered = result.unwrap();
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let dom_id = format!("merman-{styled_group}");
                let group = document
                    .descendants()
                    .find(|element| element.attribute("id") == Some(dom_id.as_str()))
                    .unwrap();
                assert_eq!(
                    group.descendants().any(|element| {
                        element
                            .attribute("style")
                            .is_some_and(|style| style.contains("SourceFace"))
                    }),
                    html,
                );
                let completion = rendered.into_completion();
                let evidence = merman_render::__private::family_evidence(completion.report());
                assert_eq!(evidence.theme_residual_count(), if html { 2 } else { 0 });
                assert_eq!(evidence.applied_count(), if html { 0 } else { 2 });
            }
        }
    }
}

fn render_subgraph_typography(
    source: &str,
    spec: DiagramThemeSpec,
    requirement: ThemePortabilityRequirement,
    typed_group_font_size: Option<(&str, &str)>,
) -> Result<(String, [usize; 3]), merman_render::Error> {
    let theme = DiagramThemeCompiler::new().compile(spec).unwrap();
    let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let parsed = if let Some((group_id, size)) = typed_group_font_size {
        let (metadata, mut model, context) = parsed.into_parts();
        let RenderSemanticModel::Er(er) = &mut model else {
            panic!("expected ER model");
        };
        let group = er
            .subgraphs
            .iter_mut()
            .find(|group| group.id == group_id)
            .unwrap();
        let font_size = group
            .css_styles
            .iter_mut()
            .find(|style| style.as_str() == "font-size:0px")
            .expect("valid source seed supplies a zero-sized group font");
        *font_size = format!("font-size:{size}");
        PresentationDiagram::try_from_frozen(
            metadata,
            model,
            context,
            &InputResourcePolicy::default(),
            &OperationControl::new(),
        )
        .expect("validate caller-authored ER presentation")
        .expect("admit caller-authored ER presentation")
    } else {
        parsed
    };
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(requirement)
        .begin_session_with_theme(&theme)
        .unwrap();
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)?
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    let svg = rendered.svg().to_owned();
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    Ok((
        svg,
        [
            evidence.applied_count(),
            evidence.not_applicable_count(),
            evidence.theme_residual_count(),
        ],
    ))
}

#[test]
fn er_html_subgraph_titles_consume_fonts_when_entity_text_is_zero_sized() {
    for (nested, restored) in [(false, false), (false, true), (true, true)] {
        let title = if restored {
            "<span style='font-size:20px'>Visible lane</span>"
        } else {
            "Visible lane"
        };
        let lane = format!("subgraph Lane[\"{title}\"]\nA\nend\n");
        let groups = if nested {
            format!("subgraph Outer[Hidden outer]\n{lane}end\n")
        } else {
            lane
        };
        let source = format!(
            "---\nconfig: {}\n---\nerDiagram\n{groups}style A font-size:0px\n",
            json!({"layout": "swimlane", "htmlLabels": nested,
                "flowchart": {"htmlLabels": !nested},
                "themeVariables": {"fontSize": if restored { "0px" } else { "16px" }}}),
        );
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(
                DiagramFamilyId::ER,
                ThemeTextStyle::default()
                    .with_font_stack(FontStack::single("TypedFace").unwrap())
                    .with_font_weight(600)
                    .unwrap(),
            ),
        );
        let (svg, evidence) = render_subgraph_typography(
            &source,
            spec.clone(),
            ThemePortabilityRequirement::BestEffort,
            None,
        )
        .unwrap();
        assert_eq!(evidence, [1, 0, 1], "nested={nested}, restored={restored}");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let lane = document
            .descendants()
            .find(|node| node.attribute("id") == Some("merman-Lane"))
            .unwrap();
        assert!(
            lane.descendants()
                .any(|node| node.has_tag_name("foreignObject"))
        );
        assert!(
            lane.descendants()
                .any(|node| node.text() == Some("Visible lane"))
        );
        assert!(svg.contains("font-family:TypedFace;"));
        let error = render_subgraph_typography(
            &source,
            spec,
            ThemePortabilityRequirement::RequirePortable,
            None,
        )
        .unwrap_err();
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::ER, 1))
        );
    }
}

#[test]
fn er_html_subgraph_local_fonts_only_own_potentially_visible_title_runs() {
    for (root, flowchart, styled_group) in [(false, true, "Outer"), (true, false, "Inner")] {
        for (source_size, typed_size, title, visible) in [
            ("0px", None, "Hidden title", false),
            (
                "0px",
                None,
                "<span style='font-size:20px'>Restored title</span>",
                true,
            ),
            // ER source styles exclude parentheses; author CSS functions in the typed model.
            ("0px", Some("var(--host-size)"), "Possible title", true),
        ] {
            let size = typed_size.unwrap_or(source_size);
            let typed_group_font_size = typed_size.map(|size| (styled_group, size));
            let source = format!(
                "---\nconfig: {}\n---\nerDiagram\nsubgraph Outer[\"{}\"]\nsubgraph Inner[\"{}\"]\nA\nend\nend\nstyle {styled_group} font-size:{source_size},font-family:Other\n",
                json!({"layout": "swimlane", "htmlLabels": root,
                    "flowchart": {"htmlLabels": flowchart}}),
                if styled_group == "Outer" {
                    title
                } else {
                    "Outer title"
                },
                if styled_group == "Inner" {
                    title
                } else {
                    "Inner title"
                },
            );
            let spec = DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    DiagramFamilyId::ER,
                    ThemeTextStyle::default()
                        .with_font_stack(FontStack::single("TypedFace").unwrap())
                        .with_font_size_px(24.0)
                        .unwrap(),
                ),
            );
            let (svg, evidence) = render_subgraph_typography(
                &source,
                spec.clone(),
                ThemePortabilityRequirement::BestEffort,
                typed_group_font_size,
            )
            .unwrap();
            assert_eq!(
                evidence,
                if visible { [0, 0, 2] } else { [2, 0, 0] },
                "{styled_group}: {size}, {title}"
            );
            let document = roxmltree::Document::parse(&svg).unwrap();
            let dom_id = format!("merman-{styled_group}");
            let group = document
                .descendants()
                .find(|node| node.attribute("id") == Some(dom_id.as_str()))
                .unwrap();
            assert!(group.descendants().any(|node| {
                node.has_tag_name("span")
                    && node.attribute("class") == Some("nodeLabel")
                    && node.attribute("style").is_some_and(|style| {
                        style.contains("font-family:Other !important")
                            && style.contains(&format!("font-size:{size} !important"))
                    })
            }));
            let portable = render_subgraph_typography(
                &source,
                spec,
                ThemePortabilityRequirement::RequirePortable,
                typed_group_font_size,
            );
            if visible {
                assert_eq!(
                    portable.unwrap_err().unverified_family_theme(),
                    Some((DiagramFamilyId::ER, 2))
                );
            } else {
                assert_eq!(portable.unwrap().1, [2, 0, 0]);
            }
        }
    }
}

#[test]
fn er_html_subgraph_preparation_charges_its_retained_projection() {
    use merman_render::resources::{RenderResourcePolicy, ResourceLimitId};
    for config in [
        json!({"layout": "dagre", "htmlLabels": true}),
        json!({"layout": "swimlane", "htmlLabels": false,
            "flowchart": {"htmlLabels": true}}),
    ] {
        let render = |title: &str, policy: RenderResourcePolicy| {
            let source = format!(
                "---\nconfig: {config}\n---\nerDiagram\nsubgraph Lane[\"{title}\"]\nA\nend\n"
            );
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let session = RenderEnvironment::deterministic()
                .with_resource_policy(policy)
                .begin_session()
                .unwrap();
            let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)?
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
            Ok::<_, merman_render::Error>(
                rendered
                    .into_completion()
                    .report()
                    .session_report()
                    .prepared_text_retained_bytes_peak(),
            )
        };
        let short_bytes =
            render("short", RenderResourcePolicy::unbounded_for_trusted_input()).unwrap();
        let title = "&".repeat(2048);
        let long_bytes =
            render(&title, RenderResourcePolicy::unbounded_for_trusted_input()).unwrap();
        assert!(long_bytes > short_bytes + title.len(), "{config}");
        let error = render(
            &title,
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, short_bytes)
                .unwrap(),
        )
        .unwrap_err();
        let merman_render::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected retained HTML-title byte rejection");
        };
        assert_eq!(error.limit, "max_prepared_text_retained_bytes");
        assert_eq!(error.max, short_bytes);
        assert!(error.actual > short_bytes);
    }
}
