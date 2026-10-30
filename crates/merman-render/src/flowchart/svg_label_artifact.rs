//! Operation-local preparation for Flowchart SVG rows and sanitized HTML labels.

use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex;

use rustc_hash::FxHashMap;

use crate::diagram_theme::{ResolvedDiagramTheme, ThemeTarget};
use crate::environment::{BuiltinTextMeasurementOperationCarrier, TextMeasurementOperation};
use crate::math::{
    ConfiguredMathBackend, MathPreparationOutcome, MathPreparationUnavailable,
    PrepareMathLabelRequest, PreparedMathEvidenceLease, PreparedMathLabel,
    PreparedMathOccurrenceId,
};
use crate::resources::{
    OperationWorkError, OperationWorkMeter, PreparedTextRetainedReservation, ResourceLimitExceeded,
};
use crate::text::{TextMeasurer, TextMetrics, TextStyle, WrapMode};

use super::label::{
    FlowchartLabelMetricsRequest, FlowchartSvgLabelSource, FlowchartSvgWidthMode,
    flowchart_label_metrics_for_layout,
};

const FLOWCHART_RENDER_ID_RECORD_BYTES: usize = 32;
const FLOWCHART_PREPARED_OWNER_SLOT_BYTES: usize = 192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum FlowchartSvgLabelOwner {
    Node(usize),
    EmptySubgraphNode(usize),
    Edge(usize),
    SubgraphTitle(usize),
    SwimlaneNode(usize),
    SwimlaneEdgeLabel(usize),
    SwimlaneGroupTitle(usize),
}

impl FlowchartSvgLabelOwner {
    fn typography_target(self) -> ThemeTarget {
        match self {
            Self::Node(_) | Self::EmptySubgraphNode(_) | Self::SwimlaneNode(_) => {
                ThemeTarget::NodeLabel
            }
            Self::Edge(_) | Self::SwimlaneEdgeLabel(_) => ThemeTarget::EdgeLabel,
            Self::SubgraphTitle(_) | Self::SwimlaneGroupTitle(_) => ThemeTarget::ClusterLabel,
        }
    }

    fn semantic_index(self) -> usize {
        match self {
            Self::Node(index)
            | Self::EmptySubgraphNode(index)
            | Self::Edge(index)
            | Self::SubgraphTitle(index)
            | Self::SwimlaneNode(index)
            | Self::SwimlaneEdgeLabel(index)
            | Self::SwimlaneGroupTitle(index) => index,
        }
    }

    fn prepared_math_occurrence_id(self) -> PreparedMathOccurrenceId {
        match self {
            Self::Node(index) => PreparedMathOccurrenceId::indexed(
                crate::DiagramFamilyId::FLOWCHART,
                "node-label",
                index,
            ),
            Self::EmptySubgraphNode(index) => PreparedMathOccurrenceId::indexed(
                crate::DiagramFamilyId::FLOWCHART,
                "empty-subgraph-node-label",
                index,
            ),
            Self::Edge(index) => PreparedMathOccurrenceId::indexed(
                crate::DiagramFamilyId::FLOWCHART,
                "edge-label",
                index,
            ),
            Self::SubgraphTitle(index) => PreparedMathOccurrenceId::indexed(
                crate::DiagramFamilyId::FLOWCHART,
                "subgraph-title",
                index,
            ),
            Self::SwimlaneNode(index) => PreparedMathOccurrenceId::indexed(
                crate::DiagramFamilyId::SWIMLANE,
                "node-label",
                index,
            ),
            Self::SwimlaneEdgeLabel(index) => PreparedMathOccurrenceId::indexed(
                crate::DiagramFamilyId::SWIMLANE,
                "edge-label",
                index,
            ),
            Self::SwimlaneGroupTitle(index) => PreparedMathOccurrenceId::indexed(
                crate::DiagramFamilyId::SWIMLANE,
                "group-title",
                index,
            ),
        }
    }
}

#[derive(Debug)]
struct FlowchartSvgLabelRoleSlots<T> {
    entries: Vec<(usize, T)>,
}

impl<T> Default for FlowchartSvgLabelRoleSlots<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<T> FlowchartSvgLabelRoleSlots<T> {
    fn get(&self, index: usize) -> Option<&T> {
        self.entries
            .binary_search_by_key(&index, |(entry_index, _)| *entry_index)
            .ok()
            .map(|position| &self.entries[position].1)
    }

    fn insert(&mut self, index: usize, value: T) {
        if self
            .entries
            .last()
            .is_none_or(|(last_index, _)| *last_index < index)
        {
            self.entries.push((index, value));
            return;
        }

        match self
            .entries
            .binary_search_by_key(&index, |(entry_index, _)| *entry_index)
        {
            Ok(position) => self.entries[position].1 = value,
            Err(position) => self.entries.insert(position, (index, value)),
        }
    }

    fn remove(&mut self, index: usize) -> Option<T> {
        self.entries
            .binary_search_by_key(&index, |(entry_index, _)| *entry_index)
            .ok()
            .map(|position| self.entries.remove(position).1)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }

    fn into_iter(self) -> impl Iterator<Item = (usize, T)> {
        self.entries.into_iter()
    }
}

/// Compact semantic-owner storage. Entries are sorted by semantic index within each role,
/// so sparse source indices do not allocate placeholders for labels that do not exist.
#[derive(Debug)]
struct FlowchartSvgLabelSlots<T> {
    nodes: FlowchartSvgLabelRoleSlots<T>,
    empty_subgraph_nodes: FlowchartSvgLabelRoleSlots<T>,
    edges: FlowchartSvgLabelRoleSlots<T>,
    subgraph_titles: FlowchartSvgLabelRoleSlots<T>,
    swimlane_nodes: FlowchartSvgLabelRoleSlots<T>,
    swimlane_edge_labels: FlowchartSvgLabelRoleSlots<T>,
    swimlane_group_titles: FlowchartSvgLabelRoleSlots<T>,
}

impl<T> Default for FlowchartSvgLabelSlots<T> {
    fn default() -> Self {
        Self {
            nodes: FlowchartSvgLabelRoleSlots::default(),
            empty_subgraph_nodes: FlowchartSvgLabelRoleSlots::default(),
            edges: FlowchartSvgLabelRoleSlots::default(),
            subgraph_titles: FlowchartSvgLabelRoleSlots::default(),
            swimlane_nodes: FlowchartSvgLabelRoleSlots::default(),
            swimlane_edge_labels: FlowchartSvgLabelRoleSlots::default(),
            swimlane_group_titles: FlowchartSvgLabelRoleSlots::default(),
        }
    }
}

impl<T> FlowchartSvgLabelSlots<T> {
    fn get(&self, owner: FlowchartSvgLabelOwner) -> Option<&T> {
        match owner {
            FlowchartSvgLabelOwner::Node(index) => self.nodes.get(index),
            FlowchartSvgLabelOwner::EmptySubgraphNode(index) => {
                self.empty_subgraph_nodes.get(index)
            }
            FlowchartSvgLabelOwner::Edge(index) => self.edges.get(index),
            FlowchartSvgLabelOwner::SubgraphTitle(index) => self.subgraph_titles.get(index),
            FlowchartSvgLabelOwner::SwimlaneNode(index) => self.swimlane_nodes.get(index),
            FlowchartSvgLabelOwner::SwimlaneEdgeLabel(index) => {
                self.swimlane_edge_labels.get(index)
            }
            FlowchartSvgLabelOwner::SwimlaneGroupTitle(index) => {
                self.swimlane_group_titles.get(index)
            }
        }
    }

    fn insert(&mut self, owner: FlowchartSvgLabelOwner, value: T) {
        match owner {
            FlowchartSvgLabelOwner::Node(index) => self.nodes.insert(index, value),
            FlowchartSvgLabelOwner::EmptySubgraphNode(index) => {
                self.empty_subgraph_nodes.insert(index, value)
            }
            FlowchartSvgLabelOwner::Edge(index) => self.edges.insert(index, value),
            FlowchartSvgLabelOwner::SubgraphTitle(index) => {
                self.subgraph_titles.insert(index, value)
            }
            FlowchartSvgLabelOwner::SwimlaneNode(index) => self.swimlane_nodes.insert(index, value),
            FlowchartSvgLabelOwner::SwimlaneEdgeLabel(index) => {
                self.swimlane_edge_labels.insert(index, value)
            }
            FlowchartSvgLabelOwner::SwimlaneGroupTitle(index) => {
                self.swimlane_group_titles.insert(index, value)
            }
        }
    }

    fn remove(&mut self, owner: FlowchartSvgLabelOwner) -> Option<T> {
        match owner {
            FlowchartSvgLabelOwner::Node(index) => self.nodes.remove(index),
            FlowchartSvgLabelOwner::EmptySubgraphNode(index) => {
                self.empty_subgraph_nodes.remove(index)
            }
            FlowchartSvgLabelOwner::Edge(index) => self.edges.remove(index),
            FlowchartSvgLabelOwner::SubgraphTitle(index) => self.subgraph_titles.remove(index),
            FlowchartSvgLabelOwner::SwimlaneNode(index) => self.swimlane_nodes.remove(index),
            FlowchartSvgLabelOwner::SwimlaneEdgeLabel(index) => {
                self.swimlane_edge_labels.remove(index)
            }
            FlowchartSvgLabelOwner::SwimlaneGroupTitle(index) => {
                self.swimlane_group_titles.remove(index)
            }
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        [
            &self.nodes,
            &self.empty_subgraph_nodes,
            &self.edges,
            &self.subgraph_titles,
            &self.swimlane_nodes,
            &self.swimlane_edge_labels,
            &self.swimlane_group_titles,
        ]
        .into_iter()
        .map(|slots| slots.len())
        .sum()
    }

    fn for_each(self, mut visit: impl FnMut(FlowchartSvgLabelOwner, T)) {
        for (index, value) in self.nodes.into_iter() {
            visit(FlowchartSvgLabelOwner::Node(index), value);
        }
        for (index, value) in self.empty_subgraph_nodes.into_iter() {
            visit(FlowchartSvgLabelOwner::EmptySubgraphNode(index), value);
        }
        for (index, value) in self.edges.into_iter() {
            visit(FlowchartSvgLabelOwner::Edge(index), value);
        }
        for (index, value) in self.subgraph_titles.into_iter() {
            visit(FlowchartSvgLabelOwner::SubgraphTitle(index), value);
        }
        for (index, value) in self.swimlane_nodes.into_iter() {
            visit(FlowchartSvgLabelOwner::SwimlaneNode(index), value);
        }
        for (index, value) in self.swimlane_edge_labels.into_iter() {
            visit(FlowchartSvgLabelOwner::SwimlaneEdgeLabel(index), value);
        }
        for (index, value) in self.swimlane_group_titles.into_iter() {
            visit(FlowchartSvgLabelOwner::SwimlaneGroupTitle(index), value);
        }
    }

    fn iter(&self) -> impl Iterator<Item = &T> {
        [
            &self.nodes,
            &self.empty_subgraph_nodes,
            &self.edges,
            &self.subgraph_titles,
            &self.swimlane_nodes,
            &self.swimlane_edge_labels,
            &self.swimlane_group_titles,
        ]
        .into_iter()
        .flat_map(|slots| slots.entries.iter().map(|(_, value)| value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FlowchartSvgTextStyleKey {
    font_family: Option<String>,
    font_size_bits: u64,
    font_weight: Option<String>,
    font_style: Option<String>,
}

impl FlowchartSvgTextStyleKey {
    fn new(style: &TextStyle) -> Self {
        Self {
            font_family: style.font_family.clone(),
            font_size_bits: canonical_f64_bits(style.font_size),
            font_weight: style.font_weight.clone(),
            font_style: style.font_style.clone(),
        }
    }

    fn matches(&self, style: &TextStyle) -> bool {
        self.font_family.as_deref() == style.font_family.as_deref()
            && self.font_size_bits == canonical_f64_bits(style.font_size)
            && self.font_weight.as_deref() == style.font_weight.as_deref()
            && self.font_style.as_deref() == style.font_style.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FlowchartSvgLabelBinding {
    wrap_style: FlowchartSvgTextStyleKey,
    metrics_style: FlowchartSvgTextStyleKey,
    max_width_bits: Option<u64>,
    break_long_words: bool,
    width_mode: FlowchartSvgWidthMode,
    computed_length_carrier: Option<BuiltinTextMeasurementOperationCarrier>,
    wrapped_carrier: Option<BuiltinTextMeasurementOperationCarrier>,
}

#[derive(Debug, Clone, Copy)]
struct FlowchartSvgLabelWrapBindingRequest<'a> {
    wrap_style: &'a TextStyle,
    max_width_bits: Option<u64>,
    break_long_words: bool,
    computed_length_carrier: Option<BuiltinTextMeasurementOperationCarrier>,
}

impl<'a> FlowchartSvgLabelWrapBindingRequest<'a> {
    fn for_measurer(
        measurer: &dyn TextMeasurer,
        wrap_style: &'a TextStyle,
        max_width_px: Option<f64>,
        break_long_words: bool,
    ) -> Option<Self> {
        Some(Self {
            wrap_style,
            max_width_bits: normalized_width_bits(max_width_px),
            break_long_words,
            computed_length_carrier: Some(
                measurer.builtin_operation_carrier(TextMeasurementOperation::ComputedLength)?,
            ),
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct FlowchartSvgLabelBindingRequest<'a> {
    wrap_style: &'a TextStyle,
    metrics_style: &'a TextStyle,
    max_width_bits: Option<u64>,
    break_long_words: bool,
    width_mode: FlowchartSvgWidthMode,
    computed_length_carrier: Option<BuiltinTextMeasurementOperationCarrier>,
    wrapped_carrier: Option<BuiltinTextMeasurementOperationCarrier>,
}

impl<'a> FlowchartSvgLabelBindingRequest<'a> {
    fn for_measurer(
        measurer: &dyn TextMeasurer,
        wrap_style: &'a TextStyle,
        metrics_style: &'a TextStyle,
        max_width_px: Option<f64>,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> Option<Self> {
        Some(Self {
            wrap_style,
            metrics_style,
            max_width_bits: normalized_width_bits(max_width_px),
            break_long_words,
            width_mode,
            computed_length_carrier: Some(
                measurer.builtin_operation_carrier(TextMeasurementOperation::ComputedLength)?,
            ),
            wrapped_carrier: Some(
                measurer.builtin_operation_carrier(TextMeasurementOperation::Wrapped)?,
            ),
        })
    }

    fn into_owned(self) -> FlowchartSvgLabelBinding {
        FlowchartSvgLabelBinding {
            wrap_style: FlowchartSvgTextStyleKey::new(self.wrap_style),
            metrics_style: FlowchartSvgTextStyleKey::new(self.metrics_style),
            max_width_bits: self.max_width_bits,
            break_long_words: self.break_long_words,
            width_mode: self.width_mode,
            computed_length_carrier: self.computed_length_carrier,
            wrapped_carrier: self.wrapped_carrier,
        }
    }
}

impl FlowchartSvgLabelBinding {
    fn matches_wrapping(&self, request: &FlowchartSvgLabelWrapBindingRequest<'_>) -> bool {
        self.wrap_style.matches(request.wrap_style)
            && self.max_width_bits == request.max_width_bits
            && self.break_long_words == request.break_long_words
            && self.computed_length_carrier == request.computed_length_carrier
    }

    fn matches(&self, request: &FlowchartSvgLabelBindingRequest<'_>) -> bool {
        self.wrap_style.matches(request.wrap_style)
            && self.metrics_style.matches(request.metrics_style)
            && self.max_width_bits == request.max_width_bits
            && self.break_long_words == request.break_long_words
            && self.width_mode == request.width_mode
            && self.computed_length_carrier == request.computed_length_carrier
            && self.wrapped_carrier == request.wrapped_carrier
    }
}

fn canonical_f64_bits(value: f64) -> u64 {
    if value == 0.0 {
        0.0_f64.to_bits()
    } else if value.is_nan() {
        f64::NAN.to_bits()
    } else {
        value.to_bits()
    }
}

fn normalized_width_bits(width: Option<f64>) -> Option<u64> {
    width
        .filter(|width| width.is_finite() && *width > 0.0)
        .map(canonical_f64_bits)
}

/// A fully measured label bound to one exact style, width, wrapping mode, and built-in route.
#[derive(Debug)]
pub(crate) struct PreparedFlowchartSvgLabel {
    binding: FlowchartSvgLabelBinding,
    wrapped_lines: Vec<Vec<String>>,
    metrics: TextMetrics,
}

#[derive(Debug)]
struct FlowchartSvgLabelSourceEntry {
    raw_source: Box<str>,
    source: FlowchartSvgLabelSource,
}

impl FlowchartSvgLabelSourceEntry {
    fn new(raw_source: &str) -> Self {
        Self {
            raw_source: raw_source.into(),
            source: FlowchartSvgLabelSource::new(raw_source),
        }
    }

    fn matches(&self, raw_source: &str) -> bool {
        self.raw_source.as_ref() == raw_source
    }
}

impl PreparedFlowchartSvgLabel {
    fn new(
        binding: FlowchartSvgLabelBinding,
        wrapped_lines: Vec<Vec<String>>,
        metrics: TextMetrics,
    ) -> Self {
        Self {
            binding,
            wrapped_lines,
            metrics,
        }
    }

    pub(crate) fn wrapped_lines(&self) -> &[Vec<String>] {
        &self.wrapped_lines
    }

    pub(crate) fn metrics(&self) -> TextMetrics {
        self.metrics
    }

    fn centered_shadow_bounds(&self, metrics_style: &TextStyle) -> Option<[f64; 4]> {
        if !self.binding.metrics_style.matches(metrics_style) {
            return None;
        }
        let em = metrics_style.font_size;
        let TextMetrics { width, height, .. } = self.metrics;
        if !em.is_finite()
            || em <= 0.0
            || !width.is_finite()
            || width < 0.0
            || !height.is_finite()
            || height < 0.0
        {
            return None;
        }
        // Host metrics bound the allocation, not arbitrary glyph ink. Keep the actual SVG
        // row baselines and a line-scaled reserve for fallback font advances. Native export
        // still checks the final glyph coverage; this allocation does not certify font ink.
        let horizontal_reserve = width.max(em);
        let last_baseline = em * (1.0 + self.wrapped_lines.len().saturating_sub(1) as f64 * 1.1);
        Some([
            -width / 2.0 - horizontal_reserve,
            -em,
            width / 2.0 + horizontal_reserve,
            height.max(last_baseline) + em,
        ])
    }

    fn matches(&self, binding: &FlowchartSvgLabelBindingRequest<'_>) -> bool {
        self.binding.matches(binding)
    }

    fn matches_wrapping(&self, binding: &FlowchartSvgLabelWrapBindingRequest<'_>) -> bool {
        self.binding.matches_wrapping(binding)
    }
}

/// Mutable preparation state used only while one Flowchart family artifact is laid out.
///
/// Pure source projections are retained for every eligible label. Browser math is retained only
/// after its operation-scoped terminal style and backend outcome are bound to the owner; native
/// text preparation remains independently governed by its existing catalog ledger.
#[derive(Debug, Default)]
pub(crate) struct FlowchartSvgLabelSidecarBuilder {
    pending: RefCell<PendingFlowchartSvgLabels>,
    base_typography: Option<super::FlowchartBaseTypographyPlan>,
    label_weights: super::FlowchartLabelWeights,
    math_backend: Option<ConfiguredMathBackend>,
    work_meter: Option<Arc<OperationWorkMeter>>,
    edge_label_padding: super::FlowchartEdgeLabelPadding,
    prepared_resource_error: RefCell<Option<ResourceLimitExceeded>>,
    prepared_work_error: RefCell<Option<OperationWorkError>>,
    math_terminal_style: FlowchartMathTerminalStylePlan,
    #[cfg(test)]
    prepared_hits: Cell<usize>,
    #[cfg(test)]
    source_plans_by_owner: RefCell<FxHashMap<FlowchartSvgLabelOwner, usize>>,
}

#[derive(Debug, Default)]
struct PendingFlowchartSvgLabels {
    node_source_typography: FlowchartSvgLabelSlots<Arc<FlowchartNodeSourceTypography>>,
    sources: FlowchartSvgLabelSlots<FlowchartSvgLabelSourceEntry>,
    prepared: FlowchartSvgLabelSlots<PreparedFlowchartSvgLabel>,
    math: FlowchartSvgLabelSlots<PreparedFlowchartMathLabel>,
    html: FlowchartSvgLabelSlots<PreparedFlowchartHtmlLabel>,
    render_ids: FlowchartSvgLabelSlots<Box<str>>,
    render_id_reservations: FlowchartSvgLabelSlots<PreparedTextRetainedReservation>,
}

#[derive(Debug)]
struct PreparedFlowchartHtmlLabel {
    source: Box<str>,
    label_type: Box<str>,
    xhtml: String,
    _retained_reservation: Option<PreparedTextRetainedReservation>,
}

impl PreparedFlowchartHtmlLabel {
    fn matches(&self, source: &str, label_type: &str) -> bool {
        self.source.as_ref() == source && self.label_type.as_ref() == label_type
    }
}

#[derive(Debug)]
pub(crate) struct FlowchartNodeSourceTypography {
    style: TextStyle,
    _retained_reservation: Option<PreparedTextRetainedReservation>,
}

impl FlowchartNodeSourceTypography {
    pub(crate) fn from_style(
        style: TextStyle,
        work: &Arc<OperationWorkMeter>,
    ) -> Result<Self, ResourceLimitExceeded> {
        let retained_bytes = Self::retained_bytes(&style);
        let reservation = work.reserve_prepared_text_retained_bytes(retained_bytes)?;
        Ok(Self {
            style,
            _retained_reservation: Some(reservation),
        })
    }

    fn retained_bytes(style: &TextStyle) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(style.font_family.as_ref().map_or(0, String::len))
            .saturating_add(style.font_weight.as_ref().map_or(0, String::len))
            .saturating_add(style.font_style.as_ref().map_or(0, String::len))
            .saturating_add(FLOWCHART_PREPARED_OWNER_SLOT_BYTES)
    }
}

impl std::ops::Deref for FlowchartNodeSourceTypography {
    type Target = TextStyle;

    fn deref(&self) -> &Self::Target {
        &self.style
    }
}

#[derive(Debug)]
struct PreparedFlowchartMathLabel {
    occurrence_id: PreparedMathOccurrenceId,
    source: Arc<str>,
    style: TextStyle,
    foreground: super::FlowchartTerminalForeground,
    max_width_bits: Option<u64>,
    wrap_mode: WrapMode,
    outcome: MathPreparationOutcome,
    expected_terminal_emissions: usize,
    retained_reservation: RefCell<Option<PreparedTextRetainedReservation>>,
}

impl PreparedFlowchartMathLabel {
    fn matches(
        &self,
        source: &str,
        style: &TextStyle,
        foreground: &str,
        max_width_px: Option<f64>,
        wrap_mode: WrapMode,
    ) -> bool {
        self.source.as_ref() == source
            && self.style.font_family == style.font_family
            && self.style.font_size.to_bits() == style.font_size.to_bits()
            && self.style.font_weight == style.font_weight
            && self.style.font_style == style.font_style
            && self.foreground.value() == foreground
            && self.max_width_bits == normalized_width_bits(max_width_px)
            && self.wrap_mode == wrap_mode
    }

    fn owner_retained_bytes(
        source: &str,
        style: &TextStyle,
        foreground: &super::FlowchartTerminalForeground,
        render_id: &str,
    ) -> usize {
        let mut bytes = std::mem::size_of::<Self>();
        for retained in [
            source.len(),
            style.font_family.as_ref().map_or(0, String::len),
            style.font_weight.as_ref().map_or(0, String::len),
            style.font_style.as_ref().map_or(0, String::len),
            foreground.value().len(),
            FLOWCHART_RENDER_ID_RECORD_BYTES,
            render_id.len(),
            FLOWCHART_PREPARED_OWNER_SLOT_BYTES,
        ] {
            bytes = bytes.saturating_add(retained);
        }
        bytes
    }

    fn retained_bytes_without_reservation(
        source: &str,
        style: &TextStyle,
        foreground: &super::FlowchartTerminalForeground,
        render_id: &str,
        occurrence_id: &PreparedMathOccurrenceId,
        outcome: &MathPreparationOutcome,
    ) -> usize {
        let owner_bytes = Self::owner_retained_bytes(source, style, foreground, render_id);
        outcome.prepared().map_or_else(
            || owner_bytes.saturating_add(occurrence_id.as_str().len()),
            |artifact| owner_bytes.saturating_add(artifact.retained_bytes()),
        )
    }

    fn terminal_expectation(&self) -> Option<crate::math::PreparedMathExpectation> {
        let expected_emissions = self.expected_terminal_emissions;
        if expected_emissions == 0 {
            return None;
        }
        match &self.outcome {
            MathPreparationOutcome::Prepared(prepared) => prepared.expectation(expected_emissions),
            MathPreparationOutcome::Unavailable(MathPreparationUnavailable::NotMath) => None,
            MathPreparationOutcome::Unavailable(_) => {
                Some(crate::math::PreparedMathExpectation::unavailable(
                    self.occurrence_id.clone(),
                    expected_emissions,
                ))
            }
        }
    }

    fn take_terminal_reservation(&self) -> Option<PreparedTextRetainedReservation> {
        let expectation = self.terminal_expectation()?;
        let mut reservation = self.retained_reservation.borrow_mut().take()?;
        let projection = self
            .outcome
            .prepared()
            .and_then(PreparedMathLabel::native_projection)
            .map(Arc::as_ref);
        reservation.reconcile_downward(PreparedMathEvidenceLease::retained_entry_bytes(
            &expectation,
            projection,
        ));
        Some(reservation)
    }
}

#[derive(Debug, Clone)]
struct FlowchartMathTerminalStylePlan {
    node: super::FlowchartTerminalForeground,
    title: super::FlowchartTerminalForeground,
}

impl Default for FlowchartMathTerminalStylePlan {
    fn default() -> Self {
        Self {
            node: super::FlowchartTerminalForeground::new(
                "#333",
                super::FlowchartTerminalForegroundProvenance::ThemeNode,
            ),
            title: super::FlowchartTerminalForeground::new(
                "#333",
                super::FlowchartTerminalForegroundProvenance::ThemeTitle,
            ),
        }
    }
}

impl FlowchartMathTerminalStylePlan {
    #[cfg(test)]
    fn from_config(config: &merman_model::presentation::PresentationConfig) -> Self {
        Self::from_binding(&super::FlowchartCompatibilityBinding::resolve(
            config.as_value(),
        ))
    }

    fn from_binding(binding: &super::FlowchartCompatibilityBinding) -> Self {
        Self {
            node: super::FlowchartTerminalForeground::new(
                &binding.node_text_color,
                super::FlowchartTerminalForegroundProvenance::ThemeNode,
            ),
            title: super::FlowchartTerminalForeground::new(
                &binding.title_color,
                super::FlowchartTerminalForegroundProvenance::ThemeTitle,
            ),
        }
    }

    fn for_owner(&self, owner: FlowchartSvgLabelOwner) -> &super::FlowchartTerminalForeground {
        match owner {
            FlowchartSvgLabelOwner::SubgraphTitle(_)
            | FlowchartSvgLabelOwner::SwimlaneGroupTitle(_) => &self.title,
            FlowchartSvgLabelOwner::Node(_)
            | FlowchartSvgLabelOwner::EmptySubgraphNode(_)
            | FlowchartSvgLabelOwner::Edge(_)
            | FlowchartSvgLabelOwner::SwimlaneNode(_)
            | FlowchartSvgLabelOwner::SwimlaneEdgeLabel(_) => &self.node,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) enum FlowchartPreparedLabelResolution<'a> {
    #[default]
    NotPrepared,
    PreparedMath(&'a PreparedMathLabel),
    PreparedHtml(&'a str),
    Unavailable(MathPreparationUnavailable),
}

enum PreparedMathMeasurement {
    NotApplicable,
    Prepared(TextMetrics),
    Unavailable,
}

impl FlowchartSvgLabelSidecarBuilder {
    pub(crate) fn new_with_work_meter(
        resolved_theme: Option<&ResolvedDiagramTheme>,
        work_meter: Arc<OperationWorkMeter>,
    ) -> Self {
        let weights = super::FlowchartLabelWeights::resolve(resolved_theme, Some(&work_meter));
        let (label_weights, work_error) = match weights {
            Ok(weights) => (weights, None),
            Err(error) => (super::FlowchartLabelWeights::default(), Some(error)),
        };
        Self {
            label_weights,
            prepared_work_error: RefCell::new(work_error),
            work_meter: Some(work_meter),
            ..Self::default()
        }
    }

    pub(crate) fn with_typography_config_ownership(
        mut self,
        ownership: crate::flowchart::FlowchartTypographyConfigOwnership,
    ) -> Self {
        self.label_weights = self.label_weights.with_config(ownership);
        self
    }

    pub(crate) fn with_base_typography(mut self, plan: super::FlowchartBaseTypographyPlan) -> Self {
        self.base_typography = Some(plan);
        self
    }

    pub(crate) fn label_weights(&self) -> super::FlowchartLabelWeights {
        self.label_weights
    }

    pub(crate) fn layout_settings(
        &self,
        effective_config: &serde_json::Value,
    ) -> super::FlowchartLayoutSettings {
        self.base_typography.as_ref().map_or_else(
            || super::FlowchartConfigView::new(effective_config).layout_settings(),
            |plan| plan.layout_settings(effective_config),
        )
    }

    #[cfg(test)]
    pub(crate) fn with_math_backend(
        mut self,
        math_backend: Option<&ConfiguredMathBackend>,
        config: &merman_model::presentation::PresentationConfig,
    ) -> Self {
        self.math_backend = math_backend.cloned();
        self.math_terminal_style = FlowchartMathTerminalStylePlan::from_config(config);
        if self.work_meter.is_none() {
            self.work_meter = Some(Arc::new(OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            )));
        }
        self
    }

    pub(crate) fn with_prepared_math_backend(
        mut self,
        math_backend: Option<&ConfiguredMathBackend>,
        binding: &super::FlowchartCompatibilityBinding,
    ) -> Self {
        self.math_backend = math_backend.cloned();
        self.math_terminal_style = FlowchartMathTerminalStylePlan::from_binding(binding);
        if self.work_meter.is_none() {
            self.work_meter = Some(Arc::new(OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            )));
        }
        self
    }

    pub(crate) fn with_edge_label_padding(
        mut self,
        padding: super::FlowchartEdgeLabelPadding,
    ) -> Self {
        self.edge_label_padding = padding;
        self
    }

    pub(crate) const fn edge_label_padding(&self) -> super::FlowchartEdgeLabelPadding {
        self.edge_label_padding
    }

    pub(crate) fn has_prepared_math(
        &self,
        owner: FlowchartSvgLabelOwner,
        raw_source: &str,
    ) -> bool {
        self.pending.borrow().math.get(owner).is_some_and(|entry| {
            entry.source.as_ref() == raw_source && entry.outcome.prepared().is_some()
        })
    }

    fn record_prepared_resource_error(&self, error: ResourceLimitExceeded) {
        let mut prepared_resource_error = self.prepared_resource_error.borrow_mut();
        if prepared_resource_error.is_none() {
            *prepared_resource_error = Some(error);
        }
    }

    fn record_prepared_work_error(&self, error: OperationWorkError) {
        let error = match error {
            OperationWorkError::ResourceLimitExceeded(resource) => {
                self.record_prepared_resource_error(resource);
                return;
            }
            error => error,
        };
        let mut prepared_work_error = self.prepared_work_error.borrow_mut();
        if prepared_work_error.is_none() {
            *prepared_work_error = Some(error);
        }
    }

    fn record_render_id(&self, owner: FlowchartSvgLabelOwner, render_id: &str) -> bool {
        // Math owners without a source-typography slot already include this index in their
        // retained reservation. An unavailable backend may additionally prepare plain HTML.
        if self.pending.borrow().math.get(owner).is_some()
            && self
                .pending
                .borrow()
                .render_ids
                .get(owner)
                .is_some_and(|id| id.as_ref() == render_id)
        {
            return true;
        }
        if self
            .pending
            .borrow()
            .render_id_reservations
            .get(owner)
            .is_some()
        {
            debug_assert_eq!(
                self.pending
                    .borrow()
                    .render_ids
                    .get(owner)
                    .map(AsRef::as_ref),
                Some(render_id)
            );
            return true;
        }
        let retained_bytes = FLOWCHART_RENDER_ID_RECORD_BYTES
            .saturating_add(render_id.len())
            .saturating_add(FLOWCHART_PREPARED_OWNER_SLOT_BYTES);
        let reservation = match self
            .work_meter
            .as_ref()
            .map(|work| work.reserve_prepared_text_retained_bytes(retained_bytes))
            .transpose()
        {
            Ok(reservation) => reservation,
            Err(error) => {
                self.record_prepared_resource_error(error);
                return false;
            }
        };
        let mut pending = self.pending.borrow_mut();
        pending.render_ids.insert(owner, render_id.into());
        if let Some(reservation) = reservation {
            pending.render_id_reservations.insert(owner, reservation);
        }
        true
    }

    #[cfg(test)]
    pub(crate) fn measure_for_layout(
        &self,
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: FlowchartLabelMetricsRequest<'_>,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> TextMetrics {
        self.measure_for_layout_with_terminal_foreground(
            owner,
            render_id,
            request,
            None,
            break_long_words,
            width_mode,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn measure_for_layout_with_terminal_foreground(
        &self,
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: FlowchartLabelMetricsRequest<'_>,
        terminal_foreground: Option<&super::FlowchartTerminalForeground>,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> TextMetrics {
        let metrics_style = request.style;
        self.measure_for_layout_with_metrics_style_and_terminal_foreground(
            owner,
            render_id,
            request,
            metrics_style,
            terminal_foreground,
            break_long_words,
            width_mode,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub(crate) fn measure_for_layout_with_metrics_style(
        &self,
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: FlowchartLabelMetricsRequest<'_>,
        metrics_style: &TextStyle,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> TextMetrics {
        self.measure_for_layout_with_metrics_style_and_terminal_foreground(
            owner,
            render_id,
            request,
            metrics_style,
            None,
            break_long_words,
            width_mode,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn measure_for_layout_with_metrics_style_and_terminal_foreground(
        &self,
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: FlowchartLabelMetricsRequest<'_>,
        metrics_style: &TextStyle,
        terminal_foreground: Option<&super::FlowchartTerminalForeground>,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> TextMetrics {
        if self.prepared_resource_error.borrow().is_some()
            || self.prepared_work_error.borrow().is_some()
        {
            return failed_prepared_metrics();
        }
        if matches!(
            owner,
            FlowchartSvgLabelOwner::Node(_)
                | FlowchartSvgLabelOwner::EmptySubgraphNode(_)
                | FlowchartSvgLabelOwner::SwimlaneNode(_)
                | FlowchartSvgLabelOwner::SubgraphTitle(_)
                | FlowchartSvgLabelOwner::SwimlaneGroupTitle(_)
        ) && self
            .pending
            .borrow()
            .node_source_typography
            .get(owner)
            .is_none()
        {
            if !self.record_render_id(owner, render_id) {
                return failed_prepared_metrics();
            }
            let retained_bytes = FlowchartNodeSourceTypography::retained_bytes(request.style);
            let reservation = match self
                .work_meter
                .as_ref()
                .map(|work| work.reserve_prepared_text_retained_bytes(retained_bytes))
                .transpose()
            {
                Ok(reservation) => reservation,
                Err(error) => {
                    self.record_prepared_resource_error(error);
                    return failed_prepared_metrics();
                }
            };
            let mut pending = self.pending.borrow_mut();
            pending.node_source_typography.insert(
                owner,
                Arc::new(FlowchartNodeSourceTypography {
                    style: request.style.clone(),
                    _retained_reservation: reservation,
                }),
            );
        }
        let weighted_wrap = self
            .label_weights
            .apply(owner.typography_target(), request.style);
        let weighted_metrics = self
            .label_weights
            .apply(owner.typography_target(), metrics_style);
        let request = FlowchartLabelMetricsRequest {
            style: &weighted_wrap,
            ..request
        };
        let metrics_style = weighted_metrics.as_ref();
        if self
            .base_typography
            .as_ref()
            .is_some_and(super::FlowchartBaseTypographyPlan::requires_terminal_evidence)
        {
            self.pending
                .borrow_mut()
                .render_ids
                .insert(owner, render_id.into());
        }
        match self.measure_prepared_math(
            owner,
            render_id,
            &request,
            metrics_style,
            terminal_foreground,
        ) {
            PreparedMathMeasurement::Prepared(metrics) => return metrics,
            PreparedMathMeasurement::Unavailable => {
                return self.measure_prepared_html(
                    owner,
                    render_id,
                    FlowchartLabelMetricsRequest {
                        style: metrics_style,
                        math_renderer: None,
                        ..request
                    },
                );
            }
            PreparedMathMeasurement::NotApplicable => {}
        }
        if request.wrap_mode == WrapMode::HtmlLike {
            return self.measure_prepared_html(
                owner,
                render_id,
                FlowchartLabelMetricsRequest {
                    style: metrics_style,
                    ..request
                },
            );
        }
        if !supports_svg_source_preparation(owner, &request) {
            return flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
                style: metrics_style,
                ..request
            });
        }

        let binding = FlowchartSvgLabelBindingRequest::for_measurer(
            request.measurer,
            request.style,
            metrics_style,
            request.max_width_px,
            break_long_words,
            width_mode,
        );

        let measure_wrapped = |source: &FlowchartSvgLabelSource, wrapped_lines: &[Vec<String>]| {
            if request.label_type == "markdown" {
                // Plain Markdown shares the prepared SVG rows, but retains its computed-length
                // measurement and 1/64px width lattice rather than the non-Markdown bbox path.
                let markdown_lines = wrapped_lines
                    .iter()
                    .map(|line| {
                        line.iter()
                            .map(|word| {
                                (word.clone(), crate::text::MermaidMarkdownWordType::Normal)
                            })
                            .collect()
                    })
                    .collect::<Vec<_>>();
                crate::text::measure_svg_markdown_word_lines(
                    request.measurer,
                    &markdown_lines,
                    metrics_style,
                    crate::text::SvgTextEntityMode::CreateTextSource,
                )
            } else {
                source.metrics_from_wrapped(
                    request.measurer,
                    metrics_style,
                    wrapped_lines,
                    width_mode,
                )
            }
        };

        let measured_from_existing_source = {
            let pending = self.pending.borrow();
            let source = pending
                .render_ids
                .get(owner)
                .filter(|pending_render_id| pending_render_id.as_ref() == render_id)
                .and_then(|_| pending.sources.get(owner))
                .filter(|source| source.matches(request.raw_label));

            if let (Some(_), Some(binding)) = (source, binding.as_ref())
                && let Some(metrics) = pending
                    .prepared
                    .get(owner)
                    .filter(|prepared| prepared.matches(binding))
                    .map(PreparedFlowchartSvgLabel::metrics)
            {
                #[cfg(test)]
                self.prepared_hits
                    .set(self.prepared_hits.get().saturating_add(1));
                return metrics;
            }

            source.map(|source| {
                let wrapped_lines = source.source.wrapped_lines(
                    request.measurer,
                    request.style,
                    request.max_width_px,
                    break_long_words,
                );
                let metrics = measure_wrapped(&source.source, &wrapped_lines);
                (wrapped_lines, metrics)
            })
        };

        let (new_source, wrapped_lines, metrics) = measured_from_existing_source.map_or_else(
            || {
                let source = FlowchartSvgLabelSourceEntry::new(request.raw_label);
                let wrapped_lines = source.source.wrapped_lines(
                    request.measurer,
                    request.style,
                    request.max_width_px,
                    break_long_words,
                );
                let metrics = measure_wrapped(&source.source, &wrapped_lines);
                (Some(source), wrapped_lines, metrics)
            },
            |(wrapped_lines, metrics)| (None, wrapped_lines, metrics),
        );

        #[cfg(test)]
        if new_source.is_some() {
            let mut source_plans = self.source_plans_by_owner.borrow_mut();
            let owner_plans = source_plans.entry(owner).or_default();
            *owner_plans = owner_plans.saturating_add(1);
        }

        let mut pending = self.pending.borrow_mut();
        pending.render_ids.insert(owner, render_id.into());
        if let Some(source) = new_source {
            pending.sources.insert(owner, source);
            pending.prepared.remove(owner);
        }
        if let Some(binding) = binding {
            pending.prepared.insert(
                owner,
                PreparedFlowchartSvgLabel::new(binding.into_owned(), wrapped_lines, metrics),
            );
        } else {
            pending.prepared.remove(owner);
        }
        metrics
    }

    fn measure_prepared_html(
        &self,
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: FlowchartLabelMetricsRequest<'_>,
    ) -> TextMetrics {
        let resolved_config = crate::graph_label::flowchart_label_sanitize_config(request.config);
        {
            let pending = self.pending.borrow();
            if let Some(prepared) = pending
                .html
                .get(owner)
                .filter(|prepared| prepared.matches(request.raw_label, request.label_type))
            {
                return crate::graph_label::flowchart_html_label_metrics_for_layout(
                    request,
                    &prepared.xhtml,
                );
            }
        }
        if !self.record_render_id(owner, render_id) {
            return failed_prepared_metrics();
        }
        let owner_bytes = std::mem::size_of::<PreparedFlowchartHtmlLabel>()
            .saturating_add(FLOWCHART_PREPARED_OWNER_SLOT_BYTES)
            .saturating_add(request.raw_label.len())
            .saturating_add(request.label_type.len());
        let trusted_meter;
        let work = match self.work_meter.as_deref() {
            Some(work) => work,
            None => {
                trusted_meter = OperationWorkMeter::new(
                    crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
                );
                &trusted_meter
            }
        };
        if let Err(error) = work.preflight_prepared_text_retained_bytes(owner_bytes) {
            self.record_prepared_work_error(error);
            return failed_prepared_metrics();
        }
        if let Err(error) = work.charge(1usize.saturating_add(request.raw_label.len() / 64)) {
            self.record_prepared_work_error(error);
            return failed_prepared_metrics();
        }
        let max = work
            .remaining_prepared_text_retained_bytes()
            .map_or(usize::MAX, |remaining| {
                remaining.saturating_sub(owner_bytes)
            });
        let parse_work = Cell::new(0);
        let sink = crate::html::HtmlSanitizeSink::new(
            max,
            work,
            &parse_work,
            merman_model::OperationPhase::Layout,
        );
        let html = crate::graph_label::flowchart_label_html_source(
            request.raw_label,
            request.label_type,
            request.config,
        );
        let xhtml = match merman_markup::sanitize::sanitize_xhtml_with_sink(
            &html,
            resolved_config.as_ref(),
            &sink,
        ) {
            Ok(xhtml) => xhtml,
            Err(merman_markup::sanitize::SanitizeFailure::Output(
                crate::html::HtmlOutputError::Operation(error),
            )) => {
                self.record_prepared_work_error(error);
                return failed_prepared_metrics();
            }
            Err(merman_markup::sanitize::SanitizeFailure::Output(
                crate::html::HtmlOutputError::Limit { attempted },
            )) => {
                if let Err(error) = work
                    .preflight_prepared_text_retained_bytes(owner_bytes.saturating_add(attempted))
                {
                    self.record_prepared_work_error(error);
                    return failed_prepared_metrics();
                }
                String::new()
            }
            // Preserve the string sanitizer's fail-closed compatibility behavior.
            Err(_) => String::new(),
        };
        let retained_bytes = owner_bytes.saturating_add(xhtml.len());
        let reservation = match self
            .work_meter
            .as_ref()
            .map(|work| work.reserve_prepared_text_retained_bytes(retained_bytes))
            .transpose()
        {
            Ok(reservation) => reservation,
            Err(error) => {
                self.record_prepared_resource_error(error);
                return failed_prepared_metrics();
            }
        };
        let source = request.raw_label;
        let label_type = request.label_type;
        let metrics = crate::graph_label::flowchart_html_label_metrics_for_layout(request, &xhtml);
        self.pending.borrow_mut().html.insert(
            owner,
            PreparedFlowchartHtmlLabel {
                source: source.into(),
                label_type: label_type.into(),
                xhtml,
                _retained_reservation: reservation,
            },
        );
        metrics
    }

    fn measure_prepared_math(
        &self,
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: &FlowchartLabelMetricsRequest<'_>,
        metrics_style: &TextStyle,
        terminal_foreground: Option<&super::FlowchartTerminalForeground>,
    ) -> PreparedMathMeasurement {
        if request.wrap_mode != WrapMode::HtmlLike || !request.raw_label.contains("$$") {
            return PreparedMathMeasurement::NotApplicable;
        }
        let foreground =
            terminal_foreground.unwrap_or_else(|| self.math_terminal_style.for_owner(owner));
        {
            let pending = self.pending.borrow();
            if let Some(prepared) = pending.math.get(owner).filter(|prepared| {
                prepared.matches(
                    request.raw_label,
                    metrics_style,
                    foreground.value(),
                    request.max_width_px,
                    request.wrap_mode,
                )
            }) {
                #[cfg(test)]
                self.prepared_hits
                    .set(self.prepared_hits.get().saturating_add(1));
                return match prepared.outcome.prepared() {
                    Some(artifact) => PreparedMathMeasurement::Prepared(artifact.metrics()),
                    None => PreparedMathMeasurement::Unavailable,
                };
            }
        }

        let owner_retained_bytes = PreparedFlowchartMathLabel::owner_retained_bytes(
            request.raw_label,
            metrics_style,
            foreground,
            render_id,
        )
        .saturating_sub(
            if self
                .pending
                .borrow()
                .node_source_typography
                .get(owner)
                .is_some()
            {
                FLOWCHART_RENDER_ID_RECORD_BYTES.saturating_add(render_id.len())
            } else {
                0
            },
        );
        let occurrence_id = owner.prepared_math_occurrence_id();
        let outcome = match self.math_backend.as_ref() {
            Some(backend) => match backend.prepare(
                PrepareMathLabelRequest::flowchart(
                    request.raw_label,
                    request.config,
                    metrics_style,
                    foreground.value(),
                    request.max_width_px,
                    request.wrap_mode,
                )
                .with_text_measurer(request.measurer)
                .with_occurrence_id(&occurrence_id)
                .with_owner_retained_bytes(owner_retained_bytes),
                self.work_meter
                    .as_ref()
                    .expect("math preparation requires an operation work meter"),
            ) {
                Ok(outcome) => outcome,
                Err(error) => {
                    self.record_prepared_work_error(error);
                    return PreparedMathMeasurement::Unavailable;
                }
            },
            None => {
                MathPreparationOutcome::Unavailable(MathPreparationUnavailable::BackendUnavailable)
            }
        };
        let retained_bytes = PreparedFlowchartMathLabel::retained_bytes_without_reservation(
            request.raw_label,
            metrics_style,
            foreground,
            render_id,
            &occurrence_id,
            &outcome,
        )
        .saturating_sub(
            if self
                .pending
                .borrow()
                .node_source_typography
                .get(owner)
                .is_some()
            {
                FLOWCHART_RENDER_ID_RECORD_BYTES.saturating_add(render_id.len())
            } else {
                0
            },
        );
        let Some(work_meter) = self.work_meter.as_ref() else {
            return PreparedMathMeasurement::Unavailable;
        };
        let reservation = match work_meter.reserve_prepared_text_retained_bytes(retained_bytes) {
            Ok(reservation) => reservation,
            Err(error) => {
                self.record_prepared_resource_error(error);
                return PreparedMathMeasurement::Unavailable;
            }
        };
        let metrics = outcome.prepared().map(PreparedMathLabel::metrics);
        let mut pending = self.pending.borrow_mut();
        pending.render_ids.insert(owner, render_id.into());
        pending.math.insert(
            owner,
            PreparedFlowchartMathLabel {
                occurrence_id,
                source: Arc::from(request.raw_label),
                style: metrics_style.clone(),
                foreground: foreground.clone(),
                max_width_bits: normalized_width_bits(request.max_width_px),
                wrap_mode: request.wrap_mode,
                outcome,
                // One semantic Flowchart/Swimlane label owner maps to one terminal label.
                expected_terminal_emissions: 1,
                retained_reservation: RefCell::new(Some(reservation)),
            },
        );
        match metrics {
            Some(metrics) => PreparedMathMeasurement::Prepared(metrics),
            None => PreparedMathMeasurement::Unavailable,
        }
    }

    pub(crate) fn finish(self) -> FlowchartSvgLabelSidecar {
        #[cfg(test)]
        let source_plans_by_owner = self.source_plans_by_owner.into_inner();
        let pending = self.pending.into_inner();
        FlowchartSvgLabelSidecar::new(
            pending.node_source_typography,
            pending.sources,
            pending.prepared,
            pending.math,
            pending.html,
            pending.render_ids,
            pending.render_id_reservations,
            self.base_typography,
            self.label_weights,
            self.edge_label_padding,
            self.prepared_resource_error.into_inner(),
            self.prepared_work_error.into_inner(),
            #[cfg(test)]
            source_plans_by_owner,
        )
    }

    #[cfg(test)]
    pub(crate) fn prepared_count(&self) -> usize {
        self.pending.borrow().prepared.len()
    }

    #[cfg(test)]
    pub(crate) fn prepared_hit_count(&self) -> usize {
        self.prepared_hits.get()
    }
}

fn failed_prepared_metrics() -> TextMetrics {
    TextMetrics {
        width: 0.0,
        height: 0.0,
        line_count: 0,
    }
}

#[cfg(test)]
pub(crate) fn measure_flowchart_svg_label_for_layout(
    sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    owner: Option<FlowchartSvgLabelOwner>,
    render_id: Option<&str>,
    request: FlowchartLabelMetricsRequest<'_>,
    width_mode: FlowchartSvgWidthMode,
) -> TextMetrics {
    measure_flowchart_svg_label_for_layout_with_terminal_foreground(
        sidecar, owner, render_id, request, None, width_mode,
    )
}

pub(crate) fn measure_flowchart_svg_label_for_layout_with_terminal_foreground(
    sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    owner: Option<FlowchartSvgLabelOwner>,
    render_id: Option<&str>,
    request: FlowchartLabelMetricsRequest<'_>,
    terminal_foreground: Option<&super::FlowchartTerminalForeground>,
    width_mode: FlowchartSvgWidthMode,
) -> TextMetrics {
    match sidecar.zip(owner).zip(render_id) {
        Some(((sidecar, owner), render_id)) => sidecar.measure_for_layout_with_terminal_foreground(
            owner,
            render_id,
            request,
            terminal_foreground,
            true,
            width_mode,
        ),
        None => measure_svg_label_without_sidecar(request, width_mode),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn measure_flowchart_svg_label_for_layout_with_metrics_style_and_terminal_foreground(
    sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    owner: Option<FlowchartSvgLabelOwner>,
    render_id: Option<&str>,
    request: FlowchartLabelMetricsRequest<'_>,
    metrics_style: &TextStyle,
    terminal_foreground: Option<&super::FlowchartTerminalForeground>,
    width_mode: FlowchartSvgWidthMode,
) -> TextMetrics {
    match sidecar.zip(owner).zip(render_id) {
        Some(((sidecar, owner), render_id)) => sidecar
            .measure_for_layout_with_metrics_style_and_terminal_foreground(
                owner,
                render_id,
                request,
                metrics_style,
                terminal_foreground,
                true,
                width_mode,
            ),
        None => {
            measure_svg_label_without_sidecar_with_metrics_style(request, metrics_style, width_mode)
        }
    }
}

fn supports_svg_source_preparation(
    owner: FlowchartSvgLabelOwner,
    request: &FlowchartLabelMetricsRequest<'_>,
) -> bool {
    if request.wrap_mode != WrapMode::SvgLike {
        return false;
    }
    if request.label_type != "markdown" {
        return true;
    }
    matches!(owner, FlowchartSvgLabelOwner::SwimlaneGroupTitle(_))
        && markdown_matches_non_markdown_svg_source(request.raw_label)
}

fn markdown_matches_non_markdown_svg_source(raw_label: &str) -> bool {
    if !crate::text::mermaid_markdown_is_plain_text(raw_label) {
        return false;
    }
    let markdown = crate::text::analyze_mermaid_markdown(raw_label, true);
    if !markdown.all_runs_normal() {
        return false;
    }
    let svg_lines = crate::flowchart::flowchart_non_markdown_svg_source_word_lines(raw_label);
    markdown.lines.len() == svg_lines.len()
        && markdown.lines.iter().zip(svg_lines).all(|(markdown, svg)| {
            markdown.len() == svg.len()
                && markdown
                    .iter()
                    .zip(svg)
                    .all(|((markdown, _), svg)| markdown == &svg)
        })
}

fn measure_svg_label_without_sidecar(
    request: FlowchartLabelMetricsRequest<'_>,
    width_mode: FlowchartSvgWidthMode,
) -> TextMetrics {
    let metrics_style = request.style;
    measure_svg_label_without_sidecar_with_metrics_style(request, metrics_style, width_mode)
}

fn measure_svg_label_without_sidecar_with_metrics_style(
    request: FlowchartLabelMetricsRequest<'_>,
    metrics_style: &TextStyle,
    width_mode: FlowchartSvgWidthMode,
) -> TextMetrics {
    if request.wrap_mode != WrapMode::SvgLike || request.label_type == "markdown" {
        return flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            style: metrics_style,
            ..request
        });
    }
    let source = FlowchartSvgLabelSource::new(request.raw_label);
    let wrapped_lines =
        source.wrapped_lines(request.measurer, request.style, request.max_width_px, true);
    source.metrics_from_wrapped(request.measurer, metrics_style, &wrapped_lines, width_mode)
}

/// Immutable render-side index. It is private to a single prepared Flowchart family artifact.
#[derive(Debug, Default)]
pub(crate) struct FlowchartSvgLabelSidecar {
    _render_id_reservations: FlowchartSvgLabelSlots<PreparedTextRetainedReservation>,
    node_source_typography: FlowchartSvgLabelSlots<Arc<FlowchartNodeSourceTypography>>,
    sources: FlowchartSvgLabelSlots<FlowchartSvgLabelSourceEntry>,
    prepared: FlowchartSvgLabelSlots<PreparedFlowchartSvgLabel>,
    math: FlowchartSvgLabelSlots<PreparedFlowchartMathLabel>,
    html: FlowchartSvgLabelSlots<PreparedFlowchartHtmlLabel>,
    base_typography: Option<super::FlowchartBaseTypographyPlan>,
    label_weights: super::FlowchartLabelWeights,
    node_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    empty_subgraph_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    edge_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    subgraph_title_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    swimlane_node_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    swimlane_edge_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    swimlane_group_title_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    edge_label_padding: super::FlowchartEdgeLabelPadding,
    prepared_resource_error: Option<ResourceLimitExceeded>,
    prepared_work_error: Option<OperationWorkError>,
    #[cfg(test)]
    prepared_hits_by_owner: Mutex<FxHashMap<FlowchartSvgLabelOwner, usize>>,
    #[cfg(test)]
    source_plans_by_owner: Mutex<FxHashMap<FlowchartSvgLabelOwner, usize>>,
}

impl FlowchartSvgLabelSidecar {
    pub(crate) fn label_weights(&self) -> super::FlowchartLabelWeights {
        self.label_weights
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "The sidecar seals independent prepared label slots and terminal indexes"
    )]
    fn new(
        node_source_typography: FlowchartSvgLabelSlots<Arc<FlowchartNodeSourceTypography>>,
        sources: FlowchartSvgLabelSlots<FlowchartSvgLabelSourceEntry>,
        prepared: FlowchartSvgLabelSlots<PreparedFlowchartSvgLabel>,
        math: FlowchartSvgLabelSlots<PreparedFlowchartMathLabel>,
        html: FlowchartSvgLabelSlots<PreparedFlowchartHtmlLabel>,
        render_ids: FlowchartSvgLabelSlots<Box<str>>,
        render_id_reservations: FlowchartSvgLabelSlots<PreparedTextRetainedReservation>,
        base_typography: Option<super::FlowchartBaseTypographyPlan>,
        label_weights: super::FlowchartLabelWeights,
        edge_label_padding: super::FlowchartEdgeLabelPadding,
        prepared_resource_error: Option<ResourceLimitExceeded>,
        prepared_work_error: Option<OperationWorkError>,
        #[cfg(test)] source_plans_by_owner: FxHashMap<FlowchartSvgLabelOwner, usize>,
    ) -> Self {
        let mut sidecar = Self {
            _render_id_reservations: render_id_reservations,
            node_source_typography,
            sources,
            prepared,
            math,
            html,
            base_typography,
            label_weights,
            edge_label_padding,
            prepared_resource_error,
            prepared_work_error,
            #[cfg(test)]
            source_plans_by_owner: Mutex::new(source_plans_by_owner),
            ..Self::default()
        };
        render_ids.for_each(|owner, render_id| {
            let render_id = render_id.into_string();
            match owner {
                FlowchartSvgLabelOwner::Node(_) => {
                    insert_last_owner(&mut sidecar.node_owner_by_id, render_id, owner);
                }
                FlowchartSvgLabelOwner::EmptySubgraphNode(_) => {
                    insert_last_owner(&mut sidecar.empty_subgraph_owner_by_id, render_id, owner);
                }
                FlowchartSvgLabelOwner::Edge(_) => {
                    insert_last_owner(&mut sidecar.edge_owner_by_id, render_id, owner);
                }
                FlowchartSvgLabelOwner::SubgraphTitle(_) => {
                    // Parsed models have unique group ids. Preserve the canonical first title
                    // owner for public typed models that explicitly contain duplicate groups.
                    insert_first_owner(&mut sidecar.subgraph_title_owner_by_id, render_id, owner);
                }
                FlowchartSvgLabelOwner::SwimlaneNode(_) => {
                    insert_last_owner(&mut sidecar.swimlane_node_owner_by_id, render_id, owner);
                }
                FlowchartSvgLabelOwner::SwimlaneEdgeLabel(_) => {
                    insert_last_owner(&mut sidecar.swimlane_edge_owner_by_id, render_id, owner);
                }
                FlowchartSvgLabelOwner::SwimlaneGroupTitle(_) => {
                    insert_first_owner(
                        &mut sidecar.swimlane_group_title_owner_by_id,
                        render_id,
                        owner,
                    );
                }
            }
        });
        sidecar
    }

    pub(crate) fn prepared_resource_error(&self) -> Option<&ResourceLimitExceeded> {
        self.prepared_resource_error.as_ref()
    }

    pub(crate) fn prepared_work_error(&self) -> Option<&OperationWorkError> {
        self.prepared_work_error.as_ref()
    }

    pub(crate) const fn edge_label_padding(&self) -> super::FlowchartEdgeLabelPadding {
        self.edge_label_padding
    }

    pub(crate) fn base_typography(&self) -> Option<&super::FlowchartBaseTypographyPlan> {
        self.base_typography.as_ref()
    }

    pub(crate) fn prepared_math_evidence(&self) -> PreparedMathEvidenceLease {
        let entries = self
            .math
            .iter()
            .filter_map(|entry| {
                Some((
                    entry.terminal_expectation()?,
                    entry
                        .outcome
                        .prepared()
                        .and_then(PreparedMathLabel::native_projection)
                        .cloned(),
                ))
            })
            .collect();
        let reservations = self
            .math
            .iter()
            .filter_map(PreparedFlowchartMathLabel::take_terminal_reservation)
            .collect();
        PreparedMathEvidenceLease::with_native_projections(entries, reservations)
    }

    pub(crate) fn node_owner(
        &self,
        node_id: &str,
        swimlane: bool,
    ) -> Option<FlowchartSvgLabelOwner> {
        if swimlane {
            self.swimlane_node_owner_by_id.get(node_id).copied()
        } else {
            self.empty_subgraph_owner_by_id
                .get(node_id)
                .or_else(|| self.node_owner_by_id.get(node_id))
                .copied()
        }
    }

    pub(crate) fn node_source_style(
        &self,
        owner: FlowchartSvgLabelOwner,
    ) -> Option<&Arc<FlowchartNodeSourceTypography>> {
        self.node_source_typography.get(owner)
    }

    pub(crate) fn edge_owner(
        &self,
        edge_id: &str,
        swimlane: bool,
    ) -> Option<FlowchartSvgLabelOwner> {
        if swimlane {
            self.swimlane_edge_owner_by_id.get(edge_id).copied()
        } else {
            self.edge_owner_by_id.get(edge_id).copied()
        }
    }

    pub(crate) fn subgraph_title_owner(&self, subgraph_id: &str) -> Option<FlowchartSvgLabelOwner> {
        self.subgraph_title_owner_by_id.get(subgraph_id).copied()
    }

    pub(crate) fn swimlane_group_title_owner(
        &self,
        subgraph_id: &str,
    ) -> Option<FlowchartSvgLabelOwner> {
        self.swimlane_group_title_owner_by_id
            .get(subgraph_id)
            .copied()
    }

    #[cfg(test)]
    pub(crate) fn prepared_math(
        &self,
        owner: FlowchartSvgLabelOwner,
        raw_source: &str,
    ) -> FlowchartPreparedLabelResolution<'_> {
        let Some(prepared) = self
            .math
            .get(owner)
            .filter(|prepared| prepared.source.as_ref() == raw_source)
        else {
            return FlowchartPreparedLabelResolution::NotPrepared;
        };
        Self::resolve_prepared_math(prepared)
    }

    pub(crate) fn prepared_label_for_terminal(
        &self,
        owner: FlowchartSvgLabelOwner,
        raw_source: &str,
    ) -> FlowchartPreparedLabelResolution<'_> {
        let prepared_math = self
            .math
            .get(owner)
            .filter(|prepared| prepared.source.as_ref() == raw_source);
        if let Some(artifact) = prepared_math.and_then(|prepared| prepared.outcome.prepared()) {
            return FlowchartPreparedLabelResolution::PreparedMath(artifact);
        }
        if let Some(prepared) = self
            .html
            .get(owner)
            .filter(|prepared| prepared.source.as_ref() == raw_source)
        {
            return FlowchartPreparedLabelResolution::PreparedHtml(&prepared.xhtml);
        }
        let Some(prepared) = prepared_math else {
            return FlowchartPreparedLabelResolution::NotPrepared;
        };
        Self::resolve_prepared_math(prepared)
    }

    fn resolve_prepared_math(
        prepared: &PreparedFlowchartMathLabel,
    ) -> FlowchartPreparedLabelResolution<'_> {
        match &prepared.outcome {
            MathPreparationOutcome::Prepared(artifact) => {
                FlowchartPreparedLabelResolution::PreparedMath(artifact)
            }
            MathPreparationOutcome::Unavailable(reason) => {
                FlowchartPreparedLabelResolution::Unavailable(*reason)
            }
        }
    }

    #[cfg(test)]
    fn prepared_math_arc(
        &self,
        owner: FlowchartSvgLabelOwner,
        raw_source: &str,
    ) -> Option<&Arc<PreparedMathLabel>> {
        self.math
            .get(owner)
            .filter(|prepared| prepared.source.as_ref() == raw_source)
            .and_then(|prepared| match &prepared.outcome {
                MathPreparationOutcome::Prepared(artifact) => Some(artifact),
                MathPreparationOutcome::Unavailable(_) => None,
            })
    }

    fn prepared(
        &self,
        owner: FlowchartSvgLabelOwner,
        raw_source: &str,
        binding: &FlowchartSvgLabelBindingRequest<'_>,
    ) -> Option<(&FlowchartSvgLabelSource, &PreparedFlowchartSvgLabel)> {
        let source = self
            .sources
            .get(owner)
            .filter(|source| source.matches(raw_source));
        let prepared = source.map(|source| &source.source).zip(
            self.prepared
                .get(owner)
                .filter(|prepared| prepared.matches(binding)),
        );
        #[cfg(test)]
        if prepared.is_some() {
            let mut hits = self
                .prepared_hits_by_owner
                .lock()
                .expect("prepared-hit test observer lock");
            let owner_hits = hits.entry(owner).or_default();
            *owner_hits = owner_hits.saturating_add(1);
        }
        prepared
    }

    fn prepared_wrapping(
        &self,
        owner: FlowchartSvgLabelOwner,
        raw_source: &str,
        binding: &FlowchartSvgLabelWrapBindingRequest<'_>,
    ) -> Option<(&FlowchartSvgLabelSource, &PreparedFlowchartSvgLabel)> {
        let source = self
            .sources
            .get(owner)
            .filter(|source| source.matches(raw_source));
        let prepared = source.map(|source| &source.source).zip(
            self.prepared
                .get(owner)
                .filter(|prepared| prepared.matches_wrapping(binding)),
        );
        #[cfg(test)]
        if prepared.is_some() {
            let mut hits = self
                .prepared_hits_by_owner
                .lock()
                .expect("prepared-hit test observer lock");
            let owner_hits = hits.entry(owner).or_default();
            *owner_hits = owner_hits.saturating_add(1);
        }
        prepared
    }

    fn source(
        &self,
        owner: FlowchartSvgLabelOwner,
        raw_source: &str,
    ) -> Option<&FlowchartSvgLabelSource> {
        self.sources
            .get(owner)
            .filter(|source| source.matches(raw_source))
            .map(|source| &source.source)
    }

    #[cfg(test)]
    pub(crate) fn prepared_hit_count(&self, owner: FlowchartSvgLabelOwner) -> usize {
        self.prepared_hits_by_owner
            .lock()
            .expect("prepared-hit test observer lock")
            .get(&owner)
            .copied()
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn source_plan_count(&self, owner: FlowchartSvgLabelOwner) -> usize {
        self.source_plans_by_owner
            .lock()
            .expect("source-plan test observer lock")
            .get(&owner)
            .copied()
            .unwrap_or_default()
    }

    /// Returns a shadow allocation in centered text coordinates, using native ink when known.
    /// Host-backed allocations retain a bounded reserve and require final-target observation.
    pub(crate) fn centered_shadow_bounds(
        &self,
        owner: FlowchartSvgLabelOwner,
        raw_source: &str,
        metrics_style: &TextStyle,
    ) -> Option<[f64; 4]> {
        let source = self
            .sources
            .get(owner)
            .filter(|source| source.matches(raw_source))?;
        if source.source.plain_text().trim().is_empty() {
            return None;
        }
        self.prepared
            .get(owner)?
            .centered_shadow_bounds(metrics_style)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepared_metrics(
        &self,
        owner: FlowchartSvgLabelOwner,
        raw_source: &str,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        max_width_px: Option<f64>,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> Option<TextMetrics> {
        let binding = FlowchartSvgLabelBindingRequest::for_measurer(
            measurer,
            style,
            style,
            max_width_px,
            break_long_words,
            width_mode,
        )?;
        self.prepared(owner, raw_source, &binding)
            .map(|(_, prepared)| prepared.metrics())
    }
}

fn insert_last_owner(
    index: &mut FxHashMap<String, FlowchartSvgLabelOwner>,
    id: String,
    owner: FlowchartSvgLabelOwner,
) {
    match index.get(id.as_str()).copied() {
        Some(current) if current.semantic_index() > owner.semantic_index() => {}
        _ => {
            index.insert(id, owner);
        }
    }
}

fn insert_first_owner(
    index: &mut FxHashMap<String, FlowchartSvgLabelOwner>,
    id: String,
    owner: FlowchartSvgLabelOwner,
) {
    index.entry(id).or_insert(owner);
}

pub(crate) enum FlowchartSvgLabelRenderPlan<'a> {
    Prepared {
        source: &'a FlowchartSvgLabelSource,
        measured: &'a PreparedFlowchartSvgLabel,
        font_weight: Option<u16>,
    },
    Source {
        source: Cow<'a, FlowchartSvgLabelSource>,
        measurer: &'a dyn TextMeasurer,
        style: &'a TextStyle,
        font_weight: Option<u16>,
        max_width_px: Option<f64>,
        break_long_words: bool,
    },
}

impl<'a> FlowchartSvgLabelRenderPlan<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        sidecar: Option<&'a FlowchartSvgLabelSidecar>,
        owner: Option<FlowchartSvgLabelOwner>,
        raw_source: &str,
        measurer: &'a dyn TextMeasurer,
        style: &'a TextStyle,
        max_width_px: Option<f64>,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> Self {
        Self::new_with_metrics_style(
            sidecar,
            owner,
            raw_source,
            measurer,
            style,
            style,
            max_width_px,
            break_long_words,
            width_mode,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_metrics_style(
        sidecar: Option<&'a FlowchartSvgLabelSidecar>,
        owner: Option<FlowchartSvgLabelOwner>,
        raw_source: &str,
        measurer: &'a dyn TextMeasurer,
        wrap_style: &'a TextStyle,
        metrics_style: &TextStyle,
        max_width_px: Option<f64>,
        break_long_words: bool,
        _width_mode: FlowchartSvgWidthMode,
    ) -> Self {
        let font_weight = sidecar.zip(owner).and_then(|(sidecar, owner)| {
            sidecar.label_weights.get(owner.typography_target())?;
            metrics_style
                .font_weight
                .as_deref()?
                .parse::<u16>()
                .ok()
                .filter(|weight| (1..=1000).contains(weight))
        });
        // SVG emission consumes only the prepared source projection and wrapped rows. Final bbox
        // style and width-mode differences therefore must not trigger a second tokenize/wrap pass.
        // Exact metric reuse remains separately guarded by `prepared_metrics`.
        let binding = FlowchartSvgLabelWrapBindingRequest::for_measurer(
            measurer,
            wrap_style,
            max_width_px,
            break_long_words,
        );
        if let Some((source, measured)) =
            sidecar
                .zip(owner)
                .zip(binding.as_ref())
                .and_then(|((sidecar, owner), binding)| {
                    sidecar.prepared_wrapping(owner, raw_source, binding)
                })
        {
            Self::Prepared {
                source,
                measured,
                font_weight,
            }
        } else {
            let source = sidecar
                .zip(owner)
                .and_then(|(sidecar, owner)| sidecar.source(owner, raw_source))
                .map_or_else(
                    || Cow::Owned(FlowchartSvgLabelSource::new(raw_source)),
                    Cow::Borrowed,
                );
            Self::Source {
                source,
                measurer,
                style: wrap_style,
                font_weight,
                max_width_px,
                break_long_words,
            }
        }
    }

    pub(crate) fn plain_text(&self) -> &str {
        match self {
            Self::Prepared { source, .. } => source.plain_text(),
            Self::Source { source, .. } => source.plain_text(),
        }
    }

    pub(crate) const fn is_prepared(&self) -> bool {
        matches!(self, Self::Prepared { .. })
    }

    pub(crate) fn font_weight(&self) -> Option<u16> {
        match self {
            Self::Prepared { font_weight, .. } | Self::Source { font_weight, .. } => *font_weight,
        }
    }

    pub(crate) fn merge_emission_font_style(&self, existing: Option<&str>) -> Option<String> {
        if let Some(weight) = self.font_weight() {
            use std::fmt::Write as _;
            let mut style = existing.unwrap_or_default().to_owned();
            let _ = write!(style, ";font-weight:{weight} !important;");
            Some(style)
        } else {
            None
        }
    }

    pub(crate) fn wrapped_lines(&self) -> Cow<'_, [Vec<String>]> {
        match self {
            Self::Prepared { measured, .. } => Cow::Borrowed(measured.wrapped_lines()),
            Self::Source {
                source,
                measurer,
                style,
                max_width_px,
                break_long_words,
                ..
            } => {
                Cow::Owned(source.wrapped_lines(*measurer, style, *max_width_px, *break_long_words))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::environment::{RenderEnvironment, TextMeasurementPhase};

    use crate::math::MathRenderer;

    use crate::text::{TextMeasurer, TextMetrics, TextStyle, WrapMode};
    use merman_model::presentation::PresentationConfig;

    use super::*;

    #[test]
    fn recovered_html_is_prepared_once_borrowed_by_terminal_and_releases_retained_bytes() {
        let work = Arc::new(OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let builder = FlowchartSvgLabelSidecarBuilder::new_with_work_meter(None, Arc::clone(&work));
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let config = crate::test_support::presentation_config(
            serde_json::json!({ "securityLevel": "loose" }),
        );
        let style = TextStyle::default();
        let owner = FlowchartSvgLabelOwner::Node(0);
        let source = "<b><i>API</b> tail</i>";
        let request = || FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: source,
            label_type: "text",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &config,
            math_renderer: None,
        };
        let first =
            builder.measure_for_layout(owner, "api", request(), true, FlowchartSvgWidthMode::Bbox);
        let parse_work = work.used();
        let retained = work.prepared_text_retained_bytes();
        let second =
            builder.measure_for_layout(owner, "api", request(), true, FlowchartSvgWidthMode::Bbox);
        assert_eq!(
            (first.width, first.height, first.line_count),
            (second.width, second.height, second.line_count)
        );
        assert_eq!(
            work.used(),
            parse_work,
            "repeated measurement reuses recovered XHTML"
        );
        assert_eq!(work.prepared_text_retained_bytes(), retained);

        let sidecar = builder.finish();
        let FlowchartPreparedLabelResolution::PreparedHtml(xhtml) =
            sidecar.prepared_label_for_terminal(owner, source)
        else {
            panic!("HTML label must be prepared before terminal emission")
        };
        let wrapped = format!("<root>{xhtml}</root>");
        let document = roxmltree::Document::parse(&wrapped).expect("prepared label is valid XHTML");
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.has_tag_name("i"))
                .count(),
            2
        );
        assert_eq!(
            work.used(),
            parse_work,
            "terminal lookup does not parse HTML"
        );
        drop(sidecar);
        assert_eq!(work.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn html_preparation_records_parser_work_rejection_and_cancellation() {
        let config = crate::test_support::presentation_config(
            serde_json::json!({ "securityLevel": "loose" }),
        );
        let style = TextStyle::default();
        let measurer = crate::text::DeterministicTextMeasurer::default();
        for cancel in [false, true] {
            let control = merman_model::OperationControl::new();
            let policy = if cancel {
                control.cancel_after_checkpoints(16);
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
            } else {
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(crate::resources::ResourceLimitId::MaxLayoutWorkUnits, 1)
                    .unwrap()
            };
            let work = Arc::new(OperationWorkMeter::new_with_control(policy, control));
            let builder =
                FlowchartSvgLabelSidecarBuilder::new_with_work_meter(None, Arc::clone(&work));
            builder.measure_for_layout(
                FlowchartSvgLabelOwner::Node(0),
                "api",
                FlowchartLabelMetricsRequest {
                    measurer: &measurer,
                    raw_label: "<b><i>API</b> tail</i>",
                    label_type: "text",
                    style: &style,
                    max_width_px: Some(200.0),
                    wrap_mode: WrapMode::HtmlLike,
                    config: &config,
                    math_renderer: None,
                },
                true,
                FlowchartSvgWidthMode::Bbox,
            );
            let sidecar = builder.finish();
            if cancel {
                assert!(matches!(
                    sidecar.prepared_work_error(),
                    Some(OperationWorkError::Cancelled(_))
                ));
            } else {
                assert_eq!(
                    sidecar.prepared_resource_error().unwrap().limit,
                    "max_layout_work_units"
                );
            }
            drop(sidecar);
            assert_eq!(work.prepared_text_retained_bytes(), 0);
        }
    }

    #[test]
    fn source_typography_fallback_reserves_and_releases_its_owned_fonts() {
        use crate::resources::{RenderResourcePolicy, ResourceLimitId};
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 1)
            .unwrap();
        let limited = Arc::new(OperationWorkMeter::new(policy));
        let style = TextStyle {
            font_family: Some("Caller Font".into()),
            ..TextStyle::default()
        };
        let error = FlowchartNodeSourceTypography::from_style(style.clone(), &limited).unwrap_err();
        assert_eq!(error.limit, "max_prepared_text_retained_bytes");
        assert_eq!(limited.prepared_text_retained_bytes(), 0);
        let work = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let projection = Arc::new(FlowchartNodeSourceTypography::from_style(style, &work).unwrap());
        assert!(work.prepared_text_retained_bytes() > 0);
        let terminal = Arc::clone(&projection);
        drop(projection);
        assert!(work.prepared_text_retained_bytes() > 0);
        drop(terminal);
        assert_eq!(work.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn render_id_budget_is_single_owned_across_source_and_math() {
        use crate::resources::RenderResourcePolicy;
        fn retained_for(render_id: &str, math: bool, owner: FlowchartSvgLabelOwner) -> usize {
            let work = Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input(),
            ));
            let config = PresentationConfig::default();
            let builder =
                FlowchartSvgLabelSidecarBuilder::new_with_work_meter(None, Arc::clone(&work));
            let style = TextStyle::default();
            let environment = RenderEnvironment::deterministic();
            let session = environment.begin_session().unwrap();
            let measurer = session.text_measurer(TextMeasurementPhase::Layout);
            builder.measure_for_layout(
                owner,
                render_id,
                FlowchartLabelMetricsRequest {
                    measurer: &measurer,
                    raw_label: if math { "$$x$$" } else { "label" },
                    label_type: "text",
                    style: &style,
                    max_width_px: None,
                    wrap_mode: if math {
                        WrapMode::HtmlLike
                    } else {
                        WrapMode::SvgLike
                    },
                    config: &config,
                    math_renderer: None,
                },
                true,
                FlowchartSvgWidthMode::Bbox,
            );
            if !math {
                // Some SVG owner kinds prepare no source artifact. Establish the
                // terminal binding before checking that repeated registration is free.
                assert!(builder.record_render_id(owner, render_id));
                let retained = work.prepared_text_retained_bytes();
                assert!(builder.record_render_id(owner, render_id));
                assert_eq!(work.prepared_text_retained_bytes(), retained);
                return retained;
            }
            assert!(builder.pending.borrow().math.get(owner).is_some());
            work.prepared_text_retained_bytes()
        }
        for owner in [
            FlowchartSvgLabelOwner::Node(0),
            FlowchartSvgLabelOwner::SubgraphTitle(0),
            FlowchartSvgLabelOwner::SwimlaneGroupTitle(0),
            FlowchartSvgLabelOwner::Edge(0),
            FlowchartSvgLabelOwner::SwimlaneEdgeLabel(0),
        ] {
            for math in [false, true] {
                assert_eq!(
                    retained_for("abcdefghij", math, owner) - retained_for("a", math, owner),
                    9
                );
            }
        }
    }

    #[test]
    fn node_source_typography_survives_non_svg_measurement_paths() {
        let config = PresentationConfig::default();
        let measurer = StatefulOpaqueTraceMeasurer::new();
        let style = TextStyle {
            font_family: Some("Source Sans".to_owned()),
            font_size: 23.0,
            font_weight: Some("650".to_owned()),
            font_style: Some("italic".to_owned()),
        };
        for (owner, label_type, wrap_mode, swimlane) in [
            (
                FlowchartSvgLabelOwner::Node(2),
                "html",
                WrapMode::HtmlLike,
                false,
            ),
            (
                FlowchartSvgLabelOwner::EmptySubgraphNode(1),
                "markdown",
                WrapMode::SvgLike,
                false,
            ),
            (
                FlowchartSvgLabelOwner::SwimlaneNode(3),
                "text",
                WrapMode::SvgLike,
                true,
            ),
        ] {
            let builder = FlowchartSvgLabelSidecarBuilder::default();
            builder.measure_for_layout(
                owner,
                "source-node",
                FlowchartLabelMetricsRequest {
                    measurer: &measurer,
                    raw_label: "Source label",
                    label_type,
                    style: &style,
                    max_width_px: Some(80.0),
                    wrap_mode,
                    config: &config,
                    math_renderer: None,
                },
                true,
                FlowchartSvgWidthMode::Bbox,
            );
            let sidecar = builder.finish();
            assert_eq!(sidecar.node_owner("source-node", swimlane), Some(owner));
            let source = Arc::clone(sidecar.node_source_style(owner).expect("source typography"));
            drop(sidecar);
            assert_eq!(source.font_family, style.font_family);
            assert_eq!(source.font_size, style.font_size);
            assert_eq!(source.font_weight, style.font_weight);
            assert_eq!(source.font_style, style.font_style);
        }
    }

    #[test]
    fn cluster_source_typography_keeps_first_owner_on_repeated_measurement() {
        let config = PresentationConfig::default();
        let measurer = StatefulOpaqueTraceMeasurer::new();
        for owner in [
            FlowchartSvgLabelOwner::SubgraphTitle(0),
            FlowchartSvgLabelOwner::SwimlaneGroupTitle(0),
        ] {
            let builder = FlowchartSvgLabelSidecarBuilder::default();
            for (render_id, size) in [("first", 19.0), ("second", 27.0)] {
                let style = TextStyle {
                    font_size: size,
                    ..TextStyle::default()
                };
                builder.measure_for_layout(
                    owner,
                    render_id,
                    FlowchartLabelMetricsRequest {
                        measurer: &measurer,
                        raw_label: "Title",
                        label_type: "html",
                        style: &style,
                        max_width_px: None,
                        wrap_mode: WrapMode::HtmlLike,
                        config: &config,
                        math_renderer: None,
                    },
                    true,
                    FlowchartSvgWidthMode::Bbox,
                );
            }
            let sidecar = builder.finish();
            assert_eq!(sidecar.node_source_style(owner).unwrap().font_size, 19.0);
        }
    }

    #[test]
    fn host_label_shadow_allocation_scales_with_line_width() {
        let style = TextStyle {
            font_size: 14.0,
            ..TextStyle::default()
        };
        let environment = RenderEnvironment::deterministic();
        let session = environment.begin_session().unwrap();
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let binding = FlowchartSvgLabelBindingRequest::for_measurer(
            &measurer,
            &style,
            &style,
            None,
            true,
            FlowchartSvgWidthMode::ComputedLength,
        )
        .unwrap()
        .into_owned();
        let label = PreparedFlowchartSvgLabel::new(
            binding,
            vec![vec!["Browse Products".to_owned()]],
            TextMetrics {
                width: 98.56,
                height: 15.4,
                line_count: 1,
            },
        );
        let bounds = label.centered_shadow_bounds(&style).unwrap();
        // DejaVu Sans Bold ink plus the Cyberpunk halo crosses the old one-em allocation.
        assert!(bounds[0] - 20.0 <= -85.177246);
        assert!(bounds[2] + 20.0 >= 85.79932);
        assert!(
            label
                .centered_shadow_bounds(&TextStyle {
                    font_size: 16.0,
                    ..style
                })
                .is_none()
        );
    }

    fn report_call_count(session: &crate::environment::RenderSession) -> u64 {
        session
            .text_measurement_report()
            .entries()
            .iter()
            .map(crate::environment::TextMeasurementSummary::count)
            .sum()
    }

    #[derive(Debug)]
    struct CountingPreparedMathRenderer {
        render_calls: Arc<AtomicUsize>,
        measure_calls: Arc<AtomicUsize>,
    }

    impl MathRenderer for CountingPreparedMathRenderer {
        fn render_html_label(&self, text: &str, _config: &PresentationConfig) -> Option<String> {
            self.render_calls.fetch_add(1, Ordering::SeqCst);
            Some(format!("<span class=\"prepared-math\">{text}</span>"))
        }

        fn measure_html_label(
            &self,
            _text: &str,
            _config: &PresentationConfig,
            _style: &TextStyle,
            _max_width_px: Option<f64>,
            _wrap_mode: WrapMode,
        ) -> Option<TextMetrics> {
            self.measure_calls.fetch_add(1, Ordering::SeqCst);
            Some(TextMetrics {
                width: 42.0,
                height: 18.0,
                line_count: 1,
            })
        }
    }

    #[derive(Debug, Default)]
    struct StyleDependentPreparedMathRenderer {
        available: std::sync::atomic::AtomicBool,
    }

    impl MathRenderer for StyleDependentPreparedMathRenderer {
        fn render_html_label(&self, _text: &str, _config: &PresentationConfig) -> Option<String> {
            self.available
                .load(Ordering::SeqCst)
                .then(|| "<b>math</b>".into())
        }

        fn measure_html_label(
            &self,
            _text: &str,
            _config: &PresentationConfig,
            style: &TextStyle,
            _max_width_px: Option<f64>,
            _wrap_mode: WrapMode,
        ) -> Option<TextMetrics> {
            self.available
                .store(style.font_size >= 24.0, Ordering::SeqCst);
            Some(TextMetrics {
                width: 42.0,
                height: 18.0,
                line_count: 1,
            })
        }
    }

    #[test]
    fn prepared_math_replaces_an_earlier_html_fallback_after_style_changes() {
        let backend = ConfiguredMathBackend::external(Arc::new(
            StyleDependentPreparedMathRenderer::default(),
        ));
        let config = crate::test_support::presentation_config(
            serde_json::json!({ "securityLevel": "loose" }),
        );
        let builder =
            FlowchartSvgLabelSidecarBuilder::default().with_math_backend(Some(&backend), &config);
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let owner = FlowchartSvgLabelOwner::Node(0);
        for font_size in [16.0, 24.0] {
            let style = TextStyle {
                font_size,
                ..TextStyle::default()
            };
            builder.measure_for_layout(
                owner,
                "math",
                FlowchartLabelMetricsRequest {
                    measurer: &measurer,
                    raw_label: "$$x$$",
                    label_type: "text",
                    style: &style,
                    max_width_px: Some(200.0),
                    wrap_mode: WrapMode::HtmlLike,
                    config: &config,
                    math_renderer: None,
                },
                true,
                FlowchartSvgWidthMode::Bbox,
            );
            if font_size == 16.0 {
                assert!(builder.pending.borrow().html.get(owner).is_some());
            }
        }
        let sidecar = builder.finish();
        let FlowchartPreparedLabelResolution::PreparedMath(prepared) =
            sidecar.prepared_label_for_terminal(owner, "$$x$$")
        else {
            panic!("successful math preparation must take precedence over an older HTML fallback")
        };
        assert!(prepared.browser_xhtml().contains("font-size:24px"));
        assert!(prepared.browser_xhtml().contains("<b>math</b>"));
    }

    #[test]
    fn flowchart_math_occurrence_reuses_the_layout_artifact_for_terminal_emission() {
        let render_calls = Arc::new(AtomicUsize::new(0));
        let measure_calls = Arc::new(AtomicUsize::new(0));
        let backend = ConfiguredMathBackend::external(Arc::new(CountingPreparedMathRenderer {
            render_calls: Arc::clone(&render_calls),
            measure_calls: Arc::clone(&measure_calls),
        }));
        let config = crate::test_support::presentation_config(serde_json::json!({
            "themeVariables": { "nodeTextColor": "#e5e7eb" }
        }));
        let builder =
            FlowchartSvgLabelSidecarBuilder::default().with_math_backend(Some(&backend), &config);
        let style = TextStyle {
            font_size: 24.0,
            ..TextStyle::default()
        };
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let request = |max_width_px| FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: "$$x^2$$",
            label_type: "text",
            style: &style,
            max_width_px,
            wrap_mode: WrapMode::HtmlLike,
            config: &config,
            math_renderer: None,
        };
        let owner = FlowchartSvgLabelOwner::Node(0);

        let first = builder.measure_for_layout(
            owner,
            "node-a",
            request(Some(20.0)),
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        let layout_arc = {
            let pending = builder.pending.borrow();
            let MathPreparationOutcome::Prepared(layout_arc) =
                &pending.math.get(owner).expect("layout artifact").outcome
            else {
                panic!("prepared layout artifact");
            };
            Arc::clone(layout_arc)
        };
        let sidecar = builder.finish();
        let resolved_owner = sidecar.node_owner("node-a", false).expect("node owner");
        let prepared = sidecar
            .prepared_math_arc(resolved_owner, "$$x^2$$")
            .expect("prepared occurrence");

        assert_eq!(first.width, 42.0);
        assert!(Arc::ptr_eq(&layout_arc, prepared));
        assert!(prepared.browser_xhtml().contains("color:#e5e7eb"));
        assert!(prepared.browser_xhtml().contains("font-size:24px"));
        assert_eq!(render_calls.load(Ordering::SeqCst), 1);
        assert_eq!(measure_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn declined_flowchart_math_terminal_is_retained_as_native_unavailable_evidence() {
        let backend = ConfiguredMathBackend::external(Arc::new(crate::math::NoopMathRenderer));
        let config = PresentationConfig::default();
        let builder =
            FlowchartSvgLabelSidecarBuilder::default().with_math_backend(Some(&backend), &config);
        let work_meter = Arc::clone(
            builder
                .work_meter
                .as_ref()
                .expect("math sidecar operation work meter"),
        );
        let style = TextStyle::default();
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let owner = FlowchartSvgLabelOwner::Node(0);

        builder.measure_for_layout(
            owner,
            "node-a",
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label: "$$x$$",
                label_type: "text",
                style: &style,
                max_width_px: Some(200.0),
                wrap_mode: WrapMode::HtmlLike,
                config: &config,
                math_renderer: None,
            },
            true,
            FlowchartSvgWidthMode::Bbox,
        );

        let sidecar = builder.finish();
        assert!(matches!(
            sidecar.prepared_math(owner, "$$x$$"),
            FlowchartPreparedLabelResolution::Unavailable(
                MathPreparationUnavailable::BackendDeclined
            )
        ));
        let evidence = sidecar.prepared_math_evidence();
        assert_eq!(evidence.entries().len(), 1);
        assert_eq!(evidence.entries()[0].expected_emissions(), 1);
        assert!(evidence.entries()[0].projection_fingerprint().is_none());
        drop(sidecar);
        assert!(work_meter.prepared_text_retained_bytes() > 0);
        drop(evidence);
        assert_eq!(work_meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn svg_like_flowchart_math_keeps_the_legacy_plain_measurement_path() {
        let render_calls = Arc::new(AtomicUsize::new(0));
        let measure_calls = Arc::new(AtomicUsize::new(0));
        let renderer = Arc::new(CountingPreparedMathRenderer {
            render_calls: Arc::clone(&render_calls),
            measure_calls: Arc::clone(&measure_calls),
        });
        let backend = ConfiguredMathBackend::external(renderer.clone());
        let config = PresentationConfig::default();
        let builder =
            FlowchartSvgLabelSidecarBuilder::default().with_math_backend(Some(&backend), &config);
        let style = TextStyle::default();
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let owner = FlowchartSvgLabelOwner::Node(0);

        let metrics = builder.measure_for_layout(
            owner,
            "node-a",
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label: "$$x^2$$",
                label_type: "text",
                style: &style,
                max_width_px: Some(200.0),
                wrap_mode: WrapMode::SvgLike,
                config: &config,
                math_renderer: Some(renderer.as_ref()),
            },
            true,
            FlowchartSvgWidthMode::Bbox,
        );

        assert!(metrics.width > 0.0);
        assert_eq!(render_calls.load(Ordering::SeqCst), 0);
        assert_eq!(measure_calls.load(Ordering::SeqCst), 0);
        assert_eq!(builder.pending.borrow().math.len(), 0);
        let sidecar = builder.finish();
        assert!(matches!(
            sidecar.prepared_math(owner, "$$x^2$$"),
            FlowchartPreparedLabelResolution::NotPrepared
        ));
    }

    #[test]
    fn flowchart_math_occurrence_uses_the_resolved_source_foreground() {
        let backend = ConfiguredMathBackend::external(Arc::new(CountingPreparedMathRenderer {
            render_calls: Arc::new(AtomicUsize::new(0)),
            measure_calls: Arc::new(AtomicUsize::new(0)),
        }));
        let config = crate::test_support::presentation_config(serde_json::json!({
            "themeVariables": { "nodeTextColor": "#111827" }
        }));
        let builder =
            FlowchartSvgLabelSidecarBuilder::default().with_math_backend(Some(&backend), &config);
        let style = TextStyle::default();
        let terminal_foreground = super::super::FlowchartTerminalForeground::new(
            "#f43f5e",
            super::super::FlowchartTerminalForegroundProvenance::InlineStyle,
        );
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let owner = FlowchartSvgLabelOwner::Node(0);

        builder.measure_for_layout_with_terminal_foreground(
            owner,
            "node-a",
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label: "$$x^2$$",
                label_type: "text",
                style: &style,
                max_width_px: Some(200.0),
                wrap_mode: WrapMode::HtmlLike,
                config: &config,
                math_renderer: None,
            },
            Some(&terminal_foreground),
            true,
            FlowchartSvgWidthMode::Bbox,
        );

        assert_eq!(
            builder
                .pending
                .borrow()
                .math
                .get(owner)
                .expect("source-colored occurrence")
                .foreground
                .provenance(),
            super::super::FlowchartTerminalForegroundProvenance::InlineStyle
        );

        let sidecar = builder.finish();
        let FlowchartPreparedLabelResolution::PreparedMath(prepared) =
            sidecar.prepared_math(owner, "$$x^2$$")
        else {
            panic!("prepared source-colored math occurrence");
        };
        assert!(prepared.browser_xhtml().contains("color:#f43f5e"));
    }

    #[derive(Debug, Clone, PartialEq)]
    enum OpaqueMeasurementCall {
        Measure {
            text: String,
            font_size_bits: u64,
        },
        ComputedLength {
            text: String,
            font_size_bits: u64,
        },
        Wrapped {
            text: String,
            font_size_bits: u64,
            max_width_bits: Option<u64>,
            wrap_mode: WrapMode,
        },
    }

    struct StatefulOpaqueTraceMeasurer {
        calls: RefCell<Vec<OpaqueMeasurementCall>>,
    }

    impl StatefulOpaqueTraceMeasurer {
        fn new() -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
            }
        }

        fn stateful_width(&self, text: &str, style: &TextStyle) -> f64 {
            text.chars().count() as f64 * style.font_size * 0.5
                + self.calls.borrow().len() as f64 * 0.125
        }

        fn snapshot(&self) -> Vec<OpaqueMeasurementCall> {
            self.calls.borrow().clone()
        }
    }

    impl TextMeasurer for StatefulOpaqueTraceMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.calls
                .borrow_mut()
                .push(OpaqueMeasurementCall::Measure {
                    text: text.to_string(),
                    font_size_bits: style.font_size.to_bits(),
                });
            TextMetrics {
                width: self.stateful_width(text, style),
                height: style.font_size,
                line_count: 1,
            }
        }

        fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.calls
                .borrow_mut()
                .push(OpaqueMeasurementCall::ComputedLength {
                    text: text.to_string(),
                    font_size_bits: style.font_size.to_bits(),
                });
            self.stateful_width(text, style)
        }

        fn measure_wrapped(
            &self,
            text: &str,
            style: &TextStyle,
            max_width: Option<f64>,
            wrap_mode: WrapMode,
        ) -> TextMetrics {
            self.calls
                .borrow_mut()
                .push(OpaqueMeasurementCall::Wrapped {
                    text: text.to_string(),
                    font_size_bits: style.font_size.to_bits(),
                    max_width_bits: max_width.map(f64::to_bits),
                    wrap_mode,
                });
            let width = self.stateful_width(text, style);
            TextMetrics {
                width: max_width.map_or(width, |max_width| width.min(max_width)),
                height: text.lines().count().max(1) as f64 * style.font_size,
                line_count: text.lines().count().max(1),
            }
        }
    }

    #[test]
    fn prepared_math_retention_counts_the_terminal_render_identity_and_owner_slot() {
        let style = TextStyle {
            font_family: Some("Catalog Sans".to_string()),
            ..TextStyle::default()
        };
        let foreground = super::super::FlowchartTerminalForeground::new(
            "#334155",
            super::super::FlowchartTerminalForegroundProvenance::ThemeNode,
        );
        let outcome =
            MathPreparationOutcome::Unavailable(MathPreparationUnavailable::BackendDeclined);
        let occurrence_id = FlowchartSvgLabelOwner::Node(0).prepared_math_occurrence_id();
        let short = PreparedFlowchartMathLabel::retained_bytes_without_reservation(
            "$$x$$",
            &style,
            &foreground,
            "n",
            &occurrence_id,
            &outcome,
        );
        let long_id = "node-".repeat(256);
        let long = PreparedFlowchartMathLabel::retained_bytes_without_reservation(
            "$$x$$",
            &style,
            &foreground,
            &long_id,
            &occurrence_id,
            &outcome,
        );

        assert_eq!(long - short, long_id.len() - 1);
        assert!(
            short
                > std::mem::size_of::<PreparedFlowchartMathLabel>()
                    + FLOWCHART_RENDER_ID_RECORD_BYTES
                    + FLOWCHART_PREPARED_OWNER_SLOT_BYTES
                    + "$$x$$".len()
                    + "Catalog Sans".len()
                    + foreground.value().len()
                    + occurrence_id.as_str().len()
        );
    }

    #[test]
    fn plain_swimlane_markdown_preparation_keeps_computed_length_geometry() {
        struct PhaseSensitiveMeasurer {
            computed_calls: Cell<usize>,
        }

        impl TextMeasurer for PhaseSensitiveMeasurer {
            fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
                self.measure_wrapped(text, style, None, WrapMode::SvgLike)
            }

            fn measure_svg_text_computed_length_px(&self, text: &str, _style: &TextStyle) -> f64 {
                self.computed_calls.set(self.computed_calls.get() + 1);
                text.chars().count() as f64 * 6.0 + 0.122
            }

            fn measure_wrapped(
                &self,
                text: &str,
                style: &TextStyle,
                _max_width: Option<f64>,
                _wrap_mode: WrapMode,
            ) -> TextMetrics {
                let line_count = text.lines().count().max(1);
                TextMetrics {
                    width: 999.0,
                    height: line_count as f64 * style.font_size,
                    line_count,
                }
            }
        }

        let measurer = PhaseSensitiveMeasurer {
            computed_calls: Cell::new(0),
        };
        let config = PresentationConfig::default();
        let style = TextStyle::default();
        for (raw_label, expected_width, line_count) in [
            ("Outer lane", 60.125, 1),
            ("Outer &amp; &lt; lane", 84.125, 1),
            ("Outer lane\nInner lane", 60.125, 2),
        ] {
            let request = || FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label,
                label_type: "markdown",
                style: &style,
                max_width_px: Some(600.0),
                wrap_mode: WrapMode::SvgLike,
                config: &config,
                math_renderer: None,
            };
            let legacy = flowchart_label_metrics_for_layout(request());
            assert_eq!(legacy.width, expected_width);
            assert_eq!(legacy.line_count, line_count);

            let builder = FlowchartSvgLabelSidecarBuilder::default();
            let owner = FlowchartSvgLabelOwner::SwimlaneGroupTitle(0);
            // Exercise both creation and reuse of the source rows with an opaque host measurer.
            for _ in 0..2 {
                measurer.computed_calls.set(0);
                let prepared = builder.measure_for_layout(
                    owner,
                    "lane",
                    request(),
                    true,
                    FlowchartSvgWidthMode::Bbox,
                );
                assert_eq!(prepared.width, legacy.width);
                assert_eq!(prepared.height, legacy.height);
                assert_eq!(prepared.line_count, legacy.line_count);
                assert!(measurer.computed_calls.get() > 0);
            }
            let sidecar = builder.finish();
            assert_eq!(sidecar.source_plan_count(owner), 1);
        }
    }

    fn opaque_roundtrip_trace(
        with_sidecar: bool,
    ) -> (TextMetrics, Vec<Vec<String>>, Vec<OpaqueMeasurementCall>) {
        let measurer = StatefulOpaqueTraceMeasurer::new();
        let config = PresentationConfig::default();
        let style = TextStyle::default();
        let builder = with_sidecar.then(FlowchartSvgLabelSidecarBuilder::default);
        let owner = FlowchartSvgLabelOwner::Node(0);
        let raw_label = "alpha $$x$$ beta gamma delta";
        let metrics = measure_flowchart_svg_label_for_layout(
            builder.as_ref(),
            with_sidecar.then_some(owner),
            with_sidecar.then_some("node"),
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label,
                label_type: "text",
                style: &style,
                max_width_px: Some(96.0),
                wrap_mode: WrapMode::SvgLike,
                config: &config,
                math_renderer: Some(&crate::math::NoopMathRenderer),
            },
            FlowchartSvgWidthMode::ComputedLength,
        );

        let sidecar = builder.map(FlowchartSvgLabelSidecarBuilder::finish);
        let resolved_owner = sidecar
            .as_ref()
            .and_then(|sidecar| sidecar.node_owner("node", false));
        let plan = FlowchartSvgLabelRenderPlan::new(
            sidecar.as_ref(),
            resolved_owner,
            raw_label,
            &measurer,
            &style,
            Some(96.0),
            true,
            FlowchartSvgWidthMode::ComputedLength,
        );
        let wrapped_lines = plan.wrapped_lines().into_owned();
        (metrics, wrapped_lines, measurer.snapshot())
    }

    #[test]
    fn opaque_stateful_trace_is_identical_with_and_without_the_sidecar() {
        let (control_metrics, control_lines, control_trace) = opaque_roundtrip_trace(false);
        let (sidecar_metrics, sidecar_lines, sidecar_trace) = opaque_roundtrip_trace(true);

        assert_eq!(
            sidecar_metrics.width.to_bits(),
            control_metrics.width.to_bits()
        );
        assert_eq!(
            sidecar_metrics.height.to_bits(),
            control_metrics.height.to_bits()
        );
        assert_eq!(sidecar_metrics.line_count, control_metrics.line_count);
        assert_eq!(sidecar_lines, control_lines);
        assert_eq!(sidecar_trace, control_trace);
    }

    #[test]
    fn routed_builtin_measurement_is_reused_during_svg_emission() {
        let environment = RenderEnvironment::deterministic();
        let session = environment.begin_session().expect("deterministic session");
        let layout_measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let config = PresentationConfig::default();
        let style = TextStyle::default();
        let builder = FlowchartSvgLabelSidecarBuilder::default();
        let owner = FlowchartSvgLabelOwner::SubgraphTitle(0);

        let metrics = builder.measure_for_layout(
            owner,
            "group",
            FlowchartLabelMetricsRequest {
                measurer: &layout_measurer,
                raw_label: "alpha beta gamma",
                label_type: "text",
                style: &style,
                max_width_px: Some(60.0),
                wrap_mode: WrapMode::SvgLike,
                config: &config,
                math_renderer: None,
            },
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        assert!(metrics.width > 0.0);
        assert_eq!(builder.prepared_count(), 1);

        let sidecar = builder.finish();
        let resolved_owner = sidecar.subgraph_title_owner("group");
        assert_eq!(resolved_owner, Some(owner));
        let calls_before_render = report_call_count(&session);
        let render_measurer = session.text_measurer(TextMeasurementPhase::SvgBBox);
        let plan = FlowchartSvgLabelRenderPlan::new(
            Some(&sidecar),
            resolved_owner,
            "alpha beta gamma",
            &render_measurer,
            &style,
            Some(60.0),
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        assert!(matches!(plan, FlowchartSvgLabelRenderPlan::Prepared { .. }));
        assert!(!plan.wrapped_lines().is_empty());
        assert_eq!(report_call_count(&session), calls_before_render);

        let reused_metrics = sidecar
            .prepared_metrics(
                owner,
                "alpha beta gamma",
                &render_measurer,
                &style,
                Some(60.0),
                true,
                FlowchartSvgWidthMode::Bbox,
            )
            .expect("prepared metrics");
        assert_eq!(reused_metrics.width.to_bits(), metrics.width.to_bits());
        assert_eq!(reused_metrics.height.to_bits(), metrics.height.to_bits());
        assert_eq!(reused_metrics.line_count, metrics.line_count);
        assert_eq!(report_call_count(&session), calls_before_render);
    }

    #[test]
    fn svg_math_source_keeps_computed_length_preparation() {
        let environment = RenderEnvironment::deterministic();
        let session = environment.begin_session().expect("deterministic session");
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let config = PresentationConfig::default();
        let style = TextStyle::default();
        let builder = FlowchartSvgLabelSidecarBuilder::default();
        let owner = FlowchartSvgLabelOwner::Node(0);

        builder.measure_for_layout(
            owner,
            "node",
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label: "left $$x$$ right",
                label_type: "text",
                style: &style,
                max_width_px: Some(120.0),
                wrap_mode: WrapMode::SvgLike,
                config: &config,
                math_renderer: Some(&crate::math::NoopMathRenderer),
            },
            true,
            FlowchartSvgWidthMode::ComputedLength,
        );
        assert_eq!(builder.prepared_count(), 1);

        let sidecar = builder.finish();
        assert!(matches!(
            FlowchartSvgLabelRenderPlan::new(
                Some(&sidecar),
                Some(owner),
                "left $$x$$ right",
                &measurer,
                &style,
                Some(120.0),
                true,
                FlowchartSvgWidthMode::ComputedLength,
            ),
            FlowchartSvgLabelRenderPlan::Prepared { .. }
        ));
        assert!(matches!(
            FlowchartSvgLabelRenderPlan::new(
                Some(&sidecar),
                Some(owner),
                "left $$x$$ right",
                &measurer,
                &style,
                Some(120.0),
                true,
                FlowchartSvgWidthMode::Bbox,
            ),
            FlowchartSvgLabelRenderPlan::Prepared { .. }
        ));
        assert!(
            sidecar
                .prepared_metrics(
                    owner,
                    "left $$x$$ right",
                    &measurer,
                    &style,
                    Some(120.0),
                    true,
                    FlowchartSvgWidthMode::Bbox,
                )
                .is_none(),
            "wrapped-row reuse must not weaken exact metric binding"
        );
    }

    #[test]
    fn prepared_wrapping_requires_an_exact_wrapping_binding() {
        let environment = RenderEnvironment::deterministic();
        let session = environment.begin_session().expect("deterministic session");
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let config = PresentationConfig::default();
        let style = TextStyle::default();
        let builder = FlowchartSvgLabelSidecarBuilder::default();
        let owner = FlowchartSvgLabelOwner::SubgraphTitle(0);
        builder.measure_for_layout(
            owner,
            "group",
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label: "bound label",
                label_type: "text",
                style: &style,
                max_width_px: Some(80.0),
                wrap_mode: WrapMode::SvgLike,
                config: &config,
                math_renderer: None,
            },
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        let sidecar = builder.finish();

        let width_mismatch = FlowchartSvgLabelRenderPlan::new(
            Some(&sidecar),
            Some(owner),
            "bound label",
            &measurer,
            &style,
            Some(81.0),
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        assert!(matches!(
            width_mismatch,
            FlowchartSvgLabelRenderPlan::Source { .. }
        ));

        let mut different_style = style.clone();
        different_style.font_size += 1.0;
        let style_mismatch = FlowchartSvgLabelRenderPlan::new(
            Some(&sidecar),
            Some(owner),
            "bound label",
            &measurer,
            &different_style,
            Some(80.0),
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        assert!(matches!(
            style_mismatch,
            FlowchartSvgLabelRenderPlan::Source { .. }
        ));

        let mode_mismatch = FlowchartSvgLabelRenderPlan::new(
            Some(&sidecar),
            Some(owner),
            "bound label",
            &measurer,
            &style,
            Some(80.0),
            true,
            FlowchartSvgWidthMode::ComputedLength,
        );
        assert!(matches!(
            mode_mismatch,
            FlowchartSvgLabelRenderPlan::Prepared { .. }
        ));
        assert!(
            sidecar
                .prepared_metrics(
                    owner,
                    "bound label",
                    &measurer,
                    &style,
                    Some(80.0),
                    true,
                    FlowchartSvgWidthMode::ComputedLength,
                )
                .is_none(),
            "metric reuse must retain exact width-mode binding"
        );
    }

    struct StreamingOpaqueMeasurer {
        wrapped_calls: Cell<usize>,
        literal_bbox_calls: Cell<usize>,
    }

    impl TextMeasurer for StreamingOpaqueMeasurer {
        #[allow(private_interfaces)]
        fn begin_svg_text_computed_length(
            &self,
            style: &TextStyle,
        ) -> Option<crate::environment::BuiltinSvgComputedLength> {
            Some(crate::environment::BuiltinSvgComputedLength::deterministic(
                style,
            ))
        }

        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: text.chars().count() as f64 * style.font_size * 0.5,
                height: style.font_size,
                line_count: 1,
            }
        }

        fn measure_svg_tspan_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.literal_bbox_calls
                .set(self.literal_bbox_calls.get() + 1);
            self.measure(text, style).width
        }

        fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.literal_bbox_calls
                .set(self.literal_bbox_calls.get() + 1);
            self.measure(text, style).height
        }

        fn measure_wrapped(
            &self,
            text: &str,
            style: &TextStyle,
            _max_width: Option<f64>,
            _wrap_mode: WrapMode,
        ) -> TextMetrics {
            self.wrapped_calls.set(self.wrapped_calls.get() + 1);
            self.measure(text, style)
        }
    }

    #[test]
    fn streaming_state_without_builtin_carriers_is_not_cached() {
        let measurer = StreamingOpaqueMeasurer {
            wrapped_calls: Cell::new(0),
            literal_bbox_calls: Cell::new(0),
        };
        let config = PresentationConfig::default();
        let style = TextStyle::default();
        let builder = FlowchartSvgLabelSidecarBuilder::default();
        let owner = FlowchartSvgLabelOwner::Node(0);
        let measure = || {
            builder.measure_for_layout(
                owner,
                "node",
                FlowchartLabelMetricsRequest {
                    measurer: &measurer,
                    raw_label: "opaque callback contract",
                    label_type: "text",
                    style: &style,
                    max_width_px: Some(100.0),
                    wrap_mode: WrapMode::SvgLike,
                    config: &config,
                    math_renderer: None,
                },
                true,
                FlowchartSvgWidthMode::Bbox,
            )
        };

        let first = measure();
        let first_bbox_calls = measurer.literal_bbox_calls.get();
        assert!(
            first_bbox_calls > 0,
            "layout must measure literal SVG row bounds"
        );
        let second = measure();
        assert_eq!(
            measurer.literal_bbox_calls.get(),
            2 * first_bbox_calls,
            "opaque row measurements must run again even with a computed-length streaming state"
        );
        assert_eq!(
            (first.width, first.height, first.line_count),
            (second.width, second.height, second.line_count)
        );
        assert_eq!(measurer.wrapped_calls.get(), 0);
        assert_eq!(builder.prepared_count(), 0);
        let sidecar = builder.finish();
        let calls_before_render = measurer.literal_bbox_calls.get();
        let plan = FlowchartSvgLabelRenderPlan::new(
            Some(&sidecar),
            Some(owner),
            "opaque callback contract",
            &measurer,
            &style,
            Some(100.0),
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        assert!(matches!(
            plan,
            FlowchartSvgLabelRenderPlan::Source {
                source: Cow::Borrowed(_),
                ..
            }
        ));
        assert!(plan.wrapped_lines().len() > 1);
        assert_eq!(
            measurer.literal_bbox_calls.get(),
            calls_before_render,
            "render wrapping uses computed length without measuring literal row bounds again"
        );
        assert_eq!(measurer.wrapped_calls.get(), 0);
    }

    #[test]
    fn duplicate_subgraph_ids_resolve_to_the_first_semantic_owner() {
        let environment = RenderEnvironment::deterministic();
        let session = environment.begin_session().expect("deterministic session");
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let config = PresentationConfig::default();
        let style = TextStyle::default();
        let builder = FlowchartSvgLabelSidecarBuilder::default();

        for (index, title) in ["first", "second"].into_iter().enumerate() {
            builder.measure_for_layout(
                FlowchartSvgLabelOwner::SubgraphTitle(index),
                "dup",
                FlowchartLabelMetricsRequest {
                    measurer: &measurer,
                    raw_label: title,
                    label_type: "text",
                    style: &style,
                    max_width_px: None,
                    wrap_mode: WrapMode::SvgLike,
                    config: &config,
                    math_renderer: None,
                },
                true,
                FlowchartSvgWidthMode::Bbox,
            );
        }

        let sidecar = builder.finish();
        assert_eq!(
            sidecar.subgraph_title_owner("dup"),
            Some(FlowchartSvgLabelOwner::SubgraphTitle(0))
        );
    }

    #[test]
    fn self_loop_render_id_resolves_to_the_original_edge_owner() {
        let environment = RenderEnvironment::deterministic();
        let session = environment.begin_session().expect("deterministic session");
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let config = PresentationConfig::default();
        let style = TextStyle::default();
        let builder = FlowchartSvgLabelSidecarBuilder::default();
        let owner = FlowchartSvgLabelOwner::Edge(3);

        builder.measure_for_layout(
            owner,
            "L_A_A_0",
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label: "self loop label",
                label_type: "text",
                style: &style,
                max_width_px: Some(200.0),
                wrap_mode: WrapMode::SvgLike,
                config: &config,
                math_renderer: None,
            },
            true,
            FlowchartSvgWidthMode::Bbox,
        );

        assert_eq!(builder.finish().edge_owner("L_A_A_0", false), Some(owner));
    }

    #[test]
    fn sparse_semantic_indices_retain_only_real_label_entries() {
        let environment = RenderEnvironment::deterministic();
        let session = environment.begin_session().expect("deterministic session");
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let config = PresentationConfig::default();
        let style = TextStyle::default();
        let builder = FlowchartSvgLabelSidecarBuilder::default();
        let owner = FlowchartSvgLabelOwner::Edge(100_000);

        builder.measure_for_layout(
            owner,
            "sparse-edge",
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label: "only the final semantic edge has a label",
                label_type: "text",
                style: &style,
                max_width_px: Some(120.0),
                wrap_mode: WrapMode::SvgLike,
                config: &config,
                math_renderer: None,
            },
            true,
            FlowchartSvgWidthMode::Bbox,
        );

        {
            let pending = builder.pending.borrow();
            assert_eq!(pending.sources.edges.entries.len(), 1);
            assert_eq!(pending.prepared.edges.entries.len(), 1);
            assert_eq!(pending.render_ids.edges.entries.len(), 1);
            assert_eq!(pending.prepared.len(), 1);
        }

        let sidecar = builder.finish();
        assert_eq!(sidecar.sources.edges.entries.len(), 1);
        assert_eq!(sidecar.prepared.edges.entries.len(), 1);
        assert_eq!(sidecar.edge_owner("sparse-edge", false), Some(owner));
    }

    #[test]
    fn edge_preparation_wraps_with_base_style_and_measures_with_final_style() {
        let environment = RenderEnvironment::deterministic();
        let session = environment.begin_session().expect("deterministic session");
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let config = PresentationConfig::default();
        let wrap_style = TextStyle::default();
        let mut metrics_style = wrap_style.clone();
        metrics_style.font_size *= 2.0;
        let raw_label = "alpha beta gamma delta epsilon zeta eta theta";
        let max_width = Some(96.0);
        let source = FlowchartSvgLabelSource::new(raw_label);
        let expected_lines = source.wrapped_lines(&measurer, &wrap_style, max_width, true);
        let prematurely_styled_lines =
            source.wrapped_lines(&measurer, &metrics_style, max_width, true);
        assert_ne!(expected_lines, prematurely_styled_lines);
        let expected_metrics = source.metrics_from_wrapped(
            &measurer,
            &metrics_style,
            &expected_lines,
            FlowchartSvgWidthMode::Bbox,
        );

        let builder = FlowchartSvgLabelSidecarBuilder::default();
        let owner = FlowchartSvgLabelOwner::Edge(0);
        let metrics = builder.measure_for_layout_with_metrics_style(
            owner,
            "edge",
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label,
                label_type: "text",
                style: &wrap_style,
                max_width_px: max_width,
                wrap_mode: WrapMode::SvgLike,
                config: &config,
                math_renderer: None,
            },
            &metrics_style,
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        assert_eq!(metrics.width.to_bits(), expected_metrics.width.to_bits());
        assert_eq!(metrics.height.to_bits(), expected_metrics.height.to_bits());
        assert_eq!(metrics.line_count, expected_metrics.line_count);

        let sidecar = builder.finish();
        let plan = FlowchartSvgLabelRenderPlan::new_with_metrics_style(
            Some(&sidecar),
            Some(owner),
            raw_label,
            &measurer,
            &wrap_style,
            &metrics_style,
            max_width,
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        assert!(matches!(plan, FlowchartSvgLabelRenderPlan::Prepared { .. }));
        assert_eq!(plan.wrapped_lines().as_ref(), expected_lines.as_slice());
    }
}
