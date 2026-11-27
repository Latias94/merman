//! Operation-local preparation for Flow SVG rows and sanitized HTML labels.

#[cfg(any(test, feature = "test-support"))]
use std::cell::Cell;
use std::{borrow::Cow, cell::RefCell, sync::Arc};

#[cfg(any(test, feature = "test-support"))]
use merman_model::presentation::PresentationConfig;
use merman_render_core::{
    Result,
    config::ConfigLookup,
    math::{ConfiguredMathBackend, PreparedMathLabel},
    mermaid_style::HtmlTextGeometryDeclarations,
    resources::{
        OperationWorkError, OperationWorkMeter, RenderResourcePolicy, ResourceLimitExceeded,
    },
    text::{
        TextMetrics, TextStyle, WrapMode, analyze_mermaid_markdown,
        create_text_html::FlowchartLabelMetricsRequest,
        html_label_layout::measure_flowchart_label_for_layout,
        literal_label::{
            FlowchartSvgLabelSource, FlowchartSvgWidthMode,
            flowchart_non_markdown_svg_source_word_lines,
        },
        mermaid_markdown_is_plain_text,
    },
};
#[cfg(any(test, feature = "test-support"))]
use rustc_hash::FxHashMap;

use super::{
    FLOWCHART_PREPARED_OWNER_SLOT_BYTES, FLOWCHART_RENDER_ID_RECORD_BYTES,
    FlowchartNodeSourceTypography, FlowchartSvgLabelMeasurementRequest, FlowchartSvgLabelOwner,
    FlowchartSvgLabelSidecar, FlowchartSvgLabelSourceEntry, PendingFlowchartSvgLabels,
    PreparedFlowchartHtmlLabel, PreparedFlowchartMathLabel,
};
use crate::{
    layout_config::{FlowchartBaseTypographyPlan, FlowchartLayoutSettings},
    node_geometry::PreparedFlowchartNodeGeometry,
    style::{
        FlowchartTerminalForeground, FlowchartTerminalForegroundProvenance,
        FlowchartTextStyleResolution,
    },
    theme::FlowchartCompatibilityBinding,
};

/// Mutable preparation state used only while one Flowchart family artifact is laid out.
///
/// Pure source projections are retained for every eligible label. Browser math is retained only
/// after its operation-scoped terminal style and backend outcome are bound to the owner; native
/// text preparation remains independently governed by its existing catalog ledger.
#[derive(Debug, Default)]
pub struct FlowchartSvgLabelSidecarBuilder {
    pending: RefCell<PendingFlowchartSvgLabels>,
    base_typography: Option<FlowchartBaseTypographyPlan>,
    native_density: Option<merman_theme_contract::v2::Density>,
    math_backend: Option<ConfiguredMathBackend>,
    work_meter: Option<Arc<OperationWorkMeter>>,
    prepared_resource_error: RefCell<Option<ResourceLimitExceeded>>,
    prepared_work_error: RefCell<Option<OperationWorkError>>,
    math_terminal_style: FlowchartMathTerminalStylePlan,
    #[cfg(any(test, feature = "test-support"))]
    prepared_hits: Cell<usize>,
    #[cfg(any(test, feature = "test-support"))]
    source_plans_by_owner: RefCell<FxHashMap<FlowchartSvgLabelOwner, usize>>,
}

#[derive(Debug, Clone)]
struct FlowchartMathTerminalStylePlan {
    node: FlowchartTerminalForeground,
    title: FlowchartTerminalForeground,
}

impl Default for FlowchartMathTerminalStylePlan {
    fn default() -> Self {
        Self {
            node: FlowchartTerminalForeground::new(
                "#333",
                FlowchartTerminalForegroundProvenance::ThemeNode,
            ),
            title: FlowchartTerminalForeground::new(
                "#333",
                FlowchartTerminalForegroundProvenance::ThemeTitle,
            ),
        }
    }
}

impl FlowchartMathTerminalStylePlan {
    #[cfg(any(test, feature = "test-support"))]
    fn from_config(config: &PresentationConfig) -> Self {
        Self::from_binding(&FlowchartCompatibilityBinding::resolve(config))
    }

    fn from_binding(binding: &FlowchartCompatibilityBinding) -> Self {
        Self {
            node: FlowchartTerminalForeground::new(
                &binding.node_text_color,
                FlowchartTerminalForegroundProvenance::ThemeNode,
            ),
            title: FlowchartTerminalForeground::new(
                &binding.title_color,
                FlowchartTerminalForegroundProvenance::ThemeTitle,
            ),
        }
    }

    fn for_owner(&self, owner: FlowchartSvgLabelOwner) -> &FlowchartTerminalForeground {
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

enum PreparedMathMeasurement {
    NotApplicable,
    Prepared(TextMetrics),
    Unavailable,
}

impl FlowchartSvgLabelSidecarBuilder {
    #[cfg(any(feature = "layout-grid", feature = "layout-layered"))]
    pub(crate) fn check_prepared_label_errors(&self) -> Result<()> {
        if let Some(error) = self.prepared_work_error.borrow().as_ref() {
            return Err(error.clone().into());
        }
        if let Some(error) = self.prepared_resource_error.borrow().as_ref() {
            return Err(error.clone().into());
        }
        Ok(())
    }

    pub fn new_with_work_meter(work_meter: Arc<OperationWorkMeter>) -> Self {
        Self {
            work_meter: Some(work_meter),
            ..Self::default()
        }
    }

    pub fn with_base_typography(mut self, plan: FlowchartBaseTypographyPlan) -> Self {
        self.base_typography = Some(plan);
        self
    }

    pub fn with_native_density(
        mut self,
        density: Option<merman_theme_contract::v2::Density>,
    ) -> Self {
        self.native_density = density;
        self
    }

    pub const fn native_density(&self) -> Option<merman_theme_contract::v2::Density> {
        self.native_density
    }

    pub fn layout_settings(
        &self,
        effective_config: &(impl ConfigLookup + ?Sized),
    ) -> FlowchartLayoutSettings {
        FlowchartLayoutSettings::resolve(
            effective_config,
            self.base_typography.as_ref(),
            self.native_density,
        )
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_math_backend(
        mut self,
        math_backend: Option<&ConfiguredMathBackend>,
        config: &PresentationConfig,
    ) -> Self {
        self.math_backend = math_backend.cloned();
        self.math_terminal_style = FlowchartMathTerminalStylePlan::from_config(config);
        if self.work_meter.is_none() {
            self.work_meter = Some(Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input(),
            )));
        }
        self
    }

    pub fn with_prepared_math_backend(
        mut self,
        math_backend: Option<&ConfiguredMathBackend>,
        binding: &FlowchartCompatibilityBinding,
    ) -> Self {
        self.math_backend = math_backend.cloned();
        self.math_terminal_style = FlowchartMathTerminalStylePlan::from_binding(binding);
        if self.work_meter.is_none() {
            self.work_meter = Some(Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input(),
            )));
        }
        self
    }

    pub fn has_prepared_math(&self, owner: FlowchartSvgLabelOwner, raw_source: &str) -> bool {
        self.pending.borrow().math.get(owner).is_some_and(|entry| {
            entry.source() == raw_source && entry.outcome().prepared().is_some()
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
    fn measure_for_layout(
        &self,
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: FlowchartLabelMetricsRequest<'_>,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> TextMetrics {
        self.measure_for_layout_with_source_style(
            owner,
            render_id,
            request,
            None,
            break_long_words,
            width_mode,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn measure_for_layout_with_source_style(
        &self,
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: FlowchartLabelMetricsRequest<'_>,
        source_style: Option<&FlowchartTextStyleResolution<'_>>,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> TextMetrics {
        let metrics_style = request.style;
        self.measure_for_layout_with_metrics_and_source_style(
            owner,
            render_id,
            request,
            metrics_style,
            source_style,
            break_long_words,
            width_mode,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    fn measure_for_layout_with_metrics_style(
        &self,
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: FlowchartLabelMetricsRequest<'_>,
        metrics_style: &TextStyle,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> TextMetrics {
        self.measure_for_layout_with_metrics_and_source_style(
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
    fn measure_for_layout_with_metrics_and_source_style(
        &self,
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: FlowchartLabelMetricsRequest<'_>,
        metrics_style: &TextStyle,
        source_style: Option<&FlowchartTextStyleResolution<'_>>,
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
            let typography = match FlowchartNodeSourceTypography::prepare(
                Cow::Borrowed(request.style),
                self.work_meter.as_ref(),
            ) {
                Ok(typography) => typography,
                Err(error) => {
                    self.record_prepared_resource_error(error);
                    return failed_prepared_metrics();
                }
            };
            self.pending
                .borrow_mut()
                .node_source_typography
                .insert(owner, Arc::new(typography));
        }
        let html_geometry = source_style.map_or(Default::default(), |style| style.html_geometry);
        match self.measure_prepared_math(
            owner,
            render_id,
            &request,
            metrics_style,
            source_style.and_then(FlowchartTextStyleResolution::terminal_foreground),
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
                    html_geometry,
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
                html_geometry,
            );
        }
        if !supports_svg_source_preparation(owner, &request) {
            return measure_flowchart_label_for_layout(
                FlowchartLabelMetricsRequest {
                    style: metrics_style,
                    ..request
                },
                HtmlTextGeometryDeclarations::default(),
                true,
            );
        }

        let svg_measurement = FlowchartSvgLabelMeasurementRequest::new(
            request.measurer,
            request.style,
            metrics_style,
            request.max_width_px,
            break_long_words,
            width_mode,
        );

        let markdown = request.label_type == "markdown";

        let measured_from_existing_source = {
            let pending = self.pending.borrow();
            let source = pending
                .render_ids
                .get(owner)
                .filter(|pending_render_id| pending_render_id.as_ref() == render_id)
                .and_then(|_| pending.sources.get(owner))
                .filter(|source| source.matches(request.raw_label));

            if source.is_some()
                && svg_measurement.has_reusable_binding()
                && let Some(metrics) = pending
                    .prepared
                    .get(owner)
                    .and_then(|prepared| svg_measurement.matching_metrics(prepared))
            {
                #[cfg(any(test, feature = "test-support"))]
                self.prepared_hits
                    .set(self.prepared_hits.get().saturating_add(1));
                return metrics;
            }

            source.map(|source| svg_measurement.measure(source.source(), markdown))
        };

        let (new_source, measured) = measured_from_existing_source.map_or_else(
            || {
                let source = FlowchartSvgLabelSourceEntry::new(request.raw_label);
                let measured = svg_measurement.measure(source.source(), markdown);
                (Some(source), measured)
            },
            |measured| (None, measured),
        );

        #[cfg(any(test, feature = "test-support"))]
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
        let metrics = measured.metrics();
        if let Some(prepared) = measured.into_prepared() {
            pending.prepared.insert(owner, prepared);
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
        html_geometry: HtmlTextGeometryDeclarations,
    ) -> TextMetrics {
        {
            let pending = self.pending.borrow();
            if let Some(prepared) = pending
                .html
                .get(owner)
                .filter(|prepared| prepared.matches(request.raw_label, request.label_type))
            {
                return prepared.layout_metrics(request, html_geometry);
            }
        }
        if !self.record_render_id(owner, render_id) {
            return failed_prepared_metrics();
        }
        let prepared = match PreparedFlowchartHtmlLabel::prepare(&request, self.work_meter.as_ref())
        {
            Ok(prepared) => prepared,
            Err(error) => {
                self.record_prepared_work_error(error);
                return failed_prepared_metrics();
            }
        };
        let metrics = prepared.layout_metrics(request, html_geometry);
        self.pending.borrow_mut().html.insert(owner, prepared);
        metrics
    }

    fn measure_prepared_math(
        &self,
        owner: FlowchartSvgLabelOwner,
        render_id: &str,
        request: &FlowchartLabelMetricsRequest<'_>,
        metrics_style: &TextStyle,
        terminal_foreground: Option<&FlowchartTerminalForeground>,
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
                #[cfg(any(test, feature = "test-support"))]
                self.prepared_hits
                    .set(self.prepared_hits.get().saturating_add(1));
                return match prepared.outcome().prepared() {
                    Some(artifact) => PreparedMathMeasurement::Prepared(artifact.metrics()),
                    None => PreparedMathMeasurement::Unavailable,
                };
            }
        }

        let has_source_typography = self
            .pending
            .borrow()
            .node_source_typography
            .get(owner)
            .is_some();
        let prepared = match PreparedFlowchartMathLabel::prepare(
            owner,
            render_id,
            request,
            metrics_style,
            foreground,
            has_source_typography,
            self.math_backend.as_ref(),
            self.work_meter.as_ref(),
        ) {
            Ok(Some(prepared)) => prepared,
            Ok(None) => return PreparedMathMeasurement::Unavailable,
            Err(error) => {
                self.record_prepared_work_error(error);
                return PreparedMathMeasurement::Unavailable;
            }
        };
        let metrics = prepared
            .outcome()
            .prepared()
            .map(PreparedMathLabel::metrics);
        let mut pending = self.pending.borrow_mut();
        pending.render_ids.insert(owner, render_id.into());
        pending.math.insert(owner, prepared);
        match metrics {
            Some(metrics) => PreparedMathMeasurement::Prepared(metrics),
            None => PreparedMathMeasurement::Unavailable,
        }
    }

    pub fn record_node_geometry(
        &self,
        owner: FlowchartSvgLabelOwner,
        geometry: PreparedFlowchartNodeGeometry,
    ) -> Result<()> {
        if let Some(work) = &self.work_meter {
            work.charge(1)?;
        }
        self.pending
            .borrow_mut()
            .node_geometry
            .insert(owner, geometry);
        Ok(())
    }

    pub fn measured_node_geometry(
        &self,
        owner: FlowchartSvgLabelOwner,
    ) -> Option<PreparedFlowchartNodeGeometry> {
        self.pending.borrow().node_geometry.get(owner).copied()
    }

    pub fn finish(self) -> FlowchartSvgLabelSidecar {
        let sidecar = FlowchartSvgLabelSidecar::new(
            self.pending.into_inner(),
            self.base_typography,
            self.prepared_resource_error.into_inner(),
            self.prepared_work_error.into_inner(),
        );
        #[cfg(any(test, feature = "test-support"))]
        let sidecar = sidecar.with_source_plan_counts(self.source_plans_by_owner.into_inner());
        sidecar
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn prepared_count(&self) -> usize {
        self.pending.borrow().prepared.entry_count()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn prepared_hit_count(&self) -> usize {
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
fn measure_flowchart_svg_label_for_layout(
    sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    owner: Option<FlowchartSvgLabelOwner>,
    render_id: Option<&str>,
    request: FlowchartLabelMetricsRequest<'_>,
    width_mode: FlowchartSvgWidthMode,
) -> TextMetrics {
    measure_flowchart_svg_label_for_layout_with_source_style(
        sidecar, owner, render_id, request, None, width_mode,
    )
}

pub fn measure_flowchart_svg_label_for_layout_with_source_style(
    sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    owner: Option<FlowchartSvgLabelOwner>,
    render_id: Option<&str>,
    request: FlowchartLabelMetricsRequest<'_>,
    source_style: Option<&FlowchartTextStyleResolution<'_>>,
    width_mode: FlowchartSvgWidthMode,
) -> TextMetrics {
    match sidecar.zip(owner).zip(render_id) {
        Some(((sidecar, owner), render_id)) => sidecar.measure_for_layout_with_source_style(
            owner,
            render_id,
            request,
            source_style,
            true,
            width_mode,
        ),
        None => {
            let geometry = source_style.map_or(Default::default(), |style| style.html_geometry);
            measure_svg_label_without_sidecar(request, width_mode, geometry)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn measure_flowchart_svg_label_for_layout_with_metrics_and_source_style(
    sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    owner: Option<FlowchartSvgLabelOwner>,
    render_id: Option<&str>,
    request: FlowchartLabelMetricsRequest<'_>,
    metrics_style: &TextStyle,
    source_style: Option<&FlowchartTextStyleResolution<'_>>,
    width_mode: FlowchartSvgWidthMode,
) -> TextMetrics {
    match sidecar.zip(owner).zip(render_id) {
        Some(((sidecar, owner), render_id)) => sidecar
            .measure_for_layout_with_metrics_and_source_style(
                owner,
                render_id,
                request,
                metrics_style,
                source_style,
                true,
                width_mode,
            ),
        None => {
            let geometry = source_style.map_or(Default::default(), |style| style.html_geometry);
            measure_svg_label_without_sidecar_with_metrics_style(
                request,
                metrics_style,
                width_mode,
                geometry,
            )
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
    if !mermaid_markdown_is_plain_text(raw_label) {
        return false;
    }
    let markdown = analyze_mermaid_markdown(raw_label, true);
    if !markdown.all_runs_normal() {
        return false;
    }
    let svg_lines = flowchart_non_markdown_svg_source_word_lines(raw_label);
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
    geometry: HtmlTextGeometryDeclarations,
) -> TextMetrics {
    let metrics_style = request.style;
    measure_svg_label_without_sidecar_with_metrics_style(
        request,
        metrics_style,
        width_mode,
        geometry,
    )
}

fn measure_svg_label_without_sidecar_with_metrics_style(
    request: FlowchartLabelMetricsRequest<'_>,
    metrics_style: &TextStyle,
    width_mode: FlowchartSvgWidthMode,
    geometry: HtmlTextGeometryDeclarations,
) -> TextMetrics {
    if request.wrap_mode != WrapMode::SvgLike || request.label_type == "markdown" {
        return measure_flowchart_label_for_layout(
            FlowchartLabelMetricsRequest {
                style: metrics_style,
                ..request
            },
            geometry,
            true,
        );
    }
    let source = FlowchartSvgLabelSource::new(request.raw_label);
    let wrapped_lines =
        source.wrapped_lines(request.measurer, request.style, request.max_width_px, true);
    source.metrics_from_wrapped(request.measurer, metrics_style, &wrapped_lines, width_mode)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use merman_model::{
        MermaidConfig, OperationControl,
        resources::{InputResourcePolicy, ResourceProfile},
    };
    use merman_render_core::{
        math::{
            MathPreparationOutcome, MathPreparationUnavailable, MathRenderer, NoopMathRenderer,
        },
        resources::ResourceLimitId,
        text::{
            DeterministicTextMeasurer, TextMeasurer,
            measurement::{
                HostMeasurementResult, HostTextMeasurementRequest, HostTextMeasurer,
                MeasurementProfileId, RoutedTextMeasurer, TextMeasurementOperation,
                TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfile,
                TextMeasurementProfileIdentity, TextMeasurementSession, TextMeasurementSummary,
                measurement_reuse_key,
            },
        },
    };
    use quick_xml::events::Event;

    use super::super::{FlowchartPreparedLabelResolution, FlowchartSvgLabelRenderPlan};
    use super::*;

    fn presentation_config(value: serde_json::Value) -> PresentationConfig {
        PresentationConfig::from_config_controlled(
            MermaidConfig::from_value(value),
            &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap()
    }

    struct TestMeasurementSession {
        measurement: TextMeasurementSession,
        work: OperationWorkMeter,
    }

    impl TestMeasurementSession {
        fn new(policy: TextMeasurementPolicy) -> Self {
            Self {
                measurement: TextMeasurementSession::new(policy),
                work: OperationWorkMeter::new(RenderResourcePolicy::interactive()),
            }
        }

        fn deterministic() -> Self {
            Self::new(TextMeasurementPolicy::deterministic())
        }

        fn text_measurer(&self, phase: TextMeasurementPhase) -> RoutedTextMeasurer<'_> {
            self.measurement.routed(&self.work, phase, None)
        }
    }

    #[test]
    fn recovered_html_is_prepared_once_borrowed_by_terminal_and_releases_retained_bytes() {
        let work = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let builder = FlowchartSvgLabelSidecarBuilder::new_with_work_meter(Arc::clone(&work));
        let measurer = DeterministicTextMeasurer::default();
        let config = presentation_config(serde_json::json!({ "securityLevel": "loose" }));
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
        let FlowchartPreparedLabelResolution::PreparedHtml(prepared) =
            sidecar.prepared_label_for_terminal(owner, source)
        else {
            panic!("HTML label must be prepared before terminal emission")
        };
        let wrapped = format!("<root>{}</root>", prepared.xhtml());
        let mut reader = quick_xml::Reader::from_str(&wrapped);
        let mut italic_elements = 0;
        loop {
            match reader.read_event().expect("prepared label is valid XHTML") {
                Event::Start(tag) | Event::Empty(tag) if tag.name().as_ref() == "i" => {
                    italic_elements += 1;
                }
                Event::End(tag) if tag.name().as_ref() == "root" => break,
                Event::Eof => panic!("prepared XHTML must close the root"),
                _ => {}
            }
        }
        assert!(matches!(reader.read_event().unwrap(), Event::Eof));
        assert_eq!(italic_elements, 2);
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
        let config = presentation_config(serde_json::json!({ "securityLevel": "loose" }));
        let style = TextStyle::default();
        let measurer = DeterministicTextMeasurer::default();
        for cancel in [false, true] {
            let control = merman_model::OperationControl::new();
            let policy = if cancel {
                control.cancel_after_checkpoints(16);
                RenderResourcePolicy::unbounded_for_trusted_input()
            } else {
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
                    .unwrap()
            };
            let work = Arc::new(OperationWorkMeter::new_with_control(policy, control));
            let builder = FlowchartSvgLabelSidecarBuilder::new_with_work_meter(Arc::clone(&work));
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
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 1)
            .unwrap();
        let limited = Arc::new(OperationWorkMeter::new(policy));
        let style = TextStyle {
            font_family: Some("Caller Font".into()),
            ..TextStyle::default()
        };
        let error =
            FlowchartNodeSourceTypography::prepare(Cow::Owned(style.clone()), Some(&limited))
                .unwrap_err();
        assert_eq!(error.limit, "max_prepared_text_retained_bytes");
        assert_eq!(limited.prepared_text_retained_bytes(), 0);
        let work = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let projection = Arc::new(
            FlowchartNodeSourceTypography::prepare(Cow::Owned(style), Some(&work)).unwrap(),
        );
        assert!(work.prepared_text_retained_bytes() > 0);
        let terminal = Arc::clone(&projection);
        drop(projection);
        assert!(work.prepared_text_retained_bytes() > 0);
        drop(terminal);
        assert_eq!(work.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn render_id_budget_is_single_owned_across_source_and_math() {
        fn retained_for(render_id: &str, math: bool, owner: FlowchartSvgLabelOwner) -> usize {
            let work = Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input(),
            ));
            let config = PresentationConfig::default();
            let builder = FlowchartSvgLabelSidecarBuilder::new_with_work_meter(Arc::clone(&work));
            let style = TextStyle::default();
            let session = TestMeasurementSession::deterministic();
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

    fn report_call_count(session: &TestMeasurementSession) -> u64 {
        session
            .measurement
            .report()
            .entries()
            .iter()
            .map(TextMeasurementSummary::count)
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
        let config = presentation_config(serde_json::json!({ "securityLevel": "loose" }));
        let builder =
            FlowchartSvgLabelSidecarBuilder::default().with_math_backend(Some(&backend), &config);
        let measurer = DeterministicTextMeasurer::default();
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
        let config = presentation_config(serde_json::json!({
            "themeVariables": { "nodeTextColor": "#e5e7eb" }
        }));
        let builder =
            FlowchartSvgLabelSidecarBuilder::default().with_math_backend(Some(&backend), &config);
        let style = TextStyle {
            font_size: 24.0,
            ..TextStyle::default()
        };
        let measurer = DeterministicTextMeasurer::default();
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
                pending.math.get(owner).expect("layout artifact").outcome()
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
        let backend = ConfiguredMathBackend::external(Arc::new(NoopMathRenderer));
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
        let measurer = DeterministicTextMeasurer::default();
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
        let measurer = DeterministicTextMeasurer::default();
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
        assert_eq!(builder.pending.borrow().math.entry_count(), 0);
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
        let config = presentation_config(serde_json::json!({
            "themeVariables": { "nodeTextColor": "#111827" }
        }));
        let builder =
            FlowchartSvgLabelSidecarBuilder::default().with_math_backend(Some(&backend), &config);
        let style = TextStyle::default();
        let terminal_foreground = FlowchartTerminalForeground::new(
            "#f43f5e",
            FlowchartTerminalForegroundProvenance::InlineStyle,
        );
        let mut source_style = FlowchartTextStyleResolution::borrowed(&style);
        source_style.terminal_foreground = Some(terminal_foreground);
        let measurer = DeterministicTextMeasurer::default();
        let owner = FlowchartSvgLabelOwner::Node(0);

        builder.measure_for_layout_with_source_style(
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
            Some(&source_style),
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
                .foreground()
                .provenance(),
            FlowchartTerminalForegroundProvenance::InlineStyle
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
        let foreground = FlowchartTerminalForeground::new(
            "#334155",
            FlowchartTerminalForegroundProvenance::ThemeNode,
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
            let legacy = measure_flowchart_label_for_layout(
                request(),
                HtmlTextGeometryDeclarations::default(),
                true,
            );
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
                math_renderer: Some(&NoopMathRenderer),
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
        let session = TestMeasurementSession::deterministic();
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
        let session = TestMeasurementSession::deterministic();
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
                math_renderer: Some(&NoopMathRenderer),
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
        struct UnexpectedHost;

        impl HostTextMeasurer for UnexpectedHost {
            fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
                panic!("prepared lookup must not call the host: {request:?}");
            }
        }

        let session = TestMeasurementSession::deterministic();
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

        let host_identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.prepared-svg-mixed-routes").unwrap(),
            "1",
        )
        .unwrap();
        for (host_phase, expect_wrapping) in [
            (TextMeasurementPhase::Wrap, true),
            (TextMeasurementPhase::ComputedLength, false),
        ] {
            let mixed = TestMeasurementSession::new(TextMeasurementPolicy::host_display(
                host_identity.clone(),
                Arc::new(UnexpectedHost),
                [host_phase],
            ));
            let measurer = mixed.text_measurer(TextMeasurementPhase::Layout);
            let plan = FlowchartSvgLabelRenderPlan::new(
                Some(&sidecar),
                Some(owner),
                "bound label",
                &measurer,
                &style,
                Some(80.0),
                true,
                FlowchartSvgWidthMode::Bbox,
            );
            assert_eq!(
                matches!(plan, FlowchartSvgLabelRenderPlan::Prepared { .. }),
                expect_wrapping,
                "host phase {host_phase:?}"
            );
            assert!(
                sidecar
                    .prepared_metrics(
                        owner,
                        "bound label",
                        &measurer,
                        &style,
                        Some(80.0),
                        true,
                        FlowchartSvgWidthMode::Bbox,
                    )
                    .is_none(),
                "terminal metrics must not reuse a host route with a built-in fallback"
            );
        }
    }

    #[test]
    fn streaming_state_without_builtin_carriers_is_not_cached() {
        let direct = DeterministicTextMeasurer::default();
        let wrap_profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.opaque-svg-wrap").unwrap(),
                "1",
            )
            .unwrap(),
            Arc::new(DeterministicTextMeasurer::default()),
        );
        let session = TestMeasurementSession::new(
            TextMeasurementPolicy::deterministic()
                .with_profile_for_phase(TextMeasurementPhase::Wrap, wrap_profile),
        );
        let routed = session.text_measurer(TextMeasurementPhase::Layout);
        assert!(measurement_reuse_key(&direct, TextMeasurementOperation::ComputedLength).is_none());
        assert!(measurement_reuse_key(&routed, TextMeasurementOperation::ComputedLength).is_some());
        assert!(measurement_reuse_key(&routed, TextMeasurementOperation::Wrapped).is_none());

        let literal_bbox_calls = || {
            let report = session.measurement.report();
            assert!(!report.entries().iter().any(|entry| {
                matches!(
                    entry.provenance().operation,
                    TextMeasurementOperation::Wrapped
                        | TextMeasurementOperation::WrappedWithRawWidth
                )
            }));
            report
                .entries()
                .iter()
                .filter(|entry| {
                    matches!(
                        entry.provenance().operation,
                        TextMeasurementOperation::TspanBBoxWidth
                            | TextMeasurementOperation::TspanBBoxHeight
                    )
                })
                .map(TextMeasurementSummary::count)
                .sum::<u64>()
        };
        let config = PresentationConfig::default();
        let style = TextStyle::default();
        let owner = FlowchartSvgLabelOwner::Node(0);
        for (measurer, records_calls) in [(&direct as &dyn TextMeasurer, false), (&routed, true)] {
            let builder = FlowchartSvgLabelSidecarBuilder::default();
            let measure = || {
                builder.measure_for_layout(
                    owner,
                    "node",
                    FlowchartLabelMetricsRequest {
                        measurer,
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
            let first_bbox_calls = literal_bbox_calls();
            let second = measure();
            assert_eq!(
                (first.width, first.height, first.line_count),
                (second.width, second.height, second.line_count)
            );
            if records_calls {
                assert!(
                    first_bbox_calls > 0,
                    "layout must measure literal SVG row bounds"
                );
                assert_eq!(
                    literal_bbox_calls(),
                    2 * first_bbox_calls,
                    "layout must repeat bbox probes when terminal metrics cannot be reused"
                );
            }
            assert_eq!(builder.prepared_count(), 0);
            let sidecar = builder.finish();
            let calls_before_render = literal_bbox_calls();
            let plan = FlowchartSvgLabelRenderPlan::new(
                Some(&sidecar),
                Some(owner),
                "opaque callback contract",
                measurer,
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
                literal_bbox_calls(),
                calls_before_render,
                "render wrapping must not repeat literal SVG bbox probes"
            );
        }
    }

    #[test]
    fn duplicate_subgraph_ids_resolve_to_the_first_semantic_owner() {
        let session = TestMeasurementSession::deterministic();
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
        let session = TestMeasurementSession::deterministic();
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
        let session = TestMeasurementSession::deterministic();
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
            assert_eq!(pending.sources.entry_count(), 1);
            assert_eq!(pending.prepared.entry_count(), 1);
            assert_eq!(pending.render_ids.entry_count(), 1);
        }

        let sidecar = builder.finish();
        assert_eq!(sidecar.row_counts(), (1, 1));
        assert_eq!(sidecar.edge_owner("sparse-edge", false), Some(owner));
    }

    #[test]
    fn edge_preparation_wraps_with_base_style_and_measures_with_final_style() {
        let session = TestMeasurementSession::deterministic();
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
