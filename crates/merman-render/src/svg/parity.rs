use super::pipeline::{ScopedCssPostprocessor, SvgPipeline, SvgPostprocessMetadata};
use crate::environment::{RenderSession, RoutedTextMeasurer, TextMeasurementPhase};
#[cfg(feature = "layout-cytoscape")]
use crate::model::ArchitectureDiagramLayout;
use crate::model::{
    BlockDiagramLayout, Bounds, ClassDiagramLayout, CynefinDiagramLayout, ErDiagramLayout,
    ErrorDiagramLayout, EventModelingDiagramLayout, FlowchartLayout, InfoDiagramLayout,
    IshikawaDiagramLayout, LayoutCluster, LayoutNode, MindmapDiagramLayout, PacketDiagramLayout,
    PieDiagramLayout, QuadrantChartDiagramLayout, RadarDiagramLayout, RailroadDiagramLayout,
    SankeyDiagramLayout, SequenceDiagramLayout, StateDiagramLayout, TimelineDiagramLayout,
    TreeViewDiagramLayout, VennDiagramLayout, XyChartDiagramLayout,
};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use base64::Engine as _;
use indexmap::IndexMap;
use std::fmt::Write as _;

#[cfg(feature = "layout-cytoscape")]
mod architecture;
mod block;
mod c4;
mod class;
mod css;
mod curve;
mod cynefin;
mod edge_label_geometry;
mod emitted_bounds;
mod er;
mod error;
mod eventmodeling;
mod flowchart;
mod gantt;
mod gitgraph;
mod info;
mod ishikawa;
mod journey;
mod kanban;
mod label;
mod layout_debug;
mod mindmap;
mod packet;
mod path_bounds;
mod pie;
mod quadrantchart;
mod radar;
mod railroad;
mod requirement;
mod root_svg;
mod roughjs_common;
mod sankey;
mod sequence;
mod state;
mod style;
pub(crate) mod theme;
mod timeline;
mod timing;
mod tree_view;
mod treemap;
mod util;
mod venn;
mod wardley;
mod xychart;
mod zenuml;
use css::{
    er_css, gantt_css, info_css_parts_with_config, info_css_parts_with_theme_font_size_only,
    info_css_with_config, pie_css, push_xychart_css, requirement_css, sankey_css, treemap_css,
};
use path_bounds::{svg_path_bounds_from_d, svg_path_length_from_d};
pub(crate) fn mindmap_cloud_rendered_bbox_size_px(w: f64, h: f64) -> Option<(f64, f64)> {
    mindmap::mindmap_cloud_rendered_bbox_size_px(w, h)
}

pub use emitted_bounds::{
    SvgEmittedBoundsContributor, SvgEmittedBoundsDebug, debug_svg_emitted_bounds,
};
use emitted_bounds::{svg_emitted_bounds_from_svg, svg_emitted_bounds_from_svg_inner};
use state::{roughjs_ops_to_svg_path_d, roughjs_parse_hex_color_to_srgba, roughjs_paths_for_rect};
use style::{is_rect_style_key, is_text_style_key, parse_style_decl};
use theme::PresentationTheme;
use util::{
    SvgTheme, config_bool, config_diagram_look, config_f64, config_f64_css_px, config_string,
    css_rgba_fade, decode_mermaid_entities_for_render_text, escape_attr, escape_attr_display,
    escape_attr_into, escape_xml, escape_xml_display, escape_xml_into, fmt, fmt_display, fmt_into,
    fmt_path, fmt_path_into, fmt_points, fmt_string, json_stringify_points,
    json_stringify_points_into, normalize_css_font_family, scoped_svg_id, scoped_svg_url,
    theme_token,
};

/// Converts arbitrary host input into the single conservative SVG id grammar used by every
/// family renderer.
pub fn sanitize_svg_id(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return "m-untitled".to_string();
    }

    let mut iter = raw.chars();
    let Some(first_raw) = iter.next() else {
        return "m-untitled".to_string();
    };

    let sanitize_char = |ch: char| {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.') {
            ch
        } else {
            '-'
        }
    };
    let first = sanitize_char(first_raw);
    let mut out = String::with_capacity(raw.len() + 2);
    let mut previous_was_dash = false;

    if !first.is_ascii_alphabetic() {
        out.push('m');
        if first != '-' {
            out.push('-');
            previous_was_dash = true;
        }
    }

    let push = |ch: char, out: &mut String, previous_was_dash: &mut bool| {
        if ch == '-' {
            if *previous_was_dash {
                return;
            }
            *previous_was_dash = true;
        } else {
            *previous_was_dash = false;
        }
        out.push(ch);
    };

    push(first, &mut out, &mut previous_was_dash);
    for ch in iter {
        push(sanitize_char(ch), &mut out, &mut previous_was_dash);
    }
    while out.ends_with('-') {
        out.pop();
    }

    if out.is_empty() || out == "m" {
        "m-untitled".to_string()
    } else {
        out
    }
}

#[derive(Debug, Clone)]
pub struct SvgRenderOptions {
    /// Adds extra space around the computed viewBox.
    pub viewbox_padding: f64,
    /// Optional diagram id used for Mermaid-like marker ids.
    pub diagram_id: Option<String>,
}

impl Default for SvgRenderOptions {
    fn default() -> Self {
        Self {
            viewbox_padding: 8.0,
            diagram_id: None,
        }
    }
}

impl SvgRenderOptions {
    pub(crate) fn normalized(&self) -> Self {
        Self {
            viewbox_padding: self.viewbox_padding,
            diagram_id: self.diagram_id.as_deref().map(sanitize_svg_id),
        }
    }
}

/// A point captured while diagnosing one flowchart edge route.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FlowchartEdgeTracePoint {
    pub x: f64,
    pub y: f64,
}

/// An in-memory diagnostic record for one flowchart edge route.
///
/// The renderer never serializes this value or writes it to a host filesystem. Callers that need
/// a file own that I/O boundary and can serialize a drained record themselves.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FlowchartEdgeTrace {
    pub fixture_diagram_id: String,
    pub edge_id: String,
    pub from: String,
    pub to: String,
    pub layout_from: String,
    pub layout_to: String,
    pub from_cluster: Option<String>,
    pub to_cluster: Option<String>,
    pub origin_x: f64,
    pub origin_y: f64,
    pub tx: f64,
    pub ty: f64,
    pub base_points: Vec<FlowchartEdgeTracePoint>,
    pub points_after_intersect: Vec<FlowchartEdgeTracePoint>,
    pub points_for_render: Vec<FlowchartEdgeTracePoint>,
    pub points_for_data_points: Vec<FlowchartEdgeTracePoint>,
}

/// Explicit, caller-owned storage for flowchart route diagnostics.
///
/// Clones share one collection so a caller can retain a handle while a render owns a clone. A
/// poisoned lock is recovered because trace collection is diagnostic-only and must not turn a
/// completed render into an ambient host failure.
#[derive(Debug, Clone, Default)]
pub struct FlowchartEdgeTraceCollector(std::sync::Arc<std::sync::Mutex<Vec<FlowchartEdgeTrace>>>);

impl FlowchartEdgeTraceCollector {
    pub(crate) fn record(&self, trace: FlowchartEdgeTrace) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(trace);
    }

    /// Returns a snapshot without consuming the collected records.
    pub fn snapshot(&self) -> Vec<FlowchartEdgeTrace> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Drains all records collected so far.
    pub fn drain(&self) -> Vec<FlowchartEdgeTrace> {
        std::mem::take(
            &mut *self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
}

/// An opaque flowchart trace request created by [`SvgDebugOptions::with_flowchart_edge_trace`].
#[derive(Debug, Clone)]
pub struct FlowchartEdgeTraceRequest {
    edge_id: String,
    collector: FlowchartEdgeTraceCollector,
}

/// Diagnostic visibility controls kept separate from production render requests.
#[derive(Debug, Clone)]
pub struct SvgDebugOptions {
    pub include_edges: bool,
    pub include_nodes: bool,
    pub include_clusters: bool,
    pub include_cluster_debug_markers: bool,
    pub include_edge_id_labels: bool,
    pub include_timing_diagnostics: bool,
    /// Optional caller-owned trace collection request.
    ///
    /// Use [`SvgDebugOptions::with_flowchart_edge_trace`] to construct a non-empty request. The
    /// field remains publicly updateable so existing `SvgDebugOptions { ..Default::default() }`
    /// callers keep their normal Rust struct-update ergonomics.
    pub flowchart_edge_trace: Option<FlowchartEdgeTraceRequest>,
}

impl Default for SvgDebugOptions {
    fn default() -> Self {
        Self {
            include_edges: true,
            include_nodes: true,
            include_clusters: true,
            include_cluster_debug_markers: false,
            include_edge_id_labels: false,
            include_timing_diagnostics: false,
            flowchart_edge_trace: None,
        }
    }
}

impl SvgDebugOptions {
    /// Captures diagnostics for one edge in caller-owned memory.
    ///
    /// This replaces the former implicit current-working-directory trace file. Hosts that want a
    /// file must drain `collector` after rendering and perform their own checked I/O.
    pub fn with_flowchart_edge_trace(
        mut self,
        edge_id: impl Into<String>,
        collector: FlowchartEdgeTraceCollector,
    ) -> Self {
        self.flowchart_edge_trace = Some(FlowchartEdgeTraceRequest {
            edge_id: edge_id.into(),
            collector,
        });
        self
    }

    pub(crate) fn flowchart_edge_trace(&self) -> Option<(&str, &FlowchartEdgeTraceCollector)> {
        self.flowchart_edge_trace
            .as_ref()
            .map(|trace| (trace.edge_id.as_str(), &trace.collector))
    }
}

pub(crate) struct SvgExecution<'a> {
    request: &'a SvgRenderOptions,
    session: &'a RenderSession,
    text_measurer: RoutedTextMeasurer<'a>,
    timing: timing::RenderTiming,
    pub(crate) debug: &'a SvgDebugOptions,
}

impl<'a> SvgExecution<'a> {
    fn new(
        request: &'a SvgRenderOptions,
        debug: &'a SvgDebugOptions,
        session: &'a RenderSession,
    ) -> Result<Self> {
        let timing = if debug.include_timing_diagnostics {
            timing::RenderTiming::enabled(
                session
                    .operation_context()
                    .require_timing()
                    .map_err(Error::from)?,
            )
        } else {
            timing::RenderTiming::disabled()
        };
        Ok(Self {
            request,
            session,
            text_measurer: session.text_measurer(TextMeasurementPhase::SvgBBox),
            timing,
            debug,
        })
    }

    pub(crate) fn text_measurer(&self) -> &dyn TextMeasurer {
        &self.text_measurer
    }

    pub(crate) fn text_measurer_for(&self, phase: TextMeasurementPhase) -> RoutedTextMeasurer<'_> {
        self.session.text_measurer(phase)
    }

    pub(crate) fn math_renderer(&self) -> Option<&(dyn crate::math::MathRenderer + Send + Sync)> {
        self.session.math_renderer()
    }

    pub(crate) fn icon_registry(&self) -> Option<&super::icon_registry::IconRegistry> {
        self.session.icon_registry()
    }

    pub(crate) fn unix_ms(&self) -> i64 {
        self.session.unix_millis()
    }

    pub(crate) fn local_time_zone(&self) -> &merman_core::time::LocalTimeZone {
        self.session.local_time_zone()
    }

    pub(crate) fn seed(&self) -> u64 {
        self.session.render_seed().get()
    }

    pub(crate) fn rough_randomness(
        &self,
        configured_seed: f64,
        owner_domain: &str,
    ) -> roughr::core::RoughRandomness {
        let resolved_seed = if configured_seed == 0.0 {
            self.seed() as f64
        } else {
            configured_seed
        };
        let operation = self.session.operation_context();
        roughr::core::RoughRandomness::new(
            roughr::core::RoughJsSeed::new(resolved_seed),
            roughr::core::RoughMathRandom::new(operation.derive_u64(owner_domain, 0)),
        )
    }

    pub(crate) fn timing(&self) -> timing::RenderTiming {
        self.timing
    }

    pub(crate) fn work_meter(&self) -> &crate::resources::OperationWorkMeter {
        self.session.work_meter().as_ref()
    }
}

impl std::ops::Deref for SvgExecution<'_> {
    type Target = SvgRenderOptions;

    fn deref(&self) -> &Self::Target {
        self.request
    }
}

#[cfg(test)]
pub(crate) fn with_test_svg_execution<T>(
    request: &SvgRenderOptions,
    run: impl FnOnce(&SvgExecution<'_>) -> T,
) -> T {
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session()
        .expect("create test render session");
    let debug = SvgDebugOptions::default();
    let execution = SvgExecution::new(request, &debug, &session)
        .expect("default test SVG execution does not request timing");
    run(&execution)
}

pub(crate) fn render_builtin_family_artifact(
    family: &crate::family::BuiltinFamilyArtifact,
    metadata: &merman_core::ParseMetadata,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    let execution = SvgExecution::new(options, debug, session)?;
    let rooted_svg = render_builtin_family_artifact_raw(family, metadata, &execution)?;
    let svg = rooted_svg.into_string_for(family.kind())?;
    apply_theme_css(svg, metadata.effective_config.as_value(), session)
}

#[cfg(feature = "layout-cytoscape")]
#[inline(never)]
pub(crate) fn render_architecture_family_artifact(
    pair: &crate::family::FamilyPair<
        merman_core::diagrams::architecture::ArchitectureDiagramRenderModel,
        ArchitectureDiagramLayout,
    >,
    effective_config: &merman_core::MermaidConfig,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    // Keep the deep-group Architecture path out of the heterogeneous dispatcher so it fits in
    // the renderer's supported low-stack worker budget.
    let execution = SvgExecution::new(options, debug, session)?;
    let rooted_svg = architecture::render_architecture_diagram_svg_typed_with_config(
        pair.layout(),
        pair.semantic(),
        effective_config,
        &execution,
    )?;
    let svg = rooted_svg.into_string_for(crate::family::RenderFamilyKind::Architecture)?;
    apply_theme_css(svg, effective_config.as_value(), session)
}

fn render_builtin_family_artifact_raw(
    family: &crate::family::BuiltinFamilyArtifact,
    metadata: &merman_core::ParseMetadata,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    use crate::family::BuiltinFamilyArtifact;

    let measurer = options.text_measurer();
    let effective_config = &metadata.effective_config;
    let effective_config_value = effective_config.as_value();
    let diagram_type = metadata.diagram_type.as_str();
    let title = metadata.title.as_deref();

    match family {
        BuiltinFamilyArtifact::Error(pair) => error::render_error_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            options,
        ),
        #[cfg(feature = "layout-cytoscape")]
        BuiltinFamilyArtifact::Architecture(pair) => {
            architecture::render_architecture_diagram_svg_typed_with_config(
                pair.layout(),
                pair.semantic(),
                effective_config,
                options,
            )
        }
        BuiltinFamilyArtifact::Flowchart(artifact) => {
            flowchart::render_flowchart_svg_model_with_config(
                artifact.pair().layout(),
                artifact.pair().semantic(),
                effective_config,
                diagram_type,
                title,
                artifact.policy(),
                options,
            )
        }
        BuiltinFamilyArtifact::Swimlane(pair) => flowchart::render_swimlane_svg_model_with_config(
            pair.layout(),
            pair.semantic(),
            effective_config,
            diagram_type,
            title,
            options,
        ),
        BuiltinFamilyArtifact::Cynefin(pair) => cynefin::render_cynefin_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            title,
            options,
        ),
        BuiltinFamilyArtifact::Wardley(pair) => wardley::render_wardley_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            title,
            options,
        ),
        BuiltinFamilyArtifact::Railroad(pair) => railroad::render_railroad_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            measurer,
            options,
        ),
        BuiltinFamilyArtifact::Mindmap(pair) => {
            mindmap::render_mindmap_diagram_svg_model_with_config(
                pair.layout(),
                pair.semantic(),
                effective_config,
                options,
            )
        }
        BuiltinFamilyArtifact::State(pair) => state::render_state_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            title,
            measurer,
            options,
        ),
        BuiltinFamilyArtifact::Class(pair) => class::render_class_diagram_svg_model_with_config(
            pair.layout(),
            pair.semantic(),
            effective_config,
            title,
            measurer,
            options,
        ),
        BuiltinFamilyArtifact::Sequence(pair) => {
            sequence::render_sequence_diagram_svg_model_with_config(
                pair.layout(),
                pair.semantic(),
                effective_config,
                title,
                measurer,
                options,
            )
        }
        BuiltinFamilyArtifact::Zenuml(pair) => zenuml::render_zenuml_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            title,
            options,
        ),
        BuiltinFamilyArtifact::Kanban(pair) => {
            kanban::render_kanban_diagram_svg(pair.layout(), effective_config, options)
        }
        BuiltinFamilyArtifact::Gantt(pair) => gantt::render_gantt_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            options,
        ),
        BuiltinFamilyArtifact::Pie(pair) => pie::render_pie_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            options,
        ),
        BuiltinFamilyArtifact::Packet(pair) => packet::render_packet_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            title,
            options,
        ),
        BuiltinFamilyArtifact::Timeline(pair) => timeline::render_timeline_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            title,
            measurer,
            options,
        ),
        BuiltinFamilyArtifact::Journey(pair) => journey::render_journey_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            title,
            measurer,
            options,
        ),
        BuiltinFamilyArtifact::Requirement(pair) => {
            requirement::render_requirement_diagram_svg_model(
                pair.layout(),
                pair.semantic(),
                effective_config,
                title,
                measurer,
                options,
            )
        }
        BuiltinFamilyArtifact::Sankey(pair) => {
            sankey::render_sankey_diagram_svg(pair.layout(), effective_config_value, options)
        }
        BuiltinFamilyArtifact::Radar(pair) => radar::render_radar_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            title,
            options,
        ),
        BuiltinFamilyArtifact::Info(pair) => {
            info::render_info_diagram_svg(pair.layout(), effective_config_value, options)
        }
        BuiltinFamilyArtifact::Treemap(pair) => {
            treemap::render_treemap_diagram_svg(pair.layout(), effective_config_value, options)
        }
        BuiltinFamilyArtifact::Venn(pair) => venn::render_venn_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            title,
            options,
        ),
        BuiltinFamilyArtifact::Block(pair) => block::render_block_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            options,
        ),
        BuiltinFamilyArtifact::Er(pair) => er::render_er_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            title,
            measurer,
            options,
        ),
        BuiltinFamilyArtifact::QuadrantChart(pair) => {
            quadrantchart::render_quadrantchart_diagram_svg(
                pair.layout(),
                pair.semantic(),
                effective_config_value,
                options,
            )
        }
        BuiltinFamilyArtifact::XyChart(pair) => xychart::render_xychart_diagram_svg(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            options,
        ),
        BuiltinFamilyArtifact::GitGraph(pair) => gitgraph::render_gitgraph_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            title,
            measurer,
            options,
        ),
        BuiltinFamilyArtifact::TreeView(pair) => tree_view::render_tree_view_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            options,
        ),
        BuiltinFamilyArtifact::Ishikawa(pair) => {
            ishikawa::render_ishikawa_diagram_svg(pair.layout(), effective_config_value, options)
        }
        BuiltinFamilyArtifact::EventModeling(pair) => {
            eventmodeling::render_eventmodeling_diagram_svg(
                pair.layout(),
                pair.semantic(),
                effective_config_value,
                options,
            )
        }
        BuiltinFamilyArtifact::C4(pair) => c4::render_c4_diagram_svg_typed(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            title,
            measurer,
            options,
        ),
    }
}

fn apply_theme_css(
    svg: String,
    effective_config: &serde_json::Value,
    session: &RenderSession,
) -> Result<String> {
    const UNBALANCED_CSS_ERROR: &str = "{ /* ERROR: Unbalanced CSS */ }";

    let Some(theme_css) = effective_config
        .get("themeCSS")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|css| !css.is_empty() && *css != UNBALANCED_CSS_ERROR)
    else {
        return Ok(svg);
    };

    let metadata = SvgPostprocessMetadata::from_svg(&svg);
    let pipeline = SvgPipeline::parity()
        .with_postprocessor(ScopedCssPostprocessor::new(theme_css).with_existing_style_merge());
    pipeline.process_to_string_with_metadata(&svg, &metadata, session)
}

fn curve_basis_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve::curve_basis_path_d(points)
}

fn compute_layout_bounds(
    clusters: &[LayoutCluster],
    nodes: &[LayoutNode],
    edges: &[crate::model::LayoutEdge],
) -> Option<Bounds> {
    layout_debug::compute_layout_bounds(clusters, nodes, edges)
}

#[cfg(test)]
mod operation_time_tests {
    use super::*;

    #[test]
    fn svg_execution_uses_the_operation_session_time() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_runtime_policy(
                merman_core::runtime::RuntimePolicy::deterministic().with_fixed_unix_millis(1_000),
            )
            .begin_session()
            .expect("begin render session");
        let request = SvgRenderOptions::default();
        let debug = SvgDebugOptions::default();
        let execution = SvgExecution::new(&request, &debug, &session).expect("SVG execution");

        assert_eq!(execution.unix_ms(), session.unix_millis());
    }

    #[test]
    fn svg_execution_preserves_truthy_javascript_number_seeds() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin render session");
        let request = SvgRenderOptions::default();
        let debug = SvgDebugOptions::default();
        let execution = SvgExecution::new(&request, &debug, &session).expect("SVG execution");

        for seed in [
            serde_json::json!(-1),
            serde_json::json!(1.75),
            serde_json::json!(4_294_967_297_u64),
        ] {
            let seed = seed.as_f64().expect("numeric seed");
            assert_eq!(
                execution
                    .rough_randomness(seed, "render.test.roughjs")
                    .seed()
                    .number(),
                seed
            );
        }
    }

    #[test]
    fn svg_execution_resolves_falsy_seed_and_shared_math_random_stream() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin render session");
        let request = SvgRenderOptions::default();
        let debug = SvgDebugOptions::default();
        let execution = SvgExecution::new(&request, &debug, &session).expect("SVG execution");

        for seed in [0.0, -0.0] {
            assert_eq!(
                execution
                    .rough_randomness(seed, "render.test.roughjs")
                    .seed()
                    .number(),
                execution.seed() as f64
            );
        }

        let randomness = execution.rough_randomness(0.0, "render.test.roughjs");
        assert_eq!(
            randomness.math_random().initial_seed(),
            session
                .operation_context()
                .derive_u64("render.test.roughjs", 0)
        );
        assert_ne!(
            randomness.math_random().initial_seed(),
            execution
                .rough_randomness(0.0, "render.other.roughjs")
                .math_random()
                .initial_seed()
        );
    }
}
