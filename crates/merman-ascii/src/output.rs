use crate::color::AsciiColorMode;
use crate::operation::AsciiExecution;
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::safe_text::terminal_line_display_width;
use crate::{AsciiError, Result};
use merman_core::{OperationPhase, ParseMetadata};
use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;

pub const ASCII_OUTPUT_SCHEMA_VERSION: u16 = 1;
const EXTENT_CHECKPOINT_INTERVAL: usize = 64;

/// 宽度超限时的输出策略。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverflowPolicy {
    /// 保留完整主投影，即使它超过请求宽度。
    #[default]
    Allow,
    /// 尝试完整的结构化文本投影；无法安全适配时返回错误。
    Fallback,
    /// 宽度超限立即返回独立的宽度错误。
    Error,
}

impl OverflowPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Fallback => "fallback",
            Self::Error => "error",
        }
    }
}

/// `OverflowPolicy` 的 ASCII 命名别名，便于调用方表达目标边界。
pub type AsciiOverflowPolicy = OverflowPolicy;

/// 输出尾部空格处理策略。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AsciiTrimPolicy {
    #[default]
    Preserve,
    TrimTrailingSpaces,
}

impl AsciiTrimPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preserve => "preserve",
            Self::TrimTrailingSpaces => "trim_trailing_spaces",
        }
    }
}

/// Provider-neutral 的终端 viewport 请求策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AsciiViewportPolicy {
    pub max_width: Option<usize>,
    pub overflow: OverflowPolicy,
    pub trim: AsciiTrimPolicy,
}

impl AsciiViewportPolicy {
    pub const fn unrestricted() -> Self {
        Self {
            max_width: None,
            overflow: OverflowPolicy::Allow,
            trim: AsciiTrimPolicy::Preserve,
        }
    }

    pub const fn with_max_width(max_width: usize) -> Self {
        Self {
            max_width: Some(max_width),
            ..Self::unrestricted()
        }
    }

    pub const fn max_width(mut self, max_width: usize) -> Self {
        self.max_width = Some(max_width);
        self
    }

    pub const fn overflow(mut self, overflow: OverflowPolicy) -> Self {
        self.overflow = overflow;
        self
    }

    pub const fn trim(mut self, trim: AsciiTrimPolicy) -> Self {
        self.trim = trim;
        self
    }

    pub fn validate(self) -> Result<()> {
        if self.max_width == Some(0) {
            return Err(AsciiError::InvalidOption {
                field: "ascii_viewport.max_width",
                message: "must be greater than 0",
            });
        }
        Ok(())
    }
}

/// 逻辑终端 extent，宽度按 display cells、行数按逻辑文本行计算。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AsciiExtent {
    pub width: usize,
    pub height: usize,
}

impl AsciiExtent {
    pub const fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    pub const fn width(self) -> usize {
        self.width
    }

    pub const fn height(self) -> usize {
        self.height
    }

    #[cfg(test)]
    pub(crate) fn measure(text: &str, profile: TerminalWidthProfile) -> Self {
        if text.is_empty() {
            return Self::default();
        }
        let mut extent = Self::default();
        for line in text.split('\n') {
            extent.width = extent.width.max(terminal_line_display_width(line, profile));
            extent.height = extent.height.saturating_add(1);
        }
        extent
    }

    pub(crate) fn measure_with_execution(
        text: &str,
        color_mode: AsciiColorMode,
        profile: TerminalWidthProfile,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        if text.is_empty() {
            return Ok(Self::default());
        }

        let resources = execution.new_resource_context(OperationPhase::Emit);
        let mut extent = Self::default();
        for (line_index, line) in text.split('\n').enumerate() {
            execution.checkpoint(OperationPhase::Emit)?;
            resources.charge_layout_work(1)?;
            let visible = match color_mode {
                AsciiColorMode::Plain => std::borrow::Cow::Borrowed(line),
                AsciiColorMode::Ansi16
                | AsciiColorMode::Ansi256
                | AsciiColorMode::TrueColor
                | AsciiColorMode::Html => std::borrow::Cow::Owned(strip_encoding(line, color_mode)),
            };
            let mut line_width = 0usize;
            let mut grapheme_count = 0usize;
            for (grapheme_index, grapheme) in visible.graphemes(true).enumerate() {
                execution.checkpoint_loop(OperationPhase::Emit, grapheme_index)?;
                line_width = resources
                    .checked_grid_add(line_width, terminal_line_display_width(grapheme, profile))?;
                grapheme_count = resources.checked_grid_add(grapheme_count, 1)?;
            }
            extent.width = extent.width.max(line_width);
            extent.height = resources.checked_grid_add(line_index, 1)?;

            let grapheme_work = grapheme_count.div_ceil(EXTENT_CHECKPOINT_INTERVAL);
            resources.charge_layout_work(grapheme_work)?;
            execution.checkpoint_loop(OperationPhase::Emit, line_index)?;
        }
        Ok(extent)
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsciiProjection {
    Diagrammatic,
    StructuredText,
}

impl AsciiProjection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Diagrammatic => "diagrammatic",
            Self::StructuredText => "structured_text",
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsciiOutputOutcome {
    Primary,
    WideAllowed,
    Fallback,
    Empty,
}

impl AsciiOutputOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::WideAllowed => "wide_allowed",
            Self::Fallback => "fallback",
            Self::Empty => "empty",
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsciiFallbackCapability {
    Unsupported,
    Available,
}

impl AsciiFallbackCapability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::Available => "available",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FallbackMetadata {
    pub capability: AsciiFallbackCapability,
    pub attempted: bool,
    pub reason: Option<AsciiFallbackReason>,
}

impl Default for FallbackMetadata {
    fn default() -> Self {
        Self {
            capability: AsciiFallbackCapability::Unsupported,
            attempted: false,
            reason: None,
        }
    }
}

/// Stable reason attached to a fallback attempt.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsciiFallbackReason {
    PrimaryOverflow,
}

impl AsciiFallbackReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PrimaryOverflow => "primary_overflow",
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lossiness {
    #[default]
    None,
    /// The fallback retains authored semantic fields but not the primary diagrammatic layout.
    PresentationOnly,
}

impl Lossiness {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::PresentationOnly => "presentation_only",
        }
    }
}

/// ASCII 输出的机器可读报告；`text` 仍是完整的兼容文本投影。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsciiOutput {
    pub schema_version: u16,
    pub family: String,
    pub projection: AsciiProjection,
    pub text: String,
    pub primary_extent: AsciiExtent,
    pub emitted_extent: AsciiExtent,
    pub width_profile: TerminalWidthProfile,
    pub layout_profile: crate::options::AsciiLayoutProfile,
    pub requested_max_width: Option<usize>,
    pub overflowed: bool,
    pub outcome: AsciiOutputOutcome,
    pub fallback: FallbackMetadata,
    pub trimmed: bool,
    pub lossiness: Lossiness,
}

#[derive(Clone, Copy)]
pub(crate) struct OutputBuildContext<'a> {
    pub color_mode: AsciiColorMode,
    pub profile: TerminalWidthProfile,
    pub layout_profile: crate::options::AsciiLayoutProfile,
    pub policy: AsciiViewportPolicy,
    pub execution: crate::operation::AsciiExecution<'a>,
}

impl AsciiOutput {
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }

    pub fn into_text(self) -> String {
        self.text
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.text.into_bytes()
    }

    pub fn as_text(&self) -> &str {
        &self.text
    }
}

pub(crate) fn build_output(
    family: &str,
    primary_text: String,
    projection: AsciiProjection,
    primary_extent: AsciiExtent,
    context: OutputBuildContext<'_>,
) -> Result<AsciiOutput> {
    let OutputBuildContext {
        color_mode,
        profile,
        layout_profile,
        policy,
        execution,
    } = context;
    policy.validate()?;
    let overflowed = policy
        .max_width
        .is_some_and(|max_width| primary_extent.width > max_width);

    if overflowed {
        match policy.overflow {
            OverflowPolicy::Error => {
                return Err(AsciiError::WidthOverflow {
                    max_width: policy.max_width.expect("overflow requires a bound"),
                    actual_width: primary_extent.width,
                    profile,
                });
            }
            OverflowPolicy::Fallback => {
                return Err(AsciiError::FallbackUnavailable {
                    diagram_type: family.to_string(),
                    max_width: policy.max_width.expect("overflow requires a bound"),
                    actual_width: primary_extent.width,
                });
            }
            OverflowPolicy::Allow => {}
        }
    }

    let primary_is_empty = primary_text.is_empty();
    let (emitted_text, emitted_extent, trimmed) = match policy.trim {
        AsciiTrimPolicy::Preserve => (primary_text, primary_extent, false),
        AsciiTrimPolicy::TrimTrailingSpaces => {
            let emitted_text = trim_text(&primary_text, policy.trim);
            let trimmed = emitted_text != primary_text;
            let emitted_extent = if trimmed {
                AsciiExtent::measure_with_execution(&emitted_text, color_mode, profile, execution)?
            } else {
                primary_extent
            };
            (emitted_text, emitted_extent, trimmed)
        }
    };
    execution.checkpoint(OperationPhase::Emit)?;
    Ok(AsciiOutput {
        schema_version: crate::ASCII_OUTPUT_SCHEMA_VERSION,
        family: family.to_string(),
        projection,
        text: emitted_text,
        primary_extent,
        emitted_extent,
        width_profile: profile,
        layout_profile,
        requested_max_width: policy.max_width,
        overflowed,
        outcome: if overflowed {
            AsciiOutputOutcome::WideAllowed
        } else {
            if primary_is_empty {
                AsciiOutputOutcome::Empty
            } else {
                AsciiOutputOutcome::Primary
            }
        },
        fallback: FallbackMetadata::default(),
        trimmed,
        lossiness: Lossiness::None,
    })
}

pub(crate) fn build_structured_fallback(
    family: &str,
    primary_text: String,
    primary_extent: AsciiExtent,
    context: OutputBuildContext<'_>,
) -> Result<AsciiOutput> {
    let OutputBuildContext {
        color_mode,
        profile,
        layout_profile,
        policy,
        execution,
    } = context;
    policy.validate()?;
    let max_width = policy.max_width.expect("fallback requires a width bound");
    if color_mode != AsciiColorMode::Plain {
        return Err(AsciiError::FallbackUnavailable {
            diagram_type: family.to_string(),
            max_width,
            actual_width: primary_extent.width,
        });
    }
    let reflowed = reflow_text(&primary_text, max_width, profile, execution)?;
    let emitted_text = trim_text(&reflowed, policy.trim);
    let trimmed = emitted_text != reflowed;
    let emitted_extent =
        AsciiExtent::measure_with_execution(&emitted_text, color_mode, profile, execution)?;
    if emitted_extent.width > max_width {
        return Err(AsciiError::FallbackUnavailable {
            diagram_type: family.to_string(),
            max_width,
            actual_width: emitted_extent.width,
        });
    }
    execution.admit_fallback_output(&emitted_text, profile)?;
    Ok(AsciiOutput {
        schema_version: crate::ASCII_OUTPUT_SCHEMA_VERSION,
        family: family.to_string(),
        projection: AsciiProjection::StructuredText,
        text: emitted_text,
        primary_extent,
        emitted_extent,
        width_profile: profile,
        layout_profile,
        requested_max_width: Some(max_width),
        overflowed: true,
        outcome: AsciiOutputOutcome::Fallback,
        fallback: FallbackMetadata {
            capability: AsciiFallbackCapability::Available,
            attempted: true,
            reason: Some(AsciiFallbackReason::PrimaryOverflow),
        },
        trimmed,
        lossiness: Lossiness::None,
    })
}

/// Builds a complete structured fallback from the typed compatibility projection.
///
/// This path is intentionally model-only: it never reparses Mermaid source and never truncates
/// authored values. The flattened representation keeps field paths visible to agents while still
/// allowing the shared grapheme-safe reflow to satisfy a narrow terminal viewport.
pub(crate) fn build_semantic_fallback(
    model: &merman_core::diagram::RenderSemanticModel,
    metadata: &ParseMetadata,
    primary_extent: AsciiExtent,
    context: OutputBuildContext<'_>,
) -> Result<AsciiOutput> {
    let OutputBuildContext {
        color_mode,
        profile,
        layout_profile,
        policy,
        execution,
    } = context;
    policy.validate()?;
    let max_width = policy
        .max_width
        .expect("semantic fallback requires a width bound");
    if color_mode != AsciiColorMode::Plain {
        return Err(AsciiError::FallbackUnavailable {
            diagram_type: model.kind().to_string(),
            max_width,
            actual_width: primary_extent.width,
        });
    }
    let control = execution.cloned_control();
    let value =
        semantic_fallback_value(model, metadata, &control).map_err(|error| match error {
            SemanticFallbackError::Cancelled(cancelled) => AsciiError::Cancelled(cancelled),
            SemanticFallbackError::Unavailable => AsciiError::FallbackUnavailable {
                diagram_type: model.kind().to_string(),
                max_width,
                actual_width: primary_extent.width,
            },
        })?;
    let mut fallback = SemanticFallbackWriter::new(execution, max_width, profile);
    fallback.push(format!("family: {}", model.kind()))?;
    flatten_json_value("model", &value, &mut fallback)?;
    let reflowed = fallback.finish();
    let emitted_text = trim_text(&reflowed, policy.trim);
    let trimmed = emitted_text != reflowed;
    let emitted_extent =
        AsciiExtent::measure_with_execution(&emitted_text, color_mode, profile, execution)?;
    if emitted_extent.width > max_width {
        return Err(AsciiError::FallbackUnavailable {
            diagram_type: model.kind().to_string(),
            max_width,
            actual_width: emitted_extent.width,
        });
    }
    execution.admit_fallback_output(&emitted_text, profile)?;
    Ok(AsciiOutput {
        schema_version: crate::ASCII_OUTPUT_SCHEMA_VERSION,
        family: model.kind().to_string(),
        projection: AsciiProjection::StructuredText,
        text: emitted_text,
        primary_extent,
        emitted_extent,
        width_profile: profile,
        layout_profile,
        requested_max_width: Some(max_width),
        overflowed: true,
        outcome: AsciiOutputOutcome::Fallback,
        fallback: FallbackMetadata {
            capability: AsciiFallbackCapability::Available,
            attempted: true,
            reason: Some(AsciiFallbackReason::PrimaryOverflow),
        },
        trimmed,
        lossiness: Lossiness::PresentationOnly,
    })
}

enum SemanticFallbackError {
    Cancelled(merman_core::OperationCancelled),
    Unavailable,
}

/// Returns a complete typed projection for fallback without reparsing source text.
///
/// Flowchart compatibility JSON intentionally removes render-only edge fields for Mermaid's
/// legacy semantic contract. The agent-facing fallback needs those authored edge semantics, so
/// this path serializes the typed render model directly and only adds the stable family marker.
/// Other families continue to use their existing family-owned compatibility projection until
/// their field-level fallback coverage is admitted explicitly.
fn semantic_fallback_value(
    model: &merman_core::diagram::RenderSemanticModel,
    metadata: &ParseMetadata,
    control: &merman_core::OperationControl,
) -> std::result::Result<serde_json::Value, SemanticFallbackError> {
    control
        .checkpoint()
        .map_err(SemanticFallbackError::Cancelled)?;
    let value = match model {
        merman_core::diagram::RenderSemanticModel::Flowchart(flowchart) => {
            serde_json::to_value(flowchart).map_err(|_| SemanticFallbackError::Unavailable)?
        }
        _ => model
            .compatibility_json_controlled(metadata, control)
            .map_err(SemanticFallbackError::Cancelled)?
            .map_err(|_| SemanticFallbackError::Unavailable)?,
    };
    control
        .checkpoint()
        .map_err(SemanticFallbackError::Cancelled)?;
    let mut value = value;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "type".to_string(),
            serde_json::Value::String(metadata.diagram_type.clone()),
        );
    }
    Ok(value)
}

struct SemanticFallbackWriter<'a> {
    output: String,
    semantic_resources: ResourceContext,
    emit_resources: ResourceContext,
    execution: AsciiExecution<'a>,
    max_width: usize,
    profile: TerminalWidthProfile,
    output_bytes: usize,
    document_cells: usize,
    work: usize,
    line_count: usize,
}

impl<'a> SemanticFallbackWriter<'a> {
    fn new(execution: AsciiExecution<'a>, max_width: usize, profile: TerminalWidthProfile) -> Self {
        Self {
            output: String::new(),
            semantic_resources: execution.new_resource_context(OperationPhase::Semantic),
            emit_resources: execution.new_resource_context(OperationPhase::Emit),
            execution,
            max_width,
            profile,
            output_bytes: 0,
            document_cells: 0,
            work: 0,
            line_count: 0,
        }
    }

    fn push(&mut self, line: String) -> Result<()> {
        self.execution.checkpoint(OperationPhase::Semantic)?;
        if self.line_count > 0 {
            self.push_fragment("\n")?;
        }
        let mut width = 0usize;
        let mut has_content = false;
        for grapheme in line.graphemes(true) {
            self.emit_resources.charge_layout_work(1)?;
            self.execution
                .checkpoint_loop(OperationPhase::Emit, self.work)?;
            self.work = self.work.saturating_add(1);
            let grapheme_width = terminal_line_display_width(grapheme, self.profile);
            if has_content
                && self
                    .emit_resources
                    .checked_grid_add(width, grapheme_width)?
                    > self.max_width
            {
                self.push_fragment("\n")?;
                width = 0;
            }
            let next_width = self
                .emit_resources
                .checked_grid_add(width, grapheme_width)?;
            let next_document_cells = self
                .emit_resources
                .checked_grid_add(self.document_cells, grapheme_width)?;
            self.emit_resources
                .check(AsciiResourceLimitId::MaxDocumentCells, next_document_cells)?;
            self.push_fragment(grapheme)?;
            width = next_width;
            self.document_cells = next_document_cells;
            has_content = true;
        }
        self.line_count = self.emit_resources.checked_grid_add(self.line_count, 1)?;
        Ok(())
    }

    fn admit_node(&self, depth: usize) -> Result<()> {
        self.semantic_resources.charge_layout_work(1)?;
        self.semantic_resources.check_nesting_depth(depth)
    }

    fn push_fragment(&mut self, fragment: &str) -> Result<()> {
        self.emit_resources
            .check(AsciiResourceLimitId::MaxGraphemeBytes, fragment.len())?;
        let output_bytes = self
            .output_bytes
            .checked_add(fragment.len())
            .ok_or_else(|| {
                self.emit_resources
                    .overflow(AsciiResourceLimitId::MaxOutputBytes)
            })?;
        self.emit_resources
            .check(AsciiResourceLimitId::MaxOutputBytes, output_bytes)?;
        self.output
            .try_reserve(fragment.len())
            .map_err(|_| AsciiError::allocation_failed("ascii_semantic_fallback"))?;
        self.output.push_str(fragment);
        self.output_bytes = output_bytes;
        Ok(())
    }

    fn finish(self) -> String {
        self.output
    }
}

fn flatten_json_value(
    path: &str,
    value: &Value,
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<()> {
    fn visit(
        path: &str,
        value: &Value,
        lines: &mut SemanticFallbackWriter<'_>,
        depth: usize,
    ) -> Result<()> {
        lines.admit_node(depth)?;
        match value {
            Value::Object(object) => {
                let mut keys = object.keys().collect::<Vec<_>>();
                keys.sort();
                for key in keys {
                    let child_path = append_path_key(path, key);
                    visit(&child_path, &object[key], lines, depth + 1)?;
                }
            }
            Value::Array(values) => {
                for (index, value) in values.iter().enumerate() {
                    visit(&format!("{path}[{index}]"), value, lines, depth + 1)?;
                }
                if values.is_empty() {
                    lines.push(format!("{path}: []"))?;
                }
            }
            Value::String(value) => lines.push(format!("{path}: {value:?}"))?,
            Value::Number(value) => lines.push(format!("{path}: {value}"))?,
            Value::Bool(value) => lines.push(format!("{path}: {value}"))?,
            Value::Null => lines.push(format!("{path}: null"))?,
        }
        Ok(())
    }

    visit(path, value, lines, 0)
}

fn append_path_key(path: &str, key: &str) -> String {
    if key.is_empty()
        || key.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || matches!(character, '.' | '[' | ']' | ':' | '\\' | '"')
        })
    {
        format!("{path}[{key:?}]")
    } else {
        format!("{path}.{key}")
    }
}

fn trim_text(text: &str, trim: AsciiTrimPolicy) -> String {
    match trim {
        AsciiTrimPolicy::Preserve => text.to_string(),
        AsciiTrimPolicy::TrimTrailingSpaces => {
            let mut output = String::with_capacity(text.len());
            for (index, line) in text.split('\n').enumerate() {
                if index > 0 {
                    output.push('\n');
                }
                output.push_str(line.trim_end_matches([' ', '\t']));
            }
            output
        }
    }
}

fn reflow_text(
    text: &str,
    max_width: usize,
    profile: TerminalWidthProfile,
    execution: crate::operation::AsciiExecution<'_>,
) -> Result<String> {
    reflow_lines(text.split('\n'), max_width, profile, execution)
}

fn reflow_lines<'a, I>(
    lines: I,
    max_width: usize,
    profile: TerminalWidthProfile,
    execution: crate::operation::AsciiExecution<'_>,
) -> Result<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut output = String::new();
    let resources = execution.new_resource_context(OperationPhase::Emit);
    let mut work = 0usize;
    // `split('\n')` deliberately retains trailing empty rows so authored hard breaks and a final
    // newline survive the bounded projection. `str::lines()` would silently erase both.
    for (line_index, line) in lines.into_iter().enumerate() {
        execution.checkpoint(OperationPhase::Emit)?;
        if line_index > 0 {
            output.push('\n');
        }
        let mut width = 0usize;
        let mut has_content = false;
        for grapheme in line.graphemes(true) {
            resources.charge_layout_work(1)?;
            execution.checkpoint_loop(OperationPhase::Emit, work)?;
            work = work.saturating_add(1);
            let grapheme_width = terminal_line_display_width(grapheme, profile);
            if has_content && width.saturating_add(grapheme_width) > max_width {
                output.push('\n');
                width = 0;
            }
            output.push_str(grapheme);
            width = width.saturating_add(grapheme_width);
            has_content = true;
        }
    }
    Ok(output)
}

fn strip_encoding(text: &str, color_mode: AsciiColorMode) -> String {
    match color_mode {
        AsciiColorMode::Plain => text.to_string(),
        AsciiColorMode::Ansi16 | AsciiColorMode::Ansi256 | AsciiColorMode::TrueColor => {
            strip_ansi(text)
        }
        AsciiColorMode::Html => strip_html(text),
    }
}

fn strip_ansi(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            output.push(ch);
            continue;
        }
        if chars.next() != Some('[') {
            continue;
        }
        for code in chars.by_ref() {
            if code.is_ascii_alphabetic() {
                break;
            }
        }
    }
    output
}

fn strip_html(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '<' {
            for tag in chars.by_ref() {
                if tag == '>' {
                    break;
                }
            }
            continue;
        }
        if ch == '&' {
            let mut entity = String::new();
            while let Some(&next) = chars.peek() {
                entity.push(next);
                chars.next();
                if next == ';' || entity.len() >= 16 {
                    break;
                }
            }
            match entity.as_str() {
                "amp;" => output.push('&'),
                "lt;" => output.push('<'),
                "gt;" => output.push('>'),
                "quot;" => output.push('"'),
                "#39;" => output.push('\''),
                _ => {
                    output.push('&');
                    output.push_str(&entity);
                }
            }
            continue;
        }
        output.push(ch);
    }
    output
}

pub(crate) fn supports_structured_fallback(
    model: &merman_core::diagram::RenderSemanticModel,
) -> bool {
    let Some(capability) = crate::ascii_capabilities()
        .iter()
        .find(|capability| capability.diagram_type == model.kind())
    else {
        return false;
    };
    capability.structured_text_fallback
}

pub(crate) fn projection_for(
    model: &merman_core::diagram::RenderSemanticModel,
    text: &str,
) -> AsciiProjection {
    let capability_projection = crate::ascii_capabilities()
        .iter()
        .find(|capability| capability.diagram_type == model.kind())
        .map(|capability| capability.primary_projection);
    if capability_projection == Some(crate::AsciiPrimaryProjection::StructuredText)
        || is_structured_projection_text(text)
    {
        AsciiProjection::StructuredText
    } else {
        AsciiProjection::Diagrammatic
    }
}

fn is_structured_projection_text(text: &str) -> bool {
    text.starts_with("relations:\n")
        || text.starts_with("title(bytes=")
        || text.starts_with("direction: ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_extent_is_zero_by_zero() {
        assert_eq!(
            AsciiExtent::measure("", TerminalWidthProfile::Unicode),
            AsciiExtent::default()
        );
    }

    #[test]
    fn extent_preserves_trailing_empty_rows() {
        assert_eq!(
            AsciiExtent::measure("alpha\n\n", TerminalWidthProfile::Unicode),
            AsciiExtent::new(5, 3)
        );
    }

    #[test]
    fn viewport_rejects_zero_width() {
        let error = AsciiViewportPolicy::with_max_width(0)
            .validate()
            .expect_err("zero viewport width must be rejected");
        assert!(matches!(
            error,
            AsciiError::InvalidOption {
                field: "ascii_viewport.max_width",
                ..
            }
        ));
    }

    #[test]
    fn reflow_preserves_graphemes_and_fits_bound() {
        let context = merman_core::runtime::RuntimePolicy::deterministic()
            .begin_operation()
            .expect("operation context");
        let control = merman_core::OperationControl::new();
        let resources = crate::AsciiResourcePolicy::default();
        let execution = crate::operation::AsciiExecution::new(&control, &resources);
        let text = reflow_text("é 👩‍💻 超长", 4, TerminalWidthProfile::Unicode, execution)
            .expect("grapheme-safe reflow");
        let _ = context;
        assert!(
            text.lines()
                .all(|line| AsciiExtent::measure(line, TerminalWidthProfile::Unicode).width <= 4)
        );
        assert!(text.contains("é"));
        assert!(text.contains("👩‍💻"));
        assert!(text.contains("超"));
    }

    #[test]
    fn trim_policy_preserves_newlines_and_only_removes_row_suffixes() {
        assert_eq!(
            trim_text("a  \n\n b\t", AsciiTrimPolicy::TrimTrailingSpaces),
            "a\n\n b"
        );
        assert_eq!(
            trim_text("a  \n\n b\t", AsciiTrimPolicy::Preserve),
            "a  \n\n b\t"
        );
    }

    #[test]
    fn reflow_preserves_trailing_newlines() {
        let resources = crate::AsciiResourcePolicy::unbounded();
        let execution = crate::operation::AsciiExecution::for_test(&resources);
        let reflowed = reflow_text("alpha\n\n", 3, TerminalWidthProfile::Unicode, execution)
            .expect("reflow should preserve empty trailing rows");
        assert_eq!(reflowed, "alp\nha\n\n");
    }
}
