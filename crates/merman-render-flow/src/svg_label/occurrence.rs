//! Retained label occurrences consumed by Flow geometry and SVG terminals.

#[cfg(any(test, feature = "test-support"))]
use std::sync::Mutex;
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    sync::Arc,
};

use merman_model::family::DiagramFamilyId;
use merman_render_core::{
    html::{HtmlOutputError, HtmlSanitizeSink},
    math::{
        ConfiguredMathBackend, MathPreparationOutcome, MathPreparationUnavailable,
        PrepareMathLabelRequest, PreparedMathEvidenceLease, PreparedMathExpectation,
        PreparedMathLabel, PreparedMathOccurrenceId,
    },
    resources::{
        OperationWorkError, OperationWorkMeter, PreparedTextRetainedReservation,
        RenderResourcePolicy, ResourceLimitExceeded,
    },
    text::{
        TextMeasurer, TextMetrics, TextStyle, WrapMode,
        create_text_html::{
            FlowchartLabelMetricsRequest, flowchart_label_html_source,
            flowchart_label_sanitize_config,
        },
        literal_label::{FlowchartSvgLabelSource, FlowchartSvgWidthMode},
    },
};
use rustc_hash::FxHashMap;

use super::{
    FLOWCHART_PREPARED_OWNER_SLOT_BYTES, FlowchartNodeSourceTypography,
    FlowchartSvgLabelMeasurementRequest, FlowchartSvgLabelSourceEntry,
    FlowchartSvgLabelWrapBindingRequest, PreparedFlowchartSvgLabel, normalized_label_width_bits,
};
use crate::{
    layout_config::FlowchartBaseTypographyPlan, node_geometry::PreparedFlowchartNodeGeometry,
    style::FlowchartTerminalForeground,
};

pub const FLOWCHART_RENDER_ID_RECORD_BYTES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FlowchartSvgLabelOwner {
    Node(usize),
    EmptySubgraphNode(usize),
    Edge(usize),
    SubgraphTitle(usize),
    SwimlaneNode(usize),
    SwimlaneEdgeLabel(usize),
    SwimlaneGroupTitle(usize),
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

    pub fn prepared_math_occurrence_id(self) -> PreparedMathOccurrenceId {
        match self {
            Self::Node(index) => {
                PreparedMathOccurrenceId::indexed(DiagramFamilyId::FLOWCHART, "node-label", index)
            }
            Self::EmptySubgraphNode(index) => PreparedMathOccurrenceId::indexed(
                DiagramFamilyId::FLOWCHART,
                "empty-subgraph-node-label",
                index,
            ),
            Self::Edge(index) => {
                PreparedMathOccurrenceId::indexed(DiagramFamilyId::FLOWCHART, "edge-label", index)
            }
            Self::SubgraphTitle(index) => PreparedMathOccurrenceId::indexed(
                DiagramFamilyId::FLOWCHART,
                "subgraph-title",
                index,
            ),
            Self::SwimlaneNode(index) => {
                PreparedMathOccurrenceId::indexed(DiagramFamilyId::SWIMLANE, "node-label", index)
            }
            Self::SwimlaneEdgeLabel(index) => {
                PreparedMathOccurrenceId::indexed(DiagramFamilyId::SWIMLANE, "edge-label", index)
            }
            Self::SwimlaneGroupTitle(index) => {
                PreparedMathOccurrenceId::indexed(DiagramFamilyId::SWIMLANE, "group-title", index)
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

    #[cfg(any(test, feature = "test-support"))]
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
pub struct FlowchartSvgLabelSlots<T> {
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
    pub fn get(&self, owner: FlowchartSvgLabelOwner) -> Option<&T> {
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

    pub fn insert(&mut self, owner: FlowchartSvgLabelOwner, value: T) {
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

    pub fn remove(&mut self, owner: FlowchartSvgLabelOwner) -> Option<T> {
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

    #[cfg(any(test, feature = "test-support"))]
    pub fn entry_count(&self) -> usize {
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

#[derive(Debug, Default)]
pub struct PendingFlowchartSvgLabels {
    pub node_source_typography: FlowchartSvgLabelSlots<Arc<FlowchartNodeSourceTypography>>,
    pub node_geometry: FlowchartSvgLabelSlots<PreparedFlowchartNodeGeometry>,
    pub sources: FlowchartSvgLabelSlots<FlowchartSvgLabelSourceEntry>,
    pub prepared: FlowchartSvgLabelSlots<PreparedFlowchartSvgLabel>,
    pub math: FlowchartSvgLabelSlots<PreparedFlowchartMathLabel>,
    pub html: FlowchartSvgLabelSlots<PreparedFlowchartHtmlLabel>,
    pub render_ids: FlowchartSvgLabelSlots<Box<str>>,
    pub render_id_reservations: FlowchartSvgLabelSlots<PreparedTextRetainedReservation>,
}

#[derive(Debug)]
pub struct PreparedFlowchartHtmlLabel {
    source: Box<str>,
    label_type: Box<str>,
    xhtml: String,
    _retained_reservation: Option<PreparedTextRetainedReservation>,
}

impl PreparedFlowchartHtmlLabel {
    pub fn prepare(
        request: &FlowchartLabelMetricsRequest<'_>,
        work_meter: Option<&Arc<OperationWorkMeter>>,
    ) -> Result<Self, OperationWorkError> {
        let resolved_config = flowchart_label_sanitize_config(request.config);
        let owner_bytes = std::mem::size_of::<PreparedFlowchartHtmlLabel>()
            .saturating_add(FLOWCHART_PREPARED_OWNER_SLOT_BYTES)
            .saturating_add(request.raw_label.len())
            .saturating_add(request.label_type.len());
        let trusted_meter;
        let work = match work_meter.map(Arc::as_ref) {
            Some(work) => work,
            None => {
                trusted_meter =
                    OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
                &trusted_meter
            }
        };
        work.preflight_prepared_text_retained_bytes(owner_bytes)?;
        work.charge(1usize.saturating_add(request.raw_label.len() / 64))?;
        let max = work
            .remaining_prepared_text_retained_bytes()
            .map_or(usize::MAX, |remaining| {
                remaining.saturating_sub(owner_bytes)
            });
        let parse_work = Cell::new(0);
        let sink =
            HtmlSanitizeSink::new(max, work, &parse_work, merman_model::OperationPhase::Layout);
        let html =
            flowchart_label_html_source(request.raw_label, request.label_type, request.config);
        let xhtml = match merman_markup::sanitize::sanitize_xhtml_with_sink(
            &html,
            resolved_config.as_ref(),
            &sink,
        ) {
            Ok(xhtml) => xhtml,
            Err(merman_markup::sanitize::SanitizeFailure::Output(HtmlOutputError::Operation(
                error,
            ))) => {
                return Err(error);
            }
            Err(merman_markup::sanitize::SanitizeFailure::Output(HtmlOutputError::Limit {
                attempted,
            })) => {
                work.preflight_prepared_text_retained_bytes(owner_bytes.saturating_add(attempted))?;
                String::new()
            }
            // Preserve the string sanitizer's fail-closed compatibility behavior.
            Err(_) => String::new(),
        };
        let retained_bytes = owner_bytes.saturating_add(xhtml.len());
        let reservation = work_meter
            .map(|work| work.reserve_prepared_text_retained_bytes(retained_bytes))
            .transpose()?;
        Ok(Self {
            source: request.raw_label.into(),
            label_type: request.label_type.into(),
            xhtml,
            _retained_reservation: reservation,
        })
    }

    pub fn xhtml(&self) -> &str {
        &self.xhtml
    }

    pub fn matches(&self, source: &str, label_type: &str) -> bool {
        self.source.as_ref() == source && self.label_type.as_ref() == label_type
    }
}

#[derive(Debug)]
pub struct PreparedFlowchartMathLabel {
    occurrence_id: PreparedMathOccurrenceId,
    source: Arc<str>,
    style: TextStyle,
    foreground: FlowchartTerminalForeground,
    max_width_bits: Option<u64>,
    wrap_mode: WrapMode,
    outcome: MathPreparationOutcome,
    expected_terminal_emissions: usize,
    retained_reservation: RefCell<Option<PreparedTextRetainedReservation>>,
}

impl PreparedFlowchartMathLabel {
    #[allow(
        clippy::too_many_arguments,
        reason = "Bind the original Flow label occurrence, effective style, math backend, and operation meter"
    )]
    pub fn prepare(
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: &FlowchartLabelMetricsRequest<'_>,
        metrics_style: &TextStyle,
        foreground: &FlowchartTerminalForeground,
        has_source_typography: bool,
        backend: Option<&ConfiguredMathBackend>,
        work_meter: Option<&Arc<OperationWorkMeter>>,
    ) -> Result<Option<Self>, OperationWorkError> {
        let Some(work_meter) = work_meter else {
            return Ok(None);
        };
        let recorded_id_bytes = if has_source_typography {
            FLOWCHART_RENDER_ID_RECORD_BYTES.saturating_add(render_id.len())
        } else {
            0
        };
        let owner_retained_bytes =
            Self::owner_retained_bytes(request.raw_label, metrics_style, foreground, render_id)
                .saturating_sub(recorded_id_bytes);
        let occurrence_id = owner.prepared_math_occurrence_id();
        let outcome = match backend {
            Some(backend) => backend.prepare(
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
                work_meter,
            )?,
            None => {
                MathPreparationOutcome::Unavailable(MathPreparationUnavailable::BackendUnavailable)
            }
        };
        let retained_bytes = Self::retained_bytes_without_reservation(
            request.raw_label,
            metrics_style,
            foreground,
            render_id,
            &occurrence_id,
            &outcome,
        )
        .saturating_sub(recorded_id_bytes);
        let reservation = work_meter.reserve_prepared_text_retained_bytes(retained_bytes)?;
        Ok(Some(Self {
            occurrence_id,
            source: request.raw_label.into(),
            style: metrics_style.clone(),
            foreground: foreground.clone(),
            max_width_bits: normalized_label_width_bits(request.max_width_px),
            wrap_mode: request.wrap_mode,
            outcome,
            // One semantic Flowchart/Swimlane label owner maps to one terminal label.
            expected_terminal_emissions: 1,
            retained_reservation: RefCell::new(Some(reservation)),
        }))
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn outcome(&self) -> &MathPreparationOutcome {
        &self.outcome
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn foreground(&self) -> &FlowchartTerminalForeground {
        &self.foreground
    }

    pub fn matches(
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
            && self.max_width_bits == normalized_label_width_bits(max_width_px)
            && self.wrap_mode == wrap_mode
    }

    pub fn owner_retained_bytes(
        source: &str,
        style: &TextStyle,
        foreground: &FlowchartTerminalForeground,
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

    pub fn retained_bytes_without_reservation(
        source: &str,
        style: &TextStyle,
        foreground: &FlowchartTerminalForeground,
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

    fn terminal_expectation(&self) -> Option<PreparedMathExpectation> {
        let expected_emissions = self.expected_terminal_emissions;
        if expected_emissions == 0 {
            return None;
        }
        match &self.outcome {
            MathPreparationOutcome::Prepared(prepared) => prepared.expectation(expected_emissions),
            MathPreparationOutcome::Unavailable(MathPreparationUnavailable::NotMath) => None,
            MathPreparationOutcome::Unavailable(_) => Some(PreparedMathExpectation::unavailable(
                self.occurrence_id.clone(),
                expected_emissions,
            )),
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

#[derive(Debug, Clone, Copy, Default)]
pub enum FlowchartPreparedLabelResolution<'a> {
    #[default]
    NotPrepared,
    PreparedMath(&'a PreparedMathLabel),
    PreparedHtml(&'a PreparedFlowchartHtmlLabel),
    Unavailable(MathPreparationUnavailable),
}

/// Immutable render-side index. It is private to a single prepared Flowchart family artifact.
#[derive(Debug, Default)]
pub struct FlowchartSvgLabelSidecar {
    _render_id_reservations: FlowchartSvgLabelSlots<PreparedTextRetainedReservation>,
    node_source_typography: FlowchartSvgLabelSlots<Arc<FlowchartNodeSourceTypography>>,
    node_geometry: FlowchartSvgLabelSlots<PreparedFlowchartNodeGeometry>,
    sources: FlowchartSvgLabelSlots<FlowchartSvgLabelSourceEntry>,
    prepared: FlowchartSvgLabelSlots<PreparedFlowchartSvgLabel>,
    math: FlowchartSvgLabelSlots<PreparedFlowchartMathLabel>,
    html: FlowchartSvgLabelSlots<PreparedFlowchartHtmlLabel>,
    base_typography: Option<FlowchartBaseTypographyPlan>,
    node_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    empty_subgraph_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    #[cfg(any(test, feature = "test-support"))]
    edge_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    subgraph_title_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    swimlane_node_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    #[cfg(any(test, feature = "test-support"))]
    swimlane_edge_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    swimlane_group_title_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    prepared_resource_error: Option<ResourceLimitExceeded>,
    prepared_work_error: Option<OperationWorkError>,
    #[cfg(any(test, feature = "test-support"))]
    prepared_hits_by_owner: Mutex<FxHashMap<FlowchartSvgLabelOwner, usize>>,
    #[cfg(any(test, feature = "test-support"))]
    source_plans_by_owner: Mutex<FxHashMap<FlowchartSvgLabelOwner, usize>>,
}

impl FlowchartSvgLabelSidecar {
    pub fn new(
        pending: PendingFlowchartSvgLabels,
        base_typography: Option<FlowchartBaseTypographyPlan>,
        prepared_resource_error: Option<ResourceLimitExceeded>,
        prepared_work_error: Option<OperationWorkError>,
    ) -> Self {
        let PendingFlowchartSvgLabels {
            node_source_typography,
            node_geometry,
            sources,
            prepared,
            math,
            html,
            render_ids,
            render_id_reservations,
        } = pending;
        let mut sidecar = Self {
            _render_id_reservations: render_id_reservations,
            node_source_typography,
            node_geometry,
            sources,
            prepared,
            math,
            html,
            base_typography,
            prepared_resource_error,
            prepared_work_error,
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
                    #[cfg(any(test, feature = "test-support"))]
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
                    #[cfg(any(test, feature = "test-support"))]
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

    pub fn prepared_resource_error(&self) -> Option<&ResourceLimitExceeded> {
        self.prepared_resource_error.as_ref()
    }

    pub fn prepared_work_error(&self) -> Option<&OperationWorkError> {
        self.prepared_work_error.as_ref()
    }

    pub fn base_typography(&self) -> Option<&FlowchartBaseTypographyPlan> {
        self.base_typography.as_ref()
    }

    pub fn prepared_math_evidence(&self) -> PreparedMathEvidenceLease {
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

    pub fn node_owner(&self, node_id: &str, swimlane: bool) -> Option<FlowchartSvgLabelOwner> {
        if swimlane {
            self.swimlane_node_owner_by_id.get(node_id).copied()
        } else {
            self.empty_subgraph_owner_by_id
                .get(node_id)
                .or_else(|| self.node_owner_by_id.get(node_id))
                // Collapsed groups emit a node while retaining their prepared title owner.
                .or_else(|| self.subgraph_title_owner_by_id.get(node_id))
                .copied()
        }
    }

    pub fn node_source_style(
        &self,
        owner: FlowchartSvgLabelOwner,
    ) -> Option<&Arc<FlowchartNodeSourceTypography>> {
        self.node_source_typography.get(owner)
    }

    pub fn node_geometry(
        &self,
        node_id: &str,
        swimlane: bool,
    ) -> Option<PreparedFlowchartNodeGeometry> {
        self.node_geometry
            .get(self.node_owner(node_id, swimlane)?)
            .copied()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn edge_owner(&self, edge_id: &str, swimlane: bool) -> Option<FlowchartSvgLabelOwner> {
        if swimlane {
            self.swimlane_edge_owner_by_id.get(edge_id).copied()
        } else {
            self.edge_owner_by_id.get(edge_id).copied()
        }
    }

    pub fn subgraph_title_owner(&self, subgraph_id: &str) -> Option<FlowchartSvgLabelOwner> {
        self.subgraph_title_owner_by_id.get(subgraph_id).copied()
    }

    pub fn swimlane_group_title_owner(&self, subgraph_id: &str) -> Option<FlowchartSvgLabelOwner> {
        self.swimlane_group_title_owner_by_id
            .get(subgraph_id)
            .copied()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn prepared_math(
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

    pub fn prepared_label_for_terminal(
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
            return FlowchartPreparedLabelResolution::PreparedHtml(prepared);
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

    #[cfg(any(test, feature = "test-support"))]
    pub fn prepared_math_arc(
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
        let prepared = source.map(FlowchartSvgLabelSourceEntry::source).zip(
            self.prepared
                .get(owner)
                .filter(|prepared| prepared.matches_wrapping(binding)),
        );
        #[cfg(any(test, feature = "test-support"))]
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
            .map(FlowchartSvgLabelSourceEntry::source)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn prepared_hit_count(&self, owner: FlowchartSvgLabelOwner) -> usize {
        self.prepared_hits_by_owner
            .lock()
            .expect("prepared-hit test observer lock")
            .get(&owner)
            .copied()
            .unwrap_or_default()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_source_plan_counts(
        mut self,
        counts: FxHashMap<FlowchartSvgLabelOwner, usize>,
    ) -> Self {
        self.source_plans_by_owner = Mutex::new(counts);
        self
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn row_counts(&self) -> (usize, usize) {
        (self.sources.entry_count(), self.prepared.entry_count())
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn source_plan_count(&self, owner: FlowchartSvgLabelOwner) -> usize {
        self.source_plans_by_owner
            .lock()
            .expect("source-plan test observer lock")
            .get(&owner)
            .copied()
            .unwrap_or_default()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepared_metrics(
        &self,
        owner: FlowchartSvgLabelOwner,
        raw_source: &str,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        max_width_px: Option<f64>,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> Option<TextMetrics> {
        let svg_measurement = FlowchartSvgLabelMeasurementRequest::new(
            measurer,
            style,
            style,
            max_width_px,
            break_long_words,
            width_mode,
        );
        if !svg_measurement.has_reusable_binding() {
            return None;
        }
        self.source(owner, raw_source)?;
        let metrics = svg_measurement.matching_metrics(self.prepared.get(owner)?)?;
        #[cfg(any(test, feature = "test-support"))]
        {
            let mut hits = self
                .prepared_hits_by_owner
                .lock()
                .expect("prepared-hit test observer lock");
            let owner_hits = hits.entry(owner).or_default();
            *owner_hits = owner_hits.saturating_add(1);
        }
        Some(metrics)
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

pub enum FlowchartSvgLabelRenderPlan<'a> {
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
    pub fn new(
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
    pub fn new_with_metrics_style(
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

    pub fn plain_text(&self) -> &str {
        match self {
            Self::Prepared { source, .. } => source.plain_text(),
            Self::Source { source, .. } => source.plain_text(),
        }
    }

    pub const fn is_prepared(&self) -> bool {
        matches!(self, Self::Prepared { .. })
    }

    pub fn wrapped_lines(&self) -> Cow<'_, [Vec<String>]> {
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
