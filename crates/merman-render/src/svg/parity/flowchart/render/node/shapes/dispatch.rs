//! Flowchart node shape dispatch.

use crate::flowchart::{FlowchartShape, LeanGeometry, LeanKind, resolve_flowchart_shape};
use crate::{Error, Result};

use super::{OuterPathShape, TerminalNodeBody, body_svg, node_body};

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
            super::render_circle(out, ctx, common, details)?;
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
            super::render_diamond(out, common, details);
        }
        FlowchartShape::Document => {
            super::render_wave_document(out, ctx, common, label, details);
        }
        FlowchartShape::DoubleCircle => {
            super::render_double_circle(out, ctx, common, details)?;
        }
        FlowchartShape::Ellipse => {
            return Err(Error::InvalidModel {
                message:
                    "Flowchart ellipse is present in FlowDB but broken in Mermaid 11.16's renderer"
                        .to_string(),
            });
        }
        FlowchartShape::Hexagon => {
            super::render_hexagon(out, common, details);
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
            super::render_odd(out, common, label, details);
        }
        FlowchartShape::Process => {
            super::render_process_rectangle(out, ctx, common, details);
        }
        FlowchartShape::RoundedRectangle => {
            super::render_rounded_rect(out, ctx, common, details);
        }
        FlowchartShape::StackedDocument => {
            super::render_stacked_document(out, ctx, common, label, details);
        }
        FlowchartShape::Stadium => {
            super::render_stadium(out, ctx, common, label, details);
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
