use crate::{
    AnalysisCancellationToken, AnalysisCancelled, DiagnosticFix, DiagnosticFixEdit, SourceMap,
    source_directives::{ByteSpan, init_directive_spans_cancellable},
};
#[cfg(test)]
use merman_core::preprocess::{parse_frontmatter_yaml_fields, split_frontmatter_block};
use merman_core::{
    MermaidConfig,
    preprocess::{
        FrontmatterBlock, locate_frontmatter_block_controlled,
        parse_frontmatter_yaml_fields_bounded_controlled, split_frontmatter_block_controlled,
    },
};
use serde_json::{Map, Value};

const REWRITE_SCAN_CHECKPOINT_BYTES: usize = 4 * 1024;
// Quick fixes are advisory. Bound every owned migration input and the complete replacement output
// independently from source limits because YAML alias materialization and frontmatter indentation
// can amplify the captured source config while analysis itself may be configured without a limit.
const MAX_CONFIG_MIGRATION_FIX_WEIGHT_BYTES: usize = 1024 * 1024;
const MAX_FRONTMATTER_MIGRATION_INPUT_BYTES: usize = 1024 * 1024;
const MAX_FRONTMATTER_MIGRATION_MATERIALIZED_BYTES: usize = 1024 * 1024;
const MAX_FRONTMATTER_MIGRATION_OUTPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_FRONTMATTER_MIGRATION_NESTING_DEPTH: usize = 64;
const MAX_CONFIG_MIGRATION_FIX_EDITS: usize = 128;

struct BoundedMigrationConfig(Value);

impl BoundedMigrationConfig {
    fn capture(
        config: &MermaidConfig,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Option<Self>, AnalysisCancelled> {
        config
            .clone_value_bounded_controlled(
                MAX_CONFIG_MIGRATION_FIX_WEIGHT_BYTES,
                MAX_FRONTMATTER_MIGRATION_NESTING_DEPTH,
                cancellation.parse_control(),
            )
            .map_err(|_| AnalysisCancelled)
            .map(|config| config.map(Self))
    }

    fn into_value(self) -> Value {
        self.0
    }
}

pub(crate) fn init_directives_to_frontmatter_fix_cancellable(
    source: &str,
    source_map: &SourceMap,
    config: &MermaidConfig,
    cancellation: &AnalysisCancellationToken,
) -> Result<Option<DiagnosticFix>, AnalysisCancelled> {
    cancellation.checkpoint()?;
    let init_directives = init_directive_spans_cancellable(source, cancellation)?;
    if init_directives.is_empty() {
        return Ok(None);
    }
    if init_directives.len() > MAX_CONFIG_MIGRATION_FIX_EDITS {
        return Ok(None);
    }

    let Some(config) = BoundedMigrationConfig::capture(config, cancellation)? else {
        return Ok(None);
    };
    if matches!(&config.0, Value::Object(map) if map.is_empty()) {
        return Ok(None);
    }

    let mut removals = Vec::with_capacity(init_directives.len());
    for directive in init_directives {
        removals.push(directive_removal_span_cancellable(
            source,
            directive.full,
            cancellation,
        )?);
    }

    frontmatter_config_fix_cancellable(
        source,
        source_map,
        config,
        removals,
        "Move init directive config into frontmatter",
        cancellation,
    )
}

#[cfg(test)]
pub(crate) fn init_directives_to_frontmatter_fix(
    source: &str,
    source_map: &SourceMap,
    config: &MermaidConfig,
) -> Option<DiagnosticFix> {
    let cancellation = AnalysisCancellationToken::new();
    init_directives_to_frontmatter_fix_cancellable(source, source_map, config, &cancellation)
        .expect("a private analysis cancellation token cannot be cancelled")
}

fn frontmatter_config_fix_cancellable(
    source: &str,
    source_map: &SourceMap,
    config: BoundedMigrationConfig,
    removals: Vec<ByteSpan>,
    title: &'static str,
    cancellation: &AnalysisCancellationToken,
) -> Result<Option<DiagnosticFix>, AnalysisCancelled> {
    cancellation.checkpoint()?;
    let Some(frontmatter) = frontmatter_config_edit_cancellable(source, config, cancellation)?
    else {
        return Ok(None);
    };
    let inserts_at_start = frontmatter.span.start == 0 && frontmatter.span.end == 0;
    let folds_first_removal = inserts_at_start
        && removals
            .first()
            .is_some_and(|first_removal| first_removal.start == 0);
    let required_edits = removals
        .len()
        .saturating_add(usize::from(!folds_first_removal));
    if required_edits > MAX_CONFIG_MIGRATION_FIX_EDITS {
        return Ok(None);
    }
    let mut removals = removals.into_iter();
    let mut first_removal = removals.next();
    cancellation.checkpoint()?;

    let mut edits = Vec::new();
    if inserts_at_start {
        if let Some(removal) = first_removal.take().filter(|span| span.start == 0) {
            let Ok(span) = source_map.span_cancellable(removal.start, removal.end, cancellation)?
            else {
                return Ok(None);
            };
            edits.push(DiagnosticFixEdit::new(span, frontmatter.replacement));
        } else {
            let Ok(span) = source_map.span_cancellable(
                frontmatter.span.start,
                frontmatter.span.end,
                cancellation,
            )?
            else {
                return Ok(None);
            };
            edits.push(DiagnosticFixEdit::new(span, frontmatter.replacement));
        }
    } else {
        let Ok(span) = source_map.span_cancellable(
            frontmatter.span.start,
            frontmatter.span.end,
            cancellation,
        )?
        else {
            return Ok(None);
        };
        edits.push(DiagnosticFixEdit::new(span, frontmatter.replacement));
    }

    for removal in first_removal.into_iter().chain(removals) {
        let Ok(span) = source_map.span_cancellable(removal.start, removal.end, cancellation)?
        else {
            return Ok(None);
        };
        edits.push(DiagnosticFixEdit::new(span, ""));
    }

    cancellation.checkpoint()?;
    Ok(Some(DiagnosticFix::new(title, edits).preferred()))
}

#[cfg(test)]
pub(crate) fn frontmatter_config_fix(
    source: &str,
    source_map: &SourceMap,
    config: Value,
    removals: Vec<ByteSpan>,
    title: &'static str,
) -> Option<DiagnosticFix> {
    let cancellation = AnalysisCancellationToken::new();
    let config = MermaidConfig::from_value(config);
    let config = BoundedMigrationConfig::capture(&config, &cancellation)
        .expect("a private analysis cancellation token cannot be cancelled")?;
    frontmatter_config_fix_cancellable(source, source_map, config, removals, title, &cancellation)
        .expect("a private analysis cancellation token cannot be cancelled")
}

struct FrontmatterEdit {
    span: ByteSpan,
    replacement: String,
}

fn frontmatter_config_edit_cancellable(
    source: &str,
    config: BoundedMigrationConfig,
    cancellation: &AnalysisCancellationToken,
) -> Result<Option<FrontmatterEdit>, AnalysisCancelled> {
    let config = config.into_value();
    let newline = newline_for_source_cancellable(source, cancellation)?;
    cancellation.checkpoint()?;
    let Some(location) = locate_frontmatter_block_controlled(source, cancellation.parse_control())
        .map_err(|_| AnalysisCancelled)?
    else {
        cancellation.checkpoint()?;
        let replacement = frontmatter_document_cancellable(
            frontmatter_fields_with_config(Map::new(), config),
            "",
            newline,
            cancellation,
        )?;
        cancellation.checkpoint()?;
        return Ok(replacement.map(|replacement| FrontmatterEdit {
            span: ByteSpan { start: 0, end: 0 },
            replacement,
        }));
    };
    if location.full.end.saturating_sub(location.full.start) > MAX_FRONTMATTER_MIGRATION_INPUT_BYTES
    {
        return Ok(None);
    }
    let Some(frontmatter) =
        split_frontmatter_block_controlled(source, cancellation.parse_control())
            .map_err(|_| AnalysisCancelled)?
    else {
        return Ok(None);
    };
    cancellation.checkpoint()?;

    let newline = newline_for_source_cancellable(
        &source[frontmatter.full.start..frontmatter.full.end],
        cancellation,
    )?;
    let Some(existing_fields) =
        parse_frontmatter_fields_cancellable(frontmatter.dedented_body.as_ref(), cancellation)?
    else {
        return Ok(None);
    };
    cancellation.checkpoint()?;
    if !existing_fields.contains_key("config") {
        let replacement = frontmatter_config_insertion_cancellable(
            source,
            &frontmatter,
            config,
            newline,
            cancellation,
        )?;
        cancellation.checkpoint()?;
        return Ok(replacement.map(|replacement| FrontmatterEdit {
            span: ByteSpan {
                start: frontmatter.body.end,
                end: frontmatter.body.end,
            },
            replacement,
        }));
    }
    if frontmatter_contains_lossy_yaml_syntax_cancellable(
        frontmatter.dedented_body.as_ref(),
        cancellation,
    )? {
        return Ok(None);
    }

    cancellation.checkpoint()?;
    let replacement = frontmatter_document_cancellable(
        frontmatter_fields_with_config(existing_fields, config),
        frontmatter.indent,
        newline,
        cancellation,
    )?;
    cancellation.checkpoint()?;
    Ok(replacement.map(|replacement| FrontmatterEdit {
        span: ByteSpan {
            start: frontmatter.full.start,
            end: frontmatter.full.end,
        },
        replacement,
    }))
}

fn frontmatter_fields_with_config(
    mut fields: Map<String, Value>,
    config: Value,
) -> Map<String, Value> {
    if let Some(replaced) = fields.insert("config".to_string(), config) {
        drop(MermaidConfig::from_value(replaced));
    }
    fields
}

fn frontmatter_config_insertion_cancellable(
    source: &str,
    frontmatter: &FrontmatterBlock<'_>,
    config: Value,
    newline: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<Option<String>, AnalysisCancelled> {
    let mut fields = Map::new();
    fields.insert("config".to_string(), config);
    let Some(body) = frontmatter_body_cancellable(fields, cancellation)? else {
        return Ok(None);
    };
    let Some(indented_body) =
        frontmatter_body_with_indent_cancellable(&body, frontmatter.indent, newline, cancellation)?
    else {
        return Ok(None);
    };
    let mut insertion = BoundedReplacement::new();
    let existing_body = &source[frontmatter.body.start..frontmatter.body.end];
    let insertion_splits_crlf =
        existing_body.ends_with('\r') && source[frontmatter.body.end..].starts_with('\n');
    if !is_whitespace_cancellable(existing_body, cancellation)? {
        if insertion_splits_crlf {
            if !insertion.push_char('\n') {
                return Ok(None);
            }
        } else if !existing_body.ends_with('\n') && !insertion.push_str(newline) {
            return Ok(None);
        }
    }
    if !insertion.push_str(&indented_body) {
        return Ok(None);
    }
    if insertion_splits_crlf {
        if !insertion.push_char('\r') {
            return Ok(None);
        }
    } else if frontmatter.body.start == frontmatter.body.end && !insertion.push_str(newline) {
        return Ok(None);
    }
    cancellation.checkpoint()?;
    Ok(Some(insertion.finish()))
}

fn frontmatter_contains_lossy_yaml_syntax_cancellable(
    yaml_body: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<bool, AnalysisCancelled> {
    for line in yaml_body.lines() {
        if frontmatter_line_contains_lossy_yaml_syntax_cancellable(line, cancellation)? {
            return Ok(true);
        }
    }
    cancellation.checkpoint()?;
    Ok(false)
}

fn frontmatter_line_contains_lossy_yaml_syntax_cancellable(
    line: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<bool, AnalysisCancelled> {
    let trimmed = trim_start_cancellable(line, cancellation)?;
    if trimmed.starts_with('#')
        || trimmed.starts_with("<<:")
        || trimmed == "?"
        || trimmed.starts_with("? ")
        || trimmed.starts_with('!')
        || trimmed.starts_with("- !")
    {
        return Ok(true);
    }
    if frontmatter_line_has_inline_lossy_marker_cancellable(line, cancellation)?
        || frontmatter_line_starts_with_flow_complex_key_cancellable(trimmed, cancellation)?
        || frontmatter_line_uses_block_scalar_cancellable(trimmed, cancellation)?
    {
        return Ok(true);
    }
    cancellation.checkpoint()?;
    Ok(false)
}

fn frontmatter_line_has_inline_lossy_marker_cancellable(
    line: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<bool, AnalysisCancelled> {
    let bytes = line.as_bytes();
    let mut next_checkpoint = 0usize;
    for index in 0..bytes.len() {
        checkpoint_scan_offset(index, &mut next_checkpoint, cancellation)?;
        if bytes[index] == b' ' && matches!(bytes.get(index + 1), Some(b'#' | b'&' | b'*')) {
            return Ok(true);
        }
        if bytes[index] == b':'
            && bytes.get(index + 1) == Some(&b' ')
            && bytes.get(index + 2) == Some(&b'!')
        {
            return Ok(true);
        }
    }
    cancellation.checkpoint()?;
    Ok(false)
}

fn frontmatter_line_starts_with_flow_complex_key_cancellable(
    trimmed: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<bool, AnalysisCancelled> {
    if flow_complex_key_has_colon_cancellable(trimmed, cancellation)? {
        return Ok(true);
    }
    let Some(item) = trimmed.strip_prefix("- ") else {
        return Ok(false);
    };
    let item = trim_start_cancellable(item, cancellation)?;
    flow_complex_key_has_colon_cancellable(item, cancellation)
}

fn flow_complex_key_has_colon_cancellable(
    trimmed: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<bool, AnalysisCancelled> {
    let close = match trimmed.as_bytes().first().copied() {
        Some(b'[') => b']',
        Some(b'{') => b'}',
        _ => return Ok(false),
    };
    let bytes = trimmed.as_bytes();
    let mut next_checkpoint = 0usize;
    for (offset, byte) in bytes.iter().copied().enumerate() {
        checkpoint_scan_offset(offset, &mut next_checkpoint, cancellation)?;
        if byte == close {
            let suffix = trim_start_cancellable(&trimmed[offset + 1..], cancellation)?;
            return Ok(suffix.starts_with(':'));
        }
    }
    cancellation.checkpoint()?;
    Ok(false)
}

fn trim_start_cancellable<'a>(
    input: &'a str,
    cancellation: &AnalysisCancellationToken,
) -> Result<&'a str, AnalysisCancelled> {
    let mut next_checkpoint = 0usize;
    for (offset, ch) in input.char_indices() {
        checkpoint_scan_offset(offset, &mut next_checkpoint, cancellation)?;
        if !ch.is_whitespace() {
            return Ok(&input[offset..]);
        }
    }
    cancellation.checkpoint()?;
    Ok(&input[input.len()..])
}

fn is_whitespace_cancellable(
    input: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<bool, AnalysisCancelled> {
    Ok(trim_start_cancellable(input, cancellation)?.is_empty())
}

fn frontmatter_line_uses_block_scalar_cancellable(
    trimmed: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<bool, AnalysisCancelled> {
    let bytes = trimmed.as_bytes();
    let mut next_checkpoint = 0usize;
    for (offset, byte) in bytes.iter().copied().enumerate() {
        checkpoint_scan_offset(offset, &mut next_checkpoint, cancellation)?;
        if byte == b':' {
            let value = trim_start_cancellable(&trimmed[offset + 1..], cancellation)?;
            return Ok(value.starts_with('|') || value.starts_with('>'));
        }
    }
    cancellation.checkpoint()?;
    Ok(false)
}

fn frontmatter_document_cancellable(
    fields: Map<String, Value>,
    indent: &str,
    newline: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<Option<String>, AnalysisCancelled> {
    let Some(body) = frontmatter_body_cancellable(fields, cancellation)? else {
        return Ok(None);
    };
    frontmatter_document_with_indent_cancellable(&body, indent, newline, cancellation)
}

fn frontmatter_body_cancellable(
    fields: Map<String, Value>,
    cancellation: &AnalysisCancellationToken,
) -> Result<Option<String>, AnalysisCancelled> {
    cancellation.checkpoint()?;
    // The serializer does not expose an inner parse-control hook. The writer caps retained output
    // length and checkpoints at output-chunk boundaries; input size, materialization, and nesting
    // bounds keep serializer-internal scalar work finite during this atomic traversal.
    let fields = MermaidConfig::from_value(Value::Object(fields));
    let mut writer = BoundedYamlWriter::new(cancellation);
    let serialization = serde_saphyr::to_fmt_writer(&mut writer, fields.as_value());
    let failure = writer.failure;
    let serialized = writer.text;
    drop(fields);
    match failure {
        Some(BoundedWriteFailure::Cancelled) => return Err(AnalysisCancelled),
        Some(BoundedWriteFailure::Limit | BoundedWriteFailure::Allocation) => return Ok(None),
        None if serialization.is_err() => return Ok(None),
        None => {}
    }
    cancellation.checkpoint()?;

    let body = serialized.strip_prefix("---\n").unwrap_or(&serialized);
    let body = body.strip_suffix("...\n").unwrap_or(body);
    let trailing_newline = usize::from(!body.ends_with('\n'));
    let Some(required_len) = body.len().checked_add(trailing_newline) else {
        return Ok(None);
    };
    if required_len > MAX_FRONTMATTER_MIGRATION_OUTPUT_BYTES {
        return Ok(None);
    }

    let mut normalized = String::new();
    if normalized.try_reserve_exact(required_len).is_err() {
        return Ok(None);
    }
    normalized.push_str(body);
    if trailing_newline != 0 {
        normalized.push('\n');
    }
    cancellation.checkpoint()?;
    Ok(Some(normalized))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoundedWriteFailure {
    Cancelled,
    Limit,
    Allocation,
}

struct BoundedYamlWriter<'a> {
    text: String,
    cancellation: &'a AnalysisCancellationToken,
    failure: Option<BoundedWriteFailure>,
    next_checkpoint: usize,
}

impl<'a> BoundedYamlWriter<'a> {
    fn new(cancellation: &'a AnalysisCancellationToken) -> Self {
        Self {
            text: String::new(),
            cancellation,
            failure: None,
            next_checkpoint: REWRITE_SCAN_CHECKPOINT_BYTES,
        }
    }

    fn fail(&mut self, failure: BoundedWriteFailure) -> std::fmt::Result {
        self.failure = Some(failure);
        Err(std::fmt::Error)
    }
}

impl std::fmt::Write for BoundedYamlWriter<'_> {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        if self.failure.is_some() {
            return Err(std::fmt::Error);
        }
        let Some(required_len) = self.text.len().checked_add(value.len()) else {
            return self.fail(BoundedWriteFailure::Limit);
        };
        if required_len > MAX_FRONTMATTER_MIGRATION_OUTPUT_BYTES {
            return self.fail(BoundedWriteFailure::Limit);
        }
        while required_len >= self.next_checkpoint {
            if self.cancellation.checkpoint().is_err() {
                return self.fail(BoundedWriteFailure::Cancelled);
            }
            self.next_checkpoint = self
                .next_checkpoint
                .saturating_add(REWRITE_SCAN_CHECKPOINT_BYTES);
        }
        if self.text.try_reserve(value.len()).is_err() {
            return self.fail(BoundedWriteFailure::Allocation);
        }
        self.text.push_str(value);
        Ok(())
    }
}

fn frontmatter_body_with_indent_cancellable(
    body: &str,
    indent: &str,
    newline: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<Option<String>, AnalysisCancelled> {
    let mut document = BoundedReplacement::new();
    for (index, line) in body.split_inclusive('\n').enumerate() {
        checkpoint_long_slice(line, cancellation)?;
        if index > 0 && !document.push_str(newline) {
            return Ok(None);
        }
        if !document.push_str(indent) || !document.push_str(line.trim_end_matches('\n')) {
            return Ok(None);
        }
    }
    cancellation.checkpoint()?;
    Ok(Some(document.finish()))
}

fn frontmatter_document_with_indent_cancellable(
    body: &str,
    indent: &str,
    newline: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<Option<String>, AnalysisCancelled> {
    let mut document = BoundedReplacement::new();
    if !document.push_str(indent) || !document.push_str("---") || !document.push_str(newline) {
        return Ok(None);
    }
    for line in body.split_terminator('\n') {
        checkpoint_long_slice(line, cancellation)?;
        if !document.push_str(indent) || !document.push_str(line) || !document.push_str(newline) {
            return Ok(None);
        }
    }
    if !document.push_str(indent) || !document.push_str("---") || !document.push_str(newline) {
        return Ok(None);
    }
    cancellation.checkpoint()?;
    Ok(Some(document.finish()))
}

fn parse_frontmatter_fields_cancellable(
    yaml_body: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<Option<Map<String, Value>>, AnalysisCancelled> {
    parse_frontmatter_yaml_fields_bounded_controlled(
        yaml_body,
        MAX_FRONTMATTER_MIGRATION_INPUT_BYTES,
        MAX_FRONTMATTER_MIGRATION_NESTING_DEPTH,
        MAX_FRONTMATTER_MIGRATION_MATERIALIZED_BYTES,
        cancellation.parse_control(),
    )
    .map_err(|_| AnalysisCancelled)
    .map(|parsed| parsed.ok())
}

struct BoundedReplacement {
    text: String,
}

impl BoundedReplacement {
    fn new() -> Self {
        Self {
            text: String::new(),
        }
    }

    fn push_str(&mut self, value: &str) -> bool {
        let Some(required_len) = self.text.len().checked_add(value.len()) else {
            return false;
        };
        if required_len > MAX_FRONTMATTER_MIGRATION_OUTPUT_BYTES
            || self.text.try_reserve(value.len()).is_err()
        {
            return false;
        }
        self.text.push_str(value);
        true
    }

    fn push_char(&mut self, value: char) -> bool {
        let mut encoded = [0; 4];
        self.push_str(value.encode_utf8(&mut encoded))
    }

    fn finish(self) -> String {
        self.text
    }
}

fn newline_for_source_cancellable(
    source: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<&'static str, AnalysisCancelled> {
    let bytes = source.as_bytes();
    let mut next_checkpoint = 0usize;
    for index in 0..bytes.len().saturating_sub(1) {
        checkpoint_scan_offset(index, &mut next_checkpoint, cancellation)?;
        if bytes[index] == b'\r' && bytes[index + 1] == b'\n' {
            return Ok("\r\n");
        }
    }
    cancellation.checkpoint()?;
    Ok("\n")
}

fn directive_removal_span_cancellable(
    source: &str,
    directive: ByteSpan,
    cancellation: &AnalysisCancellationToken,
) -> Result<ByteSpan, AnalysisCancelled> {
    let bytes = source.as_bytes();
    let mut line_start = directive.start;
    let mut scanned = 0usize;
    while line_start > 0 {
        if scanned.is_multiple_of(REWRITE_SCAN_CHECKPOINT_BYTES) {
            cancellation.checkpoint()?;
        }
        if bytes[line_start - 1] == b'\n' {
            break;
        }
        line_start -= 1;
        scanned += 1;
    }

    let mut line_end = directive.end;
    scanned = 0;
    while line_end < bytes.len() {
        if scanned.is_multiple_of(REWRITE_SCAN_CHECKPOINT_BYTES) {
            cancellation.checkpoint()?;
        }
        if bytes[line_end] == b'\n' {
            break;
        }
        line_end += 1;
        scanned += 1;
    }
    let line_end_with_newline = if line_end < source.len() {
        line_end + 1
    } else {
        line_end
    };

    if is_whitespace_cancellable(&source[line_start..directive.start], cancellation)?
        && is_whitespace_cancellable(&source[directive.end..line_end], cancellation)?
    {
        Ok(ByteSpan {
            start: line_start,
            end: line_end_with_newline,
        })
    } else {
        Ok(directive)
    }
}

fn checkpoint_long_slice(
    source: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<(), AnalysisCancelled> {
    for _ in (0..source.len()).step_by(REWRITE_SCAN_CHECKPOINT_BYTES) {
        cancellation.checkpoint()?;
    }
    cancellation.checkpoint()
}

fn checkpoint_scan_offset(
    offset: usize,
    next_checkpoint: &mut usize,
    cancellation: &AnalysisCancellationToken,
) -> Result<(), AnalysisCancelled> {
    if offset >= *next_checkpoint {
        cancellation.checkpoint()?;
        *next_checkpoint = offset.saturating_add(REWRITE_SCAN_CHECKPOINT_BYTES);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_core::Engine;

    fn migration_fix(source: &str, source_map: &SourceMap) -> Option<DiagnosticFix> {
        let metadata = merman_core::Engine::new()
            .parse_metadata_sync(source)
            .ok()?;
        init_directives_to_frontmatter_fix(source, source_map, &metadata.config)
    }

    fn apply_fix(source: &str, fix: &DiagnosticFix) -> String {
        let mut edited = source.to_string();
        let mut edits = fix.edits.iter().cloned().collect::<Vec<_>>();
        edits.sort_by(|left, right| {
            right
                .span
                .byte_start
                .cmp(&left.span.byte_start)
                .then_with(|| right.span.byte_end.cmp(&left.span.byte_end))
        });

        for edit in edits {
            edited.replace_range(edit.span.byte_start..edit.span.byte_end, &edit.replacement);
        }

        edited
    }

    #[test]
    fn init_directive_migration_observes_cancellation_during_fix_construction() {
        let mut source = "%%{ init: { theme: 'dark' } }%%\n".repeat(512);
        source.push_str("flowchart TD\nA-->B\n");
        let source_map = SourceMap::new(source.as_str());
        let config = MermaidConfig::from_value(serde_json::json!({ "theme": "dark" }));
        let cancellation = AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(8);

        assert!(matches!(
            init_directives_to_frontmatter_fix_cancellable(
                &source,
                &source_map,
                &config,
                &cancellation,
            ),
            Err(AnalysisCancelled)
        ));
    }

    #[test]
    fn bounded_yaml_writer_observes_cancellation_across_output_chunks() {
        let cancellation = AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(1);
        let mut writer = BoundedYamlWriter::new(&cancellation);

        assert!(
            std::fmt::Write::write_str(
                &mut writer,
                &"x".repeat(REWRITE_SCAN_CHECKPOINT_BYTES * 3),
            )
            .is_err()
        );
        assert_eq!(writer.failure, Some(BoundedWriteFailure::Cancelled));
    }

    #[test]
    fn frontmatter_migration_skips_oversized_existing_frontmatter() {
        let oversized = "x".repeat(MAX_FRONTMATTER_MIGRATION_INPUT_BYTES + 1);
        let source = format!("---\nnotes: {oversized}\n---\nflowchart TD\nA-->B\n");
        let source_map = SourceMap::new(source.as_str());

        let fix = frontmatter_config_fix(
            &source,
            &source_map,
            serde_json::json!({ "theme": "dark" }),
            Vec::new(),
            "Add frontmatter config",
        );

        assert!(fix.is_none());
    }

    #[test]
    fn frontmatter_migration_skips_indent_amplified_output() {
        let indent = " ".repeat(32 * 1024);
        let source =
            format!("{indent}---\n{indent}title: Demo\n{indent}---\nflowchart TD\nA-->B\n");
        let source_map = SourceMap::new(source.as_str());
        let config = Value::Object(
            (0..96)
                .map(|index| (format!("setting_{index}"), Value::Bool(true)))
                .collect(),
        );

        let fix = frontmatter_config_fix(
            &source,
            &source_map,
            config,
            Vec::new(),
            "Add frontmatter config",
        );

        assert!(fix.is_none());
    }

    fn assert_only_crlf_newlines(text: &str) {
        let bytes = text.as_bytes();
        for (index, byte) in bytes.iter().enumerate() {
            if *byte == b'\n' {
                assert!(index > 0 && bytes[index - 1] == b'\r');
            }
        }
    }

    #[test]
    fn init_directive_migration_inserts_frontmatter_and_removes_directive_line() {
        let source = "%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        let fix = migration_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);

        assert!(fix.is_preferred);
        assert!(edited.starts_with("---\nconfig:\n"));
        assert!(edited.contains("theme: dark\n"));
        assert!(!edited.contains("%%{ init"));
        assert!(edited.contains("flowchart TD\nA-->B\n"));
        assert_eq!(fix.edits.len(), 1);
    }

    #[test]
    fn init_directive_migration_preserves_existing_frontmatter_fields() {
        let source = "---\ntitle: Demo\ncustom: keep\nconfig:\n  theme: default\n---\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        let fix = migration_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);

        assert!(edited.starts_with("---\ntitle: Demo\ncustom: keep\nconfig:\n"));
        assert!(edited.contains("theme: dark\n"));
        assert!(!edited.contains("%%{ init"));
        assert_eq!(fix.edits.len(), 2);
    }

    #[test]
    fn serde_saphyr_rewrite_preserves_yaml_values_order_and_document_shape() {
        let source = "flowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);
        let config = serde_json::json!({
            "quoted_true": "true",
            "quoted_null": "null",
            "quoted_number": "001",
            "null_value": null,
            "multiline": "first line\nsecond line",
            "ordered_first": "first",
            "ordered_second": "second"
        });

        let fix = frontmatter_config_fix(
            source,
            &source_map,
            config.clone(),
            Vec::new(),
            "Add frontmatter config",
        )
        .expect("frontmatter fix");
        let edited = apply_fix(source, &fix);
        let frontmatter = split_frontmatter_block(&edited).expect("generated frontmatter");
        let document = &edited[frontmatter.full.start..frontmatter.full.end];
        let body = frontmatter.dedented_body.as_ref();
        let fields = parse_frontmatter_yaml_fields(body).expect("parse generated frontmatter");

        assert_eq!(fields.get("config"), Some(&config));
        assert!(document.starts_with("---\n"));
        assert!(document.ends_with("---\n"));
        assert!(!document.ends_with("---\n\n"));
        assert_eq!(document.matches("---").count(), 2);

        for key in ["quoted_true", "quoted_null", "quoted_number"] {
            let line = body
                .lines()
                .find(|line| line.trim_start().starts_with(&format!("{key}:")))
                .expect("serialized string field");
            let value = line
                .split_once(':')
                .map(|(_, value)| value.trim())
                .expect("serialized scalar value");
            assert!(
                (value.starts_with('\'') && value.ends_with('\''))
                    || (value.starts_with('"') && value.ends_with('"')),
                "ambiguous YAML string must remain quoted: {line}"
            );
        }

        let ordered_keys = [
            "quoted_true:",
            "quoted_null:",
            "quoted_number:",
            "null_value:",
            "multiline:",
            "ordered_first:",
            "ordered_second:",
        ];
        let positions = ordered_keys.map(|key| body.find(key).expect("serialized key position"));
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn init_directive_migration_preserves_crlf_when_creating_frontmatter() {
        let source = "%%{ init: {\"theme\":\"dark\"} }%%\r\nflowchart TD\r\nA-->B\r\n";
        let source_map = SourceMap::new(source);

        let fix = migration_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);

        assert!(edited.starts_with("---\r\nconfig:\r\n"));
        assert!(edited.contains("theme: dark\r\n"));
        assert!(!edited.contains("%%{ init"));
        assert_only_crlf_newlines(&edited);
    }

    #[test]
    fn init_directive_migration_preserves_crlf_when_inserting_config() {
        let source = "---\r\ntitle: Demo\r\ncustom: keep\r\n---\r\n%%{ init: {\"theme\":\"dark\"} }%%\r\nflowchart TD\r\nA-->B\r\n";
        let source_map = SourceMap::new(source);

        let fix = migration_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);

        assert!(
            edited.starts_with("---\r\ntitle: Demo\r\ncustom: keep\r\nconfig:\r\n"),
            "{edited:?}"
        );
        assert!(edited.contains("theme: dark\r\n"));
        assert!(!edited.contains("%%{ init"));
        assert_only_crlf_newlines(&edited);
    }

    #[test]
    fn init_directive_migration_preserves_crlf_when_rewriting_config() {
        let source = "---\r\ntitle: Demo\r\nconfig:\r\n  theme: default\r\n---\r\n%%{ init: {\"theme\":\"dark\"} }%%\r\nflowchart TD\r\nA-->B\r\n";
        let source_map = SourceMap::new(source);

        let fix = migration_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);

        assert!(edited.starts_with("---\r\ntitle: Demo\r\nconfig:\r\n"));
        assert!(edited.contains("theme: dark\r\n"));
        assert!(!edited.contains("%%{ init"));
        assert_only_crlf_newlines(&edited);
    }

    #[test]
    fn init_directive_migration_inserts_config_without_dropping_frontmatter_comments() {
        let source = "---\n# keep rationale\ntitle: Demo\ncustom: keep\n---\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        let fix = migration_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);

        assert!(edited.starts_with("---\n# keep rationale\ntitle: Demo\ncustom: keep\nconfig:\n"));
        assert!(edited.contains("theme: dark\n"));
        assert!(!edited.contains("%%{ init"));
        assert_eq!(edited.matches("# keep rationale").count(), 1);
        assert_eq!(fix.edits.len(), 2);
    }

    #[test]
    fn init_directive_migration_skips_lossy_config_rewrite_for_commented_frontmatter() {
        let source = "---\n# keep rationale\ntitle: Demo\nconfig:\n  theme: default\n---\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        assert!(migration_fix(source, &source_map).is_none());
    }

    #[test]
    fn init_directive_migration_ignores_non_string_frontmatter_keys_without_dropping_fields() {
        let source = "---\ntitle: Demo\n? [non, string, key]\n: ignored\ncustom: keep\nconfig:\n  theme: default\n---\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        assert!(migration_fix(source, &source_map).is_none());
    }

    #[test]
    fn init_directive_migration_skips_lossy_config_rewrite_for_block_scalar_frontmatter() {
        let source = "---\ntitle: Demo\nnotes: |\n  keep exact text\nconfig:\n  theme: default\n---\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);

        assert!(migration_fix(source, &source_map).is_none());
    }

    #[test]
    fn init_directive_migration_skips_lossy_config_rewrite_for_flow_style_complex_keys() {
        for source in [
            "---\ntitle: Demo\n[non, string, key]: ignored\nconfig:\n  theme: default\n---\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n",
            "---\ntitle: Demo\nmetadata:\n  [non, string, key]: ignored\nconfig:\n  theme: default\n---\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n",
        ] {
            let source_map = SourceMap::new(source);

            assert!(migration_fix(source, &source_map).is_none());
        }
    }

    #[test]
    fn init_directive_migration_preserves_effective_diagram_config() {
        let source = "---\nconfig:\n  flowchart:\n    curve: basis\n---\n%%{ initialize: {\"theme\":\"dark\",\"flowchart\":{\"htmlLabels\":false}} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);
        let engine = Engine::new();
        let original = engine
            .parse_metadata_sync(source)
            .expect("original metadata");

        let fix = migration_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);
        let migrated = engine
            .parse_metadata_sync(&edited)
            .expect("migrated metadata");

        assert_eq!(migrated.config.as_value(), original.config.as_value());
        assert!(!edited.contains("%%{ initialize"));
    }

    #[test]
    fn init_directive_migration_updates_indented_frontmatter_with_core_semantics() {
        let source = "  ---\n  title: Demo\n  config:\n    theme: default\n  ---\n%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
        let source_map = SourceMap::new(source);
        let engine = Engine::new();
        let original = engine
            .parse_metadata_sync(source)
            .expect("original metadata");

        let fix = migration_fix(source, &source_map).expect("migration fix");
        let edited = apply_fix(source, &fix);
        let migrated = engine
            .parse_metadata_sync(&edited)
            .expect("migrated metadata");

        assert!(edited.starts_with("  ---\n"));
        assert!(!edited.starts_with("---\n  ---\n"));
        assert_eq!(edited.matches("title: Demo").count(), 1);
        assert!(!edited.contains("%%{ init"));
        assert_eq!(migrated.config.as_value(), original.config.as_value());
        assert_eq!(migrated.title.as_deref(), Some("Demo"));
    }
}
