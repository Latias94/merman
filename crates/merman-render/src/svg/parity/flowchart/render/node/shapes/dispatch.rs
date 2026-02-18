//! Flowchart node shape dispatch.

use std::fmt::Write as _;

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::fmt;

pub(in super::super) fn render_flowchart_v2_shape(
    out: &mut String,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    shape: &str,
    layout_node: &crate::model::LayoutNode,
    label_text: &mut &str,
    label_type: &mut &str,
    node_classes: &[String],
    node_styles: &[String],
    node_icon: Option<&str>,
    node_img: Option<&str>,
    node_pos: Option<&str>,
    node_constraint: Option<&str>,
    node_asset_width: Option<f64>,
    node_asset_height: Option<f64>,
    style: &str,
    fill_color: &str,
    stroke_color: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    hand_drawn_seed: u64,
    wrapped_in_a: bool,
    timing_enabled: bool,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
    label_dx: &mut f64,
    label_dy: &mut f64,
    compact_label_translate: &mut bool,
) -> bool {
    match shape {
        // Flowchart v2 shapes with no label group are handled earlier.

        // Flowchart v2 hourglass/collate: Mermaid clears `node.label` but still emits an empty
        // label group (via `labelHelper(...)`).
        "hourglass" | "collate" => {
            *label_text = "";
            *label_type = "text";
            super::render_hourglass_collate(
                out,
                layout_node,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }

        // Flowchart v2 card/notched-rectangle.
        "notch-rect" | "notched-rectangle" | "card" => {
            super::render_notched_rectangle(out, layout_node);
        }

        // Flowchart v2 delay / half-rounded rectangle.
        "delay" => {
            super::render_delay(
                out,
                ctx,
                *label_text,
                *label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }

        // Flowchart v2 lined cylinder (Disk storage).
        "lin-cyl" => {
            super::render_lined_cylinder(out, layout_node, label_dy);
        }

        // Flowchart v2 curved trapezoid (Display).
        "curv-trap" => {
            super::render_curved_trapezoid(
                out,
                ctx,
                layout_node,
                *label_text,
                *label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }

        // Flowchart v2 divided rectangle (Divided process).
        "div-rect" => {
            super::render_divided_rect(
                out,
                layout_node,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
                label_dy,
            );
        }

        // Flowchart v2 notched pentagon (Loop limit).
        "notch-pent" => {
            super::render_notched_pentagon(
                out,
                ctx,
                *label_text,
                *label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }

        // Flowchart v2 bow tie rectangle (Stored data).
        "bow-rect" => {
            super::render_bow_tie_rect(
                out,
                ctx,
                *label_text,
                *label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }

        // Flowchart v2 tagged rectangle (Tagged process).
        "tag-rect" => {
            super::render_tag_rect(
                out,
                ctx,
                layout_node,
                *label_text,
                *label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }

        // Flowchart v2 wave edged rectangle (Document).
        "doc" => {
            *compact_label_translate = true;
            super::render_wave_document(
                out,
                ctx,
                layout_node,
                *label_text,
                *label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
                label_dx,
                label_dy,
            );
        }

        // Flowchart v2 lined wave edged rectangle (Lined document).
        "lin-doc" => {
            *compact_label_translate = true;
            super::render_lined_wave_document(
                out,
                ctx,
                layout_node,
                *label_text,
                *label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
                label_dx,
                label_dy,
            );
        }

        // Flowchart v2 tagged wave edged rectangle (Tagged document).
        "tag-doc" => {
            *compact_label_translate = true;
            super::render_tagged_wave_document(
                out,
                ctx,
                layout_node,
                *label_text,
                *label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
                label_dx,
                label_dy,
            );
        }

        // Flowchart v2 triangle (Extract).
        "tri" => {
            super::render_triangle_extract(
                out,
                ctx,
                *label_text,
                *label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
                label_dy,
            );
        }

        // Flowchart v2 shaded process / lined rectangle.
        "lin-rect" | "lined-rectangle" | "lined-process" | "lin-proc" => {
            *label_dx = 4.0;
            *compact_label_translate = true;
            super::render_shaded_process(
                out,
                layout_node,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }

        // Flowchart v2 curly brace/comment shapes (rendering-elements).
        "comment" | "brace" | "brace-l" | "brace-r" | "braces" => {
            super::render_curly_brace_comment(
                out,
                shape,
                layout_node,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
                compact_label_translate,
                label_dx,
            );
        }

        "imageSquare" => {
            if super::try_render_image_square(
                out,
                ctx,
                layout_node,
                *label_text,
                *label_type,
                node_pos,
                node_img,
                node_asset_height,
                node_asset_width,
                node_constraint,
                style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                wrapped_in_a,
                timing_enabled,
                details,
            ) {
                return true;
            }
        }
        "iconSquare" => {
            if super::try_render_icon_square(
                out,
                ctx,
                *label_text,
                *label_type,
                node_icon,
                node_pos,
                node_asset_width,
                node_asset_height,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                wrapped_in_a,
            ) {
                return true;
            }
        }
        "manual-file" | "flipped-triangle" | "flip-tri" => {
            super::render_manual_file(
                out,
                layout_node,
                style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }
        "manual-input" | "sloped-rectangle" | "sl-rect" => {
            super::render_manual_input(
                out,
                layout_node,
                style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }
        "docs" | "documents" | "st-doc" | "stacked-document" => {
            super::render_stacked_document(
                out,
                layout_node,
                style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }
        "procs" | "processes" | "st-rect" | "stacked-rectangle" => {
            super::render_stacked_rectangle(
                out,
                layout_node,
                style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }
        "paper-tape" | "flag" => {
            super::render_paper_tape(
                out,
                layout_node,
                style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }
        "subroutine" | "fr-rect" | "subproc" | "subprocess" => {
            super::render_subroutine(out, layout_node, style);
        }
        "cylinder" | "cyl" => {
            super::render_cylinder(out, ctx, layout_node, style, label_dy);
        }
        "h-cyl" | "das" | "horizontal-cylinder" => {
            super::render_horizontal_cylinder(out, layout_node, style, label_dx);
        }
        "win-pane" | "internal-storage" | "window-pane" => {
            super::render_window_pane(
                out,
                layout_node,
                style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                label_dx,
                label_dy,
            );
        }
        "diamond" | "question" | "diam" => {
            super::render_diamond(out, layout_node, style);
        }
        "circle" => {
            super::render_circle(out, layout_node, style);
        }
        "doublecircle" | "dbl-circ" | "double-circle" => {
            super::render_double_circle(out, layout_node, style);
        }
        "roundedRect" | "rounded" => {
            super::render_rounded_rect(
                out,
                layout_node,
                style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }
        "stadium" => {
            super::render_stadium(
                out,
                ctx,
                *label_text,
                *label_type,
                node_classes,
                style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }
        "hexagon" | "hex" => {
            super::render_hexagon(
                out,
                layout_node,
                style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }
        "lean_right" | "lean-r" | "lean-right" => {
            super::render_lean_right(out, layout_node, style);
        }
        "lean_left" | "lean-l" | "lean-left" => {
            super::render_lean_left(out, layout_node, style);
        }
        "trapezoid" | "trap-b" => {
            super::render_trapezoid(out, layout_node, style);
        }
        "inv_trapezoid" | "inv-trapezoid" | "trap-t" => {
            super::render_inv_trapezoid(out, layout_node, style);
        }
        "odd" => {
            super::render_odd(
                out,
                layout_node,
                style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
                label_dx,
            );
        }
        "text" => {
            // Mermaid `text.ts`: invisible rect used only to size/position the label.
            let w = layout_node.width.max(0.0);
            let h = layout_node.height.max(0.0);
            let _ = write!(
                out,
                r#"<rect class="text" style="{}" rx="0" ry="0" x="{}" y="{}" width="{}" height="{}"/>"#,
                escape_attr(style),
                fmt(-w / 2.0),
                fmt(-h / 2.0),
                fmt(w),
                fmt(h)
            );
        }
        _ => {
            let w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let _ = write!(
                out,
                r#"<rect class="basic label-container" style="{}" x="{}" y="{}" width="{}" height="{}"/>"#,
                escape_attr(style),
                fmt(-w / 2.0),
                fmt(-h / 2.0),
                fmt(w),
                fmt(h)
            );
        }
    }

    false
}
