use crate::sanitize::sanitize_text;
use crate::{Error, MermaidConfig, OperationControl, OperationControlResult, Result};

use merman_model::families::mindmap::{MindmapDiagramModel, MindmapNodeModel};

#[derive(Debug, Default)]
pub(super) struct MindmapDb {
    pub(super) nodes: Vec<MindmapNodeModel>,
    base_level: Option<i32>,
    ancestry: Vec<(i32, usize)>,
}

pub(super) struct MindmapNodeInput<'a> {
    pub(super) indent_level: i32,
    pub(super) id_raw: &'a str,
    pub(super) descr_raw: &'a str,
    pub(super) descr_is_markdown: bool,
    pub(super) ty: i32,
    pub(super) diagram_type: &'a str,
}

impl MindmapDb {
    pub(super) fn clear(&mut self) {
        self.nodes.clear();
        self.base_level = None;
        self.ancestry.clear();
    }

    pub(super) fn into_semantic_model(self) -> MindmapDiagramModel {
        MindmapDiagramModel { nodes: self.nodes }
    }

    #[cfg(all(test, feature = "presentation"))]
    pub(super) fn add_node(
        &mut self,
        input: MindmapNodeInput<'_>,
        config: &MermaidConfig,
    ) -> Result<()> {
        let control = OperationControl::new();
        self.add_node_controlled(input, config, &control)
            .expect("a private parse control cannot be cancelled")
    }

    pub(super) fn add_node_controlled(
        &mut self,
        input: MindmapNodeInput<'_>,
        config: &MermaidConfig,
        control: &OperationControl,
    ) -> OperationControlResult<Result<()>> {
        control.checkpoint()?;
        let mut level = input.indent_level;
        let is_root;
        if self.nodes.is_empty() {
            self.base_level = Some(level);
            level = 0;
            is_root = true;
        } else if let Some(base) = self.base_level {
            level -= base;
            is_root = false;
        } else {
            is_root = false;
        }

        let id = self.nodes.len() as i32;
        control.checkpoint()?;
        let node = MindmapNodeModel {
            id,
            node_id: sanitize_text(input.id_raw, config),
            level,
            descr: if input.descr_is_markdown {
                input.descr_raw.to_string()
            } else {
                sanitize_text(input.descr_raw, config)
            },
            descr_is_markdown: input.descr_is_markdown,
            node_type: input.ty,
            children: Vec::new(),
            class: None,
            icon: None,
            is_root,
        };

        let mut popped = 0usize;
        while self
            .ancestry
            .last()
            .is_some_and(|(ancestor_level, _)| *ancestor_level >= level)
        {
            if popped.is_multiple_of(128) {
                control.checkpoint()?;
            }
            self.ancestry.pop();
            popped = popped.saturating_add(1);
        }

        if let Some((_, parent_idx)) = self.ancestry.last().copied() {
            self.nodes[parent_idx].children.push(id);
            self.nodes.push(node);
            self.ancestry.push((level, id as usize));
            control.checkpoint()?;
            return Ok(Ok(()));
        }

        if is_root {
            self.nodes.push(node);
            self.ancestry.push((level, id as usize));
            control.checkpoint()?;
            return Ok(Ok(()));
        }

        Ok(Err(Error::diagram_parse_fallback(
            input.diagram_type.to_string(),
            format!(
                "There can be only one root. No parent could be found for (\"{}\")",
                node.descr
            ),
        )))
    }

    pub(super) fn decorate_last(
        &mut self,
        class: Option<String>,
        icon: Option<String>,
        config: &MermaidConfig,
    ) {
        let Some(last) = self.nodes.last_mut() else {
            return;
        };
        if let Some(icon) = icon {
            last.icon = Some(sanitize_text(&icon, config));
        }
        if let Some(class) = class {
            last.class = Some(sanitize_text(&class, config));
        }
    }
}
