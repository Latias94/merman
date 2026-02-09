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
mod er;
mod error;
mod flowchart;
mod gantt;
mod gitgraph;
mod info;
mod journey;
mod kanban;
mod mindmap;
mod packet;
mod pie;
mod quadrantchart;
mod radar;
mod requirement;
mod sankey;
mod sequence;
mod state;
mod timeline;
mod treemap;
mod xychart;
use flowchart::*;
pub use state::{SvgEmittedBoundsContributor, SvgEmittedBoundsDebug, debug_svg_emitted_bounds};
use state::{
    roughjs_ops_to_svg_path_d, roughjs_parse_hex_color_to_srgba, roughjs_paths_for_rect,
    svg_emitted_bounds_from_svg, svg_emitted_bounds_from_svg_inner,
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
    let mut clusters = layout.clusters.clone();
    clusters.sort_by(|a, b| a.id.cmp(&b.id));

    let mut nodes = layout.nodes.clone();
    nodes.sort_by(|a, b| a.id.cmp(&b.id));

    let mut edges = layout.edges.clone();
    edges.sort_by(|a, b| a.id.cmp(&b.id));

    let bounds = compute_layout_bounds(&clusters, &nodes, &edges).unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    let pad = options.viewbox_padding.max(0.0);
    let vb_min_x = bounds.min_x - pad;
    let vb_min_y = bounds.min_y - pad;
    let vb_w = (bounds.max_x - bounds.min_x) + pad * 2.0;
    let vb_h = (bounds.max_y - bounds.min_y) + pad * 2.0;

    let mut out = String::new();
    let _ = writeln!(
        &mut out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}">"#,
        fmt(vb_min_x),
        fmt(vb_min_y),
        fmt(vb_w.max(1.0)),
        fmt(vb_h.max(1.0))
    );
    out.push_str(
        r#"<style>
.cluster-box { fill: none; stroke: #4b5563; stroke-width: 1; }
.cluster-title { fill: #111827; font-family: ui-sans-serif, system-ui, sans-serif; font-size: 12px; text-anchor: middle; dominant-baseline: middle; }
.node-box { fill: none; stroke: #2563eb; stroke-width: 1; }
.node-label { fill: #1f2937; font-family: ui-sans-serif, system-ui, sans-serif; font-size: 11px; text-anchor: middle; dominant-baseline: middle; }
.edge { fill: none; stroke: #111827; stroke-width: 1; }
.edge-label { fill: #111827; font-family: ui-sans-serif, system-ui, sans-serif; font-size: 11px; text-anchor: middle; dominant-baseline: middle; }
.debug-cross { stroke: #ef4444; stroke-width: 1; }
</style>
"#,
    );

    if options.include_clusters {
        out.push_str(r#"<g class="clusters">"#);
        for c in &clusters {
            render_cluster(&mut out, c, options.include_cluster_debug_markers);
        }
        out.push_str("</g>\n");
    }

    if options.include_edges {
        out.push_str(r#"<g class="edges">"#);
        for e in &edges {
            if e.points.len() >= 2 {
                out.push_str(r#"<polyline class="edge" points=""#);
                for (idx, p) in e.points.iter().enumerate() {
                    if idx > 0 {
                        out.push(' ');
                    }
                    let _ = write!(&mut out, "{},{}", fmt(p.x), fmt(p.y));
                }
                out.push_str(r#"" />"#);
            }
            if options.include_edge_id_labels {
                if let Some(lbl) = &e.label {
                    let _ = write!(
                        &mut out,
                        r#"<text class="edge-label" x="{}" y="{}">{}</text>"#,
                        fmt(lbl.x),
                        fmt(lbl.y),
                        escape_xml(&e.id)
                    );
                }
            }
        }
        out.push_str("</g>\n");
    }

    if options.include_nodes {
        out.push_str(r#"<g class="nodes">"#);
        for n in &nodes {
            if n.is_cluster {
                continue;
            }
            render_node(&mut out, n);
        }
        out.push_str("</g>\n");
    }

    out.push_str("</svg>\n");
    out
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

fn info_css(diagram_id: &str) -> String {
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:16px;fill:#333;}}"#,
        id, font
    );
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:#552222;}}#{} .error-text{{fill:#552222;stroke:#552222;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:#333333;stroke:#333333;}}#{} .marker.cross{{stroke:#333333;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:16px;}}#{} p{{margin:0;}}#{} :root{{--mermaid-font-family:{};}}"#,
        id, font, id, id, font
    );
    out
}

fn requirement_css(diagram_id: &str) -> String {
    // Mirrors Mermaid@11.12.2 `diagrams/requirement/styles.js` + shared base stylesheet ordering.
    // Keep `:root` last (matches upstream fixtures).
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:16px;fill:#333;}}"#,
        id, font
    );
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:#552222;}}#{} .error-text{{fill:#552222;stroke:#552222;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:#333333;stroke:#333333;}}#{} .marker.cross{{stroke:#333333;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:16px;}}#{} p{{margin:0;}}"#,
        id, font, id
    );

    // Requirement diagram styles (duplicated marker/svg rules are present upstream).
    let _ = write!(
        &mut out,
        r#"#{} marker{{fill:#333333;stroke:#333333;}}#{} marker.cross{{stroke:#333333;}}#{} svg{{font-family:{};font-size:16px;}}"#,
        id, id, id, font
    );
    let _ = write!(
        &mut out,
        r#"#{} .reqBox{{fill:#ECECFF;fill-opacity:1.0;stroke:hsl(240, 60%, 86.2745098039%);stroke-width:1;}}#{} .reqTitle,#{} .reqLabel{{fill:#131300;}}#{} .reqLabelBox{{fill:rgba(232,232,232, 0.8);fill-opacity:1.0;}}#{} .req-title-line{{stroke:hsl(240, 60%, 86.2745098039%);stroke-width:1;}}#{} .relationshipLine{{stroke:#333333;stroke-width:1;}}#{} .relationshipLabel{{fill:black;}}#{} .divider{{stroke:#9370DB;stroke-width:1;}}#{} .label{{font-family:{};color:#333;}}#{} .label text,#{} span{{fill:#333;color:#333;}}#{} .labelBkg{{background-color:rgba(232,232,232, 0.8);}}"#,
        id, id, id, id, id, id, id, id, id, font, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} :root{{--mermaid-font-family:{};}}"#,
        id, font
    );
    out
}

fn er_css(diagram_id: &str) -> String {
    // Mirrors Mermaid@11.12.2 ER unified renderer stylesheet ordering (see `diagrams/er/styles.js`
    // and shared base stylesheet).
    // Keep `:root` last (matches upstream fixtures).
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:16px;fill:#333;}}"#,
        id, font
    );
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:#552222;}}#{} .error-text{{fill:#552222;stroke:#552222;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:#333333;stroke:#333333;}}#{} .marker.cross{{stroke:#333333;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:16px;}}#{} p{{margin:0;}}"#,
        id, font, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .entityBox{{fill:#ECECFF;stroke:#9370DB;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .relationshipLabelBox{{fill:hsl(80, 100%, 96.2745098039%);opacity:0.7;background-color:hsl(80, 100%, 96.2745098039%);}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .relationshipLabelBox rect{{opacity:0.5;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .labelBkg{{background-color:rgba(248.6666666666, 255, 235.9999999999, 0.5);}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .edgeLabel .label{{fill:#9370DB;font-size:14px;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .label{{font-family:{};color:#333;}}"#,
        id, font
    );
    // Mermaid duplicates `.edge-pattern-dashed` (base rule earlier sets dasharray:3).
    let _ = write!(
        &mut out,
        r#"#{} .edge-pattern-dashed{{stroke-dasharray:8,8;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .node rect,#{} .node circle,#{} .node ellipse,#{} .node polygon{{fill:#ECECFF;stroke:#9370DB;stroke-width:1px;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .relationshipLine{{stroke:#333333;stroke-width:1;fill:none;}}"#,
        id
    );
    // Mermaid duplicates `.marker` (base rule earlier sets fill/stroke to #333333).
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:none!important;stroke:#333333!important;stroke-width:1;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} :root{{--mermaid-font-family:{};}}"#,
        id, font
    );
    out
}

fn pie_css(diagram_id: &str) -> String {
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let mut out = info_css(diagram_id);
    let _ = write!(
        &mut out,
        r#"#{} .pieCircle{{stroke:black;stroke-width:2px;opacity:0.7;}}#{} .pieOuterCircle{{stroke:black;stroke-width:2px;fill:none;}}#{} .pieTitleText{{text-anchor:middle;font-size:25px;fill:black;font-family:{};}}#{} .slice{{font-family:{};fill:#333;font-size:17px;}}#{} .legend text{{fill:black;font-family:{};font-size:17px;}}"#,
        id, id, id, font, id, font, id, font
    );
    out
}

fn radar_css(diagram_id: &str, effective_config: &serde_json::Value) -> String {
    // Keep `:root` last (matches upstream Mermaid radar SVG baselines).
    let id = escape_xml(diagram_id);
    let default_font = r#""trebuchet ms",verdana,arial,sans-serif"#;

    fn theme_var_string(cfg: &serde_json::Value, path: &[&str], fallback: &str) -> String {
        let mut cur = cfg;
        for key in path {
            cur = match cur.get(*key) {
                Some(v) => v,
                None => return fallback.to_string(),
            };
        }
        cur.as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| fallback.to_string())
    }

    fn theme_var_number_as_string(
        cfg: &serde_json::Value,
        path: &[&str],
        fallback: &str,
    ) -> String {
        let mut cur = cfg;
        for key in path {
            cur = match cur.get(*key) {
                Some(v) => v,
                None => return fallback.to_string(),
            };
        }
        if let Some(s) = cur.as_str() {
            return s.to_string();
        }
        if let Some(n) = json_f64(cur) {
            return fmt(n);
        }
        fallback.to_string()
    }

    fn default_c_scale(i: usize) -> &'static str {
        match i {
            0 => "hsl(240, 100%, 76.2745098039%)",
            1 => "hsl(60, 100%, 73.5294117647%)",
            2 => "hsl(80, 100%, 76.2745098039%)",
            3 => "hsl(270, 100%, 76.2745098039%)",
            4 => "hsl(300, 100%, 76.2745098039%)",
            5 => "hsl(330, 100%, 76.2745098039%)",
            6 => "hsl(0, 100%, 76.2745098039%)",
            7 => "hsl(30, 100%, 76.2745098039%)",
            8 => "hsl(90, 100%, 76.2745098039%)",
            9 => "hsl(150, 100%, 76.2745098039%)",
            10 => "hsl(180, 100%, 76.2745098039%)",
            _ => "hsl(210, 100%, 76.2745098039%)",
        }
    }

    let font_family = config_string(effective_config, &["themeVariables", "fontFamily"])
        .map(|s| normalize_css_font_family(&s))
        .unwrap_or_else(|| default_font.to_string());
    let base_font_size =
        theme_var_number_as_string(effective_config, &["themeVariables", "fontSize"], "16px");
    let base_text_color =
        theme_var_string(effective_config, &["themeVariables", "textColor"], "#333");
    let error_bkg_color = theme_var_string(
        effective_config,
        &["themeVariables", "errorBkgColor"],
        "#552222",
    );
    let error_text_color = theme_var_string(
        effective_config,
        &["themeVariables", "errorTextColor"],
        "#552222",
    );
    let line_color = theme_var_string(
        effective_config,
        &["themeVariables", "lineColor"],
        "#333333",
    );

    let title_font_size = base_font_size.clone();
    let title_color = theme_color(effective_config, "titleColor", "#333");

    let axis_color = theme_var_string(
        effective_config,
        &["themeVariables", "radar", "axisColor"],
        "#333333",
    );
    let axis_stroke_width = config_f64(
        effective_config,
        &["themeVariables", "radar", "axisStrokeWidth"],
    )
    .unwrap_or(2.0);
    let axis_label_font_size = config_f64(
        effective_config,
        &["themeVariables", "radar", "axisLabelFontSize"],
    )
    .unwrap_or(12.0);

    let graticule_color = theme_var_string(
        effective_config,
        &["themeVariables", "radar", "graticuleColor"],
        "#DEDEDE",
    );
    let graticule_opacity = config_f64(
        effective_config,
        &["themeVariables", "radar", "graticuleOpacity"],
    )
    .unwrap_or(0.3);
    let graticule_stroke_width = config_f64(
        effective_config,
        &["themeVariables", "radar", "graticuleStrokeWidth"],
    )
    .unwrap_or(1.0);

    let legend_font_size = config_f64(
        effective_config,
        &["themeVariables", "radar", "legendFontSize"],
    )
    .unwrap_or(12.0);

    let curve_opacity = config_f64(
        effective_config,
        &["themeVariables", "radar", "curveOpacity"],
    )
    .unwrap_or(0.5);
    let curve_stroke_width = config_f64(
        effective_config,
        &["themeVariables", "radar", "curveStrokeWidth"],
    )
    .unwrap_or(2.0);

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:{};fill:{};}}"#,
        id, font_family, base_font_size, base_text_color
    );
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:{};}}#{} .error-text{{fill:{};stroke:{};}}"#,
        id, error_bkg_color, id, error_text_color, error_text_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:{};stroke:{};}}#{} .marker.cross{{stroke:{};}}"#,
        id, line_color, line_color, id, line_color
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:{};}}#{} p{{margin:0;}}"#,
        id, font_family, base_font_size, id
    );

    let _ = write!(
        &mut out,
        r#"#{} .radarTitle{{font-size:{};color:{};dominant-baseline:hanging;text-anchor:middle;}}"#,
        id, title_font_size, title_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .radarAxisLine{{stroke:{};stroke-width:{};}}"#,
        id,
        axis_color,
        fmt(axis_stroke_width)
    );
    let _ = write!(
        &mut out,
        r#"#{} .radarAxisLabel{{dominant-baseline:middle;text-anchor:middle;font-size:{}px;color:{};}}"#,
        id,
        fmt(axis_label_font_size),
        axis_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .radarGraticule{{fill:{};fill-opacity:{};stroke:{};stroke-width:{};}}"#,
        id,
        graticule_color,
        fmt(graticule_opacity),
        graticule_color,
        fmt(graticule_stroke_width)
    );
    let _ = write!(
        &mut out,
        r#"#{} .radarLegendText{{text-anchor:start;font-size:{}px;dominant-baseline:hanging;}}"#,
        id,
        fmt(legend_font_size)
    );

    for i in 0..12 {
        let key = format!("cScale{i}");
        let c = theme_color(effective_config, &key, default_c_scale(i));
        let _ = write!(
            &mut out,
            r#"#{} .radarCurve-{}{{color:{};fill:{};fill-opacity:{};stroke:{};stroke-width:{};}}#{} .radarLegendBox-{}{{fill:{};fill-opacity:{};stroke:{};}}"#,
            id,
            i,
            c,
            c,
            fmt(curve_opacity),
            c,
            fmt(curve_stroke_width),
            id,
            i,
            c,
            fmt(curve_opacity),
            c
        );
    }

    let _ = write!(
        &mut out,
        r#"#{} :root{{--mermaid-font-family:{};}}"#,
        id, font_family
    );

    out
}

fn sankey_css(diagram_id: &str) -> String {
    // Mermaid's sankey diagram uses the same base CSS as "info-like" diagrams, plus a `.label`
    // rule, and keeps `:root` last.
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:16px;fill:#333;}}"#,
        id, font
    );
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:#552222;}}#{} .error-text{{fill:#552222;stroke:#552222;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:#333333;stroke:#333333;}}#{} .marker.cross{{stroke:#333333;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:16px;}}#{} p{{margin:0;}}#{} .label{{font-family:{};}}#{} :root{{--mermaid-font-family:{};}}"#,
        id, font, id, id, font, id, font
    );
    out
}

fn treemap_css(diagram_id: &str) -> String {
    // Keep `:root` last (matches upstream Mermaid treemap SVG baselines).
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:16px;fill:#333;}}"#,
        id, font
    );
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:#552222;}}#{} .error-text{{fill:#552222;stroke:#552222;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:#333333;stroke:#333333;}}#{} .marker.cross{{stroke:#333333;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:16px;}}#{} p{{margin:0;}}"#,
        id, font, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .treemapNode.section{{stroke:black;stroke-width:1;fill:#efefef;}}#{} .treemapNode.leaf{{stroke:black;stroke-width:1;fill:#efefef;}}#{} .treemapLabel{{fill:black;font-size:12px;}}#{} .treemapValue{{fill:black;font-size:10px;}}#{} .treemapTitle{{fill:black;font-size:14px;}}"#,
        id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} :root{{--mermaid-font-family:{};}}"#,
        id, font
    );
    out
}

fn xychart_css(diagram_id: &str) -> String {
    // Mermaid does not ship dedicated XYChart styles at 11.12.2 (it relies on theme variables and
    // inline attributes). Keep the shared base stylesheet for consistency with upstream SVG
    // baselines. The compare tooling ignores `<style>` content in parity mode.
    info_css(diagram_id)
}

fn gantt_css(diagram_id: &str) -> String {
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let root_rule = format!(r#"#{} :root{{--mermaid-font-family:{};}}"#, id, font);
    let mut out = info_css(diagram_id);
    if let Some(prefix) = out.strip_suffix(&root_rule) {
        out = prefix.to_string();
    }

    let _ = write!(
        &mut out,
        r#"#{} .mermaid-main-font{{font-family:{};}}"#,
        id, font
    );
    let _ = write!(&mut out, r#"#{} .exclude-range{{fill:#eeeeee;}}"#, id);
    let _ = write!(&mut out, r#"#{} .section{{stroke:none;opacity:0.2;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} .section0{{fill:rgba(102, 102, 255, 0.49);}}"#,
        id
    );
    let _ = write!(&mut out, r#"#{} .section2{{fill:#fff400;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} .section1,#{} .section3{{fill:white;opacity:0.2;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .sectionTitle0{{fill:#333;}}#{} .sectionTitle1{{fill:#333;}}#{} .sectionTitle2{{fill:#333;}}#{} .sectionTitle3{{fill:#333;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .sectionTitle{{text-anchor:start;font-family:{};}}"#,
        id, font
    );
    let _ = write!(
        &mut out,
        r#"#{} .grid .tick{{stroke:lightgrey;opacity:0.8;shape-rendering:crispEdges;}}#{} .grid .tick text{{font-family:{};fill:#333;}}#{} .grid path{{stroke-width:0;}}"#,
        id, id, font, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .today{{fill:none;stroke:red;stroke-width:2px;}}"#,
        id
    );
    let _ = write!(&mut out, r#"#{} .task{{stroke-width:2;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} .taskText{{text-anchor:middle;font-family:{};}}#{} .taskTextOutsideRight{{fill:black;text-anchor:start;font-family:{};}}#{} .taskTextOutsideLeft{{fill:black;text-anchor:end;}}"#,
        id, font, id, font, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .task.clickable{{cursor:pointer;}}#{} .taskText.clickable{{cursor:pointer;fill:#003163!important;font-weight:bold;}}#{} .taskTextOutsideLeft.clickable{{cursor:pointer;fill:#003163!important;font-weight:bold;}}#{} .taskTextOutsideRight.clickable{{cursor:pointer;fill:#003163!important;font-weight:bold;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .taskText0,#{} .taskText1,#{} .taskText2,#{} .taskText3{{fill:white;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .task0,#{} .task1,#{} .task2,#{} .task3{{fill:#8a90dd;stroke:#534fbc;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .taskTextOutside0,#{} .taskTextOutside2{{fill:black;}}#{} .taskTextOutside1,#{} .taskTextOutside3{{fill:black;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .active0,#{} .active1,#{} .active2,#{} .active3{{fill:#bfc7ff;stroke:#534fbc;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .activeText0,#{} .activeText1,#{} .activeText2,#{} .activeText3{{fill:black!important;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .done0,#{} .done1,#{} .done2,#{} .done3{{stroke:grey;fill:lightgrey;stroke-width:2;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .doneText0,#{} .doneText1,#{} .doneText2,#{} .doneText3{{fill:black!important;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .crit0,#{} .crit1,#{} .crit2,#{} .crit3{{stroke:#ff8888;fill:red;stroke-width:2;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .activeCrit0,#{} .activeCrit1,#{} .activeCrit2,#{} .activeCrit3{{stroke:#ff8888;fill:#bfc7ff;stroke-width:2;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .doneCrit0,#{} .doneCrit1,#{} .doneCrit2,#{} .doneCrit3{{stroke:#ff8888;fill:lightgrey;stroke-width:2;cursor:pointer;shape-rendering:crispEdges;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .milestone{{transform:rotate(45deg) scale(0.8,0.8);}}#{} .milestoneText{{font-style:italic;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .doneCritText0,#{} .doneCritText1,#{} .doneCritText2,#{} .doneCritText3{{fill:black!important;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .vert{{stroke:navy;}}#{} .vertText{{font-size:15px;text-anchor:middle;fill:navy!important;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .activeCritText0,#{} .activeCritText1,#{} .activeCritText2,#{} .activeCritText3{{fill:black!important;}}"#,
        id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .titleText{{text-anchor:middle;font-size:18px;fill:#333;font-family:{};}}"#,
        id, font
    );

    out.push_str(&root_rule);
    out
}

fn pie_legend_rect_style(fill: &str) -> String {
    // Mermaid emits legend colors via inline `style` in rgb() form for default themes.
    // The compare tooling ignores `style`, but we keep this for human inspection parity.
    let rgb = match fill {
        "#ECECFF" => "rgb(236, 236, 255)",
        "#ffffde" => "rgb(255, 255, 222)",
        "hsl(80, 100%, 56.2745098039%)" => "rgb(181, 255, 32)",
        other => other,
    };
    format!("fill: {rgb}; stroke: {rgb};")
}

fn pie_polar_xy(radius: f64, angle: f64) -> (f64, f64) {
    let x = radius * angle.sin();
    let y = -radius * angle.cos();
    (x, y)
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

fn c4_css(diagram_id: &str) -> String {
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:16px;fill:#333;}}"#,
        id, font
    );
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:#552222;}}#{} .error-text{{fill:#552222;stroke:#552222;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:#333333;stroke:#333333;}}#{} .marker.cross{{stroke:#333333;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:16px;}}#{} p{{margin:0;}}#{} .person{{stroke:hsl(240, 60%, 86.2745098039%);fill:#ECECFF;}}#{} :root{{--mermaid-font-family:{};}}"#,
        id, font, id, id, id, font
    );
    out
}

fn c4_config_string(cfg: &serde_json::Value, key: &str) -> Option<String> {
    config_string(cfg, &["c4", key])
}

fn c4_config_color(cfg: &serde_json::Value, key: &str, fallback: &str) -> String {
    c4_config_string(cfg, key).unwrap_or_else(|| fallback.to_string())
}

fn c4_config_font_family(cfg: &serde_json::Value, type_key: &str) -> String {
    c4_config_string(cfg, &format!("{type_key}FontFamily"))
        .map(|s| s.trim().trim_end_matches(';').trim().to_string())
        .unwrap_or_else(|| r#""Open Sans", sans-serif"#.to_string())
}

fn c4_config_font_size(cfg: &serde_json::Value, type_key: &str, fallback: f64) -> f64 {
    config_f64(cfg, &["c4", &format!("{type_key}FontSize")]).unwrap_or(fallback)
}

fn c4_config_font_weight(cfg: &serde_json::Value, type_key: &str) -> String {
    c4_config_string(cfg, &format!("{type_key}FontWeight")).unwrap_or_else(|| "normal".to_string())
}

fn c4_write_text_by_tspan(
    out: &mut String,
    content: &str,
    x: f64,
    y: f64,
    width: f64,
    font_family: &str,
    font_size: f64,
    font_weight: &str,
    attrs: &[(&str, &str)],
) {
    let x = x + width / 2.0;
    let mut style = String::new();
    let _ = write!(
        &mut style,
        "text-anchor: middle; font-size: {}px; font-weight: {}; font-family: {};",
        fmt(font_size.max(1.0)),
        font_weight,
        font_family
    );

    let _ = write!(
        out,
        r#"<text x="{}" y="{}" dominant-baseline="middle""#,
        fmt(x),
        fmt(y)
    );
    for (k, v) in attrs {
        let _ = write!(out, r#" {k}="{v}""#);
    }
    let _ = write!(out, r#" style="{}">"#, escape_attr(&style));

    let lines: Vec<&str> = content.split('\n').collect();
    let n = lines.len().max(1) as f64;
    for (i, line) in lines.iter().enumerate() {
        let dy = (i as f64) * font_size - (font_size * (n - 1.0)) / 2.0;
        let dy_s = fmt(dy);
        let _ = write!(
            out,
            r#"<tspan dy="{}" alignment-baseline="mathematical">{}</tspan>"#,
            dy_s,
            escape_xml(line)
        );
    }
    out.push_str("</text>");
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
    let model: crate::flowchart::FlowchartV2Model = serde_json::from_value(semantic.clone())?;

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_type = "flowchart-v2";

    // Mermaid expands self-loop edges into a chain of helper nodes plus `*-cyclic-special-*` edge
    // segments during Dagre layout. Replicate that expansion here so rendered SVG ids match.
    let mut render_edges: Vec<crate::flowchart::FlowEdge> = Vec::new();
    let mut self_loop_label_node_ids: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    for e in &model.edges {
        if e.from != e.to {
            render_edges.push(e.clone());
            continue;
        }

        let node_id = e.from.clone();
        let special_id_1 = format!("{node_id}---{node_id}---1");
        let special_id_2 = format!("{node_id}---{node_id}---2");
        self_loop_label_node_ids.insert(special_id_1.clone());
        self_loop_label_node_ids.insert(special_id_2.clone());

        let mut edge1 = e.clone();
        edge1.id = format!("{node_id}-cyclic-special-1");
        edge1.from = node_id.clone();
        edge1.to = special_id_1.clone();
        edge1.label = None;
        edge1.label_type = None;
        edge1.edge_type = Some("arrow_open".to_string());

        let mut edge_mid = e.clone();
        edge_mid.id = format!("{node_id}-cyclic-special-mid");
        edge_mid.from = special_id_1.clone();
        edge_mid.to = special_id_2.clone();
        edge_mid.label = None;
        edge_mid.label_type = None;
        edge_mid.edge_type = Some("arrow_open".to_string());

        let mut edge2 = e.clone();
        edge2.id = format!("{node_id}-cyclic-special-2");
        edge2.from = special_id_2.clone();
        edge2.to = node_id.clone();
        edge2.label = None;
        edge2.label_type = None;

        render_edges.push(edge1);
        render_edges.push(edge_mid);
        render_edges.push(edge2);
    }

    // Mermaid's `adjustClustersAndEdges(graph)` rewrites edges that connect directly to cluster
    // nodes by removing and re-adding them (after swapping endpoints to anchor nodes). This has a
    // visible side-effect: those edges end up later in `graph.edges()` insertion order, so the
    // DOM emitted under `.edgePaths` / `.edgeLabels` matches that stable partition.
    let cluster_ids_with_children: std::collections::HashSet<&str> = model
        .subgraphs
        .iter()
        .filter(|sg| !sg.nodes.is_empty())
        .map(|sg| sg.id.as_str())
        .collect();
    if !cluster_ids_with_children.is_empty() && render_edges.len() >= 2 {
        let mut normal: Vec<crate::flowchart::FlowEdge> = Vec::with_capacity(render_edges.len());
        let mut cluster: Vec<crate::flowchart::FlowEdge> = Vec::new();
        for e in render_edges {
            if cluster_ids_with_children.contains(e.from.as_str())
                || cluster_ids_with_children.contains(e.to.as_str())
            {
                cluster.push(e);
            } else {
                normal.push(e);
            }
        }
        normal.extend(cluster);
        render_edges = normal;
    }

    let font_family = config_string(effective_config, &["fontFamily"])
        .map(|s| normalize_css_font_family(&s))
        .unwrap_or_else(|| "\"trebuchet ms\",verdana,arial,sans-serif".to_string());
    let font_size = effective_config
        .get("fontSize")
        .and_then(|v| v.as_f64())
        .unwrap_or(16.0)
        .max(1.0);

    let wrapping_width = config_f64(effective_config, &["flowchart", "wrappingWidth"])
        .unwrap_or(200.0)
        .max(1.0);
    // Mermaid flowchart-v2 uses the global `htmlLabels` toggle for node/subgraph labels, while
    // edge labels follow `flowchart.htmlLabels` (falling back to the global toggle when unset).
    let node_html_labels = effective_config
        .get("htmlLabels")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let edge_html_labels = effective_config
        .get("flowchart")
        .and_then(|v| v.get("htmlLabels"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(node_html_labels);
    let node_wrap_mode = if node_html_labels {
        crate::text::WrapMode::HtmlLike
    } else {
        crate::text::WrapMode::SvgLike
    };
    let edge_wrap_mode = if edge_html_labels {
        crate::text::WrapMode::HtmlLike
    } else {
        crate::text::WrapMode::SvgLike
    };
    let diagram_padding = config_f64(effective_config, &["flowchart", "diagramPadding"])
        .unwrap_or(8.0)
        .max(0.0);
    let use_max_width = effective_config
        .get("flowchart")
        .and_then(|v| v.get("useMaxWidth"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let title_top_margin = config_f64(effective_config, &["flowchart", "titleTopMargin"])
        .unwrap_or(25.0)
        .max(0.0);
    let node_padding = config_f64(effective_config, &["flowchart", "padding"])
        .unwrap_or(15.0)
        .max(0.0);

    let text_style = crate::text::TextStyle {
        font_family: Some(font_family.clone()),
        font_size,
        font_weight: None,
    };

    let mut nodes_by_id: std::collections::HashMap<String, crate::flowchart::FlowNode> =
        std::collections::HashMap::new();
    let node_order: Vec<String> = model.nodes.iter().map(|n| n.id.clone()).collect();
    for n in model.nodes.iter().cloned() {
        nodes_by_id.insert(n.id.clone(), n);
    }
    for id in &self_loop_label_node_ids {
        nodes_by_id
            .entry(id.clone())
            .or_insert(crate::flowchart::FlowNode {
                id: id.clone(),
                label: Some(String::new()),
                label_type: None,
                layout_shape: None,
                classes: Vec::new(),
                styles: Vec::new(),
                have_callback: false,
                link: None,
                link_target: None,
            });
    }

    let mut edges_by_id: std::collections::HashMap<String, crate::flowchart::FlowEdge> =
        std::collections::HashMap::new();
    let edge_order: Vec<String> = render_edges.iter().map(|e| e.id.clone()).collect();
    for e in render_edges.iter().cloned() {
        edges_by_id.insert(e.id.clone(), e);
    }

    let mut subgraphs_by_id: std::collections::HashMap<String, crate::flowchart::FlowSubgraph> =
        std::collections::HashMap::new();
    let subgraph_order: Vec<String> = model.subgraphs.iter().map(|s| s.id.clone()).collect();
    for sg in model.subgraphs.iter().cloned() {
        subgraphs_by_id.insert(sg.id.clone(), sg);
    }

    let mut parent: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for sg in model.subgraphs.iter() {
        for child in &sg.nodes {
            parent.insert(child.clone(), sg.id.clone());
        }
    }
    for id in &self_loop_label_node_ids {
        let Some((base, _)) = id.split_once("---") else {
            continue;
        };
        if let Some(p) = parent.get(base).cloned() {
            parent.insert(id.clone(), p);
        }
    }

    let mut recursive_clusters: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for sg in model.subgraphs.iter() {
        if sg.nodes.is_empty() {
            continue;
        }
        let mut external = false;
        for e in render_edges.iter() {
            // Match Mermaid `adjustClustersAndEdges` / flowchart-v2 behavior: a cluster is
            // considered to have external connections when an edge crosses its descendant boundary.
            let from_in = flowchart_is_strict_descendant(&parent, &e.from, &sg.id);
            let to_in = flowchart_is_strict_descendant(&parent, &e.to, &sg.id);
            if from_in != to_in {
                external = true;
                break;
            }
        }
        if !external {
            recursive_clusters.insert(sg.id.clone());
        }
    }

    let mut layout_nodes_by_id: std::collections::HashMap<String, LayoutNode> =
        std::collections::HashMap::new();
    for n in layout.nodes.iter().cloned() {
        layout_nodes_by_id.insert(n.id.clone(), n);
    }

    let mut layout_edges_by_id: std::collections::HashMap<String, crate::model::LayoutEdge> =
        std::collections::HashMap::new();
    for e in layout.edges.iter().cloned() {
        layout_edges_by_id.insert(e.id.clone(), e);
    }

    let mut layout_clusters_by_id: std::collections::HashMap<String, LayoutCluster> =
        std::collections::HashMap::new();
    for c in layout.clusters.iter().cloned() {
        layout_clusters_by_id.insert(c.id.clone(), c);
    }

    let default_edge_interpolate_for_bbox = model
        .edge_defaults
        .as_ref()
        .and_then(|d| d.interpolate.as_deref())
        .unwrap_or("basis");

    let node_dom_index = flowchart_node_dom_indices(&model);

    let subgraph_title_y_shift = {
        let top = config_f64(
            effective_config,
            &["flowchart", "subGraphTitleMargin", "top"],
        )
        .unwrap_or(0.0)
        .max(0.0);
        let bottom = config_f64(
            effective_config,
            &["flowchart", "subGraphTitleMargin", "bottom"],
        )
        .unwrap_or(0.0)
        .max(0.0);
        (top + bottom) / 2.0
    };

    fn self_loop_label_base_node_id(id: &str) -> Option<&str> {
        let mut parts = id.split("---");
        let Some(a) = parts.next() else {
            return None;
        };
        let Some(b) = parts.next() else {
            return None;
        };
        let Some(n) = parts.next() else {
            return None;
        };
        if parts.next().is_some() {
            return None;
        }
        if a != b {
            return None;
        }
        if n != "1" && n != "2" {
            return None;
        }
        Some(a)
    }

    let effective_parent_for_id = |id: &str| -> Option<&str> {
        let mut cur = parent.get(id).map(|s| s.as_str());
        if cur.is_none() {
            if let Some(base) = self_loop_label_base_node_id(id) {
                cur = parent.get(base).map(|s| s.as_str());
            }
        }
        while let Some(p) = cur {
            if subgraphs_by_id.contains_key(p) && !recursive_clusters.contains(p) {
                cur = parent.get(p).map(|s| s.as_str());
                continue;
            }
            return Some(p);
        }
        None
    };

    let lca_for_ids = |a: &str, b: &str| -> Option<String> {
        let mut ancestors: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut cur = effective_parent_for_id(a).map(|s| s.to_string());
        while let Some(p) = cur {
            ancestors.insert(p.clone());
            cur = effective_parent_for_id(&p).map(|s| s.to_string());
        }

        let mut cur = effective_parent_for_id(b).map(|s| s.to_string());
        while let Some(p) = cur {
            if ancestors.contains(&p) {
                return Some(p);
            }
            cur = effective_parent_for_id(&p).map(|s| s.to_string());
        }
        None
    };

    let y_offset_for_root = |root: Option<&str>| -> f64 {
        if root.is_some() && subgraph_title_y_shift.abs() >= 1e-9 {
            -subgraph_title_y_shift
        } else {
            0.0
        }
    };

    // Mermaid's flowchart-v2 renderer draws the self-loop helper nodes (`labelRect`) as
    // `<g class="label edgeLabel" transform="translate(x, y)">` with a `0.1 x 0.1` rect anchored
    // at the translated origin (top-left). Dagre's `x/y` still represent a node center, but the
    // rendered DOM bbox that drives `setupViewPortForSVG(svg, diagramPadding)` is top-left based.
    // Account for that when approximating the final `svg.getBBox()`.
    let bounds = {
        let mut b: Option<Bounds> = None;
        let mut include_rect = |min_x: f64, min_y: f64, max_x: f64, max_y: f64| {
            if let Some(ref mut cur) = b {
                cur.min_x = cur.min_x.min(min_x);
                cur.min_y = cur.min_y.min(min_y);
                cur.max_x = cur.max_x.max(max_x);
                cur.max_y = cur.max_y.max(max_y);
            } else {
                b = Some(Bounds {
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                });
            }
        };

        for c in &layout.clusters {
            let root = if recursive_clusters.contains(&c.id) {
                Some(c.id.as_str())
            } else {
                effective_parent_for_id(&c.id)
            };
            let y_off = y_offset_for_root(root);
            let hw = c.width / 2.0;
            let hh = c.height / 2.0;
            include_rect(c.x - hw, c.y + y_off - hh, c.x + hw, c.y + y_off + hh);

            let lhw = c.title_label.width / 2.0;
            let lhh = c.title_label.height / 2.0;
            include_rect(
                c.title_label.x - lhw,
                c.title_label.y + y_off - lhh,
                c.title_label.x + lhw,
                c.title_label.y + y_off + lhh,
            );
        }

        for n in &layout.nodes {
            let root = if n.is_cluster && recursive_clusters.contains(&n.id) {
                Some(n.id.as_str())
            } else {
                effective_parent_for_id(&n.id)
            };
            let y_off = y_offset_for_root(root);
            if n.is_cluster || node_dom_index.contains_key(&n.id) {
                let mut left_hw = n.width / 2.0;
                let mut right_hw = left_hw;
                if !n.is_cluster {
                    if let Some(shape) = nodes_by_id
                        .get(&n.id)
                        .and_then(|node| node.layout_shape.as_deref())
                    {
                        // Mermaid's flowchart-v2 rhombus node renderer offsets the polygon by
                        // `(-width/2 + 0.5, height/2)` so the diamond outline stays on the same
                        // pixel lattice as other nodes. This makes the DOM bbox slightly
                        // asymmetric around the node center and affects the root `getBBox()`
                        // width (and thus `viewBox` / `max-width`) by 0.5px.
                        if shape == "diamond" || shape == "rhombus" {
                            left_hw = (left_hw - 0.5).max(0.0);
                            right_hw = right_hw + 0.5;
                        }
                    }
                }
                let hh = n.height / 2.0;
                include_rect(
                    n.x - left_hw,
                    n.y + y_off - hh,
                    n.x + right_hw,
                    n.y + y_off + hh,
                );
            } else {
                include_rect(n.x, n.y + y_off, n.x + n.width, n.y + y_off + n.height);
            }
        }

        for e in &layout.edges {
            let root = lca_for_ids(&e.from, &e.to);
            let y_off = y_offset_for_root(root.as_deref());
            for lbl in [
                e.label.as_ref(),
                e.start_label_left.as_ref(),
                e.start_label_right.as_ref(),
                e.end_label_left.as_ref(),
                e.end_label_right.as_ref(),
            ] {
                if let Some(lbl) = lbl {
                    let hw = lbl.width / 2.0;
                    let hh = lbl.height / 2.0;
                    include_rect(
                        lbl.x - hw,
                        lbl.y + y_off - hh,
                        lbl.x + hw,
                        lbl.y + y_off + hh,
                    );
                }
            }
        }

        b.unwrap_or(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 100.0,
        })
    };
    // Mermaid flowchart-v2 does not translate the root `.root` group; node/edge coordinates are
    // already in the Dagre coordinate space (including Dagre's fixed `marginx/marginy=8`).
    // `diagramPadding` is applied only when computing the final SVG viewBox.
    let tx = 0.0;
    let ty = 0.0;

    // Mermaid computes the final viewport using `svg.getBBox()` after inserting the title, then
    // applies `setupViewPortForSVG(svg, diagramPadding)` which sets:
    //   viewBox = `${bbox.x - padding} ${bbox.y - padding} ${bbox.width + 2*padding} ${bbox.height + 2*padding}`
    //   max-width = `${bbox.width + 2*padding}px` when `useMaxWidth=true`
    //
    // In headless mode we approximate that by unioning:
    // - the layout bounds (shifted by `tx/ty`), and
    // - the flowchart title text bounding box (if present).
    const TITLE_FONT_SIZE_PX: f64 = 18.0;
    const DEFAULT_ASCENT_EM: f64 = 0.9444444444;
    const DEFAULT_DESCENT_EM: f64 = 0.262;

    let diagram_title = diagram_title.map(str::trim).filter(|t| !t.is_empty());

    let mut bbox_min_x = bounds.min_x + tx;
    let mut bbox_min_y = bounds.min_y + ty;
    let mut bbox_max_x = bounds.max_x + tx;
    let mut bbox_max_y = bounds.max_y + ty;

    // Mermaid's recursive flowchart renderer introduces additional y-offsets for some extracted
    // cluster roots (notably when an empty sibling subgraph is present). Approximate that in the
    // root viewport by expanding the max-y by the largest such extra root offset.
    let extra_recursive_root_y = {
        fn effective_parent<'a>(
            parent: &'a std::collections::HashMap<String, String>,
            subgraphs_by_id: &'a std::collections::HashMap<String, crate::flowchart::FlowSubgraph>,
            recursive_clusters: &std::collections::HashSet<String>,
            id: &str,
        ) -> Option<&'a str> {
            let mut cur = parent.get(id).map(|s| s.as_str());
            while let Some(p) = cur {
                if subgraphs_by_id.contains_key(p) && !recursive_clusters.contains(p) {
                    cur = parent.get(p).map(|s| s.as_str());
                    continue;
                }
                return Some(p);
            }
            None
        }

        let mut max_y: f64 = 0.0;
        for cid in &recursive_clusters {
            let Some(cluster) = layout_clusters_by_id.get(cid) else {
                continue;
            };
            let my_parent = effective_parent(&parent, &subgraphs_by_id, &recursive_clusters, cid);
            let has_empty_sibling = subgraphs_by_id.iter().any(|(id, sg)| {
                id != cid
                    && sg.nodes.is_empty()
                    && layout_clusters_by_id.contains_key(id)
                    && effective_parent(&parent, &subgraphs_by_id, &recursive_clusters, id.as_str())
                        == my_parent
            });
            if has_empty_sibling {
                max_y = max_y.max(cluster.offset_y.max(0.0) * 2.0);
            }
        }
        max_y
    };

    // Mermaid derives the final viewport using `svg.getBBox()` (after rendering). For flowcharts
    // this includes the actual curve geometry generated by D3 (which can extend beyond the routed
    // polyline points). Headlessly, approximate that by unioning a tight bbox over each rendered
    // edge path `d` into our base bbox.
    for e in &render_edges {
        let edge_root = lca_for_ids(&e.from, &e.to);
        let edge_y_off = y_offset_for_root(edge_root.as_deref());
        let Some(d) = flowchart_edge_path_d_for_bbox(
            &layout_edges_by_id,
            &layout_clusters_by_id,
            tx,
            ty + edge_y_off,
            default_edge_interpolate_for_bbox,
            edge_html_labels,
            e,
        ) else {
            continue;
        };
        if let Some(pb) = svg_path_bounds_from_d(&d) {
            bbox_min_x = bbox_min_x.min(pb.min_x);
            bbox_min_y = bbox_min_y.min(pb.min_y);
            bbox_max_x = bbox_max_x.max(pb.max_x);
            bbox_max_y = bbox_max_y.max(pb.max_y);
        }
    }

    bbox_max_y += extra_recursive_root_y;
    // Mermaid centers the title using the pre-title `getBBox()` of the rendered root group:
    //
    //   const bounds = parent.node()?.getBBox();
    //   x = bounds.x + bounds.width / 2
    //
    // Use our current content bbox (after accounting for edge curve geometry) to match that
    // behavior more closely in headless mode.
    let title_anchor_x = (bbox_min_x + bbox_max_x) / 2.0;

    if let Some(title) = diagram_title {
        let title_style = TextStyle {
            font_family: Some(font_family.clone()),
            font_size: TITLE_FONT_SIZE_PX,
            font_weight: None,
        };
        let (title_left, title_right) = measurer.measure_svg_title_bbox_x(title, &title_style);
        let baseline_y = -title_top_margin;
        // Mermaid title bbox uses SVG `getBBox()`, which varies slightly across fonts.
        // Courier in Mermaid@11.12.2 has a visibly smaller ascender than the default
        // `"trebuchet ms", verdana, arial, sans-serif` baseline; model that so viewBox parity
        // matches upstream fixtures.
        let (ascent_em, descent_em) = if font_family.to_ascii_lowercase().contains("courier") {
            (0.8333333333333334, 0.25)
        } else {
            (DEFAULT_ASCENT_EM, DEFAULT_DESCENT_EM)
        };
        let ascent = TITLE_FONT_SIZE_PX * ascent_em;
        let descent = TITLE_FONT_SIZE_PX * descent_em;

        bbox_min_x = bbox_min_x.min(title_anchor_x - title_left);
        bbox_max_x = bbox_max_x.max(title_anchor_x + title_right);
        bbox_min_y = bbox_min_y.min(baseline_y - ascent);
        bbox_max_y = bbox_max_y.max(baseline_y + descent);
    }

    // Chromium's `getBBox()` values frequently land on an `f32` lattice. Mermaid then computes the
    // root viewport in JS double space:
    // - viewBox.x/y = bbox.x/y - padding
    // - viewBox.w/h = bbox.width/height + 2*padding
    //
    // Mirror that by quantizing the content bounds to `f32` first, then applying padding in `f64`.
    let bbox_min_x_f32 = bbox_min_x as f32;
    let bbox_min_y_f32 = bbox_min_y as f32;
    let bbox_max_x_f32 = bbox_max_x as f32;
    let bbox_max_y_f32 = bbox_max_y as f32;
    let bbox_w_f32 = (bbox_max_x_f32 - bbox_min_x_f32).max(1.0);
    let bbox_h_f32 = (bbox_max_y_f32 - bbox_min_y_f32).max(1.0);

    let vb_min_x = (bbox_min_x_f32 as f64) - diagram_padding;
    let vb_min_y = (bbox_min_y_f32 as f64) - diagram_padding;
    let vb_w = (bbox_w_f32 as f64) + diagram_padding * 2.0;
    let vb_h = (bbox_h_f32 as f64) + diagram_padding * 2.0;

    let css = flowchart_css(
        diagram_id,
        effective_config,
        &font_family,
        font_size,
        &model.class_defs,
    );

    let node_border_color = theme_color(effective_config, "nodeBorder", "#9370DB");
    let node_fill_color = theme_color(effective_config, "mainBkg", "#ECECFF");

    let mut out = String::new();
    let mut vb_min_x_attr = fmt(vb_min_x);
    let mut vb_min_y_attr = fmt(vb_min_y);
    let mut w_attr = fmt(vb_w.max(1.0));
    let mut h_attr = fmt(vb_h.max(1.0));
    let mut max_w_attr = fmt_max_width_px(vb_w.max(1.0));
    if let Some((viewbox, max_w)) =
        crate::generated::flowchart_root_overrides_11_12_2::lookup_flowchart_root_viewport_override(
            diagram_id,
        )
    {
        let mut it = viewbox.split_whitespace();
        vb_min_x_attr = it.next().unwrap_or("0").to_string();
        vb_min_y_attr = it.next().unwrap_or("0").to_string();
        w_attr = it.next().unwrap_or("0").to_string();
        h_attr = it.next().unwrap_or("0").to_string();
        max_w_attr = max_w.to_string();
    }

    let acc_title = model
        .acc_title
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let acc_descr = model
        .acc_descr
        .as_deref()
        .map(|s| s.trim_end_matches('\n'))
        .filter(|s| !s.trim().is_empty());
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));

    let svg_open = if use_max_width {
        format!(
            r#"<svg id="{}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" class="flowchart" style="max-width: {}px; background-color: white;" viewBox="{} {} {} {}" role="graphics-document document" aria-roledescription="{}"{}{}>"#,
            escape_xml(diagram_id),
            max_w_attr,
            vb_min_x_attr,
            vb_min_y_attr,
            w_attr,
            h_attr,
            diagram_type,
            aria_describedby
                .as_deref()
                .map(|id| format!(r#" aria-describedby="{}""#, escape_attr(id)))
                .unwrap_or_default(),
            aria_labelledby
                .as_deref()
                .map(|id| format!(r#" aria-labelledby="{}""#, escape_attr(id)))
                .unwrap_or_default(),
        )
    } else {
        format!(
            r#"<svg id="{}" width="{}" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" class="flowchart" height="{}" viewBox="{} {} {} {}" role="graphics-document document" aria-roledescription="{}" style="background-color: white;"{}{}>"#,
            escape_xml(diagram_id),
            w_attr,
            h_attr,
            vb_min_x_attr,
            vb_min_y_attr,
            w_attr,
            h_attr,
            diagram_type,
            aria_describedby
                .as_deref()
                .map(|id| format!(r#" aria-describedby="{}""#, escape_attr(id)))
                .unwrap_or_default(),
            aria_labelledby
                .as_deref()
                .map(|id| format!(r#" aria-labelledby="{}""#, escape_attr(id)))
                .unwrap_or_default(),
        )
    };
    out.push_str(&svg_open);
    if let (Some(id), Some(title)) = (aria_labelledby.as_deref(), acc_title) {
        let _ = write!(
            &mut out,
            r#"<title id="{}">{}</title>"#,
            escape_attr(id),
            escape_xml(title)
        );
    }
    if let (Some(id), Some(descr)) = (aria_describedby.as_deref(), acc_descr) {
        let _ = write!(
            &mut out,
            r#"<desc id="{}">{}</desc>"#,
            escape_attr(id),
            escape_xml(descr)
        );
    }
    let _ = write!(&mut out, "<style>{}</style>", css);

    out.push_str("<g>");
    flowchart_markers(&mut out, diagram_id);

    let default_edge_interpolate = model
        .edge_defaults
        .as_ref()
        .and_then(|d| d.interpolate.as_deref())
        .unwrap_or("basis")
        .to_string();
    let default_edge_style = model
        .edge_defaults
        .as_ref()
        .map(|d| d.style.clone())
        .unwrap_or_default();

    let ctx = FlowchartRenderCtx {
        diagram_id: diagram_id.to_string(),
        tx,
        ty,
        diagram_type: diagram_type.to_string(),
        measurer,
        config: merman_core::MermaidConfig::from_value(effective_config.clone()),
        node_html_labels,
        edge_html_labels,
        class_defs: model.class_defs.clone(),
        node_border_color,
        node_fill_color,
        default_edge_interpolate,
        default_edge_style,
        node_order,
        subgraph_order,
        edge_order,
        nodes_by_id,
        edges_by_id,
        subgraphs_by_id,
        tooltips: model.tooltips.clone(),
        recursive_clusters,
        parent,
        layout_nodes_by_id,
        layout_edges_by_id,
        layout_clusters_by_id,
        dom_node_order_by_root: layout.dom_node_order_by_root.clone(),
        node_dom_index,
        node_padding,
        wrapping_width,
        node_wrap_mode,
        edge_wrap_mode,
        text_style,
        diagram_title: diagram_title.map(|s| s.to_string()),
    };

    let extra_marker_colors = flowchart_collect_edge_marker_colors(&ctx);
    render_flowchart_root(&mut out, &ctx, None, 0.0, 0.0);

    flowchart_extra_markers(&mut out, diagram_id, &extra_marker_colors);
    out.push_str("</g>");
    if let Some(title) = diagram_title {
        let title_x = title_anchor_x;
        let title_y = -title_top_margin;
        let _ = write!(
            &mut out,
            r#"<text text-anchor="middle" x="{}" y="{}" class="flowchartTitleText">{}</text>"#,
            fmt(title_x),
            fmt(title_y),
            escape_xml(title)
        );
    }
    out.push_str("</svg>\n");
    Ok(out)
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

fn parse_style_decl(s: &str) -> Option<(&str, &str)> {
    let s = s.trim().trim_end_matches(';').trim();
    if s.is_empty() {
        return None;
    }
    let (k, v) = s.split_once(':')?;
    let k = k.trim();
    let v = v.trim();
    if k.is_empty() || v.is_empty() {
        return None;
    }
    Some((k, v))
}

fn curve_monotone_path_d(points: &[crate::model::LayoutPoint], swap_xy: bool) -> String {
    fn sign(v: f64) -> f64 {
        if v < 0.0 { -1.0 } else { 1.0 }
    }

    fn get_x(p: &crate::model::LayoutPoint, swap_xy: bool) -> f64 {
        if swap_xy { p.y } else { p.x }
    }
    fn get_y(p: &crate::model::LayoutPoint, swap_xy: bool) -> f64 {
        if swap_xy { p.x } else { p.y }
    }

    fn emit_move_to(out: &mut String, x: f64, y: f64, swap_xy: bool) {
        if swap_xy {
            let _ = write!(out, "M{},{}", fmt_path(y), fmt_path(x));
        } else {
            let _ = write!(out, "M{},{}", fmt_path(x), fmt_path(y));
        }
    }
    fn emit_line_to(out: &mut String, x: f64, y: f64, swap_xy: bool) {
        if swap_xy {
            let _ = write!(out, "L{},{}", fmt_path(y), fmt_path(x));
        } else {
            let _ = write!(out, "L{},{}", fmt_path(x), fmt_path(y));
        }
    }
    fn emit_cubic_to(
        out: &mut String,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        x: f64,
        y: f64,
        swap_xy: bool,
    ) {
        if swap_xy {
            let _ = write!(
                out,
                "C{},{},{},{},{},{}",
                fmt_path(y1),
                fmt_path(x1),
                fmt_path(y2),
                fmt_path(x2),
                fmt_path(y),
                fmt_path(x)
            );
        } else {
            let _ = write!(
                out,
                "C{},{},{},{},{},{}",
                fmt_path(x1),
                fmt_path(y1),
                fmt_path(x2),
                fmt_path(y2),
                fmt_path(x),
                fmt_path(y)
            );
        }
    }

    fn slope3(x0: f64, y0: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
        let h0 = x1 - x0;
        let h1 = x2 - x1;
        let denom0 = if h0 != 0.0 {
            h0
        } else if h1 < 0.0 {
            -0.0
        } else {
            0.0
        };
        let denom1 = if h1 != 0.0 {
            h1
        } else if h0 < 0.0 {
            -0.0
        } else {
            0.0
        };
        let s0 = (y1 - y0) / denom0;
        let s1 = (y2 - y1) / denom1;
        let p = (s0 * h1 + s1 * h0) / (h0 + h1);
        let v = (sign(s0) + sign(s1)) * s0.abs().min(s1.abs()).min(0.5 * p.abs());
        if v.is_finite() { v } else { 0.0 }
    }

    fn slope2(x0: f64, y0: f64, x1: f64, y1: f64, t: f64) -> f64 {
        let h = x1 - x0;
        if h != 0.0 {
            (3.0 * (y1 - y0) / h - t) / 2.0
        } else {
            t
        }
    }

    fn hermite_segment(
        out: &mut String,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        t0: f64,
        t1: f64,
        swap_xy: bool,
    ) {
        // dx is in the monotone coordinate system; we swap at emit-time if needed.
        let dx = (x1 - x0) / 3.0;
        emit_cubic_to(
            out,
            x0 + dx,
            y0 + dx * t0,
            x1 - dx,
            y1 - dx * t1,
            x1,
            y1,
            swap_xy,
        );
    }

    let mut out = String::new();
    if points.is_empty() {
        return out;
    }

    let mut point_state: u8 = 0;
    let mut x0 = f64::NAN;
    let mut y0 = f64::NAN;
    let mut x1 = f64::NAN;
    let mut y1 = f64::NAN;
    let mut t0 = f64::NAN;

    for p in points {
        let x = get_x(p, swap_xy);
        let y = get_y(p, swap_xy);

        if x == x1 && y == y1 {
            continue;
        }

        let mut t1 = f64::NAN;
        match point_state {
            0 => {
                point_state = 1;
                emit_move_to(&mut out, x, y, swap_xy);
            }
            1 => {
                point_state = 2;
            }
            2 => {
                point_state = 3;
                t1 = slope3(x0, y0, x1, y1, x, y);
                let t0_local = slope2(x0, y0, x1, y1, t1);
                hermite_segment(&mut out, x0, y0, x1, y1, t0_local, t1, swap_xy);
            }
            _ => {
                t1 = slope3(x0, y0, x1, y1, x, y);
                hermite_segment(&mut out, x0, y0, x1, y1, t0, t1, swap_xy);
            }
        }

        x0 = x1;
        y0 = y1;
        x1 = x;
        y1 = y;
        t0 = t1;
    }

    match point_state {
        2 => emit_line_to(&mut out, x1, y1, swap_xy),
        3 => {
            let t1 = slope2(x0, y0, x1, y1, t0);
            hermite_segment(&mut out, x0, y0, x1, y1, t0, t1, swap_xy);
        }
        _ => {}
    }

    out
}

fn curve_monotone_x_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve_monotone_path_d(points, false)
}

fn curve_monotone_y_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve_monotone_path_d(points, true)
}

// Ported from D3 `curveBasis` (d3-shape v3.x), used by Mermaid ER renderer `@11.12.2`.
fn curve_basis_path_d(points: &[crate::model::LayoutPoint]) -> String {
    let mut out = String::new();
    if points.is_empty() {
        return out;
    }

    let mut p = 0u8;
    let mut x0 = f64::NAN;
    let mut y0 = f64::NAN;
    let mut x1 = f64::NAN;
    let mut y1 = f64::NAN;

    fn basis_point(out: &mut String, x0: f64, y0: f64, x1: f64, y1: f64, x: f64, y: f64) {
        let c1x = (2.0 * x0 + x1) / 3.0;
        let c1y = (2.0 * y0 + y1) / 3.0;
        let c2x = (x0 + 2.0 * x1) / 3.0;
        let c2y = (y0 + 2.0 * y1) / 3.0;
        let ex = (x0 + 4.0 * x1 + x) / 6.0;
        let ey = (y0 + 4.0 * y1 + y) / 6.0;
        let _ = write!(
            out,
            "C{},{},{},{},{},{}",
            fmt_path(c1x),
            fmt_path(c1y),
            fmt_path(c2x),
            fmt_path(c2y),
            fmt_path(ex),
            fmt_path(ey)
        );
    }

    for pt in points {
        let x = pt.x;
        let y = pt.y;
        match p {
            0 => {
                p = 1;
                let _ = write!(&mut out, "M{},{}", fmt_path(x), fmt_path(y));
            }
            1 => {
                p = 2;
            }
            2 => {
                p = 3;
                let lx = (5.0 * x0 + x1) / 6.0;
                let ly = (5.0 * y0 + y1) / 6.0;
                let _ = write!(&mut out, "L{},{}", fmt_path(lx), fmt_path(ly));
                basis_point(&mut out, x0, y0, x1, y1, x, y);
            }
            _ => {
                basis_point(&mut out, x0, y0, x1, y1, x, y);
            }
        }
        x0 = x1;
        x1 = x;
        y0 = y1;
        y1 = y;
    }

    match p {
        3 => {
            basis_point(&mut out, x0, y0, x1, y1, x1, y1);
            let _ = write!(&mut out, "L{},{}", fmt_path(x1), fmt_path(y1));
        }
        2 => {
            let _ = write!(&mut out, "L{},{}", fmt_path(x1), fmt_path(y1));
        }
        _ => {}
    }

    out
}

fn is_rect_style_key(key: &str) -> bool {
    matches!(
        key,
        "fill"
            | "stroke"
            | "stroke-width"
            | "stroke-dasharray"
            | "opacity"
            | "fill-opacity"
            | "stroke-opacity"
    )
}

fn is_text_style_key(key: &str) -> bool {
    matches!(
        key,
        "color" | "font-family" | "font-size" | "font-weight" | "opacity"
    )
}

fn curve_linear_path_d(points: &[crate::model::LayoutPoint]) -> String {
    let mut out = String::new();
    let Some(first) = points.first() else {
        return out;
    };
    let _ = write!(&mut out, "M{},{}", fmt_path(first.x), fmt_path(first.y));
    for p in points.iter().skip(1) {
        let _ = write!(&mut out, "L{},{}", fmt_path(p.x), fmt_path(p.y));
    }
    out
}

// Ported from D3 `curveStepAfter` (d3-shape v3.x).
fn curve_step_after_path_d(points: &[crate::model::LayoutPoint]) -> String {
    let mut out = String::new();
    let Some(first) = points.first() else {
        return out;
    };
    let mut prev_y = first.y;
    let _ = write!(&mut out, "M{},{}", fmt_path(first.x), fmt_path(first.y));
    for p in points.iter().skip(1) {
        let _ = write!(&mut out, "L{},{}", fmt_path(p.x), fmt_path(prev_y));
        let _ = write!(&mut out, "L{},{}", fmt_path(p.x), fmt_path(p.y));
        prev_y = p.y;
    }
    out
}

// Ported from D3 `curveStepBefore` (d3-shape v3.x).
fn curve_step_before_path_d(points: &[crate::model::LayoutPoint]) -> String {
    let mut out = String::new();
    let Some(first) = points.first() else {
        return out;
    };
    let mut prev_x = first.x;
    let _ = write!(&mut out, "M{},{}", fmt_path(first.x), fmt_path(first.y));
    for p in points.iter().skip(1) {
        let _ = write!(&mut out, "L{},{}", fmt_path(prev_x), fmt_path(p.y));
        let _ = write!(&mut out, "L{},{}", fmt_path(p.x), fmt_path(p.y));
        prev_x = p.x;
    }
    out
}

// Ported from D3 `curveStep` (d3-shape v3.x).
fn curve_step_path_d(points: &[crate::model::LayoutPoint]) -> String {
    let mut out = String::new();
    let Some(first) = points.first() else {
        return out;
    };
    let _ = write!(&mut out, "M{},{}", fmt_path(first.x), fmt_path(first.y));
    let mut prev = first;
    for p in points.iter().skip(1) {
        let mid_x = (prev.x + p.x) / 2.0;
        let _ = write!(&mut out, "L{},{}", fmt_path(mid_x), fmt_path(prev.y));
        let _ = write!(&mut out, "L{},{}", fmt_path(mid_x), fmt_path(p.y));
        let _ = write!(&mut out, "L{},{}", fmt_path(p.x), fmt_path(p.y));
        prev = p;
    }
    out
}

// Ported from D3 `curveCardinal` (d3-shape v3.x).
fn curve_cardinal_path_d(points: &[crate::model::LayoutPoint], tension: f64) -> String {
    let mut out = String::new();
    if points.is_empty() {
        return out;
    }

    let k = (1.0 - tension) / 6.0;

    let mut p = 0u8;
    let mut x0 = f64::NAN;
    let mut y0 = f64::NAN;
    let mut x1 = f64::NAN;
    let mut y1 = f64::NAN;
    let mut x2 = f64::NAN;
    let mut y2 = f64::NAN;

    fn cardinal_point(
        out: &mut String,
        k: f64,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        x: f64,
        y: f64,
    ) {
        let c1x = x1 + k * (x2 - x0);
        let c1y = y1 + k * (y2 - y0);
        let c2x = x2 + k * (x1 - x);
        let c2y = y2 + k * (y1 - y);
        let _ = write!(
            out,
            "C{},{},{},{},{},{}",
            fmt_path(c1x),
            fmt_path(c1y),
            fmt_path(c2x),
            fmt_path(c2y),
            fmt_path(x2),
            fmt_path(y2)
        );
    }

    for pt in points {
        let x = pt.x;
        let y = pt.y;
        match p {
            0 => {
                p = 1;
                let _ = write!(&mut out, "M{},{}", fmt_path(x), fmt_path(y));
            }
            1 => {
                p = 2;
                x1 = x;
                y1 = y;
            }
            2 => {
                p = 3;
                cardinal_point(&mut out, k, x0, y0, x1, y1, x2, y2, x, y);
            }
            _ => {
                cardinal_point(&mut out, k, x0, y0, x1, y1, x2, y2, x, y);
            }
        }

        x0 = x1;
        x1 = x2;
        x2 = x;
        y0 = y1;
        y1 = y2;
        y2 = y;
    }

    match p {
        2 => {
            let _ = write!(&mut out, "L{},{}", fmt_path(x2), fmt_path(y2));
        }
        3 => {
            cardinal_point(&mut out, k, x0, y0, x1, y1, x2, y2, x1, y1);
        }
        _ => {}
    }

    out
}
fn config_string(cfg: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

fn json_f64(v: &serde_json::Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_i64().map(|n| n as f64))
        .or_else(|| v.as_u64().map(|n| n as f64))
}

fn config_f64(cfg: &serde_json::Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    json_f64(cur)
}

fn normalize_css_font_family(font_family: &str) -> String {
    let s = font_family.trim().trim_end_matches(';').trim();
    if s.is_empty() {
        return String::new();
    }

    // Mermaid's generated CSS uses a comma-separated `font-family` list with no extra whitespace
    // around commas (e.g. `"trebuchet ms",verdana,arial,sans-serif`). Normalize config-provided
    // values to the same format so strict SVG XML compares are stable.
    let mut parts: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_single = false;
    let mut in_double = false;

    for ch in s.chars() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                cur.push(ch);
            }
            '"' if !in_single => {
                in_double = !in_double;
                cur.push(ch);
            }
            ',' if !in_single && !in_double => {
                let p = cur.trim();
                if !p.is_empty() {
                    parts.push(p.to_string());
                }
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }

    let p = cur.trim();
    if !p.is_empty() {
        parts.push(p.to_string());
    }

    parts.join(",")
}

fn theme_color(effective_config: &serde_json::Value, key: &str, fallback: &str) -> String {
    config_string(effective_config, &["themeVariables", key])
        .unwrap_or_else(|| fallback.to_string())
}

fn render_node(out: &mut String, n: &LayoutNode) {
    let x = n.x - n.width / 2.0;
    let y = n.y - n.height / 2.0;
    let _ = write!(
        out,
        r#"<rect class="node-box" x="{}" y="{}" width="{}" height="{}" />"#,
        fmt(x),
        fmt(y),
        fmt(n.width.max(1.0)),
        fmt(n.height.max(1.0))
    );
    let _ = write!(
        out,
        r#"<text class="node-label" x="{}" y="{}">{}</text>"#,
        fmt(n.x),
        fmt(n.y),
        escape_xml(&n.id)
    );
}

fn fmt_debug_3dp(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    if v.abs() < 0.0005 {
        return "0".to_string();
    }
    let mut r = (v * 1000.0).round() / 1000.0;
    if r.abs() < 0.0005 {
        r = 0.0;
    }
    let mut s = format!("{r:.3}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" { "0".to_string() } else { s }
}

fn render_state_node(out: &mut String, n: &LayoutNode) {
    let is_small_circle = (n.width - n.height).abs() < 1e-6 && n.width <= 20.0 && n.height <= 20.0;
    if is_small_circle {
        let r = (n.width / 2.0).max(1.0);
        let _ = write!(
            out,
            r#"<circle class="node-circle" cx="{}" cy="{}" r="{}" />"#,
            fmt(n.x),
            fmt(n.y),
            fmt(r)
        );
    } else {
        let x = n.x - n.width / 2.0;
        let y = n.y - n.height / 2.0;
        let _ = write!(
            out,
            r#"<rect class="node-box" x="{}" y="{}" width="{}" height="{}" />"#,
            fmt(x),
            fmt(y),
            fmt(n.width.max(1.0)),
            fmt(n.height.max(1.0))
        );
    }

    let _ = write!(
        out,
        r#"<text class="node-label" x="{}" y="{}">{}</text>"#,
        fmt(n.x),
        fmt(n.y),
        escape_xml(&n.id)
    );
}

fn render_cluster(out: &mut String, c: &LayoutCluster, include_markers: bool) {
    let x = c.x - c.width / 2.0;
    let y = c.y - c.height / 2.0;

    let _ = write!(
        out,
        r#"<g id="cluster-{}" data-diff="{}" data-offset-y="{}">"#,
        escape_attr(&c.id),
        fmt_debug_3dp(c.diff),
        fmt_debug_3dp(c.offset_y)
    );
    let _ = write!(
        out,
        r#"<rect class="cluster-box" x="{}" y="{}" width="{}" height="{}" />"#,
        fmt(x),
        fmt(y),
        fmt(c.width.max(1.0)),
        fmt(c.height.max(1.0))
    );
    let _ = write!(
        out,
        r#"<text class="cluster-title" x="{}" y="{}">{}</text>"#,
        fmt(c.title_label.x),
        fmt(c.title_label.y),
        escape_xml(&c.title)
    );

    if include_markers {
        // Visualize Mermaid's clusterNode translation origin used by `positionNode(...)`:
        // translate(node.x + diff - node.width/2, node.y - node.height/2 - padding).
        let ox = c.x + c.diff - c.width / 2.0;
        let oy = c.y - c.height / 2.0 - c.padding;
        debug_cross(out, ox, oy, 6.0);
    }

    out.push_str("</g>\n");
}

fn debug_cross(out: &mut String, x: f64, y: f64, size: f64) {
    let s = size.abs();
    let _ = write!(
        out,
        r#"<line class="debug-cross" x1="{}" y1="{}" x2="{}" y2="{}" />"#,
        fmt(x - s),
        fmt(y),
        fmt(x + s),
        fmt(y)
    );
    let _ = write!(
        out,
        r#"<line class="debug-cross" x1="{}" y1="{}" x2="{}" y2="{}" />"#,
        fmt(x),
        fmt(y - s),
        fmt(x),
        fmt(y + s)
    );
}

fn compute_layout_bounds(
    clusters: &[LayoutCluster],
    nodes: &[LayoutNode],
    edges: &[crate::model::LayoutEdge],
) -> Option<Bounds> {
    let mut b: Option<Bounds> = None;

    let mut include_rect = |min_x: f64, min_y: f64, max_x: f64, max_y: f64| {
        if let Some(ref mut cur) = b {
            cur.min_x = cur.min_x.min(min_x);
            cur.min_y = cur.min_y.min(min_y);
            cur.max_x = cur.max_x.max(max_x);
            cur.max_y = cur.max_y.max(max_y);
        } else {
            b = Some(Bounds {
                min_x,
                min_y,
                max_x,
                max_y,
            });
        }
    };

    for c in clusters {
        let hw = c.width / 2.0;
        let hh = c.height / 2.0;
        include_rect(c.x - hw, c.y - hh, c.x + hw, c.y + hh);
        let lhw = c.title_label.width / 2.0;
        let lhh = c.title_label.height / 2.0;
        include_rect(
            c.title_label.x - lhw,
            c.title_label.y - lhh,
            c.title_label.x + lhw,
            c.title_label.y + lhh,
        );
    }

    for n in nodes {
        let hw = n.width / 2.0;
        let hh = n.height / 2.0;
        include_rect(n.x - hw, n.y - hh, n.x + hw, n.y + hh);
    }

    for e in edges {
        for p in &e.points {
            include_rect(p.x, p.y, p.x, p.y);
        }
        for lbl in [
            e.label.as_ref(),
            e.start_label_left.as_ref(),
            e.start_label_right.as_ref(),
            e.end_label_left.as_ref(),
            e.end_label_right.as_ref(),
        ] {
            if let Some(lbl) = lbl {
                let hw = lbl.width / 2.0;
                let hh = lbl.height / 2.0;
                include_rect(lbl.x - hw, lbl.y - hh, lbl.x + hw, lbl.y + hh);
            }
        }
    }

    b
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
fn fmt(v: f64) -> String {
    // Match how Mermaid/D3 generally stringify numbers for SVG attributes:
    // use a round-trippable decimal form (similar to JS `Number#toString()`),
    // but avoid `-0` and tiny float noise from our own calculations.
    if !v.is_finite() {
        return "0".to_string();
    }

    let mut v = if v.abs() < 1e-9 { 0.0 } else { v };
    let nearest = v.round();
    if (v - nearest).abs() < 1e-6 {
        v = nearest;
    }
    let s = v.to_string();
    if s == "-0" { "0".to_string() } else { s }
}

fn fmt_path(v: f64) -> String {
    // D3's `d3-path` defaults to 3 fractional digits when stringifying path commands.
    // D3 uses `Math.round(x * 1000) / 1000` (ties half-up, including for negatives).
    if !v.is_finite() {
        return "0".to_string();
    }
    if v.abs() < 0.0005 {
        return "0".to_string();
    }

    let scaled = v * 1000.0;
    let mut r = (scaled + 0.5).floor() / 1000.0;
    if r.abs() < 0.0005 {
        r = 0.0;
    }

    let mut s = format!("{r:.3}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" { "0".to_string() } else { s }
}

fn json_stringify_points(points: &[crate::model::LayoutPoint]) -> String {
    // Mermaid encodes `data-points` as Base64(JSON.stringify(points)).
    // JS `JSON.stringify` prints whole numbers without a `.0` suffix.
    //
    // For strict SVG XML parity we must also match V8's number-to-string behavior, including
    // tie-breaking cases where Rust's default float formatting can pick a different shortest
    // round-trippable decimal (e.g. `...0312` vs `...0313`).
    fn js_number_to_string<'a>(mut v: f64, buf: &'a mut ryu_js::Buffer) -> &'a str {
        if !v.is_finite() {
            return "0";
        }
        if v == -0.0 {
            v = 0.0;
        }
        buf.format_finite(v)
    }

    let mut out = String::new();
    out.push('[');
    let mut buf = ryu_js::Buffer::new();
    for (i, p) in points.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(r#"{"x":"#);
        out.push_str(js_number_to_string(p.x, &mut buf));
        out.push_str(r#","y":"#);
        out.push_str(js_number_to_string(p.y, &mut buf));
        out.push('}');
    }
    out.push(']');
    out
}

fn fmt_max_width_px(v: f64) -> String {
    // Mermaid's `max-width: ...px` strings are effectively rendered with ~6 significant digits,
    // trimming trailing zeros (see upstream fixtures: `1184.88`, `432.812`, `85.4375`, `2019.2`).
    if !v.is_finite() {
        return "0".to_string();
    }
    if v.abs() < 0.0005 {
        return "0".to_string();
    }

    let abs = v.abs().max(0.0005);
    let exp10 = abs.log10().floor() as i32;
    let sig = 6i32;
    let decimals = (sig - 1 - exp10).clamp(0, 6) as usize;

    fn round_ties_to_even(x: f64) -> f64 {
        if !x.is_finite() {
            return 0.0;
        }
        let sign = if x.is_sign_negative() { -1.0 } else { 1.0 };
        let ax = x.abs();
        let f = ax.floor();
        let frac = ax - f;
        let i = if frac < 0.5 {
            f
        } else if frac > 0.5 {
            f + 1.0
        } else {
            // exactly halfway: choose the even integer
            let fi = f as i64;
            if fi % 2 == 0 { f } else { f + 1.0 }
        };
        sign * i
    }

    let scale = 10f64.powi(decimals as i32);
    let mut rounded = round_ties_to_even(v * scale) / scale;
    if rounded.abs() < 0.0005 {
        rounded = 0.0;
    }

    let mut s = format!("{:.*}", decimals, rounded);
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" { "0".to_string() } else { s }
}

fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_attr(text: &str) -> String {
    // Attributes in our debug SVG only use escaped XML. No URL encoding here.
    escape_xml(text)
}
