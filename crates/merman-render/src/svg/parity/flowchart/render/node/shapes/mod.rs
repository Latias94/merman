//! Flowchart node shape renderers.

use merman_render_flow::node_body::{
    self, LayoutNodeBody, MeasuredNodeBody, NoLabelStyle, NodeBodyGeometry, NodeBodySvg,
    RectangleBody, RectangleWithDivider,
};

use crate::flowchart::FlowchartShape;

use crate::svg::parity::flowchart::types::{FlowchartRenderCtx, FlowchartRenderDetails};

use super::{FlowchartNodeGeometry, FlowchartNodeLabelState, FlowchartNodeRenderCommon, helpers};

mod basic_geoms;
mod bpmn;
mod collapsed_group;
mod curly_braces;
mod cylinders;
mod dispatch;
mod document_shapes;
mod hexagon;
mod icon;
mod icon_circle;
mod icon_square;
mod image_square;
mod manuals;
mod odd;
mod outer_path;
mod stadium;
mod triangle;

pub(super) use basic_geoms::{render_circle, render_diamond, render_double_circle};
pub(super) use bpmn::render_bpmn;
pub(super) use collapsed_group::{render_collapsed_group_body, render_collapsed_group_indicators};
pub(super) use curly_braces::render_curly_brace_comment;
pub(in crate::svg::parity::flowchart) use curly_braces::{
    curly_brace_comment_geometry, curly_brace_comment_intersection_points,
};
pub(super) use cylinders::{render_cylinder, render_horizontal_cylinder, render_lined_cylinder};
pub(super) use dispatch::render_flowchart_shape;
pub(super) use document_shapes::{
    render_lined_wave_document, render_stacked_document, render_tagged_wave_document,
    render_wave_document,
};
pub(super) use hexagon::render_hexagon;
pub(super) use icon::render_icon;
pub(super) use icon_circle::render_icon_circle;
pub(super) use icon_square::{render_icon_rounded, render_icon_square};
pub(super) use image_square::try_render_image_square;
pub(super) use manuals::{render_manual_file, render_manual_input};
pub(super) use odd::render_odd;
pub(super) use stadium::render_stadium;
pub(super) use triangle::render_triangle_extract;

use outer_path::OuterPathShape;

fn shape_label_metrics(
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeGeometry<'_>,
    label: &FlowchartNodeLabelState<'_>,
    layout_node: Option<&crate::model::LayoutNode>,
) -> crate::text::TextMetrics {
    helpers::compute_node_label_metrics(
        ctx,
        common.node_id,
        layout_node,
        label.text,
        label.label_type,
        common.node_classes,
        common.node_styles,
    )
}

/// Bind only the facts needed by the lower body. Host measurement stays on the
/// original terminal path and the query measures only when the shape uses Rough.
enum TerminalNodeBody<'a> {
    Layout(LayoutNodeBody<'a>),
    Measured(MeasuredNodeBody<'a>),
}

impl<'a> TerminalNodeBody<'a> {
    fn for_shape(
        ctx: &'a FlowchartRenderCtx<'_>,
        common: &FlowchartNodeGeometry<'_>,
        shape: FlowchartShape,
    ) -> crate::Result<Self> {
        let body = match shape {
            FlowchartShape::Browser => Self::Measured(MeasuredNodeBody::Browser {
                accent: ctx.node_border_color.as_str(),
            }),
            FlowchartShape::Bucket => Self::Measured(MeasuredNodeBody::Bucket {
                accent: ctx.node_border_color.as_str(),
            }),
            FlowchartShape::Console => Self::Measured(MeasuredNodeBody::Console {
                accent: ctx.node_border_color.as_str(),
            }),
            FlowchartShape::Folder => Self::Measured(MeasuredNodeBody::Folder),
            FlowchartShape::PaperTape => Self::Measured(MeasuredNodeBody::PaperTape {
                neo: common.look_is_neo(),
            }),
            FlowchartShape::Person => Self::Measured(MeasuredNodeBody::Person),
            FlowchartShape::TaggedRectangle => Self::Measured(MeasuredNodeBody::TaggedRectangle {
                neo: common.look_is_neo(),
            }),
            FlowchartShape::Bang | FlowchartShape::Cloud => Self::Layout(LayoutNodeBody::Organic {
                cloud: shape == FlowchartShape::Cloud,
                label_size: kurbo::Size::new(
                    common.layout_node.label_width.unwrap_or(0.0).max(0.0),
                    common.layout_node.label_height.unwrap_or(0.0).max(0.0),
                ),
                padding: ctx.node_padding,
                html_labels: ctx.node_html_labels,
            }),
            FlowchartShape::Datastore => Self::Layout(LayoutNodeBody::Datastore),
            FlowchartShape::DividedRectangle => {
                Self::Layout(LayoutNodeBody::Divider(RectangleWithDivider::Divided {
                    authored_padding: ctx.node_padding,
                    neo: common.look_is_neo(),
                }))
            }
            FlowchartShape::Hourglass => Self::Layout(LayoutNodeBody::Hourglass),
            FlowchartShape::NotchedRectangle => Self::Layout(LayoutNodeBody::NotchedRectangle),
            FlowchartShape::Note => Self::Layout(LayoutNodeBody::Note {
                fill: &ctx.compatibility.note_fill,
                stroke: &ctx.compatibility.note_stroke,
            }),
            FlowchartShape::ShadedProcess => {
                Self::Layout(LayoutNodeBody::Divider(RectangleWithDivider::Shaded))
            }
            FlowchartShape::StackedRectangle => Self::Layout(LayoutNodeBody::StackedRectangle {
                neo: common.look_is_neo(),
            }),
            FlowchartShape::Subroutine => Self::Layout(LayoutNodeBody::Subroutine),
            FlowchartShape::Text => Self::Layout(LayoutNodeBody::Text),
            FlowchartShape::WindowPane => Self::Layout(LayoutNodeBody::WindowPane),
            _ => {
                return Err(crate::Error::InvalidModel {
                    message: format!(
                        "Flowchart shape {} ({shape:?}) has no terminal body",
                        common.shape
                    ),
                });
            }
        };
        Ok(body)
    }

    fn write(
        self,
        out: &mut impl crate::svg::parity::SvgOutput,
        ctx: &FlowchartRenderCtx<'_>,
        common: &FlowchartNodeRenderCommon<'_>,
        label: &mut FlowchartNodeLabelState<'_>,
        details: &mut FlowchartRenderDetails,
    ) {
        match self {
            Self::Layout(shape) => {
                if matches!(shape, LayoutNodeBody::Hourglass) {
                    // Mermaid clears the text and still emits the original label kind.
                    label.text = "";
                }
                shape.write(out, body_svg(common, details), &mut label.dx, &mut label.dy);
            }
            Self::Measured(shape) => {
                let metrics = shape_label_metrics(ctx, common, label, Some(common.layout_node));
                shape.write(
                    out,
                    body_svg(common, details),
                    metrics,
                    ctx.node_padding,
                    &mut label.dy,
                );
            }
        }
    }

    fn advance_rough_geometry(
        self,
        ctx: &FlowchartRenderCtx<'_>,
        common: &FlowchartNodeGeometry<'_>,
        label: &mut FlowchartNodeLabelState<'_>,
    ) {
        let geometry = body_geometry(common);
        match self {
            Self::Layout(shape) => {
                shape.advance_rough_geometry(geometry, &mut label.dx, &mut label.dy)
            }
            Self::Measured(shape) if shape.queries_rough(geometry.hand_drawn) => {
                let metrics = shape_label_metrics(ctx, common, label, Some(common.layout_node));
                shape.advance_rough_geometry(geometry, metrics, ctx.node_padding);
            }
            Self::Measured(_) => {}
        }
    }
}

fn body_geometry<'a>(common: &'a FlowchartNodeGeometry<'_>) -> NodeBodyGeometry<'a> {
    NodeBodyGeometry {
        width: common.layout_node.width,
        height: common.layout_node.height,
        hand_drawn: common.look_is_hand_drawn(),
        stroke_width: common.stroke_width,
        stroke_dasharray: common.stroke_dasharray,
        randomness: common.hand_drawn_seed,
        work_meter: common.work_meter,
    }
}

fn body_svg<'a>(
    common: &'a FlowchartNodeRenderCommon<'_>,
    details: &'a mut FlowchartRenderDetails,
) -> NodeBodySvg<'a> {
    NodeBodySvg {
        geometry: body_geometry(common),
        style: common.style,
        rough_group_style: common.rough_group_style,
        fill_color: common.fill_color,
        stroke_color: common.stroke_color,
        timing: common.timing,
        rough_time: &mut details.node_roughjs,
        rough_calls: &mut details.node_roughjs_calls,
    }
}

pub(super) fn try_render_flowchart_no_label(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeRenderCommon<'_>,
    details: &mut FlowchartRenderDetails,
) -> bool {
    node_body::try_write_no_label(
        out,
        common.shape,
        body_svg(common, details),
        NoLabelStyle {
            junction_style: common.junction_style,
            theme_style: common.theme_style,
        },
        ctx.compatibility,
        ctx.document_ids().scope(),
        crate::flowchart::CROSSED_CIRCLE_RADIUS,
    )
}

fn advance_lean(common: &FlowchartNodeGeometry<'_>, kind: crate::flowchart::LeanKind) {
    let geometry = body_geometry(common);
    if geometry.hand_drawn {
        let lean =
            crate::flowchart::LeanGeometry::from_bounds(kind, geometry.width, geometry.height);
        node_body::advance_lean_hand_drawn(geometry, &lean.points);
    }
}

pub(super) fn advance_rough_geometry(
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::FlowchartNodeGeometry<'_>,
    label: &mut super::FlowchartNodeLabelState<'_>,
) -> crate::Result<()> {
    use crate::flowchart::{LeanKind, resolve_flowchart_shape};
    let shape = resolve_flowchart_shape(common.shape)?;
    match shape {
        FlowchartShape::Anchor
        | FlowchartShape::Choice
        | FlowchartShape::CrossedCircle
        | FlowchartShape::FilledCircle
        | FlowchartShape::ForkJoin
        | FlowchartShape::FramedCircle
        | FlowchartShape::LightningBolt
        | FlowchartShape::SmallCircle => node_body::advance_no_label_rough_geometry(
            shape,
            body_geometry(common),
            crate::flowchart::CROSSED_CIRCLE_RADIUS,
        ),
        FlowchartShape::BowTieRectangle => OuterPathShape::BowTieRectangle
            .prepare(ctx, common, label)
            .advance_rough_geometry(body_geometry(common)),
        FlowchartShape::BraceLeft | FlowchartShape::BraceRight | FlowchartShape::Braces => {
            curly_braces::advance_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Circle | FlowchartShape::Diamond | FlowchartShape::DoubleCircle => {
            basic_geoms::advance_rough_geometry(ctx, common)?
        }
        FlowchartShape::CollapsedGroup => {
            collapsed_group::advance_rough_geometry(ctx, common, label)
        }
        FlowchartShape::CurvedTrapezoid => OuterPathShape::CurvedTrapezoid
            .prepare(ctx, common, label)
            .advance_rough_geometry(body_geometry(common)),
        FlowchartShape::Delay => OuterPathShape::Delay
            .prepare(ctx, common, label)
            .advance_rough_geometry(body_geometry(common)),
        FlowchartShape::Document => {
            document_shapes::advance_wave_document_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Ellipse => {
            return Err(crate::Error::InvalidModel {
                message:
                    "Flowchart ellipse is present in FlowDB but broken in Mermaid 11.16's renderer"
                        .into(),
            });
        }
        FlowchartShape::Hexagon => hexagon::advance_hexagon_rough_geometry(ctx, common, label),
        FlowchartShape::IconCircle => icon_circle::advance_rough_geometry(ctx, common, label),
        FlowchartShape::IconRounded => icon_square::advance_rough_geometry(ctx, common, label, 5.0),
        FlowchartShape::IconSquare => icon_square::advance_rough_geometry(ctx, common, label, 0.1),
        FlowchartShape::ImageSquare => image_square::advance_rough_geometry(ctx, common, label)?,
        FlowchartShape::InvertedTrapezoid => advance_lean(common, LeanKind::InvertedTrapezoid),
        FlowchartShape::LeanLeft => advance_lean(common, LeanKind::Left),
        FlowchartShape::LeanRight => advance_lean(common, LeanKind::Right),
        FlowchartShape::LinedDocument => {
            document_shapes::advance_lined_wave_document_rough_geometry(ctx, common, label)
        }
        FlowchartShape::ManualFile => {
            manuals::advance_manual_file_rough_geometry(ctx, common, label)
        }
        FlowchartShape::ManualInput => {
            manuals::advance_manual_input_rough_geometry(ctx, common, label)
        }
        FlowchartShape::NotchedPentagon => OuterPathShape::NotchedPentagon
            .prepare(ctx, common, label)
            .advance_rough_geometry(body_geometry(common)),
        FlowchartShape::Odd => odd::advance_odd_rough_geometry(ctx, common, label),
        FlowchartShape::Process => {
            RectangleBody::Process.advance_rough_geometry(body_geometry(common), ctx.compatibility)
        }
        FlowchartShape::RoundedRectangle => {
            RectangleBody::Rounded.advance_rough_geometry(body_geometry(common), ctx.compatibility)
        }
        FlowchartShape::StackedDocument => {
            document_shapes::advance_stacked_document_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Stadium => stadium::advance_stadium_rough_geometry(ctx, common, label),
        FlowchartShape::TaggedDocument => {
            document_shapes::advance_tagged_wave_document_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Trapezoid => advance_lean(common, LeanKind::Trapezoid),
        FlowchartShape::Triangle => {
            triangle::advance_triangle_extract_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Cylinder
        | FlowchartShape::Bpmn(_)
        | FlowchartShape::HorizontalCylinder
        | FlowchartShape::LinedCylinder
        | FlowchartShape::Icon => {}
        shape => TerminalNodeBody::for_shape(ctx, common, shape)?
            .advance_rough_geometry(ctx, common, label),
    }
    Ok(())
}
