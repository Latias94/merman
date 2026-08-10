//! Operation-local preparation for non-Markdown Flowchart SVG labels.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
#[cfg(test)]
use std::sync::Mutex;

use rustc_hash::FxHashMap;

use crate::diagram_theme::{
    ResolvedDiagramTheme, TextTransform as ThemeTextTransform, ThemeTarget, ThemeTextStyle,
    ThemeVariant,
};
use crate::environment::{BuiltinTextMeasurementOperationCarrier, TextMeasurementOperation};
use crate::text::{
    PendingPreparedTextLabelLedgerEntry, PrepareTextRequest, PreparedTextLabelFamily,
    PreparedTextLabelId, PreparedTextLabelLedgerEntry, PreparedTextLayout, PreparedTextWrap,
    TextLayoutError, TextMeasurer, TextMetrics, TextStyle, WrapMode,
    merge_prepared_text_typography,
};

use super::label::{
    FlowchartLabelMetricsRequest, FlowchartSvgLabelSource, FlowchartSvgWidthMode,
    flowchart_label_metrics_for_layout,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum FlowchartSvgLabelOwner {
    Node(usize),
    EmptySubgraphNode(usize),
    Edge(usize),
    SubgraphTitle(usize),
    SwimlaneNode(usize),
    SwimlaneEdgeLabel(usize),
}

impl FlowchartSvgLabelOwner {
    fn semantic_index(self) -> usize {
        match self {
            Self::Node(index)
            | Self::EmptySubgraphNode(index)
            | Self::Edge(index)
            | Self::SubgraphTitle(index)
            | Self::SwimlaneNode(index)
            | Self::SwimlaneEdgeLabel(index) => index,
        }
    }

    fn prepared_text_family(self) -> PreparedTextLabelFamily {
        match self {
            Self::SwimlaneNode(_) | Self::SwimlaneEdgeLabel(_) => PreparedTextLabelFamily::Swimlane,
            Self::Node(_) | Self::EmptySubgraphNode(_) | Self::Edge(_) | Self::SubgraphTitle(_) => {
                PreparedTextLabelFamily::Flowchart
            }
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
    }

    fn iter(&self) -> impl Iterator<Item = &T> {
        [
            &self.nodes,
            &self.empty_subgraph_nodes,
            &self.edges,
            &self.subgraph_titles,
            &self.swimlane_nodes,
            &self.swimlane_edge_labels,
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
    pending_label_entry: Option<PendingPreparedTextLabelLedgerEntry>,
    label_entry: Option<PreparedTextLabelLedgerEntry>,
    label_consumed: Cell<bool>,
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
            pending_label_entry: None,
            label_entry: None,
            label_consumed: Cell::new(false),
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
}

/// Mutable preparation state used only while one Flowchart family artifact is laid out.
///
/// Pure source projections are retained for every eligible label. Measured results are retained
/// only when both operations resolve directly to built-in routes, so host callbacks, host
/// fallback, and opaque custom measurers keep their original layout-to-render request sequence and
/// measurement report while avoiding repeated tokenization.
#[derive(Debug, Default)]
pub(crate) struct FlowchartSvgLabelSidecarBuilder {
    pending: RefCell<PendingFlowchartSvgLabels>,
    prepared_text_layout: Option<PreparedTextLayout>,
    resolved_theme: Option<ResolvedDiagramTheme>,
    prepared_error: RefCell<Option<TextLayoutError>>,
    #[cfg(test)]
    prepared_hits: Cell<usize>,
    #[cfg(test)]
    source_plans_by_owner: RefCell<FxHashMap<FlowchartSvgLabelOwner, usize>>,
}

#[derive(Debug, Default)]
struct PendingFlowchartSvgLabels {
    sources: FlowchartSvgLabelSlots<FlowchartSvgLabelSourceEntry>,
    prepared: FlowchartSvgLabelSlots<PreparedFlowchartSvgLabel>,
    render_ids: FlowchartSvgLabelSlots<Box<str>>,
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

    fn record_prepared_error(&self, error: TextLayoutError) {
        let mut prepared_error = self.prepared_error.borrow_mut();
        if prepared_error.is_none() {
            *prepared_error = Some(error);
        }
    }

    pub(crate) fn measure_for_layout(
        &self,
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: FlowchartLabelMetricsRequest<'_>,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> TextMetrics {
        let metrics_style = request.style;
        self.measure_for_layout_with_metrics_style(
            owner,
            render_id,
            request,
            metrics_style,
            break_long_words,
            width_mode,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn measure_for_layout_with_metrics_style(
        &self,
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: FlowchartLabelMetricsRequest<'_>,
        metrics_style: &TextStyle,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> TextMetrics {
        if !supports_svg_source_preparation(&request) {
            if self.prepared_text_layout.is_some() {
                self.record_prepared_error(TextLayoutError::UnsupportedPreparedTextPath(
                    "flowchart_label",
                ));
                return TextMetrics {
                    width: 0.0,
                    height: 0.0,
                    line_count: 0,
                };
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
                break_long_words,
                width_mode,
                native_binding,
            ) {
                Ok(metrics) => return metrics,
                Err(error) => {
                    self.record_prepared_error(error);
                    return TextMetrics {
                        width: 0.0,
                        height: 0.0,
                        line_count: 0,
                    };
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
                PreparedFlowchartSvgLabel::new(binding.into_owned(), wrapped_lines, metrics),
            );
        } else {
            pending.prepared.remove(owner);
        }
        metrics
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_native_label(
        &self,
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: FlowchartLabelMetricsRequest<'_>,
        metrics_style: &TextStyle,
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
        let (wrapping_typography, metrics_typography) =
            native_typography_pair(theme, owner, request.style, metrics_style)?;
        if source.source.plain_text().is_empty() {
            let mut pending = self.pending.borrow_mut();
            pending.render_ids.insert(owner, render_id.into());
            pending.sources.insert(owner, source);
            pending.prepared.insert(
                owner,
                PreparedFlowchartSvgLabel::new(
                    binding.into_owned(),
                    Vec::new(),
                    TextMetrics {
                        width: 0.0,
                        height: 0.0,
                        line_count: 0,
                    },
                ),
            );
            return Ok(TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            });
        }

        let source_projection = source
            .source
            .prepared_projection(wrapping_typography.transform())?;
        let wrapping_typography = wrapping_typography.with_transform(ThemeTextTransform::None);
        let metrics_typography = metrics_typography.with_transform(ThemeTextTransform::None);

        let prepared = layout.prepare_text(
            &PrepareTextRequest::new(source_projection.visible_text(), wrapping_typography)
                .with_metrics_typography(metrics_typography)
                .with_family_normalized_projection()
                .with_wrap(PreparedTextWrap::SvgLike {
                    max_width_px: request.max_width_px,
                    break_long_words,
                }),
        )?;
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

        let mut pending = self.pending.borrow_mut();
        pending.render_ids.insert(owner, render_id.into());
        pending.sources.insert(owner, source);
        pending.prepared.insert(
            owner,
            PreparedFlowchartSvgLabel::new(binding.into_owned(), wrapped_lines, metrics)
                .with_label_entry(label_entry),
        );
        Ok(metrics)
    }

    pub(crate) fn finish(self) -> FlowchartSvgLabelSidecar {
        #[cfg(test)]
        let source_plans_by_owner = self.source_plans_by_owner.into_inner();
        let pending = self.pending.into_inner();
        FlowchartSvgLabelSidecar::new(
            pending.sources,
            pending.prepared,
            pending.render_ids,
            self.prepared_error.into_inner(),
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

pub(crate) fn measure_flowchart_svg_label_for_layout(
    sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    owner: Option<FlowchartSvgLabelOwner>,
    render_id: Option<&str>,
    request: FlowchartLabelMetricsRequest<'_>,
    width_mode: FlowchartSvgWidthMode,
) -> TextMetrics {
    match sidecar.zip(owner).zip(render_id) {
        Some(((sidecar, owner), render_id)) => {
            sidecar.measure_for_layout(owner, render_id, request, true, width_mode)
        }
        None => measure_svg_label_without_sidecar(request, width_mode),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn measure_flowchart_svg_label_for_layout_with_metrics_style(
    sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    owner: Option<FlowchartSvgLabelOwner>,
    render_id: Option<&str>,
    request: FlowchartLabelMetricsRequest<'_>,
    metrics_style: &TextStyle,
    width_mode: FlowchartSvgWidthMode,
) -> TextMetrics {
    match sidecar.zip(owner).zip(render_id) {
        Some(((sidecar, owner), render_id)) => sidecar.measure_for_layout_with_metrics_style(
            owner,
            render_id,
            request,
            metrics_style,
            true,
            width_mode,
        ),
        None => {
            measure_svg_label_without_sidecar_with_metrics_style(request, metrics_style, width_mode)
        }
    }
}

fn supports_svg_source_preparation(request: &FlowchartLabelMetricsRequest<'_>) -> bool {
    request.wrap_mode == WrapMode::SvgLike && request.label_type != "markdown"
}

fn native_typography_pair(
    theme: &ResolvedDiagramTheme,
    owner: FlowchartSvgLabelOwner,
    wrap_style: &TextStyle,
    metrics_style: &TextStyle,
) -> Result<(ThemeTextStyle, ThemeTextStyle), TextLayoutError> {
    let target = match owner {
        FlowchartSvgLabelOwner::Node(_)
        | FlowchartSvgLabelOwner::EmptySubgraphNode(_)
        | FlowchartSvgLabelOwner::SwimlaneNode(_) => ThemeTarget::NodeLabel,
        FlowchartSvgLabelOwner::Edge(_) | FlowchartSvgLabelOwner::SwimlaneEdgeLabel(_) => {
            ThemeTarget::EdgeLabel
        }
        FlowchartSvgLabelOwner::SubgraphTitle(_) => ThemeTarget::ClusterLabel,
    };
    let base = theme
        .style(target, ThemeVariant::Default, None)
        .typography()
        .clone();
    Ok((
        merge_prepared_text_typography(&base, wrap_style)?,
        merge_prepared_text_typography(&base, metrics_style)?,
    ))
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
    if !supports_svg_source_preparation(&request) {
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
    node_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    empty_subgraph_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    edge_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    subgraph_title_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    swimlane_node_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    swimlane_edge_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    prepared_error: Option<TextLayoutError>,
    #[cfg(test)]
    prepared_hits_by_owner: Mutex<FxHashMap<FlowchartSvgLabelOwner, usize>>,
    #[cfg(test)]
    source_plans_by_owner: Mutex<FxHashMap<FlowchartSvgLabelOwner, usize>>,
}

impl FlowchartSvgLabelSidecar {
    fn new(
        sources: FlowchartSvgLabelSlots<FlowchartSvgLabelSourceEntry>,
        mut prepared: FlowchartSvgLabelSlots<PreparedFlowchartSvgLabel>,
        render_ids: FlowchartSvgLabelSlots<Box<str>>,
        mut prepared_error: Option<TextLayoutError>,
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
            prepared_error,
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
            }
        });
        sidecar
    }

    pub(crate) fn prepared_error(&self) -> Option<&TextLayoutError> {
        self.prepared_error.as_ref()
    }

    pub(crate) fn prepared_text_label_ledger(
        &self,
    ) -> impl Iterator<Item = &PreparedTextLabelLedgerEntry> {
        self.prepared
            .iter()
            .filter_map(PreparedFlowchartSvgLabel::consumed_label_entry)
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

    pub(crate) fn prepared_text_label_id(&self) -> Option<PreparedTextLabelId> {
        match self {
            Self::Prepared { measured, .. } => measured.label_id_for_emission(),
            Self::Source { .. } => None,
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

    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontAssetSpec, FontCatalogSpec, FontSourcePolicy,
        FontStack, TextTransform, ThemeAssets, TypographySpec,
    };
    use crate::environment::{RenderEnvironment, TextMeasurementPhase};
    use crate::render_family::RenderFamilyKind;
    use crate::text::{
        NativeTextLayoutBackend, PrepareCatalogRequest, TextMeasurer, TextMetrics, TextStyle,
        WrapMode,
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

    fn native_flowchart_text_fixture() -> (PreparedTextLayout, ResolvedDiagramTheme) {
        native_flowchart_text_fixture_with_transform(TextTransform::None)
    }

    fn native_flowchart_text_fixture_with_transform(
        transform: TextTransform,
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
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_typography(TypographySpec::default().with_default(typography))
                    .with_assets(ThemeAssets::default().with_font_catalog(catalog)),
            )
            .expect("fixture theme should compile");
        let prepared = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                theme.font_catalog().clone(),
                FontSourcePolicy::embedded_only(),
            ))
            .expect("fixture catalog should prepare");
        let resolved = theme.resolve(RenderFamilyKind::Flowchart);
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
        let builder = FlowchartSvgLabelSidecarBuilder::new(Some(&prepared), Some(&theme));
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
        assert!(measurer.snapshot().is_empty());
    }

    #[test]
    fn native_prepared_label_shapes_visible_text_and_reprojects_authored_source() {
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
                "straße".to_string(),
                "&amp;".to_string(),
                "ß".to_string(),
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
        assert_eq!(sidecar.prepared_text_label_ledger().count(), 0);

        let mut svg = String::new();
        crate::svg::write_flowchart_svg_label_plan_for_test(&mut svg, &node_plan, true);
        assert!(
            svg.contains(r#"<text y="-10.1" style="" id="merman-prepared-flowchart-0">"#),
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
        let mut fallback_svg = String::new();
        crate::svg::write_flowchart_svg_label_plan_for_test(&mut fallback_svg, &fallback, true);
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
