//! Journey DOM grouping projected from public semantic scopes and positioned text.

use super::*;

impl DocumentSvgEncoder<'_> {
    /// The source legend uses text > tspan with an absolute tspan x. Keep this identity
    /// spelling only for single-line host text; all paint/state still uses the common writer.
    pub(super) fn journey_legend_text(&self, run: &TextRun, semantic_id: Option<&str>) -> bool {
        matches!(self.svg_body, SvgStructureBody::Journey(body)
            if semantic_id.is_some_and(|id| id.starts_with("journey.actor.")
                && body.text_classes.get(id).is_some_and(|class| class == "legend")))
            && !run.text.contains(['\n', '\r'])
    }

    pub(super) fn begin_journey_semantic_group(
        &mut self,
        id: &str,
        semantic: &SemanticAnnotation,
    ) -> Result<bool> {
        let SvgStructureBody::Journey(body) = self.svg_body else {
            return Ok(false);
        };
        let (role, emitted) = if id == "journey.document" {
            (SemanticRole::Document, false)
        } else if id == "journey.activity" {
            (SemanticRole::Edge, false)
        } else if id.starts_with("journey.actor.") && body.semantic_classes.contains_key(id) {
            (SemanticRole::Label, false)
        } else if (id.starts_with("journey.section.") && body.semantic_classes.contains_key(id))
            || (id.starts_with("journey.task.") && id.ends_with(".expression"))
        {
            (SemanticRole::Group, true)
        } else if id.starts_with("journey.task.") && body.semantic_classes.contains_key(id) {
            (SemanticRole::Node, true)
        } else {
            return Ok(false);
        };
        // Edited metadata uses the ordinary semantic projection rather than disappearing when
        // its source scope happened not to have an SVG wrapper.
        if semantic.role != role
            || semantic.link.is_some()
            || !self.debug_visibility(role)
            || (id != "journey.document" && semantic.description.is_some())
            || (id != "journey.document"
                && semantic
                    .title
                    .as_deref()
                    .is_some_and(|name| !name.is_empty())
                && !self.journey_scope_has_native_name(semantic.title.as_deref().unwrap_or(""))?)
        {
            return Ok(false);
        }
        let emitted = emitted || self.debug.include_drawing_list_metadata;
        if emitted {
            self.output.push_str("<g")?;
            write_semantic_metadata(&mut self.output, self.debug, id)?;
            self.output.push('>')?;
        }
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted,
            semantic_id: id.to_owned(),
            projected_transform: Transform::IDENTITY,
        });
        Ok(true)
    }

    pub(super) fn emit_journey_actor(&mut self, index: usize) -> Result<Option<usize>> {
        if !matches!(self.svg_body, SvgStructureBody::Journey(_)) {
            return Ok(None);
        }
        let Some(
            [
                DrawingCommand::BeginSemanticGroup { semantic_id },
                DrawingCommand::DrawPath { path, style },
                DrawingCommand::EndSemanticGroup,
                ..,
            ],
        ) = self.document.commands.get(index..)
        else {
            return Ok(None);
        };
        if !semantic_id.starts_with("journey.task.")
            || !semantic_id.contains(".actor.")
            || path.as_str().strip_suffix(".circle") != Some(semantic_id.as_str())
        {
            return Ok(None);
        }
        let semantic = self
            .semantics
            .get(semantic_id)
            .copied()
            .ok_or_else(|| invalid("Journey actor has no semantic annotation"))?;
        if semantic.role != SemanticRole::Label
            || semantic.link.is_some()
            || !self.debug_visibility(semantic.role)
        {
            return Ok(None);
        }
        let Some((center, radius)) = circle_from_path(self.path_resource(path)?) else {
            return Ok(None);
        };
        write!(
            self.output,
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"",
            fmt(center.x),
            fmt(center.y),
            fmt(radius)
        )?;
        if let Some(class) = self.path_class(path) {
            write!(self.output, " class=\"{}\"", escaped_attr(class.as_ref()))?;
        }
        self.write_fill_stroke_style(style)?;
        self.write_state_attrs()?;
        write_resource_metadata(&mut self.output, self.debug, path.as_str())?;
        write_semantic_metadata(&mut self.output, self.debug, semantic_id)?;
        self.output.push('>')?;
        if let Some(title) = semantic.title.as_deref() {
            self.output.push_str("<title>")?;
            output::escape_xml(&mut self.output, title)?;
            self.output.push_str("</title>")?;
        }
        if let Some(description) = semantic.description.as_deref() {
            self.output.push_str("<desc>")?;
            output::escape_xml(&mut self.output, description)?;
            self.output.push_str("</desc>")?;
        }
        self.output.push_str("</circle>")?;
        Ok(Some(3))
    }

    /// Preserve the source HTML identity shell around one public clipped text scope. The
    /// marked group is the complete native projection, including its authoritative clip.
    pub(super) fn emit_journey_box_text(&mut self, index: usize) -> Result<Option<usize>> {
        let SvgStructureBody::Journey(body) = self.svg_body else {
            return Ok(None);
        };
        if self.state.transform != Transform::IDENTITY
            || self.state.opacity != 1.0
            || self.state.blend_mode != BlendMode::Normal
        {
            return Ok(None);
        }
        let Some(
            [
                DrawingCommand::Save,
                DrawingCommand::ClipPath { path, .. },
                ..,
            ],
        ) = self.document.commands.get(index..)
        else {
            return Ok(None);
        };
        let Some(scope) = self.current_semantic_id() else {
            return Ok(None);
        };
        if path.as_str().strip_suffix(".label.clip") != Some(scope) {
            return Ok(None);
        }
        let Some(class) = body.text_classes.get(scope) else {
            return Ok(None);
        };
        let Some(bounds) = rectangle_from_path(self.path_resource(path)?) else {
            return Ok(None);
        };
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return Ok(None);
        }
        let mut end = index + 2;
        while let Some(DrawingCommand::DrawText { run }) = self.document.commands.get(end) {
            self.session.checkpoint(OperationPhase::Emit)?;
            if !matches!(run.obligation, TextObligation::HostText { .. })
                || run.text.contains(['\n', '\r'])
            {
                return Ok(None);
            }
            end += 1;
        }
        if !matches!(
            self.document.commands.get(end..),
            Some([
                DrawingCommand::Restore,
                DrawingCommand::EndSemanticGroup,
                ..
            ])
        ) {
            return Ok(None);
        }
        // Overflow is governed only by the public clip below. The HTML and nested SVG shells
        // must not add independent clipping or ask the browser to wrap this text a second time.
        write!(
            self.output,
            "<switch><foreignObject x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" overflow=\"visible\"><div xmlns=\"http://www.w3.org/1999/xhtml\" class=\"{}\" style=\"display: table; height: 100%; width: 100%; position: relative;\"><div class=\"label\" style=\"display: table-cell; text-align: center; vertical-align: middle;\"><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" overflow=\"visible\" style=\"position: absolute; left: 0; top: 0; overflow: visible;\"><g transform=\"translate({}, {})\"><g data-merman-native-text=\"v1\">",
            fmt(bounds.x),
            fmt(bounds.y),
            fmt(bounds.width),
            fmt(bounds.height),
            escaped_attr(class),
            fmt(bounds.width),
            fmt(bounds.height),
            fmt(-bounds.x),
            fmt(-bounds.y)
        )?;
        for command in &self.document.commands[index..=end] {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_command(command)?;
        }
        self.output
            .push_str("</g></g></svg></div></div></foreignObject></switch>")?;
        Ok(Some(end - index + 1))
    }

    fn journey_scope_has_native_name(&self, name: &str) -> Result<bool> {
        let mut remaining = name.as_bytes();
        let mut has_text = false;
        let mut commands = &self.document.commands[self.command_index + 1..];
        while let Some((command, rest)) = commands.split_first() {
            self.session.checkpoint(OperationPhase::Emit)?;
            match command {
                DrawingCommand::BeginSemanticGroup { .. } => {
                    // Only fixed non-text child scopes can be skipped. Nested or edited text
                    // keeps the explicit parent name, without an unbounded recursive scan.
                    let consumed = match commands {
                        [
                            DrawingCommand::BeginSemanticGroup { .. },
                            DrawingCommand::DrawPath { .. },
                            DrawingCommand::EndSemanticGroup,
                            ..,
                        ] => 3,
                        [
                            DrawingCommand::BeginSemanticGroup { .. },
                            DrawingCommand::DrawPath { .. },
                            DrawingCommand::DrawPath { .. },
                            DrawingCommand::DrawPath { .. },
                            DrawingCommand::EndSemanticGroup,
                            ..,
                        ] => 5,
                        _ => return Ok(false),
                    };
                    commands = &commands[consumed..];
                    continue;
                }
                DrawingCommand::EndSemanticGroup => return Ok(has_text && remaining.is_empty()),
                DrawingCommand::DrawText { run } => {
                    if has_text {
                        let Some(tail) = remaining.strip_prefix(b"\n") else {
                            return Ok(false);
                        };
                        remaining = tail;
                    }
                    let Some(tail) = remaining.strip_prefix(run.text.as_bytes()) else {
                        return Ok(false);
                    };
                    remaining = tail;
                    has_text = true;
                }
                _ => {}
            }
            commands = rest;
        }
        Ok(false)
    }
}
