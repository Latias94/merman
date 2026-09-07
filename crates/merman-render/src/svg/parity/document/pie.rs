//! Pie DOM projection from canonical geometry, paint and text commands.

use super::*;

impl DocumentSvgEncoder<'_> {
    pub(super) fn emit_pie_path(
        &mut self,
        path_id: &ResourceId,
        style: &PathStyle,
    ) -> Result<bool> {
        let id = path_id.as_str();
        if id == "pie.background" {
            return Ok(false);
        }
        let paint_css = |encoder: &Self, paint: Option<&Paint>| -> Result<String> {
            match paint {
                None => Ok("none".to_string()),
                Some(Paint::Solid { color }) => Ok(format!(
                    "rgba({},{},{},{})",
                    color.red,
                    color.green,
                    color.blue,
                    fmt(f64::from(color.alpha) / 255.0)
                )),
                Some(Paint::Resource { id }) => {
                    Ok(format!("url(#{})", encoder.svg_resource_id(id.as_str())?))
                }
            }
        };
        let mut css = format!(
            "fill:{};fill-rule:{};",
            paint_css(self, style.fill.as_ref())?,
            fill_rule_name(style.fill_rule)
        );
        if let Some(stroke) = &style.stroke {
            write!(css, "stroke:{};stroke-width:{};stroke-linecap:{};stroke-linejoin:{};stroke-miterlimit:{};stroke-dashoffset:{};stroke-dasharray:",
                paint_css(self, Some(&stroke.paint))?, fmt(stroke.width), line_cap(stroke.line_cap), line_join(stroke.line_join), fmt(stroke.miter_limit), fmt(stroke.dash_offset))
                .map_err(|_| invalid("Pie stroke style"))?;
            if stroke.dash_array.is_empty() {
                css.push_str("none");
            }
            for (index, dash) in stroke.dash_array.iter().enumerate() {
                if index != 0 {
                    css.push(',');
                }
                write!(css, "{}", fmt(*dash)).map_err(|_| invalid("Pie dash style"))?;
            }
            css.push(';');
        } else {
            css.push_str("stroke:none;");
        }
        write!(css, "opacity:{};", fmt(self.state.opacity)).map_err(|_| invalid("Pie opacity"))?;
        if self.state.transform != Transform::IDENTITY {
            write!(
                css,
                "transform:matrix({},{},{},{},{},{});",
                fmt(self.state.transform.a),
                fmt(self.state.transform.b),
                fmt(self.state.transform.c),
                fmt(self.state.transform.d),
                fmt(self.state.transform.e),
                fmt(self.state.transform.f)
            )
            .map_err(|_| invalid("Pie transform"))?;
        }
        if let Some(blend) = blend_css(self.state.blend_mode) {
            write!(css, "mix-blend-mode:{blend};").map_err(|_| invalid("Pie blend style"))?;
        }
        let path = self.path_resource(path_id)?;
        let mut opening = String::new();
        if id == "pie.outer" {
            let Some((center, radius)) = circle_from_path(path) else {
                return Ok(false);
            };
            write!(
                opening,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"",
                fmt(center.x),
                fmt(center.y),
                fmt(radius)
            )
            .map_err(|_| invalid("Pie outer circle"))?;
        } else if id.starts_with("pie.legend.") && id.ends_with(".swatch") {
            let Some(bounds) = rectangle_from_path(path) else {
                return Ok(false);
            };
            write!(
                opening,
                "<rect width=\"{}\" height=\"{}\"",
                fmt(bounds.width),
                fmt(bounds.height)
            )
            .map_err(|_| invalid("Pie legend rectangle"))?;
            if bounds.x != 0.0 || bounds.y != 0.0 {
                write!(opening, " x=\"{}\" y=\"{}\"", fmt(bounds.x), fmt(bounds.y))
                    .map_err(|_| invalid("Pie legend origin"))?;
            }
        } else if id.starts_with("pie.slice.") && id.ends_with(".shape") {
            write!(
                opening,
                "<path d=\"{}\"",
                super::super::curve::drawing_path_segments_d_unrounded(&path.segments)
            )
            .map_err(|_| invalid("Pie slice path"))?;
            if let Some(Paint::Solid { color }) = &style.fill {
                write!(opening, " fill=\"{}\"", color_css(*color))
                    .map_err(|_| invalid("Pie slice fill"))?;
            } else if style.fill.is_none() {
                opening.push_str(" fill=\"none\"");
            }
        } else {
            return Ok(false);
        }
        write!(opening, " style=\"{}\"", escaped_attr(&css))
            .map_err(|_| invalid("Pie path style"))?;
        if let Some(class) = self.path_class(path_id) {
            write!(opening, " class=\"{}\"", escaped_attr(&class))
                .map_err(|_| invalid("Pie path class"))?;
        }
        opening.push_str("/>");
        self.output.push_str(&opening);
        Ok(true)
    }

    pub(super) fn emit_compact_pie_text(
        &mut self,
        run: &TextRun,
        semantic_id: Option<&str>,
        text_index: Option<usize>,
    ) -> Result<bool> {
        let Some(styles) = &self.pie_styles else {
            return Ok(false);
        };
        let class = self
            .text_class(run, text_index)
            .map(|class| class.into_owned())
            .or_else(|| {
                semantic_id
                    .filter(|id| id.starts_with("pie.legend."))
                    .map(|_| "legend text".to_string())
            });
        let Some(class) = class else { return Ok(false) };
        if styles.texts.get(class.as_str()).copied().flatten() != Some(&run.style)
            || run.style.stroke.is_some()
            || run.style.font.resource.is_some()
            || run.style.font.weight != 400
            || run.style.font.style != FontStyle::Normal
            || run.style.letter_spacing != 0.0
            || run.baseline != TextBaseline::Alphabetic
            || run.direction != TextDirection::Auto
            || self.state.blend_mode != BlendMode::Normal
            || run.language.is_some()
            || !matches!(run.style.fill, Paint::Solid { .. })
        {
            return Ok(false);
        }
        if class == "slice" && self.state.transform == Transform::IDENTITY {
            write!(
                self.output,
                "<text transform=\"translate({},{})\" class=\"slice\" style=\"text-anchor: {};\"",
                fmt(run.origin.x),
                fmt(run.origin.y),
                text_anchor(run.anchor)
            )
            .map_err(|_| invalid("Pie slice text"))?;
        } else {
            write!(
                self.output,
                "<text x=\"{}\" y=\"{}\"",
                fmt(run.origin.x),
                fmt(run.origin.y)
            )
            .map_err(|_| invalid("Pie text"))?;
            if run.anchor != TextAnchor::Start {
                write!(
                    self.output,
                    " style=\"text-anchor: {};\"",
                    text_anchor(run.anchor)
                )
                .map_err(|_| invalid("Pie text anchor"))?;
            }
            if class != "legend text" {
                write!(self.output, " class=\"{}\"", escaped_attr(&class))
                    .map_err(|_| invalid("Pie text class"))?;
            }
        }
        self.write_state_attrs();
        if run.text.is_empty() {
            self.output.push_str("/>");
        } else {
            self.output.push('>');
            escape_xml_into(&mut self.output, &run.text);
            self.output.push_str("</text>");
        }
        Ok(true)
    }
}
