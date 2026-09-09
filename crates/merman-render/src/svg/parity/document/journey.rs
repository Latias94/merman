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
        } else if id.starts_with("journey.section.") && body.semantic_classes.contains_key(id) {
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

    fn journey_scope_has_native_name(&self, name: &str) -> Result<bool> {
        let mut remaining = name.as_bytes();
        let mut has_text = false;
        for command in &self.document.commands[self.command_index + 1..] {
            self.session.checkpoint(OperationPhase::Emit)?;
            match command {
                DrawingCommand::BeginSemanticGroup { .. } => return Ok(false),
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
        }
        Ok(false)
    }
}
