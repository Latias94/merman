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
mod flowchart;
mod gantt;
mod gitgraph;
mod info;
mod journey;
mod kanban;
mod layout_debug;
mod mindmap;
mod packet;
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
pub use state::{SvgEmittedBoundsContributor, SvgEmittedBoundsDebug, debug_svg_emitted_bounds};
use state::{
    roughjs_ops_to_svg_path_d, roughjs_parse_hex_color_to_srgba, roughjs_paths_for_rect,
    svg_emitted_bounds_from_svg, svg_emitted_bounds_from_svg_inner,
};
use style::{is_rect_style_key, is_text_style_key, parse_style_decl};
use util::{
    config_f64, config_string, escape_attr, escape_xml, fmt, fmt_debug_3dp, fmt_max_width_px,
    fmt_path, json_f64, json_stringify_points, normalize_css_font_family, theme_color,
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
    label: String,
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

#[derive(Debug, Clone, Copy)]
struct SvgPathBounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl SvgPathBounds {
    fn include_point(&mut self, x: f64, y: f64) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
    }
}

fn svg_path_bounds_from_d(d: &str) -> Option<SvgPathBounds> {
    use std::f64::consts::PI;

    fn skip_sep(bytes: &[u8], i: &mut usize) {
        while *i < bytes.len() {
            match bytes[*i] {
                b' ' | b'\n' | b'\r' | b'\t' | b',' => *i += 1,
                _ => break,
            }
        }
    }

    fn parse_number(d: &str, bytes: &[u8], i: &mut usize) -> Option<f64> {
        skip_sep(bytes, i);
        if *i >= bytes.len() {
            return None;
        }
        let start = *i;
        if matches!(bytes[*i], b'+' | b'-') {
            *i += 1;
        }
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
        if *i < bytes.len() && bytes[*i] == b'.' {
            *i += 1;
            while *i < bytes.len() && bytes[*i].is_ascii_digit() {
                *i += 1;
            }
        }
        if *i < bytes.len() && matches!(bytes[*i], b'e' | b'E') {
            *i += 1;
            if *i < bytes.len() && matches!(bytes[*i], b'+' | b'-') {
                *i += 1;
            }
            while *i < bytes.len() && bytes[*i].is_ascii_digit() {
                *i += 1;
            }
        }
        d[start..*i].parse::<f64>().ok()
    }

    fn try_parse_number(d: &str, bytes: &[u8], i: &mut usize) -> Option<f64> {
        let save = *i;
        let v = parse_number(d, bytes, i);
        if v.is_none() {
            *i = save;
        }
        v
    }

    fn parse_pair(d: &str, bytes: &[u8], i: &mut usize) -> Option<(f64, f64)> {
        let x = parse_number(d, bytes, i)?;
        let y = parse_number(d, bytes, i)?;
        Some((x, y))
    }

    fn try_parse_pair(d: &str, bytes: &[u8], i: &mut usize) -> Option<(f64, f64)> {
        let save = *i;
        let v = parse_pair(d, bytes, i);
        if v.is_none() {
            *i = save;
        }
        v
    }

    fn cubic_eval(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
        let a = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
        let b = 3.0 * p0 - 6.0 * p1 + 3.0 * p2;
        let c = -3.0 * p0 + 3.0 * p1;
        ((a * t + b) * t + c) * t + p0
    }

    fn cubic_include_bounds(
        b: &mut SvgPathBounds,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        x3: f64,
        y3: f64,
    ) {
        b.include_point(x0, y0);
        b.include_point(x3, y3);

        fn include_extrema(
            b: &mut SvgPathBounds,
            p0: f64,
            p1: f64,
            p2: f64,
            p3: f64,
            is_x: bool,
            fixed0: f64,
            fixed1: f64,
            fixed2: f64,
            fixed3: f64,
        ) {
            let a = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
            let bb = 3.0 * p0 - 6.0 * p1 + 3.0 * p2;
            let c = -3.0 * p0 + 3.0 * p1;
            let qa = 3.0 * a;
            let qb = 2.0 * bb;
            let qc = c;

            const EPS: f64 = 1e-12;
            let mut roots: [f64; 2] = [f64::NAN, f64::NAN];
            let mut root_count = 0usize;
            if qa.abs() <= EPS {
                if qb.abs() > EPS {
                    let t = -qc / qb;
                    roots[0] = t;
                    root_count = 1;
                }
            } else {
                let disc = qb * qb - 4.0 * qa * qc;
                let tol = 1e-12 * (qb * qb + (4.0 * qa * qc).abs() + 1.0);
                if disc >= -tol {
                    let s = disc.max(0.0).sqrt();
                    roots[0] = (-qb + s) / (2.0 * qa);
                    roots[1] = (-qb - s) / (2.0 * qa);
                    root_count = 2;
                }
            }

            for idx in 0..root_count {
                let t = roots[idx];
                if !(t > 0.0 && t < 1.0) {
                    continue;
                }
                let v = cubic_eval(p0, p1, p2, p3, t);
                let other = cubic_eval(fixed0, fixed1, fixed2, fixed3, t);
                if is_x {
                    b.include_point(v, other);
                } else {
                    b.include_point(other, v);
                }
            }
        }

        include_extrema(b, x0, x1, x2, x3, true, y0, y1, y2, y3);
        include_extrema(b, y0, y1, y2, y3, false, x0, x1, x2, x3);
    }

    fn quadratic_include_bounds(
        b: &mut SvgPathBounds,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    ) {
        // Convert quadratic to cubic for extrema math:
        // https://pomax.github.io/bezierinfo/#circles_cubic
        let cx1 = x0 + (2.0 / 3.0) * (x1 - x0);
        let cy1 = y0 + (2.0 / 3.0) * (y1 - y0);
        let cx2 = x2 + (2.0 / 3.0) * (x1 - x2);
        let cy2 = y2 + (2.0 / 3.0) * (y1 - y2);
        cubic_include_bounds(b, x0, y0, cx1, cy1, cx2, cy2, x2, y2);
    }

    fn normalize_angle(mut a: f64) -> f64 {
        let two_pi = 2.0 * PI;
        a %= two_pi;
        if a < 0.0 {
            a += two_pi;
        }
        a
    }

    fn angle_between(theta: f64, start: f64, delta: f64) -> bool {
        let two_pi = 2.0 * PI;
        let eps = 1e-9;
        let t = normalize_angle(theta - start);
        if delta >= 0.0 {
            t <= delta + eps
        } else {
            t >= two_pi + delta - eps
        }
    }

    fn vec_angle(ux: f64, uy: f64, vx: f64, vy: f64) -> f64 {
        let dot = ux * vx + uy * vy;
        let det = ux * vy - uy * vx;
        det.atan2(dot)
    }

    #[allow(clippy::too_many_arguments)]
    fn arc_include_bounds(
        b: &mut SvgPathBounds,
        x0: f64,
        y0: f64,
        rx0: f64,
        ry0: f64,
        x_axis_rotation_deg: f64,
        large_arc: bool,
        sweep: bool,
        x1: f64,
        y1: f64,
    ) {
        // Per SVG 1.1 endpoint-to-center arc conversion.
        // See: https://www.w3.org/TR/SVG/implnote.html#ArcImplementationNotes
        if rx0.abs() < 1e-12 || ry0.abs() < 1e-12 {
            b.include_point(x0, y0);
            b.include_point(x1, y1);
            return;
        }

        let phi = x_axis_rotation_deg.to_radians();
        let (cos_phi, sin_phi) = (phi.cos(), phi.sin());
        let mut rx = rx0.abs();
        let mut ry = ry0.abs();

        let dx2 = (x0 - x1) / 2.0;
        let dy2 = (y0 - y1) / 2.0;

        let x1p = cos_phi * dx2 + sin_phi * dy2;
        let y1p = -sin_phi * dx2 + cos_phi * dy2;

        let x1p2 = x1p * x1p;
        let y1p2 = y1p * y1p;

        // Ensure radii are large enough.
        let lam = x1p2 / (rx * rx) + y1p2 / (ry * ry);
        if lam > 1.0 {
            let s = lam.sqrt();
            rx *= s;
            ry *= s;
        }

        let rx2 = rx * rx;
        let ry2 = ry * ry;

        let num = (rx2 * ry2) - (rx2 * y1p2) - (ry2 * x1p2);
        let den = (rx2 * y1p2) + (ry2 * x1p2);
        if den.abs() < 1e-24 {
            b.include_point(x0, y0);
            b.include_point(x1, y1);
            return;
        }
        let mut sq = num / den;
        if sq < 0.0 {
            sq = 0.0;
        }
        let sign = if large_arc == sweep { -1.0 } else { 1.0 };
        let coef = sign * sq.sqrt();

        let cxp = coef * (rx * y1p) / ry;
        let cyp = coef * (-ry * x1p) / rx;

        let cx = cos_phi * cxp - sin_phi * cyp + (x0 + x1) / 2.0;
        let cy = sin_phi * cxp + cos_phi * cyp + (y0 + y1) / 2.0;

        let ux = (x1p - cxp) / rx;
        let uy = (y1p - cyp) / ry;
        let vx = (-x1p - cxp) / rx;
        let vy = (-y1p - cyp) / ry;

        let theta1 = vec_angle(1.0, 0.0, ux, uy);
        let mut delta = vec_angle(ux, uy, vx, vy);

        if !sweep && delta > 0.0 {
            delta -= 2.0 * PI;
        } else if sweep && delta < 0.0 {
            delta += 2.0 * PI;
        }

        let start = theta1;
        let end = theta1 + delta;

        let arc_point = |theta: f64| -> (f64, f64) {
            let (ct, st) = (theta.cos(), theta.sin());
            let x = cx + rx * ct * cos_phi - ry * st * sin_phi;
            let y = cy + rx * ct * sin_phi + ry * st * cos_phi;
            (x, y)
        };

        b.include_point(x0, y0);
        b.include_point(x1, y1);
        let (sx, sy) = arc_point(start);
        let (ex, ey) = arc_point(end);
        b.include_point(sx, sy);
        b.include_point(ex, ey);

        // Extrema angles for rotated ellipse. See derivative analysis in many references.
        // x extrema: tan(theta) = -(ry*sin(phi)) / (rx*cos(phi))
        // y extrema: tan(theta) =  (ry*cos(phi)) / (rx*sin(phi))
        let tx_base = (-ry * sin_phi).atan2(rx * cos_phi);
        for k in 0..2 {
            let t = tx_base + (k as f64) * PI;
            if angle_between(t, start, delta) {
                let (x, y) = arc_point(t);
                b.include_point(x, y);
            }
        }

        let ty_base = (ry * cos_phi).atan2(rx * sin_phi);
        for k in 0..2 {
            let t = ty_base + (k as f64) * PI;
            if angle_between(t, start, delta) {
                let (x, y) = arc_point(t);
                b.include_point(x, y);
            }
        }
    }

    let bytes = d.as_bytes();
    let mut i = 0usize;
    let mut cmd: u8 = 0;
    let mut cx = 0.0f64;
    let mut cy = 0.0f64;
    let (mut sx, mut sy) = (0.0f64, 0.0f64);
    let mut last_cubic_ctrl: Option<(f64, f64)> = None;
    let mut last_quad_ctrl: Option<(f64, f64)> = None;
    let mut prev_cmd: u8 = 0;
    let mut b: Option<SvgPathBounds> = None;

    while i < bytes.len() {
        skip_sep(bytes, &mut i);
        if i >= bytes.len() {
            break;
        }
        let ch = bytes[i];
        if ch.is_ascii_alphabetic() {
            cmd = ch;
            i += 1;
        } else if cmd == 0 {
            return None;
        }

        let is_rel = cmd.is_ascii_lowercase();
        let ucmd = cmd.to_ascii_uppercase();

        fn ensure_bounds(b: &mut Option<SvgPathBounds>, x: f64, y: f64) -> &mut SvgPathBounds {
            if b.is_none() {
                *b = Some(SvgPathBounds {
                    min_x: x,
                    min_y: y,
                    max_x: x,
                    max_y: y,
                });
            }
            b.as_mut().expect("just set")
        }

        match ucmd {
            b'M' => {
                // First pair is move-to; subsequent pairs are implicit line-to.
                let (mut x, mut y) = parse_pair(d, bytes, &mut i)?;
                if is_rel {
                    x += cx;
                    y += cy;
                }
                cx = x;
                cy = y;
                sx = x;
                sy = y;
                ensure_bounds(&mut b, cx, cy).include_point(cx, cy);
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
                prev_cmd = ucmd;

                loop {
                    let Some((mut nx, mut ny)) = try_parse_pair(d, bytes, &mut i) else {
                        break;
                    };
                    if is_rel {
                        nx += cx;
                        ny += cy;
                    }
                    cx = nx;
                    cy = ny;
                    ensure_bounds(&mut b, cx, cy).include_point(cx, cy);
                    prev_cmd = b'L';
                }
            }
            b'Z' => {
                // Close path: line to subpath start.
                let cur = ensure_bounds(&mut b, cx, cy);
                cur.include_point(cx, cy);
                cur.include_point(sx, sy);
                cx = sx;
                cy = sy;
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
                prev_cmd = ucmd;
            }
            b'L' => {
                let (mut x, mut y) = parse_pair(d, bytes, &mut i)?;
                if is_rel {
                    x += cx;
                    y += cy;
                }
                cx = x;
                cy = y;
                ensure_bounds(&mut b, cx, cy).include_point(cx, cy);
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
                prev_cmd = ucmd;

                loop {
                    let Some((mut nx, mut ny)) = try_parse_pair(d, bytes, &mut i) else {
                        break;
                    };
                    if is_rel {
                        nx += cx;
                        ny += cy;
                    }
                    cx = nx;
                    cy = ny;
                    ensure_bounds(&mut b, cx, cy).include_point(cx, cy);
                    prev_cmd = ucmd;
                }
            }
            b'H' => {
                let mut x = parse_number(d, bytes, &mut i)?;
                if is_rel {
                    x += cx;
                }
                cx = x;
                ensure_bounds(&mut b, cx, cy).include_point(cx, cy);
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
                prev_cmd = ucmd;

                loop {
                    let Some(mut nx) = try_parse_number(d, bytes, &mut i) else {
                        break;
                    };
                    if is_rel {
                        nx += cx;
                    }
                    cx = nx;
                    ensure_bounds(&mut b, cx, cy).include_point(cx, cy);
                    prev_cmd = ucmd;
                }
            }
            b'V' => {
                let mut y = parse_number(d, bytes, &mut i)?;
                if is_rel {
                    y += cy;
                }
                cy = y;
                ensure_bounds(&mut b, cx, cy).include_point(cx, cy);
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
                prev_cmd = ucmd;

                loop {
                    let Some(mut ny) = try_parse_number(d, bytes, &mut i) else {
                        break;
                    };
                    if is_rel {
                        ny += cy;
                    }
                    cy = ny;
                    ensure_bounds(&mut b, cx, cy).include_point(cx, cy);
                    prev_cmd = ucmd;
                }
            }
            b'C' => {
                let mut x1 = parse_number(d, bytes, &mut i)?;
                let mut y1 = parse_number(d, bytes, &mut i)?;
                let mut x2 = parse_number(d, bytes, &mut i)?;
                let mut y2 = parse_number(d, bytes, &mut i)?;
                let mut x3 = parse_number(d, bytes, &mut i)?;
                let mut y3 = parse_number(d, bytes, &mut i)?;
                if is_rel {
                    x1 += cx;
                    y1 += cy;
                    x2 += cx;
                    y2 += cy;
                    x3 += cx;
                    y3 += cy;
                }
                let cur = ensure_bounds(&mut b, cx, cy);
                cubic_include_bounds(cur, cx, cy, x1, y1, x2, y2, x3, y3);
                cx = x3;
                cy = y3;
                last_cubic_ctrl = Some((x2, y2));
                last_quad_ctrl = None;
                prev_cmd = ucmd;

                loop {
                    let save = i;
                    let Some(mut nx1) = try_parse_number(d, bytes, &mut i) else {
                        break;
                    };
                    let Some(mut ny1) = try_parse_number(d, bytes, &mut i) else {
                        i = save;
                        break;
                    };
                    let Some(mut nx2) = try_parse_number(d, bytes, &mut i) else {
                        i = save;
                        break;
                    };
                    let Some(mut ny2) = try_parse_number(d, bytes, &mut i) else {
                        i = save;
                        break;
                    };
                    let Some(mut nx3) = try_parse_number(d, bytes, &mut i) else {
                        i = save;
                        break;
                    };
                    let Some(mut ny3) = try_parse_number(d, bytes, &mut i) else {
                        i = save;
                        break;
                    };
                    if is_rel {
                        nx1 += cx;
                        ny1 += cy;
                        nx2 += cx;
                        ny2 += cy;
                        nx3 += cx;
                        ny3 += cy;
                    }
                    let cur = ensure_bounds(&mut b, cx, cy);
                    cubic_include_bounds(cur, cx, cy, nx1, ny1, nx2, ny2, nx3, ny3);
                    cx = nx3;
                    cy = ny3;
                    last_cubic_ctrl = Some((nx2, ny2));
                    last_quad_ctrl = None;
                    prev_cmd = ucmd;
                }
            }
            b'S' => {
                let (x1, y1) = if matches!(prev_cmd, b'C' | b'S') {
                    if let Some((px, py)) = last_cubic_ctrl {
                        (2.0 * cx - px, 2.0 * cy - py)
                    } else {
                        (cx, cy)
                    }
                } else {
                    (cx, cy)
                };

                let mut x2 = parse_number(d, bytes, &mut i)?;
                let mut y2 = parse_number(d, bytes, &mut i)?;
                let mut x3 = parse_number(d, bytes, &mut i)?;
                let mut y3 = parse_number(d, bytes, &mut i)?;
                if is_rel {
                    x2 += cx;
                    y2 += cy;
                    x3 += cx;
                    y3 += cy;
                }
                let cur = ensure_bounds(&mut b, cx, cy);
                cubic_include_bounds(cur, cx, cy, x1, y1, x2, y2, x3, y3);
                cx = x3;
                cy = y3;
                last_cubic_ctrl = Some((x2, y2));
                last_quad_ctrl = None;
                prev_cmd = ucmd;
            }
            b'Q' => {
                let mut x1 = parse_number(d, bytes, &mut i)?;
                let mut y1 = parse_number(d, bytes, &mut i)?;
                let mut x2 = parse_number(d, bytes, &mut i)?;
                let mut y2 = parse_number(d, bytes, &mut i)?;
                if is_rel {
                    x1 += cx;
                    y1 += cy;
                    x2 += cx;
                    y2 += cy;
                }
                let cur = ensure_bounds(&mut b, cx, cy);
                quadratic_include_bounds(cur, cx, cy, x1, y1, x2, y2);
                cx = x2;
                cy = y2;
                last_quad_ctrl = Some((x1, y1));
                last_cubic_ctrl = None;
                prev_cmd = ucmd;
            }
            b'T' => {
                let (x1, y1) = if matches!(prev_cmd, b'Q' | b'T') {
                    if let Some((qx, qy)) = last_quad_ctrl {
                        (2.0 * cx - qx, 2.0 * cy - qy)
                    } else {
                        (cx, cy)
                    }
                } else {
                    (cx, cy)
                };
                let (mut x2, mut y2) = parse_pair(d, bytes, &mut i)?;
                if is_rel {
                    x2 += cx;
                    y2 += cy;
                }
                let cur = ensure_bounds(&mut b, cx, cy);
                quadratic_include_bounds(cur, cx, cy, x1, y1, x2, y2);
                cx = x2;
                cy = y2;
                last_quad_ctrl = Some((x1, y1));
                last_cubic_ctrl = None;
                prev_cmd = ucmd;
            }
            b'A' => {
                let rx = parse_number(d, bytes, &mut i)?;
                let ry = parse_number(d, bytes, &mut i)?;
                let rot = parse_number(d, bytes, &mut i)?;
                let laf = parse_number(d, bytes, &mut i)?;
                let sf = parse_number(d, bytes, &mut i)?;
                let (mut x1, mut y1) = parse_pair(d, bytes, &mut i)?;
                if is_rel {
                    x1 += cx;
                    y1 += cy;
                }
                let large_arc = laf.abs() > 0.5;
                let sweep = sf.abs() > 0.5;
                if let Some(cur) = b.as_mut() {
                    arc_include_bounds(cur, cx, cy, rx, ry, rot, large_arc, sweep, x1, y1);
                } else {
                    let mut cur = SvgPathBounds {
                        min_x: cx,
                        min_y: cy,
                        max_x: cx,
                        max_y: cy,
                    };
                    arc_include_bounds(&mut cur, cx, cy, rx, ry, rot, large_arc, sweep, x1, y1);
                    b = Some(cur);
                }
                cx = x1;
                cy = y1;
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
                prev_cmd = ucmd;
            }
            _ => return None,
        }
    }

    b
}
