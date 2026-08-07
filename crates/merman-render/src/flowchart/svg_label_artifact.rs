//! Operation-local preparation for non-Markdown Flowchart SVG labels.

use std::borrow::Cow;
use std::cell::RefCell;

use rustc_hash::FxHashMap;

use crate::environment::{BuiltinTextMeasurementOperationCarrier, TextMeasurementOperation};
use crate::text::{TextMeasurer, TextMetrics, TextStyle, WrapMode};

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
    computed_length_carrier: BuiltinTextMeasurementOperationCarrier,
    wrapped_carrier: BuiltinTextMeasurementOperationCarrier,
}

#[derive(Debug, Clone, Copy)]
struct FlowchartSvgLabelWrapBindingRequest<'a> {
    wrap_style: &'a TextStyle,
    max_width_bits: Option<u64>,
    break_long_words: bool,
    computed_length_carrier: BuiltinTextMeasurementOperationCarrier,
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
            computed_length_carrier: measurer
                .builtin_operation_carrier(TextMeasurementOperation::ComputedLength)?,
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
    computed_length_carrier: BuiltinTextMeasurementOperationCarrier,
    wrapped_carrier: BuiltinTextMeasurementOperationCarrier,
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
            computed_length_carrier: measurer
                .builtin_operation_carrier(TextMeasurementOperation::ComputedLength)?,
            wrapped_carrier: measurer
                .builtin_operation_carrier(TextMeasurementOperation::Wrapped)?,
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

impl PreparedFlowchartSvgLabel {
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

#[derive(Debug, Default)]
struct PendingFlowchartSvgLabels {
    sources: FxHashMap<FlowchartSvgLabelOwner, FlowchartSvgLabelSourceEntry>,
    prepared: FxHashMap<FlowchartSvgLabelOwner, PreparedFlowchartSvgLabel>,
    render_ids: FxHashMap<FlowchartSvgLabelOwner, Box<str>>,
}

/// Mutable preparation state used only while one Flowchart family artifact is laid out.
///
/// Pure source projections are retained for every eligible label. Measured results are retained
/// only when the routed operations prove an exact built-in backend. Opaque host and fallback
/// paths therefore keep their callback sequence while still avoiding repeated tokenization.
#[derive(Debug, Default)]
pub(crate) struct FlowchartSvgLabelSidecarBuilder {
    pending: RefCell<PendingFlowchartSvgLabels>,
}

impl FlowchartSvgLabelSidecarBuilder {
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

        let measured_from_existing_source = {
            let pending = self.pending.borrow();
            let source = pending
                .render_ids
                .get(&owner)
                .filter(|pending_render_id| pending_render_id.as_ref() == render_id)
                .and_then(|_| pending.sources.get(&owner))
                .filter(|source| source.matches(request.raw_label));

            if let (Some(_), Some(binding)) = (source, binding.as_ref())
                && let Some(metrics) = pending
                    .prepared
                    .get(&owner)
                    .filter(|prepared| prepared.matches(binding))
                    .map(PreparedFlowchartSvgLabel::metrics)
            {
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

        let mut pending = self.pending.borrow_mut();
        pending.render_ids.insert(owner, render_id.into());
        if let Some(source) = new_source {
            pending.sources.insert(owner, source);
            pending.prepared.remove(&owner);
        }
        if let Some(binding) = binding {
            pending.prepared.insert(
                owner,
                PreparedFlowchartSvgLabel {
                    binding: binding.into_owned(),
                    wrapped_lines,
                    metrics,
                },
            );
        } else {
            pending.prepared.remove(&owner);
        }
        metrics
    }

    pub(crate) fn finish(self) -> FlowchartSvgLabelSidecar {
        let pending = self.pending.into_inner();
        FlowchartSvgLabelSidecar::new(pending.sources, pending.prepared, pending.render_ids)
    }

    #[cfg(test)]
    pub(crate) fn prepared_count(&self) -> usize {
        self.pending.borrow().prepared.len()
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

/// Immutable render-side index private to one prepared Flowchart family artifact.
#[derive(Debug, Default)]
pub(crate) struct FlowchartSvgLabelSidecar {
    sources: FxHashMap<FlowchartSvgLabelOwner, FlowchartSvgLabelSourceEntry>,
    prepared: FxHashMap<FlowchartSvgLabelOwner, PreparedFlowchartSvgLabel>,
    node_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    empty_subgraph_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    edge_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    subgraph_title_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    swimlane_node_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    swimlane_edge_owner_by_id: FxHashMap<String, FlowchartSvgLabelOwner>,
    #[cfg(test)]
    prepared_hits_by_owner: std::sync::Mutex<FxHashMap<FlowchartSvgLabelOwner, usize>>,
    #[cfg(test)]
    source_plans_by_owner: std::sync::Mutex<FxHashMap<FlowchartSvgLabelOwner, usize>>,
}

impl FlowchartSvgLabelSidecar {
    fn new(
        sources: FxHashMap<FlowchartSvgLabelOwner, FlowchartSvgLabelSourceEntry>,
        prepared: FxHashMap<FlowchartSvgLabelOwner, PreparedFlowchartSvgLabel>,
        render_ids: FxHashMap<FlowchartSvgLabelOwner, Box<str>>,
    ) -> Self {
        let mut sidecar = Self {
            sources,
            prepared,
            ..Self::default()
        };
        for (owner, render_id) in render_ids {
            let render_id = render_id.into_string();
            match owner {
                FlowchartSvgLabelOwner::Node(_) => {
                    insert_last_owner(&mut sidecar.node_owner_by_id, render_id, owner);
                }
                FlowchartSvgLabelOwner::EmptySubgraphNode(_) => {
                    insert_first_owner(&mut sidecar.empty_subgraph_owner_by_id, render_id, owner);
                }
                FlowchartSvgLabelOwner::Edge(_) => {
                    insert_last_owner(&mut sidecar.edge_owner_by_id, render_id, owner);
                }
                FlowchartSvgLabelOwner::SubgraphTitle(_) => {
                    insert_first_owner(&mut sidecar.subgraph_title_owner_by_id, render_id, owner);
                }
                FlowchartSvgLabelOwner::SwimlaneNode(_) => {
                    insert_last_owner(&mut sidecar.swimlane_node_owner_by_id, render_id, owner);
                }
                FlowchartSvgLabelOwner::SwimlaneEdgeLabel(_) => {
                    insert_last_owner(&mut sidecar.swimlane_edge_owner_by_id, render_id, owner);
                }
            }
        }
        sidecar
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

    fn prepared(
        &self,
        owner: FlowchartSvgLabelOwner,
        raw_source: &str,
        binding: &FlowchartSvgLabelBindingRequest<'_>,
    ) -> Option<(&FlowchartSvgLabelSource, &PreparedFlowchartSvgLabel)> {
        let source = self
            .sources
            .get(&owner)
            .filter(|source| source.matches(raw_source));
        let prepared = source.map(|source| &source.source).zip(
            self.prepared
                .get(&owner)
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
            .get(&owner)
            .filter(|source| source.matches(raw_source));
        let prepared = source.map(|source| &source.source).zip(
            self.prepared
                .get(&owner)
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
            .get(&owner)
            .filter(|source| source.matches(raw_source))
            .map(|source| &source.source)
    }

    #[cfg(test)]
    fn record_source_plan(&self, owner: FlowchartSvgLabelOwner) {
        let mut plans = self
            .source_plans_by_owner
            .lock()
            .expect("source-plan test observer lock");
        let owner_plans = plans.entry(owner).or_default();
        *owner_plans = owner_plans.saturating_add(1);
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
    match index.get(id.as_str()).copied() {
        Some(current) if current.semantic_index() < owner.semantic_index() => {}
        _ => {
            index.insert(id, owner);
        }
    }
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
    pub(crate) fn new(
        sidecar: Option<&'a FlowchartSvgLabelSidecar>,
        owner: Option<FlowchartSvgLabelOwner>,
        raw_source: &str,
        measurer: &'a dyn TextMeasurer,
        style: &'a TextStyle,
        max_width_px: Option<f64>,
        break_long_words: bool,
    ) -> Self {
        let binding = FlowchartSvgLabelWrapBindingRequest::for_measurer(
            measurer,
            style,
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
            #[cfg(test)]
            if let Some((sidecar, owner)) = sidecar.zip(owner) {
                sidecar.record_source_plan(owner);
            }
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
                style,
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
    use std::cell::RefCell;

    use crate::environment::{RenderEnvironment, TextMeasurementPhase};
    use crate::text::{TextMeasurer, TextMetrics, TextStyle, WrapMode};
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
        );
        assert!(matches!(plan, FlowchartSvgLabelRenderPlan::Prepared { .. }));
        assert!(!plan.wrapped_lines().is_empty());
        assert_eq!(report_call_count(&session), calls_before_render);
        assert_eq!(sidecar.prepared_hit_count(owner), 1);
        assert_eq!(sidecar.source_plan_count(owner), 0);

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
    fn prepared_measurement_requires_an_exact_parameter_binding() {
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
        );
        assert!(matches!(
            style_mismatch,
            FlowchartSvgLabelRenderPlan::Source { .. }
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

        assert_eq!(
            builder.finish().subgraph_title_owner("dup"),
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
        let plan = FlowchartSvgLabelRenderPlan::new(
            Some(&sidecar),
            Some(owner),
            raw_label,
            &measurer,
            &wrap_style,
            max_width,
            true,
        );
        assert!(matches!(plan, FlowchartSvgLabelRenderPlan::Prepared { .. }));
        assert_eq!(plan.wrapped_lines().as_ref(), expected_lines.as_slice());
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
            assert_eq!(pending.sources.len(), 1);
            assert_eq!(pending.prepared.len(), 1);
            assert_eq!(pending.render_ids.len(), 1);
        }

        let sidecar = builder.finish();
        assert_eq!(sidecar.sources.len(), 1);
        assert_eq!(sidecar.prepared.len(), 1);
        assert_eq!(sidecar.edge_owner("sparse-edge", false), Some(owner));
    }
}
