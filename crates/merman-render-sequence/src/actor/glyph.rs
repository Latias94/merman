use super::geometry::{SEQUENCE_GLYPH_BAND_HEIGHT, SequenceActorBands};
use merman_render_core::number_format::canonical_number as fmt;
use merman_render_core::svg_css::{SvgId, scoped_svg_id, scoped_svg_url};
use merman_render_core::svg_output::SvgOutput;
use merman_render_core::xml::{escape_attr, escape_attr_display};

/// Resolved actor-man placement. The caller owns participant and label preparation.
pub struct ActorManGlyph<'a> {
    pub actor_id: &'a str,
    pub actor_type: &'a str,
    pub cx: f64,
    pub actor_y: f64,
    pub width: f64,
    pub row_height: f64,
    pub actor_height: f64,
    pub label_box_height: f64,
    pub footer: bool,
    pub line_index: usize,
    pub diagram_id: SvgId<'a>,
    pub bands: Option<SequenceActorBands>,
    pub classic_label_center_y: Option<f64>,
}

/// Writes the opening actor group and its glyph, returning the label's y coordinate.
/// The caller writes the label and closes the group using its original output checkpoints.
pub fn write_actor_man_glyph<S: SvgOutput>(
    out: &mut S,
    glyph: ActorManGlyph<'_>,
    write_style_attr: impl FnOnce(&mut S),
) -> f64 {
    let ActorManGlyph {
        actor_id,
        actor_type,
        cx,
        actor_y,
        width,
        row_height,
        actor_height,
        label_box_height,
        footer,
        line_index: idx,
        diagram_id,
        bands,
        classic_label_center_y,
    } = glyph;
    let height = if bands.is_some() {
        row_height
    } else if footer {
        match actor_type {
            "boundary" | "entity" => 44.0 + label_box_height,
            "control" => 44.0 + 2.0 * label_box_height,
            _ => actor_height,
        }
    } else {
        actor_height
    };
    let placement_class = if footer { "actor-bottom" } else { "actor-top" };
    let group_class = if actor_type == "entity" {
        "actor"
    } else {
        "actor-man"
    };
    let _ = write!(
        out,
        r#"<g class="{group_class} {placement_class}" name="{}""#,
        escape_attr(actor_id)
    );
    let translate_y = if bands.is_none() {
        match actor_type {
            "boundary" => {
                out.push_str(r#" transform="translate(0,21)""#);
                21.0
            }
            "entity" => {
                let offset = if footer { 22 } else { 6 };
                let _ = write!(out, r#" transform="translate(0, {offset})""#);
                f64::from(offset)
            }
            _ => 0.0,
        }
    } else {
        0.0
    };
    if bands.is_some() && matches!(actor_type, "boundary" | "entity") {
        write_neo_actor_shadow_attr(out, diagram_id);
    }
    write_style_attr(out);
    if !footer {
        let _ = write!(
            out,
            r#" data-et="participant" data-type="{}" data-id="{}""#,
            escape_attr(actor_type),
            escape_attr(actor_id)
        );
    }
    out.push('>');

    let cx_text = fmt(cx);
    let cy = bands
        .as_ref()
        .map(|band| band.glyph_bottom_y - 22.0)
        .unwrap_or_else(|| {
            actor_y
                + match actor_type {
                    "boundary" => 12.0,
                    "entity" if footer => 10.0,
                    "entity" => 25.0,
                    _ => 32.0,
                }
        });
    match actor_type {
        "actor" => {
            let scale = if bands.is_some() {
                SEQUENCE_GLYPH_BAND_HEIGHT / 65.0
            } else {
                1.0
            };
            let gy = |offset: f64| {
                bands.as_ref().map_or(actor_y + offset, |band| {
                    band.glyph_bottom_y - (60.0 - offset) * scale
                })
            };
            let gx = |offset: f64| cx + offset * scale;
            let _ = write!(
                out,
                r#"<line id="actor-man-torso{idx}" x1="{cx_text}" y1="{torso_top}" x2="{cx_text}" y2="{torso_bottom}"/><line id="actor-man-arms{idx}" x1="{left}" y1="{arms}" x2="{right}" y2="{arms}"/><line x1="{left}" y1="{feet}" x2="{cx_text}" y2="{torso_bottom}"/><line x1="{cx_text}" y1="{torso_bottom}" x2="{right_leg}" y2="{feet}"/><circle cx="{cx_text}" cy="{head}" r="{radius}" width="{width}" height="{height}"/>"#,
                torso_top = fmt(gy(25.0)),
                torso_bottom = fmt(gy(45.0)),
                left = fmt(gx(-18.0)),
                right = fmt(gx(18.0)),
                arms = fmt(gy(33.0)),
                feet = fmt(gy(60.0)),
                right_leg = fmt(gx(16.0)),
                head = fmt(gy(10.0)),
                radius = fmt(15.0 * scale),
                width = fmt(width),
                height = fmt(height)
            );
        }
        "boundary" => {
            let _ = write!(
                out,
                r#"<line id="actor-man-torso{idx}" x1="{left}" y1="{cy}" x2="{right}" y2="{cy}"/><line id="actor-man-arms{idx}" x1="{left}" y1="{top}" x2="{left}" y2="{bottom}"/><circle cx="{cx_text}" cy="{cy}" r="22"/>"#,
                left = fmt(cx - 55.0),
                right = fmt(cx - 15.0),
                cy = fmt(cy),
                top = fmt(cy - 10.0),
                bottom = fmt(cy + 10.0)
            );
        }
        "control" => {
            let marker_id = scoped_svg_id(diagram_id, "filled-head-control");
            let marker_url = scoped_svg_url(diagram_id, "filled-head-control");
            let _ = write!(
                out,
                r#"<defs><marker id="{marker_id}" refX="11" refY="5.8" markerWidth="20" markerHeight="28" orient="172.5" stroke-width="1.2"><path d="M 14.4 5.6 L 7.2 10.4 L 8.8 5.6 L 7.2 0.8 Z"/></marker></defs><circle cx="{cx_text}" cy="{cy}" r="22""#,
                marker_id = escape_attr_display(marker_id),
                cy = fmt(cy)
            );
            if bands.is_some() {
                write_neo_actor_shadow_attr(out, diagram_id);
            } else {
                out.push_str(r#" filter="""#);
            }
            let _ = write!(
                out,
                r#"/><line marker-end="{marker_url}" transform="translate({cx_text}, {top})"/>"#,
                marker_url = escape_attr_display(marker_url),
                top = fmt(cy - 22.0)
            );
        }
        "entity" => {
            let _ = write!(
                out,
                r#"<circle cx="{cx_text}" cy="{cy}" r="22" width="{width}" height="{height}"/><line x1="{left}" x2="{right}" y1="{bottom}" y2="{bottom}" stroke-width="2"/>"#,
                cy = fmt(cy),
                width = fmt(width),
                height = fmt(height),
                left = fmt(cx - 22.0),
                right = fmt(cx + 22.0),
                bottom = fmt(cy + 22.0)
            );
        }
        _ => {}
    }
    bands
        .as_ref()
        .map(|band| band.label_center_y)
        .or_else(|| classic_label_center_y.map(|center| actor_y + center - translate_y))
        .unwrap_or_else(|| {
            actor_y
                + height / 2.0
                + match actor_type {
                    "actor" => 35.0,
                    "boundary" => 15.0,
                    "control" if footer => 27.0,
                    "control" => 34.0,
                    "entity" if footer => 15.0,
                    "entity" => 30.0,
                    _ => 0.0,
                }
        })
}

pub fn write_neo_actor_shadow_attr(out: &mut impl SvgOutput, diagram_id: SvgId<'_>) {
    let _ = write!(
        out,
        r#" filter="url(#{}-drop-shadow)""#,
        escape_attr_display(diagram_id)
    );
}

pub fn is_actor_man_variant(actor_type: &str) -> bool {
    matches!(actor_type, "actor" | "boundary" | "control" | "entity")
}

pub const LIFELINE_STROKE_WIDTH_PX: f64 = 0.5;

pub fn write_actor_man_lifeline(
    out: &mut impl SvgOutput,
    idx: usize,
    cx: f64,
    y1: f64,
    y2: f64,
    actor_id: &str,
) {
    let actor_id = escape_attr(actor_id);
    let _ = write!(
        out,
        r##"<g><line id="actor{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}" class="actor-line 200" stroke-width="{stroke_width}px" stroke="#999" name="{actor_id}" data-et="life-line" data-id="{actor_id}"/></g>"##,
        idx = idx,
        cx = fmt(cx),
        y1 = fmt(y1),
        y2 = fmt(y2),
        stroke_width = fmt(LIFELINE_STROKE_WIDTH_PX),
    );
}
