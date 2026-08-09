use super::label::GraphLabel;
use super::model::{GraphDirection, GraphNodeShape};
use crate::error::{AsciiError, Result};
use crate::options::AsciiRenderOptions;
use crate::resource::ResourceContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphNodeShapeProjection {
    Fixed(GraphNodeShape),
    Approximate(GraphNodeShape),
    PerpendicularForkJoin,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphNodeLabelPolicy {
    Preserve,
    Suppress,
}

#[derive(Debug, Clone, Copy)]
struct GraphNodeShapeDefinition {
    names: &'static [&'static str],
    projection: GraphNodeShapeProjection,
    label_policy: GraphNodeLabelPolicy,
}

impl GraphNodeShapeDefinition {
    fn contains(self, name: &str) -> bool {
        self.names.contains(&name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ResolvedGraphNodeShape {
    pub(super) shape: GraphNodeShape,
    label_policy: GraphNodeLabelPolicy,
}

impl ResolvedGraphNodeShape {
    pub(super) fn projected_label(self, label: &str) -> &str {
        match self.label_policy {
            GraphNodeLabelPolicy::Preserve => label,
            GraphNodeLabelPolicy::Suppress => "",
        }
    }
}

macro_rules! shape_definition {
    ($names:expr, $projection:expr, $label_policy:expr) => {
        GraphNodeShapeDefinition {
            names: $names,
            projection: $projection,
            label_policy: $label_policy,
        }
    };
}

const GRAPH_NODE_SHAPE_DEFINITIONS: &[GraphNodeShapeDefinition] = &[
    shape_definition!(
        &[
            "rect",
            "rectangle",
            "square",
            "squareRect",
            "proc",
            "process"
        ],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::Rect),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["roundedRect", "rounded", "event", "ellipse"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::Rounded),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["circle", "circ"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::Circle),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["stadium", "terminal", "pill"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::Stadium),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["doublecircle", "dbl-circ", "double-circle"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::DoubleCircle),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["diamond", "question", "diam", "decision"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::Diamond),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &[
            "subroutine",
            "fr-rect",
            "subproc",
            "subprocess",
            "framed-rectangle"
        ],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::Subroutine),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["cylinder", "cyl", "db", "database"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::Cylinder),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["lean_right", "lean-r", "lean-right", "in-out"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::LeanRight),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["lean_left", "lean-l", "lean-left", "out-in"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::LeanLeft),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["sl-rect", "manual-input", "sloped-rectangle"],
        GraphNodeShapeProjection::Approximate(GraphNodeShape::ManualInput),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["datastore", "data-store"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::Datastore),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["bow-rect", "stored-data", "bow-tie-rectangle"],
        GraphNodeShapeProjection::Approximate(GraphNodeShape::BowTie),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["doc", "document"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::Document),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["docs", "documents", "st-doc", "stacked-document"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::StackedDocument),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["lin-doc", "lined-document"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::LinedDocument),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["tag-doc", "tagged-document"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::TaggedDocument),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["st-rect", "stacked-rectangle", "processes", "procs"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::StackedRect),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &[
            "lin-rect",
            "lin-proc",
            "lined-process",
            "lined-rectangle",
            "shaded-process"
        ],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::LinedRect),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["tag-rect", "tag-proc", "tagged-process", "tagged-rectangle"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::TaggedRect),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["hexagon", "hex", "prepare"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::Hexagon),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["odd", "rect_left_inv_arrow"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::Asymmetric),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["flag", "paper-tape"],
        GraphNodeShapeProjection::Approximate(GraphNodeShape::PaperTape),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["choice"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::Choice),
        GraphNodeLabelPolicy::Suppress
    ),
    shape_definition!(
        &["fork", "join", "forkJoin"],
        GraphNodeShapeProjection::PerpendicularForkJoin,
        GraphNodeLabelPolicy::Suppress
    ),
    shape_definition!(
        &[
            "start",
            "small-circle",
            "sm-circ",
            "stateStart",
            "f-circ",
            "junction",
            "filled-circle",
        ],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::StateStart),
        GraphNodeLabelPolicy::Suppress
    ),
    shape_definition!(
        &["stop", "framed-circle", "fr-circ", "stateEnd"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::StateEnd),
        GraphNodeLabelPolicy::Suppress
    ),
    shape_definition!(
        &["trapezoid", "trap-b", "priority", "trapezoid-bottom"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::Trapezoid),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &[
            "inv_trapezoid",
            "inv-trapezoid",
            "trap-t",
            "manual",
            "trapezoid-top"
        ],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::TrapezoidAlt),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["text", "labelRect"],
        GraphNodeShapeProjection::Fixed(GraphNodeShape::Text),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &["state"],
        GraphNodeShapeProjection::Approximate(GraphNodeShape::Rounded),
        GraphNodeLabelPolicy::Preserve
    ),
    shape_definition!(
        &[
            "anchor",
            "bang",
            "bolt",
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
            "kanbanItem",
            "lightning-bolt",
            "lin-cyl",
            "lined-cylinder",
            "loop-limit",
            "manual-file",
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
        GraphNodeShapeProjection::Unsupported,
        GraphNodeLabelPolicy::Preserve
    ),
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
    let shape = match definition.projection {
        GraphNodeShapeProjection::Fixed(shape) | GraphNodeShapeProjection::Approximate(shape) => {
            shape
        }
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
            | GraphNodeShape::TaggedDocument => GraphNodeShapeSize {
                width: resources.checked_grid_add(framed_width, 2)?,
                height: framed_height,
            },
            GraphNodeShape::ManualInput | GraphNodeShape::BowTie | GraphNodeShape::PaperTape => {
                GraphNodeShapeSize {
                    width: framed_width,
                    height: framed_height,
                }
            }
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
    fn shape_registry_preserves_upstream_alias_handler_families() {
        let alias_families = [
            (
                &["sl-rect", "manual-input", "sloped-rectangle"] as &[&str],
                GraphNodeShape::ManualInput,
                "Label only",
            ),
            (
                &["bow-rect", "stored-data", "bow-tie-rectangle"],
                GraphNodeShape::BowTie,
                "Label only",
            ),
            (
                &["flag", "paper-tape"],
                GraphNodeShape::PaperTape,
                "Label only",
            ),
            (
                &["f-circ", "junction", "filled-circle"],
                GraphNodeShape::StateStart,
                "",
            ),
            (&["text", "labelRect"], GraphNodeShape::Text, "Label only"),
        ];

        for (aliases, expected_shape, expected_label) in alias_families {
            for alias in aliases {
                let resolved =
                    resolve_flowchart_node_shape(Some(alias), GraphDirection::LeftRight).unwrap();
                assert_eq!(resolved.shape, expected_shape, "alias: {alias}");
                assert_eq!(
                    resolved.projected_label("Label only"),
                    expected_label,
                    "alias: {alias}"
                );
            }
        }

        let datastore =
            resolve_flowchart_node_shape(Some("data-store"), GraphDirection::LeftRight).unwrap();
        assert_eq!(datastore.shape, GraphNodeShape::Datastore);
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
