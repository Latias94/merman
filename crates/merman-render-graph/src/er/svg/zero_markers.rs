use super::super::{ErCssBinding, markers::er_zero_marker_circle};
use kurbo::{BezPath, Circle, PathSeg, Shape};
use merman_markup::theme_color::{ColorChannel, ThemeColor};
use merman_model::presentation::PresentationConfig;
use merman_render_core::{
    LayoutEdge, Result, number_format::canonical_number as fmt, resources::OperationWorkMeter,
    svg_output::SvgOutput, xml::escape_attr,
};
use merman_theme_contract::v2::Color;
use std::borrow::Cow;

pub(super) fn background<'a>(
    config: &'a PresentationConfig,
    binding: &'a ErCssBinding,
    canvas_background: Option<Color>,
    canvas_has_texture: bool,
) -> Cow<'a, str> {
    if canvas_has_texture {
        return Cow::Borrowed("none");
    }
    if let Some(color) = canvas_background {
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
    work_meter: &OperationWorkMeter,
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
    work_meter.charge(d.len())?;
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
    // The mask applies to the shaft and its attached markers. Keep the entire glyph viewport
    // inside the mask region, then subtract the union of both circle interiors.
    let outer_margin = 57.0 * scale + stroke_width;
    let outer = path.bounding_box().inflate(outer_margin, outer_margin);
    let _ = write!(
        out,
        r##"<defs><mask id="{}" maskUnits="userSpaceOnUse" maskContentUnits="userSpaceOnUse" mask-type="luminance" x="{}" y="{}" width="{}" height="{}"><rect x="{}" y="{}" width="{}" height="{}" fill="#fff"/>"##,
        escape_attr(id),
        fmt(outer.x0),
        fmt(outer.y0),
        fmt(outer.width()),
        fmt(outer.height()),
        fmt(outer.x0),
        fmt(outer.y0),
        fmt(outer.width()),
        fmt(outer.height()),
    );
    for circle in terminals.into_iter().flatten() {
        let _ = write!(
            out,
            r##"<circle cx="{}" cy="{}" r="{}" fill="#000"/>"##,
            fmt(circle.center.x),
            fmt(circle.center.y),
            fmt(circle.radius),
        );
    }
    out.push_str("</mask></defs>");
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

#[cfg(test)]
mod tests {
    use super::*;
    use merman_render_core::resources::RenderResourcePolicy;

    #[test]
    fn overlapping_zero_marker_interiors_stay_subtracted() {
        let config = PresentationConfig::default();
        let binding = ErCssBinding::resolve(config.as_value());
        let edge = LayoutEdge {
            id: "edge".into(),
            from: "A".into(),
            to: "B".into(),
            from_cluster: None,
            to_cluster: None,
            points: Vec::new(),
            label: None,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: Some("ZERO_OR_ONE_START".into()),
            end_marker: Some("ZERO_OR_ONE_END".into()),
            stroke_dasharray: None,
        };
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let mut out = String::new();
        assert!(
            write_cutout(
                &mut out,
                "overlap",
                "M0 20L40 20",
                &edge,
                &binding,
                true,
                &work
            )
            .unwrap()
        );
        let document = roxmltree::Document::parse(&out).unwrap();
        let mask = document.root_element().first_element_child().unwrap();
        assert!(mask.has_tag_name("mask"));
        assert_eq!(mask.attribute("mask-type"), Some("luminance"));
        let mut shapes = mask.children().filter(|child| child.is_element());
        assert_eq!(shapes.next().unwrap().attribute("fill"), Some("#fff"));
        let [a, b] = [shapes.next().unwrap(), shapes.next().unwrap()];
        let geometry = |node: roxmltree::Node<'_, '_>, field| {
            node.attribute(field).unwrap().parse::<f64>().unwrap()
        };
        assert!(a.has_tag_name("circle") && b.has_tag_name("circle"));
        assert!(
            (geometry(a, "cx") - geometry(b, "cx")).abs() < geometry(a, "r") + geometry(b, "r")
        );
        assert_eq!(a.attribute("fill"), Some("#000"));
        assert_eq!(b.attribute("fill"), Some("#000"));
    }
}
