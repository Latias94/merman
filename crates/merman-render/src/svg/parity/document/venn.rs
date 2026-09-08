//! Venn's source DOM projected from public geometry, paint, and text commands.

use super::*;
use merman_display_list::StrokeStyle;

/// Recognize one complete independent paint scope, without changing encoder state.
fn paint_scope(
    commands: &[DrawingCommand],
    index: usize,
) -> Option<(&ResourceId, &PathStyle, f64, usize)> {
    match commands.get(index)? {
        DrawingCommand::DrawPath { path, style } => Some((path, style, 1.0, 1)),
        DrawingCommand::Save => {
            let [
                DrawingCommand::Save,
                DrawingCommand::SetOpacity { opacity },
                DrawingCommand::DrawPath { path, style },
                DrawingCommand::Restore,
            ] = commands.get(index..index.checked_add(4)?)?
            else {
                return None;
            };
            Some((path, style, *opacity, 4))
        }
        _ => None,
    }
}

fn compact_stroke(stroke: &StrokeStyle) -> bool {
    matches!(stroke.paint, Paint::Solid { .. })
        && stroke.dash_array.is_empty()
        && stroke.dash_offset == 0.0
        && stroke.line_cap == LineCap::Butt
        && stroke.line_join == LineJoin::Miter
        && stroke.miter_limit == 4.0
}

impl DocumentSvgEncoder<'_> {
    pub(super) fn begin_venn_semantic_group(
        &mut self,
        id: &str,
        semantic: &SemanticAnnotation,
    ) -> Result<bool> {
        let SvgStructureBody::Venn(body) = self.svg_body else {
            return Ok(false);
        };
        let area = body.semantic_data_sets.contains_key(id);
        if semantic.link.is_some()
            || !(area || matches!(id, "venn.document" | "venn.content" | "venn.title"))
        {
            return Ok(false);
        }
        let emitted = area || id == "venn.content";
        let projected_transform = if emitted {
            self.state.transform
        } else {
            Transform::IDENTITY
        };
        if emitted {
            self.output.push_str("<g")?;
            if let Some(class) = body.semantic_classes.get(id) {
                write!(self.output, " class=\"{}\"", escaped_attr(class))?;
            }
            if let Some(sets) = body.semantic_data_sets.get(id) {
                write!(self.output, " data-venn-sets=\"{}\"", escaped_attr(sets))?;
            }
            if projected_transform != Transform::IDENTITY || id == "venn.content" {
                let t = projected_transform;
                if t.a == 1.0 && t.b == 0.0 && t.c == 0.0 && t.d == 1.0 {
                    write!(
                        self.output,
                        " transform=\"translate({}, {})\"",
                        fmt(t.e),
                        fmt(t.f)
                    )?;
                } else {
                    write!(self.output, " transform=\"matrix({})\"", matrix_attr(t))?;
                }
                self.state.transform = Transform::IDENTITY;
            }
            if area && !self.debug_visibility(semantic.role) {
                self.output.push_str(" display=\"none\"")?;
            }
            self.output.push('>')?;
        }
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted,
            semantic_id: id.to_owned(),
            projected_transform,
        });
        Ok(true)
    }

    fn is_venn_area_path(&self, path: &ResourceId) -> bool {
        let SvgStructureBody::Venn(body) = self.svg_body else {
            return false;
        };
        path.as_str().strip_suffix(".shape").is_some_and(|id| {
            self.current_semantic_id() == Some(id) && body.semantic_data_sets.contains_key(id)
        })
    }

    pub(super) fn emit_venn_fill_and_stroke(&mut self, index: usize) -> Result<Option<usize>> {
        if !matches!(self.svg_body, SvgStructureBody::Venn(_))
            || self.state.opacity != 1.0
            || self.state.blend_mode != BlendMode::Normal
        {
            return Ok(None);
        }
        let Some((path, fill, fill_opacity, fill_count)) =
            paint_scope(&self.document.commands, index)
        else {
            return Ok(None);
        };
        if !self.is_venn_area_path(path)
            || fill.stroke.is_some()
            || fill.fill_rule != FillRule::NonZero
        {
            return Ok(None);
        }
        let Some(Paint::Solid { color }) = fill.fill.as_ref() else {
            return Ok(None);
        };
        let Some((stroke_path, stroke, stroke_opacity, stroke_count)) =
            paint_scope(&self.document.commands, index + fill_count)
        else {
            return Ok(None);
        };
        if path != stroke_path || stroke.fill.is_some() || stroke.fill_rule != FillRule::NonZero {
            return Ok(None);
        }
        let Some(stroke) = stroke
            .stroke
            .as_ref()
            .filter(|stroke| compact_stroke(stroke))
        else {
            return Ok(None);
        };
        self.write_venn_path(
            path,
            Some((*color, fill_opacity)),
            Some((stroke, stroke_opacity)),
        )?;
        Ok(Some(fill_count + stroke_count))
    }

    pub(super) fn emit_venn_path(&mut self, path: &ResourceId, style: &PathStyle) -> Result<bool> {
        if !self.is_venn_area_path(path)
            || style.fill_rule != FillRule::NonZero
            || self.state.blend_mode != BlendMode::Normal
            || (style.fill.is_some() && style.stroke.is_some() && self.state.opacity != 1.0)
        {
            return Ok(false);
        }
        let fill = match style.fill.as_ref() {
            Some(Paint::Solid { color }) => Some((*color, self.state.opacity)),
            None => None,
            _ => return Ok(false),
        };
        if style
            .stroke
            .as_ref()
            .is_some_and(|stroke| !compact_stroke(stroke))
        {
            return Ok(false);
        }
        self.write_venn_path(
            path,
            fill,
            style
                .stroke
                .as_ref()
                .map(|stroke| (stroke, self.state.opacity)),
        )?;
        Ok(true)
    }

    fn write_venn_path(
        &mut self,
        id: &ResourceId,
        fill: Option<(Color, f64)>,
        stroke: Option<(&StrokeStyle, f64)>,
    ) -> Result<()> {
        let path = self.path_resource(id)?;
        write!(
            self.output,
            "<path d=\"{}\" style=\"",
            path_d(&path.segments)
        )?;
        if let Some((color, opacity)) = fill {
            write!(
                self.output,
                "fill-opacity: {}; fill: {};",
                fmt(f64::from(color.alpha) / 255.0 * opacity),
                color_css(color)
            )?;
        } else {
            self.output
                .push_str("fill-opacity: 0; fill: transparent;")?;
        }
        if let Some((stroke, opacity)) = stroke {
            let Paint::Solid { color } = stroke.paint else {
                return Err(invalid("compact Venn stroke must be solid"));
            };
            write!(
                self.output,
                " stroke: {}; stroke-width: {}; stroke-opacity: {};",
                color_css(color),
                fmt(stroke.width),
                fmt(f64::from(color.alpha) / 255.0 * opacity)
            )?;
        }
        self.output.push('"')?;
        self.write_transform_and_blend()?;
        self.output.push_str("/>")
    }

    pub(super) fn emit_compact_venn_text(
        &mut self,
        run: &TextRun,
        semantic_id: Option<&str>,
    ) -> Result<bool> {
        let SvgStructureBody::Venn(body) = self.svg_body else {
            return Ok(false);
        };
        let Some(class) = semantic_id.and_then(|id| body.text_classes.get(id)) else {
            return Ok(false);
        };
        let title = class == "venn-title";
        if (!title && class != "label")
            || run.direction != TextDirection::Auto
            || run.language.is_some()
            || run.text.contains(['\n', '\r'])
            || run.style.stroke.is_some()
            || run.style.letter_spacing != 0.0
            || self.state.blend_mode != BlendMode::Normal
            || run.anchor != TextAnchor::Middle
            || run.baseline
                != if title {
                    TextBaseline::Middle
                } else {
                    TextBaseline::Alphabetic
                }
        {
            return Ok(false);
        }
        let Paint::Solid { color } = run.style.fill else {
            return Ok(false);
        };
        let font = self.font_families(&run.style.font)?;
        let y = if title {
            run.origin.y
        } else {
            run.origin.y - 0.35 * run.style.font_size
        };
        write!(
            self.output,
            "<text class=\"{}\" text-anchor=\"middle\"",
            escaped_attr(class)
        )?;
        if title {
            self.output.push_str(" dominant-baseline=\"middle\"")?;
            write!(self.output, " font-size=\"{}px\"", fmt(run.style.font_size))?;
        } else {
            self.output.push_str(" dy=\".35em\"")?;
        }
        // Venn's root uses the document viewport directly, without the generic padding option.
        if title
            && self.document.viewport.bounds.x == 0.0
            && run.origin.x == self.document.viewport.bounds.width / 2.0
        {
            self.output.push_str(" x=\"50%\"")?;
        } else {
            write!(self.output, " x=\"{}\"", fmt(run.origin.x))?;
        }
        write!(
            self.output,
            " y=\"{}\" style=\"fill: {}; fill-opacity: {}; font-size: {}px; font-family: {}; font-weight: {}; font-style: {};\"",
            fmt(y),
            color_css(color),
            fmt(f64::from(color.alpha) / 255.0),
            fmt(run.style.font_size),
            escaped_attr(&font),
            run.style.font.weight,
            font_style(run.style.font.style)
        )?;
        self.write_state_attrs()?;
        self.output.push('>')?;
        if !title {
            write!(
                self.output,
                "<tspan x=\"{}\" y=\"{}\" dy=\"0.35em\">",
                fmt(run.origin.x),
                fmt(y)
            )?;
        }
        output::escape_xml(&mut self.output, &run.text)?;
        if !title {
            self.output.push_str("</tspan>")?;
        }
        self.output.push_str("</text>")?;
        Ok(true)
    }
}
