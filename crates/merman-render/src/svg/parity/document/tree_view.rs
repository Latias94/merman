//! TreeView source containers projected from public semantic ownership.

use super::*;

impl DocumentSvgEncoder<'_> {
    pub(super) fn begin_tree_view_semantic_group(
        &mut self,
        semantic_id: &str,
        semantic: &SemanticAnnotation,
    ) -> Result<bool> {
        let SvgStructureBody::TreeView(body) = self.svg_body else {
            return Ok(false);
        };
        if semantic.link.is_some() || !self.debug_visibility(semantic.role) {
            return Ok(false);
        }
        let class = body.semantic_classes.get(semantic_id).map(String::as_str);
        let tail = &self.document.commands[self.command_index + 1..];
        let emitted = match (class, semantic.role) {
            (Some("tree-view"), SemanticRole::Document) if semantic_id == "treeView.document" => {
                // Root title/description already have their own labelledby/describedby targets.
                true
            }
            (Some("treeView-node"), SemanticRole::Node) => {
                if !self.tree_view_node_names_are_visible(semantic_id, semantic)? {
                    return Ok(false);
                }
                true
            }
            (
                Some("treeView-node-label-group" | "treeView-node-description-group"),
                SemanticRole::Label,
            ) => {
                let [
                    DrawingCommand::DrawText { run },
                    DrawingCommand::EndSemanticGroup,
                    ..,
                ] = tail
                else {
                    return Ok(false);
                };
                if semantic.description.is_some()
                    || semantic
                        .title
                        .as_deref()
                        .is_some_and(|name| name != run.text)
                    || !matches!(run.obligation, TextObligation::HostText { .. })
                {
                    return Ok(false);
                }
                false
            }
            (Some("treeView-node-line"), SemanticRole::Edge) => {
                if semantic.title.is_some()
                    || semantic.description.is_some()
                    || !matches!(
                        tail,
                        [
                            DrawingCommand::DrawPath { .. },
                            DrawingCommand::EndSemanticGroup,
                            ..
                        ] | [DrawingCommand::EndSemanticGroup, ..]
                    )
                {
                    return Ok(false);
                }
                false
            }
            _ => return Ok(false),
        };
        if emitted {
            self.output.push_str("<g")?;
            if semantic.role == SemanticRole::Document {
                self.output.push_str(" class=\"tree-view\"")?;
            }
            write_semantic_metadata(&mut self.output, self.debug, semantic_id)?;
            self.output.push('>')?;
        }
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted,
            semantic_id: semantic_id.to_owned(),
            projected_transform: Transform::IDENTITY,
        });
        Ok(true)
    }

    fn tree_view_node_names_are_visible(
        &self,
        semantic_id: &str,
        semantic: &SemanticAnnotation,
    ) -> Result<bool> {
        let mut title_visible = semantic.title.is_none();
        let mut description_visible = semantic.description.is_none();
        let mut depth = 0;
        let tail = &self.document.commands[self.command_index + 1..];
        for (index, command) in tail.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            match command {
                DrawingCommand::BeginSemanticGroup { semantic_id: child } => {
                    // Source nodes are siblings, never nested containers. Refuse unexpected
                    // nesting rather than repeatedly scanning overlapping descendant subtrees.
                    if depth != 0 {
                        return Ok(false);
                    }
                    depth += 1;
                    let [
                        DrawingCommand::DrawText { run },
                        DrawingCommand::EndSemanticGroup,
                        ..,
                    ] = &tail[index + 1..]
                    else {
                        return Ok(false);
                    };
                    let Some(annotation) = self.semantics.get(child) else {
                        return Ok(false);
                    };
                    if !self.debug_visibility(SemanticRole::Label)
                        || !matches!(run.obligation, TextObligation::HostText { .. })
                        || annotation.role != SemanticRole::Label
                        || annotation.link.is_some()
                        || annotation.description.is_some()
                        || annotation
                            .title
                            .as_deref()
                            .is_some_and(|name| name != run.text)
                    {
                        return Ok(false);
                    }
                    match child.strip_prefix(semantic_id) {
                        Some(".label") => {
                            title_visible |= semantic.title.as_deref() == Some(run.text.as_str())
                        }
                        Some(".description") => {
                            description_visible |=
                                semantic.description.as_deref() == Some(run.text.as_str())
                        }
                        _ => return Ok(false),
                    }
                }
                DrawingCommand::EndSemanticGroup => {
                    if depth == 0 {
                        return Ok(title_visible && description_visible);
                    }
                    depth -= 1;
                }
                _ => {}
            }
        }
        Ok(false)
    }

    /// Collapsed label/edge scopes keep their diagnostic association on the actual leaf.
    pub(super) fn write_tree_view_leaf_metadata(&mut self) -> Result<()> {
        if matches!(self.svg_body, SvgStructureBody::TreeView(_))
            && let Some(GroupKind::Semantic {
                semantic_id,
                emitted: false,
                ..
            }) = self.groups.last()
        {
            write_semantic_metadata(&mut self.output, self.debug, semantic_id)?;
        }
        Ok(())
    }
}
