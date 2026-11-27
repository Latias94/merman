use merman_render_core::{
    Result, number_format::canonical_number as fmt, svg_css::SvgId, svg_output::SvgOutput,
    xml::escape_attr,
};
#[cfg(feature = "layout-shoal")]
use {
    kurbo::{BezPath, Point, Rect, Shape, Vec2},
    merman_render_core::{LayoutPoint, route_types::Marker},
};

struct Cardinality {
    card: &'static str,
    name: &'static str,
    ends: [MarkerGlyph; 2],
}

struct MarkerGlyph {
    ref_x: u8,
    ref_y: u8,
    width: u8,
    height: u8,
    path: &'static str,
    circle_x: Option<(f64, f64)>,
    #[cfg(feature = "layout-shoal")]
    outline: std::sync::OnceLock<Rect>,
}

const CIRCLE_RADIUS: u8 = 6;

impl MarkerGlyph {
    const fn new(
        ref_x: u8,
        ref_y: u8,
        width: u8,
        height: u8,
        path: &'static str,
        circle_x: Option<(f64, f64)>,
    ) -> Self {
        Self {
            ref_x,
            ref_y,
            width,
            height,
            path,
            circle_x,
            #[cfg(feature = "layout-shoal")]
            outline: std::sync::OnceLock::new(),
        }
    }

    fn write_content(&self, out: &mut impl SvgOutput, neo: Option<(&str, &str)>) {
        if let Some((classic_x, neo_x)) = self.circle_x {
            if let Some((stroke_width, background)) = neo {
                let _ = write!(
                    out,
                    r#"<circle fill="{background}" cx="{}" cy="{}" stroke-width="{stroke_width}" r="{CIRCLE_RADIUS}"/>"#,
                    fmt(neo_x),
                    self.ref_y,
                );
            } else {
                let _ = write!(
                    out,
                    r#"<circle fill="white" cx="{}" cy="{}" r="{CIRCLE_RADIUS}"/>"#,
                    fmt(classic_x),
                    self.ref_y,
                );
            }
        }
        let _ = write!(out, r#"<path d="{}""#, self.path);
        if let Some((stroke_width, _)) = neo {
            let _ = write!(out, r#" stroke-width="{stroke_width}""#);
        }
        out.push_str("/>");
    }

    #[cfg(feature = "layout-shoal")]
    fn footprint(&self, neo: bool) -> Rect {
        let viewport = Rect::new(0.0, 0.0, f64::from(self.width), f64::from(self.height));
        if neo {
            return viewport;
        }
        *self.outline.get_or_init(|| {
            let mut ink = BezPath::from_svg(self.path)
                .expect("static ER marker path must be valid")
                .bounding_box();
            if let Some((classic_x, _)) = self.circle_x {
                let diameter = 2.0 * f64::from(CIRCLE_RADIUS);
                ink = ink.union(Rect::from_center_size(
                    (classic_x, f64::from(self.ref_y)),
                    (diameter, diameter),
                ));
            }
            ink.inflate(0.5, 0.5).intersect(viewport)
        })
    }
}

static CARDINALITIES: [Cardinality; 4] = [
    Cardinality {
        card: "ONLY_ONE",
        name: "onlyOne",
        ends: [
            MarkerGlyph::new(0, 9, 18, 18, "M9,0 L9,18 M15,0 L15,18", None),
            MarkerGlyph::new(18, 9, 18, 18, "M3,0 L3,18 M9,0 L9,18", None),
        ],
    },
    Cardinality {
        card: "ZERO_OR_ONE",
        name: "zeroOrOne",
        ends: [
            MarkerGlyph::new(0, 9, 30, 18, "M9,0 L9,18", Some((21.0, 21.0))),
            MarkerGlyph::new(30, 9, 30, 18, "M21,0 L21,18", Some((9.0, 9.0))),
        ],
    },
    Cardinality {
        card: "ONE_OR_MORE",
        name: "oneOrMore",
        ends: [
            MarkerGlyph::new(
                18,
                18,
                45,
                36,
                "M0,18 Q 18,0 36,18 Q 18,36 0,18 M42,9 L42,27",
                None,
            ),
            MarkerGlyph::new(
                27,
                18,
                45,
                36,
                "M3,9 L3,27 M9,18 Q27,0 45,18 Q27,36 9,18",
                None,
            ),
        ],
    },
    Cardinality {
        card: "ZERO_OR_MORE",
        name: "zeroOrMore",
        ends: [
            MarkerGlyph::new(
                18,
                18,
                57,
                36,
                "M0,18 Q18,0 36,18 Q18,36 0,18",
                Some((48.0, 45.5)),
            ),
            MarkerGlyph::new(
                39,
                18,
                57,
                36,
                "M21,18 Q39,0 57,18 Q39,36 21,18",
                Some((9.0, 11.0)),
            ),
        ],
    },
];

// Canonical (Shoal) ER markers emit the visible open crow's-foot half of Mermaid's symmetric
// lens (exact De Casteljau split at t = 0.5) with refX anchored directly on the entity wall.
// This avoids pulling shafts 18 px off entity borders and never exposes a closed back-half loop.
static CANONICAL_CROW_FOOT: [Cardinality; 2] = [
    Cardinality {
        card: "ONE_OR_MORE",
        name: "oneOrMore",
        ends: [
            MarkerGlyph::new(
                0,
                18,
                27,
                36,
                "M0,9 Q9,9 18,18 Q9,27 0,27 M24,9 L24,27",
                None,
            ),
            MarkerGlyph::new(
                27,
                18,
                27,
                36,
                "M3,9 L3,27 M27,9 Q18,9 9,18 Q18,27 27,27",
                None,
            ),
        ],
    },
    Cardinality {
        card: "ZERO_OR_MORE",
        name: "zeroOrMore",
        ends: [
            MarkerGlyph::new(
                0,
                18,
                39,
                36,
                "M0,9 Q9,9 18,18 Q9,27 0,27",
                Some((30.0, 27.5)),
            ),
            MarkerGlyph::new(
                39,
                18,
                39,
                36,
                "M39,9 Q30,9 21,18 Q30,27 39,27",
                Some((9.0, 11.0)),
            ),
        ],
    },
];

fn cardinality(card: &str) -> Option<&'static Cardinality> {
    CARDINALITIES.iter().find(|entry| entry.card == card)
}

fn canonical_cardinality(card: &str) -> Option<&'static Cardinality> {
    CANONICAL_CROW_FOOT
        .iter()
        .find(|entry| entry.card == card)
        .or_else(|| cardinality(card))
}

/// Projects a semantic cardinality to Mermaid's layout marker spelling.
pub fn er_marker_id(card: &str, suffix: &str) -> Option<String> {
    // Mermaid ER does not assign a dedicated layout marker for MD_PARENT.
    cardinality(card)?;
    Some(format!("{card}_{suffix}"))
}

fn marker_suffix(value: &str) -> Option<(&str, bool)> {
    if let Some(card) = value.strip_suffix("_START") {
        Some((card, true))
    } else {
        value.strip_suffix("_END").map(|card| (card, false))
    }
}

pub fn er_unified_marker_id(diagram_id: SvgId<'_>, upstream_marker: &str) -> String {
    let upstream_marker = upstream_marker.trim();
    let Some((card, start)) = marker_suffix(upstream_marker) else {
        return upstream_marker.to_owned();
    };
    let name = if card == "MD_PARENT" {
        "mdParent"
    } else if let Some(entry) = cardinality(card) {
        entry.name
    } else {
        return upstream_marker.to_owned();
    };
    let end = if start { "Start" } else { "End" };
    format!("{diagram_id}_er-{name}{end}")
}

#[cfg(feature = "layout-shoal")]
pub fn er_canonical_marker(card: &str, start: bool, neo: bool) -> Marker {
    let Some(entry) = canonical_cardinality(card) else {
        return Marker::default();
    };
    let glyph = &entry.ends[usize::from(!start)];
    let footprint = glyph.footprint(neo);
    let ref_x = f64::from(glyph.ref_x);
    let ref_y = f64::from(glyph.ref_y);
    let (trim, depth) = if start {
        (ref_x - footprint.x0, footprint.x1 - ref_x)
    } else {
        (footprint.x1 - ref_x, ref_x - footprint.x0)
    };
    Marker {
        trim: trim.max(0.0),
        depth,
        width: 2.0 * (ref_y - footprint.y0).max(footprint.y1 - ref_y),
    }
}

#[cfg(feature = "layout-shoal")]
pub fn er_marker_paint_bounds(
    upstream_marker: &str,
    neo: bool,
    reference: &LayoutPoint,
    shoulder: &LayoutPoint,
    start: bool,
) -> Option<Rect> {
    let (card, glyph_at_start) = marker_suffix(upstream_marker.trim())?;
    let glyph = &canonical_cardinality(card)?.ends[usize::from(!glyph_at_start)];
    let reference = Point::new(reference.x, reference.y);
    let shoulder = Point::new(shoulder.x, shoulder.y);
    let forward = if start {
        shoulder - reference
    } else {
        reference - shoulder
    }
    .normalize();
    let across = Vec2::new(-forward.y, forward.x);
    let footprint = glyph.footprint(neo);
    let mut bounds = Rect::from_points(reference, reference);
    for x in [footprint.x0, footprint.x1] {
        for y in [footprint.y0, footprint.y1] {
            let point = reference
                + forward * (x - f64::from(glyph.ref_x))
                + across * (y - f64::from(glyph.ref_y));
            bounds = bounds.union_pt(point);
        }
    }
    Some(bounds)
}

pub struct ErMarkerDefs<'a> {
    pub diagram_id: SvgId<'a>,
    pub include_md_parent: bool,
    pub canonical: bool,
    pub neo: bool,
    pub neo_stroke_width: f64,
    pub main_bkg: &'a str,
}

pub fn write_er_marker_definitions(out: &mut impl SvgOutput, defs: ErMarkerDefs<'_>) -> Result<()> {
    let ErMarkerDefs {
        diagram_id,
        include_md_parent,
        canonical,
        neo,
        neo_stroke_width,
        main_bkg,
    } = defs;
    if include_md_parent {
        for (end, glyph) in [
            (
                "Start",
                MarkerGlyph::new(0, 7, 190, 240, "M 18,7 L9,13 L1,7 L9,1 Z", None),
            ),
            (
                "End",
                MarkerGlyph::new(19, 7, 20, 28, "M 18,7 L9,13 L1,7 L9,1 Z", None),
            ),
        ] {
            let id = format!("{diagram_id}_er-mdParent{end}");
            write_marker_start(out, &id, "mdParent", &glyph, false);
            glyph.write_content(out, None);
            out.push_str("</marker></defs>\n");
            out.checkpoint()?;
        }
    }

    let neo_width = fmt(neo_stroke_width).to_string();
    let neo_background = escape_attr(main_bkg);
    for entry in &CARDINALITIES {
        let active = if canonical {
            canonical_cardinality(entry.card).unwrap_or(entry)
        } else {
            entry
        };
        for (end, glyph) in ["Start", "End"].into_iter().zip(&active.ends) {
            let name = active.name;
            let id = format!("{diagram_id}_er-{name}{end}");
            write_marker_start(out, &id, name, glyph, neo || canonical);
            glyph.write_content(
                out,
                neo.then_some((neo_width.as_str(), neo_background.as_str())),
            );
            out.push_str("</marker></defs>\n");
            out.checkpoint()?;
        }
    }
    Ok(())
}

fn write_marker_start(
    out: &mut impl SvgOutput,
    id: &str,
    name: &str,
    glyph: &MarkerGlyph,
    absolute_units: bool,
) {
    let _ = write!(
        out,
        r#"<defs><marker id="{}" class="marker {name} er" refX="{}" refY="{}" markerWidth="{}" markerHeight="{}" orient="auto""#,
        escape_attr(id),
        glyph.ref_x,
        glyph.ref_y,
        glyph.width,
        glyph.height,
    );
    if absolute_units {
        out.push_str(r#" markerUnits="userSpaceOnUse""#);
    }
    out.push('>');
}

#[cfg(all(test, feature = "layout-shoal"))]
mod tests {
    use super::*;

    #[test]
    fn canonical_crow_foot_markers_require_zero_trim_and_emit_open_forks() {
        for card in ["ONE_OR_MORE", "ZERO_OR_MORE"] {
            for start in [true, false] {
                for neo in [false, true] {
                    let metrics = er_canonical_marker(card, start, neo);
                    assert_eq!(
                        metrics.trim, 0.0,
                        "{card} start={start} neo={neo} must seat directly at entity border without line trim"
                    );
                    assert!(metrics.depth > 0.0 && metrics.width > 0.0);
                }
            }
        }

        let mut canonical_svg = String::new();
        write_er_marker_definitions(
            &mut canonical_svg,
            ErMarkerDefs {
                diagram_id: SvgId::new("er").unwrap(),
                include_md_parent: false,
                canonical: true,
                neo: false,
                neo_stroke_width: 1.5,
                main_bkg: "#FFFFFF",
            },
        )
        .unwrap();
        assert!(canonical_svg.contains(r#"id="er_er-oneOrMoreStart" class="marker oneOrMore er" refX="0" refY="18" markerWidth="27""#));
        assert!(canonical_svg.contains(r#"id="er_er-oneOrMoreEnd" class="marker oneOrMore er" refX="27" refY="18" markerWidth="27""#));
        assert!(canonical_svg.contains(r#"id="er_er-zeroOrMoreStart" class="marker zeroOrMore er" refX="0" refY="18" markerWidth="39""#));
        assert!(canonical_svg.contains(r#"id="er_er-zeroOrMoreEnd" class="marker zeroOrMore er" refX="39" refY="18" markerWidth="39""#));

        let mut legacy_svg = String::new();
        write_er_marker_definitions(
            &mut legacy_svg,
            ErMarkerDefs {
                diagram_id: SvgId::new("er").unwrap(),
                include_md_parent: false,
                canonical: false,
                neo: false,
                neo_stroke_width: 1.5,
                main_bkg: "#FFFFFF",
            },
        )
        .unwrap();
        assert!(
            legacy_svg
                .contains(r#"id="er_er-oneOrMoreStart" class="marker oneOrMore er" refX="18""#)
        );
        assert!(legacy_svg.contains(r#"id="er_er-oneOrMoreEnd" class="marker oneOrMore er" refX="27" refY="18" markerWidth="45""#));
    }
}
