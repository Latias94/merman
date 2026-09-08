//! Sankey SVG grouping projected from public semantic scopes, never the family model.

use super::*;
use merman_display_list::TextStyle;

/// An exact single-stroke projection, not a second drawing-state interpreter.
/// Any other command shape uses the ordinary serializer with all state intact.
pub(super) struct LinkProjection {
    pub(super) collection: usize,
    pub(super) opacity: f64,
    groups: Vec<BlendMode>,
}

impl LinkProjection {
    pub(super) fn new(
        document: &DrawingListDocument,
        session: &RenderSession,
    ) -> Result<Option<Self>> {
        let mut collection = None;
        for (index, command) in document.commands.iter().enumerate() {
            session.checkpoint(OperationPhase::Emit)?;
            if matches!(command, DrawingCommand::BeginSemanticGroup { semantic_id } if semantic_id == "sankey.links")
                && collection.replace(index).is_some()
            {
                return Ok(None);
            }
        }
        let Some(collection) = collection else {
            return Ok(None);
        };
        let mut cursor = collection + 1;
        let mut opacity = None;
        let mut groups = Vec::new();
        loop {
            session.checkpoint(OperationPhase::Emit)?;
            let remaining = &document.commands[cursor..];
            if matches!(remaining.first(), Some(DrawingCommand::EndSemanticGroup)) {
                return Ok(opacity.map(|opacity| Self {
                    collection,
                    opacity,
                    groups,
                }));
            }
            let [
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup { semantic_id },
                DrawingCommand::SetOpacity {
                    opacity: current_opacity,
                },
                DrawingCommand::SetBlendMode { blend_mode },
                DrawingCommand::DrawPath { style, .. },
                DrawingCommand::EndSemanticGroup,
                DrawingCommand::Restore,
                ..,
            ] = remaining
            else {
                return Ok(None);
            };
            let Some(stroke) = style.stroke.as_ref() else {
                return Ok(None);
            };
            if !semantic_id.starts_with("sankey.link.")
                || style.fill.is_some()
                || stroke.line_cap != LineCap::Butt
                || stroke.line_join != LineJoin::Miter
                || stroke.miter_limit != 4.0
                || !stroke.dash_array.is_empty()
                || stroke.dash_offset != 0.0
                || matches!(stroke.paint, Paint::Solid { color } if color.alpha != 255)
                || opacity.is_some_and(|previous| previous != *current_opacity)
            {
                return Ok(None);
            }
            opacity = Some(*current_opacity);
            groups.push(*blend_mode);
            cursor += 7;
        }
    }

    pub(super) fn contains_path(&self, command_index: usize) -> bool {
        command_index
            .checked_sub(3)
            .is_some_and(|index| self.group_blend(index).is_some())
    }

    fn group_blend(&self, command_index: usize) -> Option<BlendMode> {
        let relative = command_index.checked_sub(self.collection + 2)?;
        if !relative.is_multiple_of(7) {
            return None;
        }
        self.groups.get(relative / 7).copied()
    }
}

// Only properties inherited by the shared label group belong here. Outline strokes remain
// on their individual text commands, so the foreground/background layers cannot double-paint.
fn same_inherited_style(left: &TextStyle, right: &TextStyle) -> bool {
    left.font == right.font
        && left.font_size == right.font_size
        && left.letter_spacing == right.letter_spacing
        && left.fill == right.fill
}

pub(super) fn shared_label_style<'a>(
    document: &'a DrawingListDocument,
    session: &RenderSession,
) -> Result<Option<&'a TextStyle>> {
    let mut shared: Option<&TextStyle> = None;
    for command in &document.commands {
        session.checkpoint(OperationPhase::Emit)?;
        if let DrawingCommand::DrawText { run } = command {
            if run.style.font.resource.is_some() || !matches!(run.style.fill, Paint::Solid { .. }) {
                return Ok(None);
            }
            if let Some(previous) = shared {
                if !same_inherited_style(previous, &run.style) {
                    return Ok(None);
                }
            } else {
                shared = Some(&run.style);
            }
        }
    }
    Ok(shared)
}

impl DocumentSvgEncoder<'_> {
    pub(super) fn emit_compact_sankey_link(
        &mut self,
        path_id: &ResourceId,
        style: &PathStyle,
    ) -> Result<bool> {
        if !self
            .sankey_links
            .as_ref()
            .is_some_and(|projection| projection.contains_path(self.command_index))
        {
            return Ok(false);
        }
        let stroke = style
            .stroke
            .as_ref()
            .ok_or_else(|| invalid("projected Sankey link has no stroke"))?;
        let path = self.path_resource(path_id)?;
        write!(self.output, "<path d=\"{}\"", path_d(&path.segments))?;
        self.write_paint("stroke", &stroke.paint)?;
        write!(self.output, " stroke-width=\"{}\"", fmt(stroke.width))?;
        if self.state.transform != Transform::IDENTITY {
            write!(
                self.output,
                " transform=\"matrix({})\"",
                matrix_attr(self.state.transform)
            )?;
        }
        self.output.push_str("/>")?;
        Ok(true)
    }

    pub(super) fn sankey_css(&self) -> Result<String> {
        // Rasterization is backend-local; geometry and paint remain public commands.
        let mut css = format!(
            "#{} .node rect{{shape-rendering:crispEdges;}}",
            self.diagram_id
        );
        let Some(style) = self.sankey_label_style else {
            return Ok(css);
        };
        let Paint::Solid { color } = style.fill else {
            return Err(invalid("Sankey shared text paint must be solid"));
        };
        let font = self.font_families(&style.font)?;
        write!(css, "#{} .node-labels{{font-family:{};font-weight:{};font-style:{};letter-spacing:{}px;fill:{};fill-opacity:{};}}",
            self.diagram_id,
            font,
            style.font.weight,
            font_style(style.font.style),
            fmt(style.letter_spacing),
            color_css(color),
            fmt(f64::from(color.alpha) / 255.0),
        ).map_err(|_| invalid("SVG component formatting failed"))?;
        Ok(css)
    }

    pub(super) fn emit_compact_sankey_text(
        &mut self,
        run: &TextRun,
        semantic_id: Option<&str>,
    ) -> Result<bool> {
        let SvgStructureBody::Sankey(body) = self.svg_body else {
            return Ok(false);
        };
        if !semantic_id.is_some_and(|id| id.starts_with("sankey.label."))
            || self.sankey_label_style.is_none()
            || run.baseline != TextBaseline::Alphabetic
            || run.direction != TextDirection::Auto
            || run.language.is_some()
            || run.text.contains('\n')
        {
            return Ok(false);
        }
        write!(
            self.output,
            "<text x=\"{}\" y=\"{}\" dy=\"{}em\" text-anchor=\"{}\"",
            fmt(run.origin.x),
            fmt(run.origin.y - body.label_dy_em * run.style.font_size),
            fmt(body.label_dy_em),
            text_anchor(run.anchor)
        )?;
        if let Some(class) = semantic_id.and_then(|id| body.semantic_classes.get(id)) {
            write!(self.output, " class=\"{}\"", escaped_attr(class))?;
        }
        if let Some(stroke) = &run.style.stroke {
            self.write_stroke_style(Some(stroke))?;
            let order = match run.style.paint_order {
                merman_display_list::TextPaintOrder::FillThenStroke => "fill stroke",
                merman_display_list::TextPaintOrder::StrokeThenFill => "stroke fill",
            };
            write!(self.output, " paint-order=\"{order}\"")?;
        }
        self.write_state_attrs()?;
        self.output.push('>')?;
        output::escape_xml(&mut self.output, &run.text)?;
        self.output.push_str("</text>")?;
        Ok(true)
    }

    pub(super) fn write_sankey_referenced_gradient(&mut self, style: &PathStyle) -> Result<()> {
        for paint in style
            .fill
            .iter()
            .chain(style.stroke.iter().map(|stroke| &stroke.paint))
        {
            let Paint::Resource { id } = paint else {
                continue;
            };
            if !self.sankey_inline_gradients.contains(id.as_str())
                || !self.emitted_sankey_gradients.insert(id.as_str().to_owned())
            {
                continue;
            }
            self.session.checkpoint(OperationPhase::Emit)?;
            let Some(DrawingResource::LinearGradient(gradient)) =
                self.resources.get(id.as_str()).copied()
            else {
                return Err(invalid("Sankey inline gradient resource is not linear"));
            };
            let svg_id = self.svg_resource_id(id.as_str())?;
            self.write_linear_gradient(&svg_id, gradient)?;
        }
        Ok(())
    }

    pub(super) fn begin_sankey_semantic_group(&mut self, semantic_id: &str) -> Result<()> {
        // Labels stay in their shared source layer; individual logical label and document
        // scopes do not introduce DOM wrappers. Only the verified single-stroke projection
        // moves equivalent paint/compositing attributes from public commands onto SVG groups.
        let class = (!semantic_id.starts_with("sankey.label."))
            .then(|| self.semantic_extra_class(semantic_id).map(str::to_owned))
            .flatten();
        let emitted = class.is_some();
        let mut projected_transform = Transform::IDENTITY;
        if let Some(class) = class {
            write!(self.output, "<g class=\"{}\"", escaped_attr(&class))?;
            if let Some(projection) = &self.sankey_links {
                if self.command_index == projection.collection {
                    write!(
                        self.output,
                        " fill=\"none\" stroke-opacity=\"{}\"",
                        fmt(projection.opacity)
                    )?;
                }
                if let Some(blend) = projection.group_blend(self.command_index) {
                    write_blend_style(&mut self.output, blend)?;
                }
            }
            if semantic_id == "sankey.labels"
                && let Some(style) = self.sankey_label_style
            {
                write!(self.output, " font-size=\"{}\"", fmt(style.font_size))?;
            }
            if let Some(index) = semantic_id
                .strip_prefix("sankey.node.")
                .and_then(|index| index.parse::<usize>().ok())
                .and_then(|index| index.checked_add(1))
            {
                let prefix = if self.options.diagram_id.is_some() {
                    format!("{}-", self.diagram_id)
                } else {
                    String::new()
                };
                write!(self.output, " id=\"{}node-{index}\"", escaped_attr(&prefix))?;
                let transform = self.state.transform;
                if transform.a == 1.0
                    && transform.b == 0.0
                    && transform.c == 0.0
                    && transform.d == 1.0
                {
                    write!(
                        self.output,
                        " transform=\"translate({},{})\" x=\"{}\" y=\"{}\"",
                        fmt(transform.e),
                        fmt(transform.f),
                        fmt(transform.e),
                        fmt(transform.f)
                    )?;
                } else {
                    write!(
                        self.output,
                        " transform=\"matrix({})\"",
                        matrix_attr(transform)
                    )?;
                }
                projected_transform = transform;
                self.state.transform = Transform::IDENTITY;
            }
            self.output.push('>')?;
        }
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted,
            semantic_id: semantic_id.to_owned(),
            projected_transform,
        });
        Ok(())
    }

    pub(super) fn emit_sankey_node_rect(&mut self, bounds: Rect, style: &PathStyle) -> Result<()> {
        write!(
            self.output,
            "<rect height=\"{}\" width=\"{}\"",
            fmt(bounds.height),
            fmt(bounds.width)
        )?;
        if bounds.x != 0.0 || bounds.y != 0.0 {
            write!(
                self.output,
                " x=\"{}\" y=\"{}\"",
                fmt(bounds.x),
                fmt(bounds.y)
            )?;
        }
        if let Some(fill) = &style.fill {
            self.write_paint("fill", fill)?;
        } else {
            self.output.push_str(" fill=\"none\"")?;
        }
        if style.stroke.is_some() {
            self.write_stroke_style(style.stroke.as_ref())?;
        }
        self.write_state_attrs()?;
        self.output.push_str("/>")?;
        Ok(())
    }
}
