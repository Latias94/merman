use std::fmt::{self, Write};

use merman_theme_contract::v2::{Color, Paint, Texture};

use crate::number_format::canonical_number;

/// Writes the native background and any overlays in the coordinate space of the root viewport.
pub(crate) fn write_canvas(
    out: &mut impl Write,
    scope: &str,
    viewport: Option<[f64; 4]>,
    base: &Paint,
    textures: &[Texture],
    guide: &Paint,
) -> fmt::Result {
    let bounds = match viewport {
        Some([x, y, w, h]) => format!(
            r#"x="{}" y="{}" width="{}" height="{}""#,
            canonical_number(x),
            canonical_number(y),
            canonical_number(w),
            canonical_number(h)
        ),
        None => r#"x="0" y="0" width="100%" height="100%""#.to_owned(),
    };
    out.write_str(r#"<g class="merman-theme-canvas" aria-hidden="true" pointer-events="none">"#)?;
    match base {
        Paint::Solid(color) => {
            if color.channels()[3] != 0 {
                write!(out, "<rect {bounds} ")?;
                write_color(out, "fill", *color, 1.0)?;
                out.write_str("/>")?;
            }
        }
        Paint::LinearGradient(gradient) => {
            let id = format!("{scope}-merman-canvas-base");
            let (dx, dy) = gradient.angle_deg.to_radians().sin_cos();
            let dy = -dy;
            let scale = 0.5 / dx.abs().max(dy.abs());
            write!(
                out,
                r#"<defs><linearGradient id="{id}" x1="{}" y1="{}" x2="{}" y2="{}">"#,
                canonical_number(0.5 - scale * dx),
                canonical_number(0.5 - scale * dy),
                canonical_number(0.5 + scale * dx),
                canonical_number(0.5 + scale * dy)
            )?;
            for stop in &gradient.stops {
                write!(out, r#"<stop offset="{}" "#, canonical_number(stop.offset))?;
                write_color(out, "stop-color", stop.color, stop.opacity)?;
                out.write_str("/>")?;
            }
            write!(
                out,
                r#"</linearGradient></defs><rect {bounds} fill="url(#{id})"/>"#
            )?;
        }
    }
    let guide = match guide {
        Paint::Solid(color) => *color,
        Paint::LinearGradient(gradient) => gradient
            .stops
            .first()
            .map_or(Color::TRANSPARENT, |stop| stop.color),
    };
    for (index, texture) in textures.iter().enumerate() {
        let id = format!("{scope}-merman-canvas-texture-{index}");
        if write_texture(out, &id, texture, guide)? {
            write!(out, r#"<rect {bounds} fill="url(#{id})"/>"#)?;
        }
        if let Texture::Ruled {
            margin_rule_color: Some(color),
            margin_rule_x_px,
            ..
        } = texture
        {
            let [x, y, _, h] = viewport.unwrap_or([0.0, 0.0, 0.0, 0.0]);
            let x = canonical_number(x + margin_rule_x_px);
            write!(
                out,
                r#"<line x1="{x}" x2="{x}" y1="{}" y2=""#,
                canonical_number(y)
            )?;
            if viewport.is_some() {
                write!(out, "{}", canonical_number(y + h))?;
            } else {
                out.write_str("100%")?;
            }
            out.write_str(r#"" "#)?;
            write_color(out, "stroke", *color, 1.0)?;
            out.write_str("/>")?;
        }
    }
    out.write_str("</g>")
}

fn write_texture(
    out: &mut impl Write,
    id: &str,
    texture: &Texture,
    guide: Color,
) -> Result<bool, fmt::Error> {
    match *texture {
        Texture::None => return Ok(false),
        Texture::Grid {
            step_px,
            major_every,
            color,
            opacity,
            major_opacity,
        } => {
            let color = color.unwrap_or(guide);
            let period = f64::from(major_every) * step_px;
            open_pattern(out, id, period, period)?;
            out.write_str(r#"<path fill="none" stroke-width="0.6" d=""#)?;
            for index in 1..major_every {
                let grid = canonical_number(f64::from(index) * step_px);
                let end = canonical_number(period);
                write!(out, "M{grid} 0V{end}M0 {grid}H{end}")?;
            }
            out.write_str(r#"" "#)?;
            write_color(out, "stroke", color, opacity)?;
            out.write_str(r#"/><path fill="none" d=""#)?;
            let end = canonical_number(period);
            write!(out, r#"M0.5 {end}V0.5H{end}" "#)?;
            write_color(out, "stroke", color, major_opacity)?;
            out.write_str("/></pattern></defs>")?;
        }
        Texture::Dots {
            step_px,
            radius_px,
            color,
            opacity,
        } => {
            open_pattern(out, id, step_px, step_px)?;
            let mid = canonical_number(step_px * 0.5);
            write!(
                out,
                r#"<circle cx="{mid}" cy="{mid}" r="{}" "#,
                canonical_number(radius_px)
            )?;
            write_color(out, "fill", color.unwrap_or(guide), opacity)?;
            out.write_str("/></pattern></defs>")?;
        }
        Texture::Ruled {
            line_height_px,
            color,
            opacity,
            ..
        } => {
            write_stripe(out, id, line_height_px, color.unwrap_or(guide), opacity)?;
        }
        Texture::Scanlines {
            step_px,
            color,
            opacity,
        } => {
            write_stripe(out, id, step_px, color.unwrap_or(guide), opacity)?;
        }
        Texture::Fibers {
            angle_deg,
            cross,
            color,
            opacity,
        } => {
            open_pattern(out, id, 24.0, 24.0)?;
            let angle = canonical_number(angle_deg);
            write!(
                out,
                r#"<path d="M3 9h6m5-4h4m-5 13h6" transform="rotate({angle} 12 12)" fill="none" stroke-width="0.65" "#
            )?;
            write_color(out, "stroke", color.unwrap_or(guide), opacity)?;
            out.write_str("/>")?;
            if cross {
                write!(
                    out,
                    r#"<path d="M4 15h5m6-4h4" transform="rotate(-{angle} 12 12)" fill="none" stroke-width="0.65" "#
                )?;
                write_color(out, "stroke", color.unwrap_or(guide), opacity)?;
                out.write_str("/>")?;
            }
            out.write_str("</pattern></defs>")?;
        }
        Texture::RadialWash {
            color,
            opacity,
            cx,
            cy,
            radius,
        } => {
            write!(
                out,
                r#"<defs><radialGradient id="{id}" cx="{}" cy="{}" r="{}"><stop offset="0" "#,
                canonical_number(cx),
                canonical_number(cy),
                canonical_number(radius)
            )?;
            write_color(out, "stop-color", color, opacity)?;
            out.write_str(r#"/><stop offset="1" "#)?;
            write_color(out, "stop-color", color, 0.0)?;
            out.write_str("/></radialGradient></defs>")?;
        }
    }
    Ok(true)
}

fn open_pattern(out: &mut impl Write, id: &str, width: f64, height: f64) -> fmt::Result {
    write!(
        out,
        r#"<defs><pattern id="{id}" width="{}" height="{}" patternUnits="userSpaceOnUse">"#,
        canonical_number(width),
        canonical_number(height)
    )
}

fn write_stripe(
    out: &mut impl Write,
    id: &str,
    period: f64,
    color: Color,
    opacity: f64,
) -> fmt::Result {
    open_pattern(out, id, 1.0, period)?;
    out.write_str(r#"<rect width="1" height="1" "#)?;
    write_color(out, "fill", color, opacity)?;
    out.write_str("/></pattern></defs>")
}

pub(crate) fn write_color(
    out: &mut impl Write,
    attribute: &str,
    color: Color,
    opacity: f64,
) -> fmt::Result {
    let [r, g, b, alpha] = color.channels();
    write!(out, r##"{attribute}="#{r:02X}{g:02X}{b:02X}""##)?;
    let opacity = f64::from(alpha) / 255.0 * opacity;
    if opacity != 1.0 {
        let opacity = canonical_number(opacity);
        if attribute == "stop-color" {
            write!(out, r#" stop-opacity="{opacity}""#)?;
        } else {
            write!(out, r#" {attribute}-opacity="{opacity}""#)?;
        }
    }
    Ok(())
}
