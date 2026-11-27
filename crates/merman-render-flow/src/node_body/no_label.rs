//! Flowchart v2 node shapes that do not emit a label group.

use merman_model::OperationPhase;
use merman_render_core::{
    rough_svg::{
        RoughRectSpec, closed_path_d_from_points as path_from_points,
        paths::{RoughGeometryTarget, roughjs_prepare_paths_for_svg_path},
        roughjs_circle_operations, roughjs_circle_path_d, roughjs_paths_for_rect,
        roughjs_rect_operations,
    },
    svg_output::SvgOutput,
    xml::escape_attr_display,
};

use crate::{
    document::FlowchartDocumentScope, shapes::FlowchartShape, theme::FlowchartCompatibilityBinding,
};

use super::{NodeBodyGeometry, NodeBodySvg, escape_attr, fmt};

// stateStart.ts/stateEnd.ts apply the small filter independently of the general node filter.
fn write_small_state_shadow_attr(
    out: &mut impl SvgOutput,
    shadow_id: impl std::fmt::Display,
    width: f64,
    look: &str,
    node_shadow: bool,
    theme_style: &str,
) {
    let small_shadow = width < 25.0 && node_shadow && look != "handDrawn";
    if small_shadow || !theme_style.is_empty() {
        out.push_str(r#" style=""#);
        if small_shadow {
            let _ = write!(out, "filter:url(#{})", escape_attr_display(shadow_id));
        }
        if !theme_style.is_empty() {
            if small_shadow {
                out.push(';');
            }
            let _ = write!(out, "{}", escape_attr(theme_style));
        }
        out.push('"');
    }
}

/// Source-resolved declarations for the junction and start shapes.
pub struct NoLabelStyle<'a> {
    pub junction_style: &'a str,
    pub theme_style: &'a str,
}

/// Writes a balanced shape body for the Mermaid aliases that skip label emission.
pub fn try_write_no_label(
    out: &mut impl SvgOutput,
    shape: &str,
    mut body: NodeBodySvg<'_>,
    style: NoLabelStyle<'_>,
    compatibility: &FlowchartCompatibilityBinding,
    scope: &FlowchartDocumentScope,
    crossed_circle_radius: f64,
) -> bool {
    match shape {
        // Flowchart v2 anchor: a tiny dot used as an invisible anchor node. Mermaid ignores
        // `node.label` and does not emit a label group.
        "anchor" => {
            let d = body
                .timed_rough(|geometry| roughjs_circle_path_d(2.0, geometry.randomness))
                .unwrap_or_else(|| "M0,0".to_string());
            let _ = write!(
                out,
                r##"<g class="anchor" style=""><path d="{}" stroke="none" stroke-width="0" fill="black"/></g>"##,
                escape_attr(&d),
            );
            true
        }
        // Flowchart v2 "rendering-elements" aliases for state diagram start/end nodes.
        // Mermaid ignores `node.label` for these shapes and does not emit a label group.
        "sm-circ" | "small-circle" | "start" => {
            out.push_str(r#"<circle class="state-start" r="7" width="14" height="14""#);
            write_small_state_shadow_attr(
                out,
                scope.drop_shadow_small(),
                body.geometry.width,
                compatibility.look.as_str(),
                compatibility.small_state_shadow,
                style.theme_style,
            );
            out.push_str("/>");
            true
        }
        "fr-circ" | "framed-circle" | "stop" => {
            let line_color = &compatibility.line_color;
            let inner_fill = &compatibility.state_inner_fill;

            let outer_d = body
                .timed_rough(|geometry| roughjs_circle_path_d(14.0, geometry.randomness))
                .unwrap_or_else(|| "M0,0".to_string());
            let inner_d = body
                .timed_rough(|geometry| roughjs_circle_path_d(5.0, geometry.randomness))
                .unwrap_or_else(|| "M0,0".to_string());

            out.push_str(r#"<g class="outer-path""#);
            write_small_state_shadow_attr(
                out,
                scope.drop_shadow_small(),
                body.geometry.width,
                compatibility.look.as_str(),
                compatibility.small_state_shadow,
                "",
            );
            out.push('>');
            let _ = write!(
                out,
                r##"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="2" fill="none" stroke-dasharray="{}" style="{}"/><g><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="2" fill="none" stroke-dasharray="{}" style="{}"/></g></g>"##,
                outer_d,
                escape_attr(body.fill_color),
                escape_attr(body.style),
                outer_d,
                escape_attr(line_color),
                escape_attr(body.geometry.stroke_dasharray),
                escape_attr(body.style),
                inner_d,
                escape_attr(inner_fill),
                escape_attr(body.style),
                inner_d,
                escape_attr(inner_fill),
                escape_attr(body.geometry.stroke_dasharray),
                escape_attr(body.style),
            );
            true
        }
        // Flowchart v2 fork/join (no label; uses `lineColor` fill/stroke).
        "fork" | "join" => {
            // Mermaid inflates Dagre dimensions after `updateNodeBounds(...)` but does not
            // re-render the bar at the inflated size. Render the canonical shape dimensions.
            let (w, h) = fork_dimensions(&body.geometry);
            let line_color = &compatibility.line_color;
            let (fill_d, stroke_d) = body
                .timed_rough(|geometry| {
                    roughjs_paths_for_rect(RoughRectSpec {
                        x: -w / 2.0,
                        y: -h / 2.0,
                        w,
                        h,
                        stroke_width: geometry.stroke_width,
                        work_meter: geometry.work_meter,
                        phase: OperationPhase::Emit,
                        randomness: geometry.randomness,
                    })
                })
                .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));
            let _ = write!(
                out,
                r##"<g><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/></g>"##,
                fill_d,
                escape_attr(line_color),
                escape_attr(body.style),
                stroke_d,
                escape_attr(line_color),
                fmt(body.geometry.stroke_width as f64),
                escape_attr(body.geometry.stroke_dasharray),
                escape_attr(body.style),
            );
            true
        }
        // Flowchart v2 "rendering-elements" alias for state diagram choice pseudo-state.
        // Mermaid ignores `node.label` and does not emit a label group.
        "choice" => {
            // These path data strings match Mermaid's flowchart-v2 output (via the
            // rendering-elements/state pipeline) at 11.12.2.
            let fill_d = r#"M0 14 C3.0797827916219833 10.920217208378016, 6.159565583243967 7.840434416756033, 14 0 C9.445017992146312 -4.554982007853687, 4.890035984292625 -9.109964015707375, 0 -14 C-4.69590768981725 -9.30409231018275, -9.3918153796345 -4.608184620365501, -14 0 C-10.594632213003933 3.4053677869960666, -7.189264426007867 6.810735573992133, 0 14"#;
            let stroke_d = r#"M0 14 C2.800062938220799 11.1999370617792, 5.600125876441598 8.399874123558401, 14 0 M0 14 C3.0989264858886605 10.901073514111339, 6.197852971777321 7.802147028222679, 14 0 M14 0 C10.954967711679636 -3.045032288320363, 7.909935423359274 -6.090064576640726, 0 -14 M14 0 C10.242459432967006 -3.757540567032993, 6.484918865934014 -7.515081134065986, 0 -14 M0 -14 C-3.0194146709516647 -10.980585329048335, -6.038829341903329 -7.961170658096671, -14 0 M0 -14 C-5.262776161544025 -8.737223838455975, -10.52555232308805 -3.47444767691195, -14 0 M-14 0 C-9.98466955255717 4.01533044744283, -5.96933910511434 8.03066089488566, 0 14 M-14 0 C-8.907272248156367 5.092727751843632, -3.8145444963127364 10.185455503687264, 0 14"#;

            let _ = write!(
                out,
                r##"<g><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/></g>"##,
                fill_d,
                escape_attr(body.fill_color),
                escape_attr(body.style),
                stroke_d,
                escape_attr(body.stroke_color),
                fmt(body.geometry.stroke_width as f64),
                escape_attr(body.geometry.stroke_dasharray),
                escape_attr(body.style),
            );
            true
        }
        // Flowchart v2 lightning bolt (Communication link). Mermaid clears `node.label` and does
        // not emit a label group.
        "bolt" | "com-link" | "lightning-bolt" => {
            // Mermaid uses `width = max(35, node.width)` and `height = max(35, node.height)`,
            // then draws a 2*height tall bolt and translates it by `(-width/2, -height)`.
            let (width, height, path_data) = bolt_geometry(&body.geometry);
            let (fill_d, stroke_d) = body
                .solid_path_pair(&path_data)
                .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));
            let _ = write!(
                out,
                r#"<g class="outer-path" transform="translate({},{})"><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/></g>"#,
                fmt(-width / 2.0),
                fmt(-height),
                escape_attr(&fill_d),
                escape_attr(body.fill_color),
                escape_attr(body.style),
                escape_attr(&stroke_d),
                escape_attr(body.stroke_color),
                fmt(body.geometry.stroke_width as f64),
                escape_attr(body.geometry.stroke_dasharray),
                escape_attr(body.style),
            );
            true
        }
        // Flowchart v2 filled circle (junction). Mermaid clears `node.label` and does not emit a
        // label group. Note that even in non-handDrawn mode Mermaid still uses RoughJS circle
        // paths (roughness=0), which have a slightly asymmetric bbox in Chromium.
        "f-circ" | "junction" | "filled-circle" => {
            let effective_style = style.junction_style;

            let d = body
                .timed_rough(|geometry| roughjs_circle_path_d(14.0, geometry.randomness))
                .unwrap_or_else(|| "M0,0".into());
            let _ = write!(
                out,
                r##"<g><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/></g>"##,
                escape_attr(&d),
                escape_attr(body.fill_color),
                escape_attr(effective_style),
                escape_attr(&d),
                escape_attr(body.stroke_color),
                fmt(body.geometry.stroke_width as f64),
                escape_attr(body.geometry.stroke_dasharray),
                escape_attr(effective_style),
            );
            true
        }
        // Flowchart v2 crossed circle (summary). Mermaid clears `node.label` and does not emit a
        // label group.
        "cross-circ" | "summary" | "crossed-circle" => {
            let circle_d = body
                .timed_rough(|geometry| {
                    roughjs_circle_path_d(crossed_circle_radius * 2.0, geometry.randomness)
                })
                .unwrap_or_else(|| "M0,0".into());

            let line_path = crossed_circle_line_path(crossed_circle_radius);
            let (line_fill_d, line_stroke_d) = body
                .solid_path_pair(&line_path)
                .unwrap_or_else(|| ("".to_string(), "M0,0".to_string()));

            let _ = write!(
                out,
                r##"<g class="outer-path"><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/><g><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/></g></g>"##,
                escape_attr(&circle_d),
                escape_attr(body.fill_color),
                escape_attr(body.style),
                escape_attr(&circle_d),
                escape_attr(body.stroke_color),
                fmt(body.geometry.stroke_width as f64),
                escape_attr(body.geometry.stroke_dasharray),
                escape_attr(body.style),
                escape_attr(&line_fill_d),
                escape_attr(body.fill_color),
                escape_attr(body.style),
                escape_attr(&line_stroke_d),
                escape_attr(body.stroke_color),
                fmt(body.geometry.stroke_width as f64),
                escape_attr(body.geometry.stroke_dasharray),
                escape_attr(body.style),
            );
            true
        }
        _ => false,
    }
}

fn fork_dimensions(geometry: &NodeBodyGeometry<'_>) -> (f64, f64) {
    if geometry.width >= geometry.height {
        (70.0, 10.0)
    } else {
        (10.0, 70.0)
    }
}

fn bolt_geometry(geometry: &NodeBodyGeometry<'_>) -> (f64, f64, String) {
    let width = geometry.width.max(35.0);
    let height = (geometry.height / 2.0).max(35.0);
    let gap = 7.0;

    let points: Vec<(f64, f64)> = vec![
        (width, 0.0),
        (0.0, height + gap / 2.0),
        (width - 2.0 * gap, height + gap / 2.0),
        (0.0, 2.0 * height),
        (width, height - gap / 2.0),
        (2.0 * gap, height - gap / 2.0),
    ];
    let path_data = path_from_points(&points);
    (width, height, path_data)
}

fn crossed_circle_line_path(radius: f64) -> String {
    // Port of Mermaid `createLine(r)` in `crossedCircle.ts`.
    let x_axis_45 = (std::f64::consts::PI / 4.0).cos();
    let y_axis_45 = (std::f64::consts::PI / 4.0).sin();
    let point_q1 = (radius * x_axis_45, radius * y_axis_45);
    let point_q2 = (-radius * x_axis_45, radius * y_axis_45);
    let point_q3 = (-radius * x_axis_45, -radius * y_axis_45);
    let point_q4 = (radius * x_axis_45, -radius * y_axis_45);
    format!(
        "M {},{} L {},{} M {},{} L {},{}",
        point_q2.0,
        point_q2.1,
        point_q4.0,
        point_q4.1,
        point_q1.0,
        point_q1.1,
        point_q3.0,
        point_q3.1
    )
}

/// Advances the same RoughJS stream for layout/math queries without an SVG sink or scope.
pub fn advance_no_label_rough_geometry(
    shape: FlowchartShape,
    geometry: NodeBodyGeometry<'_>,
    crossed_circle_radius: f64,
) {
    match shape {
        FlowchartShape::Anchor => {
            let _ = roughjs_circle_operations(2.0, geometry.randomness);
        }
        FlowchartShape::FramedCircle => {
            let _ = roughjs_circle_operations(14.0, geometry.randomness);
            let _ = roughjs_circle_operations(5.0, geometry.randomness);
        }
        FlowchartShape::ForkJoin => {
            let (w, h) = fork_dimensions(&geometry);
            let _ = roughjs_rect_operations(RoughRectSpec {
                x: -w / 2.0,
                y: -h / 2.0,
                w,
                h,
                stroke_width: geometry.stroke_width,
                work_meter: geometry.work_meter,
                phase: OperationPhase::Layout,
                randomness: geometry.randomness,
            });
        }
        FlowchartShape::LightningBolt => {
            let (_, _, path) = bolt_geometry(&geometry);
            let _ = roughjs_prepare_paths_for_svg_path(
                RoughGeometryTarget::Query,
                &path,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                geometry.work_meter,
                geometry.randomness,
            );
        }
        FlowchartShape::FilledCircle => {
            let _ = roughjs_circle_operations(14.0, geometry.randomness);
        }
        FlowchartShape::CrossedCircle => {
            let _ = roughjs_circle_operations(crossed_circle_radius * 2.0, geometry.randomness);
            let _ = roughjs_prepare_paths_for_svg_path(
                RoughGeometryTarget::Query,
                &crossed_circle_line_path(crossed_circle_radius),
                geometry.stroke_width,
                geometry.stroke_dasharray,
                geometry.work_meter,
                geometry.randomness,
            );
        }
        _ => {}
    }
}

#[cfg(test)]
mod small_state_shadow_tests {
    use super::write_small_state_shadow_attr;

    #[test]
    fn small_state_shadow_uses_source_width_theme_and_look_conditions() {
        for (width, look, enabled, expected) in [
            (14.0, "neo", true, true),
            (24.999, "classic", true, true),
            (25.0, "neo", true, false),
            (40.0, "neo", true, false),
            (14.0, "neo", false, false),
            (14.0, "handDrawn", true, false),
        ] {
            let mut out = String::new();
            write_small_state_shadow_attr(
                &mut out,
                "diagram-drop-shadow-small",
                width,
                look,
                enabled,
                "",
            );
            assert_eq!(
                out,
                if expected {
                    r#" style="filter:url(#diagram-drop-shadow-small)""#
                } else {
                    ""
                },
                "width={width}, look={look}, enabled={enabled}"
            );
        }
        let mut out = String::new();
        write_small_state_shadow_attr(&mut out, "drop-shadow-small", 14.0, "classic", true, "");
        assert_eq!(out, r#" style="filter:url(#drop-shadow-small)""#);
    }

    #[test]
    fn small_state_shadow_preserves_typed_theme_declarations() {
        let mut out = String::new();
        write_small_state_shadow_attr(
            &mut out,
            "allocated-shadow",
            14.0,
            "classic",
            true,
            "fill:#123456 !important;stroke:#654321 !important",
        );
        assert_eq!(
            out,
            r#" style="filter:url(#allocated-shadow);fill:#123456 !important;stroke:#654321 !important""#
        );
    }
}
