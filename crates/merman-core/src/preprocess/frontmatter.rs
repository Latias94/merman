//! Frontmatter YAML fields, Mermaid config projection, and source evidence.

use super::config_scan::MAX_CONFIG_NESTING_DEPTH;
#[cfg(feature = "editor-facts")]
use super::{
    FrontmatterSourceEvidence, LocalConfigKeyEvidence, PreprocessedSource, SourceConfigCaptureMode,
    SourceConfigEvidence, SourceConfigKeyEvidence, SourceConfigOrigin,
};
#[cfg(feature = "editor-facts")]
use crate::SourceSpan;
use crate::{Error, MermaidConfig, OperationControl, OperationControlResult, Result};
use merman_core_preprocess::frontmatter::{
    FrontmatterByteSpan, dedent_frontmatter_body_controlled, locate_frontmatter_block_controlled,
};
#[cfg(feature = "editor-facts")]
use merman_core_preprocess::{
    SourceEdit,
    frontmatter::{find_physical_line_ending_controlled, frontmatter_has_prefix_controlled},
    scan::controlled::ControlledScanCheckpoints,
};
use serde_json::{Map, Value};
use std::borrow::Cow;

#[cfg(feature = "editor-facts")]
pub(super) fn append_frontmatter_evidence(
    source: &PreprocessedSource,
    captured: &FrontmatterCapture,
    evidence: &mut SourceConfigEvidence,
    control: &OperationControl,
) -> OperationControlResult<()> {
    let frontmatter = captured
        .full
        .zip(captured.body)
        .and_then(|(full, local_body)| {
            let full = source.try_map_span(SourceSpan::new(full.start, full.end))?;
            let body = map_frontmatter_body_span_to_original(source, local_body)?;
            // The mapped body end is the exact edit boundary used by the existing insertion
            // projection. For CRLF it intentionally sits between `\r` and `\n`; the projection
            // replaces that boundary with a complete CRLF-delimited config block.
            let insert = SourceSpan::new(body.end, body.end);
            Some(FrontmatterSourceEvidence::new(
                full,
                body,
                captured.indent.clone(),
                captured.has_config,
                captured.rewrite_safe,
                captured.fields.clone(),
            ))
            .map(|frontmatter| (frontmatter, insert))
        });
    if let Some((frontmatter, insert)) = frontmatter {
        evidence.set_config_insert_span(insert);
        evidence.set_frontmatter(frontmatter);
    } else if captured.full.is_some() {
        evidence.mark_rewrite_unsafe();
    }

    for (index, key) in captured.keys.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        let Some(local_span) = key.span else {
            continue;
        };
        let exact_span = source.try_map_span(local_span);
        let Some(span) = exact_span.or_else(|| source.try_map_enclosing_span(local_span)) else {
            continue;
        };
        evidence.push_key(SourceConfigKeyEvidence::new(
            SourceConfigOrigin::Frontmatter,
            key.path.clone(),
            span,
            evidence.keys().len(),
            key.rewrite_safe && exact_span.is_some(),
        ));
    }
    control.checkpoint()
}

#[cfg(feature = "editor-facts")]
fn map_frontmatter_body_span_to_original(
    source: &PreprocessedSource,
    body: FrontmatterByteSpan,
) -> Option<SourceSpan> {
    let mut mapped = source.try_map_span(SourceSpan::new(body.start, body.end))?;
    // The frontmatter locator intentionally leaves the `\r` from a CRLF immediately before the
    // closing delimiter inside the body range. Preprocessing normalizes that pair to one `\n`,
    // whose left boundary maps before the original `\r`. Recover the locator's original boundary
    // from the same edit map without rescanning or retaining source text.
    if source.text().as_bytes().get(body.end) == Some(&b'\n') {
        let newline = source.try_map_span(SourceSpan::new(body.end, body.end + 1))?;
        if newline.end.saturating_sub(newline.start) == 2 && mapped.end == newline.start {
            mapped.end = newline.end - 1;
        }
    }
    Some(mapped)
}

pub(super) struct ProcessedFrontmatter {
    pub(super) title: Option<String>,
    pub(super) config: MermaidConfig,
}

pub(super) struct FrontmatterCapture {
    pub(super) result: Result<ProcessedFrontmatter>,
    pub(super) full: Option<FrontmatterByteSpan>,
    #[cfg(feature = "editor-facts")]
    body: Option<FrontmatterByteSpan>,
    #[cfg(feature = "editor-facts")]
    indent: String,
    #[cfg(feature = "editor-facts")]
    pub(super) keys: Vec<LocalConfigKeyEvidence>,
    #[cfg(feature = "editor-facts")]
    has_config: bool,
    #[cfg(feature = "editor-facts")]
    rewrite_safe: bool,
    #[cfg(feature = "editor-facts")]
    fields: Option<MermaidConfig>,
}

enum FrontmatterYamlInput<'a> {
    Plain(Cow<'a, str>),
    #[cfg(feature = "editor-facts")]
    Mapped(Box<PreprocessedSource>),
}

impl FrontmatterYamlInput<'_> {
    fn text(&self) -> &str {
        match self {
            Self::Plain(text) => text.as_ref(),
            #[cfg(feature = "editor-facts")]
            Self::Mapped(source) => source.text(),
        }
    }

    #[cfg(feature = "editor-facts")]
    fn try_map_span_to_body(&self, span: SourceSpan) -> Option<SourceSpan> {
        match self {
            Self::Plain(_) => Some(span),
            #[cfg(feature = "editor-facts")]
            Self::Mapped(source) => source.try_map_span(span),
        }
    }

    #[cfg(feature = "editor-facts")]
    fn try_map_enclosing_span_to_body(&self, span: SourceSpan) -> Option<SourceSpan> {
        match self {
            Self::Plain(_) => Some(span),
            #[cfg(feature = "editor-facts")]
            Self::Mapped(source) => source.try_map_enclosing_span(span),
        }
    }
}

#[cfg(feature = "editor-facts")]
fn map_frontmatter_yaml_span_to_preprocessed_source(
    yaml_input: &FrontmatterYamlInput<'_>,
    body_start: usize,
    span: std::ops::Range<usize>,
) -> Option<(SourceSpan, bool)> {
    let parser_span = SourceSpan::new(span.start, span.end);
    let exact = yaml_input.try_map_span_to_body(parser_span);
    let body_span = exact.or_else(|| yaml_input.try_map_enclosing_span_to_body(parser_span))?;
    Some((
        SourceSpan::new(
            body_start.checked_add(body_span.start)?,
            body_start.checked_add(body_span.end)?,
        ),
        exact.is_some(),
    ))
}

pub(super) fn process_frontmatter_controlled(
    input: &str,
    #[cfg(feature = "editor-facts")] capture_mode: SourceConfigCaptureMode,
    control: &OperationControl,
) -> OperationControlResult<FrontmatterCapture> {
    control.checkpoint()?;
    let Some(location) = locate_frontmatter_block_controlled(input, control)? else {
        return Ok(FrontmatterCapture {
            result: Ok(ProcessedFrontmatter {
                title: None,
                config: MermaidConfig::empty_object(),
            }),
            full: None,
            #[cfg(feature = "editor-facts")]
            body: None,
            #[cfg(feature = "editor-facts")]
            indent: String::new(),
            #[cfg(feature = "editor-facts")]
            keys: Vec::new(),
            #[cfg(feature = "editor-facts")]
            has_config: false,
            #[cfg(feature = "editor-facts")]
            rewrite_safe: true,
            #[cfg(feature = "editor-facts")]
            fields: None,
        });
    };
    let body = &input[location.body.start..location.body.end];
    let yaml_input = dedented_frontmatter_yaml_input_controlled(
        body,
        location.indent,
        #[cfg(feature = "editor-facts")]
        capture_mode,
        control,
    )?;
    control.checkpoint()?;
    let mut captured = crate::yaml_config::parse_yaml_with_capture_controlled(
        yaml_input.text(),
        MAX_CONFIG_NESTING_DEPTH,
        &["title", "displayMode"],
        #[cfg(feature = "editor-facts")]
        capture_mode.collects(),
        control,
    )?;
    #[cfg(feature = "editor-facts")]
    let rewrite_safe = captured.rewrite_safe;
    #[cfg(feature = "editor-facts")]
    let keys: Vec<_> = captured
        .keys
        .into_iter()
        .map(|key| {
            // The outer source edit map is applied when the operation evidence is appended.
            let mapped = key.span.and_then(|span| {
                map_frontmatter_yaml_span_to_preprocessed_source(
                    &yaml_input,
                    location.body.start,
                    span,
                )
            });
            let (span, exact) = mapped.map_or((None, false), |(span, exact)| (Some(span), exact));
            LocalConfigKeyEvidence {
                path: key.path,
                span,
                rewrite_safe: key.rewrite_safe && exact,
            }
        })
        .collect();
    let parsed_obj = match frontmatter_fields_from_yaml_value(captured.value) {
        Ok(parsed) => parsed,
        Err(message) => {
            return Ok(FrontmatterCapture {
                result: Err(Error::InvalidFrontMatterYaml { message }),
                full: Some(location.full),
                #[cfg(feature = "editor-facts")]
                body: Some(location.body),
                #[cfg(feature = "editor-facts")]
                indent: location.indent.to_string(),
                #[cfg(feature = "editor-facts")]
                has_config: keys.iter().any(|key| key.path.first() == Some("config")),
                #[cfg(feature = "editor-facts")]
                keys,
                #[cfg(feature = "editor-facts")]
                rewrite_safe: false,
                #[cfg(feature = "editor-facts")]
                fields: None,
            });
        }
    };
    if let Err(cancelled) = control.checkpoint() {
        crate::json_value::drop_value_nonrecursive(Value::Object(parsed_obj));
        return Err(cancelled);
    }

    let title = frontmatter_text(
        parsed_obj.get("title"),
        captured.collection_text.remove("title"),
    );
    let display_mode = frontmatter_text(
        parsed_obj.get("displayMode"),
        captured.collection_text.remove("displayMode"),
    );

    let mut config = MermaidConfig::empty_object();
    merge_top_level_frontmatter_diagram_configs(&parsed_obj, &mut config);
    if let Some(v) = parsed_obj
        .get("config")
        .filter(|value| frontmatter_truthy(value))
    {
        config.deep_merge(v);
    }
    crate::config::mirror_legacy_font_family_into_theme_variables(&mut config);
    if let Some(dm) = display_mode {
        config.set_value("gantt.displayMode", Value::String(dm));
    }

    #[cfg(feature = "editor-facts")]
    let has_config = parsed_obj.contains_key("config");
    #[cfg(feature = "editor-facts")]
    let fields = if capture_mode.collects() && has_config && rewrite_safe {
        Some(MermaidConfig::from_value(Value::Object(parsed_obj)))
    } else {
        crate::json_value::drop_value_nonrecursive(Value::Object(parsed_obj));
        None
    };
    #[cfg(not(feature = "editor-facts"))]
    crate::json_value::drop_value_nonrecursive(Value::Object(parsed_obj));
    control.checkpoint()?;
    Ok(FrontmatterCapture {
        result: Ok(ProcessedFrontmatter { title, config }),
        full: Some(location.full),
        #[cfg(feature = "editor-facts")]
        body: Some(location.body),
        #[cfg(feature = "editor-facts")]
        indent: location.indent.to_string(),
        #[cfg(feature = "editor-facts")]
        keys,
        #[cfg(feature = "editor-facts")]
        has_config,
        #[cfg(feature = "editor-facts")]
        rewrite_safe,
        #[cfg(feature = "editor-facts")]
        fields,
    })
}

fn dedented_frontmatter_yaml_input_controlled<'a>(
    body: &'a str,
    indent: &str,
    #[cfg(feature = "editor-facts")] capture_mode: SourceConfigCaptureMode,
    control: &OperationControl,
) -> OperationControlResult<FrontmatterYamlInput<'a>> {
    #[cfg(not(feature = "editor-facts"))]
    {
        dedent_frontmatter_body_controlled(body, indent, control).map(FrontmatterYamlInput::Plain)
    }
    #[cfg(feature = "editor-facts")]
    {
        if !capture_mode.collects() {
            return dedent_frontmatter_body_controlled(body, indent, control)
                .map(FrontmatterYamlInput::Plain);
        }
        if indent.is_empty() {
            control.checkpoint()?;
            return Ok(FrontmatterYamlInput::Plain(Cow::Borrowed(body)));
        }

        let mut source = PreprocessedSource::new_controlled(body, true, control)?;
        let mut checkpoints = ControlledScanCheckpoints::new(control)?;
        let mut edits = Vec::new();
        let mut line_start = 0usize;
        loop {
            let line_ending =
                find_physical_line_ending_controlled(body, line_start, &mut checkpoints)?;
            let (line_end, next_line_start) = line_ending.unwrap_or((body.len(), body.len()));
            let line = &body[line_start..line_end];
            if frontmatter_has_prefix_controlled(line, indent, &mut checkpoints)? {
                edits.push(SourceEdit::delete(line_start..line_start + indent.len()));
            }
            if line_ending.is_none() || next_line_start == body.len() {
                break;
            }
            line_start = next_line_start;
        }
        checkpoints.finish()?;
        source.apply_edits(edits, control)?;
        Ok(FrontmatterYamlInput::Mapped(Box::new(source)))
    }
}

pub fn parse_frontmatter_yaml_fields(
    input: &str,
) -> std::result::Result<Map<String, Value>, String> {
    let control = OperationControl::new();
    parse_frontmatter_yaml_fields_controlled(input, &control)
        .expect("a private parse control cannot be cancelled")
}

/// Parses frontmatter YAML fields while observing cooperative cancellation.
pub fn parse_frontmatter_yaml_fields_controlled(
    input: &str,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<Map<String, Value>, String>> {
    let parsed =
        crate::yaml_config::parse_yaml_value_controlled(input, MAX_CONFIG_NESTING_DEPTH, control)?;
    Ok(frontmatter_fields_from_yaml_value(parsed))
}

/// Parses frontmatter fields with caller-owned nesting and materialization budgets.
pub fn parse_frontmatter_yaml_fields_bounded_controlled(
    input: &str,
    max_input_bytes: usize,
    max_nesting_depth: usize,
    max_materialized_bytes: usize,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<Map<String, Value>, String>> {
    let parsed = crate::yaml_config::parse_yaml_value_with_limits_controlled(
        input,
        max_input_bytes,
        max_nesting_depth,
        max_materialized_bytes,
        control,
    )?;
    Ok(frontmatter_fields_from_yaml_value(parsed))
}

fn frontmatter_fields_from_yaml_value(
    parsed: std::result::Result<Value, String>,
) -> std::result::Result<Map<String, Value>, String> {
    let parsed = parsed?;
    match parsed {
        Value::Object(map) => Ok(map),
        other => {
            crate::json_value::drop_value_nonrecursive(other);
            Ok(Map::new())
        }
    }
}

fn frontmatter_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn frontmatter_text(value: Option<&Value>, collection: Option<String>) -> Option<String> {
    match value.filter(|value| frontmatter_truthy(value))? {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null => None,
        Value::Array(_) | Value::Object(_) => collection,
    }
}

fn merge_top_level_frontmatter_diagram_configs(
    parsed_obj: &serde_json::Map<String, Value>,
    config: &mut MermaidConfig,
) {
    // Mermaid upstream only consumes `config`, but users commonly read docs examples as allowing
    // diagram config namespaces at the YAML root. Keep this compatibility narrow and explicit.
    for fact in crate::family::frontmatter_config_aliases() {
        if let Some(value) = parsed_obj.get(fact.source) {
            config.set_value(
                fact.namespace,
                crate::json_value::clone_value_nonrecursive(value),
            );
        }
    }

    for &key in crate::family::frontmatter_config_namespaces() {
        if let Some(value) = parsed_obj.get(key) {
            config.set_value(key, crate::json_value::clone_value_nonrecursive(value));
        }
    }
}
