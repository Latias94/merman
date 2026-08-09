use super::label::GraphLabel;
use super::model::{GraphDirection, GraphNodeShape};
use crate::error::{AsciiError, Result};
use crate::options::AsciiRenderOptions;
use crate::resource::ResourceContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphNodeShapeFidelity {
    Diagrammatic,
    Approximation,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphNodeShapeProjection {
    Fixed(GraphNodeShape),
    PerpendicularForkJoin,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphNodeLabelPolicy {
    Preserve,
    Suppress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphNodeShapeDecorator {
    None,
    Stacked,
    Lined,
    Tagged,
    Flag,
    PaperTape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphNodePortPolicy {
    Boundary,
    Radial,
    Diamond,
    SynchronizationBar,
    None,
}

#[derive(Debug, Clone, Copy)]
struct GraphNodeShapeDefinition {
    names: &'static [&'static str],
    fidelity: GraphNodeShapeFidelity,
    projection: GraphNodeShapeProjection,
    label_policy: GraphNodeLabelPolicy,
    decorator: GraphNodeShapeDecorator,
    port_policy: GraphNodePortPolicy,
}

impl GraphNodeShapeDefinition {
    fn contains(self, name: &str) -> bool {
        self.names.contains(&name)
    }

    fn contract_is_consistent(self) -> bool {
        match (self.fidelity, self.projection, self.port_policy) {
            (
                GraphNodeShapeFidelity::Unsupported,
                GraphNodeShapeProjection::Unsupported,
                GraphNodePortPolicy::None,
            ) => true,
            (GraphNodeShapeFidelity::Unsupported, _, _)
            | (_, GraphNodeShapeProjection::Unsupported, _) => false,
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ResolvedGraphNodeShape {
    pub(super) shape: GraphNodeShape,
    label_policy: GraphNodeLabelPolicy,
}

impl ResolvedGraphNodeShape {
    pub(super) fn projected_label<'a>(self, label: &'a str) -> &'a str {
        match self.label_policy {
            GraphNodeLabelPolicy::Preserve => label,
            GraphNodeLabelPolicy::Suppress => "",
        }
    }
}

const GRAPH_NODE_SHAPE_DEFINITIONS: &[GraphNodeShapeDefinition] = &[
    GraphNodeShapeDefinition {
        names: &[
            "rect",
            "rectangle",
            "square",
            "squareRect",
            "proc",
            "process",
            "labelRect",
        ],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::Rect),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["roundedRect", "rounded", "event", "ellipse"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::Rounded),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["circle", "circ"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::Circle),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Radial,
    },
    GraphNodeShapeDefinition {
        names: &["stadium", "terminal", "pill"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::Stadium),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["doublecircle", "dbl-circ", "double-circle"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::DoubleCircle),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Radial,
    },
    GraphNodeShapeDefinition {
        names: &["diamond", "question", "diam", "decision"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::Diamond),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Diamond,
    },
    GraphNodeShapeDefinition {
        names: &[
            "subroutine",
            "fr-rect",
            "subproc",
            "subprocess",
            "framed-rectangle",
        ],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::Subroutine),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::Lined,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["cylinder", "cyl", "db", "database"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::Cylinder),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &[
            "lean_right",
            "lean-r",
            "lean-right",
            "in-out",
            "sl-rect",
            "sloped-rectangle",
        ],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::LeanRight),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["lean_left", "lean-l", "lean-left", "out-in"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::LeanLeft),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["datastore", "data-store", "stored-data"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::Datastore),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["doc", "document"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::Document),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["docs", "documents", "st-doc", "stacked-document"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::StackedDocument),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::Stacked,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["lin-doc", "lined-document"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::LinedDocument),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::Lined,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["tag-doc", "tagged-document"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::TaggedDocument),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::Tagged,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["st-rect", "stacked-rectangle", "processes", "procs"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::StackedRect),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::Stacked,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &[
            "lin-rect",
            "lin-proc",
            "lined-process",
            "lined-rectangle",
            "shaded-process",
        ],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::LinedRect),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::Lined,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["tag-rect", "tag-proc", "tagged-process", "tagged-rectangle"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::TaggedRect),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::Tagged,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["hexagon", "hex", "prepare"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::Hexagon),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["odd", "rect_left_inv_arrow"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::Asymmetric),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["flag"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::Flag),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::Flag,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["paper-tape"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::PaperTape),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::PaperTape,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["choice"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::Choice),
        label_policy: GraphNodeLabelPolicy::Suppress,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Diamond,
    },
    GraphNodeShapeDefinition {
        names: &["fork", "join", "forkJoin"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::PerpendicularForkJoin,
        label_policy: GraphNodeLabelPolicy::Suppress,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::SynchronizationBar,
    },
    GraphNodeShapeDefinition {
        names: &[
            "start",
            "small-circle",
            "sm-circ",
            "stateStart",
            "f-circ",
            "filled-circle",
        ],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::StateStart),
        label_policy: GraphNodeLabelPolicy::Suppress,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Radial,
    },
    GraphNodeShapeDefinition {
        names: &["stop", "framed-circle", "fr-circ", "stateEnd"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::StateEnd),
        label_policy: GraphNodeLabelPolicy::Suppress,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Radial,
    },
    GraphNodeShapeDefinition {
        names: &["trapezoid", "trap-b", "priority", "trapezoid-bottom"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::Trapezoid),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &[
            "inv_trapezoid",
            "inv-trapezoid",
            "trap-t",
            "manual",
            "trapezoid-top",
        ],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::TrapezoidAlt),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["text"],
        fidelity: GraphNodeShapeFidelity::Diagrammatic,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::Text),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &["state"],
        fidelity: GraphNodeShapeFidelity::Approximation,
        projection: GraphNodeShapeProjection::Fixed(GraphNodeShape::Rounded),
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::Boundary,
    },
    GraphNodeShapeDefinition {
        names: &[
            "anchor",
            "bang",
            "bolt",
            "bow-rect",
            "bow-tie-rectangle",
            "brace",
            "brace-l",
            "brace-r",
            "braces",
            "card",
            "classBox",
            "cloud",
            "collate",
            "com-link",
            "comment",
            "cross-circ",
            "crossed-circle",
            "curv-trap",
            "curved-trapezoid",
            "das",
            "defaultMindmapNode",
            "delay",
            "disk",
            "display",
            "div-proc",
            "div-rect",
            "divided-process",
            "divided-rectangle",
            "erBox",
            "extract",
            "flip-tri",
            "flipped-triangle",
            "h-cyl",
            "half-rounded-rectangle",
            "horizontal-cylinder",
            "hourglass",
            "icon",
            "iconCircle",
            "iconRounded",
            "iconSquare",
            "imageSquare",
            "internal-storage",
            "junction",
            "kanbanItem",
            "lightning-bolt",
            "lin-cyl",
            "lined-cylinder",
            "loop-limit",
            "manual-file",
            "manual-input",
            "mindmapCircle",
            "notch-pent",
            "notch-rect",
            "notched-pentagon",
            "notched-rectangle",
            "note",
            "rectWithTitle",
            "requirementBox",
            "summary",
            "tri",
            "triangle",
            "win-pane",
            "window-pane",
        ],
        fidelity: GraphNodeShapeFidelity::Unsupported,
        projection: GraphNodeShapeProjection::Unsupported,
        label_policy: GraphNodeLabelPolicy::Preserve,
        decorator: GraphNodeShapeDecorator::None,
        port_policy: GraphNodePortPolicy::None,
    },
];

pub(super) fn resolve_flowchart_node_shape(
    name: Option<&str>,
    direction: GraphDirection,
) -> Result<ResolvedGraphNodeShape> {
    let name = name.unwrap_or("squareRect");
    let Some(definition) = GRAPH_NODE_SHAPE_DEFINITIONS
        .iter()
        .copied()
        .find(|definition| definition.contains(name))
    else {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "unknown flowchart node shapes",
        });
    };
    debug_assert!(definition.contract_is_consistent());
    let _terminal_contract = (
        definition.fidelity,
        definition.decorator,
        definition.port_policy,
    );
    let shape = match definition.projection {
        GraphNodeShapeProjection::Fixed(shape) => shape,
        GraphNodeShapeProjection::PerpendicularForkJoin => match direction.canonical() {
            GraphDirection::LeftRight => GraphNodeShape::ForkJoinVertical,
            GraphDirection::TopDown => GraphNodeShape::ForkJoinHorizontal,
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        },
        GraphNodeShapeProjection::Unsupported => {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "unsupported flowchart node shape projections",
            });
        }
    };
    Ok(ResolvedGraphNodeShape {
        shape,
        label_policy: definition.label_policy,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GraphNodeShapeSemantics {
    shape: GraphNodeShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GraphNodeShapeSize {
    pub(super) width: usize,
    pub(super) height: usize,
}

impl GraphNodeShapeSemantics {
    pub(super) fn new(shape: GraphNodeShape) -> Self {
        Self { shape }
    }

    #[cfg(test)]
    pub(super) fn size_for_label(
        self,
        label: &GraphLabel,
        options: &AsciiRenderOptions,
    ) -> GraphNodeShapeSize {
        let resources = ResourceContext::new(crate::resource::AsciiResourcePolicy::for_profile(
            merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
        ));
        self.try_size_for_label(label, options, &resources)
            .expect("trusted graph shape geometry must remain representable")
    }

    pub(super) fn try_size_for_label(
        self,
        label: &GraphLabel,
        options: &AsciiRenderOptions,
        resources: &ResourceContext,
    ) -> Result<GraphNodeShapeSize> {
        let border_padding = resources.checked_grid_mul(options.box_border_padding, 2)?;
        let framed_width = resources.checked_grid_add(
            resources.checked_grid_add(label.width(), border_padding)?,
            2,
        )?;
        let framed_height = resources.checked_grid_add(
            resources.checked_grid_add(label.content_height(), border_padding)?,
            2,
        )?;

        let size = match self.shape {
            GraphNodeShape::StateStart | GraphNodeShape::StateEnd | GraphNodeShape::Choice => {
                GraphNodeShapeSize {
                    width: 5,
                    height: 3,
                }
            }
            GraphNodeShape::ForkJoinHorizontal => GraphNodeShapeSize {
                width: 7,
                height: 3,
            },
            GraphNodeShape::ForkJoinVertical => GraphNodeShapeSize {
                width: 3,
                height: 7,
            },
            GraphNodeShape::Subroutine | GraphNodeShape::Cylinder => GraphNodeShapeSize {
                width: resources.checked_grid_add(framed_width, 2)?,
                height: framed_height,
            },
            GraphNodeShape::StackedRect | GraphNodeShape::StackedDocument => GraphNodeShapeSize {
                width: resources.checked_grid_add(framed_width, 2)?,
                height: resources.checked_grid_add(framed_height, 1)?,
            },
            GraphNodeShape::LinedRect
            | GraphNodeShape::TaggedRect
            | GraphNodeShape::LinedDocument
            | GraphNodeShape::TaggedDocument
            | GraphNodeShape::Flag => GraphNodeShapeSize {
                width: resources.checked_grid_add(framed_width, 2)?,
                height: framed_height,
            },
            GraphNodeShape::PaperTape => GraphNodeShapeSize {
                width: framed_width,
                height: framed_height,
            },
            GraphNodeShape::Text => GraphNodeShapeSize {
                width: label.width().max(1),
                height: label.content_height().max(1),
            },
            GraphNodeShape::LeanRight | GraphNodeShape::LeanLeft => GraphNodeShapeSize {
                width: resources.checked_grid_add(framed_width, framed_height.saturating_sub(1))?,
                height: framed_height,
            },
            GraphNodeShape::Rect
            | GraphNodeShape::Rounded
            | GraphNodeShape::Circle
            | GraphNodeShape::DoubleCircle
            | GraphNodeShape::Diamond
            | GraphNodeShape::Hexagon
            | GraphNodeShape::Asymmetric
            | GraphNodeShape::Trapezoid
            | GraphNodeShape::TrapezoidAlt => GraphNodeShapeSize {
                width: framed_width,
                height: framed_height,
            },
            GraphNodeShape::Stadium => GraphNodeShapeSize {
                width: resources.checked_grid_add(framed_width, 2)?,
                height: framed_height,
            },
            GraphNodeShape::Datastore | GraphNodeShape::Document => GraphNodeShapeSize {
                width: framed_width,
                height: framed_height,
            },
        };
        resources.grid_extent(size.width, size.height)?;
        Ok(size)
    }

    pub(super) fn uses_external_self_loop_connector(self) -> bool {
        !matches!(
            self.shape,
            GraphNodeShape::Diamond | GraphNodeShape::Choice | GraphNodeShape::Text
        )
    }

    pub(super) fn uses_drop_then_turn_bent_route(self) -> bool {
        matches!(
            self.shape,
            GraphNodeShape::StateStart
                | GraphNodeShape::StateEnd
                | GraphNodeShape::ForkJoinHorizontal
                | GraphNodeShape::ForkJoinVertical
                | GraphNodeShape::Choice
                | GraphNodeShape::Text
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GRAPH_NODE_SHAPE_DEFINITIONS, GraphNodeShapeSemantics, GraphNodeShapeSize,
        resolve_flowchart_node_shape,
    };
    use crate::AsciiRenderOptions;
    use crate::graph::label::GraphLabel;
    use crate::graph::model::{GraphDirection, GraphNodeShape};
    use std::collections::HashSet;

    #[test]
    fn shape_registry_is_unique_and_covers_every_pinned_mermaid_name() {
        let mut registered = HashSet::new();
        for definition in GRAPH_NODE_SHAPE_DEFINITIONS {
            assert!(definition.contract_is_consistent());
            for name in definition.names {
                assert!(
                    registered.insert(*name),
                    "shape name {name:?} must have exactly one terminal disposition"
                );
            }
        }

        for name in merman_core::diagrams::flowchart::flowchart_pinned_shape_names() {
            assert!(
                registered.contains(name),
                "pinned Mermaid shape {name:?} is missing a terminal disposition"
            );
        }
    }

    #[test]
    fn shape_registry_resolves_process_and_flow_perpendicular_fork_semantics() {
        let process =
            resolve_flowchart_node_shape(Some("process"), GraphDirection::LeftRight).unwrap();
        assert_eq!(process.shape, GraphNodeShape::Rect);
        assert_eq!(process.projected_label("Process"), "Process");

        let left_right_fork =
            resolve_flowchart_node_shape(Some("fork"), GraphDirection::LeftRight).unwrap();
        assert_eq!(left_right_fork.shape, GraphNodeShape::ForkJoinVertical);
        assert_eq!(left_right_fork.projected_label("implementation id"), "");

        let top_down_fork =
            resolve_flowchart_node_shape(Some("join"), GraphDirection::TopDown).unwrap();
        assert_eq!(top_down_fork.shape, GraphNodeShape::ForkJoinHorizontal);
        assert_eq!(top_down_fork.projected_label("implementation id"), "");
    }

    #[test]
    fn shape_size_accounts_for_side_frame_shapes() {
        let options = AsciiRenderOptions::unicode();
        let label = GraphLabel::new("中A");

        let rect =
            GraphNodeShapeSemantics::new(GraphNodeShape::Rect).size_for_label(&label, &options);
        let subroutine = GraphNodeShapeSemantics::new(GraphNodeShape::Subroutine)
            .size_for_label(&label, &options);
        let cylinder =
            GraphNodeShapeSemantics::new(GraphNodeShape::Cylinder).size_for_label(&label, &options);

        assert_eq!(
            rect,
            GraphNodeShapeSize {
                width: label.width() + options.box_border_padding * 2 + 2,
                height: label.content_height() + options.box_border_padding * 2 + 2,
            }
        );
        assert_eq!(subroutine.width, rect.width + 2);
        assert_eq!(subroutine.height, rect.height);
        assert_eq!(cylinder.width, rect.width + 2);
        assert_eq!(cylinder.height, rect.height);
    }

    #[test]
    fn shape_size_accounts_for_corner_decorated_boxes() {
        let options = AsciiRenderOptions::unicode();
        let label = GraphLabel::new("中A");

        let rect =
            GraphNodeShapeSemantics::new(GraphNodeShape::Rect).size_for_label(&label, &options);
        let choice =
            GraphNodeShapeSemantics::new(GraphNodeShape::Choice).size_for_label(&label, &options);

        for shape in [
            GraphNodeShape::Circle,
            GraphNodeShape::DoubleCircle,
            GraphNodeShape::Hexagon,
            GraphNodeShape::Asymmetric,
            GraphNodeShape::Trapezoid,
            GraphNodeShape::TrapezoidAlt,
        ] {
            assert_eq!(
                GraphNodeShapeSemantics::new(shape).size_for_label(&label, &options),
                rect
            );
        }
        assert_eq!(
            choice,
            GraphNodeShapeSize {
                width: 5,
                height: 3,
            }
        );
    }

    #[test]
    fn shape_size_accounts_for_stadium_width() {
        let options = AsciiRenderOptions::unicode();
        let label = GraphLabel::new("中A");

        let rect =
            GraphNodeShapeSemantics::new(GraphNodeShape::Rect).size_for_label(&label, &options);
        let stadium =
            GraphNodeShapeSemantics::new(GraphNodeShape::Stadium).size_for_label(&label, &options);

        assert_eq!(stadium.width, rect.width + 2);
        assert_eq!(stadium.height, rect.height);
    }

    #[test]
    fn fixed_control_shapes_keep_terminal_dimensions() {
        let options = AsciiRenderOptions::unicode();
        let label = GraphLabel::new("very long label");

        assert_eq!(
            GraphNodeShapeSemantics::new(GraphNodeShape::StateStart)
                .size_for_label(&label, &options),
            GraphNodeShapeSize {
                width: 5,
                height: 3,
            }
        );
        assert_eq!(
            GraphNodeShapeSemantics::new(GraphNodeShape::ForkJoinHorizontal)
                .size_for_label(&label, &options),
            GraphNodeShapeSize {
                width: 7,
                height: 3,
            }
        );
        assert_eq!(
            GraphNodeShapeSemantics::new(GraphNodeShape::ForkJoinVertical)
                .size_for_label(&label, &options),
            GraphNodeShapeSize {
                width: 3,
                height: 7,
            }
        );
    }

    #[test]
    fn shape_route_semantics_distinguish_choice_and_diamond() {
        let rect = GraphNodeShapeSemantics::new(GraphNodeShape::Rect);
        let diamond = GraphNodeShapeSemantics::new(GraphNodeShape::Diamond);
        let choice = GraphNodeShapeSemantics::new(GraphNodeShape::Choice);
        let state_start = GraphNodeShapeSemantics::new(GraphNodeShape::StateStart);

        assert!(rect.uses_external_self_loop_connector());
        assert!(!diamond.uses_external_self_loop_connector());
        assert!(!choice.uses_external_self_loop_connector());

        assert!(!rect.uses_drop_then_turn_bent_route());
        assert!(!diamond.uses_drop_then_turn_bent_route());
        assert!(choice.uses_drop_then_turn_bent_route());
        assert!(state_start.uses_drop_then_turn_bent_route());
    }
}
