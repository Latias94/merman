use crate::model::{
    ArchitectureDiagramLayout, BlockDiagramLayout, Bounds, ClassDiagramV2Layout, ErDiagramLayout,
    ErrorDiagramLayout, FlowchartV2Layout, InfoDiagramLayout, LayoutCluster, LayoutNode,
    MindmapDiagramLayout, PacketDiagramLayout, PieDiagramLayout, QuadrantChartDiagramLayout,
    RadarDiagramLayout, RequirementDiagramLayout, SankeyDiagramLayout, SequenceDiagramLayout,
    StateDiagramV2Layout, TimelineDiagramLayout, XyChartDiagramLayout,
};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use base64::Engine as _;
use indexmap::IndexMap;
use serde::Deserialize;
use std::fmt::Write as _;

mod architecture;
mod block;
mod c4;
mod class;
mod css;
mod curve;
mod er;
mod error;
mod fallback;
mod flowchart;
mod gantt;
mod gitgraph;
mod info;
mod journey;
mod kanban;
mod layout_debug;
mod mindmap;
mod packet;
mod path_bounds;
mod pie;
mod quadrantchart;
mod radar;
mod requirement;
mod sankey;
mod sequence;
mod state;
mod style;
mod timeline;
mod treemap;
mod util;
mod xychart;
use css::{
    er_css, gantt_css, info_css, pie_css, requirement_css, sankey_css, treemap_css, xychart_css,
};
pub use fallback::foreign_object_label_fallback_svg_text;
use path_bounds::svg_path_bounds_from_d;
pub use state::{SvgEmittedBoundsContributor, SvgEmittedBoundsDebug, debug_svg_emitted_bounds};
use state::{
    roughjs_ops_to_svg_path_d, roughjs_parse_hex_color_to_srgba, roughjs_paths_for_rect,
    svg_emitted_bounds_from_svg, svg_emitted_bounds_from_svg_inner,
};
use style::{is_rect_style_key, is_text_style_key, parse_style_decl};
use util::{
    config_f64, config_string, escape_attr, escape_xml, escape_xml_display, escape_xml_into, fmt,
    fmt_debug_3dp, fmt_max_width_px, fmt_path, json_f64, json_stringify_points,
    normalize_css_font_family, theme_color,
};

const MERMAID_SEQUENCE_BASE_DEFS_11_12_2: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/sequence_base_defs_11_12_2.svgfrag"
));

#[derive(Debug, Clone)]
pub struct SvgRenderOptions {
    /// Adds extra space around the computed viewBox.
    pub viewbox_padding: f64,
    /// Optional diagram id used for Mermaid-like marker ids.
    pub diagram_id: Option<String>,
    /// Optional override for the root SVG `aria-roledescription` attribute.
    ///
    /// This is primarily used to reproduce Mermaid's per-header accessibility metadata quirks
    /// (e.g. `classDiagram-v2` differs from `classDiagram` at Mermaid 11.12.2).
    pub aria_roledescription: Option<String>,
    /// When true, include edge polylines.
    pub include_edges: bool,
    /// When true, include node bounding boxes and ids.
    pub include_nodes: bool,
    /// When true, include cluster bounding boxes and titles.
    pub include_clusters: bool,
    /// When true, draw markers that visualize Mermaid cluster positioning metadata.
    pub include_cluster_debug_markers: bool,
    /// When true, label edge routes with edge ids.
    pub include_edge_id_labels: bool,
    /// Optional override for "current time" used by diagrams that render time-dependent markers
    /// (e.g. Gantt `today` line). This exists to make parity/golden comparisons reproducible.
    pub now_ms_override: Option<i64>,
}

impl Default for SvgRenderOptions {
    fn default() -> Self {
        Self {
            viewbox_padding: 8.0,
            diagram_id: None,
            aria_roledescription: None,
            include_edges: true,
            include_nodes: true,
            include_clusters: true,
            include_cluster_debug_markers: false,
            include_edge_id_labels: false,
            now_ms_override: None,
        }
    }
}

pub fn render_layouted_svg(
    diagram: &crate::model::LayoutedDiagram,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    use crate::model::LayoutDiagram;

    match &diagram.layout {
        LayoutDiagram::ErrorDiagram(layout) => render_error_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            options,
        ),
        LayoutDiagram::BlockDiagram(layout) => render_block_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            options,
        ),
        LayoutDiagram::RequirementDiagram(layout) => render_requirement_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            options,
        ),
        LayoutDiagram::ArchitectureDiagram(layout) => render_architecture_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            options,
        ),
        LayoutDiagram::MindmapDiagram(layout) => render_mindmap_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            options,
        ),
        LayoutDiagram::SankeyDiagram(layout) => render_sankey_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            options,
        ),
        LayoutDiagram::RadarDiagram(layout) => render_radar_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            options,
        ),
        LayoutDiagram::TreemapDiagram(layout) => render_treemap_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            options,
        ),
        LayoutDiagram::XyChartDiagram(layout) => render_xychart_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            options,
        ),
        LayoutDiagram::QuadrantChartDiagram(layout) => render_quadrantchart_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            options,
        ),
        LayoutDiagram::FlowchartV2(layout) => render_flowchart_v2_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            diagram.meta.title.as_deref(),
            measurer,
            options,
        ),
        LayoutDiagram::StateDiagramV2(layout) => render_state_diagram_v2_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            diagram.meta.title.as_deref(),
            measurer,
            options,
        ),
        LayoutDiagram::ClassDiagramV2(layout) => render_class_diagram_v2_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            diagram.meta.title.as_deref(),
            measurer,
            options,
        ),
        LayoutDiagram::ErDiagram(layout) => render_er_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            diagram.meta.title.as_deref(),
            measurer,
            options,
        ),
        LayoutDiagram::SequenceDiagram(layout) => render_sequence_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            diagram.meta.title.as_deref(),
            measurer,
            options,
        ),
        LayoutDiagram::InfoDiagram(layout) => render_info_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            options,
        ),
        LayoutDiagram::PacketDiagram(layout) => render_packet_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            diagram.meta.title.as_deref(),
            options,
        ),
        LayoutDiagram::TimelineDiagram(layout) => render_timeline_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            diagram.meta.title.as_deref(),
            measurer,
            options,
        ),
        LayoutDiagram::PieDiagram(layout) => render_pie_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            options,
        ),
        LayoutDiagram::JourneyDiagram(layout) => render_journey_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            diagram.meta.title.as_deref(),
            measurer,
            options,
        ),
        LayoutDiagram::KanbanDiagram(layout) => render_kanban_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            options,
        ),
        LayoutDiagram::GitGraphDiagram(layout) => render_gitgraph_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            measurer,
            options,
        ),
        LayoutDiagram::GanttDiagram(layout) => render_gantt_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            options,
        ),
        LayoutDiagram::C4Diagram(layout) => render_c4_diagram_svg(
            layout,
            &diagram.semantic,
            &diagram.meta.effective_config,
            diagram.meta.title.as_deref(),
            measurer,
            options,
        ),
    }
}

pub fn render_flowchart_v2_debug_svg(
    layout: &FlowchartV2Layout,
    options: &SvgRenderOptions,
) -> String {
    flowchart::render_flowchart_v2_debug_svg(layout, options)
}

#[derive(Debug, Clone, Deserialize)]
struct PieSvgSection {
    #[allow(dead_code)]
    label: String,
    #[allow(dead_code)]
    value: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct PieSvgModel {
    #[serde(rename = "accTitle")]
    acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    acc_descr: Option<String>,
    #[serde(rename = "showData")]
    show_data: bool,
    title: Option<String>,
    #[allow(dead_code)]
    sections: Vec<PieSvgSection>,
}

pub fn render_sequence_diagram_debug_svg(
    layout: &SequenceDiagramLayout,
    options: &SvgRenderOptions,
) -> String {
    sequence::render_sequence_diagram_debug_svg(layout, options)
}

pub fn render_sequence_diagram_svg(
    layout: &SequenceDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    sequence::render_sequence_diagram_svg(
        layout,
        semantic,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

pub fn render_error_diagram_svg(
    layout: &ErrorDiagramLayout,
    _semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    error::render_error_diagram_svg(layout, _semantic, _effective_config, options)
}

pub fn render_info_diagram_svg(
    layout: &InfoDiagramLayout,
    _semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    info::render_info_diagram_svg(layout, _semantic, _effective_config, options)
}

pub fn render_pie_diagram_svg(
    layout: &PieDiagramLayout,
    semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    pie::render_pie_diagram_svg(layout, semantic, _effective_config, options)
}

pub fn render_requirement_diagram_svg(
    layout: &RequirementDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    requirement::render_requirement_diagram_svg(layout, semantic, effective_config, options)
}

pub fn render_block_diagram_svg(
    layout: &BlockDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    block::render_block_diagram_svg(layout, semantic, effective_config, options)
}

pub fn render_radar_diagram_svg(
    layout: &RadarDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    radar::render_radar_diagram_svg(layout, semantic, effective_config, options)
}

pub fn render_quadrantchart_diagram_svg(
    layout: &QuadrantChartDiagramLayout,
    _semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    quadrantchart::render_quadrantchart_diagram_svg(layout, _semantic, _effective_config, options)
}

pub fn render_xychart_diagram_svg(
    layout: &XyChartDiagramLayout,
    _semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    xychart::render_xychart_diagram_svg(layout, _semantic, _effective_config, options)
}

pub fn render_treemap_diagram_svg(
    layout: &crate::model::TreemapDiagramLayout,
    _semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    treemap::render_treemap_diagram_svg(layout, _semantic, effective_config, options)
}

pub fn render_packet_diagram_svg(
    layout: &PacketDiagramLayout,
    semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    options: &SvgRenderOptions,
) -> Result<String> {
    packet::render_packet_diagram_svg(layout, semantic, _effective_config, diagram_title, options)
}

pub fn render_timeline_diagram_svg(
    layout: &TimelineDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    _diagram_title: Option<&str>,
    _measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    timeline::render_timeline_diagram_svg(
        layout,
        semantic,
        effective_config,
        _diagram_title,
        _measurer,
        options,
    )
}

pub fn render_journey_diagram_svg(
    layout: &crate::model::JourneyDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    _diagram_title: Option<&str>,
    _measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    journey::render_journey_diagram_svg(
        layout,
        semantic,
        effective_config,
        _diagram_title,
        _measurer,
        options,
    )
}

pub fn render_kanban_diagram_svg(
    layout: &crate::model::KanbanDiagramLayout,
    _semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    kanban::render_kanban_diagram_svg(layout, _semantic, _effective_config, options)
}

pub fn render_gitgraph_diagram_svg(
    layout: &crate::model::GitGraphDiagramLayout,
    semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    gitgraph::render_gitgraph_diagram_svg(layout, semantic, _effective_config, measurer, options)
}

pub fn render_gantt_diagram_svg(
    layout: &crate::model::GanttDiagramLayout,
    semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    gantt::render_gantt_diagram_svg(layout, semantic, _effective_config, options)
}

#[derive(Debug, Clone, Deserialize)]
struct C4SvgModelText {
    #[allow(dead_code)]
    text: String,
}

#[derive(Debug, Clone, Deserialize)]
struct C4SvgModelShape {
    alias: String,
    #[serde(default, rename = "bgColor")]
    bg_color: Option<String>,
    #[serde(default, rename = "borderColor")]
    border_color: Option<String>,
    #[serde(default, rename = "fontColor")]
    font_color: Option<String>,
    #[serde(default)]
    sprite: Option<serde_json::Value>,
    #[serde(default, rename = "typeC4Shape")]
    #[allow(dead_code)]
    type_c4_shape: Option<C4SvgModelText>,
}

#[derive(Debug, Clone, Deserialize)]
struct C4SvgModelBoundary {
    alias: String,
    #[serde(default, rename = "nodeType")]
    node_type: Option<String>,
    #[serde(default, rename = "bgColor")]
    bg_color: Option<String>,
    #[serde(default, rename = "borderColor")]
    border_color: Option<String>,
    #[serde(default, rename = "fontColor")]
    #[allow(dead_code)]
    font_color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct C4SvgModelRel {
    #[serde(rename = "from")]
    from_alias: String,
    #[serde(rename = "to")]
    to_alias: String,
    #[serde(default, rename = "lineColor")]
    line_color: Option<String>,
    #[serde(default, rename = "textColor")]
    text_color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct C4SvgModel {
    #[serde(default, rename = "accTitle")]
    acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    acc_descr: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    shapes: Vec<C4SvgModelShape>,
    #[serde(default)]
    boundaries: Vec<C4SvgModelBoundary>,
    #[serde(default)]
    rels: Vec<C4SvgModelRel>,
}

pub fn render_mindmap_diagram_svg(
    layout: &MindmapDiagramLayout,
    semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    mindmap::render_mindmap_diagram_svg(layout, semantic, _effective_config, options)
}

pub fn render_architecture_diagram_svg(
    layout: &ArchitectureDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    architecture::render_architecture_diagram_svg(layout, semantic, effective_config, options)
}

pub fn render_c4_diagram_svg(
    layout: &crate::model::C4DiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    _measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    c4::render_c4_diagram_svg(
        layout,
        semantic,
        effective_config,
        diagram_title,
        _measurer,
        options,
    )
}

pub fn render_flowchart_v2_svg(
    layout: &FlowchartV2Layout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    flowchart::render_flowchart_v2_svg(
        layout,
        semantic,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

pub fn render_state_diagram_v2_svg(
    layout: &StateDiagramV2Layout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    state::render_state_diagram_v2_svg(
        layout,
        semantic,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

pub fn render_state_diagram_v2_debug_svg(
    layout: &StateDiagramV2Layout,
    options: &SvgRenderOptions,
) -> String {
    state::render_state_diagram_v2_debug_svg(layout, options)
}

pub fn render_class_diagram_v2_debug_svg(
    layout: &ClassDiagramV2Layout,
    options: &SvgRenderOptions,
) -> String {
    class::render_class_diagram_v2_debug_svg(layout, options)
}

pub fn render_class_diagram_v2_svg(
    layout: &ClassDiagramV2Layout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    class::render_class_diagram_v2_svg(
        layout,
        semantic,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

pub fn render_er_diagram_debug_svg(layout: &ErDiagramLayout, options: &SvgRenderOptions) -> String {
    er::render_er_diagram_debug_svg(layout, options)
}

pub fn render_er_diagram_svg(
    layout: &ErDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    er::render_er_diagram_svg(
        layout,
        semantic,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

pub fn render_sankey_diagram_svg(
    layout: &SankeyDiagramLayout,
    _semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    sankey::render_sankey_diagram_svg(layout, _semantic, effective_config, options)
}

fn curve_monotone_path_d(points: &[crate::model::LayoutPoint], swap_xy: bool) -> String {
    curve::curve_monotone_path_d(points, swap_xy)
}

fn curve_monotone_x_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve_monotone_path_d(points, false)
}

fn curve_monotone_y_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve_monotone_path_d(points, true)
}

// Ported from D3 `curveBasis` (d3-shape v3.x), used by Mermaid ER renderer `@11.12.2`.
fn curve_basis_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve::curve_basis_path_d(points)
}

fn curve_linear_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve::curve_linear_path_d(points)
}

// Ported from D3 `curveStepAfter` (d3-shape v3.x).
fn curve_step_after_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve::curve_step_after_path_d(points)
}

// Ported from D3 `curveStepBefore` (d3-shape v3.x).
fn curve_step_before_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve::curve_step_before_path_d(points)
}

// Ported from D3 `curveStep` (d3-shape v3.x).
fn curve_step_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve::curve_step_path_d(points)
}

// Ported from D3 `curveCardinal` (d3-shape v3.x).
fn curve_cardinal_path_d(points: &[crate::model::LayoutPoint], tension: f64) -> String {
    curve::curve_cardinal_path_d(points, tension)
}
fn render_node(out: &mut String, n: &LayoutNode) {
    layout_debug::render_node(out, n)
}

fn render_state_node(out: &mut String, n: &LayoutNode) {
    layout_debug::render_state_node(out, n)
}

fn render_cluster(out: &mut String, c: &LayoutCluster, include_markers: bool) {
    layout_debug::render_cluster(out, c, include_markers)
}

fn compute_layout_bounds(
    clusters: &[LayoutCluster],
    nodes: &[LayoutNode],
    edges: &[crate::model::LayoutEdge],
) -> Option<Bounds> {
    layout_debug::compute_layout_bounds(clusters, nodes, edges)
}
