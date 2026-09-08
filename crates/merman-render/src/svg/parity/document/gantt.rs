//! Gantt SVG chrome; geometry and paint are owned by the public command stream.

use super::*;

impl DocumentSvgEncoder<'_> {
    pub(super) fn emit_gantt_section_text(&mut self, index: usize) -> Result<Option<usize>> {
        let SvgStructureBody::Gantt(body) = self.svg_body else {
            return Ok(None);
        };
        if self.state.opacity != 1.0 || self.state.blend_mode != BlendMode::Normal {
            return Ok(None);
        }
        let document = self.document;
        let Some(
            [
                DrawingCommand::BeginSemanticGroup { semantic_id },
                DrawingCommand::DrawText { run: first },
                ..,
            ],
        ) = document.commands.get(index..)
        else {
            return Ok(None);
        };
        if !semantic_id.starts_with("gantt.section.")
            || !self.semantics.get(semantic_id).is_some_and(|semantic| {
                semantic.link.is_none()
                    && semantic.description.is_none()
                    && self.debug_visibility(semantic.role)
            })
        {
            return Ok(None);
        }
        let Some(class) = body.text_classes.get(semantic_id) else {
            return Ok(None);
        };
        let Paint::Solid { color } = first.style.fill else {
            return Ok(None);
        };
        // Combining translucent or differently styled text could change overlap compositing.
        if color.alpha != 255 || first.style.stroke.is_some() || first.style.font.resource.is_some()
        {
            return Ok(None);
        }
        let mut count = 0;
        loop {
            self.session.checkpoint(OperationPhase::Emit)?;
            match document.commands.get(index + 1 + count) {
                Some(DrawingCommand::DrawText { run })
                    if run.style == first.style
                        && run.baseline == TextBaseline::Central
                        && run.anchor == TextAnchor::Start
                        && run.direction == TextDirection::Auto
                        && run.language.is_none()
                        && matches!(&run.obligation, TextObligation::HostText { .. })
                        && !run.text.contains(['\n', '\r'])
                        && run.origin.x == first.origin.x =>
                {
                    count += 1
                }
                Some(DrawingCommand::EndSemanticGroup) => break,
                _ => return Ok(None),
            }
        }
        if count == 0 {
            return Ok(None);
        }
        let dy = -(count as f64 - 1.0) / 2.0;
        let center_y = first.origin.y - dy * first.style.font_size;
        if !center_y.is_finite() || center_y + dy * first.style.font_size != first.origin.y {
            return Ok(None);
        }
        let commands = &document.commands[index + 1..index + 1 + count];
        let last_text = commands.iter().rposition(|command| {
            matches!(command,
                DrawingCommand::DrawText { run } if crate::gantt::section_line_has_text(&run.text)
            )
        });
        let mut cursor =
            crate::gantt::GanttSectionLineCursor::new(center_y, first.style.font_size, count);
        for (line_index, command) in commands.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let DrawingCommand::DrawText { run } = command else {
                return Ok(None);
            };
            let has_later_text = last_text.is_some_and(|last| line_index < last);
            if !cursor
                .normalized_parts(&run.text, has_later_text)
                .flat_map(str::bytes)
                .eq(run.text.bytes())
            {
                return Ok(None);
            }
            if cursor.next_line(&run.text, has_later_text) != run.origin.y {
                return Ok(None);
            }
        }
        let font = self.font_families(&first.style.font)?;
        write!(
            self.output,
            "<text xml:space=\"preserve\" dy=\"{}em\" x=\"{}\" y=\"{}\" font-size=\"{}\" class=\"{}\" style=\"font-family:",
            fmt(dy),
            fmt(first.origin.x),
            fmt(center_y),
            fmt(first.style.font_size),
            escaped_attr(class)
        )?;
        output::escape_attr(&mut self.output, &font)?;
        write!(
            self.output,
            ";font-weight:{};font-style:{};letter-spacing:{}px;text-anchor:start;fill:{};stroke:none;\"",
            first.style.font.weight,
            font_style(first.style.font.style),
            fmt(first.style.letter_spacing),
            color_css(color)
        )?;
        self.write_state_attrs()?;
        self.output.push('>')?;
        for (line_index, command) in document.commands[index + 1..index + 1 + count]
            .iter()
            .enumerate()
        {
            self.session.checkpoint(OperationPhase::Emit)?;
            let DrawingCommand::DrawText { run } = command else {
                return Err(invalid("Gantt section projection requires text commands"));
            };
            write!(
                self.output,
                "<tspan alignment-baseline=\"central\" x=\"{}\"",
                fmt(run.origin.x)
            )?;
            if line_index != 0 {
                self.output.push_str(" dy=\"1em\"")?;
            }
            self.output.push('>')?;
            output::escape_xml(&mut self.output, &run.text)?;
            self.output.push_str("</tspan>")?;
        }
        self.output.push_str("</text>")?;
        Ok(Some(count + 2))
    }

    // Fuse only an exact semantic-owned layer. An edited sequence with more commands or scopes
    // retains the ordinary serializer rather than losing those commands during DOM projection.
    fn gantt_tick_layer_at(&self, index: usize) -> bool {
        if !matches!(self.svg_body, SvgStructureBody::Gantt(_)) {
            return false;
        }
        let Some(
            [
                DrawingCommand::BeginSemanticGroup { semantic_id },
                DrawingCommand::BeginLayer { .. },
                DrawingCommand::DrawPath { .. },
                DrawingCommand::DrawText { .. },
                DrawingCommand::EndLayer,
                DrawingCommand::EndSemanticGroup,
                ..,
            ],
        ) = self.document.commands.get(index..)
        else {
            return false;
        };
        semantic_id.starts_with("gantt.axis.")
            && semantic_id.contains(".tick.")
            && self.semantics.get(semantic_id).is_some_and(|semantic| {
                semantic.link.is_none()
                    && semantic.description.is_none()
                    && self.debug_visibility(semantic.role)
            })
    }

    pub(super) fn begin_gantt_tick_layer(
        &mut self,
        opacity: f64,
        blend: BlendMode,
    ) -> Result<bool> {
        let Some(index) = self.command_index.checked_sub(1) else {
            return Ok(false);
        };
        if !self.gantt_tick_layer_at(index) {
            return Ok(false);
        }
        write!(
            self.output,
            "<g class=\"tick\" opacity=\"{}\"",
            fmt(opacity)
        )?;
        self.write_gantt_group_transform()?;
        write_blend_style(&mut self.output, blend)?;
        self.output.push('>')?;
        self.groups.push(GroupKind::Layer {
            projected_transform: self.state.transform,
        });
        self.state.transform = Transform::IDENTITY;
        Ok(true)
    }

    fn write_gantt_group_transform(&mut self) -> Result<()> {
        let transform = self.state.transform;
        if transform.a == 1.0 && transform.b == 0.0 && transform.c == 0.0 && transform.d == 1.0 {
            write!(
                self.output,
                " transform=\"translate({},{})\"",
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
        Ok(())
    }

    pub(super) fn begin_gantt_collection(&mut self, semantic_id: &str) -> Result<bool> {
        if !matches!(self.svg_body, SvgStructureBody::Gantt(_)) {
            return Ok(false);
        }
        let axis = matches!(semantic_id, "gantt.axis.bottom" | "gantt.axis.top");
        if axis
            && self.semantics.get(semantic_id).is_some_and(|semantic| {
                semantic.link.is_some()
                    || semantic.description.is_some()
                    || !self.debug_visibility(semantic.role)
            })
        {
            return Ok(false);
        }
        let emitted = match semantic_id {
            "gantt.document" | "gantt.title" => false,
            "gantt.excludes" | "gantt.rows" | "gantt.tasks" | "gantt.sections" => true,
            "gantt.axis.bottom" | "gantt.axis.top" => true,
            _ if self.gantt_tick_layer_at(self.command_index) => false,
            _ => return Ok(false),
        };
        // These public scopes define paint order; Mermaid exposes only the collection wrappers.
        // The visible title and accessibility references already have their own SVG anchors.
        let projected_transform = if axis {
            self.output.push_str("<g class=\"grid\"")?;
            self.write_gantt_group_transform()?;
            self.output.push('>')?;
            let transform = self.state.transform;
            self.state.transform = Transform::IDENTITY;
            transform
        } else if emitted {
            self.output.push_str("<g>")?;
            Transform::IDENTITY
        } else {
            Transform::IDENTITY
        };
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted,
            semantic_id: semantic_id.to_owned(),
            projected_transform,
        });
        Ok(true)
    }

    pub(super) fn write_gantt_style(&mut self) -> Result<()> {
        // Retain source cursor/rasterization hints only. Legacy paint, font, opacity and
        // milestone-transform CSS would reinterpret or double-apply resolved drawing commands.
        let id = sanitize_svg_id(self.diagram_id.as_str());
        write!(
            self.output,
            "<style>#{id} .grid .tick{{shape-rendering:crispEdges;}}#{id} .clickable{{cursor:pointer;}}"
        )?;
        for index in 0..4 {
            if index != 0 {
                self.output.push(',')?;
            }
            write!(self.output, "#{id} .doneCrit{index}")?;
        }
        self.output
            .push_str("{cursor:pointer;shape-rendering:crispEdges;}</style><g/>")?;
        Ok(())
    }
}
