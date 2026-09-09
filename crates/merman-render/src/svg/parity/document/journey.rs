//! Journey DOM grouping projected from public semantic scopes and positioned text.

use super::*;

impl DocumentSvgEncoder<'_> {
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
