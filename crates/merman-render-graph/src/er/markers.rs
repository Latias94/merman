use kurbo::{BezPath, Rect, Shape};
use merman_render_core::{
    Result, number_format::canonical_number as fmt, route_types::Marker, svg_css::SvgId,
    svg_output::SvgOutput, xml::escape_attr,
};
#[cfg(feature = "layout-shoal")]
use {
    kurbo::{Point, Vec2},
    merman_render_core::LayoutPoint,
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
    inset_path: Option<&'static str>,
    circle_x: Option<(f64, f64)>,
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
            inset_path: None,
            circle_x,
            outline: std::sync::OnceLock::new(),
        }
    }

    const fn with_inset_path(mut self, path: &'static str) -> Self {
        self.inset_path = Some(path);
        self
    }

    fn write_content(
        &self,
        out: &mut impl SvgOutput,
        background: &str,
        stroke_width: Option<&str>,
        inset_prongs: bool,
    ) {
        if let Some((classic_x, neo_x)) = self.circle_x {
            if let Some(stroke_width) = stroke_width {
                let _ = write!(
                    out,
                    r#"<circle fill="{background}" cx="{}" cy="{}" stroke-width="{stroke_width}" r="{CIRCLE_RADIUS}"/>"#,
                    fmt(neo_x),
                    self.ref_y,
                );
            } else {
                let _ = write!(
                    out,
                    r#"<circle fill="{background}" cx="{}" cy="{}" r="{CIRCLE_RADIUS}"/>"#,
                    fmt(classic_x),
                    self.ref_y,
                );
            }
        }
        let path = if inset_prongs {
            self.inset_path.unwrap_or(self.path)
        } else {
            self.path
        };
        let _ = write!(out, r#"<path d="{path}""#);
        if let Some(stroke_width) = stroke_width {
            let _ = write!(out, r#" stroke-width="{stroke_width}""#);
        }
        out.push_str("/>");
    }

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

// Anchor the trimmed shaft at the fork's junction. The glyph owns all three solid prongs,
// including the center line from the entity wall, even when the relationship is dashed.
// For BT, omit the wall-facing eighth of each prong. These quadratic subdivisions retain the
// fork's junction and leave room for the entity stroke at the two nearly tangent tips.
static CANONICAL_CROW_FOOT: [Cardinality; 2] = [
    Cardinality {
        card: "ONE_OR_MORE",
        name: "oneOrMore",
        ends: [
            MarkerGlyph::new(
                18,
                18,
                27,
                36,
                "M0,9 Q9,9 18,18 Q9,27 0,27 M0,18 H18 M24,9 L24,27",
                None,
            )
            .with_inset_path(
                "M2.25,9.140625 Q10.125,10.125 18,18 Q10.125,25.875 2.25,26.859375 M0,18 H18 M24,9 L24,27",
            ),
            MarkerGlyph::new(
                9,
                18,
                27,
                36,
                "M3,9 L3,27 M27,9 Q18,9 9,18 Q18,27 27,27 M27,18 H9",
                None,
            )
            .with_inset_path(
                "M3,9 L3,27 M24.75,9.140625 Q16.875,10.125 9,18 Q16.875,25.875 24.75,26.859375 M27,18 H9",
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
                39,
                36,
                "M0,9 Q9,9 18,18 Q9,27 0,27 M0,18 H18",
                Some((30.0, 27.5)),
            )
            .with_inset_path("M2.25,9.140625 Q10.125,10.125 18,18 Q10.125,25.875 2.25,26.859375 M0,18 H18"),
            MarkerGlyph::new(
                21,
                18,
                39,
                36,
                "M39,9 Q30,9 21,18 Q30,27 39,27 M39,18 H21",
                Some((9.0, 11.0)),
            )
            .with_inset_path("M36.75,9.140625 Q28.875,10.125 21,18 Q28.875,25.875 36.75,26.859375 M39,18 H21"),
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

#[cfg(feature = "layout-grid")]
pub(super) fn has_er_marker(card: &str) -> bool {
    cardinality(card).is_some()
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

/// Center offset and radius of a cardinality marker's circle in its viewport units.
pub(super) fn er_zero_marker_circle(
    upstream_marker: &str,
    canonical: bool,
    neo: bool,
) -> Option<(f64, f64)> {
    let (card, start) = marker_suffix(upstream_marker.trim())?;
    let entry = if canonical {
        canonical_cardinality(card)
    } else {
        cardinality(card)
    }?;
    let glyph = &entry.ends[usize::from(!start)];
    let (classic_x, neo_x) = glyph.circle_x?;
    let x = if neo { neo_x } else { classic_x };
    Some((x - f64::from(glyph.ref_x), f64::from(CIRCLE_RADIUS)))
}

#[cfg(feature = "layout-shoal")]
pub fn er_canonical_marker(card: &str, start: bool, neo: bool) -> Marker {
    marker_metrics(canonical_cardinality(card), start, neo)
}

pub(super) fn er_compatibility_sketch_marker(
    marker: Option<&str>,
    neo: bool,
    width: f64,
) -> Marker {
    let Some((card, start)) = marker.and_then(marker_suffix) else {
        return Marker::default();
    };
    let metrics = marker_metrics(cardinality(card), start, neo);
    let scale = if neo { 1.0 } else { width };
    Marker {
        depth: metrics.depth * scale,
        width: metrics.width * scale,
        trim: metrics.trim * scale,
    }
}

fn marker_metrics(entry: Option<&Cardinality>, start: bool, neo: bool) -> Marker {
    let Some(entry) = entry else {
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
    /// Resolved backing paint for the zero cardinality circles.
    pub main_bkg: &'a str,
}

pub fn write_er_marker_definitions(out: &mut impl SvgOutput, defs: ErMarkerDefs<'_>) -> Result<()> {
    write_er_marker_definitions_with_inset(out, defs, false)
}

pub(super) fn write_er_marker_definitions_with_inset(
    out: &mut impl SvgOutput,
    defs: ErMarkerDefs<'_>,
    inset_crow_foot_prongs: bool,
) -> Result<()> {
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
            glyph.write_content(out, "none", None, false);
            out.push_str("</marker></defs>\n");
            out.checkpoint()?;
        }
    }

    let neo_width = fmt(neo_stroke_width).to_string();
    let background = escape_attr(main_bkg);
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
                &background,
                neo.then_some(neo_width.as_str()),
                inset_crow_foot_prongs,
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
    fn canonical_crow_foot_markers_trim_the_shaft_to_the_solid_fork() {
        for card in ["ONE_OR_MORE", "ZERO_OR_MORE"] {
            for start in [true, false] {
                for neo in [false, true] {
                    let metrics = er_canonical_marker(card, start, neo);
                    assert_eq!(metrics.trim, 18.0, "{card} start={start} neo={neo}");
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
        assert!(canonical_svg.contains(r#"id="er_er-oneOrMoreStart" class="marker oneOrMore er" refX="18" refY="18" markerWidth="27""#));
        assert!(canonical_svg.contains(r#"id="er_er-oneOrMoreEnd" class="marker oneOrMore er" refX="9" refY="18" markerWidth="27""#));
        assert!(canonical_svg.contains(r#"id="er_er-zeroOrMoreStart" class="marker zeroOrMore er" refX="18" refY="18" markerWidth="39""#));
        assert!(canonical_svg.contains(r#"id="er_er-zeroOrMoreEnd" class="marker zeroOrMore er" refX="21" refY="18" markerWidth="39""#));

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
