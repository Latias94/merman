use crate::color::AsciiColorMode;
use crate::operation::AsciiExecution;
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy, ResourceContext};
use crate::text::display_width_with_profile;
use crate::{AsciiError, Result};
use merman_core::{OperationPhase, ParseMetadata};
use serde::Serialize;
use serde_json::Value;
use serde_json::value::RawValue;
use std::collections::BTreeMap;
use std::io::{self, Write as IoWrite};
use unicode_segmentation::UnicodeSegmentation;

pub const ASCII_OUTPUT_SCHEMA_VERSION: u16 = 2;
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

/// Encoded representation carried by the emitted output bytes.
///
/// This is intentionally distinct from [`AsciiColorMode`]: color mode is a render request,
/// while output encoding is stable result metadata that transport consumers can inspect without
/// scanning the returned bytes.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsciiOutputEncoding {
    Plain,
    Ansi16,
    Ansi256,
    TrueColor,
    Html,
}

impl AsciiOutputEncoding {
    #[must_use]
    pub const fn from_color_mode(color_mode: AsciiColorMode) -> Self {
        match color_mode {
            AsciiColorMode::Plain => Self::Plain,
            AsciiColorMode::Ansi16 => Self::Ansi16,
            AsciiColorMode::Ansi256 => Self::Ansi256,
            AsciiColorMode::TrueColor => Self::TrueColor,
            AsciiColorMode::Html => Self::Html,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Ansi16 => "ansi16",
            Self::Ansi256 => "ansi256",
            Self::TrueColor => "truecolor",
            Self::Html => "html",
        }
    }
}

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
        for line in text.lines() {
            extent.width = extent.width.max(display_width_with_profile(line, profile));
            extent.height = extent.height.saturating_add(1);
        }
        extent
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
    pub encoding: AsciiOutputEncoding,
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

/// Metrics observed for one complete terminal candidate.
///
/// The candidate owns its text and the metrics are computed in the same pass. Report construction
/// and fallback admission consume this value instead of rescanning the emitted text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OutputMetrics {
    pub extent: AsciiExtent,
    pub document_cells: usize,
    pub grapheme_bytes: usize,
    pub encoded_bytes: usize,
}

#[derive(Debug)]
pub(crate) struct MeasuredOutput {
    text: String,
    metrics: OutputMetrics,
}

impl MeasuredOutput {
    pub(crate) fn measure(
        text: String,
        color_mode: AsciiColorMode,
        profile: TerminalWidthProfile,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        let encoded_bytes = text.len();
        let metrics = measure_text(&text, color_mode, profile, execution)?;
        Ok(Self {
            text,
            metrics: OutputMetrics {
                encoded_bytes,
                ..metrics
            },
        })
    }

    pub(crate) fn from_plain_metrics(text: String, metrics: OutputMetrics) -> Self {
        Self { text, metrics }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn into_text(self) -> String {
        self.text
    }

    pub(crate) const fn metrics(&self) -> OutputMetrics {
        self.metrics
    }

    pub(crate) fn trim(
        self,
        trim: AsciiTrimPolicy,
        color_mode: AsciiColorMode,
        profile: TerminalWidthProfile,
        execution: AsciiExecution<'_>,
    ) -> Result<(Self, bool)> {
        if trim == AsciiTrimPolicy::Preserve {
            return Ok((self, false));
        }
        let trimmed = trim_text(&self.text, trim);
        if trimmed == self.text {
            return Ok((self, false));
        }
        Ok((
            Self::measure(trimmed, color_mode, profile, execution)?,
            true,
        ))
    }

    pub(crate) fn admit_fallback(&self, execution: AsciiExecution<'_>) -> Result<()> {
        let resources = execution.new_resource_context(OperationPhase::Emit);
        resources.transaction(|resources| {
            resources.charge_document_cells(self.metrics.document_cells)?;
            resources.check(
                AsciiResourceLimitId::MaxOutputBytes,
                self.metrics.encoded_bytes,
            )?;
            resources.check(
                AsciiResourceLimitId::MaxGraphemeBytes,
                self.metrics.grapheme_bytes,
            )?;
            execution.checkpoint(OperationPhase::Emit)
        })
    }
}

/// Canonical metadata payload shared by CLI and binding transport adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct AsciiOutputMetadata {
    pub schema_version: u16,
    pub family: String,
    pub projection: String,
    pub encoding: String,
    pub primary_width: u64,
    pub primary_height: u64,
    pub emitted_width: u64,
    pub emitted_height: u64,
    pub width_profile: String,
    pub layout_profile: String,
    pub requested_max_width: Option<u64>,
    pub overflowed: bool,
    pub outcome: String,
    pub fallback_capability: String,
    pub fallback_attempted: bool,
    pub fallback_reason: Option<String>,
    pub trimmed: bool,
    pub lossiness: String,
}

/// Canonical CLI report payload. Binding metadata intentionally omits `text` because the
/// operation bytes already carry the exact text projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AsciiOutputReport<'a> {
    #[serde(rename = "kind")]
    pub kind: &'static str,
    #[serde(flatten)]
    pub metadata: AsciiOutputMetadata,
    pub text: &'a str,
}

fn measure_text(
    text: &str,
    color_mode: AsciiColorMode,
    profile: TerminalWidthProfile,
    execution: AsciiExecution<'_>,
) -> Result<OutputMetrics> {
    if text.is_empty() {
        return Ok(OutputMetrics {
            extent: AsciiExtent::default(),
            document_cells: 0,
            grapheme_bytes: 0,
            encoded_bytes: 0,
        });
    }

    let resources = execution.new_resource_context(OperationPhase::Emit);
    resources.check(AsciiResourceLimitId::MaxOutputBytes, text.len())?;
    let mut extent = AsciiExtent::default();
    let mut document_cells = 0usize;
    let mut grapheme_bytes = 0usize;
    for (line_index, line) in text.lines().enumerate() {
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
        let mut line_grapheme_count = 0usize;
        for (grapheme_index, grapheme) in visible.graphemes(true).enumerate() {
            execution.checkpoint_loop(OperationPhase::Emit, grapheme_index)?;
            let width = display_width_with_profile(grapheme, profile);
            line_width = line_width
                .checked_add(width)
                .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
            document_cells = document_cells
                .checked_add(width)
                .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
            grapheme_bytes = grapheme_bytes.max(grapheme.len());
            resources.check(AsciiResourceLimitId::MaxGraphemeBytes, grapheme.len())?;
            line_grapheme_count = line_grapheme_count
                .checked_add(1)
                .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxLayoutWorkUnits))?;
        }
        extent.width = extent.width.max(line_width);
        extent.height = line_index
            .checked_add(1)
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxGridCells))?;

        let grapheme_work = line_grapheme_count.div_ceil(EXTENT_CHECKPOINT_INTERVAL);
        resources.charge_layout_work(grapheme_work)?;
        execution.checkpoint_loop(OperationPhase::Emit, line_index)?;
    }
    if text.ends_with('\n') {
        // The final line terminator is encoded work, but it is not another logical content row.
        execution.checkpoint(OperationPhase::Emit)?;
        resources.charge_layout_work(1)?;
    }
    Ok(OutputMetrics {
        extent,
        document_cells,
        grapheme_bytes,
        encoded_bytes: text.len(),
    })
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

    /// Returns the canonical transport metadata used by CLI and binding adapters.
    pub fn metadata(&self) -> AsciiOutputMetadata {
        AsciiOutputMetadata {
            schema_version: self.schema_version,
            family: self.family.clone(),
            projection: self.projection.as_str().to_owned(),
            encoding: self.encoding.as_str().to_owned(),
            primary_width: self.primary_extent.width as u64,
            primary_height: self.primary_extent.height as u64,
            emitted_width: self.emitted_extent.width as u64,
            emitted_height: self.emitted_extent.height as u64,
            width_profile: self.width_profile.as_str().to_owned(),
            layout_profile: self.layout_profile.as_str().to_owned(),
            requested_max_width: self.requested_max_width.map(|value| value as u64),
            overflowed: self.overflowed,
            outcome: self.outcome.as_str().to_owned(),
            fallback_capability: self.fallback.capability.as_str().to_owned(),
            fallback_attempted: self.fallback.attempted,
            fallback_reason: self
                .fallback
                .reason
                .map(|reason| reason.as_str().to_owned()),
            trimmed: self.trimmed,
            lossiness: self.lossiness.as_str().to_owned(),
        }
    }

    /// Returns the complete CLI report, including the exact text projection.
    pub fn report(&self) -> AsciiOutputReport<'_> {
        AsciiOutputReport {
            kind: "ascii",
            metadata: self.metadata(),
            text: self.as_text(),
        }
    }
}

pub(crate) fn build_output(
    family: &str,
    primary: MeasuredOutput,
    projection: AsciiProjection,
    context: OutputBuildContext<'_>,
) -> Result<AsciiOutput> {
    let OutputBuildContext {
        color_mode,
        profile,
        layout_profile: _,
        policy,
        execution,
    } = context;
    policy.validate()?;
    let primary_extent = primary.metrics().extent;
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

    let primary_is_empty = primary.text().is_empty();
    let (emitted, trimmed) = primary.trim(policy.trim, color_mode, profile, execution)?;
    execution.checkpoint(OperationPhase::Emit)?;
    Ok(assemble_output(
        OutputAssembly {
            family,
            projection,
            primary_extent,
            emitted,
            outcome: if overflowed {
                AsciiOutputOutcome::WideAllowed
            } else if primary_is_empty {
                AsciiOutputOutcome::Empty
            } else {
                AsciiOutputOutcome::Primary
            },
            fallback: FallbackMetadata::default(),
            trimmed,
            lossiness: Lossiness::None,
        },
        context,
    ))
}

pub(crate) fn build_structured_fallback(
    family: &str,
    primary: MeasuredOutput,
    context: OutputBuildContext<'_>,
) -> Result<AsciiOutput> {
    let OutputBuildContext {
        color_mode,
        profile,
        layout_profile: _,
        policy,
        execution,
    } = context;
    policy.validate()?;
    let max_width = policy.max_width.expect("fallback requires a width bound");
    let primary_extent = primary.metrics().extent;
    if color_mode != AsciiColorMode::Plain {
        return Err(AsciiError::FallbackUnavailable {
            diagram_type: family.to_string(),
            max_width,
            actual_width: primary_extent.width,
        });
    }
    let reflowed = reflow_text(primary.text(), max_width, profile, execution)?;
    drop(primary);
    let candidate = MeasuredOutput::measure(reflowed, color_mode, profile, execution)?;
    finalize_fallback(family, primary_extent, candidate, Lossiness::None, context)
}

fn finalize_fallback(
    family: &str,
    primary_extent: AsciiExtent,
    candidate: MeasuredOutput,
    lossiness: Lossiness,
    context: OutputBuildContext<'_>,
) -> Result<AsciiOutput> {
    let OutputBuildContext {
        color_mode,
        profile,
        layout_profile: _,
        policy,
        execution,
    } = context;
    policy.validate()?;
    let max_width = policy.max_width.expect("fallback requires a width bound");
    let (emitted, trimmed) = candidate.trim(policy.trim, color_mode, profile, execution)?;
    let emitted_extent = emitted.metrics().extent;
    if emitted_extent.width > max_width {
        return Err(AsciiError::FallbackUnavailable {
            diagram_type: family.to_string(),
            max_width,
            actual_width: emitted_extent.width,
        });
    }
    emitted.admit_fallback(execution)?;
    execution.checkpoint(OperationPhase::Emit)?;
    Ok(assemble_output(
        OutputAssembly {
            family,
            projection: AsciiProjection::StructuredText,
            primary_extent,
            emitted,
            outcome: AsciiOutputOutcome::Fallback,
            fallback: FallbackMetadata {
                capability: AsciiFallbackCapability::Available,
                attempted: true,
                reason: Some(AsciiFallbackReason::PrimaryOverflow),
            },
            trimmed,
            lossiness,
        },
        context,
    ))
}

struct OutputAssembly<'a> {
    family: &'a str,
    projection: AsciiProjection,
    primary_extent: AsciiExtent,
    emitted: MeasuredOutput,
    outcome: AsciiOutputOutcome,
    fallback: FallbackMetadata,
    trimmed: bool,
    lossiness: Lossiness,
}

fn assemble_output(assembly: OutputAssembly<'_>, context: OutputBuildContext<'_>) -> AsciiOutput {
    let OutputBuildContext {
        color_mode,
        profile,
        layout_profile,
        policy,
        execution: _,
    } = context;
    let OutputAssembly {
        family,
        projection,
        primary_extent,
        emitted,
        outcome,
        fallback,
        trimmed,
        lossiness,
    } = assembly;
    let emitted_extent = emitted.metrics().extent;
    let text = emitted.into_text();
    AsciiOutput {
        schema_version: crate::ASCII_OUTPUT_SCHEMA_VERSION,
        family: family.to_string(),
        projection,
        encoding: AsciiOutputEncoding::from_color_mode(color_mode),
        text,
        primary_extent,
        emitted_extent,
        width_profile: profile,
        layout_profile,
        requested_max_width: policy.max_width,
        overflowed: matches!(
            outcome,
            AsciiOutputOutcome::WideAllowed | AsciiOutputOutcome::Fallback
        ),
        outcome,
        fallback,
        trimmed,
        lossiness,
    }
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
        layout_profile: _,
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
    preflight_semantic_model(model, execution)?;
    let projection =
        semantic_fallback_projection(model, metadata, &control, execution).map_err(|error| {
            match error {
                SemanticFallbackError::Cancelled(cancelled) => AsciiError::Cancelled(cancelled),
                SemanticFallbackError::Resource(error) => error,
                SemanticFallbackError::Unavailable => AsciiError::FallbackUnavailable {
                    diagram_type: model.kind().to_string(),
                    max_width,
                    actual_width: primary_extent.width,
                },
            }
        })?;
    let mut fallback = SemanticFallbackWriter::new(execution, max_width, profile);
    fallback.push(format!("family: {}", model.kind()))?;
    match projection {
        SemanticFallbackProjection::Serialized(bytes) => {
            flatten_serialized_json("model", &bytes, &mut fallback)?;
        }
        SemanticFallbackProjection::Value(value) => {
            flatten_json_value("model", &value, &mut fallback)?;
        }
    }
    let candidate = fallback.finish();
    finalize_fallback(
        model.kind(),
        primary_extent,
        candidate,
        Lossiness::PresentationOnly,
        context,
    )
}

/// Applies the parser-owned model complexity contract before a compatibility projector can
/// materialize a JSON value. The family projector remains the source of semantic field mapping;
/// this detached admission prevents an oversized typed model from reaching that allocation under
/// a bounded ASCII policy, while the final fallback writer still admits exact terminal metrics.
fn preflight_semantic_model(
    model: &merman_core::diagram::RenderSemanticModel,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    execution.checkpoint(OperationPhase::Semantic)?;
    let complexity = merman_core::resources::ModelComplexity::from_render_model(model);
    let resources = execution.new_resource_context(OperationPhase::Semantic);
    resources.check_nesting_depth(complexity.nesting_depth)?;
    resources.check(AsciiResourceLimitId::MaxOutputBytes, complexity.text_bytes)?;
    if !matches!(
        model,
        merman_core::diagram::RenderSemanticModel::Flowchart(_)
    ) {
        check_semantic_projection_budget(complexity, resources.policy())?;
    }
    resources.charge_layout_work(complexity.items)?;
    execution.checkpoint(OperationPhase::Semantic)
}

const SEMANTIC_PROJECTION_TEXT_MULTIPLIER: usize = 2;
const SEMANTIC_PROJECTION_ITEM_OVERHEAD: usize = 96;
const SEMANTIC_PROJECTION_DEPTH_OVERHEAD: usize = 64;

/// Conservatively bounds the temporary compatibility projection before it can allocate a
/// deep `serde_json::Value` tree. The estimate intentionally over-approximates escaped text,
/// object keys, map/vector entries, and nesting metadata; bounded profiles reject before the
/// family-owned projector is allowed to materialize the value.
fn check_semantic_projection_budget(
    complexity: merman_core::resources::ModelComplexity,
    policy: AsciiResourcePolicy,
) -> Result<()> {
    let text_bytes = complexity
        .text_bytes
        .checked_mul(SEMANTIC_PROJECTION_TEXT_MULTIPLIER)
        .ok_or_else(|| policy.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
    let item_bytes = complexity
        .items
        .checked_mul(SEMANTIC_PROJECTION_ITEM_OVERHEAD)
        .ok_or_else(|| policy.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
    let depth_bytes = complexity
        .nesting_depth
        .checked_mul(SEMANTIC_PROJECTION_DEPTH_OVERHEAD)
        .ok_or_else(|| policy.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
    let estimate = text_bytes
        .checked_add(item_bytes)
        .and_then(|value| value.checked_add(depth_bytes))
        .ok_or_else(|| policy.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
    policy.check(AsciiResourceLimitId::MaxOutputBytes, estimate)
}

enum SemanticFallbackError {
    Cancelled(merman_core::OperationCancelled),
    Resource(AsciiError),
    Unavailable,
}

/// Returns a complete typed projection for fallback without reparsing source text.
///
/// Flowchart compatibility JSON intentionally removes render-only edge fields for Mermaid's
/// legacy semantic contract. The agent-facing fallback needs those authored edge semantics, so
/// this path serializes the typed render model directly and only adds the stable family marker.
/// Other families continue to use their existing family-owned compatibility projection until
/// their field-level fallback coverage is admitted explicitly.
enum SemanticFallbackProjection {
    Serialized(Vec<u8>),
    Value(Value),
}

#[derive(Serialize)]
struct SemanticFallbackEnvelope<'a, T> {
    #[serde(flatten)]
    model: &'a T,
    #[serde(rename = "type")]
    diagram_type: &'a str,
}

fn semantic_fallback_projection(
    model: &merman_core::diagram::RenderSemanticModel,
    metadata: &ParseMetadata,
    control: &merman_core::OperationControl,
    execution: AsciiExecution<'_>,
) -> std::result::Result<SemanticFallbackProjection, SemanticFallbackError> {
    let control = control.for_phase(OperationPhase::Semantic);
    control
        .checkpoint()
        .map_err(SemanticFallbackError::Cancelled)?;
    let projection = match model {
        merman_core::diagram::RenderSemanticModel::Flowchart(flowchart) => {
            let envelope = SemanticFallbackEnvelope {
                model: flowchart,
                diagram_type: &metadata.diagram_type,
            };
            SemanticFallbackProjection::Serialized(serialize_bounded_json(&envelope, execution)?)
        }
        _ => {
            let mut value = model
                .compatibility_json_controlled(metadata, &control)
                .map_err(SemanticFallbackError::Cancelled)?
                .map_err(|_| SemanticFallbackError::Unavailable)?;
            if let Some(object) = value.as_object_mut() {
                object.insert(
                    "type".to_string(),
                    serde_json::Value::String(metadata.diagram_type.clone()),
                );
            }
            SemanticFallbackProjection::Value(value)
        }
    };
    control
        .checkpoint()
        .map_err(SemanticFallbackError::Cancelled)?;
    Ok(projection)
}

/// Serializes a typed semantic projection through the ASCII policy before materializing its
/// compatibility value. Serde writes in bounded chunks, so each chunk observes cancellation and
/// the detached candidate ledger can reject oversized intermediates before they reach the
/// flattening writer. The detached ledger is admitted again by [`SemanticFallbackWriter`] when
/// the terminal representation is constructed.
fn serialize_bounded_json<T: Serialize>(
    value: &T,
    execution: AsciiExecution<'_>,
) -> std::result::Result<Vec<u8>, SemanticFallbackError> {
    let resources = execution.new_resource_context(OperationPhase::Semantic);
    let mut writer = BoundedJsonWriter {
        output: Vec::new(),
        resources,
        execution,
        error: None,
    };
    let serialization = serde_json::to_writer(&mut writer, value);
    if let Some(error) = writer.error.take() {
        return Err(map_semantic_fallback_error(error));
    }
    serialization.map_err(|_| SemanticFallbackError::Unavailable)?;
    Ok(writer.output)
}

struct BoundedJsonWriter<'a> {
    output: Vec<u8>,
    resources: ResourceContext,
    execution: AsciiExecution<'a>,
    error: Option<AsciiError>,
}

impl BoundedJsonWriter<'_> {
    fn fail(&mut self, error: AsciiError) -> io::Result<usize> {
        self.error = Some(error);
        Err(io::Error::other(
            "bounded ASCII semantic projection rejected",
        ))
    }
}

impl IoWrite for BoundedJsonWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.execution
            .checkpoint(OperationPhase::Semantic)
            .map_err(|error| {
                self.error = Some(error);
                io::Error::other("ASCII semantic projection was cancelled")
            })?;
        if let Err(error) = self.resources.charge_layout_work(bytes.len()) {
            return self.fail(error);
        }
        let next_len = self.output.len().checked_add(bytes.len()).ok_or_else(|| {
            self.error = Some(
                self.resources
                    .overflow(AsciiResourceLimitId::MaxOutputBytes),
            );
            io::Error::other("ASCII semantic projection size overflow")
        })?;
        if let Err(error) = self
            .resources
            .check(AsciiResourceLimitId::MaxOutputBytes, next_len)
        {
            return self.fail(error);
        }
        self.output.try_reserve(bytes.len()).map_err(|_| {
            self.error = Some(AsciiError::allocation_failed("ascii_semantic_projection"));
            io::Error::other("ASCII semantic projection allocation failed")
        })?;
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn map_semantic_fallback_error(error: AsciiError) -> SemanticFallbackError {
    match error {
        AsciiError::Cancelled(cancelled) => SemanticFallbackError::Cancelled(cancelled),
        error @ (AsciiError::ResourceLimitExceeded(_)
        | AsciiError::OperationResourceTerminal(_)) => SemanticFallbackError::Resource(error),
        _ => SemanticFallbackError::Unavailable,
    }
}

struct SemanticFallbackWriter<'a> {
    output: String,
    semantic_resources: ResourceContext,
    emit_resources: ResourceContext,
    candidate_resources: ResourceContext,
    execution: AsciiExecution<'a>,
    max_width: usize,
    profile: TerminalWidthProfile,
    output_bytes: usize,
    document_cells: usize,
    max_line_width: usize,
    max_grapheme_bytes: usize,
    work: usize,
    line_count: usize,
}

impl<'a> SemanticFallbackWriter<'a> {
    fn new(execution: AsciiExecution<'a>, max_width: usize, profile: TerminalWidthProfile) -> Self {
        Self {
            output: String::new(),
            semantic_resources: execution.new_resource_context(OperationPhase::Semantic),
            emit_resources: execution.new_resource_context(OperationPhase::Emit),
            candidate_resources: execution.detached_resource_context(OperationPhase::Emit),
            execution,
            max_width,
            profile,
            output_bytes: 0,
            document_cells: 0,
            max_line_width: 0,
            max_grapheme_bytes: 0,
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
            let grapheme_width = display_width_with_profile(grapheme, self.profile);
            if has_content
                && self
                    .candidate_resources
                    .checked_grid_add(width, grapheme_width)?
                    > self.max_width
            {
                self.push_fragment("\n")?;
                width = 0;
            }
            let next_width = self
                .candidate_resources
                .checked_grid_add(width, grapheme_width)?;
            let next_document_cells = self
                .candidate_resources
                .checked_grid_add(self.document_cells, grapheme_width)?;
            self.candidate_resources
                .check(AsciiResourceLimitId::MaxDocumentCells, next_document_cells)?;
            self.push_fragment(grapheme)?;
            width = next_width;
            self.max_line_width = self.max_line_width.max(width);
            self.max_grapheme_bytes = self.max_grapheme_bytes.max(grapheme.len());
            self.document_cells = next_document_cells;
            has_content = true;
        }
        self.line_count = self
            .candidate_resources
            .checked_grid_add(self.line_count, 1)?;
        Ok(())
    }

    fn admit_node(&self, depth: usize) -> Result<()> {
        self.semantic_resources.charge_layout_work(1)?;
        self.semantic_resources.check_nesting_depth(depth)
    }

    fn push_fragment(&mut self, fragment: &str) -> Result<()> {
        self.candidate_resources
            .check(AsciiResourceLimitId::MaxGraphemeBytes, fragment.len())?;
        let output_bytes = self
            .output_bytes
            .checked_add(fragment.len())
            .ok_or_else(|| {
                self.candidate_resources
                    .overflow(AsciiResourceLimitId::MaxOutputBytes)
            })?;
        self.candidate_resources
            .check(AsciiResourceLimitId::MaxOutputBytes, output_bytes)?;
        self.output
            .try_reserve(fragment.len())
            .map_err(|_| AsciiError::allocation_failed("ascii_semantic_fallback"))?;
        self.output.push_str(fragment);
        self.output_bytes = output_bytes;
        Ok(())
    }

    fn finish(self) -> MeasuredOutput {
        let height = if self.output.is_empty() {
            0
        } else {
            self.output.bytes().filter(|byte| *byte == b'\n').count() + 1
        };
        MeasuredOutput::from_plain_metrics(
            self.output,
            OutputMetrics {
                extent: AsciiExtent::new(self.max_line_width, height),
                document_cells: self.document_cells,
                grapheme_bytes: self.max_grapheme_bytes,
                encoded_bytes: self.output_bytes,
            },
        )
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

/// Flattens a bounded serialized projection while borrowing every nested value from the one
/// policy-limited byte buffer. Objects use a sorted map of raw slices, so deterministic field
/// ordering is retained without constructing a recursive `serde_json::Value` tree.
fn flatten_serialized_json(
    path: &str,
    bytes: &[u8],
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<()> {
    let raw =
        serde_json::from_slice::<&RawValue>(bytes).map_err(|_| AsciiError::UnsupportedFeature {
            diagram_type: "ascii",
            feature: "invalid serialized semantic projection",
        })?;

    fn visit(
        path: &str,
        raw: &RawValue,
        lines: &mut SemanticFallbackWriter<'_>,
        depth: usize,
    ) -> Result<()> {
        lines.admit_node(depth)?;
        let json = raw.get().trim();
        match json.as_bytes().first().copied() {
            Some(b'{') => {
                let object =
                    serde_json::from_str::<BTreeMap<String, &RawValue>>(json).map_err(|_| {
                        AsciiError::UnsupportedFeature {
                            diagram_type: "ascii",
                            feature: "invalid serialized semantic object",
                        }
                    })?;
                for (key, child) in object {
                    let child_path = append_path_key(path, &key);
                    visit(&child_path, child, lines, depth + 1)?;
                }
            }
            Some(b'[') => {
                let values = serde_json::from_str::<Vec<&RawValue>>(json).map_err(|_| {
                    AsciiError::UnsupportedFeature {
                        diagram_type: "ascii",
                        feature: "invalid serialized semantic array",
                    }
                })?;
                for (index, child) in values.iter().enumerate() {
                    visit(&format!("{path}[{index}]"), child, lines, depth + 1)?;
                }
                if values.is_empty() {
                    lines.push(format!("{path}: []"))?;
                }
            }
            Some(b'"') => {
                let value = serde_json::from_str::<String>(json).map_err(|_| {
                    AsciiError::UnsupportedFeature {
                        diagram_type: "ascii",
                        feature: "invalid serialized semantic string",
                    }
                })?;
                lines.push(format!("{path}: {value:?}"))?;
            }
            Some(b't' | b'f') => {
                let value = serde_json::from_str::<bool>(json).map_err(|_| {
                    AsciiError::UnsupportedFeature {
                        diagram_type: "ascii",
                        feature: "invalid serialized semantic boolean",
                    }
                })?;
                lines.push(format!("{path}: {value}"))?;
            }
            Some(b'n') => {
                if json != "null" {
                    return Err(AsciiError::UnsupportedFeature {
                        diagram_type: "ascii",
                        feature: "invalid serialized semantic null",
                    });
                }
                lines.push(format!("{path}: null"))?;
            }
            Some(_) => {
                let value = serde_json::from_str::<serde_json::Number>(json).map_err(|_| {
                    AsciiError::UnsupportedFeature {
                        diagram_type: "ascii",
                        feature: "invalid serialized semantic number",
                    }
                })?;
                lines.push(format!("{path}: {value}"))?;
            }
            None => {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "ascii",
                    feature: "empty serialized semantic projection",
                });
            }
        }
        Ok(())
    }

    visit(path, raw, lines, 0)
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
    let candidate_resources = execution.detached_resource_context(OperationPhase::Emit);
    let mut output_bytes = 0usize;
    let mut document_cells = 0usize;
    let mut append_fragment = |fragment: &str, display_width: usize| -> Result<()> {
        candidate_resources.check(AsciiResourceLimitId::MaxGraphemeBytes, fragment.len())?;
        let next_output_bytes =
            candidate_resources.checked_grid_add(output_bytes, fragment.len())?;
        candidate_resources.check(AsciiResourceLimitId::MaxOutputBytes, next_output_bytes)?;
        let next_document_cells =
            candidate_resources.checked_grid_add(document_cells, display_width)?;
        candidate_resources.check(AsciiResourceLimitId::MaxDocumentCells, next_document_cells)?;
        output
            .try_reserve(fragment.len())
            .map_err(|_| AsciiError::allocation_failed("ascii_structured_fallback"))?;
        output.push_str(fragment);
        output_bytes = next_output_bytes;
        document_cells = next_document_cells;
        Ok(())
    };
    let mut work = 0usize;
    // `split('\n')` deliberately retains trailing empty rows so authored hard breaks and a final
    // newline survive the bounded projection. `str::lines()` would silently erase both.
    for (line_index, line) in lines.into_iter().enumerate() {
        execution.checkpoint(OperationPhase::Emit)?;
        if line_index > 0 {
            append_fragment("\n", 0)?;
        }
        let mut width = 0usize;
        let mut has_content = false;
        for grapheme in line.graphemes(true) {
            resources.charge_layout_work(1)?;
            execution.checkpoint_loop(OperationPhase::Emit, work)?;
            work = work.saturating_add(1);
            let grapheme_width = display_width_with_profile(grapheme, profile);
            if has_content && resources.checked_grid_add(width, grapheme_width)? > max_width {
                append_fragment("\n", 0)?;
                width = 0;
            }
            append_fragment(grapheme, grapheme_width)?;
            width = resources.checked_grid_add(width, grapheme_width)?;
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

pub(crate) fn capability_for(
    model: &merman_core::diagram::RenderSemanticModel,
) -> Option<crate::AsciiCapability> {
    crate::ascii_capabilities()
        .iter()
        .find(|capability| capability.diagram_type == model.kind())
        .copied()
}

pub(crate) fn projection_for(capability: Option<crate::AsciiCapability>) -> AsciiProjection {
    match capability.map(|capability| capability.primary_projection) {
        Some(crate::AsciiPrimaryProjection::StructuredText) => AsciiProjection::StructuredText,
        Some(crate::AsciiPrimaryProjection::Diagrammatic)
        | Some(crate::AsciiPrimaryProjection::None)
        | None => AsciiProjection::Diagrammatic,
    }
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
    fn extent_preserves_authored_empty_rows_without_counting_the_line_terminator() {
        assert_eq!(
            AsciiExtent::measure("alpha\n\n", TerminalWidthProfile::Unicode),
            AsciiExtent::new(5, 2)
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

    #[test]
    fn measured_candidate_keeps_extent_and_terminal_metrics_together() {
        let resources = crate::AsciiResourcePolicy::unbounded();
        let execution = crate::operation::AsciiExecution::for_test(&resources);
        let candidate = MeasuredOutput::measure(
            "a\n超".to_string(),
            AsciiColorMode::Plain,
            TerminalWidthProfile::Unicode,
            execution,
        )
        .expect("candidate should be measurable");

        assert_eq!(
            candidate.metrics(),
            OutputMetrics {
                extent: AsciiExtent::new(2, 2),
                document_cells: 3,
                grapheme_bytes: "超".len(),
                encoded_bytes: "a\n超".len(),
            }
        );
    }

    #[test]
    fn measured_candidate_counts_terminal_terminator_work_without_counting_an_extra_row() {
        let exact_resources = crate::AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 3)
            .expect("positive layout-work limit");
        let candidate = MeasuredOutput::measure(
            "a\n".to_string(),
            AsciiColorMode::Plain,
            TerminalWidthProfile::Unicode,
            crate::operation::AsciiExecution::for_test(&exact_resources),
        )
        .expect("the exact terminator-work boundary should succeed");
        assert_eq!(candidate.metrics().extent, AsciiExtent::new(1, 1));

        let below_resources = crate::AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 2)
            .expect("positive layout-work limit");
        let error = MeasuredOutput::measure(
            "a\n".to_string(),
            AsciiColorMode::Plain,
            TerminalWidthProfile::Unicode,
            crate::operation::AsciiExecution::for_test(&below_resources),
        )
        .expect_err("the N-1 terminator-work boundary should fail");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == 3
                    && details.max == 2
        ));
    }

    #[test]
    fn projection_resolution_is_capability_owned() {
        let timeline = merman_core::diagram::RenderSemanticModel::Timeline(Default::default());
        assert_eq!(
            projection_for(capability_for(&timeline)),
            AsciiProjection::StructuredText
        );
    }

    #[test]
    fn bounded_semantic_projection_rejects_json_before_flattening() {
        let control = merman_core::OperationControl::new();
        let resources = crate::AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, 4)
            .expect("positive output limit");
        let execution = crate::operation::AsciiExecution::new(&control, &resources);
        let value = serde_json::json!({"payload": "abcdef"});

        let result = serialize_bounded_json(&value, execution);
        assert!(matches!(
            result,
            Err(SemanticFallbackError::Resource(
                AsciiError::ResourceLimitExceeded(details)
            )) if details.limit == AsciiResourceLimitId::MaxOutputBytes
        ));
    }

    #[test]
    fn bounded_semantic_projection_reports_semantic_cancellation() {
        let control = merman_core::OperationControl::new();
        control.cancel_after_checkpoints(0);
        let resources = crate::AsciiResourcePolicy::default();
        let execution = crate::operation::AsciiExecution::new(&control, &resources);
        let value = serde_json::json!({"payload": "abcdef"});

        let result = serialize_bounded_json(&value, execution);
        assert!(matches!(
            result,
            Err(SemanticFallbackError::Cancelled(cancelled))
                if cancelled.phase == OperationPhase::Semantic
        ));
    }

    #[test]
    fn serialized_semantic_projection_flattens_sorted_fields_without_value_materialization() {
        let resources = crate::AsciiResourcePolicy::unbounded();
        let execution = crate::operation::AsciiExecution::for_test(&resources);
        let mut fallback =
            SemanticFallbackWriter::new(execution, 80, TerminalWidthProfile::Unicode);

        flatten_serialized_json(
            "model",
            br#"{"z":{"value":"last"},"a":[true,2],"empty":{},"items":[]}"#,
            &mut fallback,
        )
        .expect("serialized semantic projection should flatten");

        assert_eq!(
            fallback.finish().into_text(),
            "model.a[0]: true\nmodel.a[1]: 2\nmodel.items: []\nmodel.z.value: \"last\""
        );
    }

    #[test]
    fn serialized_semantic_projection_observes_semantic_cancellation_before_first_row() {
        let control = merman_core::OperationControl::new();
        control.cancel_after_checkpoints(0);
        let resources = crate::AsciiResourcePolicy::default();
        let execution = crate::operation::AsciiExecution::new(&control, &resources);
        let mut fallback =
            SemanticFallbackWriter::new(execution, 80, TerminalWidthProfile::Unicode);

        let result = flatten_serialized_json("model", br#"{"value":"first"}"#, &mut fallback);
        assert!(matches!(
            result,
            Err(AsciiError::Cancelled(cancelled))
                if cancelled.phase == OperationPhase::Semantic
        ));
    }

    #[test]
    fn compatibility_projection_budget_rejects_container_amplification_before_allocation() {
        let resources = crate::AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, 256)
            .expect("positive output limit");

        let error = check_semantic_projection_budget(
            merman_core::resources::ModelComplexity::new(8, 128, 2),
            resources,
        )
        .expect_err("the conservative projection budget should reject amplification");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxOutputBytes
        ));
    }
}
