//! Operation-local prepared text retained by one State family artifact.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::diagram_theme::LineHeight;

use crate::entities::decode_mermaid_entities_for_render_text;
use crate::resources::{
    OperationWorkMeter, PreparedTextRetainedReservation, ResourceLimitExceeded,
};
use crate::text::{
    CatalogAdmittedTextStyle, PendingPreparedTextLabelLedgerEntry, PrepareTextRequest,
    PreparedText, PreparedTextLabelFamily, PreparedTextLabelId, PreparedTextLabelLedgerEntry,
    PreparedTextLayout, PreparedTextWrap, TextLayoutError, TextMeasurer, TextMetrics, WrapMode,
};

use super::{
    ResolvedLabelTypography, StateLabelMeasurement, StateNativeLabelGeometry,
    measure_state_markdown_label, state_markdown_label_plain_text,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StateLabelSourceKind {
    Markdown,
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StateLabelOwner<'a> {
    Node(&'a str),
    NodeTitle(&'a str),
    NodeDescription(&'a str),
    ClusterTitle(&'a str),
    Edge(&'a str),
}

#[derive(Clone, Copy)]
pub(crate) struct StateLabelMetricsRequest<'a> {
    pub(crate) owner: StateLabelOwner<'a>,
    pub(crate) text: &'a str,
    pub(crate) source_kind: StateLabelSourceKind,
    pub(crate) measurer: &'a dyn TextMeasurer,
    pub(crate) typography: &'a ResolvedLabelTypography,
    pub(crate) max_width_px: Option<f64>,
    pub(crate) wrap_mode: WrapMode,
    pub(crate) break_long_words: bool,
}

#[derive(Debug)]
pub(crate) struct PreparedStateLabel {
    semantic_source: Arc<str>,
    prepared: PreparedText,
    admitted_typography: CatalogAdmittedTextStyle,
    uses_html_wrapping_table: bool,
    native_geometry: Option<StateNativeLabelGeometry>,
    pending_label_entry: Option<PendingPreparedTextLabelLedgerEntry>,
    label_entry: Option<PreparedTextLabelLedgerEntry>,
    label_consumed: Cell<bool>,
    retained_reservation: RefCell<Option<PreparedTextRetainedReservation>>,
}

impl PreparedStateLabel {
    pub(crate) fn wrapped_lines(&self) -> impl ExactSizeIterator<Item = &str> + '_ {
        self.prepared.wrapped_lines()
    }

    pub(crate) fn layout_metrics(&self) -> TextMetrics {
        self.native_geometry.as_ref().map_or_else(
            || self.prepared.metrics(),
            StateNativeLabelGeometry::layout_metrics,
        )
    }

    pub(crate) const fn uses_html_wrapping_table(&self) -> bool {
        self.uses_html_wrapping_table
    }

    pub(crate) const fn native_geometry(&self) -> Option<&StateNativeLabelGeometry> {
        self.native_geometry.as_ref()
    }

    pub(crate) fn matches_semantic_source(&self, source: &str) -> bool {
        self.semantic_source.as_ref() == source
    }

    pub(crate) fn merge_emission_font_style(&self, existing: Option<&str>) -> String {
        self.admitted_typography
            .merge_materialized_emission_font_style(existing)
    }

    fn bind_label_id(&mut self, key: u32) -> bool {
        let Some(pending) = self.pending_label_entry.take() else {
            return false;
        };
        self.label_entry = Some(pending.bind(PreparedTextLabelId::new(
            PreparedTextLabelFamily::State,
            key,
        )));
        true
    }

    pub(crate) fn label_id_for_emission(&self) -> Option<PreparedTextLabelId> {
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
        self.prepared
            .retained_bytes()
            .saturating_add(self.semantic_source.len())
            .saturating_add(self.admitted_typography.retained_bytes())
    }

    fn reserve_retained_bytes(
        &self,
        work_meter: &Arc<OperationWorkMeter>,
    ) -> Result<(), ResourceLimitExceeded> {
        let reservation = work_meter.reserve_prepared_text_retained_bytes(self.retained_bytes())?;
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

#[derive(Debug)]
struct StateLabelSlots<T> {
    nodes: BTreeMap<Box<str>, T>,
    node_titles: BTreeMap<Box<str>, T>,
    node_descriptions: BTreeMap<Box<str>, T>,
    cluster_titles: BTreeMap<Box<str>, T>,
    edges: BTreeMap<Box<str>, T>,
}

impl<T> Default for StateLabelSlots<T> {
    fn default() -> Self {
        Self {
            nodes: BTreeMap::new(),
            node_titles: BTreeMap::new(),
            node_descriptions: BTreeMap::new(),
            cluster_titles: BTreeMap::new(),
            edges: BTreeMap::new(),
        }
    }
}

impl<T> StateLabelSlots<T> {
    fn insert(&mut self, owner: StateLabelOwner<'_>, value: T) {
        match owner {
            StateLabelOwner::Node(id) => {
                self.nodes.insert(id.into(), value);
            }
            StateLabelOwner::NodeTitle(id) => {
                self.node_titles.insert(id.into(), value);
            }
            StateLabelOwner::NodeDescription(id) => {
                self.node_descriptions.insert(id.into(), value);
            }
            StateLabelOwner::ClusterTitle(id) => {
                self.cluster_titles.insert(id.into(), value);
            }
            StateLabelOwner::Edge(id) => {
                self.edges.insert(id.into(), value);
            }
        }
    }

    fn for_each_mut(&mut self, mut visit: impl FnMut(&mut T)) {
        for value in self.nodes.values_mut() {
            visit(value);
        }
        for value in self.node_titles.values_mut() {
            visit(value);
        }
        for value in self.node_descriptions.values_mut() {
            visit(value);
        }
        for value in self.cluster_titles.values_mut() {
            visit(value);
        }
        for value in self.edges.values_mut() {
            visit(value);
        }
    }

    fn iter(&self) -> impl Iterator<Item = &T> {
        self.nodes
            .values()
            .chain(self.node_titles.values())
            .chain(self.node_descriptions.values())
            .chain(self.cluster_titles.values())
            .chain(self.edges.values())
    }
}

fn label_slot<'a, T>(slots: &'a StateLabelSlots<T>, owner: StateLabelOwner<'_>) -> Option<&'a T> {
    match owner {
        StateLabelOwner::Node(id) => slots.nodes.get(id),
        StateLabelOwner::NodeTitle(id) => slots.node_titles.get(id),
        StateLabelOwner::NodeDescription(id) => slots.node_descriptions.get(id),
        StateLabelOwner::ClusterTitle(id) => slots.cluster_titles.get(id),
        StateLabelOwner::Edge(id) => slots.edges.get(id),
    }
}

#[derive(Debug, Default)]
pub(crate) struct StateLabelSidecarBuilder {
    prepared_active: bool,
    prepared_text_layout: Option<PreparedTextLayout>,
    work_meter: Option<Arc<OperationWorkMeter>>,
    labels: RefCell<StateLabelSlots<PreparedStateLabel>>,
    measured_native_geometries: RefCell<StateLabelSlots<StateNativeLabelGeometry>>,
    prepared_error: RefCell<Option<TextLayoutError>>,
    prepared_resource_error: RefCell<Option<ResourceLimitExceeded>>,
}

impl StateLabelSidecarBuilder {
    #[cfg(test)]
    pub(crate) fn new(prepared_text_layout: Option<&PreparedTextLayout>) -> Self {
        Self {
            prepared_active: prepared_text_layout.is_some(),
            prepared_text_layout: prepared_text_layout.cloned(),
            ..Self::default()
        }
    }

    pub(crate) fn new_with_work_meter(
        prepared_text_layout: Option<&PreparedTextLayout>,
        work_meter: Arc<OperationWorkMeter>,
    ) -> Self {
        Self {
            prepared_active: prepared_text_layout.is_some(),
            prepared_text_layout: prepared_text_layout.cloned(),
            work_meter: Some(work_meter),
            ..Self::default()
        }
    }

    pub(crate) fn prepared_native_geometry(
        &self,
        owner: StateLabelOwner<'_>,
    ) -> Option<StateNativeLabelGeometry> {
        let labels = self.labels.borrow();
        label_slot(&labels, owner)
            .and_then(PreparedStateLabel::native_geometry)
            .cloned()
    }

    pub(crate) fn measured_native_geometry(
        &self,
        owner: StateLabelOwner<'_>,
    ) -> Option<StateNativeLabelGeometry> {
        let geometries = self.measured_native_geometries.borrow();
        label_slot(&geometries, owner).cloned()
    }

    pub(crate) fn measure_for_layout(
        &self,
        request: StateLabelMetricsRequest<'_>,
    ) -> StateLabelMeasurement {
        let Some(layout) = self.prepared_text_layout.as_ref() else {
            let mut measurement = measure_without_prepared_text(request);
            if let Some(geometry) =
                StateNativeLabelGeometry::from_measurement(request, &measurement)
            {
                measurement.metrics = geometry.layout_metrics();
                self.measured_native_geometries
                    .borrow_mut()
                    .insert(request.owner, geometry);
            }
            return measurement;
        };
        if self.prepared_error.borrow().is_some() || self.prepared_resource_error.borrow().is_some()
        {
            return failed_prepared_measurement();
        }

        match self.prepare_label(layout, request) {
            Ok(label) => {
                if let Some(work_meter) = self.work_meter.as_ref()
                    && let Err(error) = label.reserve_retained_bytes(work_meter)
                {
                    self.record_resource_error(error);
                    return failed_prepared_measurement();
                }
                let measurement = StateLabelMeasurement {
                    metrics: label.layout_metrics(),
                    uses_html_wrapping_table: label.uses_html_wrapping_table(),
                };
                self.labels.borrow_mut().insert(request.owner, label);
                measurement
            }
            Err(error) => {
                self.record_error(error);
                failed_prepared_measurement()
            }
        }
    }

    fn prepare_label(
        &self,
        layout: &PreparedTextLayout,
        request: StateLabelMetricsRequest<'_>,
    ) -> Result<PreparedStateLabel, TextLayoutError> {
        let prepared_source = match request.source_kind {
            StateLabelSourceKind::Markdown => state_markdown_label_plain_text(request.text).ok_or(
                TextLayoutError::UnsupportedPreparedTextPath("state_markdown_label"),
            )?,
            StateLabelSourceKind::Plain => decode_mermaid_entities_for_render_text(request.text),
        };
        let typography = request
            .typography
            .prepared_typography()
            .ok_or(TextLayoutError::UnsupportedPreparedTextPath(
                "structured_typography",
            ))?
            .clone()
            .with_line_height(match request.wrap_mode {
                WrapMode::HtmlLike => LineHeight::Multiplier(1.5),
                WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => LineHeight::Multiplier(1.1),
            })
            .map_err(|_| TextLayoutError::InvalidRequest("line_height"))?;
        let admitted_typography = layout.admit_typography_with_css_font_stack(
            &typography,
            request.typography.source_font_stack(),
        )?;
        debug_assert_eq!(
            admitted_typography.catalog_fingerprint(),
            layout.catalog_fingerprint(),
            "prepared State typography must remain bound to the active catalog"
        );
        let wrap = match request.wrap_mode {
            WrapMode::SvgLike => PreparedTextWrap::SvgLike {
                max_width_px: normalized_width(request.max_width_px),
                break_long_words: request.break_long_words,
            },
            WrapMode::SvgLikeSingleRun => PreparedTextWrap::SingleRun,
            WrapMode::HtmlLike => PreparedTextWrap::HtmlLike {
                max_width_px: normalized_width(request.max_width_px),
            },
        };
        let prepared_request = PrepareTextRequest::new(
            prepared_source.as_ref(),
            admitted_typography.typography().clone(),
        )
        .with_family_normalized_projection()
        .with_wrap(wrap);
        let prepared = match self.work_meter.as_deref() {
            Some(work_meter) => {
                layout.prepare_text_with_work_meter(&prepared_request, work_meter)?
            }
            None => layout.prepare_text(&prepared_request)?,
        };
        let pending_label_entry = prepared
            .label_ledger_entry()
            .ok_or(TextLayoutError::InvalidPreparedText)?;
        let metrics = prepared.metrics();
        let max_width = normalized_width(request.max_width_px);
        let uses_html_wrapping_table = request.wrap_mode == WrapMode::HtmlLike
            && max_width.is_some_and(|width| {
                prepared.raw_width_px().unwrap_or(metrics.width) >= width - 1e-9
            });
        let native_geometry = matches!(
            request.wrap_mode,
            WrapMode::SvgLike | WrapMode::SvgLikeSingleRun
        )
        .then(|| StateNativeLabelGeometry::from_prepared(request.owner, &prepared))
        .transpose()?;
        Ok(PreparedStateLabel {
            semantic_source: Arc::from(request.text),
            prepared,
            admitted_typography,
            uses_html_wrapping_table,
            native_geometry,
            pending_label_entry: Some(pending_label_entry),
            label_entry: None,
            label_consumed: Cell::new(false),
            retained_reservation: RefCell::new(None),
        })
    }

    fn record_error(&self, error: TextLayoutError) {
        if self.prepared_resource_error.borrow().is_some() {
            return;
        }
        let mut prepared_error = self.prepared_error.borrow_mut();
        if prepared_error.is_none() {
            *prepared_error = Some(error);
        }
    }

    fn record_resource_error(&self, error: ResourceLimitExceeded) {
        let mut prepared_resource_error = self.prepared_resource_error.borrow_mut();
        if prepared_resource_error.is_none() {
            *prepared_resource_error = Some(error);
            self.prepared_error.borrow_mut().take();
        }
    }

    pub(crate) fn reject_unsupported(&self, path: &'static str) {
        if self.prepared_text_layout.is_some() {
            self.record_error(TextLayoutError::UnsupportedPreparedTextPath(path));
        }
    }

    pub(crate) fn finish(self) -> StateLabelSidecar {
        let mut labels = self.labels.into_inner();
        let mut next_key = 0usize;
        let mut key_overflow = false;
        labels.for_each_mut(|label| {
            let Ok(key) = u32::try_from(next_key) else {
                if label.pending_label_entry.is_some() {
                    key_overflow = true;
                }
                return;
            };
            if label.bind_label_id(key) {
                next_key = next_key.saturating_add(1);
            }
        });
        let mut prepared_error = self.prepared_error.into_inner();
        if key_overflow && prepared_error.is_none() {
            prepared_error = Some(TextLayoutError::LimitExceeded("prepared_label_ledger"));
        }
        StateLabelSidecar {
            prepared_active: self.prepared_active,
            labels,
            measured_native_geometries: self.measured_native_geometries.into_inner(),
            prepared_error,
            prepared_resource_error: self.prepared_resource_error.into_inner(),
        }
    }
}

fn failed_prepared_measurement() -> StateLabelMeasurement {
    StateLabelMeasurement {
        metrics: TextMetrics {
            width: 0.0,
            height: 0.0,
            line_count: 0,
        },
        uses_html_wrapping_table: false,
    }
}

fn normalized_width(width: Option<f64>) -> Option<f64> {
    width.filter(|width| width.is_finite() && *width > 0.0)
}

fn measure_without_prepared_text(request: StateLabelMetricsRequest<'_>) -> StateLabelMeasurement {
    match request.source_kind {
        StateLabelSourceKind::Markdown => measure_state_markdown_label(
            request.text,
            request.measurer,
            request.typography.text_style(),
            request.max_width_px,
            request.wrap_mode,
        ),
        StateLabelSourceKind::Plain => {
            let text = decode_mermaid_entities_for_render_text(request.text);
            StateLabelMeasurement {
                metrics: request.measurer.measure_wrapped(
                    text.as_ref(),
                    request.typography.text_style(),
                    request.max_width_px,
                    request.wrap_mode,
                ),
                uses_html_wrapping_table: false,
            }
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct StateLabelSidecar {
    prepared_active: bool,
    labels: StateLabelSlots<PreparedStateLabel>,
    measured_native_geometries: StateLabelSlots<StateNativeLabelGeometry>,
    prepared_error: Option<TextLayoutError>,
    prepared_resource_error: Option<ResourceLimitExceeded>,
}

impl StateLabelSidecar {
    pub(crate) fn prepared_error(&self) -> Option<&TextLayoutError> {
        self.prepared_error.as_ref()
    }

    pub(crate) fn prepared_resource_error(&self) -> Option<&ResourceLimitExceeded> {
        self.prepared_resource_error.as_ref()
    }

    pub(crate) fn prepared_text_label_ledger(
        &self,
    ) -> impl Iterator<Item = &PreparedTextLabelLedgerEntry> {
        self.labels
            .iter()
            .filter_map(PreparedStateLabel::consumed_label_entry)
    }

    pub(crate) fn take_prepared_text_retained_reservations(
        &self,
    ) -> Vec<PreparedTextRetainedReservation> {
        self.labels
            .iter()
            .filter_map(PreparedStateLabel::take_consumed_retained_reservation)
            .collect()
    }

    pub(crate) fn node(&self, id: &str) -> Option<&PreparedStateLabel> {
        self.labels.nodes.get(id)
    }

    pub(crate) fn node_title(&self, id: &str) -> Option<&PreparedStateLabel> {
        self.labels.node_titles.get(id)
    }

    pub(crate) fn node_description(&self, id: &str) -> Option<&PreparedStateLabel> {
        self.labels.node_descriptions.get(id)
    }

    pub(crate) fn cluster_title(&self, id: &str) -> Option<&PreparedStateLabel> {
        self.labels.cluster_titles.get(id)
    }

    pub(crate) fn edge(&self, id: &str) -> Option<&PreparedStateLabel> {
        self.labels.edges.get(id)
    }

    pub(crate) fn measured_native_geometry(
        &self,
        owner: StateLabelOwner<'_>,
    ) -> Option<&StateNativeLabelGeometry> {
        label_slot(&self.measured_native_geometries, owner)
    }

    pub(crate) fn validate_for_render(
        &self,
        model: &merman_core::diagrams::state::StateDiagramRenderModel,
        layout: &crate::model::StateDiagramLayout,
    ) -> Result<(), TextLayoutError> {
        if !self.prepared_active {
            return Ok(());
        }

        for layout_node in &layout.nodes {
            if layout_node.is_cluster {
                continue;
            }
            let node = model
                .nodes
                .iter()
                .find(|node| node.id == layout_node.id)
                .ok_or(TextLayoutError::InvalidPreparedText)?;
            let source = node
                .label
                .as_ref()
                .map(super::state_value_to_label_text)
                .unwrap_or_else(|| node.id.clone());
            match node.shape.as_str() {
                "stateStart" | "stateEnd" | "choice" | "fork" | "join" => {}
                "rectWithTitle" => {
                    validate_source(self.node_title(&node.id), &source)?;
                    let description = node
                        .description
                        .as_ref()
                        .map(|parts| parts.join("\n"))
                        .unwrap_or_default();
                    validate_source(self.node_description(&node.id), &description)?;
                }
                _ => validate_source(self.node(&node.id), &source)?,
            }
        }

        for cluster in &layout.clusters {
            if cluster.title.trim().is_empty() {
                continue;
            }
            validate_source(self.cluster_title(&cluster.id), &cluster.title)?;
        }

        for layout_edge in &layout.edges {
            if layout_edge.label.is_none() {
                continue;
            }
            let edge = model
                .edges
                .iter()
                .find(|edge| edge.id == layout_edge.id)
                .ok_or(TextLayoutError::InvalidPreparedText)?;
            let source = edge.label.trim();
            if !source.is_empty() {
                validate_source(self.edge(&edge.id), source)?;
            }
        }
        Ok(())
    }
}

fn validate_source(
    prepared: Option<&PreparedStateLabel>,
    semantic_source: &str,
) -> Result<(), TextLayoutError> {
    let prepared = prepared.ok_or(TextLayoutError::InvalidPreparedText)?;
    if prepared.matches_semantic_source(semantic_source) {
        Ok(())
    } else {
        Err(TextLayoutError::RequestDigestMismatch)
    }
}
