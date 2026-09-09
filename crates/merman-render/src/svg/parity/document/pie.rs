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
        let path = self.path_resource(path_id)?;
        // Decide whether this projection applies before emitting any part of the element.
        if id == "pie.outer" {
            let Some((center, radius)) = circle_from_path(path) else {
                return Ok(false);
            };
            write!(
                self.output,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"",
                fmt(center.x),
                fmt(center.y),
                fmt(radius)
            )?;
        } else if id.starts_with("pie.legend.") && id.ends_with(".swatch") {
            let Some(bounds) = rectangle_from_path(path) else {
                return Ok(false);
            };
            write!(
                self.output,
                "<rect width=\"{}\" height=\"{}\"",
                fmt(bounds.width),
                fmt(bounds.height)
            )?;
            if bounds.x != 0.0 || bounds.y != 0.0 {
                write!(
                    self.output,
                    " x=\"{}\" y=\"{}\"",
                    fmt(bounds.x),
                    fmt(bounds.y)
                )?;
            }
        } else if id.starts_with("pie.slice.") && id.ends_with(".shape") {
            write!(
                self.output,
                "<path d=\"{}\"",
                super::super::curve::drawing_path_segments_d_unrounded(&path.segments)
            )?;
            if let Some(Paint::Solid { color, .. }) = &style.fill {
                write!(self.output, " fill=\"{}\"", color_css(*color))?;
            } else if style.fill.is_none() {
                self.output.push_str(" fill=\"none\"")?;
            }
        } else {
            return Ok(false);
        }
        self.output.push_str(" style=\"fill:")?;
        self.write_pie_paint(style.fill.as_ref())?;
        if let Some(Paint::Resource { opacity, .. }) = &style.fill
            && *opacity != 1.0
        {
            write!(self.output, ";fill-opacity:{}", fmt(*opacity))?;
        }
        write!(
            self.output,
            ";fill-rule:{};",
            fill_rule_name(style.fill_rule)
        )?;
        if let Some(stroke) = &style.stroke {
            self.output.push_str("stroke:")?;
            self.write_pie_paint(Some(&stroke.paint))?;
            if let Paint::Resource { opacity, .. } = &stroke.paint
                && *opacity != 1.0
            {
                write!(self.output, ";stroke-opacity:{}", fmt(*opacity))?;
            }
            write!(
                self.output,
                ";stroke-width:{};stroke-linecap:{};stroke-linejoin:{};stroke-miterlimit:{};stroke-dashoffset:{};stroke-dasharray:",
                fmt(stroke.width),
                line_cap(stroke.line_cap),
                line_join(stroke.line_join),
                fmt(stroke.miter_limit),
                fmt(stroke.dash_offset)
            )?;
            if stroke.dash_array.is_empty() {
                self.output.push_str("none")?;
            }
            for (index, dash) in stroke.dash_array.iter().enumerate() {
                if index != 0 {
                    self.output.push(',')?;
                }
                write!(self.output, "{}", fmt(*dash))?;
            }
            self.output.push(';')?;
        } else {
            self.output.push_str("stroke:none;")?;
        }
        write!(self.output, "opacity:{};", fmt(self.state.opacity))?;
        if self.state.transform != Transform::IDENTITY {
            write!(
                self.output,
                "transform:matrix({},{},{},{},{},{});",
                fmt(self.state.transform.a),
                fmt(self.state.transform.b),
                fmt(self.state.transform.c),
                fmt(self.state.transform.d),
                fmt(self.state.transform.e),
                fmt(self.state.transform.f)
            )?;
        }
        if let Some(blend) = blend_css(self.state.blend_mode) {
            write!(self.output, "mix-blend-mode:{blend};")?;
        }
        self.output.push('"')?;
        if let Some(class) = self.path_class(path_id) {
            write!(self.output, " class=\"{}\"", escaped_attr(&class))?;
        }
        self.output.push_str("/>")?;
        Ok(true)
    }

    fn write_pie_paint(&mut self, paint: Option<&Paint>) -> Result<()> {
        match paint {
            None => self.output.push_str("none"),
            Some(paint @ Paint::Solid { color, .. }) => write!(
                self.output,
                "rgba({},{},{},{})",
                color.red,
                color.green,
                color.blue,
                fmt(paint_opacity(paint))
            ),
            Some(Paint::Resource { id, .. }) => {
                let id = self.svg_resource_id(id.as_str())?;
                write!(self.output, "url(#{})", escaped_attr(id))
            }
        }
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
            )?;
        } else {
            write!(
                self.output,
                "<text x=\"{}\" y=\"{}\"",
                fmt(run.origin.x),
                fmt(run.origin.y)
            )?;
            if run.anchor != TextAnchor::Start {
                write!(
                    self.output,
                    " style=\"text-anchor: {};\"",
                    text_anchor(run.anchor)
                )?;
            }
            if class != "legend text" {
                write!(self.output, " class=\"{}\"", escaped_attr(&class))?;
            }
        }
        self.write_state_attrs()?;
        if run.text.is_empty() {
            self.output.push_str("/>")?;
        } else {
            self.output.push('>')?;
            output::escape_xml(&mut self.output, &run.text)?;
            self.output.push_str("</text>")?;
        }
        Ok(true)
    }
}
