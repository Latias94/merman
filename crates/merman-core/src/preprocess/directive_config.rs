//! Mermaid directive config evaluation and source evidence.

mod sanitize;

use super::config_scan::{MAX_CONFIG_NESTING_DEPTH, config_nesting_exceeds_limit_controlled};
#[cfg(feature = "editor-facts")]
use super::source_config::{self, SourceAppearanceCapture, SourceAppearanceOwner};
#[cfg(feature = "editor-facts")]
use super::{
    LocalConfigKeyEvidence, PreprocessedSource, SourceConfigCaptureMode, SourceConfigEvidence,
    SourceConfigKeyEvidence, SourceConfigOrigin, SourceDirectiveEvidence,
};
#[cfg(feature = "editor-facts")]
use crate::SourceSpan;
use crate::{
    Error, MermaidConfig, OperationControl, OperationControlResult, Result, diagram::CapturedPanic,
};
use merman_core_preprocess::directives::{
    DirectiveArgument, DirectiveBlock, ScannedDirective, normalize_directive_quotes_controlled,
    scan_directive_body,
};
#[cfg(test)]
use merman_core_preprocess::directives::{DirectiveRecoveryMode, directive_blocks_controlled};
use merman_core_preprocess::scan::controlled::{ControlledScanCheckpoints, push_str_controlled};
#[cfg(test)]
pub(super) use sanitize::sanitize_directive;
pub(super) use sanitize::sanitize_directive_controlled;
use serde_json::Value;
use std::borrow::Cow;

pub(super) const MAX_DIRECTIVE_CONFIG_PARSE_BYTES: usize = 1024 * 1024;

pub(super) type DiagramTypeDetector<'a> =
    dyn Fn(&str, &OperationControl) -> OperationControlResult<Result<&'static str>> + 'a;

#[cfg(feature = "editor-facts")]
pub(super) fn append_directive_evidence(
    source: &PreprocessedSource,
    captured: &ProcessedDirectives,
    evidence: &mut SourceConfigEvidence,
    control: &OperationControl,
) -> OperationControlResult<()> {
    for (index, directive) in captured.evidence.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        let Some(full_span) = source.try_map_span(directive.full_span) else {
            evidence.mark_rewrite_unsafe();
            continue;
        };
        let Some(keyword_span) = source.try_map_span(directive.keyword_span) else {
            evidence.mark_rewrite_unsafe();
            continue;
        };
        let directive_index = evidence.push_directive(SourceDirectiveEvidence::new(
            directive.keyword.clone(),
            full_span,
            keyword_span,
            evidence.directives().len(),
            directive.complete,
            directive.rewrite_safe,
        ));
        for key in &directive.keys {
            let Some(local_span) = key.span else {
                continue;
            };
            let exact_span = source.try_map_span(local_span);
            let Some(span) = exact_span.or_else(|| source.try_map_enclosing_span(local_span))
            else {
                continue;
            };
            evidence.push_key(SourceConfigKeyEvidence::new(
                SourceConfigOrigin::Directive { directive_index },
                key.path.clone(),
                span,
                evidence.keys().len(),
                key.rewrite_safe && exact_span.is_some(),
            ));
        }
    }
    evidence.set_legacy_init_namespace(captured.legacy_init_namespace);
    if captured.recovered_incomplete_directive {
        evidence.mark_rewrite_unsafe();
    }
    control.checkpoint()
}

pub(super) struct ProcessedDirectives {
    pub(super) config: MermaidConfig,
    pub(super) removals: Vec<std::ops::Range<usize>>,
    #[cfg(feature = "editor-facts")]
    pub(super) editor_prefixes: Vec<String>,
    #[cfg(feature = "editor-facts")]
    pub(super) recovered_incomplete_directive: bool,
    #[cfg(feature = "editor-facts")]
    evidence: Vec<LocalDirectiveEvidence>,
    #[cfg(feature = "editor-facts")]
    legacy_init_namespace: Option<&'static str>,
    pub(super) error: Option<Error>,
    pub(super) panic: Option<CapturedPanic>,
    #[cfg(feature = "editor-facts")]
    pub(super) appearance_capture: SourceAppearanceCapture,
    #[cfg(feature = "editor-facts")]
    pub(super) appearance_complete: bool,
}

#[cfg(feature = "editor-facts")]
struct LocalDirectiveEvidence {
    keyword: String,
    full_span: SourceSpan,
    keyword_span: SourceSpan,
    complete: bool,
    rewrite_safe: bool,
    keys: Vec<LocalConfigKeyEvidence>,
}

pub(super) fn process_directives_controlled(
    input: &str,
    detector: &DiagramTypeDetector<'_>,
    blocks: Vec<DirectiveBlock>,
    #[cfg(feature = "editor-facts")] capture_mode: SourceConfigCaptureMode,
    evaluate_config: bool,
    control: &OperationControl,
) -> OperationControlResult<ProcessedDirectives> {
    control.checkpoint()?;
    #[cfg(feature = "editor-facts")]
    let mut appearance_capture = SourceAppearanceCapture::default();
    #[cfg(feature = "editor-facts")]
    let mut legacy_init_namespace = None;
    #[cfg(feature = "editor-facts")]
    let mut appearance_complete = true;
    if blocks.is_empty() {
        return Ok(ProcessedDirectives {
            config: MermaidConfig::empty_object(),
            removals: Vec::new(),
            #[cfg(feature = "editor-facts")]
            editor_prefixes: Vec::new(),
            #[cfg(feature = "editor-facts")]
            recovered_incomplete_directive: false,
            #[cfg(feature = "editor-facts")]
            evidence: Vec::new(),
            #[cfg(feature = "editor-facts")]
            legacy_init_namespace: None,
            error: None,
            panic: None,
            #[cfg(feature = "editor-facts")]
            appearance_capture,
            #[cfg(feature = "editor-facts")]
            appearance_complete,
        });
    }
    #[cfg(feature = "editor-facts")]
    let recovered_incomplete_directive = blocks.iter().any(|block| !block.complete);
    let mut directives = Vec::new();
    #[cfg(feature = "editor-facts")]
    let mut evidence = Vec::new();
    let mut first_error = None;
    for (index, block) in blocks.iter().enumerate() {
        if index % 32 == 0 {
            control.checkpoint()?;
        }
        if !block.complete {
            #[cfg(feature = "editor-facts")]
            if !capture_mode.collects() {
                continue;
            }
            #[cfg(not(feature = "editor-facts"))]
            continue;
        }
        let parsed = parse_directive_like_upstream_capture_controlled(
            &input[block.raw.clone()],
            #[cfg(feature = "editor-facts")]
            capture_mode,
            #[cfg(feature = "editor-facts")]
            block.complete,
            control,
        )?;
        if let Some(directive) = parsed.directive {
            #[cfg(feature = "editor-facts")]
            let mut directive = directive;
            #[cfg(feature = "editor-facts")]
            if matches!(directive.ty.as_str(), "init" | "initialize") {
                appearance_complete &= directive.arguments_complete;
            }
            #[cfg(feature = "editor-facts")]
            if capture_mode.collects() {
                directive.appearance_owner = Some(SourceAppearanceOwner {
                    origin: SourceConfigOrigin::Directive {
                        directive_index: evidence.len(),
                    },
                    construct_span: SourceSpan::new(block.range.start, block.range.end),
                });
                let raw_start = block.raw.start;
                let keyword_span = SourceSpan::new(
                    raw_start.saturating_add(directive.keyword_span.start),
                    raw_start.saturating_add(directive.keyword_span.end),
                );
                let keys = directive
                    .config_keys
                    .iter()
                    .map(|key| LocalConfigKeyEvidence {
                        path: key.path.clone(),
                        span: key.span.as_ref().map(|span| {
                            SourceSpan::new(
                                raw_start.saturating_add(span.start),
                                raw_start.saturating_add(span.end),
                            )
                        }),
                        rewrite_safe: key.rewrite_safe,
                    })
                    .collect();
                evidence.push(LocalDirectiveEvidence {
                    keyword: directive.ty.clone(),
                    full_span: SourceSpan::new(block.range.start, block.range.end),
                    keyword_span,
                    complete: block.complete,
                    rewrite_safe: block.complete && directive.rewrite_safe,
                    keys,
                });
            }
            if block.complete && parsed.error.is_none() {
                directives.push(directive);
            }
        }
        if block.complete && first_error.is_none() {
            first_error = parsed.error;
        }
        if first_error.is_some() {
            #[cfg(feature = "editor-facts")]
            if capture_mode.collects() {
                continue;
            }
            return Ok(ProcessedDirectives {
                config: MermaidConfig::empty_object(),
                removals: Vec::new(),
                #[cfg(feature = "editor-facts")]
                editor_prefixes: Vec::new(),
                #[cfg(feature = "editor-facts")]
                recovered_incomplete_directive,
                #[cfg(feature = "editor-facts")]
                evidence,
                #[cfg(feature = "editor-facts")]
                legacy_init_namespace: None,
                error: first_error,
                panic: None,
                #[cfg(feature = "editor-facts")]
                appearance_capture,
                #[cfg(feature = "editor-facts")]
                appearance_complete: false,
            });
        }
    }
    control.checkpoint()?;
    let input_without_directives = remove_directive_blocks_controlled(input, &blocks, control)?;
    let wrap = directives.iter().any(|d| d.ty == "wrap");
    #[cfg(feature = "editor-facts")]
    let mut editor_prefixes = Vec::new();
    #[cfg(feature = "editor-facts")]
    if capture_mode.collects_editor() {
        for (index, directive) in directives.iter().enumerate() {
            if index % 32 == 0 {
                control.checkpoint()?;
            }
            if matches!(directive.ty.as_str(), "init" | "initialize" | "wrap")
                && !editor_prefixes.contains(&directive.ty)
            {
                editor_prefixes.push(directive.ty.clone());
            }
        }
    }

    let detected_init = if !evaluate_config || first_error.is_some() {
        Ok(MermaidConfig::empty_object())
    } else {
        let detect_init = || {
            detect_init_controlled(
                &directives,
                input_without_directives.as_ref(),
                detector,
                #[cfg(feature = "editor-facts")]
                &mut appearance_capture,
                #[cfg(feature = "editor-facts")]
                &mut legacy_init_namespace,
                control,
            )
        };
        #[cfg(feature = "editor-facts")]
        let mut detect_init = detect_init;
        #[cfg(feature = "editor-facts")]
        {
            if capture_mode.collects() {
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(detect_init)) {
                    Ok(result) => result?,
                    Err(payload) => {
                        return Ok(ProcessedDirectives {
                            config: MermaidConfig::empty_object(),
                            removals: blocks.into_iter().map(|block| block.range).collect(),
                            #[cfg(feature = "editor-facts")]
                            editor_prefixes,
                            #[cfg(feature = "editor-facts")]
                            recovered_incomplete_directive,
                            #[cfg(feature = "editor-facts")]
                            evidence,
                            #[cfg(feature = "editor-facts")]
                            legacy_init_namespace: None,
                            error: first_error,
                            panic: Some(CapturedPanic::from_payload(payload)),
                            #[cfg(feature = "editor-facts")]
                            appearance_capture,
                            #[cfg(feature = "editor-facts")]
                            appearance_complete: false,
                        });
                    }
                }
            } else {
                detect_init()?
            }
        }
        #[cfg(not(feature = "editor-facts"))]
        {
            detect_init()?
        }
    };
    let init = match detected_init {
        Ok(init) => init,
        Err(error) => {
            if first_error.is_none() {
                first_error = Some(error);
            }
            MermaidConfig::empty_object()
        }
    };
    let mut merged = init;
    if wrap {
        merged.set_value("wrap", Value::Bool(true));
    }

    control.checkpoint()?;
    Ok(ProcessedDirectives {
        config: merged,
        removals: blocks.into_iter().map(|block| block.range).collect(),
        #[cfg(feature = "editor-facts")]
        editor_prefixes,
        #[cfg(feature = "editor-facts")]
        recovered_incomplete_directive,
        #[cfg(feature = "editor-facts")]
        evidence,
        #[cfg(feature = "editor-facts")]
        legacy_init_namespace,
        error: first_error,
        panic: None,
        #[cfg(feature = "editor-facts")]
        appearance_capture,
        #[cfg(feature = "editor-facts")]
        appearance_complete,
    })
}

pub(super) fn detect_init_controlled(
    directives: &[Directive],
    input: &str,
    detector: &DiagramTypeDetector<'_>,
    #[cfg(feature = "editor-facts")] appearance_capture: &mut SourceAppearanceCapture,
    #[cfg(feature = "editor-facts")] legacy_init_namespace: &mut Option<&'static str>,
    control: &OperationControl,
) -> OperationControlResult<Result<MermaidConfig>> {
    control.checkpoint()?;
    let mut merged = MermaidConfig::empty_object();
    let multiple_inits = directives
        .iter()
        .filter(|directive| matches!(directive.ty.as_str(), "init" | "initialize"))
        .take(2)
        .count()
        > 1;

    for (index, d) in directives.iter().enumerate() {
        if index % 16 == 0 {
            control.checkpoint()?;
        }
        if d.ty != "init" && d.ty != "initialize" {
            continue;
        }

        let mut args = match &d.args {
            Some(v) => crate::json_value::clone_value_nonrecursive(v),
            None => Value::Object(Default::default()),
        };
        // Mermaid sanitizes multiple init arguments before merging, so their legacy `config`
        // key is filtered out. A single init remaps that key before source sanitization.
        if !multiple_inits
            && let Some(diagram_specific_value) = args
                .as_object_mut()
                .and_then(|object| object.remove("config"))
        {
            // Legacy init uses the source detector ID, even when a parser type was supplied.
            // Mermaid only remaps flowchart-v2; other IDs still pass through source admission.
            if let Ok(ty) = detector(input, control)? {
                let key = if ty == "flowchart-v2" {
                    "flowchart"
                } else {
                    ty
                };
                if let Value::Object(obj) = &mut args {
                    if let Some(old) = obj.insert(key.to_string(), diagram_specific_value) {
                        crate::json_value::drop_value_nonrecursive(old);
                    }
                    #[cfg(feature = "editor-facts")]
                    {
                        *legacy_init_namespace = Some(key);
                    }
                }
            } else {
                crate::json_value::drop_value_nonrecursive(diagram_specific_value);
            }
        }
        sanitize_directive_controlled(&mut args, control)?;
        crate::config::mirror_legacy_font_family_into_theme_variables_value(&mut args);

        #[cfg(feature = "editor-facts")]
        merged.merge_with_mutation_observer(&args, &mut |path| {
            if let Some(owner) = d.appearance_owner {
                appearance_capture.record(path, owner);
            }
        });
        #[cfg(not(feature = "editor-facts"))]
        merged.deep_merge(&args);
    }

    control.checkpoint()?;
    Ok(Ok(merged))
}

#[derive(Debug, Clone)]
pub(super) struct Directive {
    ty: String,
    pub(super) args: Option<Value>,
    #[cfg(feature = "editor-facts")]
    keyword_span: std::ops::Range<usize>,
    #[cfg(feature = "editor-facts")]
    config_keys: Vec<source_config::Json5ConfigKeyEvidence>,
    #[cfg(feature = "editor-facts")]
    rewrite_safe: bool,
    #[cfg(feature = "editor-facts")]
    appearance_owner: Option<SourceAppearanceOwner>,
    #[cfg(feature = "editor-facts")]
    arguments_complete: bool,
}

fn remove_directive_blocks_controlled<'a>(
    input: &'a str,
    blocks: &[DirectiveBlock],
    control: &OperationControl,
) -> OperationControlResult<Cow<'a, str>> {
    if blocks.is_empty() {
        control.checkpoint()?;
        return Ok(Cow::Borrowed(input));
    }

    let mut retained_bytes = input.len();
    for (index, block) in blocks.iter().enumerate() {
        if index.is_multiple_of(32) {
            control.checkpoint()?;
        }
        retained_bytes = retained_bytes.saturating_sub(block.range.len());
    }
    let mut output = String::with_capacity(retained_bytes);
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    let mut cursor = 0usize;
    for block in blocks {
        push_str_controlled(
            &mut output,
            &input[cursor..block.range.start],
            &mut checkpoints,
        )?;
        cursor = block.range.end;
    }
    push_str_controlled(&mut output, &input[cursor..], &mut checkpoints)?;
    checkpoints.finish()?;
    Ok(Cow::Owned(output))
}

#[cfg(test)]
pub(super) fn parse_directive_like_upstream(raw: &str) -> Result<Option<Directive>> {
    let control = OperationControl::new();
    parse_directive_like_upstream_controlled(
        raw,
        #[cfg(feature = "editor-facts")]
        SourceConfigCaptureMode::Omit,
        &control,
    )
    .expect("a private parse control cannot be cancelled")
}

#[cfg(test)]
fn parse_directive_like_upstream_controlled(
    raw: &str,
    #[cfg(feature = "editor-facts")] capture_mode: SourceConfigCaptureMode,
    control: &OperationControl,
) -> OperationControlResult<Result<Option<Directive>>> {
    let captured = parse_directive_like_upstream_capture_controlled(
        raw,
        #[cfg(feature = "editor-facts")]
        capture_mode,
        #[cfg(feature = "editor-facts")]
        true,
        control,
    )?;
    Ok(match captured.error {
        Some(error) => Err(error),
        None => Ok(captured.directive),
    })
}

struct DirectiveParseCapture {
    directive: Option<Directive>,
    error: Option<Error>,
}

fn parse_directive_like_upstream_capture_controlled(
    raw: &str,
    #[cfg(feature = "editor-facts")] capture_mode: SourceConfigCaptureMode,
    #[cfg(feature = "editor-facts")] parse_arguments: bool,
    control: &OperationControl,
) -> OperationControlResult<DirectiveParseCapture> {
    // Incomplete authoring evidence only needs the directive keyword. Avoid walking an
    // arbitrarily large unfinished argument merely to normalize quotes that will not be parsed.
    #[cfg(feature = "editor-facts")]
    if !parse_arguments {
        return parse_directive_capture_controlled(raw, capture_mode, false, control);
    }
    let normalized = normalize_directive_quotes_controlled(raw, control)?;
    parse_directive_capture_controlled(
        normalized.as_ref(),
        #[cfg(feature = "editor-facts")]
        capture_mode,
        #[cfg(feature = "editor-facts")]
        true,
        control,
    )
}

#[cfg(test)]
pub(super) fn detect_directives(input: &str) -> Result<Vec<Directive>> {
    let control = OperationControl::new();
    let blocks = directive_blocks_controlled(input, DirectiveRecoveryMode::Strict, &control)
        .expect("a private parse control cannot be cancelled");
    let mut directives = Vec::new();
    for block in blocks {
        if !block.complete {
            continue;
        }
        if let Some(directive) = parse_directive_like_upstream(&input[block.raw])? {
            directives.push(directive);
        }
    }
    Ok(directives)
}

fn parse_directive_capture_controlled(
    raw: &str,
    #[cfg(feature = "editor-facts")] capture_mode: SourceConfigCaptureMode,
    #[cfg(feature = "editor-facts")] parse_arguments: bool,
    control: &OperationControl,
) -> OperationControlResult<DirectiveParseCapture> {
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    #[cfg(not(feature = "editor-facts"))]
    let parse_arguments = true;
    let Some(ScannedDirective {
        keyword: ty,
        #[cfg(feature = "editor-facts")]
        keyword_span,
        argument,
        ..
    }) = scan_directive_body(raw, parse_arguments, &mut checkpoints)?
    else {
        checkpoints.finish()?;
        return Ok(DirectiveParseCapture {
            directive: None,
            error: None,
        });
    };

    #[cfg(feature = "editor-facts")]
    if !parse_arguments {
        checkpoints.finish()?;
        return Ok(DirectiveParseCapture {
            directive: Some(Directive {
                ty,
                args: None,
                keyword_span,
                config_keys: Vec::new(),
                rewrite_safe: false,
                appearance_owner: None,
                arguments_complete: false,
            }),
            error: None,
        });
    }

    #[cfg(feature = "editor-facts")]
    let mut config_keys = Vec::new();
    #[cfg(feature = "editor-facts")]
    let mut rewrite_safe = true;
    #[cfg(feature = "editor-facts")]
    let mut arguments_complete = true;

    let args = match argument {
        Some(DirectiveArgument::Structured {
            source: rest,
            #[cfg(feature = "editor-facts")]
                offset: rest_offset,
            ..
        }) => {
            if rest.len() > MAX_DIRECTIVE_CONFIG_PARSE_BYTES {
                return Ok(DirectiveParseCapture {
                    directive: Some(Directive {
                        ty,
                        args: None,
                        #[cfg(feature = "editor-facts")]
                        keyword_span,
                        #[cfg(feature = "editor-facts")]
                        config_keys,
                        #[cfg(feature = "editor-facts")]
                        rewrite_safe: false,
                        #[cfg(feature = "editor-facts")]
                        appearance_owner: None,
                        #[cfg(feature = "editor-facts")]
                        arguments_complete: false,
                    }),
                    error: Some(Error::InvalidDirectiveJson {
                        message: format!(
                            "directive config exceeds the safe parser budget of {MAX_DIRECTIVE_CONFIG_PARSE_BYTES} bytes"
                        ),
                    }),
                });
            }
            if config_nesting_exceeds_limit_controlled(rest, control)? {
                return Ok(DirectiveParseCapture {
                    directive: Some(Directive {
                        ty,
                        args: None,
                        #[cfg(feature = "editor-facts")]
                        keyword_span,
                        #[cfg(feature = "editor-facts")]
                        config_keys,
                        #[cfg(feature = "editor-facts")]
                        rewrite_safe: false,
                        #[cfg(feature = "editor-facts")]
                        appearance_owner: None,
                        #[cfg(feature = "editor-facts")]
                        arguments_complete: false,
                    }),
                    error: Some(Error::InvalidDirectiveJson {
                        message: format!(
                            "config nesting exceeds {MAX_CONFIG_NESTING_DEPTH} levels"
                        ),
                    }),
                });
            }
            checkpoints.finish()?;
            let captured = parse_directive_config_value_controlled(
                rest,
                #[cfg(feature = "editor-facts")]
                capture_mode,
                control,
            )?;
            #[cfg(feature = "editor-facts")]
            {
                arguments_complete = captured.value.is_some();
            }
            #[cfg(feature = "editor-facts")]
            {
                rewrite_safe &= captured.rewrite_safe;
            }
            #[cfg(feature = "editor-facts")]
            {
                config_keys = captured
                    .keys
                    .into_iter()
                    .map(|mut key| {
                        key.span = key.span.map(|span| {
                            rest_offset.saturating_add(span.start)
                                ..rest_offset.saturating_add(span.end)
                        });
                        key
                    })
                    .collect();
            }
            captured.value
        }
        Some(DirectiveArgument::Literal(value)) => {
            #[cfg(feature = "editor-facts")]
            {
                rewrite_safe = false;
            }
            Some(Value::String(value))
        }
        None => None,
    };

    checkpoints.finish()?;
    Ok(DirectiveParseCapture {
        directive: Some(Directive {
            ty,
            args,
            #[cfg(feature = "editor-facts")]
            keyword_span,
            #[cfg(feature = "editor-facts")]
            config_keys,
            #[cfg(feature = "editor-facts")]
            rewrite_safe,
            #[cfg(feature = "editor-facts")]
            appearance_owner: None,
            #[cfg(feature = "editor-facts")]
            arguments_complete,
        }),
        error: None,
    })
}

struct DirectiveConfigCapture {
    value: Option<Value>,
    #[cfg(feature = "editor-facts")]
    keys: Vec<source_config::Json5ConfigKeyEvidence>,
    #[cfg(feature = "editor-facts")]
    rewrite_safe: bool,
}

fn parse_directive_config_value_controlled(
    input: &str,
    #[cfg(feature = "editor-facts")] capture_mode: SourceConfigCaptureMode,
    control: &OperationControl,
) -> OperationControlResult<DirectiveConfigCapture> {
    control.checkpoint()?;
    // `json5` has no cancellation hook. The caller enforces a hard input and nesting bound, so
    // this is a bounded atomic parser region rather than an unbounded cancellation gap.
    let captured = {
        #[cfg(feature = "editor-facts")]
        {
            if capture_mode.collects() {
                let captured = source_config::parse_json5_config(input);
                DirectiveConfigCapture {
                    value: captured.value,
                    keys: captured.keys,
                    #[cfg(feature = "editor-facts")]
                    rewrite_safe: captured.rewrite_safe,
                }
            } else {
                let value = crate::source_json::parse_json5_value(input);
                DirectiveConfigCapture {
                    #[cfg(feature = "editor-facts")]
                    rewrite_safe: value.is_some(),
                    value,
                    keys: Vec::new(),
                }
            }
        }
        #[cfg(not(feature = "editor-facts"))]
        {
            DirectiveConfigCapture {
                value: crate::source_json::parse_json5_value(input),
            }
        }
    };
    if let Err(cancelled) = control.checkpoint() {
        if let Some(value) = captured.value {
            crate::json_value::drop_value_nonrecursive(value);
        }
        return Err(cancelled);
    }
    Ok(captured)
}
