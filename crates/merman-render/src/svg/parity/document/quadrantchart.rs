//! QuadrantChart's DOM spelling, derived only from public drawing and semantic state.

use super::*;
use crate::drawing_list::QuadrantChartSvgBody;
use crate::quadrantchart::is_quadrant_inherited_paint;
use merman_display_list::FontDescriptor;

/// Only commands that actually use an invalid fill participate in SVG inheritance.
/// Conflicting public edits disable the hint, leaving explicit per-command paint.
pub(super) fn shared_inherited_fill(
    document: &DrawingListDocument,
    body: &QuadrantChartSvgBody,
    session: &RenderSession,
) -> Result<Option<Color>> {
    let mut shared = None;
    for (index, command) in document.commands.iter().enumerate() {
        session.checkpoint(OperationPhase::Emit)?;
        let (raw, paint) = match command {
            DrawingCommand::DrawPath { path, style } => (
                body.path_paint
                    .get(path.as_str())
                    .and_then(|hint| hint.fill.as_deref()),
                style.fill.as_ref(),
            ),
            DrawingCommand::DrawText { run } => (
                body.text_fill.get(&index).map(String::as_str),
                Some(&run.style.fill),
            ),
            _ => continue,
        };
        if !raw.is_some_and(is_quadrant_inherited_paint) {
            continue;
        }
        let Some(Paint::Solid { color }) = paint else {
            return Ok(None);
        };
        if shared.is_some_and(|previous| previous != *color) {
            return Ok(None);
        }
        shared = Some(*color);
    }
    Ok(shared)
}

fn equivalent_paint_spelling<'a>(
    raw: &'a str,
    paint: Option<&Paint>,
    inherited: Option<Color>,
) -> Option<&'a str> {
    // Unicode trimming would certify spellings containing non-CSS whitespace as valid CSS.
    let token = raw.trim_matches([' ', '\t', '\n', '\r', '\x0c']);
    if is_quadrant_inherited_paint(token) {
        return match paint {
            Some(Paint::Solid { color }) if inherited == Some(*color) => Some(raw),
            _ => None,
        };
    }
    if token.eq_ignore_ascii_case("none") {
        return paint.is_none().then_some(raw);
    }
    // Khroma parsing alone does not prove CSS function syntax. Keep only static hex/name
    // spellings here; function colors use the public, canonical RGBA representation.
    let hex = token.strip_prefix('#').is_some_and(|hex| {
        matches!(hex.len(), 3 | 4 | 6 | 8) && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
    });
    if !hex && !token.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return None;
    }
    let Some(Paint::Solid { color }) = paint else {
        return None;
    };
    let parsed = merman_core::theme_color::ThemeColor::parse(token)
        .ok()?
        .rgba_channels();
    (parsed.red == f64::from(color.red)
        && parsed.green == f64::from(color.green)
        && parsed.blue == f64::from(color.blue)
        && parsed.alpha == f64::from(color.alpha) / 255.0)
        .then_some(raw)
}

fn equivalent_width_spelling(raw: &str, width: Option<f64>) -> Option<&str> {
    let length = raw.parse::<svgtypes::Length>().ok()?;
    let pixels = match length.unit {
        svgtypes::LengthUnit::None | svgtypes::LengthUnit::Px => length.number,
        svgtypes::LengthUnit::Pt => length.number * (4.0 / 3.0),
        _ => return None,
    };
    (pixels.is_finite() && pixels >= 0.0 && width.is_none_or(|width| width == pixels))
        .then_some(raw)
}

pub(super) fn shared_font<'a>(
    document: &'a DrawingListDocument,
    session: &RenderSession,
) -> Result<Option<&'a FontDescriptor>> {
    let mut shared = None;
    for command in &document.commands {
        session.checkpoint(OperationPhase::Emit)?;
        let DrawingCommand::DrawText { run } = command else {
            continue;
        };
        if run.style.font.resource.is_some() || shared.is_some_and(|font| font != &run.style.font) {
            return Ok(None);
        }
        shared = Some(&run.style.font);
    }
    Ok(shared)
}

impl DocumentSvgEncoder<'_> {
    pub(super) fn write_quadrant_style(&mut self) -> Result<()> {
        self.output.push_str("<style>")?;
        if let Some(font) = self.quadrant_font {
            let families = self.font_families(font)?;
            write!(
                self.output,
                "#{}{{font-family:{};font-weight:{};font-style:{};}}",
                self.diagram_id,
                families,
                font.weight,
                font_style(font.style)
            )?;
        }
        if let Some(color) = self.quadrant_inherited_fill {
            // Alpha belongs to the inherited color, not fill-opacity (which would also
            // multiply unrelated explicit fills below the root).
            write!(
                self.output,
                "#{}{{fill:#{:02x}{:02x}{:02x}{:02x};}}",
                self.diagram_id, color.red, color.green, color.blue, color.alpha
            )?;
        }
        self.output.push_str("</style><g/>")
    }

    pub(super) fn begin_quadrant_semantic_group(
        &mut self,
        id: &str,
        semantic: &SemanticAnnotation,
    ) -> Result<bool> {
        let SvgStructureBody::QuadrantChart(body) = self.svg_body else {
            return Ok(false);
        };
        let Some(class) = body.semantic_classes.get(id) else {
            return Ok(false);
        };
        let expected_role = match class.as_str() {
            "main" => SemanticRole::Document,
            "data-point" => SemanticRole::Node,
            "label" => SemanticRole::Label,
            "title" if semantic.role == SemanticRole::Label => SemanticRole::Label,
            _ => SemanticRole::Group,
        };
        if semantic.role != expected_role
            || semantic.link.is_some()
            || !self.debug_visibility(semantic.role)
        {
            return Ok(false);
        }
        if id != "quadrantchart.document" {
            if semantic.description.is_some() {
                return Ok(false);
            }
            if let Some(name) = semantic.title.as_deref() {
                // A native text child names the source group. Independently edited metadata
                // retains the generic accessible wrapper; nested scopes cannot prove its name.
                let mut visible_name = false;
                for command in &self.document.commands[self.command_index + 1..] {
                    self.session.checkpoint(OperationPhase::Emit)?;
                    match command {
                        DrawingCommand::BeginSemanticGroup { .. } => return Ok(false),
                        DrawingCommand::EndSemanticGroup => break,
                        DrawingCommand::DrawText { run } => {
                            visible_name |= run.text == name
                                && matches!(run.obligation, TextObligation::HostText { .. });
                        }
                        _ => {}
                    }
                }
                if !visible_name {
                    return Ok(false);
                }
            }
        }
        write!(self.output, "<g class=\"{}\"", escaped_attr(class))?;
        write_semantic_metadata(&mut self.output, self.debug, id)?;
        self.output.push('>')?;
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted: true,
            semantic_id: id.to_owned(),
            projected_transform: Transform::IDENTITY,
        });
        Ok(true)
    }

    pub(super) fn emit_quadrant_text(&mut self, run: &TextRun) -> Result<bool> {
        if !matches!(self.svg_body, SvgStructureBody::QuadrantChart(_))
            || self.quadrant_font != Some(&run.style.font)
            || run.style.stroke.is_some()
            || run.style.letter_spacing != 0.0
            || run.direction != TextDirection::Auto
            || run.language.is_some()
            || run.text.contains(['\n', '\r'])
        {
            return Ok(false);
        }
        let Paint::Solid { .. } = run.style.fill else {
            return Ok(false);
        };
        let baseline = match run.baseline {
            TextBaseline::Middle => "middle",
            other => text_baseline(other),
        };
        let SvgStructureBody::QuadrantChart(body) = self.svg_body else {
            return Ok(false);
        };
        let spelling = body.text_fill.get(&self.command_index).map(String::as_str);
        write!(
            self.output,
            "<text x=\"{}\" y=\"{}\"",
            fmt(run.origin.x),
            fmt(run.origin.y),
        )?;
        self.write_quadrant_paint("fill", Some(&run.style.fill), spelling)?;
        write!(
            self.output,
            " font-size=\"{}\" dominant-baseline=\"{}\" text-anchor=\"{}\"",
            fmt(run.style.font_size),
            baseline,
            text_anchor(run.anchor)
        )?;
        write_text_metadata(&mut self.output, self.debug, run)?;
        self.write_state_attrs()?;
        self.output.push('>')?;
        output::escape_xml(&mut self.output, &run.text)?;
        self.output.push_str("</text>")?;
        Ok(true)
    }

    pub(super) fn emit_quadrant_path(
        &mut self,
        id: &ResourceId,
        style: &PathStyle,
    ) -> Result<bool> {
        let SvgStructureBody::QuadrantChart(body) = self.svg_body else {
            return Ok(false);
        };
        if style.fill_rule != FillRule::NonZero {
            return Ok(false);
        }
        let hint = body.path_paint.get(id.as_str());
        if id.as_str().starts_with("quadrantchart.quadrant.") && style.stroke.is_none() {
            let path = self.path_resource(id)?;
            let Some(bounds) = rectangle_from_path(path) else {
                return Ok(false);
            };
            write!(
                self.output,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                fmt(bounds.x),
                fmt(bounds.y),
                fmt(bounds.width),
                fmt(bounds.height)
            )?;
            self.write_quadrant_paint(
                "fill",
                style.fill.as_ref(),
                hint.and_then(|hint| hint.fill.as_deref()),
            )?;
        } else if id.as_str().starts_with("quadrantchart.point.") {
            if style.stroke.as_ref().is_some_and(|stroke| {
                !stroke.dash_array.is_empty()
                    || stroke.dash_offset != 0.0
                    || stroke.line_cap != LineCap::Butt
                    || stroke.line_join != LineJoin::Miter
                    || stroke.miter_limit != 4.0
            }) {
                return Ok(false);
            }
            let Some((center, radius)) = circle_from_path(self.path_resource(id)?) else {
                return Ok(false);
            };
            write!(
                self.output,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"",
                fmt(center.x),
                fmt(center.y),
                fmt(radius)
            )?;
            self.write_quadrant_paint(
                "fill",
                style.fill.as_ref(),
                hint.and_then(|hint| hint.fill.as_deref()),
            )?;
            if let Some(stroke) = &style.stroke {
                self.write_quadrant_paint(
                    "stroke",
                    Some(&stroke.paint),
                    hint.and_then(|hint| hint.stroke.as_deref()),
                )?;
                let width = hint
                    .and_then(|hint| hint.stroke_width.as_deref())
                    .and_then(|raw| equivalent_width_spelling(raw, Some(stroke.width)));
                if let Some(width) = width {
                    write!(self.output, " stroke-width=\"{}\"", escaped_attr(width))?;
                } else {
                    write!(self.output, " stroke-width=\"{}\"", fmt(stroke.width))?;
                }
            } else {
                self.write_quadrant_paint(
                    "stroke",
                    None,
                    hint.and_then(|hint| hint.stroke.as_deref()),
                )?;
                if let Some(width) = hint
                    .and_then(|hint| hint.stroke_width.as_deref())
                    .and_then(|raw| equivalent_width_spelling(raw, None))
                {
                    write!(self.output, " stroke-width=\"{}\"", escaped_attr(width))?;
                }
            }
        } else if id.as_str().starts_with("quadrantchart.border.") && style.fill.is_none() {
            let path = self.path_resource(id)?;
            let Some((start, end)) = line_from_path(path) else {
                return Ok(false);
            };
            let Some(stroke) = &style.stroke else {
                return Ok(false);
            };
            let Paint::Solid { color } = stroke.paint else {
                return Ok(false);
            };
            if self.state.blend_mode != BlendMode::Normal
                || color.alpha != 255
                || !stroke.dash_array.is_empty()
                || stroke.dash_offset != 0.0
                || stroke.line_cap != LineCap::Butt
                || stroke.line_join != LineJoin::Miter
                || stroke.miter_limit != 4.0
            {
                return Ok(false);
            }
            write!(
                self.output,
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" style=\"stroke: {}; stroke-width: {};\"",
                fmt(start.x),
                fmt(start.y),
                fmt(end.x),
                fmt(end.y),
                color_css(color),
                fmt(stroke.width)
            )?;
        } else {
            return Ok(false);
        }
        write_resource_metadata(&mut self.output, self.debug, id.as_str())?;
        self.write_state_attrs()?;
        self.output.push_str("/>")?;
        Ok(true)
    }

    fn write_quadrant_paint(
        &mut self,
        attr: &str,
        paint: Option<&Paint>,
        raw: Option<&str>,
    ) -> Result<()> {
        let inherited = if attr == "fill" {
            self.quadrant_inherited_fill
        } else {
            None
        };
        if let Some(spelling) = raw.and_then(|raw| {
            if attr == "stroke" && paint.is_none() && is_quadrant_inherited_paint(raw) {
                Some(raw)
            } else {
                equivalent_paint_spelling(raw, paint, inherited)
            }
        }) {
            write!(self.output, " {attr}=\"{}\"", escaped_attr(spelling))?;
            Ok(())
        } else if let Some(paint) = paint {
            self.write_paint(attr, paint)
        } else {
            write!(self.output, " {attr}=\"none\"")?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paint_spelling_requires_css_syntax_not_only_khroma_channels() {
        let paint = Paint::solid(Color::rgba(17, 34, 51, 255));
        for raw in ["\u{a0}#123", "rgb(17, 34 51)", "rgb(17.1,34,51)"] {
            assert_eq!(equivalent_paint_spelling(raw, Some(&paint), None), None);
        }
        assert_eq!(
            equivalent_paint_spelling(" #123 ", Some(&paint), None),
            Some(" #123 ")
        );
    }
}
