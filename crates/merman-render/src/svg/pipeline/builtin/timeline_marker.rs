//! Native export adapter for Mermaid 12.3's vertical Timeline marker mismatch.
//!
//! Parity deliberately retains `undefined-arrowhead` and `url(#arrowhead)`. Only the
//! renderer-owned Timeline resvg-safe stage may connect those exact emitted values, before ID
//! rebasing. Missing markers and other references remain terminal validation errors.

use std::borrow::Cow;

use super::util::{SvgTagScanner, next_svg_quoted_attr_with_checkpoints, start_tag_name};
use crate::svg::pipeline::{SvgPostprocessExecution, SvgPostprocessMetadata};
use crate::{DiagramFamilyId, Result};

const LEGACY_MARKER_ID: &str = "undefined-arrowhead";
const LEGACY_REFERENCE: &str = "url(#arrowhead)";
const RESOLVED_REFERENCE: &str = "url(#undefined-arrowhead)";

pub(crate) fn resolve_timeline_marker_references<'a>(
    svg: Cow<'a, str>,
    metadata: &SvgPostprocessMetadata,
    execution: SvgPostprocessExecution<'_>,
) -> Result<Cow<'a, str>> {
    execution.checkpoint()?;
    execution.preflight_svg_byte_count(svg.len())?;
    if metadata.family_id() != Some(DiagramFamilyId::TIMELINE) {
        return Ok(svg);
    }

    let mut scanner = SvgTagScanner::new(&svg);
    let mut legacy_ids = 0usize;
    let mut has_legacy_marker = false;
    let mut references = Vec::new();
    while let Some(tag) = scanner.next_with_checkpoints(&mut || execution.checkpoint())? {
        let Some(name) = start_tag_name(tag.raw()) else {
            continue;
        };
        let mut cursor = 0usize;
        while let Some(attribute) =
            next_svg_quoted_attr_with_checkpoints(tag.raw(), cursor, &mut || {
                execution.checkpoint()
            })?
        {
            let attribute_name = &tag.raw()[attribute.name_start..attribute.name_end];
            let value = &tag.raw()[attribute.value_start..attribute.value_end];
            if attribute_name == "id" {
                if value == "arrowhead" {
                    return Ok(svg);
                }
                if value == LEGACY_MARKER_ID {
                    legacy_ids += 1;
                    has_legacy_marker |= name == "marker";
                }
            } else if name == "line" && attribute_name == "marker-end" && value == LEGACY_REFERENCE
            {
                references
                    .push(tag.start() + attribute.value_start..tag.start() + attribute.value_end);
            }
            cursor = attribute.full_end;
        }
    }
    execution.checkpoint()?;
    if !has_legacy_marker || legacy_ids != 1 || references.is_empty() {
        return Ok(svg);
    }

    let projected_bytes = references
        .len()
        .checked_mul(RESOLVED_REFERENCE.len() - LEGACY_REFERENCE.len())
        .and_then(|additional| svg.len().checked_add(additional))
        .ok_or_else(|| execution.svg_byte_count_overflow())?;
    execution.preflight_svg_byte_count(projected_bytes)?;
    let mut output = String::with_capacity(projected_bytes);
    let mut copied_until = 0usize;
    for reference in references {
        append_with_checkpoints(&mut output, &svg[copied_until..reference.start], execution)?;
        output.push_str(RESOLVED_REFERENCE);
        copied_until = reference.end;
    }
    append_with_checkpoints(&mut output, &svg[copied_until..], execution)?;
    execution.checkpoint()?;
    Ok(Cow::Owned(output))
}

fn append_with_checkpoints(
    output: &mut String,
    mut value: &str,
    execution: SvgPostprocessExecution<'_>,
) -> Result<()> {
    while !value.is_empty() {
        execution.checkpoint()?;
        let mut end = value.len().min(4096);
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        output.push_str(&value[..end]);
        value = &value[end..];
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;
    use crate::environment::{RenderEnvironment, RenderSession};
    use crate::resources::{RenderResourcePolicy, ResourceLimitId, ResourceLimitPhase};
    use crate::svg::pipeline::{SvgOutputPolicy, SvgPipeline, SvgPipelinePreset};

    const LEGACY_SVG: &str = concat!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" id="round9-probe" aria-roledescription="timeline" width="100" height="100">"#,
        r#"<g class="lineWrapper"><line x1="10" y1="10" x2="90" y2="10" marker-end="url(#arrowhead)"/></g>"#,
        r#"<defs><marker id="undefined-arrowhead" refX="5" refY="2" markerWidth="6" markerHeight="4" orient="auto">"#,
        r#"<path d="M 0,0 V 4 L6,2 Z"/></marker></defs>"#,
        r#"<g class="lineWrapper"><line x1="10" y1="50" x2="90" y2="50" marker-end="url(#arrowhead)" stroke-dasharray="5,5"/></g></svg>"#,
    );

    fn session() -> RenderSession {
        RenderEnvironment::deterministic().begin_session().unwrap()
    }

    fn metadata(svg: &str) -> SvgPostprocessMetadata {
        SvgPostprocessMetadata::from_svg(svg).with_family_id(DiagramFamilyId::TIMELINE)
    }

    fn policy(preset: SvgPipelinePreset) -> SvgPipeline {
        SvgOutputPolicy {
            preset,
            ..SvgOutputPolicy::default()
        }
        .pipeline()
    }

    fn assert_missing_reference(svg: &str, fragment: &str) {
        let error = policy(SvgPipelinePreset::ResvgSafe)
            .process_resvg_compatible_with_metadata(svg, &metadata(svg), &session())
            .unwrap_err();
        assert!(
            matches!(error, Error::SvgPostprocess { ref pass, ref message }
                if pass == "validate-resvg-compatible-svg"
                    && message.contains(&format!("missing same-document fragment #{fragment}"))),
            "{error}"
        );
    }

    fn assert_marker_references_resolve(svg: &str, expected_id: &str) {
        let document = crate::xml_dom::Document::parse(svg).unwrap();
        let marker = document
            .descendants()
            .find(|node| node.has_tag_name("marker"))
            .unwrap();
        assert_eq!(marker.attribute("id"), Some(expected_id));
        let references = document
            .descendants()
            .filter_map(|node| node.attribute("marker-end"))
            .collect::<Vec<_>>();
        assert_eq!(references.len(), 2);
        for reference in references {
            assert_eq!(reference, format!("url(#{expected_id})"));
        }
    }

    #[test]
    fn canonical_timeline_policy_resolves_live_marker_before_id_rebasing() {
        for prefix in [None, Some("embed")] {
            let mut pipeline = policy(SvgPipelinePreset::ResvgSafe);
            if let Some(prefix) = prefix {
                pipeline = pipeline.with_rebased_ids(prefix);
            }
            let sealed = pipeline
                .process_resvg_compatible_with_metadata(
                    LEGACY_SVG,
                    &metadata(LEGACY_SVG),
                    &session(),
                )
                .unwrap();
            let expected = prefix.map_or_else(
                || LEGACY_MARKER_ID.to_owned(),
                |prefix| format!("{prefix}-{LEGACY_MARKER_ID}"),
            );
            assert_marker_references_resolve(sealed.as_str(), &expected);
            if prefix.is_none() {
                assert_eq!(
                    sealed.as_str(),
                    LEGACY_SVG.replace(LEGACY_REFERENCE, RESOLVED_REFERENCE)
                );
                let twice = policy(SvgPipelinePreset::ResvgSafe)
                    .process_resvg_compatible_with_metadata(
                        sealed.as_str(),
                        &metadata(sealed.as_str()),
                        &session(),
                    )
                    .unwrap();
                assert_eq!(twice.as_str(), sealed.as_str());
            }
        }
    }

    #[test]
    fn parity_and_readable_policies_preserve_timeline_bytes() {
        for preset in [SvgPipelinePreset::Parity, SvgPipelinePreset::Readable] {
            let output = policy(preset)
                .process_with_metadata(LEGACY_SVG, &metadata(LEGACY_SVG), &session())
                .unwrap();
            assert_eq!(output, LEGACY_SVG);
        }
    }

    #[test]
    fn timeline_marker_repair_requires_renderer_owned_family_identity() {
        for metadata in [
            SvgPostprocessMetadata::from_svg(LEGACY_SVG),
            SvgPostprocessMetadata::from_svg(LEGACY_SVG).with_diagram_type("timeline"),
            SvgPostprocessMetadata::from_svg(LEGACY_SVG).with_family_id(DiagramFamilyId::GIT_GRAPH),
        ] {
            let error = policy(SvgPipelinePreset::ResvgSafe)
                .process_resvg_compatible_with_metadata(LEGACY_SVG, &metadata, &session())
                .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("missing same-document fragment #arrowhead")
            );
        }
        let sealed = SvgPipeline::resvg_safe()
            .process_resvg_compatible_with_metadata(LEGACY_SVG, &metadata(LEGACY_SVG), &session())
            .unwrap();
        assert_marker_references_resolve(sealed.as_str(), LEGACY_MARKER_ID);
    }

    #[test]
    fn absent_or_lookalike_marker_definitions_are_not_guessed() {
        for definition in [
            "",
            r#"<path id="undefined-arrowhead"/>"#,
            r#"<marker id="round9-probe-undefined-arrowhead"/>"#,
            r#"<marker id="undefined-arrowhead"/><marker id="undefined-arrowhead"/>"#,
            r#"<marker id="undefined-arrowhead"/><path id="undefined-arrowhead" d="M0 0L1 1"/>"#,
            r#"<!-- <marker id="undefined-arrowhead"/> -->"#,
            r#"<![CDATA[<marker id="undefined-arrowhead"/>]]>"#,
            r#"<g data-marker='id="undefined-arrowhead"'/>"#,
        ] {
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg">{definition}<line marker-end="url(#arrowhead)"/></svg>"#
            );
            assert_missing_reference(&svg, "arrowhead");
        }
    }

    #[test]
    fn unrelated_references_are_still_terminal_errors() {
        for (extra, fragment) in [
            (r#"<line marker-end="url(#missing)"/>"#, "missing"),
            (
                r#"<path d="M0 0L10 10" marker-end="url(#arrowhead)"/>"#,
                "arrowhead",
            ),
            (r##"<use href="#missing-use"/>"##, "missing-use"),
            (r##"<use href="#arrowhead"/>"##, "arrowhead"),
        ] {
            let svg = LEGACY_SVG.replace("</svg>", &format!("{extra}</svg>"));
            assert_missing_reference(&svg, fragment);
        }
    }

    #[test]
    fn existing_arrowhead_and_horizontal_timeline_markers_are_unchanged() {
        for svg in [
            LEGACY_SVG.replace(
                "</defs>",
                r#"<marker id="arrowhead"><path d="M0 0h1v1z"/></marker></defs>"#,
            ),
            LEGACY_SVG
                .replace(LEGACY_MARKER_ID, "round9-probe-arrowhead")
                .replace(LEGACY_REFERENCE, "url(#round9-probe-arrowhead)"),
        ] {
            let sealed = policy(SvgPipelinePreset::ResvgSafe)
                .process_resvg_compatible_with_metadata(&svg, &metadata(&svg), &session())
                .unwrap();
            assert_eq!(sealed.as_str(), svg);
        }
    }

    #[cfg(feature = "diagram-timeline")]
    #[test]
    fn rendered_vertical_timeline_finalizes_through_public_output_policy() {
        use crate::LayoutOptions;
        use crate::diagram_theme::{
            DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
        };
        use crate::svg::{SvgDebugOptions, SvgRenderOptions};
        use merman_core::{MermaidEngine, ParseOptions};

        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_typography(
                    TypographySpec::default().with_family_style(
                        DiagramFamilyId::TIMELINE,
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("Cousine").unwrap()),
                    ),
                ),
            )
            .unwrap();
        let parsed = crate::test_support::install_parse_compatibility(&theme, MermaidEngine::new())
            .parse_diagram_for_render_model_sync(
                "timeline TD\nsection Work\nToday : Ship\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        for (mut pipeline, rebase) in [
            (policy(SvgPipelinePreset::ResvgSafe), false),
            (policy(SvgPipelinePreset::ResvgSafe), true),
            (SvgPipeline::resvg_safe(), false),
            (SvgPipeline::resvg_safe(), true),
        ] {
            let artifact = crate::family::prepare(
                &parsed,
                &LayoutOptions::headless_svg_defaults(),
                RenderEnvironment::deterministic()
                    .begin_session_with_theme(&theme)
                    .unwrap(),
            )
            .unwrap();
            let rendered = artifact
                .render_svg(
                    &SvgRenderOptions {
                        diagram_id: Some("round9-probe".to_owned()),
                        ..SvgRenderOptions::default()
                    },
                    &SvgDebugOptions::default(),
                )
                .unwrap();
            let parity = rendered.svg().to_owned();
            assert!(parity.contains(r#"id="undefined-arrowhead""#));
            assert!(parity.contains(LEGACY_REFERENCE));
            let rendered = rendered
                .apply_pipeline(&SvgOutputPolicy::default().pipeline())
                .unwrap();
            assert_eq!(rendered.svg(), parity);
            if rebase {
                pipeline = pipeline.with_rebased_ids("embed");
            }
            let sealed = rendered.finalize_resvg(&pipeline).unwrap();
            assert_marker_references_resolve(
                sealed.svg().as_str(),
                if rebase {
                    "embed-undefined-arrowhead"
                } else {
                    LEGACY_MARKER_ID
                },
            );
        }
    }

    #[test]
    fn marker_expansion_checks_exact_byte_ceiling_before_allocation() {
        let expected = LEGACY_SVG.replace(LEGACY_REFERENCE, RESOLVED_REFERENCE);
        for maximum in [expected.len(), expected.len() - 1] {
            let session = RenderEnvironment::deterministic()
                .with_resource_policy(
                    RenderResourcePolicy::unbounded_for_trusted_input()
                        .with_limit(ResourceLimitId::MaxSvgBytes, maximum)
                        .unwrap(),
                )
                .begin_session()
                .unwrap();
            let metadata = metadata(LEGACY_SVG);
            let result = resolve_timeline_marker_references(
                Cow::Borrowed(LEGACY_SVG),
                &metadata,
                SvgPostprocessExecution::new(&session),
            );
            if maximum == expected.len() {
                assert_eq!(result.unwrap(), expected);
            } else {
                let Error::ResourceLimitExceeded(error) = result.unwrap_err() else {
                    panic!("expected the marker expansion byte ceiling");
                };
                assert_eq!(error.phase, ResourceLimitPhase::SvgPostprocess);
                assert_eq!(error.limit, "max_svg_bytes");
                assert_eq!(error.actual, expected.len());
            }
        }
    }

    #[test]
    fn marker_scan_preserves_cancellation_during_long_attributes_and_comments() {
        use merman_model::{OperationControl, OperationPhase};
        for svg in [
            format!(r#"<svg data-long="{}"/>"#, "x".repeat(16384)),
            format!("<svg><!-- {} --></svg>", "x".repeat(16384)),
            format!("<svg>{}</svg>", "<g/>".repeat(1024)),
        ] {
            let control = OperationControl::new();
            let session = RenderEnvironment::deterministic()
                .begin_session_with_control(control.clone())
                .unwrap();
            let metadata = metadata(&svg);
            control.cancel_after_checkpoints(8);
            let error = resolve_timeline_marker_references(
                Cow::Borrowed(&svg),
                &metadata,
                SvgPostprocessExecution::new(&session),
            )
            .unwrap_err();
            let Error::Cancelled(cancelled) = error else {
                panic!("expected cooperative cancellation, got {error}");
            };
            assert_eq!(cancelled.phase, OperationPhase::Postprocess);
        }
    }
}
