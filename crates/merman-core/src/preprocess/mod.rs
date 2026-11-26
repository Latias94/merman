mod config_scan;
mod directives;
mod frontmatter;

#[cfg(feature = "presentation")]
mod full;

#[cfg(feature = "presentation")]
pub(crate) use full::preprocess_diagram_with_known_type_and_directive_recovery_controlled;
#[cfg(all(feature = "presentation", feature = "editor-facts"))]
pub(crate) use full::preprocess_diagram_with_known_type_and_directive_recovery_evidence_controlled;
#[cfg(feature = "presentation")]
pub use full::{preprocess_diagram, preprocess_diagram_with_known_type};

#[cfg(feature = "editor-facts")]
mod editor;
#[cfg(feature = "editor-facts")]
mod rewrite_order;
#[cfg(feature = "editor-facts")]
mod source_config;
mod source_edit_map;
mod text;

pub(crate) use directives::directive_removal_ranges_controlled;
pub use frontmatter::{
    parse_frontmatter_yaml_fields, parse_frontmatter_yaml_fields_bounded_controlled,
    parse_frontmatter_yaml_fields_controlled,
};
#[cfg(feature = "editor-facts")]
#[doc(hidden)]
pub use rewrite_order::{
    FrontmatterInsertOrder, FrontmatterRewrite, SourceConfigRewriteOrder,
    source_config_rewrite_order_controlled,
};
#[cfg(feature = "editor-facts")]
pub(crate) use source_config::SourceConfigPath;
#[cfg(feature = "editor-facts")]
pub use source_config::{
    FrontmatterSourceEvidence, SourceAppearanceDeclaration, SourceAppearanceSummary,
    SourceConfigEvidence, SourceConfigKeyEvidence, SourceConfigOrigin, SourceDirectiveEvidence,
};
pub use source_edit_map::PreprocessedSource;

#[cfg(feature = "editor-facts")]
use directives::append_directive_evidence;
use directives::{DiagramTypeDetector, process_directives_controlled};
#[cfg(feature = "editor-facts")]
use frontmatter::append_frontmatter_evidence;
use frontmatter::process_frontmatter_controlled;

#[cfg(feature = "editor-facts")]
use crate::EditorExpectedSyntaxKind;
#[cfg(feature = "editor-facts")]
use crate::SourceSpan;
use crate::{
    Error, MermaidConfig, OperationControl, OperationControlResult, Result, diagram::CapturedPanic,
};
use merman_core_preprocess::SourceEdit;
pub(crate) use merman_core_preprocess::directives::DirectiveRecoveryMode;
use merman_core_preprocess::directives::directive_blocks_controlled;
pub use merman_core_preprocess::frontmatter::{
    FrontmatterBlock, FrontmatterBlockLocation, FrontmatterByteSpan,
    locate_frontmatter_block_controlled, split_frontmatter_block,
    split_frontmatter_block_controlled,
};
use merman_core_preprocess::text::PreprocessMarkers;
use serde_json::Value;
#[cfg(feature = "editor-facts")]
use source_config::SourceAppearanceOwner;
#[cfg(test)]
use std::cell::Cell;
#[cfg(feature = "editor-facts")]
use text::editor_evidence_line_end;
use text::{
    cleanup_text_controlled, encode_mermaid_entities_like_upstream_controlled,
    remove_mermaid_comments_controlled, trim_source_start_controlled,
};

#[cfg(test)]
thread_local! {
    static PREPROCESS_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(all(test, feature = "all-diagrams"))]
pub(crate) fn reset_preprocess_count() {
    PREPROCESS_COUNT.set(0);
}

#[cfg(all(test, feature = "all-diagrams"))]
pub(crate) fn preprocess_count() -> usize {
    PREPROCESS_COUNT.get()
}

#[derive(Debug, Clone)]
pub struct PreprocessResult {
    pub source: PreprocessedSource,
    pub title: Option<String>,
    pub config: MermaidConfig,
    pub(crate) with_comments: Option<PreprocessedSource>,
}

#[derive(Debug)]
pub(crate) struct PreprocessCaptureOutcome {
    pub(crate) outcome: PreprocessCaptureResult,
    #[cfg(feature = "editor-facts")]
    pub(crate) source_config: SourceConfigEvidence,
}

#[derive(Debug)]
pub(crate) enum PreprocessCaptureResult {
    Ready(Box<PreprocessResult>),
    Failed(Error),
    Panicked(CapturedPanic),
}

impl PreprocessCaptureOutcome {
    fn into_result(self) -> Result<PreprocessResult> {
        match self.outcome {
            PreprocessCaptureResult::Ready(result) => Ok(*result),
            PreprocessCaptureResult::Failed(error) => Err(error),
            PreprocessCaptureResult::Panicked(panic) => {
                std::panic::resume_unwind(panic.into_payload())
            }
        }
    }
}

#[cfg(feature = "editor-facts")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceConfigCaptureMode {
    Omit,
    CollectRecovery,
    Collect,
}

#[cfg(feature = "editor-facts")]
impl SourceConfigCaptureMode {
    const fn collects(self) -> bool {
        !matches!(self, Self::Omit)
    }

    const fn collects_editor(self) -> bool {
        matches!(self, Self::Collect)
    }
}

#[cfg(feature = "editor-facts")]
#[derive(Debug)]
struct LocalConfigKeyEvidence {
    path: SourceConfigPath,
    span: Option<SourceSpan>,
    rewrite_safe: bool,
}

impl PreprocessResult {
    pub fn code(&self) -> &str {
        self.source.text()
    }
}

pub(crate) fn select_family_parser_source_controlled(
    original: &str,
    mut source: PreprocessedSource,
    with_comments: Option<PreprocessedSource>,
    diagram_type: &str,
    config: &MermaidConfig,
    control: &OperationControl,
) -> OperationControlResult<PreprocessedSource> {
    control.checkpoint()?;
    if matches!(diagram_type, "agentflow" | "eventmodeling")
        && let Some(mut with_comments) = with_comments
    {
        let max_text_size = config
            .get("maxTextSize")
            .and_then(Value::as_u64)
            .unwrap_or(50_000);
        if with_comments.text().encode_utf16().count() as u64 <= max_text_size {
            encode_mermaid_entities_like_upstream_controlled(&mut with_comments, control)?;
            return Ok(with_comments);
        }
    }
    if cfg!(feature = "diagram-agentflow")
        && diagram_type == "agentflow"
        && !source.text().is_empty()
    {
        source.rebase_parser_start_controlled(original, control)?;
    }
    Ok(source)
}

/// Runs the common Mermaid source pipeline using only the pinned built-in detection facts.
pub(crate) fn preprocess_builtin_semantic_controlled(
    input: &str,
    diagram_type: Option<&str>,
    control: &OperationControl,
) -> OperationControlResult<Result<PreprocessResult>> {
    preprocess_builtin_semantic_with_directive_recovery_controlled(
        input,
        diagram_type,
        DirectiveRecoveryMode::Strict,
        control,
    )
}

pub(crate) fn preprocess_builtin_semantic_with_directive_recovery_controlled(
    input: &str,
    diagram_type: Option<&str>,
    directive_recovery: DirectiveRecoveryMode,
    control: &OperationControl,
) -> OperationControlResult<Result<PreprocessResult>> {
    let detector = |text: &str, control: &OperationControl| {
        Ok(crate::detect::detect_builtin_controlled(text, control)?.map(|fact| fact.id))
    };
    Ok(preprocess_source_capture_controlled(
        input,
        &detector,
        diagram_type,
        directive_recovery,
        #[cfg(feature = "editor-facts")]
        SourceConfigCaptureMode::Omit,
        control,
    )?
    .into_result())
}

#[cfg(feature = "editor-facts")]
pub(crate) fn preprocess_builtin_semantic_with_evidence_controlled(
    input: &str,
    diagram_type: Option<&str>,
    editor_demand: crate::family::EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<PreprocessCaptureOutcome> {
    let detector = |text: &str, control: &OperationControl| {
        Ok(crate::detect::detect_builtin_controlled(text, control)?.map(|fact| fact.id))
    };
    preprocess_source_capture_controlled(
        input,
        &detector,
        diagram_type,
        DirectiveRecoveryMode::RecoverLine,
        if editor_demand.captures_rich() {
            SourceConfigCaptureMode::Collect
        } else {
            SourceConfigCaptureMode::CollectRecovery
        },
        control,
    )
}

fn preprocess_source_capture_controlled(
    input: &str,
    detector: &DiagramTypeDetector<'_>,
    diagram_type: Option<&str>,
    directive_recovery: DirectiveRecoveryMode,
    #[cfg(feature = "editor-facts")] capture_mode: SourceConfigCaptureMode,
    control: &OperationControl,
) -> OperationControlResult<PreprocessCaptureOutcome> {
    #[cfg(test)]
    PREPROCESS_COUNT.set(PREPROCESS_COUNT.get() + 1);

    // Mermaid 12 passes the complete DiagramCode to Diagram.fromText on both public paths.
    #[cfg(feature = "editor-facts")]
    let capture_spans = capture_mode.collects();
    #[cfg(not(feature = "editor-facts"))]
    let capture_spans = false;
    let mut source = PreprocessedSource::new_controlled(input, capture_spans, control)?;
    // The cleanup passes add only newlines and single quotes. Later config and comment
    // removal passes delete text, so the original marker set covers all of their inputs.
    let markers = PreprocessMarkers::scan(input, control)?;
    #[cfg(feature = "editor-facts")]
    let capture_editor_evidence = capture_mode.collects_editor();
    cleanup_text_controlled(&mut source, markers, control)?;
    #[cfg(feature = "editor-facts")]
    let mut source_config = SourceConfigEvidence::empty();
    #[cfg(feature = "editor-facts")]
    if capture_mode.collects() {
        source_config.appearance_complete = true;
    }
    #[cfg(feature = "editor-facts")]
    if capture_mode.collects() {
        if let Some(config_insert_span) = source.try_map_span(SourceSpan::new(0, 0)) {
            source_config.set_config_insert_span(config_insert_span);
        } else {
            source_config.mark_rewrite_unsafe();
        }
    }
    let frontmatter = process_frontmatter_controlled(
        source.text(),
        #[cfg(feature = "editor-facts")]
        capture_mode,
        control,
    )?;
    #[cfg(feature = "editor-facts")]
    if capture_mode.collects() {
        append_frontmatter_evidence(&source, &frontmatter, &mut source_config, control)?;
        if let (Some(full), Ok(_)) = (frontmatter.full, &frontmatter.result) {
            if let Some(construct_span) = source.try_map_span(SourceSpan::new(full.start, full.end))
            {
                source_config.appearance_capture.record(
                    &[],
                    SourceAppearanceOwner {
                        origin: SourceConfigOrigin::Frontmatter,
                        construct_span,
                    },
                );
            } else {
                source_config.appearance_complete = false;
            }
        }
    }
    let frontmatter_len = frontmatter.full.map_or(0, |full| full.end);
    let mut first_error = None;
    let (title, mut frontmatter_config) = match frontmatter.result {
        Ok(processed) => (processed.title, processed.config),
        Err(error) => {
            control.checkpoint()?;
            #[cfg(feature = "editor-facts")]
            {
                source_config.mark_rewrite_unsafe();
                if capture_mode.collects() {
                    first_error = Some(error);
                    (None, MermaidConfig::empty_object())
                } else {
                    return Ok(PreprocessCaptureOutcome {
                        outcome: PreprocessCaptureResult::Failed(error),
                        source_config,
                    });
                }
            }
            #[cfg(not(feature = "editor-facts"))]
            return Ok(PreprocessCaptureOutcome {
                outcome: PreprocessCaptureResult::Failed(error),
            });
        }
    };
    if frontmatter_len > 0 {
        #[cfg(feature = "editor-facts")]
        if capture_editor_evidence {
            let expected_end = editor_evidence_line_end(source.text(), frontmatter_len);
            source.record_global_expected_syntax(
                EditorExpectedSyntaxKind::Frontmatter,
                SourceSpan::new(0, expected_end),
            );
        }
        source.record_frontmatter_removal(frontmatter_len);
        source.apply_edits(vec![SourceEdit::delete(0..frontmatter_len)], control)?;
    }

    control.checkpoint()?;
    let directive_blocks = if markers.contains(PreprocessMarkers::PERCENT) {
        directive_blocks_controlled(source.text(), directive_recovery, control)?
    } else {
        Vec::new()
    };
    let processed_directives = process_directives_controlled(
        source.text(),
        detector,
        directive_blocks,
        #[cfg(feature = "editor-facts")]
        capture_mode,
        first_error.is_none(),
        control,
    )?;
    #[cfg(feature = "editor-facts")]
    let mut processed_directives = processed_directives;
    #[cfg(feature = "editor-facts")]
    if capture_mode.collects() {
        append_directive_evidence(&source, &processed_directives, &mut source_config, control)?;
        source_config.appearance_complete &= processed_directives.appearance_complete
            && !processed_directives.recovered_incomplete_directive
            && processed_directives.appearance_capture.map_spans(&source);
    }
    if let Some(panic) = processed_directives.panic {
        control.checkpoint()?;
        return Ok(PreprocessCaptureOutcome {
            outcome: PreprocessCaptureResult::Panicked(panic),
            #[cfg(feature = "editor-facts")]
            source_config,
        });
    }
    if let Some(error) = processed_directives.error {
        #[cfg(feature = "editor-facts")]
        source_config.mark_rewrite_unsafe();
        if first_error.is_none() {
            first_error = Some(error);
        }
    }
    if let Some(error) = first_error {
        control.checkpoint()?;
        return Ok(PreprocessCaptureOutcome {
            outcome: PreprocessCaptureResult::Failed(error),
            #[cfg(feature = "editor-facts")]
            source_config,
        });
    }
    #[cfg(feature = "editor-facts")]
    if processed_directives.recovered_incomplete_directive {
        source.mark_recovered_incomplete_directive();
    }
    #[cfg(feature = "editor-facts")]
    if capture_editor_evidence {
        for prefix in processed_directives.editor_prefixes.iter().cloned() {
            source.record_global_directive_prefix(prefix);
        }
        for removal in &processed_directives.removals {
            source.record_global_expected_syntax(
                EditorExpectedSyntaxKind::Directive,
                SourceSpan::new(removal.start, removal.end),
            );
        }
    }
    source.apply_edits(
        processed_directives
            .removals
            .into_iter()
            .map(SourceEdit::delete)
            .collect(),
        control,
    )?;

    #[cfg(feature = "editor-facts")]
    if capture_mode.collects() {
        source_config.appearance_capture.merge(
            &mut frontmatter_config,
            &processed_directives.config,
            &processed_directives.appearance_capture,
        );
        source_config
            .appearance_capture
            .normalize_font_alias(&frontmatter_config);
    } else {
        frontmatter_config.deep_merge(processed_directives.config.as_value());
    }
    #[cfg(not(feature = "editor-facts"))]
    frontmatter_config.deep_merge(processed_directives.config.as_value());

    control.checkpoint()?;
    // These families opt into Mermaid's comment-preserving parser input. Keep the existing
    // edit map with that buffer so editor byte spans and domain lexer positions remain distinct.
    let preserve_comments = match diagram_type {
        Some(diagram_type) => matches!(diagram_type, "agentflow" | "eventmodeling"),
        None => {
            let header = source
                .text()
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty() && !line.starts_with("%%"))
                .unwrap_or("");
            crate::detect::detector_agentflow(header)
                || crate::detect::detector_eventmodeling(header)
        }
    };
    let with_comments = preserve_comments.then(|| source.clone());
    #[cfg(feature = "editor-facts")]
    let mut with_comments = with_comments;
    if markers.contains(PreprocessMarkers::PERCENT) {
        remove_mermaid_comments_controlled(
            &mut source,
            #[cfg(feature = "editor-facts")]
            capture_editor_evidence,
            control,
        )?;
    }
    trim_source_start_controlled(&mut source, control)?;
    #[cfg(feature = "editor-facts")]
    if let Some(with_comments) = &mut with_comments {
        with_comments.copy_global_editor_evidence_from(&source);
    }
    control.checkpoint()?;
    if markers.contains(PreprocessMarkers::ENTITY) {
        encode_mermaid_entities_like_upstream_controlled(&mut source, control)?;
    }
    control.checkpoint()?;
    Ok(PreprocessCaptureOutcome {
        outcome: PreprocessCaptureResult::Ready(Box::new(PreprocessResult {
            source,
            title,
            config: frontmatter_config,
            with_comments,
        })),
        #[cfg(feature = "editor-facts")]
        source_config,
    })
}

pub fn diagram_config_key_for_type(diagram_type: &str) -> &str {
    crate::family::config_namespace_for_diagram_type(diagram_type).unwrap_or(diagram_type)
}

#[cfg(test)]
mod tests;
