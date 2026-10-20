//! Flowchart node shape renderers.

mod basic_geoms;
mod bow_tie_rect;
mod browser;
mod bucket;
mod collapsed_group;
mod console;
mod curly_braces;
mod curved_trapezoid;
mod cylinders;
mod datastore;
mod delay;
mod dispatch;
mod divided_rect;
mod folder;
mod hexagon;
mod icon;
mod icon_circle;
mod icon_square;
mod image_square;
mod label_container;
mod lean_trapezoids;
mod lined_cylinder;
mod lined_wave_document;
mod manuals;
mod no_label;
mod notched_pentagon;
mod note;
mod odd;
mod organic;
mod paper_tape;
mod person;
mod process;
mod rounded_rect;
mod shaded_process;
mod stacked_document;
mod stacked_rectangle;
mod stadium;
mod subroutine;
mod tag_rect;
mod tagged_wave_document;
mod triangle;
mod wave_document;
mod window_pane;

pub(super) use basic_geoms::{render_circle, render_diamond, render_double_circle};
pub(super) use bow_tie_rect::render_bow_tie_rect;
pub(super) use browser::render_browser;
pub(super) use bucket::render_bucket;
pub(super) use collapsed_group::{render_collapsed_group_body, render_collapsed_group_indicators};
pub(super) use console::render_console;
pub(super) use curly_braces::render_curly_brace_comment;
pub(in crate::svg::parity::flowchart) use curly_braces::{
    curly_brace_comment_geometry, curly_brace_comment_intersection_points,
};
pub(super) use curved_trapezoid::render_curved_trapezoid;
pub(super) use cylinders::{render_cylinder, render_horizontal_cylinder};
pub(super) use datastore::render_datastore;
pub(super) use delay::render_delay;
pub(super) use dispatch::render_flowchart_shape;
pub(super) use divided_rect::render_divided_rect;
pub(super) use folder::render_folder;
pub(super) use hexagon::render_hexagon;
pub(super) use icon::render_icon;
pub(super) use icon_circle::render_icon_circle;
pub(super) use icon_square::{render_icon_rounded, render_icon_square};
pub(super) use image_square::try_render_image_square;
pub(super) use label_container::{render_hourglass_collate, render_notched_rectangle};
pub(super) use lean_trapezoids::{
    render_inv_trapezoid, render_lean_left, render_lean_right, render_trapezoid,
};
pub(super) use lined_cylinder::render_lined_cylinder;
pub(super) use lined_wave_document::render_lined_wave_document;
pub(super) use manuals::{render_manual_file, render_manual_input};
pub(super) use no_label::try_render_flowchart_no_label;
pub(super) use notched_pentagon::render_notched_pentagon;
pub(super) use note::render_note;
pub(super) use odd::render_odd;
pub(super) use organic::{render_bang, render_cloud};
pub(super) use paper_tape::render_paper_tape;
pub(super) use person::render_person;
pub(super) use process::render_process_rectangle;
pub(super) use rounded_rect::render_rounded_rect;
pub(super) use shaded_process::render_shaded_process;
pub(super) use stacked_document::render_stacked_document;
pub(super) use stacked_rectangle::render_stacked_rectangle;
pub(super) use stadium::render_stadium;
pub(super) use subroutine::render_subroutine;
pub(super) use tag_rect::render_tag_rect;
pub(super) use tagged_wave_document::render_tagged_wave_document;
pub(super) use triangle::render_triangle_extract;
pub(super) use wave_document::render_wave_document;
pub(super) use window_pane::render_window_pane;

pub(super) fn advance_rough_geometry(
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::FlowchartNodeGeometry<'_>,
    label: &mut super::FlowchartNodeLabelState<'_>,
) -> crate::Result<()> {
    use crate::flowchart::{FlowchartShape, LeanKind};
    match FlowchartShape::resolve(common.shape)? {
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
        FlowchartShape::BowTieRectangle => {
            bow_tie_rect::advance_bow_tie_rect_rough_geometry(ctx, common, label)
        }
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
        FlowchartShape::CurvedTrapezoid => {
            curved_trapezoid::advance_curved_trapezoid_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Datastore => datastore::advance_rough_geometry(common),
        FlowchartShape::Delay => delay::advance_delay_rough_geometry(ctx, common, label),
        FlowchartShape::DividedRectangle => {
            divided_rect::advance_divided_rect_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Folder => folder::advance_folder_rough_geometry(ctx, common, label),
        FlowchartShape::Document => {
            wave_document::advance_wave_document_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Ellipse => {
            return Err(crate::Error::InvalidModel {
                message:
                    "Flowchart ellipse is present in FlowDB but broken in Mermaid 11.16's renderer"
                        .into(),
            });
        }
        FlowchartShape::Hexagon => hexagon::advance_hexagon_rough_geometry(ctx, common, label),
        FlowchartShape::Hourglass => label_container::advance_hourglass_rough_geometry(common),
        FlowchartShape::IconCircle => icon_circle::advance_rough_geometry(ctx, common, label),
        FlowchartShape::IconRounded => icon_square::advance_rough_geometry(ctx, common, label, 5.0),
        FlowchartShape::IconSquare => icon_square::advance_rough_geometry(ctx, common, label, 0.1),
        FlowchartShape::ImageSquare => image_square::advance_rough_geometry(ctx, common, label),
        FlowchartShape::InvertedTrapezoid => {
            lean_trapezoids::advance_rough_geometry(common, LeanKind::InvertedTrapezoid)
        }
        FlowchartShape::LeanLeft => lean_trapezoids::advance_rough_geometry(common, LeanKind::Left),
        FlowchartShape::LeanRight => {
            lean_trapezoids::advance_rough_geometry(common, LeanKind::Right)
        }
        FlowchartShape::LinedDocument => {
            lined_wave_document::advance_lined_wave_document_rough_geometry(ctx, common, label)
        }
        FlowchartShape::ManualFile => {
            manuals::advance_manual_file_rough_geometry(ctx, common, label)
        }
        FlowchartShape::ManualInput => {
            manuals::advance_manual_input_rough_geometry(ctx, common, label)
        }
        FlowchartShape::NotchedPentagon => {
            notched_pentagon::advance_notched_pentagon_rough_geometry(ctx, common, label)
        }
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
            stacked_document::advance_stacked_document_rough_geometry(ctx, common, label)
        }
        FlowchartShape::StackedRectangle => {
            stacked_rectangle::advance_stacked_rectangle_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Stadium => stadium::advance_stadium_rough_geometry(ctx, common, label),
        FlowchartShape::TaggedDocument => {
            tagged_wave_document::advance_tagged_wave_document_rough_geometry(ctx, common, label)
        }
        FlowchartShape::TaggedRectangle => {
            tag_rect::advance_tag_rect_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Trapezoid => {
            lean_trapezoids::advance_rough_geometry(common, LeanKind::Trapezoid)
        }
        FlowchartShape::Triangle => {
            triangle::advance_triangle_extract_rough_geometry(ctx, common, label)
        }
        FlowchartShape::WindowPane => {
            window_pane::advance_window_pane_rough_geometry(ctx, common, label)
        }
        FlowchartShape::Cylinder
        | FlowchartShape::HorizontalCylinder
        | FlowchartShape::LinedCylinder
        | FlowchartShape::Icon
        | FlowchartShape::NotchedRectangle
        | FlowchartShape::Subroutine
        | FlowchartShape::Text => {}
    }
    Ok(())
}
