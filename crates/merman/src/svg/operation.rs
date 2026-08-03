use super::{
    LayoutOptions, Result, ResvgCompatibleSvg, SvgDebugOptions, SvgPipeline, SvgRenderOptions,
};
use merman_render::{
    ResourceLimitExceeded,
    environment::{RenderEnvironment, RenderSession, RenderSessionReport},
    presentation::PresentationRenderPolicy,
};

/// Stable identity of the operation that produced a retained render result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RenderExecutionPath {
    HeadlessOperationTyped,
}

impl RenderExecutionPath {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HeadlessOperationTyped => "headless-operation-typed",
        }
    }
}

/// Immutable evidence emitted only after a typed headless SVG operation completes successfully.
///
/// A fresh render session yields [`RenderSessionReport`], which cannot be substituted for this
/// completed operation report. Its completion fields are private, so external callers cannot
/// forge one from a fresh session. The public [`crate::svg`] module documentation carries the
/// compile-fail contract tests for both boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOperationReport {
    execution_path: RenderExecutionPath,
    session: RenderSessionReport,
}

impl RenderOperationReport {
    pub const fn execution_path(&self) -> RenderExecutionPath {
        self.execution_path
    }

    pub fn measurement_routes(&self) -> &[super::TextMeasurementRoute; 4] {
        self.session.measurement_routes()
    }

    pub fn measurement(&self) -> &super::TextMeasurementReport {
        self.session.measurement()
    }

    pub fn operation_context(&self) -> &merman_core::runtime::OperationContext {
        self.session.operation_context()
    }

    pub const fn unix_millis(&self) -> i64 {
        self.session.unix_millis()
    }

    pub const fn local_date(&self) -> chrono::NaiveDate {
        self.session.local_date()
    }

    pub fn local_time_zone(&self) -> &merman_core::time::LocalTimeZoneProvenance {
        self.session.local_time_zone()
    }

    pub fn render_seed(&self) -> std::num::NonZeroU64 {
        self.session.render_seed()
    }
}

struct CompletedTypedHeadlessSvg {
    session: RenderSession,
}

impl CompletedTypedHeadlessSvg {
    fn new(session: RenderSession) -> Self {
        Self { session }
    }

    fn into_report(self) -> RenderOperationReport {
        RenderOperationReport {
            execution_path: RenderExecutionPath::HeadlessOperationTyped,
            session: self.session.report(),
        }
    }
}

pub(super) struct HeadlessOperation<'a> {
    engine: merman_core::Engine,
    text: &'a str,
    parse_options: merman_core::ParseOptions,
    layout_options: &'a LayoutOptions,
    session: RenderSession,
    render_policy: PresentationRenderPolicy,
}

/// A canonical typed parse that has not started layout yet.
///
/// Callers can inspect metadata and the family-owned semantic model to apply admission policy.
/// Continuing to layout consumes this stage, so the same parsed model cannot start layout twice.
pub struct PreparedSemantic {
    parsed: merman_core::ParsedDiagramRender,
    layout_options: LayoutOptions,
    session: RenderSession,
    render_policy: PresentationRenderPolicy,
}

impl PreparedSemantic {
    /// Returns preprocessed metadata for admission and diagnostics.
    pub fn metadata(&self) -> &merman_core::ParseMetadata {
        self.parsed.metadata()
    }

    /// Returns the family-owned typed semantic variant name without exposing mutable pairing.
    pub fn semantic_kind(&self) -> &'static str {
        self.parsed.model().kind()
    }

    /// Reports required and missing renderer capabilities without starting layout.
    pub fn render_plan(&self) -> Result<merman_render::family::RenderCapabilityPlan> {
        Ok(merman_render::family::plan_render_with_policy(
            &self.parsed,
            &self.session,
            self.render_policy,
        )?)
    }

    /// Runs layout exactly once and advances to the pre-SVG render stage.
    pub fn continue_layout(self) -> Result<PreparedRender> {
        let Self {
            parsed,
            layout_options,
            session,
            render_policy,
        } = self;
        let artifact = merman_render::family::prepare_with_render_policy(
            parsed,
            &layout_options,
            session,
            render_policy,
        )?;
        Ok(PreparedRender { artifact })
    }
}

/// A canonical typed render that has completed parsing and layout exactly once.
///
/// The artifact exposes metadata, a stable semantic family label, and compatibility layout JSON.
/// SVG rendering consumes it, which prevents the prepared parse/layout result from being rendered
/// more than once while keeping the typed semantic/layout pair opaque and internally consistent.
/// The public [`crate::svg`] module documentation carries the compile-fail contract test for
/// this consuming boundary.
pub struct PreparedRender {
    artifact: merman_render::family::FamilyRenderArtifact,
}

/// Complete SVG output plus immutable evidence from the operation-owned session.
pub struct RenderedSvg {
    svg: String,
    report: RenderOperationReport,
}

impl RenderedSvg {
    pub fn svg(&self) -> &str {
        &self.svg
    }

    pub fn report(&self) -> &RenderOperationReport {
        &self.report
    }

    pub fn into_svg(self) -> String {
        self.svg
    }

    pub fn into_parts(self) -> (String, RenderOperationReport) {
        (self.svg, self.report)
    }
}

impl PreparedRender {
    /// Returns preprocessed metadata for diagnostics and request policy.
    pub fn metadata(&self) -> &merman_core::ParseMetadata {
        self.artifact.metadata()
    }

    /// Returns the paired built-in render family without exposing its semantic or layout types.
    pub fn family_kind(&self) -> super::RenderFamilyKind {
        self.artifact.family_kind()
    }

    /// Returns an owned inverse projection of the Gantt time axis for parity diagnostics.
    pub fn gantt_time_axis_diagnostics(
        &self,
    ) -> Option<merman_render::family::GanttTimeAxisDiagnostics> {
        self.artifact.gantt_time_axis_diagnostics()
    }

    /// Serializes metadata, compatibility semantics, and typed layout from this artifact.
    pub fn layout_json(&self) -> Result<serde_json::Value> {
        Ok(self.artifact.layout_json()?)
    }

    /// Renders SVG once from the prepared semantic model and layout.
    ///
    /// Request-scoped values in `svg_options` affect geometry and SVG identity only. Production
    /// dependencies and deterministic policy come from the operation-owned render session.
    pub fn render_svg(self, svg_options: &SvgRenderOptions) -> Result<String> {
        self.render_svg_with_debug(svg_options, &SvgDebugOptions::default())
    }

    pub fn render_svg_with_debug(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
    ) -> Result<String> {
        self.render_svg_report_with_debug(svg_options, debug_options)
            .map(RenderedSvg::into_svg)
    }

    pub fn render_svg_report(self, svg_options: &SvgRenderOptions) -> Result<RenderedSvg> {
        self.render_svg_report_with_debug(svg_options, &SvgDebugOptions::default())
    }

    pub fn render_svg_report_with_debug(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
    ) -> Result<RenderedSvg> {
        self.render_svg_parts(svg_options, debug_options)
            .map(RenderedSvgParts::into_rendered_svg)
    }

    /// Renders SVG once and applies the supplied postprocessing pipeline.
    pub fn render_svg_with_pipeline(
        self,
        svg_options: &SvgRenderOptions,
        pipeline: &SvgPipeline,
    ) -> Result<String> {
        self.render_svg_with_pipeline_and_debug(svg_options, &SvgDebugOptions::default(), pipeline)
    }

    pub fn render_svg_with_pipeline_and_debug(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
        pipeline: &SvgPipeline,
    ) -> Result<String> {
        self.render_svg_with_pipeline_report_and_debug(svg_options, debug_options, pipeline)
            .map(RenderedSvg::into_svg)
    }

    pub fn render_svg_with_pipeline_report(
        self,
        svg_options: &SvgRenderOptions,
        pipeline: &SvgPipeline,
    ) -> Result<RenderedSvg> {
        self.render_svg_with_pipeline_report_and_debug(
            svg_options,
            &SvgDebugOptions::default(),
            pipeline,
        )
    }

    pub fn render_svg_with_pipeline_report_and_debug(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
        pipeline: &SvgPipeline,
    ) -> Result<RenderedSvg> {
        self.render_svg_parts(svg_options, debug_options)?
            .into_pipeline_svg(pipeline)
    }

    /// Renders and terminally finalizes SVG for resvg/raster consumers.
    ///
    /// The supplied pipeline contributes draft transformations. Its terminal preset is always
    /// replaced with `resvg_safe`, so no custom pass can run after compatibility validation.
    pub fn render_resvg_compatible_svg(
        self,
        svg_options: &SvgRenderOptions,
        pipeline: &SvgPipeline,
    ) -> Result<ResvgCompatibleSvg> {
        self.render_resvg_compatible_svg_with_debug(
            svg_options,
            &SvgDebugOptions::default(),
            pipeline,
        )
        .map(|(svg, _)| svg)
    }

    pub fn render_resvg_compatible_svg_with_debug(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
        pipeline: &SvgPipeline,
    ) -> Result<(ResvgCompatibleSvg, RenderOperationReport)> {
        let pipeline = pipeline.clone().into_resvg_safe();
        self.render_svg_parts(svg_options, debug_options)?
            .into_resvg_compatible(&pipeline)
    }

    fn render_svg_parts(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
    ) -> Result<RenderedSvgParts> {
        let rendered = self.artifact.render_svg(svg_options, debug_options)?;
        Ok(RenderedSvgParts { rendered })
    }
}

impl<'a> HeadlessOperation<'a> {
    pub(super) fn new(
        engine: &merman_core::Engine,
        text: &'a str,
        parse_options: merman_core::ParseOptions,
        layout_options: &'a LayoutOptions,
        environment: &RenderEnvironment,
    ) -> Result<Self> {
        Self::new_with_render_policy(
            engine,
            text,
            parse_options,
            layout_options,
            environment,
            PresentationRenderPolicy::default(),
        )
    }

    pub(super) fn new_with_render_policy(
        engine: &merman_core::Engine,
        text: &'a str,
        parse_options: merman_core::ParseOptions,
        layout_options: &'a LayoutOptions,
        environment: &RenderEnvironment,
        render_policy: PresentationRenderPolicy,
    ) -> Result<Self> {
        let session = environment.begin_session()?;
        let engine = super::engine_with_session_context(engine, &session);
        Ok(Self {
            engine,
            text,
            parse_options,
            layout_options,
            session,
            render_policy,
        })
    }

    pub(super) fn layout_json(self) -> Result<Option<serde_json::Value>> {
        let Some(prepared) = self.prepare_render()? else {
            return Ok(None);
        };
        Ok(Some(prepared.layout_json()?))
    }

    pub(super) fn prepare_render(self) -> Result<Option<PreparedRender>> {
        let Some(semantic) = self.prepare_semantic()? else {
            return Ok(None);
        };

        Ok(Some(semantic.continue_layout()?))
    }

    pub(super) fn plan_render(self) -> Result<Option<merman_render::family::RenderCapabilityPlan>> {
        let Some(semantic) = self.prepare_semantic()? else {
            return Ok(None);
        };
        Ok(Some(semantic.render_plan()?))
    }

    pub(super) fn prepare_semantic(self) -> Result<Option<PreparedSemantic>> {
        self.session
            .resource_policy()
            .check_source_bytes(self.text)
            .map_err(resource_limit_error)?;
        let Some(parsed) = self
            .engine
            .parse_diagram_for_render_model_sync(self.text, self.parse_options)?
        else {
            return Ok(None);
        };

        Ok(Some(PreparedSemantic {
            parsed,
            layout_options: self.layout_options.clone(),
            session: self.session,
            render_policy: self.render_policy,
        }))
    }

    pub(super) fn render_svg(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
    ) -> Result<Option<String>> {
        let Some(prepared) = self.prepare_render()? else {
            return Ok(None);
        };

        Ok(Some(
            prepared.render_svg_with_debug(svg_options, debug_options)?,
        ))
    }

    pub(super) fn render_svg_with_pipeline(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
        pipeline: &SvgPipeline,
    ) -> Result<Option<String>> {
        let Some(prepared) = self.prepare_render()? else {
            return Ok(None);
        };

        Ok(Some(prepared.render_svg_with_pipeline_and_debug(
            svg_options,
            debug_options,
            pipeline,
        )?))
    }
}

struct RenderedSvgParts {
    rendered: merman_render::family::RenderedFamilySvg,
}

impl RenderedSvgParts {
    fn into_rendered_svg(self) -> RenderedSvg {
        let (svg, _, _, session) = self.rendered.into_parts();
        let report = CompletedTypedHeadlessSvg::new(session).into_report();
        RenderedSvg { svg, report }
    }

    fn into_pipeline_svg(self, pipeline: &SvgPipeline) -> Result<RenderedSvg> {
        let rendered = self.rendered.apply_pipeline(pipeline)?;
        let (svg, _, _, session) = rendered.into_parts();
        let report = CompletedTypedHeadlessSvg::new(session).into_report();
        Ok(RenderedSvg { svg, report })
    }

    fn into_resvg_compatible(
        self,
        pipeline: &SvgPipeline,
    ) -> Result<(ResvgCompatibleSvg, RenderOperationReport)> {
        let rendered = self.rendered.finalize_resvg(pipeline)?;
        let (svg, _, _, session) = rendered.into_parts();
        let report = CompletedTypedHeadlessSvg::new(session).into_report();
        Ok((svg, report))
    }
}

pub(super) fn resource_limit_error(err: ResourceLimitExceeded) -> super::HeadlessError {
    merman_render::Error::ResourceLimitExceeded(err).into()
}
