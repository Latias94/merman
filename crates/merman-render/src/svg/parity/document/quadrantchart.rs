//! QuadrantChart's DOM spelling, derived only from public drawing and semantic state.

use super::*;
use merman_display_list::FontDescriptor;

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
        let Paint::Solid { color } = run.style.fill else {
            return Ok(false);
        };
        let baseline = match run.baseline {
            TextBaseline::Middle => "middle",
            other => text_baseline(other),
        };
        write!(
            self.output,
            "<text x=\"{}\" y=\"{}\" fill=\"{}\" font-size=\"{}\" dominant-baseline=\"{}\" text-anchor=\"{}\"",
            fmt(run.origin.x),
            fmt(run.origin.y),
            color_css(color),
            fmt(run.style.font_size),
            baseline,
            text_anchor(run.anchor)
        )?;
        if color.alpha != 255 {
            write!(
                self.output,
                " fill-opacity=\"{}\"",
                fmt(f64::from(color.alpha) / 255.0)
            )?;
        }
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
        if !matches!(self.svg_body, SvgStructureBody::QuadrantChart(_))
            || style.fill_rule != FillRule::NonZero
        {
            return Ok(false);
        }
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
            if let Some(fill) = &style.fill {
                self.write_paint("fill", fill)?;
            } else {
                self.output.push_str(" fill=\"none\"")?;
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
}
