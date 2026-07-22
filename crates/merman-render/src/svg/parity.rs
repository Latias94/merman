use super::pipeline::{ScopedCssPostprocessor, SvgPipeline, SvgPostprocessMetadata};
use crate::environment::{RenderSession, RoutedTextMeasurer, TextMeasurementPhase};
#[cfg(feature = "cytoscape-layout")]
use crate::model::{ArchitectureDiagramLayout, MindmapDiagramLayout};
use crate::model::{
    BlockDiagramLayout, Bounds, ClassDiagramLayout, CynefinDiagramLayout, ErDiagramLayout,
    ErrorDiagramLayout, EventModelingDiagramLayout, FlowchartLayout, InfoDiagramLayout,
    IshikawaDiagramLayout, LayoutCluster, LayoutNode, PacketDiagramLayout, PieDiagramLayout,
    QuadrantChartDiagramLayout, RadarDiagramLayout, RailroadDiagramLayout,
    RequirementDiagramLayout, SankeyDiagramLayout, SequenceDiagramLayout, StateDiagramLayout,
    TimelineDiagramLayout, TreeViewDiagramLayout, VennDiagramLayout, XyChartDiagramLayout,
};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use base64::Engine as _;
use indexmap::IndexMap;
use std::borrow::Cow;
use std::fmt::Write as _;

#[cfg(feature = "cytoscape-layout")]
mod architecture;
mod block;
mod c4;
mod class;
mod css;
mod curve;
mod cynefin;
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
mod layout_debug;
#[cfg(feature = "cytoscape-layout")]
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
#[cfg(feature = "cytoscape-layout")]
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

/// Diagnostic visibility controls kept separate from production render requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgDebugOptions {
    pub include_edges: bool,
    pub include_nodes: bool,
    pub include_clusters: bool,
    pub include_cluster_debug_markers: bool,
    pub include_edge_id_labels: bool,
    pub include_timing_diagnostics: bool,
    pub flowchart_trace_edge_id: Option<String>,
    pub flowchart_trace_output_path: Option<std::path::PathBuf>,
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
            flowchart_trace_edge_id: None,
            flowchart_trace_output_path: None,
        }
    }
}

pub(crate) struct SvgExecution<'a> {
    request: &'a SvgRenderOptions,
    session: &'a RenderSession,
    text_measurer: RoutedTextMeasurer<'a>,
    pub(crate) debug: &'a SvgDebugOptions,
}

impl<'a> SvgExecution<'a> {
    fn new(
        request: &'a SvgRenderOptions,
        debug: &'a SvgDebugOptions,
        session: &'a RenderSession,
    ) -> Self {
        Self {
            request,
            session,
            text_measurer: session.text_measurer(TextMeasurementPhase::SvgBBox),
            debug,
        }
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
        self.session.time().unix_ms()
    }

    pub(crate) const fn seed(&self) -> u64 {
        self.session.seed().seed().get()
    }

    pub(crate) const fn resource_policy(&self) -> crate::resources::RenderResourcePolicy {
        self.session.resource_policy()
    }

    fn effective_config_value<'b>(
        &self,
        effective_config: &'b serde_json::Value,
    ) -> Cow<'b, serde_json::Value> {
        let configured = effective_config
            .get("handDrawnSeed")
            .and_then(serde_json::Value::as_f64);
        let replacement = match configured {
            Some(seed) if seed != 0.0 => u64::from(javascript_to_uint32(seed)),
            _ => self.seed(),
        };
        if effective_config
            .get("handDrawnSeed")
            .and_then(serde_json::Value::as_u64)
            == Some(replacement)
        {
            return Cow::Borrowed(effective_config);
        }
        let Some(object) = effective_config.as_object() else {
            return Cow::Borrowed(effective_config);
        };
        let mut object = object.clone();
        object.insert(
            "handDrawnSeed".to_string(),
            serde_json::Value::Number(replacement.into()),
        );
        Cow::Owned(serde_json::Value::Object(object))
    }

    fn effective_config<'b>(
        &self,
        effective_config: &'b merman_core::MermaidConfig,
    ) -> Cow<'b, merman_core::MermaidConfig> {
        match self.effective_config_value(effective_config.as_value()) {
            Cow::Borrowed(_) => Cow::Borrowed(effective_config),
            Cow::Owned(value) => Cow::Owned(merman_core::MermaidConfig::from_value(value)),
        }
    }
}

fn javascript_to_uint32(value: f64) -> u32 {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    value.trunc().rem_euclid(4_294_967_296.0) as u32
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
    let session = crate::environment::RenderEnvironment::parity()
        .begin_session()
        .expect("create test render session");
    let debug = SvgDebugOptions::default();
    let execution = SvgExecution::new(request, &debug, &session);
    run(&execution)
}

pub(crate) fn render_builtin_family_artifact(
    family: &crate::family::BuiltinFamilyArtifact,
    effective_config: &merman_core::MermaidConfig,
    diagram_type: &str,
    title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    let execution = SvgExecution::new(options, debug, session);
    let rooted_svg = render_builtin_family_artifact_raw(
        family,
        effective_config,
        diagram_type,
        title,
        &execution,
    )?;
    let svg = rooted_svg.into_string_for(family.kind())?;
    apply_theme_css(svg, effective_config.as_value(), session)
}

fn render_builtin_family_artifact_raw(
    family: &crate::family::BuiltinFamilyArtifact,
    effective_config: &merman_core::MermaidConfig,
    diagram_type: &str,
    title: Option<&str>,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    use crate::family::BuiltinFamilyArtifact;

    let measurer = options.text_measurer();
    let effective_config = options.effective_config(effective_config);
    let effective_config = effective_config.as_ref();
    let effective_config_value = effective_config.as_value();

    match family {
        BuiltinFamilyArtifact::Error(pair) => error::render_error_diagram_svg_model(
            pair.layout(),
            pair.semantic(),
            effective_config_value,
            options,
        ),
        #[cfg(feature = "cytoscape-layout")]
        BuiltinFamilyArtifact::Architecture(pair) => {
            architecture::render_architecture_diagram_svg_typed_with_config(
                pair.layout(),
                pair.semantic(),
                effective_config,
                options,
            )
        }
        BuiltinFamilyArtifact::Flowchart(pair) => {
            flowchart::render_flowchart_svg_model_with_config(
                pair.layout(),
                pair.semantic(),
                effective_config,
                diagram_type,
                title,
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
        #[cfg(feature = "cytoscape-layout")]
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
            let text_measurer = options.text_measurer_for(TextMeasurementPhase::Wrap);
            kanban::render_kanban_diagram_svg(
                pair.layout(),
                effective_config_value,
                &text_measurer,
                options,
            )
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
                effective_config_value,
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
                effective_config_value,
                options,
            )
        }
        BuiltinFamilyArtifact::XyChart(pair) => {
            xychart::render_xychart_diagram_svg(pair.layout(), effective_config_value, options)
        }
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
        let snapshot = crate::environment::RenderTimeSnapshot::from_unix_millis(1_000, 0)
            .expect("valid operation time");
        let session = crate::environment::RenderEnvironment::parity()
            .with_time_snapshot(snapshot)
            .begin_session()
            .expect("begin render session");
        let request = SvgRenderOptions::default();
        let debug = SvgDebugOptions::default();
        let execution = SvgExecution::new(&request, &debug, &session);

        assert_eq!(execution.unix_ms(), session.time().unix_ms());
    }

    #[test]
    fn svg_execution_normalizes_nonzero_hand_drawn_seeds_like_javascript() {
        let session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .expect("begin render session");
        let request = SvgRenderOptions::default();
        let debug = SvgDebugOptions::default();
        let execution = SvgExecution::new(&request, &debug, &session);

        for (seed, expected) in [
            (serde_json::json!(-1), u64::from(u32::MAX)),
            (serde_json::json!(1.75), 1),
            (serde_json::json!(4_294_967_297_u64), 1),
        ] {
            let config = serde_json::json!({ "handDrawnSeed": seed });
            let effective = execution.effective_config_value(&config);
            assert_eq!(effective["handDrawnSeed"].as_u64(), Some(expected));
        }
    }
}
