use super::super::super::{SvgExecution, SvgOutput, escape_attr, fmt};
use crate::Result;
use crate::er::ErCssBinding;
use crate::model::LayoutEdge;
use kurbo::{BezPath, Circle, PathSeg, Shape};
use merman_markup::theme_color::{ColorChannel, ThemeColor};
use merman_model::presentation::PresentationConfig;
use merman_render_graph::er::er_zero_marker_circle;
use merman_theme_contract::v2::{Paint, Texture};
use std::borrow::Cow;

pub(super) fn background<'a>(
    config: &'a PresentationConfig,
    binding: &'a ErCssBinding,
    options: &SvgExecution<'_>,
) -> Cow<'a, str> {
    let session = options.family.session();
    if session.native_theme().is_some_and(|theme| {
        theme
            .resolved()
            .canvas()
            .textures
            .iter()
            .any(|texture| !matches!(texture, Texture::None))
    }) {
        return Cow::Borrowed("none");
    }
    let canvas = session.canvas_background().or_else(|| {
        options
            .native_family_theme()
            .and_then(|theme| match theme.roles.canvas {
                Paint::Solid(color) => Some(color),
                Paint::LinearGradient(_) => None,
            })
    });
    if let Some(color) = canvas {
        return if color.channels()[3] == u8::MAX {
            Cow::Owned(color.to_string())
        } else {
            Cow::Borrowed("none")
        };
    }
    if binding.native.is_some() {
        return Cow::Borrowed("none");
    }
    let neo = binding.render_settings.diagram_look == "neo";
    let dark = matches!(
        config.get_str("theme"),
        Some("dark" | "neo-dark" | "redux-dark" | "redux-dark-color")
    );
    let configured = config.get_str("themeVariables.background");
    let background = if config.explicit_config_owns_path("themeVariables.background") {
        configured.unwrap_or("none")
    } else if neo {
        &binding.main_bkg
    } else if dark {
        configured.unwrap_or("#1f2020")
    } else {
        "white"
    };
    if ThemeColor::parse(background).is_ok_and(|color| color.channel(ColorChannel::Alpha) == 1.0) {
        Cow::Borrowed(background)
    } else {
        Cow::Borrowed("none")
    }
}

pub(super) fn write_cutout(
    out: &mut impl SvgOutput,
    id: &str,
    d: &str,
    edge: &LayoutEdge,
    binding: &ErCssBinding,
    canonical: bool,
    options: &SvgExecution<'_>,
) -> Result<bool> {
    let neo = binding.render_settings.diagram_look == "neo";
    let markers = [&edge.start_marker, &edge.end_marker].map(|marker| {
        marker
            .as_deref()
            .and_then(|marker| er_zero_marker_circle(marker, canonical, neo))
    });
    if markers.iter().all(Option::is_none) {
        return Ok(false);
    }
    options.work_meter().charge(d.len())?;
    let Ok(path) = BezPath::from_svg(d) else {
        return Ok(false);
    };
    let scale = if neo || canonical {
        1.0
    } else {
        binding
            .native
            .as_ref()
            .map_or(1.0, |style| style.connector_width)
    };
    let stroke_width = if neo {
        binding.neo_marker_stroke_width
    } else {
        1.0
    };
    let mut terminals = [None; 2];
    for segment in path.segments() {
        if terminals[0].is_none() {
            terminals[0] = circle_on_segment(segment, markers[0], scale, stroke_width);
        }
        if let Some(circle) = circle_on_segment(
            segment.reverse(),
            markers[1].map(|(offset, radius)| (-offset, radius)),
            scale,
            stroke_width,
        ) {
            terminals[1] = Some(circle);
        }
    }
    if terminals.iter().all(Option::is_none) {
        return Ok(false);
    }
    // The clip applies to the shaft and its attached markers. Leave the whole glyph (up to the
    // 57-unit cardinality viewport) within the outer contour, then remove only the circle ink gap.
    let outer_margin = 57.0 * scale + stroke_width;
    let outer = path.bounding_box().inflate(outer_margin, outer_margin);
    let _ = write!(
        out,
        r#"<defs><clipPath id="{}"><path clip-rule="evenodd" d="M{} {}H{}V{}H{}Z"#,
        escape_attr(id),
        fmt(outer.x0),
        fmt(outer.y0),
        fmt(outer.x1),
        fmt(outer.y1),
        fmt(outer.x0),
    );
    for circle in terminals.into_iter().flatten() {
        let (x, y, r) = (circle.center.x, circle.center.y, circle.radius);
        let _ = write!(
            out,
            "M{} {}A{r} {r} 0 1 1 {} {}A{r} {r} 0 1 1 {} {}Z",
            fmt(x - r),
            fmt(y),
            fmt(x + r),
            fmt(y),
            fmt(x - r),
            fmt(y),
            r = fmt(r),
        );
    }
    out.push_str(r#""/></clipPath></defs>"#);
    out.checkpoint()?;
    Ok(true)
}

fn circle_on_segment(
    segment: PathSeg,
    marker: Option<(f64, f64)>,
    scale: f64,
    stroke_width: f64,
) -> Option<Circle> {
    let (offset, radius) = marker?;
    let radius = (radius - stroke_width / 2.0) * scale;
    if radius <= 0.0 {
        return None;
    }
    let segment = segment.to_cubic();
    let tangent = [segment.p1, segment.p2, segment.p3]
        .into_iter()
        .map(|point| point - segment.p0)
        .find(|tangent| tangent.hypot2() > 0.0)?;
    Some(Circle::new(
        segment.p0 + tangent.normalize() * (offset * scale),
        radius,
    ))
}
