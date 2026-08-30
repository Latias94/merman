//! Operation-local preparation for non-Markdown Flowchart SVG labels.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex;

use rustc_hash::FxHashMap;

use crate::diagram_theme::{
    ResolvedDiagramTheme, TextTransform as ThemeTextTransform, ThemeTarget, ThemeTextStyle,
    ThemeVariant,
};
use crate::environment::{BuiltinTextMeasurementOperationCarrier, TextMeasurementOperation};
use crate::math::{
    ConfiguredMathBackend, MathPreparationOutcome, MathPreparationUnavailable,
    PrepareMathLabelRequest, PreparedMathEvidenceLease, PreparedMathLabel,
    PreparedMathOccurrenceId,
};
use crate::resources::{
    OperationWorkError, OperationWorkMeter, PreparedTextRetainedReservation, ResourceLimitExceeded,
};
use crate::text::{
    CatalogAdmittedTextStyle, PendingPreparedTextLabelLedgerEntry, PrepareTextRequest,
    PreparedTextCssTypographyOverrides, PreparedTextLabelFamily, PreparedTextLabelId,
    PreparedTextLabelLedgerEntry, PreparedTextLayout, PreparedTextWrap, TextLayoutError,
    TextMeasurer, TextMetrics, TextStyle, WrapMode,
    merge_prepared_text_typography_with_css_overrides, parse_css_font_stack,
};

use super::label::{
    FlowchartLabelMetricsRequest, FlowchartSvgLabelSource, FlowchartSvgWidthMode,
    flowchart_label_metrics_for_layout,
};

const FLOWCHART_SOURCE_ENTRY_RECORD_BYTES: usize = 64;
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

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FlowchartLabelTypographyOverrides<'a> {
    pub(crate) wrapping: Option<&'a PreparedTextCssTypographyOverrides>,
    pub(crate) metrics: Option<&'a PreparedTextCssTypographyOverrides>,
    pub(crate) terminal_foreground: Option<&'a super::FlowchartTerminalForeground>,
}

impl<'a> FlowchartLabelTypographyOverrides<'a> {
    pub(crate) const fn same(overrides: &'a PreparedTextCssTypographyOverrides) -> Self {
        Self {
            wrapping: Some(overrides),
            metrics: Some(overrides),
            terminal_foreground: None,
        }
    }

    pub(crate) const fn metrics_only(overrides: &'a PreparedTextCssTypographyOverrides) -> Self {
        Self {
            wrapping: None,
            metrics: Some(overrides),
            terminal_foreground: None,
        }
    }

    pub(crate) const fn with_terminal_foreground(
        mut self,
        terminal_foreground: Option<&'a super::FlowchartTerminalForeground>,
    ) -> Self {
        self.terminal_foreground = terminal_foreground;
        self
    }
}

impl FlowchartSvgLabelOwner {
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

    fn prepared_text_family(self) -> PreparedTextLabelFamily {
        match self {
            Self::SwimlaneNode(_) | Self::SwimlaneEdgeLabel(_) | Self::SwimlaneGroupTitle(_) => {
                PreparedTextLabelFamily::Swimlane
            }
            Self::Node(_) | Self::EmptySubgraphNode(_) | Self::Edge(_) | Self::SubgraphTitle(_) => {
                PreparedTextLabelFamily::Flowchart
            }
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

    fn for_each_mut(&mut self, mut visit: impl FnMut(FlowchartSvgLabelOwner, &mut T)) {
        for (index, value) in &mut self.nodes.entries {
            visit(FlowchartSvgLabelOwner::Node(*index), value);
        }
        for (index, value) in &mut self.empty_subgraph_nodes.entries {
            visit(FlowchartSvgLabelOwner::EmptySubgraphNode(*index), value);
        }
        for (index, value) in &mut self.edges.entries {
            visit(FlowchartSvgLabelOwner::Edge(*index), value);
        }
        for (index, value) in &mut self.subgraph_titles.entries {
            visit(FlowchartSvgLabelOwner::SubgraphTitle(*index), value);
        }
        for (index, value) in &mut self.swimlane_nodes.entries {
            visit(FlowchartSvgLabelOwner::SwimlaneNode(*index), value);
        }
        for (index, value) in &mut self.swimlane_edge_labels.entries {
            visit(FlowchartSvgLabelOwner::SwimlaneEdgeLabel(*index), value);
        }
        for (index, value) in &mut self.swimlane_group_titles.entries {
            visit(FlowchartSvgLabelOwner::SwimlaneGroupTitle(*index), value);
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

    fn retained_bytes(&self) -> usize {
        self.font_family
            .as_ref()
            .map_or(0, String::len)
            .saturating_add(self.font_weight.as_ref().map_or(0, String::len))
            .saturating_add(self.font_style.as_ref().map_or(0, String::len))
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

    fn for_native(
        wrap_style: &'a TextStyle,
        metrics_style: &'a TextStyle,
        max_width_px: Option<f64>,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> Self {
        Self {
            wrap_style,
            metrics_style,
            max_width_bits: normalized_width_bits(max_width_px),
            break_long_words,
            width_mode,
            computed_length_carrier: None,
            wrapped_carrier: None,
        }
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

    fn is_native(&self) -> bool {
        self.computed_length_carrier.is_none() && self.wrapped_carrier.is_none()
    }

    fn retained_bytes(&self) -> usize {
        self.wrap_style
            .retained_bytes()
            .saturating_add(self.metrics_style.retained_bytes())
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
    admitted_typography: Option<CatalogAdmittedTextStyle>,
    pending_label_entry: Option<PendingPreparedTextLabelLedgerEntry>,
    label_entry: Option<PreparedTextLabelLedgerEntry>,
    label_consumed: Cell<bool>,
    retained_reservation: RefCell<Option<PreparedTextRetainedReservation>>,
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

    fn retained_bytes(&self) -> usize {
        FLOWCHART_SOURCE_ENTRY_RECORD_BYTES
            .saturating_add(self.raw_source.len())
            .saturating_add(self.source.retained_bytes())
    }
}

impl PreparedFlowchartSvgLabel {
    fn new(
        binding: FlowchartSvgLabelBinding,
        wrapped_lines: Vec<Vec<String>>,
        metrics: TextMetrics,
        admitted_typography: Option<CatalogAdmittedTextStyle>,
    ) -> Self {
        Self {
            binding,
            wrapped_lines,
            metrics,
            admitted_typography,
            pending_label_entry: None,
            label_entry: None,
            label_consumed: Cell::new(false),
            retained_reservation: RefCell::new(None),
        }
    }

    fn with_label_entry(mut self, entry: PendingPreparedTextLabelLedgerEntry) -> Self {
        self.pending_label_entry = Some(entry);
        self
    }

    pub(crate) fn wrapped_lines(&self) -> &[Vec<String>] {
        &self.wrapped_lines
    }

    pub(crate) fn metrics(&self) -> TextMetrics {
        self.metrics
    }

    fn merge_emission_font_style(&self, existing: Option<&str>) -> Option<String> {
        self.admitted_typography
            .as_ref()
            .map(|typography| typography.merge_materialized_emission_font_style(existing))
    }

    fn emitted_admitted_typography(&self) -> bool {
        self.admitted_typography.is_some() && self.label_consumed.get()
    }

    fn line_height_em(&self) -> Option<f64> {
        self.admitted_typography
            .as_ref()
            .map(CatalogAdmittedTextStyle::line_height_em)
    }

    fn matches(&self, binding: &FlowchartSvgLabelBindingRequest<'_>) -> bool {
        self.binding.matches(binding)
    }

    fn matches_wrapping(&self, binding: &FlowchartSvgLabelWrapBindingRequest<'_>) -> bool {
        self.binding.matches_wrapping(binding)
    }

    fn matches_native_wrapping(
        &self,
        wrap_style: &TextStyle,
        max_width_px: Option<f64>,
        break_long_words: bool,
    ) -> bool {
        self.binding.is_native()
            && self.binding.wrap_style.matches(wrap_style)
            && self.binding.max_width_bits == normalized_width_bits(max_width_px)
            && self.binding.break_long_words == break_long_words
    }

    fn bind_label_id(&mut self, owner: FlowchartSvgLabelOwner, key: u32) -> bool {
        let Some(pending) = self.pending_label_entry.take() else {
            return false;
        };
        self.label_entry =
            Some(pending.bind(PreparedTextLabelId::new(owner.prepared_text_family(), key)));
        true
    }

    fn label_id_for_emission(&self) -> Option<PreparedTextLabelId> {
        let id = self
            .label_entry
            .as_ref()
            .map(PreparedTextLabelLedgerEntry::id);
        if id.is_some() {
            self.label_consumed.set(true);
        }
        id
    }

    fn consumed_label_entry(&self) -> Option<&PreparedTextLabelLedgerEntry> {
        self.label_consumed
            .get()
            .then_some(self.label_entry.as_ref())
            .flatten()
    }

    fn retained_bytes(&self) -> usize {
        const VEC_RECORD_BYTES: usize = 24;
        const STRING_RECORD_BYTES: usize = 24;
        const LABEL_RECORD_BYTES: usize = 256;

        let wrapped_lines = self.wrapped_lines.iter().fold(0usize, |total, line| {
            line.iter().fold(
                total.saturating_add(VEC_RECORD_BYTES),
                |line_total, word| {
                    line_total
                        .saturating_add(STRING_RECORD_BYTES)
                        .saturating_add(word.len())
                },
            )
        });
        LABEL_RECORD_BYTES
            .saturating_add(self.binding.retained_bytes())
            .saturating_add(wrapped_lines)
            .saturating_add(
                self.admitted_typography
                    .as_ref()
                    .map_or(0, CatalogAdmittedTextStyle::retained_bytes),
            )
            .saturating_add(
                self.pending_label_entry
                    .as_ref()
                    .map_or(0, PendingPreparedTextLabelLedgerEntry::retained_bytes),
            )
            .saturating_add(
                self.label_entry
                    .as_ref()
                    .map_or(0, PreparedTextLabelLedgerEntry::retained_bytes),
            )
    }

    fn reserve_retained_bytes(
        &self,
        work_meter: &Arc<OperationWorkMeter>,
        retained_bytes: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        let reservation = work_meter.reserve_prepared_text_retained_bytes(retained_bytes)?;
        *self.retained_reservation.borrow_mut() = Some(reservation);
        Ok(())
    }

    fn take_consumed_retained_reservation(&self) -> Option<PreparedTextRetainedReservation> {
        let entry = self.consumed_label_entry()?;
        let mut reservation = self.retained_reservation.borrow_mut().take()?;
        let retained_bytes = entry.retained_bytes();
        if retained_bytes < reservation.retained_bytes() {
            reservation.reconcile_downward(retained_bytes);
        }
        Some(reservation)
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
    math_backend: Option<ConfiguredMathBackend>,
    prepared_text_layout: Option<PreparedTextLayout>,
    work_meter: Option<Arc<OperationWorkMeter>>,
    resolved_theme: Option<ResolvedDiagramTheme>,
    edge_label_padding: super::FlowchartEdgeLabelPadding,
    typography_config_ownership: crate::flowchart::FlowchartTypographyConfigOwnership,
    prepared_error: RefCell<Option<TextLayoutError>>,
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
    sources: FlowchartSvgLabelSlots<FlowchartSvgLabelSourceEntry>,
    prepared: FlowchartSvgLabelSlots<PreparedFlowchartSvgLabel>,
    math: FlowchartSvgLabelSlots<PreparedFlowchartMathLabel>,
    render_ids: FlowchartSvgLabelSlots<Box<str>>,
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
            bytes = bytes.checked_add(retained).unwrap_or(usize::MAX);
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
            || {
                owner_bytes
                    .checked_add(occurrence_id.as_str().len())
                    .unwrap_or(usize::MAX)
            },
            |artifact| {
                owner_bytes
                    .checked_add(artifact.retained_bytes())
                    .unwrap_or(usize::MAX)
            },
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
        reservation.reconcile_downward(expectation.retained_bytes());
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
    fn from_config(config: &merman_core::MermaidConfig) -> Self {
        let [node, title] =
            crate::svg::render_theme::flowchart_text_surface_fills(config.as_value());
        Self {
            node: super::FlowchartTerminalForeground::new(
                &node,
                super::FlowchartTerminalForegroundProvenance::ThemeNode,
            ),
            title: super::FlowchartTerminalForeground::new(
                &title,
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
pub(crate) enum FlowchartPreparedMathResolution<'a> {
    #[default]
    NotPrepared,
    Prepared(&'a PreparedMathLabel),
    Unavailable(MathPreparationUnavailable),
}

enum PreparedMathMeasurement {
    NotApplicable,
    Prepared(TextMetrics),
    Unavailable,
}

impl FlowchartSvgLabelSidecarBuilder {
    pub(crate) fn new(
        prepared_text_layout: Option<&PreparedTextLayout>,
        resolved_theme: Option<&ResolvedDiagramTheme>,
    ) -> Self {
        Self {
            prepared_text_layout: prepared_text_layout.cloned(),
            resolved_theme: resolved_theme.cloned(),
            ..Self::default()
        }
    }

    pub(crate) fn new_with_work_meter(
        prepared_text_layout: Option<&PreparedTextLayout>,
        resolved_theme: Option<&ResolvedDiagramTheme>,
        work_meter: Arc<OperationWorkMeter>,
    ) -> Self {
        Self {
            prepared_text_layout: prepared_text_layout.cloned(),
            work_meter: Some(work_meter),
            resolved_theme: resolved_theme.cloned(),
            ..Self::default()
        }
    }

    pub(crate) fn with_typography_config_ownership(
        mut self,
        ownership: crate::flowchart::FlowchartTypographyConfigOwnership,
    ) -> Self {
        self.typography_config_ownership = ownership;
        self
    }

    pub(crate) fn with_base_typography(mut self, plan: super::FlowchartBaseTypographyPlan) -> Self {
        self.base_typography = Some(plan);
        self
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

    pub(crate) fn with_math_backend(
        mut self,
        math_backend: Option<&ConfiguredMathBackend>,
        config: &merman_core::MermaidConfig,
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

    fn record_prepared_error(&self, error: TextLayoutError) {
        if self.prepared_resource_error.borrow().is_some() {
            return;
        }
        let mut prepared_error = self.prepared_error.borrow_mut();
        if prepared_error.is_none() {
            *prepared_error = Some(error);
        }
    }

    fn record_prepared_resource_error(&self, error: ResourceLimitExceeded) {
        let mut prepared_resource_error = self.prepared_resource_error.borrow_mut();
        if prepared_resource_error.is_none() {
            *prepared_resource_error = Some(error);
            self.prepared_error.borrow_mut().take();
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

    fn reserve_prepared_label(
        &self,
        label: &PreparedFlowchartSvgLabel,
        source: &FlowchartSvgLabelSourceEntry,
        render_id: &str,
    ) -> bool {
        let Some(work_meter) = self.work_meter.as_ref() else {
            return true;
        };
        let retained_bytes = label
            .retained_bytes()
            .saturating_add(source.retained_bytes())
            .saturating_add(FLOWCHART_RENDER_ID_RECORD_BYTES)
            .saturating_add(render_id.len())
            .saturating_add(FLOWCHART_PREPARED_OWNER_SLOT_BYTES);
        match label.reserve_retained_bytes(work_meter, retained_bytes) {
            Ok(()) => true,
            Err(error) => {
                self.record_prepared_resource_error(error);
                false
            }
        }
    }

    pub(crate) fn reject_unsupported_prepared_path(&self, path: &'static str) -> bool {
        if self.prepared_text_layout.is_none() {
            return false;
        }
        self.record_prepared_error(TextLayoutError::UnsupportedPreparedTextPath(path));
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
        self.measure_for_layout_with_typography_overrides(
            owner,
            render_id,
            request,
            FlowchartLabelTypographyOverrides::default(),
            break_long_words,
            width_mode,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn measure_for_layout_with_typography_overrides(
        &self,
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: FlowchartLabelMetricsRequest<'_>,
        typography_overrides: FlowchartLabelTypographyOverrides<'_>,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> TextMetrics {
        let metrics_style = request.style;
        self.measure_for_layout_with_metrics_style_and_typography_overrides(
            owner,
            render_id,
            request,
            metrics_style,
            typography_overrides,
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
        self.measure_for_layout_with_metrics_style_and_typography_overrides(
            owner,
            render_id,
            request,
            metrics_style,
            FlowchartLabelTypographyOverrides::default(),
            break_long_words,
            width_mode,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn measure_for_layout_with_metrics_style_and_typography_overrides(
        &self,
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: FlowchartLabelMetricsRequest<'_>,
        metrics_style: &TextStyle,
        typography_overrides: FlowchartLabelTypographyOverrides<'_>,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> TextMetrics {
        if self.prepared_error.borrow().is_some()
            || self.prepared_resource_error.borrow().is_some()
            || self.prepared_work_error.borrow().is_some()
        {
            return failed_prepared_metrics();
        }
        // Base typography evidence covers every visible label shell, including HTML labels that
        // intentionally bypass native SVG source preparation. Retain their semantic owner before
        // any measurement fast path so the terminal writer can bind source-local typography to
        // the exact occurrence instead of falling back to an unidentified residual.
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
            typography_overrides.terminal_foreground,
        ) {
            PreparedMathMeasurement::Prepared(metrics) => return metrics,
            PreparedMathMeasurement::Unavailable => {
                return flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
                    style: metrics_style,
                    math_renderer: None,
                    ..request
                });
            }
            PreparedMathMeasurement::NotApplicable => {}
        }
        if !supports_svg_source_preparation(owner, &request) {
            if self.prepared_text_layout.is_some() {
                self.record_prepared_error(TextLayoutError::UnsupportedPreparedTextPath(
                    "flowchart_label",
                ));
                return failed_prepared_metrics();
            }
            return flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
                style: metrics_style,
                ..request
            });
        }

        if self.prepared_text_layout.is_some() {
            let native_binding = FlowchartSvgLabelBindingRequest::for_native(
                request.style,
                metrics_style,
                request.max_width_px,
                break_long_words,
                width_mode,
            );
            {
                let pending = self.pending.borrow();
                if let Some(metrics) = pending
                    .prepared
                    .get(owner)
                    .filter(|prepared| {
                        prepared.matches_native_wrapping(
                            request.style,
                            request.max_width_px,
                            break_long_words,
                        ) && prepared.binding.metrics_style.matches(metrics_style)
                            && prepared.binding.width_mode == width_mode
                    })
                    .map(PreparedFlowchartSvgLabel::metrics)
                {
                    #[cfg(test)]
                    self.prepared_hits
                        .set(self.prepared_hits.get().saturating_add(1));
                    return metrics;
                }
            }

            match self.prepare_native_label(
                owner,
                render_id,
                request,
                metrics_style,
                typography_overrides,
                break_long_words,
                width_mode,
                native_binding,
            ) {
                Ok(metrics) => return metrics,
                Err(error) => {
                    self.record_prepared_error(error);
                    return failed_prepared_metrics();
                }
            }
        }

        let binding = FlowchartSvgLabelBindingRequest::for_measurer(
            request.measurer,
            request.style,
            metrics_style,
            request.max_width_px,
            break_long_words,
            width_mode,
        );

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
                let metrics = source.source.metrics_from_wrapped(
                    request.measurer,
                    metrics_style,
                    &wrapped_lines,
                    width_mode,
                );
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
                let metrics = source.source.metrics_from_wrapped(
                    request.measurer,
                    metrics_style,
                    &wrapped_lines,
                    width_mode,
                );
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
                PreparedFlowchartSvgLabel::new(binding.into_owned(), wrapped_lines, metrics, None),
            );
        } else {
            pending.prepared.remove(owner);
        }
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

    #[allow(clippy::too_many_arguments)]
    fn prepare_native_label(
        &self,
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: FlowchartLabelMetricsRequest<'_>,
        metrics_style: &TextStyle,
        typography_overrides: FlowchartLabelTypographyOverrides<'_>,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
        binding: FlowchartSvgLabelBindingRequest<'_>,
    ) -> Result<TextMetrics, TextLayoutError> {
        let layout = self.prepared_text_layout.as_ref().ok_or(
            TextLayoutError::UnsupportedPreparedTextPath("flowchart_label"),
        )?;
        let theme =
            self.resolved_theme
                .as_ref()
                .ok_or(TextLayoutError::UnsupportedPreparedTextPath(
                    "structured_typography",
                ))?;
        let source = FlowchartSvgLabelSourceEntry::new(request.raw_label);
        let (wrapping_typography, metrics_typography) = native_typography_pair(
            layout,
            theme,
            owner,
            request.style,
            metrics_style,
            typography_overrides,
            self.typography_config_ownership,
        )?;
        debug_assert_eq!(
            wrapping_typography.catalog_fingerprint(),
            layout.catalog_fingerprint(),
            "prepared Flowchart typography must remain bound to the active catalog"
        );
        debug_assert_eq!(
            metrics_typography.catalog_fingerprint(),
            layout.catalog_fingerprint(),
            "prepared Flowchart metrics typography must remain bound to the active catalog"
        );
        if source.source.plain_text().is_empty() {
            let label = PreparedFlowchartSvgLabel::new(
                binding.into_owned(),
                Vec::new(),
                TextMetrics {
                    width: 0.0,
                    height: 0.0,
                    line_count: 0,
                },
                Some(metrics_typography),
            );
            if !self.reserve_prepared_label(&label, &source, render_id) {
                return Ok(failed_prepared_metrics());
            }
            let mut pending = self.pending.borrow_mut();
            pending.render_ids.insert(owner, render_id.into());
            pending.sources.insert(owner, source);
            pending.prepared.insert(owner, label);
            return Ok(TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            });
        }

        let source_projection = source
            .source
            .prepared_projection(metrics_typography.typography().transform())?;
        let prepared_wrapping_typography = wrapping_typography
            .typography()
            .clone()
            .with_transform(ThemeTextTransform::None);
        let prepared_metrics_typography = metrics_typography
            .typography()
            .clone()
            .with_transform(ThemeTextTransform::None);

        let request = PrepareTextRequest::new(
            source_projection.visible_text(),
            prepared_wrapping_typography,
        )
        .with_metrics_typography(prepared_metrics_typography)
        .with_family_normalized_projection()
        .with_wrap(PreparedTextWrap::SvgLike {
            max_width_px: request.max_width_px,
            break_long_words,
        });
        let prepared = match self.work_meter.as_deref() {
            Some(work_meter) => layout.prepare_text_with_work_meter(&request, work_meter)?,
            None => layout.prepare_text(&request)?,
        };
        let wrapped_lines = source
            .source
            .project_prepared_lines(&source_projection, &prepared)?;
        let label_entry = prepared
            .label_ledger_entry()
            .ok_or(TextLayoutError::InvalidPreparedText)?;
        let mut metrics = prepared.metrics();
        if width_mode == FlowchartSvgWidthMode::ComputedLength {
            metrics.width = prepared
                .lines()
                .iter()
                .map(|line| line.computed_length_px())
                .fold(0.0, f64::max);
        }

        let label = PreparedFlowchartSvgLabel::new(
            binding.into_owned(),
            wrapped_lines,
            metrics,
            Some(metrics_typography),
        )
        .with_label_entry(label_entry);
        if !self.reserve_prepared_label(&label, &source, render_id) {
            return Ok(failed_prepared_metrics());
        }
        let mut pending = self.pending.borrow_mut();
        pending.render_ids.insert(owner, render_id.into());
        pending.sources.insert(owner, source);
        pending.prepared.insert(owner, label);
        Ok(metrics)
    }

    pub(crate) fn finish(self) -> FlowchartSvgLabelSidecar {
        #[cfg(test)]
        let source_plans_by_owner = self.source_plans_by_owner.into_inner();
        let pending = self.pending.into_inner();
        FlowchartSvgLabelSidecar::new(
            pending.sources,
            pending.prepared,
            pending.math,
            pending.render_ids,
            self.base_typography,
            self.edge_label_padding,
            self.prepared_error.into_inner(),
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
    measure_flowchart_svg_label_for_layout_with_typography_overrides(
        sidecar,
        owner,
        render_id,
        request,
        FlowchartLabelTypographyOverrides::default(),
        width_mode,
    )
}

pub(crate) fn measure_flowchart_svg_label_for_layout_with_typography_overrides(
    sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    owner: Option<FlowchartSvgLabelOwner>,
    render_id: Option<&str>,
    request: FlowchartLabelMetricsRequest<'_>,
    typography_overrides: FlowchartLabelTypographyOverrides<'_>,
    width_mode: FlowchartSvgWidthMode,
) -> TextMetrics {
    match sidecar.zip(owner).zip(render_id) {
        Some(((sidecar, owner), render_id)) => sidecar
            .measure_for_layout_with_typography_overrides(
                owner,
                render_id,
                request,
                typography_overrides,
                true,
                width_mode,
            ),
        None => measure_svg_label_without_sidecar(request, width_mode),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn measure_flowchart_svg_label_for_layout_with_metrics_style_and_typography_overrides(
    sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    owner: Option<FlowchartSvgLabelOwner>,
    render_id: Option<&str>,
    request: FlowchartLabelMetricsRequest<'_>,
    metrics_style: &TextStyle,
    typography_overrides: FlowchartLabelTypographyOverrides<'_>,
    width_mode: FlowchartSvgWidthMode,
) -> TextMetrics {
    match sidecar.zip(owner).zip(render_id) {
        Some(((sidecar, owner), render_id)) => sidecar
            .measure_for_layout_with_metrics_style_and_typography_overrides(
                owner,
                render_id,
                request,
                metrics_style,
                typography_overrides,
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

fn native_typography_pair(
    layout: &PreparedTextLayout,
    theme: &ResolvedDiagramTheme,
    owner: FlowchartSvgLabelOwner,
    wrap_style: &TextStyle,
    metrics_style: &TextStyle,
    typography_overrides: FlowchartLabelTypographyOverrides<'_>,
    config_ownership: crate::flowchart::FlowchartTypographyConfigOwnership,
) -> Result<(CatalogAdmittedTextStyle, CatalogAdmittedTextStyle), TextLayoutError> {
    let target = match owner {
        FlowchartSvgLabelOwner::Node(_)
        | FlowchartSvgLabelOwner::EmptySubgraphNode(_)
        | FlowchartSvgLabelOwner::SwimlaneNode(_) => ThemeTarget::NodeLabel,
        FlowchartSvgLabelOwner::Edge(_) | FlowchartSvgLabelOwner::SwimlaneEdgeLabel(_) => {
            ThemeTarget::EdgeLabel
        }
        FlowchartSvgLabelOwner::SubgraphTitle(_)
        | FlowchartSvgLabelOwner::SwimlaneGroupTitle(_) => ThemeTarget::ClusterLabel,
    };
    let base = theme
        .style(target, ThemeVariant::Default, None)
        .typography()
        .clone();
    Ok((
        admit_flowchart_prepared_typography(
            layout,
            &base,
            wrap_style,
            typography_overrides.wrapping,
            config_ownership,
        )?,
        admit_flowchart_prepared_typography(
            layout,
            &base,
            metrics_style,
            typography_overrides.metrics,
            config_ownership,
        )?,
    ))
}

fn admit_flowchart_prepared_typography(
    layout: &PreparedTextLayout,
    base: &ThemeTextStyle,
    legacy: &TextStyle,
    overrides: Option<&PreparedTextCssTypographyOverrides>,
    config_ownership: crate::flowchart::FlowchartTypographyConfigOwnership,
) -> Result<CatalogAdmittedTextStyle, TextLayoutError> {
    let base = flowchart_prepared_base_typography(base, legacy, config_ownership.font_stack)?;
    let mut legacy = legacy.clone();
    if !config_ownership.font_size && !overrides.is_some_and(|overrides| overrides.has_font_size())
    {
        legacy.font_size = f64::from(base.font_size_px());
    }
    layout.admit_typography_request(&merge_prepared_text_typography_with_css_overrides(
        &base, &legacy, overrides,
    )?)
}

fn flowchart_prepared_base_typography(
    base: &ThemeTextStyle,
    legacy: &TextStyle,
    mermaid_font_stack_overrides_typed: bool,
) -> Result<ThemeTextStyle, TextLayoutError> {
    if !mermaid_font_stack_overrides_typed {
        return Ok(base.clone());
    }
    let font_family = legacy
        .font_family
        .as_deref()
        .ok_or(TextLayoutError::InvalidRequest("font_family"))?;
    let parsed =
        parse_css_font_stack(font_family).ok_or(TextLayoutError::InvalidRequest("font_family"))?;
    Ok(base.clone().with_font_stack(parsed.font_stack().clone()))
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
    sources: FlowchartSvgLabelSlots<FlowchartSvgLabelSourceEntry>,
    prepared: FlowchartSvgLabelSlots<PreparedFlowchartSvgLabel>,
    math: FlowchartSvgLabelSlots<PreparedFlowchartMathLabel>,
    base_typography: Option<super::FlowchartBaseTypographyPlan>,
    node_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    empty_subgraph_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    edge_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    subgraph_title_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    swimlane_node_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    swimlane_edge_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    swimlane_group_title_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    edge_label_padding: super::FlowchartEdgeLabelPadding,
    prepared_error: Option<TextLayoutError>,
    prepared_resource_error: Option<ResourceLimitExceeded>,
    prepared_work_error: Option<OperationWorkError>,
    #[cfg(test)]
    prepared_hits_by_owner: Mutex<FxHashMap<FlowchartSvgLabelOwner, usize>>,
    #[cfg(test)]
    source_plans_by_owner: Mutex<FxHashMap<FlowchartSvgLabelOwner, usize>>,
}

impl FlowchartSvgLabelSidecar {
    fn new(
        sources: FlowchartSvgLabelSlots<FlowchartSvgLabelSourceEntry>,
        mut prepared: FlowchartSvgLabelSlots<PreparedFlowchartSvgLabel>,
        math: FlowchartSvgLabelSlots<PreparedFlowchartMathLabel>,
        render_ids: FlowchartSvgLabelSlots<Box<str>>,
        base_typography: Option<super::FlowchartBaseTypographyPlan>,
        edge_label_padding: super::FlowchartEdgeLabelPadding,
        mut prepared_error: Option<TextLayoutError>,
        prepared_resource_error: Option<ResourceLimitExceeded>,
        prepared_work_error: Option<OperationWorkError>,
        #[cfg(test)] source_plans_by_owner: FxHashMap<FlowchartSvgLabelOwner, usize>,
    ) -> Self {
        let mut next_key = 0usize;
        let mut key_overflow = false;
        prepared.for_each_mut(|owner, label| {
            let Ok(key) = u32::try_from(next_key) else {
                if label.pending_label_entry.is_some() {
                    key_overflow = true;
                }
                return;
            };
            if label.bind_label_id(owner, key) {
                next_key = next_key.saturating_add(1);
            }
        });
        if key_overflow && prepared_error.is_none() {
            prepared_error = Some(TextLayoutError::LimitExceeded("prepared_label_ledger"));
        }
        let mut sidecar = Self {
            sources,
            prepared,
            math,
            base_typography,
            edge_label_padding,
            prepared_error,
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
                    // Mermaid's FlowDB emits duplicate subgraph ids in reverse semantic order;
                    // Graphlib then updates the existing node, leaving the earliest definition's
                    // presentation value as the winner. Keep the same canonical owner here so a
                    // prepared title cannot be bound to the later definition by accident.
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

    pub(crate) fn prepared_error(&self) -> Option<&TextLayoutError> {
        self.prepared_error.as_ref()
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

    pub(crate) fn prepared_text_label_ledger(
        &self,
    ) -> impl Iterator<Item = &PreparedTextLabelLedgerEntry> {
        self.prepared
            .iter()
            .filter_map(PreparedFlowchartSvgLabel::consumed_label_entry)
    }

    pub(crate) fn take_prepared_text_retained_reservations(
        &self,
    ) -> Vec<PreparedTextRetainedReservation> {
        self.prepared
            .iter()
            .filter_map(PreparedFlowchartSvgLabel::take_consumed_retained_reservation)
            .collect()
    }

    pub(crate) fn prepared_math_evidence(&self) -> PreparedMathEvidenceLease {
        let entries = self
            .math
            .iter()
            .filter_map(PreparedFlowchartMathLabel::terminal_expectation)
            .collect();
        let reservations = self
            .math
            .iter()
            .filter_map(PreparedFlowchartMathLabel::take_terminal_reservation)
            .collect();
        PreparedMathEvidenceLease::new(entries, reservations)
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

    pub(crate) fn prepared_math(
        &self,
        owner: FlowchartSvgLabelOwner,
        raw_source: &str,
    ) -> FlowchartPreparedMathResolution<'_> {
        let Some(prepared) = self
            .math
            .get(owner)
            .filter(|prepared| prepared.source.as_ref() == raw_source)
        else {
            return FlowchartPreparedMathResolution::NotPrepared;
        };
        Self::resolve_prepared_math(prepared)
    }

    pub(crate) fn prepared_math_for_terminal(
        &self,
        owner: FlowchartSvgLabelOwner,
        raw_source: &str,
    ) -> FlowchartPreparedMathResolution<'_> {
        let Some(prepared) = self
            .math
            .get(owner)
            .filter(|prepared| prepared.source.as_ref() == raw_source)
        else {
            return FlowchartPreparedMathResolution::NotPrepared;
        };
        Self::resolve_prepared_math(prepared)
    }

    fn resolve_prepared_math(
        prepared: &PreparedFlowchartMathLabel,
    ) -> FlowchartPreparedMathResolution<'_> {
        match &prepared.outcome {
            MathPreparationOutcome::Prepared(artifact) => {
                FlowchartPreparedMathResolution::Prepared(artifact)
            }
            MathPreparationOutcome::Unavailable(reason) => {
                FlowchartPreparedMathResolution::Unavailable(*reason)
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

    fn prepared_native_wrapping(
        &self,
        owner: FlowchartSvgLabelOwner,
        raw_source: &str,
        wrap_style: &TextStyle,
        max_width_px: Option<f64>,
        break_long_words: bool,
    ) -> Option<(&FlowchartSvgLabelSource, &PreparedFlowchartSvgLabel)> {
        let source = self
            .sources
            .get(owner)
            .filter(|source| source.matches(raw_source));
        let prepared = source
            .map(|source| &source.source)
            .zip(self.prepared.get(owner).filter(|prepared| {
                prepared.matches_native_wrapping(wrap_style, max_width_px, break_long_words)
            }));
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
    },
    Source {
        source: Cow<'a, FlowchartSvgLabelSource>,
        measurer: &'a dyn TextMeasurer,
        style: &'a TextStyle,
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
        _metrics_style: &TextStyle,
        max_width_px: Option<f64>,
        break_long_words: bool,
        _width_mode: FlowchartSvgWidthMode,
    ) -> Self {
        // SVG emission consumes only the prepared source projection and wrapped rows. Final bbox
        // style and width-mode differences therefore must not trigger a second tokenize/wrap pass.
        // Exact metric reuse remains separately guarded by `prepared_metrics`.
        if let Some((source, measured)) = sidecar.zip(owner).and_then(|(sidecar, owner)| {
            sidecar.prepared_native_wrapping(
                owner,
                raw_source,
                wrap_style,
                max_width_px,
                break_long_words,
            )
        }) {
            return Self::Prepared { source, measured };
        }

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
            Self::Prepared { source, measured }
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

    pub(crate) fn prepared_text_label_id(&self) -> Option<PreparedTextLabelId> {
        match self {
            Self::Prepared { measured, .. } => measured.label_id_for_emission(),
            Self::Source { .. } => None,
        }
    }

    pub(crate) fn merge_emission_font_style(&self, existing: Option<&str>) -> Option<String> {
        match self {
            Self::Prepared { measured, .. } => measured.merge_emission_font_style(existing),
            Self::Source { .. } => None,
        }
    }

    pub(crate) fn emitted_admitted_typography(&self) -> bool {
        match self {
            Self::Prepared { measured, .. } => measured.emitted_admitted_typography(),
            Self::Source { .. } => false,
        }
    }

    pub(crate) fn line_height_em(&self) -> f64 {
        match self {
            Self::Prepared { measured, .. } => measured.line_height_em().unwrap_or(1.1),
            Self::Source { .. } => 1.1,
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

    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontAssetSpec, FontCatalogSpec, FontSourcePolicy,
        FontStack, Specified, TextStylePatch, TextTransform, ThemeAssets, ThemeRule, ThemeRuleSet,
        ThemeStylePatch, ThemeTarget, ThemeTextStyle, TypographySpec,
    };
    use crate::environment::{RenderEnvironment, TextMeasurementPhase};
    use crate::flowchart::FLOWCHART_FIXED_LABEL_WRAP_WIDTH;
    use crate::math::MathRenderer;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
    use crate::text::{
        NativeTextLayoutBackend, PrepareCatalogRequest, PreparedTextCssTypographyOverrides,
        TextMeasurer, TextMetrics, TextStyle, WrapMode,
    };
    use merman_core::MermaidConfig;

    use super::*;

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
        fn render_html_label(&self, text: &str, _config: &MermaidConfig) -> Option<String> {
            self.render_calls.fetch_add(1, Ordering::SeqCst);
            Some(format!("<span class=\"prepared-math\">{text}</span>"))
        }

        fn measure_html_label(
            &self,
            _text: &str,
            _config: &MermaidConfig,
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

    #[test]
    fn flowchart_math_occurrence_reuses_the_layout_artifact_for_terminal_emission() {
        let render_calls = Arc::new(AtomicUsize::new(0));
        let measure_calls = Arc::new(AtomicUsize::new(0));
        let backend = ConfiguredMathBackend::external(Arc::new(CountingPreparedMathRenderer {
            render_calls: Arc::clone(&render_calls),
            measure_calls: Arc::clone(&measure_calls),
        }));
        let config = MermaidConfig::from_value(serde_json::json!({
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
        let config = MermaidConfig::default();
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
            FlowchartPreparedMathResolution::Unavailable(
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
        let config = MermaidConfig::default();
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
            FlowchartPreparedMathResolution::NotPrepared
        ));
    }

    #[test]
    fn flowchart_math_occurrence_uses_the_resolved_source_foreground() {
        let backend = ConfiguredMathBackend::external(Arc::new(CountingPreparedMathRenderer {
            render_calls: Arc::new(AtomicUsize::new(0)),
            measure_calls: Arc::new(AtomicUsize::new(0)),
        }));
        let config = MermaidConfig::from_value(serde_json::json!({
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

        builder.measure_for_layout_with_typography_overrides(
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
            FlowchartLabelTypographyOverrides::default()
                .with_terminal_foreground(Some(&terminal_foreground)),
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
        let FlowchartPreparedMathResolution::Prepared(prepared) =
            sidecar.prepared_math(owner, "$$x^2$$")
        else {
            panic!("prepared source-colored math occurrence");
        };
        assert!(prepared.browser_xhtml().contains("color:#f43f5e"));
    }

    fn native_flowchart_text_fixture() -> (PreparedTextLayout, ResolvedDiagramTheme) {
        native_flowchart_text_fixture_with_options(TextTransform::None, None)
    }

    fn native_flowchart_text_fixture_with_transform(
        transform: TextTransform,
    ) -> (PreparedTextLayout, ResolvedDiagramTheme) {
        native_flowchart_text_fixture_with_options(transform, None)
    }

    fn native_flowchart_text_fixture_with_node_label_font_size(
        font_size_px: f32,
    ) -> (PreparedTextLayout, ResolvedDiagramTheme) {
        native_flowchart_text_fixture_with_options(TextTransform::None, Some(font_size_px))
    }

    fn native_flowchart_text_fixture_with_options(
        transform: TextTransform,
        node_label_font_size_px: Option<f32>,
    ) -> (PreparedTextLayout, ResolvedDiagramTheme) {
        let latin = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        let cjk = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Xiaolai-Regular-CJK-Test.woff2"
        ));
        let catalog = FontCatalogSpec::new([
            FontAssetSpec::new("latin", latin),
            FontAssetSpec::new("cjk", cjk),
        ])
        .with_alias("Xiaolai", "Xiaolai SC");
        let typography = ThemeTextStyle::default()
            .with_font_stack(
                FontStack::new(["Excalifont", "Xiaolai SC"])
                    .expect("fixture font stack should be valid"),
            )
            .with_transform(transform);
        let mut spec = DiagramThemeSpec::new()
            .with_typography(TypographySpec::default().with_default(typography))
            .with_assets(ThemeAssets::default().with_font_catalog(catalog));
        if let Some(font_size_px) = node_label_font_size_px {
            spec = spec.with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::NodeLabel,
                        ThemeStylePatch {
                            typography: TextStylePatch {
                                font_size_px: Specified::Value(font_size_px),
                                ..TextStylePatch::default()
                            },
                            ..ThemeStylePatch::default()
                        },
                    )
                    .for_family(DiagramFamilyId::FLOWCHART),
                ),
            );
        }
        let theme = DiagramThemeCompiler::new()
            .compile(spec)
            .expect("fixture theme should compile");
        let prepared = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                theme.font_catalog().clone(),
                FontSourcePolicy::embedded_only(),
            ))
            .expect("fixture catalog should prepare");
        let resolved = theme.resolve(DiagramFamilyId::FLOWCHART);
        (prepared, resolved)
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
    fn native_prepared_label_reuses_layout_geometry_without_legacy_measurement() {
        let (prepared, theme) = native_flowchart_text_fixture();
        let measurer = StatefulOpaqueTraceMeasurer::new();
        let config = MermaidConfig::default();
        let style = TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let work_meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let builder = FlowchartSvgLabelSidecarBuilder::new_with_work_meter(
            Some(&prepared),
            Some(&theme),
            Arc::clone(&work_meter),
        );
        let owner = FlowchartSvgLabelOwner::Node(0);
        let raw_label = "Portable 图 layout";
        let request = || FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label,
            label_type: "text",
            style: &style,
            max_width_px: Some(84.0),
            wrap_mode: WrapMode::SvgLike,
            config: &config,
            math_renderer: None,
        };

        let first = measure_flowchart_svg_label_for_layout(
            Some(&builder),
            Some(owner),
            Some("node"),
            request(),
            FlowchartSvgWidthMode::Bbox,
        );
        let repeated = measure_flowchart_svg_label_for_layout(
            Some(&builder),
            Some(owner),
            Some("node"),
            request(),
            FlowchartSvgWidthMode::Bbox,
        );

        assert_eq!(first.width.to_bits(), repeated.width.to_bits());
        assert_eq!(first.height.to_bits(), repeated.height.to_bits());
        assert_eq!(first.line_count, repeated.line_count);
        assert!(first.width.is_finite() && first.width > 0.0, "{first:?}");
        assert!(first.height.is_finite() && first.height > 0.0, "{first:?}");
        assert_eq!(builder.prepared_count(), 1);
        assert_eq!(builder.prepared_hit_count(), 1);
        assert!(work_meter.used() > 0);
        assert!(measurer.snapshot().is_empty());

        let sidecar = builder.finish();
        assert_eq!(sidecar.prepared_error(), None);
        let plan = FlowchartSvgLabelRenderPlan::new(
            Some(&sidecar),
            sidecar.node_owner("node", false),
            raw_label,
            &measurer,
            &style,
            Some(84.0),
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        assert!(matches!(plan, FlowchartSvgLabelRenderPlan::Prepared { .. }));
        assert!(!plan.wrapped_lines().is_empty());
        let emission_style = plan
            .merge_emission_font_style(Some(
                "font-family:\"trebuchet ms\", verdana, arial, sans-serif",
            ))
            .expect("native prepared label has admitted typography");
        assert!(emission_style.contains("font-family:\"Excalifont\", \"Xiaolai SC\" !important"));
        assert!(!emission_style.to_ascii_lowercase().contains("trebuchet"));
        assert!(!emission_style.to_ascii_lowercase().contains("arial"));
        assert!(measurer.snapshot().is_empty());
    }

    #[test]
    fn native_prepared_swimlane_title_accepts_only_equivalent_plain_markdown() {
        let (prepared, theme) = native_flowchart_text_fixture();
        let measurer = StatefulOpaqueTraceMeasurer::new();
        let config = MermaidConfig::default();
        let style = TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let owner = FlowchartSvgLabelOwner::SwimlaneGroupTitle(0);
        let builder = FlowchartSvgLabelSidecarBuilder::new(Some(&prepared), Some(&theme));

        let metrics = builder.measure_for_layout(
            owner,
            "lane",
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label: "Portable Lane",
                label_type: "markdown",
                style: &style,
                max_width_px: Some(FLOWCHART_FIXED_LABEL_WRAP_WIDTH),
                wrap_mode: WrapMode::SvgLike,
                config: &config,
                math_renderer: None,
            },
            true,
            FlowchartSvgWidthMode::Bbox,
        );

        assert!(metrics.width > 0.0);
        assert!(measurer.snapshot().is_empty());
        let sidecar = builder.finish();
        assert!(sidecar.prepared_error().is_none());
        let plan = FlowchartSvgLabelRenderPlan::new(
            Some(&sidecar),
            sidecar.swimlane_group_title_owner("lane"),
            "Portable Lane",
            &measurer,
            &style,
            Some(FLOWCHART_FIXED_LABEL_WRAP_WIDTH),
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        assert!(plan.is_prepared());
        assert_eq!(plan.plain_text(), "Portable Lane");
        let mut svg = String::new();
        crate::svg::write_flowchart_svg_label_plan_for_test(&mut svg, &plan, true);
        assert!(svg.contains("merman-prepared-swimlane-0"), "{svg}");
        assert!(plan.emitted_admitted_typography());

        for unsupported in ["**Lane**", "[Lane](https://example.invalid)", "<b>Lane</b>"] {
            assert!(
                !markdown_matches_non_markdown_svg_source(unsupported),
                "complex markdown must not enter the plain SVG preparation path: {unsupported}"
            );
        }
    }

    #[test]
    fn node_label_font_size_drives_native_measurement_and_the_consumed_writer() {
        let (default_prepared, default_theme) = native_flowchart_text_fixture();
        let (typed_prepared, typed_theme) =
            native_flowchart_text_fixture_with_node_label_font_size(26.0);
        let measurer = StatefulOpaqueTraceMeasurer::new();
        let config = MermaidConfig::default();
        let style = TextStyle {
            font_family: Some("Excalifont".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let owner = FlowchartSvgLabelOwner::Node(0);
        let raw_label = "Typed size";
        let request = || FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label,
            label_type: "text",
            style: &style,
            max_width_px: None,
            wrap_mode: WrapMode::SvgLike,
            config: &config,
            math_renderer: None,
        };

        let default_builder =
            FlowchartSvgLabelSidecarBuilder::new(Some(&default_prepared), Some(&default_theme));
        let default_metrics = measure_flowchart_svg_label_for_layout(
            Some(&default_builder),
            Some(owner),
            Some("default-node"),
            request(),
            FlowchartSvgWidthMode::Bbox,
        );

        let typed_builder =
            FlowchartSvgLabelSidecarBuilder::new(Some(&typed_prepared), Some(&typed_theme));
        let typed_metrics = measure_flowchart_svg_label_for_layout(
            Some(&typed_builder),
            Some(owner),
            Some("typed-node"),
            request(),
            FlowchartSvgWidthMode::Bbox,
        );
        assert!(typed_metrics.width > default_metrics.width * 1.5);
        assert!(typed_metrics.height > default_metrics.height * 1.5);
        assert!(measurer.snapshot().is_empty());

        let typed_sidecar = typed_builder.finish();
        let plan = FlowchartSvgLabelRenderPlan::new(
            Some(&typed_sidecar),
            typed_sidecar.node_owner("typed-node", false),
            raw_label,
            &measurer,
            &style,
            None,
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        assert!(!plan.emitted_admitted_typography());
        let mut svg = String::new();
        crate::svg::write_flowchart_svg_label_plan_for_test(&mut svg, &plan, true);
        assert!(plan.emitted_admitted_typography());
        assert!(svg.contains("font-size:26px !important"), "{svg}");
    }

    #[test]
    fn native_prepared_label_retention_transfers_to_the_emitted_ledger() {
        let (prepared, theme) = native_flowchart_text_fixture();
        let measurer = StatefulOpaqueTraceMeasurer::new();
        let config = MermaidConfig::default();
        let style = TextStyle::default();
        let work_meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let builder = FlowchartSvgLabelSidecarBuilder::new_with_work_meter(
            Some(&prepared),
            Some(&theme),
            Arc::clone(&work_meter),
        );
        let raw_label = "accounted label";
        let metrics = builder.measure_for_layout(
            FlowchartSvgLabelOwner::Node(0),
            "node",
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label,
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
        assert!(metrics.width > 0.0);
        let sidecar = builder.finish();
        let owner = FlowchartSvgLabelOwner::Node(0);
        let expected_retained_bytes = sidecar
            .prepared
            .get(owner)
            .expect("prepared Flowchart label")
            .retained_bytes()
            .saturating_add(
                sidecar
                    .sources
                    .get(owner)
                    .expect("prepared Flowchart source")
                    .retained_bytes(),
            )
            .saturating_add(FLOWCHART_RENDER_ID_RECORD_BYTES)
            .saturating_add("node".len())
            .saturating_add(FLOWCHART_PREPARED_OWNER_SLOT_BYTES);
        assert_eq!(sidecar.node_owner_by_id.get("node"), Some(&owner));
        assert_eq!(
            work_meter.prepared_text_retained_bytes(),
            expected_retained_bytes
        );

        let plan = FlowchartSvgLabelRenderPlan::new(
            Some(&sidecar),
            sidecar.node_owner("node", false),
            raw_label,
            &measurer,
            &style,
            None,
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        let mut svg = String::new();
        crate::svg::write_flowchart_svg_label_plan_for_test(&mut svg, &plan, true);
        let ledger_bytes = sidecar
            .prepared_text_label_ledger()
            .next()
            .expect("emitted Flowchart ledger entry")
            .retained_bytes();
        let reservations = sidecar.take_prepared_text_retained_reservations();

        assert_eq!(reservations.len(), 1);
        assert_eq!(work_meter.prepared_text_retained_bytes(), ledger_bytes);
        drop(reservations);
        assert_eq!(work_meter.prepared_text_retained_bytes(), 0);
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
                >= std::mem::size_of::<PreparedFlowchartMathLabel>()
                    + FLOWCHART_RENDER_ID_RECORD_BYTES
                    + FLOWCHART_PREPARED_OWNER_SLOT_BYTES
                    + "$$x$$".len()
                    + "Catalog Sans".len()
                    + foreground.value().len()
                    + occurrence_id.as_str().len()
                    + 1
        );
    }

    #[test]
    fn native_prepared_label_retention_rejection_is_structured() {
        let (prepared, theme) = native_flowchart_text_fixture();
        let measurer = StatefulOpaqueTraceMeasurer::new();
        let config = MermaidConfig::default();
        let style = TextStyle::default();
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 1)
            .unwrap();
        let work_meter = Arc::new(OperationWorkMeter::new(policy));
        let builder = FlowchartSvgLabelSidecarBuilder::new_with_work_meter(
            Some(&prepared),
            Some(&theme),
            Arc::clone(&work_meter),
        );

        let metrics = builder.measure_for_layout(
            FlowchartSvgLabelOwner::Node(0),
            "node",
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label: "rejected retained label",
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
        let sidecar = builder.finish();
        let error = sidecar
            .prepared_resource_error()
            .expect("retained-byte rejection remains structured");

        assert_eq!(metrics.width, 0.0);
        assert_eq!(error.limit, "max_prepared_text_retained_bytes");
        assert_eq!(error.max, 1);
        assert!(sidecar.prepared_error().is_none());
        let owner = FlowchartSvgLabelOwner::Node(0);
        assert!(sidecar.sources.get(owner).is_none());
        assert!(sidecar.prepared.get(owner).is_none());
        assert!(!sidecar.node_owner_by_id.contains_key("node"));
        assert_eq!(work_meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn native_prepared_label_emits_only_catalog_admitted_source_families() {
        let (prepared, theme) = native_flowchart_text_fixture();
        let measurer = StatefulOpaqueTraceMeasurer::new();
        let config = MermaidConfig::default();
        let wrapping_style = TextStyle::default();
        let metrics_style = TextStyle {
            font_family: Some("Arial, Excalifont".to_string()),
            font_size: 19.0,
            font_weight: None,
            font_style: None,
        };
        let mut metrics_overrides = PreparedTextCssTypographyOverrides::default();
        metrics_overrides.observe_declaration("font-family", "Arial, Excalifont");
        metrics_overrides.observe_declaration("font-size", "19px");
        let builder = FlowchartSvgLabelSidecarBuilder::new(Some(&prepared), Some(&theme));
        let owner = FlowchartSvgLabelOwner::Node(0);
        let raw_label = "Catalog owned label";

        let metrics = builder.measure_for_layout_with_metrics_style_and_typography_overrides(
            owner,
            "node",
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label,
                label_type: "text",
                style: &wrapping_style,
                max_width_px: Some(120.0),
                wrap_mode: WrapMode::SvgLike,
                config: &config,
                math_renderer: None,
            },
            &metrics_style,
            FlowchartLabelTypographyOverrides::metrics_only(&metrics_overrides),
            true,
            FlowchartSvgWidthMode::Bbox,
        );

        assert!(metrics.width > 0.0);
        let sidecar = builder.finish();
        assert_eq!(sidecar.prepared_error(), None);
        let plan = FlowchartSvgLabelRenderPlan::new_with_metrics_style(
            Some(&sidecar),
            sidecar.node_owner("node", false),
            raw_label,
            &measurer,
            &wrapping_style,
            &metrics_style,
            Some(120.0),
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        let emission_style = plan
            .merge_emission_font_style(Some("font-family:Arial, Excalifont;color:#123456"))
            .expect("native prepared label has admitted typography");
        assert!(emission_style.contains("color:#123456"));
        assert!(emission_style.contains("font-family:\"Excalifont\" !important"));
        assert!(emission_style.contains("font-size:19px !important"));
        assert!(emission_style.contains("font-weight:400 !important"));
        assert!(emission_style.contains("font-style:normal !important"));
        assert!(!emission_style.contains("Arial"));
    }

    #[test]
    fn native_prepared_label_uses_one_source_typography_for_measurement_and_emission() {
        let (prepared, theme) = native_flowchart_text_fixture();
        let measurer = StatefulOpaqueTraceMeasurer::new();
        let config = MermaidConfig::default();
        let style = TextStyle::default();
        let raw_label = "iii iii";

        let baseline_builder = FlowchartSvgLabelSidecarBuilder::new(Some(&prepared), Some(&theme));
        let baseline = baseline_builder.measure_for_layout(
            FlowchartSvgLabelOwner::Node(0),
            "baseline",
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label,
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

        let mut overrides = PreparedTextCssTypographyOverrides::default();
        overrides.observe_declaration("line-height", "2");
        overrides.observe_declaration("letter-spacing", "3px");
        overrides.observe_declaration("word-spacing", "4px");
        overrides.observe_declaration("text-transform", "uppercase");
        let builder = FlowchartSvgLabelSidecarBuilder::new(Some(&prepared), Some(&theme));
        let metrics = builder.measure_for_layout_with_typography_overrides(
            FlowchartSvgLabelOwner::Node(1),
            "styled",
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label,
                label_type: "text",
                style: &style,
                max_width_px: None,
                wrap_mode: WrapMode::SvgLike,
                config: &config,
                math_renderer: None,
            },
            FlowchartLabelTypographyOverrides::same(&overrides),
            true,
            FlowchartSvgWidthMode::Bbox,
        );

        assert!(metrics.width > baseline.width + 5.0);
        assert!(metrics.height > baseline.height);
        let sidecar = builder.finish();
        let plan = FlowchartSvgLabelRenderPlan::new(
            Some(&sidecar),
            sidecar.node_owner("styled", false),
            raw_label,
            &measurer,
            &style,
            None,
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        assert_eq!(
            plan.wrapped_lines().as_ref(),
            &[vec!["III".to_string(), "III".to_string()]]
        );
        let emission_style = plan
            .merge_emission_font_style(Some(
                "line-height:1;letter-spacing:0;word-spacing:0;text-transform:lowercase;color:#123456",
            ))
            .expect("native prepared label has admitted typography");
        assert!(emission_style.contains("line-height:2 !important"));
        assert!(emission_style.contains("letter-spacing:3px !important"));
        assert!(emission_style.contains("word-spacing:4px !important"));
        assert!(emission_style.contains("color:#123456"));
        assert!(!emission_style.contains("text-transform"));
        assert!(measurer.snapshot().is_empty());
    }

    #[test]
    fn native_prepared_label_stops_after_the_first_terminal_error() {
        let (prepared, theme) = native_flowchart_text_fixture();
        let measurer = StatefulOpaqueTraceMeasurer::new();
        let config = MermaidConfig::default();
        let style = TextStyle::default();
        let builder = FlowchartSvgLabelSidecarBuilder::new(Some(&prepared), Some(&theme));

        let rejected = builder.measure_for_layout(
            FlowchartSvgLabelOwner::Node(0),
            "rejected",
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label: "unsupported source",
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
        let skipped = builder.measure_for_layout(
            FlowchartSvgLabelOwner::Node(1),
            "skipped",
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label: "otherwise valid",
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

        assert_eq!(rejected.width, 0.0);
        assert_eq!(skipped.width, 0.0);
        assert_eq!(builder.prepared_count(), 0);
        assert!(matches!(
            builder.finish().prepared_error(),
            Some(TextLayoutError::UnsupportedPreparedTextPath(
                "flowchart_label"
            ))
        ));
        assert!(measurer.snapshot().is_empty());
    }

    #[test]
    fn native_prepared_label_materializes_transform_and_preserves_authored_entities() {
        let (prepared, theme) =
            native_flowchart_text_fixture_with_transform(TextTransform::Uppercase);
        let measurer = StatefulOpaqueTraceMeasurer::new();
        let config = MermaidConfig::default();
        let style = TextStyle::default();
        let builder = FlowchartSvgLabelSidecarBuilder::new(Some(&prepared), Some(&theme));
        let owner = FlowchartSvgLabelOwner::Node(0);
        let raw_label = "straße &amp; ß";

        let metrics = measure_flowchart_svg_label_for_layout(
            Some(&builder),
            Some(owner),
            Some("node"),
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label,
                label_type: "text",
                style: &style,
                max_width_px: None,
                wrap_mode: WrapMode::SvgLike,
                config: &config,
                math_renderer: None,
            },
            FlowchartSvgWidthMode::Bbox,
        );

        assert!(metrics.width.is_finite() && metrics.width > 0.0);
        assert!(measurer.snapshot().is_empty());
        let sidecar = builder.finish();
        let plan = FlowchartSvgLabelRenderPlan::new(
            Some(&sidecar),
            sidecar.node_owner("node", false),
            raw_label,
            &measurer,
            &style,
            None,
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        assert_eq!(
            plan.wrapped_lines().as_ref(),
            &[vec![
                "STRASSE".to_string(),
                "&amp;".to_string(),
                "SS".to_string(),
            ]]
        );
    }

    #[test]
    fn native_prepared_narrow_wrap_keeps_entity_source_atomic() {
        let (prepared, theme) = native_flowchart_text_fixture();
        let measurer = StatefulOpaqueTraceMeasurer::new();
        let config = MermaidConfig::default();
        let style = TextStyle::default();
        let builder = FlowchartSvgLabelSidecarBuilder::new(Some(&prepared), Some(&theme));
        let owner = FlowchartSvgLabelOwner::Node(0);
        let raw_label = "a &amp; b";

        let metrics = measure_flowchart_svg_label_for_layout(
            Some(&builder),
            Some(owner),
            Some("node"),
            FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label,
                label_type: "text",
                style: &style,
                max_width_px: Some(1.0),
                wrap_mode: WrapMode::SvgLike,
                config: &config,
                math_renderer: None,
            },
            FlowchartSvgWidthMode::Bbox,
        );

        assert!(metrics.line_count >= 3);
        let sidecar = builder.finish();
        assert_eq!(sidecar.prepared_error(), None);
        let plan = FlowchartSvgLabelRenderPlan::new(
            Some(&sidecar),
            sidecar.node_owner("node", false),
            raw_label,
            &measurer,
            &style,
            Some(1.0),
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        assert_eq!(
            plan.wrapped_lines().as_ref(),
            &[
                vec!["a".to_string()],
                vec!["&amp;".to_string()],
                vec!["b".to_string()],
            ]
        );
    }

    #[test]
    fn native_prepared_svg_emission_consumes_only_the_emitted_label_token() {
        let (prepared, theme) = native_flowchart_text_fixture();
        let measurer = StatefulOpaqueTraceMeasurer::new();
        let config = MermaidConfig::default();
        let style = TextStyle::default();
        let builder = FlowchartSvgLabelSidecarBuilder::new(Some(&prepared), Some(&theme));

        for (owner, render_id, raw_label) in [
            (FlowchartSvgLabelOwner::Node(7), "node", "alpha"),
            (FlowchartSvgLabelOwner::Edge(2), "edge", "beta"),
        ] {
            let metrics = measure_flowchart_svg_label_for_layout(
                Some(&builder),
                Some(owner),
                Some(render_id),
                FlowchartLabelMetricsRequest {
                    measurer: &measurer,
                    raw_label,
                    label_type: "text",
                    style: &style,
                    max_width_px: None,
                    wrap_mode: WrapMode::SvgLike,
                    config: &config,
                    math_renderer: None,
                },
                FlowchartSvgWidthMode::Bbox,
            );
            assert!(metrics.width > 0.0);
        }

        let sidecar = builder.finish();
        assert_eq!(sidecar.prepared_text_label_ledger().count(), 0);
        let node_plan = FlowchartSvgLabelRenderPlan::new(
            Some(&sidecar),
            sidecar.node_owner("node", false),
            "alpha",
            &measurer,
            &style,
            None,
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        assert_eq!(node_plan.wrapped_lines().len(), 1);
        assert!(!node_plan.emitted_admitted_typography());
        assert_eq!(sidecar.prepared_text_label_ledger().count(), 0);

        let mut svg = String::new();
        crate::svg::write_flowchart_svg_label_plan_for_test(&mut svg, &node_plan, true);
        assert!(node_plan.emitted_admitted_typography());
        assert!(
            svg.contains("<text y=\"-10.1\" style=\"font-family:&quot;Excalifont&quot;, &quot;",)
                && svg.contains(r#"id="merman-prepared-flowchart-0""#),
            "{svg}"
        );
        assert_eq!(svg.matches("font-family:").count(), 1, "{svg}");
        assert!(
            svg.contains(r#"<tspan class="text-inner-tspan">alpha</tspan>"#),
            "{svg}"
        );
        let ledger = sidecar.prepared_text_label_ledger().collect::<Vec<_>>();
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger[0].id().key(), 0);
        assert_eq!(ledger[0].id().family(), "flowchart");

        let fallback = FlowchartSvgLabelRenderPlan::new(
            None,
            None,
            "fallback",
            &measurer,
            &style,
            None,
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        assert!(!fallback.emitted_admitted_typography());
        let mut fallback_svg = String::new();
        crate::svg::write_flowchart_svg_label_plan_for_test(&mut fallback_svg, &fallback, true);
        assert!(!fallback.emitted_admitted_typography());
        assert!(!fallback_svg.contains("merman-prepared-"), "{fallback_svg}");
    }

    fn opaque_roundtrip_trace(
        with_sidecar: bool,
    ) -> (TextMetrics, Vec<Vec<String>>, Vec<OpaqueMeasurementCall>) {
        let measurer = StatefulOpaqueTraceMeasurer::new();
        let config = MermaidConfig::default();
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
        let config = MermaidConfig::default();
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
        let config = MermaidConfig::default();
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
        let config = MermaidConfig::default();
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
        };
        let config = MermaidConfig::default();
        let style = TextStyle::default();
        let builder = FlowchartSvgLabelSidecarBuilder::default();
        let owner = FlowchartSvgLabelOwner::Node(0);
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
        );

        assert!(measurer.wrapped_calls.get() > 0);
        assert_eq!(builder.prepared_count(), 0);
        let sidecar = builder.finish();
        let calls_before_render = measurer.wrapped_calls.get();
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
        let _ = plan.wrapped_lines();
        assert_eq!(
            measurer.wrapped_calls.get(),
            calls_before_render,
            "the computed-length carrier owns this route; no opaque wrapped callback is expected"
        );
    }

    #[test]
    fn duplicate_subgraph_ids_resolve_to_the_first_semantic_owner() {
        let environment = RenderEnvironment::deterministic();
        let session = environment.begin_session().expect("deterministic session");
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let config = MermaidConfig::default();
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
        let config = MermaidConfig::default();
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
        let config = MermaidConfig::default();
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
        let config = MermaidConfig::default();
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
