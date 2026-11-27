//! Flowchart node shape renderers.

use merman_render_flow::node_body::{self, NodeBodyGeometry, NodeBodySvg};

use crate::svg::parity::flowchart::types::{FlowchartRenderCtx, FlowchartRenderDetails};

use super::{FlowchartNodeGeometry, FlowchartNodeLabelState, FlowchartNodeRenderCommon, helpers};

mod basic_geoms;
mod bpmn;
mod browser;
mod bucket;
mod collapsed_group;
mod console;
mod curly_braces;
mod cylinders;
mod dispatch;
mod divided_rect;
mod document_shapes;
mod folder;
mod hexagon;
mod icon;
mod icon_circle;
mod icon_square;
mod image_square;
mod lined_cylinder;
mod manuals;
mod no_label;
mod note;
mod odd;
mod organic;
mod outer_path;
mod paper_tape;
mod person;
mod process;
mod rounded_rect;
mod shaded_process;
mod stacked_rectangle;
mod stadium;
mod tag_rect;
mod triangle;
mod window_pane;

pub(super) use basic_geoms::{render_circle, render_diamond, render_double_circle};
pub(super) use bpmn::render_bpmn;
pub(super) use browser::render_browser;
pub(super) use bucket::render_bucket;
pub(super) use collapsed_group::{render_collapsed_group_body, render_collapsed_group_indicators};
pub(super) use console::render_console;
pub(super) use curly_braces::render_curly_brace_comment;
pub(in crate::svg::parity::flowchart) use curly_braces::{
    curly_brace_comment_geometry, curly_brace_comment_intersection_points,
};
pub(super) use cylinders::{render_cylinder, render_horizontal_cylinder};
pub(super) use dispatch::render_flowchart_shape;
pub(super) use divided_rect::render_divided_rect;
pub(super) use document_shapes::{
    render_lined_wave_document, render_stacked_document, render_tagged_wave_document,
    render_wave_document,
};
pub(super) use folder::render_folder;
pub(super) use hexagon::render_hexagon;
pub(super) use icon::render_icon;
pub(super) use icon_circle::render_icon_circle;
pub(super) use icon_square::{render_icon_rounded, render_icon_square};
pub(super) use image_square::try_render_image_square;
pub(super) use lined_cylinder::render_lined_cylinder;
pub(super) use manuals::{render_manual_file, render_manual_input};
pub(super) use no_label::try_render_flowchart_no_label;
pub(super) use note::render_note;
pub(super) use odd::render_odd;
pub(super) use organic::{render_bang, render_cloud};
pub(super) use paper_tape::render_paper_tape;
pub(super) use person::render_person;
pub(super) use process::render_process_rectangle;
pub(super) use rounded_rect::render_rounded_rect;
pub(super) use shaded_process::render_shaded_process;
pub(super) use stacked_rectangle::render_stacked_rectangle;
pub(super) use stadium::render_stadium;
pub(super) use tag_rect::render_tag_rect;
pub(super) use triangle::render_triangle_extract;
pub(super) use window_pane::render_window_pane;

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
    use crate::flowchart::{FlowchartShape, LeanKind, resolve_flowchart_shape};
    match resolve_flowchart_shape(common.shape)? {
        FlowchartShape::Anchor
        | FlowchartShape::Choice
        | FlowchartShape::CrossedCircle
        | FlowchartShape::FilledCircle
        | FlowchartShape::ForkJoin
        | FlowchartShape::FramedCircle
        | FlowchartShape::LightningBolt
        | FlowchartShape::SmallCircle => no_label::advance_rough_geometry(common),
        FlowchartShape::Bang => organic::advance_rough_geometry(ctx, common, false),
        FlowchartShape::Cloud => organic::advance_rough_geometry(ctx, common, true),
        FlowchartShape::Browser => browser::advance_browser_rough_geometry(ctx, common, label),
        FlowchartShape::BowTieRectangle => OuterPathShape::BowTieRectangle
            .prepare(ctx, common, label)
            .advance_rough_geometry(body_geometry(common)),
        FlowchartShape::BraceLeft | FlowchartShape::BraceRight | FlowchartShape::Braces => {
            curly_braces::advance_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Bucket => bucket::advance_bucket_rough_geometry(ctx, common, label),
        FlowchartShape::Circle | FlowchartShape::Diamond | FlowchartShape::DoubleCircle => {
            basic_geoms::advance_rough_geometry(ctx, common)?
        }
        FlowchartShape::CollapsedGroup => {
            collapsed_group::advance_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Console => console::advance_console_rough_geometry(ctx, common, label),
        FlowchartShape::CurvedTrapezoid => OuterPathShape::CurvedTrapezoid
            .prepare(ctx, common, label)
            .advance_rough_geometry(body_geometry(common)),
        FlowchartShape::Datastore => node_body::advance_datastore(body_geometry(common)),
        FlowchartShape::Delay => OuterPathShape::Delay
            .prepare(ctx, common, label)
            .advance_rough_geometry(body_geometry(common)),
        FlowchartShape::DividedRectangle => {
            divided_rect::advance_divided_rect_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Folder => folder::advance_folder_rough_geometry(ctx, common, label),
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
        FlowchartShape::Hourglass => node_body::advance_hourglass(body_geometry(common)),
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
        FlowchartShape::Note => note::advance_rough_geometry(common),
        FlowchartShape::Odd => odd::advance_odd_rough_geometry(ctx, common, label),
        FlowchartShape::PaperTape => {
            paper_tape::advance_paper_tape_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Person => person::advance_person_rough_geometry(ctx, common, label),
        FlowchartShape::Process => process::advance_rough_geometry(common),
        FlowchartShape::RoundedRectangle => rounded_rect::advance_rough_geometry(ctx, common),
        FlowchartShape::ShadedProcess => {
            shaded_process::advance_shaded_process_rough_geometry(ctx, common, label)
        }
        FlowchartShape::StackedDocument => {
            document_shapes::advance_stacked_document_rough_geometry(ctx, common, label)
        }
        FlowchartShape::StackedRectangle => {
            stacked_rectangle::advance_stacked_rectangle_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Stadium => stadium::advance_stadium_rough_geometry(ctx, common, label),
        FlowchartShape::Subroutine => node_body::advance_subroutine(body_geometry(common)),
        FlowchartShape::TaggedDocument => {
            document_shapes::advance_tagged_wave_document_rough_geometry(ctx, common, label)
        }
        FlowchartShape::TaggedRectangle => {
            tag_rect::advance_tag_rect_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Trapezoid => advance_lean(common, LeanKind::Trapezoid),
        FlowchartShape::Triangle => {
            triangle::advance_triangle_extract_rough_geometry(ctx, common, label)
        }
        FlowchartShape::WindowPane => {
            window_pane::advance_window_pane_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Cylinder
        | FlowchartShape::Bpmn(_)
        | FlowchartShape::HorizontalCylinder
        | FlowchartShape::LinedCylinder
        | FlowchartShape::Icon
        | FlowchartShape::NotchedRectangle
        | FlowchartShape::Text => {}
    }
    Ok(())
}
