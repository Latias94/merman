use super::pipeline::{ScopedCssPostprocessor, SvgPipeline, SvgPostprocessMetadata};
use crate::environment::{
    RenderSession, RootViewportOverridePolicy, RoutedTextMeasurer, TextMeasurementPhase,
};
use crate::model::{
    ArchitectureDiagramLayout, BlockDiagramLayout, Bounds, ClassDiagramV2Layout,
    CynefinDiagramLayout, ErDiagramLayout, ErrorDiagramLayout, EventModelingDiagramLayout,
    FlowchartV2Layout, InfoDiagramLayout, IshikawaDiagramLayout, LayoutCluster, LayoutNode,
    MindmapDiagramLayout, PacketDiagramLayout, PieDiagramLayout, QuadrantChartDiagramLayout,
    RadarDiagramLayout, RailroadDiagramLayout, RequirementDiagramLayout, SankeyDiagramLayout,
    SequenceDiagramLayout, StateDiagramV2Layout, TimelineDiagramLayout, TreeViewDiagramLayout,
    VennDiagramLayout, XyChartDiagramLayout,
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
mod xychart;
use css::{
    er_css, gantt_css, info_css_parts_with_config, info_css_parts_with_theme_font_size_only,
    info_css_with_config, pie_css, push_xychart_css, requirement_css, sankey_css, treemap_css,
};
use path_bounds::svg_path_bounds_from_d;
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
    SvgTheme, apply_root_viewport_override, config_bool, config_diagram_look, config_f64,
    config_f64_css_px, config_string, css_rgba_fade, decode_mermaid_entities_for_render_text,
    escape_attr, escape_attr_display, escape_attr_into, escape_xml, escape_xml_display,
    escape_xml_into, fmt, fmt_debug_3dp, fmt_display, fmt_into, fmt_max_width_px, fmt_path,
    fmt_path_into, fmt_points, fmt_string, json_stringify_points, json_stringify_points_into,
    normalize_css_font_family, push_points_attr, scoped_svg_id, scoped_svg_url, theme_color,
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
}

impl Default for SvgRenderOptions {
    fn default() -> Self {
        Self {
            viewbox_padding: 8.0,
            diagram_id: None,
            aria_roledescription: None,
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

    pub(crate) const fn unix_ms(&self) -> i64 {
        self.session.time().unix_ms()
    }

    pub(crate) const fn seed(&self) -> u64 {
        self.session.seed().seed().get()
    }

    pub(crate) const fn root_viewport_override_policy(&self) -> RootViewportOverridePolicy {
        self.session.root_viewport_override_policy()
    }

    fn effective_config_value<'b>(
        &self,
        effective_config: &'b serde_json::Value,
    ) -> Cow<'b, serde_json::Value> {
        let configured = effective_config
            .get("handDrawnSeed")
            .and_then(serde_json::Value::as_u64);
        if configured.is_some_and(|seed| seed != 0) {
            return Cow::Borrowed(effective_config);
        }
        let Some(object) = effective_config.as_object() else {
            return Cow::Borrowed(effective_config);
        };
        let mut object = object.clone();
        object.insert(
            "handDrawnSeed".to_string(),
            serde_json::Value::Number(self.seed().into()),
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

impl std::ops::Deref for SvgExecution<'_> {
    type Target = SvgRenderOptions;

    fn deref(&self) -> &Self::Target {
        self.request
    }
}

pub fn render_layouted_svg(
    diagram: &crate::model::LayoutedDiagram,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_layouted_svg_with_debug(diagram, session, options, &SvgDebugOptions::default())
}

pub fn render_layouted_svg_with_debug(
    diagram: &crate::model::LayoutedDiagram,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    let flowchart_roledescription =
        matches!(&diagram.layout, crate::model::LayoutDiagram::FlowchartV2(_))
            .then_some(diagram.meta.diagram_type.as_str());
    let mut scoped_options;
    let options = if options.aria_roledescription.is_none()
        && let Some(roledescription) = flowchart_roledescription
    {
        scoped_options = options.clone();
        scoped_options.aria_roledescription = Some(roledescription.to_string());
        &scoped_options
    } else {
        options
    };
    render_layout_svg_parts_with_debug(
        &diagram.layout,
        &diagram.semantic,
        &diagram.meta.effective_config,
        diagram.meta.title.as_deref(),
        session,
        options,
        debug,
    )
}

pub fn render_layout_svg_parts(
    layout: &crate::model::LayoutDiagram,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_layout_svg_parts_with_debug(
        layout,
        semantic,
        effective_config,
        title,
        session,
        options,
        &SvgDebugOptions::default(),
    )
}

pub fn render_layout_svg_parts_with_debug(
    layout: &crate::model::LayoutDiagram,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    let execution = SvgExecution::new(options, debug, session);
    let svg = render_layout_svg_parts_raw(layout, semantic, effective_config, title, &execution)?;
    apply_theme_css(svg, effective_config, session)
}

fn render_layout_svg_parts_raw(
    layout: &crate::model::LayoutDiagram,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    title: Option<&str>,
    options: &SvgExecution<'_>,
) -> Result<String> {
    let measurer = options.text_measurer();
    let effective_config = options.effective_config_value(effective_config);
    let effective_config = effective_config.as_ref();
    use crate::model::LayoutDiagram;

    match layout {
        LayoutDiagram::ErrorDiagram(layout) => {
            render_error_diagram_svg(layout, semantic, effective_config, options)
        }
        LayoutDiagram::BlockDiagram(layout) => {
            render_block_diagram_svg(layout, semantic, effective_config, options)
        }
        LayoutDiagram::RequirementDiagram(layout) => requirement::render_requirement_diagram_svg(
            layout,
            semantic,
            effective_config,
            title,
            measurer,
            options,
        ),
        #[cfg(feature = "cytoscape-layout")]
        LayoutDiagram::ArchitectureDiagram(layout) => {
            architecture::render_architecture_diagram_svg(
                layout,
                semantic,
                effective_config,
                options,
            )
        }
        #[cfg(not(feature = "cytoscape-layout"))]
        LayoutDiagram::ArchitectureDiagram(_) => Err(Error::UnsupportedDiagram {
            diagram_type: "architecture".to_string(),
        }),
        #[cfg(feature = "cytoscape-layout")]
        LayoutDiagram::MindmapDiagram(layout) => {
            mindmap::render_mindmap_diagram_svg(layout, semantic, effective_config, options)
        }
        #[cfg(not(feature = "cytoscape-layout"))]
        LayoutDiagram::MindmapDiagram(_) => Err(Error::UnsupportedDiagram {
            diagram_type: "mindmap".to_string(),
        }),
        LayoutDiagram::SankeyDiagram(layout) => {
            sankey::render_sankey_diagram_svg(layout, semantic, effective_config, options)
        }
        LayoutDiagram::RadarDiagram(layout) => {
            render_radar_diagram_svg(layout, semantic, effective_config, options)
        }
        LayoutDiagram::TreemapDiagram(layout) => {
            treemap::render_treemap_diagram_svg(layout, semantic, effective_config, options)
        }
        LayoutDiagram::VennDiagram(layout) => {
            render_venn_diagram_svg(layout, semantic, effective_config, title, options)
        }
        LayoutDiagram::XyChartDiagram(layout) => {
            render_xychart_diagram_svg(layout, semantic, effective_config, options)
        }
        LayoutDiagram::QuadrantChartDiagram(layout) => {
            render_quadrantchart_diagram_svg(layout, semantic, effective_config, options)
        }
        LayoutDiagram::FlowchartV2(layout) => flowchart::render_flowchart_v2_svg(
            layout,
            semantic,
            effective_config,
            title,
            measurer,
            options,
        ),
        LayoutDiagram::CynefinDiagram(layout) => {
            cynefin::render_cynefin_diagram_svg(layout, semantic, effective_config, title, options)
        }
        LayoutDiagram::RailroadDiagram(layout) => railroad::render_railroad_diagram_svg(
            layout,
            semantic,
            effective_config,
            measurer,
            options,
        ),
        LayoutDiagram::StateDiagramV2(layout) => state::render_state_diagram_v2_svg(
            layout,
            semantic,
            effective_config,
            title,
            measurer,
            options,
        ),
        LayoutDiagram::ClassDiagramV2(layout) => class::render_class_diagram_v2_svg(
            layout,
            semantic,
            effective_config,
            title,
            measurer,
            options,
        ),
        LayoutDiagram::ErDiagram(layout) => {
            er::render_er_diagram_svg(layout, semantic, effective_config, title, measurer, options)
        }
        LayoutDiagram::SequenceDiagram(layout) => sequence::render_sequence_diagram_svg(
            layout,
            semantic,
            effective_config,
            title,
            measurer,
            options,
        ),
        LayoutDiagram::InfoDiagram(layout) => {
            render_info_diagram_svg(layout, semantic, effective_config, options)
        }
        LayoutDiagram::PacketDiagram(layout) => {
            render_packet_diagram_svg(layout, semantic, effective_config, title, options)
        }
        LayoutDiagram::TimelineDiagram(layout) => timeline::render_timeline_diagram_svg(
            layout,
            semantic,
            effective_config,
            title,
            measurer,
            options,
        ),
        LayoutDiagram::PieDiagram(layout) => {
            pie::render_pie_diagram_svg(layout, semantic, effective_config, options)
        }
        LayoutDiagram::JourneyDiagram(layout) => journey::render_journey_diagram_svg(
            layout,
            semantic,
            effective_config,
            title,
            measurer,
            options,
        ),
        LayoutDiagram::KanbanDiagram(layout) => {
            render_kanban_diagram_svg(layout, semantic, effective_config, options.session, options)
        }
        LayoutDiagram::GitGraphDiagram(layout) => gitgraph::render_gitgraph_diagram_svg(
            layout,
            semantic,
            effective_config,
            title,
            measurer,
            options,
        ),
        LayoutDiagram::GanttDiagram(layout) => {
            gantt::render_gantt_diagram_svg(layout, semantic, effective_config, options)
        }
        LayoutDiagram::TreeViewDiagram(layout) => {
            tree_view::render_tree_view_diagram_svg(layout, semantic, effective_config, options)
        }
        LayoutDiagram::IshikawaDiagram(layout) => {
            render_ishikawa_diagram_svg(layout, semantic, effective_config, options)
        }
        LayoutDiagram::EventModelingDiagram(layout) => {
            eventmodeling::render_eventmodeling_diagram_svg(
                layout,
                semantic,
                effective_config,
                options,
            )
        }
        LayoutDiagram::C4Diagram(layout) => {
            c4::render_c4_diagram_svg(layout, semantic, effective_config, title, measurer, options)
        }
    }
}

pub fn render_layout_svg_parts_with_config(
    layout: &crate::model::LayoutDiagram,
    semantic: &serde_json::Value,
    effective_config: &merman_core::MermaidConfig,
    title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_layout_svg_parts_with_config_and_debug(
        layout,
        semantic,
        effective_config,
        title,
        session,
        options,
        &SvgDebugOptions::default(),
    )
}

pub fn render_layout_svg_parts_with_config_and_debug(
    layout: &crate::model::LayoutDiagram,
    semantic: &serde_json::Value,
    effective_config: &merman_core::MermaidConfig,
    title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    let execution = SvgExecution::new(options, debug, session);
    let svg = render_layout_svg_parts_with_config_raw(
        layout,
        semantic,
        effective_config,
        title,
        &execution,
    )?;
    apply_theme_css(svg, effective_config.as_value(), session)
}

fn render_layout_svg_parts_with_config_raw(
    layout: &crate::model::LayoutDiagram,
    semantic: &serde_json::Value,
    effective_config: &merman_core::MermaidConfig,
    title: Option<&str>,
    options: &SvgExecution<'_>,
) -> Result<String> {
    use crate::model::LayoutDiagram;

    let effective_config = options.effective_config(effective_config);
    let effective_config = effective_config.as_ref();
    let effective_config_value = effective_config.as_value();
    let measurer = options.text_measurer();

    match layout {
        LayoutDiagram::ErrorDiagram(layout) => {
            render_error_diagram_svg(layout, semantic, effective_config_value, options)
        }
        LayoutDiagram::BlockDiagram(layout) => {
            render_block_diagram_svg(layout, semantic, effective_config_value, options)
        }
        LayoutDiagram::RequirementDiagram(layout) => requirement::render_requirement_diagram_svg(
            layout,
            semantic,
            effective_config_value,
            title,
            measurer,
            options,
        ),
        #[cfg(feature = "cytoscape-layout")]
        LayoutDiagram::ArchitectureDiagram(layout) => {
            architecture::render_architecture_diagram_svg_with_config(
                layout,
                semantic,
                effective_config,
                options,
            )
        }
        #[cfg(not(feature = "cytoscape-layout"))]
        LayoutDiagram::ArchitectureDiagram(_) => Err(Error::UnsupportedDiagram {
            diagram_type: "architecture".to_string(),
        }),
        #[cfg(feature = "cytoscape-layout")]
        LayoutDiagram::MindmapDiagram(layout) => mindmap::render_mindmap_diagram_svg_with_config(
            layout,
            semantic,
            effective_config,
            options,
        ),
        #[cfg(not(feature = "cytoscape-layout"))]
        LayoutDiagram::MindmapDiagram(_) => Err(Error::UnsupportedDiagram {
            diagram_type: "mindmap".to_string(),
        }),
        LayoutDiagram::SankeyDiagram(layout) => {
            sankey::render_sankey_diagram_svg(layout, semantic, effective_config_value, options)
        }
        LayoutDiagram::RadarDiagram(layout) => {
            render_radar_diagram_svg(layout, semantic, effective_config_value, options)
        }
        LayoutDiagram::TreemapDiagram(layout) => {
            treemap::render_treemap_diagram_svg(layout, semantic, effective_config_value, options)
        }
        LayoutDiagram::VennDiagram(layout) => {
            render_venn_diagram_svg(layout, semantic, effective_config_value, title, options)
        }
        LayoutDiagram::XyChartDiagram(layout) => {
            render_xychart_diagram_svg(layout, semantic, effective_config_value, options)
        }
        LayoutDiagram::QuadrantChartDiagram(layout) => {
            render_quadrantchart_diagram_svg(layout, semantic, effective_config_value, options)
        }
        LayoutDiagram::FlowchartV2(layout) => flowchart::render_flowchart_v2_svg_with_config(
            layout,
            semantic,
            effective_config,
            title,
            measurer,
            options,
        ),
        LayoutDiagram::CynefinDiagram(layout) => cynefin::render_cynefin_diagram_svg(
            layout,
            semantic,
            effective_config_value,
            title,
            options,
        ),
        LayoutDiagram::RailroadDiagram(layout) => railroad::render_railroad_diagram_svg(
            layout,
            semantic,
            effective_config_value,
            measurer,
            options,
        ),
        LayoutDiagram::StateDiagramV2(layout) => state::render_state_diagram_v2_svg(
            layout,
            semantic,
            effective_config_value,
            title,
            measurer,
            options,
        ),
        LayoutDiagram::ClassDiagramV2(layout) => {
            let model = crate::json::from_value_ref(semantic)?;
            class::render_class_diagram_v2_svg_model_with_config(
                layout,
                &model,
                effective_config,
                title,
                measurer,
                options,
            )
        }
        LayoutDiagram::ErDiagram(layout) => er::render_er_diagram_svg(
            layout,
            semantic,
            effective_config_value,
            title,
            measurer,
            options,
        ),
        LayoutDiagram::SequenceDiagram(layout) => {
            sequence::render_sequence_diagram_svg_with_config(
                layout,
                semantic,
                effective_config,
                title,
                measurer,
                options,
            )
        }
        LayoutDiagram::InfoDiagram(layout) => {
            render_info_diagram_svg(layout, semantic, effective_config_value, options)
        }
        LayoutDiagram::PacketDiagram(layout) => {
            render_packet_diagram_svg(layout, semantic, effective_config_value, title, options)
        }
        LayoutDiagram::TimelineDiagram(layout) => timeline::render_timeline_diagram_svg(
            layout,
            semantic,
            effective_config_value,
            title,
            measurer,
            options,
        ),
        LayoutDiagram::PieDiagram(layout) => {
            pie::render_pie_diagram_svg(layout, semantic, effective_config_value, options)
        }
        LayoutDiagram::JourneyDiagram(layout) => journey::render_journey_diagram_svg(
            layout,
            semantic,
            effective_config_value,
            title,
            measurer,
            options,
        ),
        LayoutDiagram::KanbanDiagram(layout) => render_kanban_diagram_svg(
            layout,
            semantic,
            effective_config_value,
            options.session,
            options,
        ),
        LayoutDiagram::GitGraphDiagram(layout) => gitgraph::render_gitgraph_diagram_svg(
            layout,
            semantic,
            effective_config_value,
            title,
            measurer,
            options,
        ),
        LayoutDiagram::GanttDiagram(layout) => {
            gantt::render_gantt_diagram_svg(layout, semantic, effective_config_value, options)
        }
        LayoutDiagram::TreeViewDiagram(layout) => tree_view::render_tree_view_diagram_svg(
            layout,
            semantic,
            effective_config_value,
            options,
        ),
        LayoutDiagram::IshikawaDiagram(layout) => {
            render_ishikawa_diagram_svg(layout, semantic, effective_config_value, options)
        }
        LayoutDiagram::EventModelingDiagram(layout) => {
            eventmodeling::render_eventmodeling_diagram_svg(
                layout,
                semantic,
                effective_config_value,
                options,
            )
        }
        LayoutDiagram::C4Diagram(layout) => c4::render_c4_diagram_svg(
            layout,
            semantic,
            effective_config_value,
            title,
            measurer,
            options,
        ),
    }
}

pub fn render_layout_svg_parts_for_render_model_with_config(
    layout: &crate::model::LayoutDiagram,
    semantic: &merman_core::RenderSemanticModel,
    effective_config: &merman_core::MermaidConfig,
    title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_layout_svg_parts_for_render_model_with_config_and_debug(
        layout,
        semantic,
        effective_config,
        title,
        session,
        options,
        &SvgDebugOptions::default(),
    )
}

pub fn render_layout_svg_parts_for_render_model_with_config_and_debug(
    layout: &crate::model::LayoutDiagram,
    semantic: &merman_core::RenderSemanticModel,
    effective_config: &merman_core::MermaidConfig,
    title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    let execution = SvgExecution::new(options, debug, session);
    let svg = render_layout_svg_parts_for_render_model_with_config_raw(
        layout,
        semantic,
        effective_config,
        title,
        &execution,
    )?;
    apply_theme_css(svg, effective_config.as_value(), session)
}

pub fn render_layout_svg_parts_for_render_model_with_metadata(
    layout: &crate::model::LayoutDiagram,
    semantic: &merman_core::RenderSemanticModel,
    effective_config: &merman_core::MermaidConfig,
    diagram_type: &str,
    title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_layout_svg_parts_for_render_model_with_metadata_and_debug(
        layout,
        semantic,
        effective_config,
        diagram_type,
        title,
        session,
        options,
        &SvgDebugOptions::default(),
    )
}

pub fn render_layout_svg_parts_for_render_model_with_metadata_and_debug(
    layout: &crate::model::LayoutDiagram,
    semantic: &merman_core::RenderSemanticModel,
    effective_config: &merman_core::MermaidConfig,
    diagram_type: &str,
    title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    let mut scoped_options;
    let options = if options.aria_roledescription.is_none()
        && matches!(layout, crate::model::LayoutDiagram::FlowchartV2(_))
    {
        scoped_options = options.clone();
        scoped_options.aria_roledescription = Some(diagram_type.to_string());
        &scoped_options
    } else {
        options
    };

    let execution = SvgExecution::new(options, debug, session);
    let svg = render_layout_svg_parts_for_render_model_with_config_raw(
        layout,
        semantic,
        effective_config,
        title,
        &execution,
    )?;
    apply_theme_css(svg, effective_config.as_value(), session)
}

fn render_layout_svg_parts_for_render_model_with_config_raw(
    layout: &crate::model::LayoutDiagram,
    semantic: &merman_core::RenderSemanticModel,
    effective_config: &merman_core::MermaidConfig,
    title: Option<&str>,
    options: &SvgExecution<'_>,
) -> Result<String> {
    use crate::model::LayoutDiagram;
    use merman_core::RenderSemanticModel;

    let measurer = options.text_measurer();
    let effective_config = options.effective_config(effective_config);
    let effective_config = effective_config.as_ref();

    match (layout, semantic) {
        #[cfg(feature = "cytoscape-layout")]
        (LayoutDiagram::ArchitectureDiagram(layout), RenderSemanticModel::Architecture(model)) => {
            architecture::render_architecture_diagram_svg_typed_with_config(
                layout,
                model,
                effective_config,
                options,
            )
        }
        #[cfg(not(feature = "cytoscape-layout"))]
        (LayoutDiagram::ArchitectureDiagram(_), RenderSemanticModel::Architecture(_)) => {
            Err(Error::UnsupportedDiagram {
                diagram_type: "architecture".to_string(),
            })
        }
        (LayoutDiagram::FlowchartV2(layout), RenderSemanticModel::Flowchart(model)) => {
            flowchart::render_flowchart_v2_svg_model_with_config(
                layout,
                model,
                effective_config,
                title,
                measurer,
                options,
            )
        }
        (LayoutDiagram::CynefinDiagram(layout), RenderSemanticModel::Cynefin(model)) => {
            cynefin::render_cynefin_diagram_svg_model(
                layout,
                model,
                effective_config.as_value(),
                title,
                options,
            )
        }
        (LayoutDiagram::RailroadDiagram(layout), RenderSemanticModel::Railroad(model)) => {
            railroad::render_railroad_diagram_svg_model(
                layout,
                model,
                effective_config.as_value(),
                measurer,
                options,
            )
        }
        #[cfg(feature = "cytoscape-layout")]
        (LayoutDiagram::MindmapDiagram(layout), RenderSemanticModel::Mindmap(model)) => {
            mindmap::render_mindmap_diagram_svg_model_with_config(
                layout,
                model,
                effective_config,
                options,
            )
        }
        #[cfg(not(feature = "cytoscape-layout"))]
        (LayoutDiagram::MindmapDiagram(_), RenderSemanticModel::Mindmap(_)) => {
            Err(Error::UnsupportedDiagram {
                diagram_type: "mindmap".to_string(),
            })
        }
        (LayoutDiagram::StateDiagramV2(layout), RenderSemanticModel::State(model)) => {
            state::render_state_diagram_v2_svg_model(
                layout,
                model,
                effective_config.as_value(),
                title,
                measurer,
                options,
            )
        }
        (LayoutDiagram::ClassDiagramV2(layout), RenderSemanticModel::Class(model)) => {
            class::render_class_diagram_v2_svg_model_with_config(
                layout,
                model,
                effective_config,
                title,
                measurer,
                options,
            )
        }
        (LayoutDiagram::SequenceDiagram(layout), RenderSemanticModel::Sequence(model)) => {
            sequence::render_sequence_diagram_svg_model_with_config(
                layout,
                model,
                effective_config,
                title,
                measurer,
                options,
            )
        }
        (LayoutDiagram::KanbanDiagram(layout), RenderSemanticModel::Kanban(_)) => {
            render_kanban_diagram_svg(
                layout,
                &serde_json::Value::Null,
                effective_config.as_value(),
                options.session,
                options,
            )
        }
        (LayoutDiagram::GanttDiagram(layout), RenderSemanticModel::Gantt(model)) => {
            gantt::render_gantt_diagram_svg_model(
                layout,
                model,
                effective_config.as_value(),
                options,
            )
        }
        (LayoutDiagram::PieDiagram(layout), RenderSemanticModel::Pie(model)) => {
            pie::render_pie_diagram_svg_model(layout, model, effective_config.as_value(), options)
        }
        (LayoutDiagram::PacketDiagram(layout), RenderSemanticModel::Packet(model)) => {
            packet::render_packet_diagram_svg_model(
                layout,
                model,
                effective_config.as_value(),
                title,
                options,
            )
        }
        (LayoutDiagram::TimelineDiagram(layout), RenderSemanticModel::Timeline(model)) => {
            timeline::render_timeline_diagram_svg_model(
                layout,
                model,
                effective_config.as_value(),
                title,
                measurer,
                options,
            )
        }
        (LayoutDiagram::JourneyDiagram(layout), RenderSemanticModel::Journey(model)) => {
            journey::render_journey_diagram_svg_model(
                layout,
                model,
                effective_config.as_value(),
                title,
                measurer,
                options,
            )
        }
        (LayoutDiagram::RequirementDiagram(layout), RenderSemanticModel::Requirement(model)) => {
            requirement::render_requirement_diagram_svg_model(
                layout,
                model,
                effective_config.as_value(),
                title,
                measurer,
                options,
            )
        }
        (LayoutDiagram::SankeyDiagram(layout), RenderSemanticModel::Sankey(_)) => {
            sankey::render_sankey_diagram_svg(
                layout,
                &serde_json::Value::Null,
                effective_config.as_value(),
                options,
            )
        }
        (LayoutDiagram::RadarDiagram(layout), RenderSemanticModel::Radar(model)) => {
            radar::render_radar_diagram_svg_model(
                layout,
                model,
                effective_config.as_value(),
                options,
            )
        }
        (LayoutDiagram::InfoDiagram(layout), RenderSemanticModel::Info(_)) => {
            render_info_diagram_svg(
                layout,
                &serde_json::Value::Null,
                effective_config.as_value(),
                options,
            )
        }
        (LayoutDiagram::TreemapDiagram(layout), RenderSemanticModel::Treemap(_)) => {
            treemap::render_treemap_diagram_svg(
                layout,
                &serde_json::Value::Null,
                effective_config.as_value(),
                options,
            )
        }
        (LayoutDiagram::VennDiagram(layout), RenderSemanticModel::Venn(model)) => {
            venn::render_venn_diagram_svg_model(
                layout,
                model,
                effective_config.as_value(),
                title,
                options,
            )
        }
        (LayoutDiagram::BlockDiagram(layout), RenderSemanticModel::Block(model)) => {
            render_block_diagram_svg_model(layout, model, effective_config.as_value(), options)
        }
        (LayoutDiagram::ErDiagram(layout), RenderSemanticModel::Er(model)) => {
            er::render_er_diagram_svg_model(
                layout,
                model,
                effective_config.as_value(),
                title,
                measurer,
                options,
            )
        }
        (LayoutDiagram::QuadrantChartDiagram(layout), RenderSemanticModel::QuadrantChart(_)) => {
            render_quadrantchart_diagram_svg(
                layout,
                &serde_json::Value::Null,
                effective_config.as_value(),
                options,
            )
        }
        (LayoutDiagram::XyChartDiagram(layout), RenderSemanticModel::XyChart(_)) => {
            render_xychart_diagram_svg(
                layout,
                &serde_json::Value::Null,
                effective_config.as_value(),
                options,
            )
        }
        (LayoutDiagram::GitGraphDiagram(layout), RenderSemanticModel::GitGraph(model)) => {
            gitgraph::render_gitgraph_diagram_svg_model(
                layout,
                model,
                effective_config.as_value(),
                title,
                measurer,
                options,
            )
        }
        (LayoutDiagram::TreeViewDiagram(layout), RenderSemanticModel::TreeView(model)) => {
            tree_view::render_tree_view_diagram_svg_model(
                layout,
                model,
                effective_config.as_value(),
                options,
            )
        }
        (LayoutDiagram::IshikawaDiagram(layout), RenderSemanticModel::Ishikawa(_)) => {
            render_ishikawa_diagram_svg(
                layout,
                &serde_json::Value::Null,
                effective_config.as_value(),
                options,
            )
        }
        (LayoutDiagram::EventModelingDiagram(layout), RenderSemanticModel::EventModeling(_)) => {
            eventmodeling::render_eventmodeling_diagram_svg(
                layout,
                &serde_json::Value::Null,
                effective_config.as_value(),
                options,
            )
        }
        (LayoutDiagram::C4Diagram(layout), RenderSemanticModel::C4(model)) => {
            c4::render_c4_diagram_svg_typed(
                layout,
                model,
                effective_config.as_value(),
                title,
                measurer,
                options,
            )
        }
        (_, RenderSemanticModel::Json(semantic)) => render_layout_svg_parts_with_config_raw(
            layout,
            semantic,
            effective_config,
            title,
            options,
        ),
        _ => Err(Error::InvalidModel {
            message: "semantic model does not match layout diagram type".to_string(),
        }),
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

pub fn render_flowchart_v2_debug_svg(
    layout: &FlowchartV2Layout,
    options: &SvgRenderOptions,
) -> String {
    render_flowchart_v2_debug_svg_with_debug(layout, options, &SvgDebugOptions::default())
}

pub fn render_flowchart_v2_debug_svg_with_debug(
    layout: &FlowchartV2Layout,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> String {
    flowchart::render_flowchart_v2_debug_svg(layout, options, debug)
}

pub fn render_sequence_diagram_debug_svg(
    layout: &SequenceDiagramLayout,
    options: &SvgRenderOptions,
) -> String {
    render_sequence_diagram_debug_svg_with_debug(layout, options, &SvgDebugOptions::default())
}

pub fn render_sequence_diagram_debug_svg_with_debug(
    layout: &SequenceDiagramLayout,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> String {
    sequence::render_sequence_diagram_debug_svg(layout, options, debug)
}

pub fn render_sequence_diagram_svg(
    layout: &SequenceDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_sequence_diagram_svg_with_debug(
        layout,
        semantic,
        effective_config,
        diagram_title,
        session,
        options,
        &SvgDebugOptions::default(),
    )
}

pub fn render_sequence_diagram_svg_with_debug(
    layout: &SequenceDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    let execution = SvgExecution::new(options, debug, session);
    let measurer = execution.text_measurer();
    sequence::render_sequence_diagram_svg(
        layout,
        semantic,
        effective_config,
        diagram_title,
        measurer,
        &execution,
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
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    let debug = SvgDebugOptions::default();
    let execution = SvgExecution::new(options, &debug, session);
    pie::render_pie_diagram_svg(layout, semantic, _effective_config, &execution)
}

pub fn render_cynefin_diagram_svg(
    layout: &CynefinDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    cynefin::render_cynefin_diagram_svg(layout, semantic, effective_config, None, options)
}

pub fn render_railroad_diagram_svg(
    layout: &RailroadDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    let measurer = session.text_measurer(TextMeasurementPhase::SvgBBox);
    railroad::render_railroad_diagram_svg(layout, semantic, effective_config, &measurer, options)
}

pub fn render_requirement_diagram_svg(
    layout: &RequirementDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    let measurer = session.text_measurer(TextMeasurementPhase::SvgBBox);
    requirement::render_requirement_diagram_svg(
        layout,
        semantic,
        effective_config,
        diagram_title,
        &measurer,
        options,
    )
}

pub fn render_block_diagram_svg(
    layout: &BlockDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    block::render_block_diagram_svg(layout, semantic, effective_config, options)
}

pub fn render_block_diagram_svg_model(
    layout: &BlockDiagramLayout,
    model: &merman_core::diagrams::block::BlockDiagramRenderModel,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    block::render_block_diagram_svg_model(layout, model, effective_config, options)
}

pub fn render_er_diagram_svg_model(
    layout: &ErDiagramLayout,
    model: &merman_core::diagrams::er::ErDiagramRenderModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_er_diagram_svg_model_with_debug(
        layout,
        model,
        effective_config,
        diagram_title,
        session,
        options,
        &SvgDebugOptions::default(),
    )
}

pub fn render_er_diagram_svg_model_with_debug(
    layout: &ErDiagramLayout,
    model: &merman_core::diagrams::er::ErDiagramRenderModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    let execution = SvgExecution::new(options, debug, session);
    let measurer = execution.text_measurer();
    er::render_er_diagram_svg_model(
        layout,
        model,
        effective_config,
        diagram_title,
        measurer,
        &execution,
    )
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
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_treemap_diagram_svg_with_debug(
        layout,
        _semantic,
        effective_config,
        session,
        options,
        &SvgDebugOptions::default(),
    )
}

pub fn render_treemap_diagram_svg_with_debug(
    layout: &crate::model::TreemapDiagramLayout,
    _semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    let execution = SvgExecution::new(options, debug, session);
    treemap::render_treemap_diagram_svg(layout, _semantic, effective_config, &execution)
}

pub fn render_venn_diagram_svg(
    layout: &VennDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    options: &SvgRenderOptions,
) -> Result<String> {
    venn::render_venn_diagram_svg(layout, semantic, effective_config, diagram_title, options)
}

pub fn render_venn_diagram_svg_model(
    layout: &VennDiagramLayout,
    model: &merman_core::diagrams::venn::VennDiagramRenderModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    options: &SvgRenderOptions,
) -> Result<String> {
    venn::render_venn_diagram_svg_model(layout, model, effective_config, diagram_title, options)
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
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    let debug = SvgDebugOptions::default();
    let execution = SvgExecution::new(options, &debug, session);
    let measurer = execution.text_measurer();
    timeline::render_timeline_diagram_svg(
        layout,
        semantic,
        effective_config,
        _diagram_title,
        measurer,
        &execution,
    )
}

pub fn render_journey_diagram_svg(
    layout: &crate::model::JourneyDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    _diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    let measurer = session.text_measurer(TextMeasurementPhase::SvgBBox);
    journey::render_journey_diagram_svg(
        layout,
        semantic,
        effective_config,
        _diagram_title,
        &measurer,
        options,
    )
}

pub fn render_kanban_diagram_svg(
    layout: &crate::model::KanbanDiagramLayout,
    _semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
    kanban::render_kanban_diagram_svg(layout, _semantic, _effective_config, &measurer, options)
}

pub fn render_gitgraph_diagram_svg(
    layout: &crate::model::GitGraphDiagramLayout,
    semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    let measurer = session.text_measurer(TextMeasurementPhase::SvgBBox);
    gitgraph::render_gitgraph_diagram_svg(
        layout,
        semantic,
        _effective_config,
        diagram_title,
        &measurer,
        options,
    )
}

pub fn render_tree_view_diagram_svg(
    layout: &TreeViewDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_tree_view_diagram_svg_with_debug(
        layout,
        semantic,
        effective_config,
        session,
        options,
        &SvgDebugOptions::default(),
    )
}

pub fn render_tree_view_diagram_svg_with_debug(
    layout: &TreeViewDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    let execution = SvgExecution::new(options, debug, session);
    tree_view::render_tree_view_diagram_svg(layout, semantic, effective_config, &execution)
}

pub fn render_ishikawa_diagram_svg(
    layout: &IshikawaDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    ishikawa::render_ishikawa_diagram_svg(layout, semantic, effective_config, options)
}

pub fn render_eventmodeling_diagram_svg(
    layout: &EventModelingDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    let debug = SvgDebugOptions::default();
    let execution = SvgExecution::new(options, &debug, session);
    eventmodeling::render_eventmodeling_diagram_svg(layout, semantic, effective_config, &execution)
}

pub fn render_gantt_diagram_svg(
    layout: &crate::model::GanttDiagramLayout,
    semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_gantt_diagram_svg_with_debug(
        layout,
        semantic,
        _effective_config,
        session,
        options,
        &SvgDebugOptions::default(),
    )
}

pub fn render_gantt_diagram_svg_with_debug(
    layout: &crate::model::GanttDiagramLayout,
    semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    let execution = SvgExecution::new(options, debug, session);
    gantt::render_gantt_diagram_svg(layout, semantic, _effective_config, &execution)
}

pub fn render_mindmap_diagram_svg(
    layout: &MindmapDiagramLayout,
    semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    #[cfg(feature = "cytoscape-layout")]
    {
        let debug = SvgDebugOptions::default();
        let execution = SvgExecution::new(options, &debug, session);
        mindmap::render_mindmap_diagram_svg(layout, semantic, _effective_config, &execution)
    }
    #[cfg(not(feature = "cytoscape-layout"))]
    {
        let _ = (layout, semantic, _effective_config, session, options);
        Err(Error::UnsupportedDiagram {
            diagram_type: "mindmap".to_string(),
        })
    }
}

pub fn render_mindmap_diagram_svg_with_config(
    layout: &MindmapDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &merman_core::MermaidConfig,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    #[cfg(feature = "cytoscape-layout")]
    {
        let debug = SvgDebugOptions::default();
        let execution = SvgExecution::new(options, &debug, session);
        mindmap::render_mindmap_diagram_svg_with_config(
            layout,
            semantic,
            effective_config,
            &execution,
        )
    }
    #[cfg(not(feature = "cytoscape-layout"))]
    {
        let _ = (layout, semantic, effective_config, session, options);
        Err(Error::UnsupportedDiagram {
            diagram_type: "mindmap".to_string(),
        })
    }
}

pub fn render_architecture_diagram_svg(
    layout: &ArchitectureDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_architecture_diagram_svg_with_debug(
        layout,
        semantic,
        effective_config,
        session,
        options,
        &SvgDebugOptions::default(),
    )
}

pub fn render_architecture_diagram_svg_with_debug(
    layout: &ArchitectureDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    #[cfg(feature = "cytoscape-layout")]
    {
        let execution = SvgExecution::new(options, debug, session);
        architecture::render_architecture_diagram_svg(
            layout,
            semantic,
            effective_config,
            &execution,
        )
    }
    #[cfg(not(feature = "cytoscape-layout"))]
    {
        let _ = (layout, semantic, effective_config, session, options, debug);
        Err(Error::UnsupportedDiagram {
            diagram_type: "architecture".to_string(),
        })
    }
}

pub fn render_c4_diagram_svg(
    layout: &crate::model::C4DiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    let debug = SvgDebugOptions::default();
    let execution = SvgExecution::new(options, &debug, session);
    let measurer = execution.text_measurer();
    c4::render_c4_diagram_svg(
        layout,
        semantic,
        effective_config,
        diagram_title,
        measurer,
        &execution,
    )
}

pub fn render_flowchart_v2_svg(
    layout: &FlowchartV2Layout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_flowchart_v2_svg_with_debug(
        layout,
        semantic,
        effective_config,
        diagram_title,
        session,
        options,
        &SvgDebugOptions::default(),
    )
}

pub fn render_flowchart_v2_svg_with_debug(
    layout: &FlowchartV2Layout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    let execution = SvgExecution::new(options, debug, session);
    let measurer = execution.text_measurer();
    flowchart::render_flowchart_v2_svg(
        layout,
        semantic,
        effective_config,
        diagram_title,
        measurer,
        &execution,
    )
}

pub fn render_flowchart_v2_svg_with_config(
    layout: &FlowchartV2Layout,
    semantic: &serde_json::Value,
    effective_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_flowchart_v2_svg_with_config_and_debug(
        layout,
        semantic,
        effective_config,
        diagram_title,
        session,
        options,
        &SvgDebugOptions::default(),
    )
}

pub fn render_flowchart_v2_svg_with_config_and_debug(
    layout: &FlowchartV2Layout,
    semantic: &serde_json::Value,
    effective_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    let execution = SvgExecution::new(options, debug, session);
    let measurer = execution.text_measurer();
    flowchart::render_flowchart_v2_svg_with_config(
        layout,
        semantic,
        effective_config,
        diagram_title,
        measurer,
        &execution,
    )
}

pub fn render_flowchart_v2_svg_model_with_config(
    layout: &FlowchartV2Layout,
    model: &merman_core::diagrams::flowchart::FlowchartV2Model,
    effective_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_flowchart_v2_svg_model_with_config_and_debug(
        layout,
        model,
        effective_config,
        diagram_title,
        session,
        options,
        &SvgDebugOptions::default(),
    )
}

pub fn render_flowchart_v2_svg_model_with_config_and_debug(
    layout: &FlowchartV2Layout,
    model: &merman_core::diagrams::flowchart::FlowchartV2Model,
    effective_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    let execution = SvgExecution::new(options, debug, session);
    let measurer = execution.text_measurer();
    flowchart::render_flowchart_v2_svg_model_with_config(
        layout,
        model,
        effective_config,
        diagram_title,
        measurer,
        &execution,
    )
}

pub fn render_state_diagram_v2_svg(
    layout: &StateDiagramV2Layout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_state_diagram_v2_svg_with_debug(
        layout,
        semantic,
        effective_config,
        diagram_title,
        session,
        options,
        &SvgDebugOptions::default(),
    )
}

pub fn render_state_diagram_v2_svg_with_debug(
    layout: &StateDiagramV2Layout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    let execution = SvgExecution::new(options, debug, session);
    let measurer = execution.text_measurer();
    state::render_state_diagram_v2_svg(
        layout,
        semantic,
        effective_config,
        diagram_title,
        measurer,
        &execution,
    )
}

pub fn render_state_diagram_v2_debug_svg(
    layout: &StateDiagramV2Layout,
    options: &SvgRenderOptions,
) -> String {
    render_state_diagram_v2_debug_svg_with_debug(layout, options, &SvgDebugOptions::default())
}

pub fn render_state_diagram_v2_debug_svg_with_debug(
    layout: &StateDiagramV2Layout,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> String {
    state::render_state_diagram_v2_debug_svg(layout, options, debug)
}

pub fn render_class_diagram_v2_debug_svg(
    layout: &ClassDiagramV2Layout,
    options: &SvgRenderOptions,
) -> String {
    render_class_diagram_v2_debug_svg_with_debug(layout, options, &SvgDebugOptions::default())
}

pub fn render_class_diagram_v2_debug_svg_with_debug(
    layout: &ClassDiagramV2Layout,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> String {
    class::render_class_diagram_v2_debug_svg(layout, options, debug)
}

pub fn render_class_diagram_v2_svg(
    layout: &ClassDiagramV2Layout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    let debug = SvgDebugOptions::default();
    let execution = SvgExecution::new(options, &debug, session);
    class::render_class_diagram_v2_svg(
        layout,
        semantic,
        effective_config,
        diagram_title,
        execution.text_measurer(),
        &execution,
    )
}

pub fn render_er_diagram_debug_svg(layout: &ErDiagramLayout, options: &SvgRenderOptions) -> String {
    render_er_diagram_debug_svg_with_debug(layout, options, &SvgDebugOptions::default())
}

pub fn render_er_diagram_debug_svg_with_debug(
    layout: &ErDiagramLayout,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> String {
    er::render_er_diagram_debug_svg(layout, options, debug)
}

pub fn render_er_diagram_svg(
    layout: &ErDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_er_diagram_svg_with_debug(
        layout,
        semantic,
        effective_config,
        diagram_title,
        session,
        options,
        &SvgDebugOptions::default(),
    )
}

pub fn render_er_diagram_svg_with_debug(
    layout: &ErDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    session: &RenderSession,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<String> {
    let execution = SvgExecution::new(options, debug, session);
    let measurer = execution.text_measurer();
    er::render_er_diagram_svg(
        layout,
        semantic,
        effective_config,
        diagram_title,
        measurer,
        &execution,
    )
}

pub fn render_sankey_diagram_svg(
    layout: &SankeyDiagramLayout,
    _semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    session: &RenderSession,
    options: &SvgRenderOptions,
) -> Result<String> {
    let debug = SvgDebugOptions::default();
    let execution = SvgExecution::new(options, &debug, session);
    sankey::render_sankey_diagram_svg(layout, _semantic, effective_config, &execution)
}

// Ported from D3 `curveBasis` (d3-shape v3.x), used by Mermaid ER renderer `@11.12.2`.
fn curve_basis_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve::curve_basis_path_d(points)
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
