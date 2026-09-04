//! Canonical renderer output shared by non-SVG targets and SVG-specific adapters.
//!
//! The public [`merman_display_list`] document is intentionally only one projection of this
//! internal value. SVG-only structure stays beside it so a native host never has to understand
//! DOM details, while the SVG serializer can continue to preserve Mermaid's source-backed shape.

#[cfg(feature = "layout-cytoscape")]
mod architecture;
mod block;
mod c4;
mod class;
mod cynefin;
mod er;
mod eventmodeling;
mod flowchart;
mod gantt;
mod gitgraph;
mod info;
mod ishikawa;
mod journey;
mod kanban;
mod mindmap;
mod packet;
mod pie;
mod quadrantchart;
mod radar;
mod railroad;
mod requirement;
mod sankey;
mod sequence;
mod state;
mod support;
mod timeline;
mod tree_view;
mod treemap;
mod venn;
mod wardley;
mod xychart;
mod zenuml;

use crate::environment::RenderSession;
use crate::family::{BuiltinFamilyArtifact, RenderFamilyKind};
use crate::model::ErrorDiagramLayout;
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::theme_color::{ColorChannel, ThemeColor};
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, MeasurementProvenance,
    Paint, PathResource, PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation,
    SemanticRole, TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun, TextStyle,
    Viewport,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use svgtypes::{PathParser, PathSegment as SvgPathSegment};

#[cfg(feature = "layout-cytoscape")]
use architecture::ArchitectureSvgBody;
use flowchart::{FlowchartSvgBody, build_flowchart_document, build_swimlane_document};
use zenuml::ZenumlSvgBody;

/// The private SVG projection kept beside the public renderer-neutral document.
#[derive(Debug, Clone)]
pub(crate) struct SvgStructureSidecar {
    pub(crate) family: RenderFamilyKind,
    pub(crate) body: SvgStructureBody,
}

impl SvgStructureSidecar {
    /// Returns the stable Mermaid-like diagram role used by the SVG root.
    pub(crate) fn diagram_type(&self) -> &'static str {
        self.family.as_str()
    }
}

#[derive(Debug, Clone)]
pub(crate) enum SvgStructureBody {
    Error(ErrorSvgBody),
    Flowchart(FlowchartSvgBody),
    Swimlane(SwimlaneSvgBody),
    Class(ClassSvgBody),
    C4(C4SvgBody),
    Er(ErSvgBody),
    Info(InfoSvgBody),
    Ishikawa(IshikawaSvgBody),
    Journey(JourneySvgBody),
    Kanban(KanbanSvgBody),
    Mindmap(MindmapSvgBody),
    Packet(PacketSvgBody),
    Pie(PieSvgBody),
    QuadrantChart(QuadrantChartSvgBody),
    Radar(RadarSvgBody),
    Railroad(RailroadSvgBody),
    Requirement(RequirementSvgBody),
    Sankey(SankeySvgBody),
    Sequence(SequenceSvgBody),
    State(StateSvgBody),
    Treemap(TreemapSvgBody),
    TreeView(TreeViewSvgBody),
    Venn(VennSvgBody),
    XyChart(XyChartSvgBody),
    EventModeling(EventModelingSvgBody),
    Cynefin(CynefinSvgBody),
    Gantt(GanttSvgBody),
    GitGraph(GitGraphSvgBody),
    Timeline(TimelineSvgBody),
    Wardley(WardleySvgBody),
    Block(BlockSvgBody),
    #[cfg(feature = "layout-cytoscape")]
    Architecture(ArchitectureSvgBody),
    Zenuml(ZenumlSvgBody),
}

/// A complete canonical render document.
#[derive(Debug, Clone)]
pub(crate) struct RenderDocument {
    pub(crate) public: DrawingListDocument,
    pub(crate) svg: SvgStructureSidecar,
}

impl RenderDocument {
    pub(crate) fn into_public(self) -> DrawingListDocument {
        let SvgStructureSidecar { family, body } = &self.svg;
        debug_assert!(match (family, body) {
            (RenderFamilyKind::Error, SvgStructureBody::Error(error)) => {
                !error.version_text.is_empty()
            }
            (RenderFamilyKind::Flowchart, SvgStructureBody::Flowchart(flowchart)) => {
                !flowchart.diagram_type.is_empty()
            }
            (RenderFamilyKind::Swimlane, SvgStructureBody::Swimlane(swimlane)) => {
                !swimlane.diagram_type.is_empty()
            }
            (RenderFamilyKind::Class, SvgStructureBody::Class(class)) => {
                !class.diagram_type.is_empty()
            }
            (RenderFamilyKind::C4, SvgStructureBody::C4(c4)) => {
                !c4.diagram_type.is_empty()
            }
            (RenderFamilyKind::Er, SvgStructureBody::Er(er)) => {
                !er.diagram_type.is_empty()
            }
            (RenderFamilyKind::Info, SvgStructureBody::Info(info)) => {
                !info.version.is_empty()
            }
            (RenderFamilyKind::Ishikawa, SvgStructureBody::Ishikawa(ishikawa)) => {
                !ishikawa.diagram_type.is_empty()
            }
            (RenderFamilyKind::Journey, SvgStructureBody::Journey(journey)) => {
                !journey.diagram_type.is_empty()
            }
            (RenderFamilyKind::Kanban, SvgStructureBody::Kanban(kanban)) => {
                !kanban.diagram_type.is_empty()
            }
            (RenderFamilyKind::Mindmap, SvgStructureBody::Mindmap(mindmap)) => {
                !mindmap.diagram_type.is_empty()
            }
            (RenderFamilyKind::Packet, SvgStructureBody::Packet(packet)) => {
                !packet.diagram_type.is_empty()
            }
            (RenderFamilyKind::Pie, SvgStructureBody::Pie(pie)) => !pie.diagram_type.is_empty(),
            (RenderFamilyKind::QuadrantChart, SvgStructureBody::QuadrantChart(quadrantchart)) =>
                !quadrantchart.diagram_type.is_empty(),
            (RenderFamilyKind::Radar, SvgStructureBody::Radar(radar)) => {
                !radar.diagram_type.is_empty()
            }
            (RenderFamilyKind::Railroad, SvgStructureBody::Railroad(railroad)) => {
                !railroad.diagram_type.is_empty()
            }
            (RenderFamilyKind::Requirement, SvgStructureBody::Requirement(requirement)) => {
                !requirement.diagram_type.is_empty()
            }
            (RenderFamilyKind::Sankey, SvgStructureBody::Sankey(sankey)) => {
                !sankey.diagram_type.is_empty()
            }
            (RenderFamilyKind::Sequence, SvgStructureBody::Sequence(sequence)) => {
                !sequence.diagram_type.is_empty()
            }
            (RenderFamilyKind::State, SvgStructureBody::State(state)) => {
                !state.diagram_type.is_empty()
            }
            (RenderFamilyKind::Treemap, SvgStructureBody::Treemap(treemap)) => {
                !treemap.diagram_type.is_empty()
            }
            (RenderFamilyKind::TreeView, SvgStructureBody::TreeView(tree_view)) => {
                !tree_view.diagram_type.is_empty()
            }
            (RenderFamilyKind::Venn, SvgStructureBody::Venn(venn)) => {
                !venn.diagram_type.is_empty()
            }
            (RenderFamilyKind::XyChart, SvgStructureBody::XyChart(xychart)) => {
                !xychart.diagram_type.is_empty()
            }
            (RenderFamilyKind::EventModeling, SvgStructureBody::EventModeling(eventmodeling)) => {
                !eventmodeling.diagram_type.is_empty()
            }
            (RenderFamilyKind::Cynefin, SvgStructureBody::Cynefin(cynefin)) => {
                !cynefin.diagram_type.is_empty()
            }
            (RenderFamilyKind::Gantt, SvgStructureBody::Gantt(gantt)) => {
                !gantt.diagram_type.is_empty()
            }
            (RenderFamilyKind::GitGraph, SvgStructureBody::GitGraph(gitgraph)) => {
                !gitgraph.diagram_type.is_empty()
            }
            (RenderFamilyKind::Timeline, SvgStructureBody::Timeline(timeline)) => {
                !timeline.diagram_type.is_empty()
            }
            (RenderFamilyKind::Wardley, SvgStructureBody::Wardley(wardley)) => {
                !wardley.diagram_type.is_empty()
            }
            (RenderFamilyKind::Block, SvgStructureBody::Block(block)) => {
                !block.diagram_type.is_empty()
            }
            #[cfg(feature = "layout-cytoscape")]
            (RenderFamilyKind::Architecture, SvgStructureBody::Architecture(architecture)) => {
                !architecture.diagram_type.is_empty()
            }
            (RenderFamilyKind::Zenuml, SvgStructureBody::Zenuml(zenuml)) => {
                !zenuml.diagram_type.is_empty()
            }
            _ => false,
        });
        self.public
    }
}

/// Source-backed body data for Mermaid's suppressed-error SVG.
///
/// The path data remains in the SVG sidecar so the parity serializer can preserve upstream
/// spelling, while the same path data is converted into typed public geometry for native hosts.
#[derive(Debug, Clone)]
pub(crate) struct ErrorSvgBody {
    pub(crate) version_text: String,
    pub(crate) max_width_px: f64,
}

/// SVG-only metadata retained beside the renderer-neutral Mindmap document.
#[derive(Debug, Clone)]
pub(crate) struct MindmapSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the public Info document.
#[derive(Debug, Clone)]
pub(crate) struct InfoSvgBody {
    pub(crate) version: String,
}

/// SVG-only metadata retained beside the public Ishikawa document.
#[derive(Debug, Clone)]
pub(crate) struct IshikawaSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the renderer-neutral Journey document.
#[derive(Debug, Clone)]
pub(crate) struct JourneySvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the renderer-neutral Kanban document.
#[derive(Debug, Clone)]
pub(crate) struct KanbanSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the public Pie document.
#[derive(Debug, Clone)]
pub(crate) struct PieSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the public Packet document.
#[derive(Debug, Clone)]
pub(crate) struct PacketSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the public QuadrantChart document.
#[derive(Debug, Clone)]
pub(crate) struct QuadrantChartSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the public Radar document.
#[derive(Debug, Clone)]
pub(crate) struct RadarSvgBody {
    pub(crate) diagram_type: String,
    pub(crate) use_max_width: bool,
}

/// SVG-only metadata retained beside the public Railroad document.
#[derive(Debug, Clone)]
pub(crate) struct RailroadSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the renderer-neutral Requirement document.
#[derive(Debug, Clone)]
pub(crate) struct RequirementSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the public Sankey document.
#[derive(Debug, Clone)]
pub(crate) struct SankeySvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the renderer-neutral Sequence document.
#[derive(Debug, Clone)]
pub(crate) struct SequenceSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the public State document.
#[derive(Debug, Clone)]
pub(crate) struct StateSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the public Treemap document.
#[derive(Debug, Clone)]
pub(crate) struct TreemapSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the public TreeView document.
#[derive(Debug, Clone)]
pub(crate) struct TreeViewSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the public Venn document.
#[derive(Debug, Clone)]
pub(crate) struct VennSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the public XYChart document.
#[derive(Debug, Clone)]
pub(crate) struct XyChartSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the renderer-neutral EventModeling document.
#[derive(Debug, Clone)]
pub(crate) struct EventModelingSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the renderer-neutral Cynefin document.
#[derive(Debug, Clone)]
pub(crate) struct CynefinSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the renderer-neutral Gantt document.
#[derive(Debug, Clone)]
pub(crate) struct GanttSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the renderer-neutral GitGraph document.
#[derive(Debug, Clone)]
pub(crate) struct GitGraphSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the renderer-neutral Timeline document.
#[derive(Debug, Clone)]
pub(crate) struct TimelineSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the renderer-neutral Wardley document.
#[derive(Debug, Clone)]
pub(crate) struct WardleySvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the renderer-neutral Block document.
#[derive(Debug, Clone)]
pub(crate) struct BlockSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the public renderer-neutral Swimlane document.
#[derive(Debug, Clone)]
pub(crate) struct SwimlaneSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the renderer-neutral Class document.
#[derive(Debug, Clone)]
pub(crate) struct ClassSvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the renderer-neutral C4 document.
#[derive(Debug, Clone)]
pub(crate) struct C4SvgBody {
    pub(crate) diagram_type: String,
}

/// SVG-only metadata retained beside the renderer-neutral ER document.
#[derive(Debug, Clone)]
pub(crate) struct ErSvgBody {
    pub(crate) diagram_type: String,
}

pub(crate) const ERROR_ICON_PATHS: [&str; 6] = [
    "m411.313,123.313c6.25-6.25 6.25-16.375 0-22.625s-16.375-6.25-22.625,0l-32,32-9.375,9.375-20.688-20.688c-12.484-12.5-32.766-12.5-45.25,0l-16,16c-1.261,1.261-2.304,2.648-3.31,4.051-21.739-8.561-45.324-13.426-70.065-13.426-105.867,0-192,86.133-192,192s86.133,192 192,192 192-86.133 192-192c0-24.741-4.864-48.327-13.426-70.065 1.402-1.007 2.79-2.049 4.051-3.31l16-16c12.5-12.492 12.5-32.758 0-45.25l-20.688-20.688 9.375-9.375 32.001-31.999zm-219.313,100.687c-52.938,0-96,43.063-96,96 0,8.836-7.164,16-16,16s-16-7.164-16-16c0-70.578 57.422-128 128-128 8.836,0 16,7.164,16,16s-7.164,16-16,16z",
    "m459.02,148.98c-6.25-6.25-16.375-6.25-22.625,0s-6.25,16.375 0,22.625l16,16c3.125,3.125 7.219,4.688 11.313,4.688 4.094,0 8.188-1.563 11.313-4.688 6.25-6.25 6.25-16.375 0-22.625l-16.001-16z",
    "m340.395,75.605c3.125,3.125 7.219,4.688 11.313,4.688 4.094,0 8.188-1.563 11.313-4.688 6.25-6.25 6.25-16.375 0-22.625l-16-16c-6.25-6.25-16.375-6.25-22.625,0s-6.25,16.375 0,22.625l15.999,16z",
    "m400,64c8.844,0 16-7.164 16-16v-32c0-8.836-7.156-16-16-16-8.844,0-16,7.164-16,16v32c0,8.836 7.156,16 16,16z",
    "m496,96.586h-32c-8.844,0-16,7.164-16,16 0,8.836 7.156,16 16,16h32c8.844,0 16-7.164 16-16 0-8.836-7.156-16-16-16z",
    "m436.98,75.605c3.125,3.125 7.219,4.688 11.313,4.688 4.094,0 8.188-1.563 11.313-4.688l32-32c6.25-6.25 6.25-16.375 0-22.625s-16.375-6.25-22.625,0l-32,32c-6.251,6.25-6.251,16.375-0.001,22.625z",
];

impl ErrorSvgBody {
    pub(crate) fn new() -> Self {
        Self {
            version_text: format!("mermaid version {}", crate::error::UPSTREAM_MERMAID_VERSION),
            max_width_px: 512.0,
        }
    }

    pub(crate) fn with_max_width(mut self, max_width_px: f64) -> Self {
        self.max_width_px = max_width_px;
        self
    }

    /// Writes the source-backed body shape used by the existing SVG parity renderer.
    pub(crate) fn write_into(&self, out: &mut String) {
        out.push_str(r#"<g/>"#);
        out.push_str(r#"<g>"#);
        for path in ERROR_ICON_PATHS {
            let _ = write!(out, r#"<path class="error-icon" d="{path}"/>"#);
        }
        out.push_str(
            r#"<text class="error-text" x="1440" y="250" font-size="150px" style="text-anchor: middle;">Syntax error in text</text>"#,
        );
        let _ = write!(
            out,
            r#"<text class="error-text" x="1250" y="400" font-size="100px" style="text-anchor: middle;">{}</text>"#,
            self.version_text
        );
        out.push_str(r#"</g>"#);
    }
}

pub(crate) fn build_for_family(
    family: &BuiltinFamilyArtifact,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    match family {
        BuiltinFamilyArtifact::Error(pair) => {
            build_error_document(pair.layout(), metadata, policy, session)
        }
        BuiltinFamilyArtifact::Flowchart(artifact) => {
            build_flowchart_document(artifact, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Swimlane(artifact) => {
            build_swimlane_document(artifact, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Class(pair) => {
            class::build_class_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::C4(pair) => c4::build_c4_document(pair, metadata, policy, session),
        BuiltinFamilyArtifact::Er(pair) => er::build_er_document(pair, metadata, policy, session),
        BuiltinFamilyArtifact::Info(pair) => {
            info::build_info_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Ishikawa(pair) => {
            ishikawa::build_ishikawa_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Journey(pair) => {
            journey::build_journey_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Kanban(pair) => {
            kanban::build_kanban_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Mindmap(pair) => {
            mindmap::build_mindmap_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Packet(pair) => {
            packet::build_packet_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Pie(pair) => {
            pie::build_pie_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::QuadrantChart(pair) => {
            quadrantchart::build_quadrantchart_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Radar(pair) => {
            radar::build_radar_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Railroad(pair) => {
            railroad::build_railroad_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Requirement(pair) => {
            requirement::build_requirement_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Sankey(pair) => {
            sankey::build_sankey_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Sequence(pair) => {
            sequence::build_sequence_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::State(pair) => {
            state::build_state_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Treemap(pair) => {
            treemap::build_treemap_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::TreeView(pair) => {
            tree_view::build_tree_view_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Venn(pair) => {
            venn::build_venn_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::XyChart(pair) => {
            xychart::build_xychart_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::EventModeling(pair) => {
            eventmodeling::build_eventmodeling_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Cynefin(pair) => {
            cynefin::build_cynefin_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Gantt(pair) => {
            gantt::build_gantt_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::GitGraph(pair) => {
            gitgraph::build_gitgraph_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Timeline(pair) => {
            timeline::build_timeline_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Wardley(pair) => {
            wardley::build_wardley_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Block(pair) => {
            block::build_block_document(pair, metadata, policy, session)
        }
        #[cfg(feature = "layout-cytoscape")]
        BuiltinFamilyArtifact::Architecture(pair) => {
            architecture::build_architecture_document(pair, metadata, policy, session)
        }
        BuiltinFamilyArtifact::Zenuml(pair) => {
            zenuml::build_zenuml_document(pair, metadata, policy, session)
        }
    }
}

fn build_error_document(
    layout: &ErrorDiagramLayout,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    session.checkpoint(OperationPhase::Emit)?;
    let background = theme_color(
        metadata.effective_config.as_value(),
        "errorBkgColor",
        "#552222",
    )?;
    let text_color = theme_color(
        metadata.effective_config.as_value(),
        "errorTextColor",
        "#552222",
    )?;
    let font = FontDescriptor {
        families: parse_font_families(crate::config::config_font_family_css(
            metadata.effective_config.as_value(),
        )),
        weight: 400,
        style: FontStyle::Normal,
        postscript_name: None,
        resource: None,
    };
    let body = ErrorSvgBody::new().with_max_width(layout.max_width_px);

    let mut resources = Vec::with_capacity(ERROR_ICON_PATHS.len());
    let mut commands = vec![
        DrawingCommand::Save,
        DrawingCommand::BeginSemanticGroup {
            semantic_id: "error.document".to_string(),
        },
    ];
    for (index, path_data) in ERROR_ICON_PATHS.iter().enumerate() {
        session.checkpoint(OperationPhase::Emit)?;
        let id = ResourceId::new(format!("error.icon.{index}"));
        resources.push(DrawingResource::Path(PathResource {
            id: id.clone(),
            segments: parse_svg_path(path_data)?,
        }));
        commands.push(DrawingCommand::DrawPath {
            path: id,
            style: PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(background)),
                stroke: None,
            },
        });
    }
    commands.push(error_text_command(
        "Syntax error in text",
        Point::new(1440.0, 250.0),
        Rect::new(0.0, 100.0, layout.viewbox_width, 180.0),
        150.0,
        &font,
        text_color,
    ));
    commands.push(error_text_command(
        &body.version_text,
        Point::new(1250.0, 400.0),
        Rect::new(0.0, 300.0, layout.viewbox_width, 130.0),
        100.0,
        &font,
        text_color,
    ));
    commands.push(DrawingCommand::EndSemanticGroup);
    commands.push(DrawingCommand::Restore);

    let document = DrawingListDocument {
        version: DRAWING_LIST_VERSION,
        coordinate_system: CoordinateSystem::LogicalPixelsYDown,
        viewport: Viewport::new(Rect::new(
            0.0,
            0.0,
            layout.viewbox_width,
            layout.viewbox_height,
        )),
        policy,
        resources,
        commands,
        semantics: vec![SemanticAnnotation {
            id: "error.document".to_string(),
            role: SemanticRole::Document,
            title: Some("Syntax error in text".to_string()),
            description: Some(body.version_text.clone()),
            link: None,
        }],
        fallbacks: Vec::new(),
        extensions: BTreeMap::new(),
    };

    document.validate().map_err(Error::DrawingListContract)?;
    Ok(RenderDocument {
        public: document,
        svg: SvgStructureSidecar {
            family: RenderFamilyKind::Error,
            body: SvgStructureBody::Error(body),
        },
    })
}

fn error_text_command(
    text: &str,
    origin: Point,
    bounds: Rect,
    font_size: f64,
    font: &FontDescriptor,
    fill: Color,
) -> DrawingCommand {
    DrawingCommand::DrawText {
        run: TextRun {
            text: text.to_string(),
            origin,
            bounds,
            style: TextStyle {
                font: font.clone(),
                font_size,
                letter_spacing: 0.0,
                line_height: font_size,
                fill: Paint::solid(fill),
            },
            anchor: TextAnchor::Middle,
            baseline: TextBaseline::Alphabetic,
            direction: TextDirection::Auto,
            language: None,
            obligation: TextObligation::HostText {
                measurement: MeasurementProvenance::DeterministicFallback {
                    profile: "error-fixed-text".to_string(),
                },
            },
        },
    }
}

fn theme_color(config: &Value, key: &str, fallback: &str) -> Result<Color> {
    let value = config
        .get("themeVariables")
        .and_then(|variables| variables.get(key))
        .and_then(Value::as_str)
        .unwrap_or(fallback);
    let parsed = ThemeColor::parse(value).map_err(|error| Error::InvalidModel {
        message: format!("DrawingList cannot resolve theme color {key}: {error}"),
    })?;
    let rgb_channel =
        |kind: ColorChannel| -> u8 { parsed.channel(kind).round().clamp(0.0, 255.0) as u8 };
    let alpha = (parsed.channel(ColorChannel::Alpha) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    Ok(Color::rgba(
        rgb_channel(ColorChannel::Red),
        rgb_channel(ColorChannel::Green),
        rgb_channel(ColorChannel::Blue),
        alpha,
    ))
}

fn parse_font_families(value: String) -> Vec<String> {
    let mut families = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for character in value.chars() {
        match character {
            '\'' | '"' if quote == Some(character) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(character),
            ',' if quote.is_none() => {
                push_font_family(&mut families, &current);
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(character);
    }
    push_font_family(&mut families, &current);
    if families.is_empty() {
        vec!["sans-serif".to_string()]
    } else {
        families
    }
}

fn push_font_family(families: &mut Vec<String>, value: &str) {
    let value = value.trim().trim_matches(['\'', '"']);
    if !value.is_empty() {
        families.push(value.to_string());
    }
}

pub(crate) fn parse_svg_path(data: &str) -> Result<Vec<PathSegment>> {
    let mut cursor = PathCursor::default();
    let mut output = Vec::new();
    for segment in PathParser::from(data) {
        let segment = segment.map_err(|error| Error::InvalidModel {
            message: format!("invalid source-backed SVG path: {error}"),
        })?;
        match segment {
            SvgPathSegment::MoveTo { abs, x, y } => {
                let to = cursor.point(abs, x, y);
                cursor.current = to;
                cursor.subpath_start = to;
                cursor.reset_controls();
                output.push(PathSegment::MoveTo { to });
            }
            SvgPathSegment::LineTo { abs, x, y } => {
                let to = cursor.point(abs, x, y);
                cursor.current = to;
                cursor.reset_controls();
                output.push(PathSegment::LineTo { to });
            }
            SvgPathSegment::HorizontalLineTo { abs, x } => {
                let to = Point::new(if abs { x } else { cursor.current.x + x }, cursor.current.y);
                cursor.current = to;
                cursor.reset_controls();
                output.push(PathSegment::LineTo { to });
            }
            SvgPathSegment::VerticalLineTo { abs, y } => {
                let to = Point::new(cursor.current.x, if abs { y } else { cursor.current.y + y });
                cursor.current = to;
                cursor.reset_controls();
                output.push(PathSegment::LineTo { to });
            }
            SvgPathSegment::CurveTo {
                abs,
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                let control1 = cursor.point(abs, x1, y1);
                let control2 = cursor.point(abs, x2, y2);
                let to = cursor.point(abs, x, y);
                cursor.current = to;
                cursor.cubic_control = Some(control2);
                cursor.quadratic_control = None;
                output.push(PathSegment::CubicTo {
                    control1,
                    control2,
                    to,
                });
            }
            SvgPathSegment::SmoothCurveTo { abs, x2, y2, x, y } => {
                let control1 = cursor
                    .cubic_control
                    .map_or(cursor.current, |previous| reflect(previous, cursor.current));
                let control2 = cursor.point(abs, x2, y2);
                let to = cursor.point(abs, x, y);
                cursor.current = to;
                cursor.cubic_control = Some(control2);
                cursor.quadratic_control = None;
                output.push(PathSegment::CubicTo {
                    control1,
                    control2,
                    to,
                });
            }
            SvgPathSegment::Quadratic { abs, x1, y1, x, y } => {
                let control = cursor.point(abs, x1, y1);
                let to = cursor.point(abs, x, y);
                cursor.current = to;
                cursor.cubic_control = None;
                cursor.quadratic_control = Some(control);
                output.push(PathSegment::QuadTo { control, to });
            }
            SvgPathSegment::SmoothQuadratic { abs, x, y } => {
                let control = cursor
                    .quadratic_control
                    .map_or(cursor.current, |previous| reflect(previous, cursor.current));
                let to = cursor.point(abs, x, y);
                cursor.current = to;
                cursor.cubic_control = None;
                cursor.quadratic_control = Some(control);
                output.push(PathSegment::QuadTo { control, to });
            }
            SvgPathSegment::EllipticalArc {
                abs,
                rx,
                ry,
                x_axis_rotation,
                large_arc,
                sweep,
                x,
                y,
            } => {
                let to = cursor.point(abs, x, y);
                cursor.current = to;
                cursor.reset_controls();
                output.push(PathSegment::ArcTo {
                    radius_x: rx,
                    radius_y: ry,
                    x_axis_rotation_degrees: x_axis_rotation,
                    large_arc,
                    sweep_clockwise: sweep,
                    to,
                });
            }
            SvgPathSegment::ClosePath { .. } => {
                cursor.current = cursor.subpath_start;
                cursor.reset_controls();
                output.push(PathSegment::Close);
            }
        }
    }
    Ok(output)
}

#[derive(Debug)]
struct PathCursor {
    current: Point,
    subpath_start: Point,
    cubic_control: Option<Point>,
    quadratic_control: Option<Point>,
}

impl Default for PathCursor {
    fn default() -> Self {
        Self {
            current: Point::new(0.0, 0.0),
            subpath_start: Point::new(0.0, 0.0),
            cubic_control: None,
            quadratic_control: None,
        }
    }
}

impl PathCursor {
    fn point(&self, absolute: bool, x: f64, y: f64) -> Point {
        if absolute {
            Point::new(x, y)
        } else {
            Point::new(self.current.x + x, self.current.y + y)
        }
    }

    fn reset_controls(&mut self) {
        self.cubic_control = None;
        self.quadratic_control = None;
    }
}

fn reflect(control: Point, around: Point) -> Point {
    Point::new(2.0 * around.x - control.x, 2.0 * around.y - control.y)
}
