//! Flowchart node shape dispatch.

use merman_render_flow::node_body::{RectangleBody, RectangleStyle};

use crate::flowchart::{FlowchartShape, LeanGeometry, LeanKind, resolve_flowchart_shape};
use crate::{Error, Result};

use super::{OuterPathShape, TerminalNodeBody, body_svg, node_body, stadium_geometry};

pub(in super::super) fn render_flowchart_shape(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    label: &mut super::super::FlowchartNodeLabelState<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) -> Result<bool> {
    let resolved_shape = resolve_flowchart_shape(common.shape)?;

    match resolved_shape {
        FlowchartShape::Anchor
        | FlowchartShape::Choice
        | FlowchartShape::CrossedCircle
        | FlowchartShape::FilledCircle
        | FlowchartShape::ForkJoin
        | FlowchartShape::FramedCircle
        | FlowchartShape::LightningBolt
        | FlowchartShape::SmallCircle => {
            return Err(Error::InvalidModel {
                message: format!(
                    "Flowchart shape {} ({resolved_shape:?}) bypassed its no-label renderer",
                    common.shape
                ),
            });
        }
        FlowchartShape::Bpmn(shape) => {
            super::render_bpmn(out, ctx, common, label, shape)?;
        }
        FlowchartShape::BowTieRectangle => OuterPathShape::BowTieRectangle
            .prepare(ctx, common, label)
            .write(out, body_svg(common, details)),
        FlowchartShape::BraceLeft | FlowchartShape::BraceRight | FlowchartShape::Braces => {
            // The shared renderer uses the original spelling to select left/right/both braces.
            super::render_curly_brace_comment(out, ctx, common, label, details);
        }
        FlowchartShape::Circle => {
            node_body::write_circle(
                out,
                body_svg(common, details),
                common.node_styles,
                !common.effect_filter_style.is_empty(),
            )?;
        }
        FlowchartShape::CollapsedGroup => {
            return Err(Error::InvalidModel {
                message: "collapsedGroup must use the ordered node renderer".to_string(),
            });
        }
        FlowchartShape::CurvedTrapezoid => OuterPathShape::CurvedTrapezoid
            .prepare(ctx, common, label)
            .write(out, body_svg(common, details)),
        FlowchartShape::Cylinder => {
            super::render_cylinder(out, ctx, common, label, details);
        }
        FlowchartShape::Delay => OuterPathShape::Delay
            .prepare(ctx, common, label)
            .write(out, body_svg(common, details)),
        FlowchartShape::Diamond => {
            node_body::write_diamond(
                out,
                body_svg(common, details),
                common.centered_diamond,
                !common.effect_filter_style.is_empty(),
            );
        }
        FlowchartShape::Document => {
            super::render_wave_document(out, ctx, common, label, details);
        }
        FlowchartShape::DoubleCircle => {
            node_body::write_double_circle(
                out,
                body_svg(common, details),
                common.look_is_neo(),
                common.node_classes,
                common.node_styles,
                !common.effect_filter_style.is_empty(),
            )?;
        }
        FlowchartShape::Ellipse => {
            return Err(Error::InvalidModel {
                message:
                    "Flowchart ellipse is present in FlowDB but broken in Mermaid 11.16's renderer"
                        .to_string(),
            });
        }
        FlowchartShape::Hexagon => {
            node_body::write_hexagon(out, body_svg(common, details), common.look_is_neo());
        }
        FlowchartShape::HorizontalCylinder => {
            super::render_horizontal_cylinder(out, ctx, common, label, details);
        }
        FlowchartShape::Icon => {
            super::render_icon(out, ctx, common, label, details)?;
            return Ok(true);
        }
        FlowchartShape::IconCircle => {
            super::render_icon_circle(out, ctx, common, label, details)?;
            return Ok(true);
        }
        FlowchartShape::IconRounded => {
            super::render_icon_rounded(out, ctx, common, label, details)?;
            return Ok(true);
        }
        FlowchartShape::IconSquare => {
            super::render_icon_square(out, ctx, common, label, details)?;
            return Ok(true);
        }
        FlowchartShape::ImageSquare => {
            if super::try_render_image_square(out, ctx, common, label, details)? {
                return Ok(true);
            }
            return missing_asset_error(common.shape, "image");
        }
        FlowchartShape::InvertedTrapezoid => {
            render_lean(out, common, details, LeanKind::InvertedTrapezoid);
        }
        FlowchartShape::LeanLeft => {
            render_lean(out, common, details, LeanKind::Left);
        }
        FlowchartShape::LeanRight => {
            render_lean(out, common, details, LeanKind::Right);
        }
        FlowchartShape::LinedCylinder => {
            super::render_lined_cylinder(out, ctx, common, label, details);
        }
        FlowchartShape::LinedDocument => {
            super::render_lined_wave_document(out, ctx, common, label, details);
        }
        FlowchartShape::ManualFile => {
            super::render_manual_file(out, ctx, common, label, details);
        }
        FlowchartShape::ManualInput => {
            super::render_manual_input(out, ctx, common, label, details);
        }
        FlowchartShape::NotchedPentagon => OuterPathShape::NotchedPentagon
            .prepare(ctx, common, label)
            .write(out, body_svg(common, details)),
        FlowchartShape::Odd => {
            node_body::write_odd(out, body_svg(common, details), &mut label.dx);
        }
        FlowchartShape::Process => {
            render_rectangle(out, ctx, common, details, RectangleBody::Process);
        }
        FlowchartShape::RoundedRectangle => {
            render_rectangle(out, ctx, common, details, RectangleBody::Rounded);
        }
        FlowchartShape::StackedDocument => {
            super::render_stacked_document(out, ctx, common, label, details);
        }
        FlowchartShape::Stadium => {
            let stadium = stadium_geometry(ctx, common, label);
            node_body::write_stadium(out, body_svg(common, details), stadium);
        }
        FlowchartShape::TaggedDocument => {
            super::render_tagged_wave_document(out, ctx, common, label, details);
        }
        FlowchartShape::Trapezoid => {
            render_lean(out, common, details, LeanKind::Trapezoid);
        }
        FlowchartShape::Triangle => {
            super::render_triangle_extract(out, ctx, common, label, details);
        }
        shape => {
            TerminalNodeBody::for_shape(ctx, common, shape)?
                .write(out, ctx, common, label, details);
        }
    }

    Ok(false)
}

fn render_rectangle(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
    rectangle: RectangleBody,
) {
    let rect = ctx.rectangle_geometry(common.layout_node).shape;
    rectangle.write(
        out,
        body_svg(common, details),
        rect,
        RectangleStyle {
            source_radii: common.source_corner_radii,
            native_radius: common.native_corner_radius,
            configured_radius: common.configured_corner_radius,
            effect_filter_style: common.effect_filter_style,
        },
        ctx.compatibility,
    );
}

fn render_lean(
    out: &mut impl crate::svg::parity::SvgOutput,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
    kind: LeanKind,
) {
    let geometry =
        LeanGeometry::from_bounds(kind, common.layout_node.width, common.layout_node.height);
    node_body::write_lean(
        out,
        body_svg(common, details),
        &geometry.points,
        geometry.translate_x,
        geometry.translate_y,
    );
}

fn missing_asset_error<T>(shape: &str, asset: &str) -> Result<T> {
    Err(Error::InvalidModel {
        message: format!("Flowchart {shape} node is missing its {asset} source"),
    })
}
