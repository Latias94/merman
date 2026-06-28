use super::label::GraphLabel;
use super::model::GraphNodeShape;
use crate::options::AsciiRenderOptions;

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

    pub(super) fn size_for_label(
        self,
        label: &GraphLabel,
        options: &AsciiRenderOptions,
    ) -> GraphNodeShapeSize {
        let framed_width = label.width() + options.box_border_padding * 2 + 2;
        let framed_height = 2 + label.content_height() + options.box_border_padding * 2;

        match self.shape {
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
                width: framed_width + 2,
                height: framed_height,
            },
            GraphNodeShape::LeanRight | GraphNodeShape::LeanLeft => GraphNodeShapeSize {
                width: framed_width + framed_height.saturating_sub(1),
                height: framed_height,
            },
            GraphNodeShape::Rect | GraphNodeShape::Rounded | GraphNodeShape::Diamond => {
                GraphNodeShapeSize {
                    width: framed_width,
                    height: framed_height,
                }
            }
            GraphNodeShape::Datastore | GraphNodeShape::Document => GraphNodeShapeSize {
                width: framed_width,
                height: framed_height,
            },
        }
    }

    pub(super) fn uses_external_self_loop_connector(self) -> bool {
        !matches!(self.shape, GraphNodeShape::Diamond | GraphNodeShape::Choice)
    }

    pub(super) fn uses_drop_then_turn_bent_route(self) -> bool {
        matches!(
            self.shape,
            GraphNodeShape::StateStart
                | GraphNodeShape::StateEnd
                | GraphNodeShape::ForkJoinHorizontal
                | GraphNodeShape::ForkJoinVertical
                | GraphNodeShape::Choice
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{GraphNodeShapeSemantics, GraphNodeShapeSize};
    use crate::AsciiRenderOptions;
    use crate::graph::label::GraphLabel;
    use crate::graph::model::GraphNodeShape;

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
