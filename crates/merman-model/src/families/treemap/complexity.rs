use super::TreemapDiagramModel;
use crate::resources::{ModelComplexity, json_value_complexity};

/// Computes Treemap model complexity iteratively without serializing a user-controlled node tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreemapComplexity {
    pub nodes: usize,
    pub classes: usize,
    pub label_bytes: usize,
    pub nesting_depth: usize,
}

impl TreemapComplexity {
    pub fn from_model(model: &TreemapDiagramModel) -> Self {
        Self::from_model_with_styles(model, |_| None)
    }

    #[cfg(feature = "presentation-data")]
    pub fn from_render_model(model: &super::TreemapDiagramRenderModel) -> Self {
        Self::from_model_with_styles(&model.semantic, |index| model.compiled_styles(index))
    }

    fn from_model_with_styles<'a>(
        model: &'a TreemapDiagramModel,
        styles: impl Fn(Option<usize>) -> Option<&'a [String]>,
    ) -> Self {
        let mut nodes = 0usize;
        let mut classes = 0usize;
        let mut label_bytes = [
            model.acc_title.as_deref(),
            model.acc_descr.as_deref(),
            model.title.as_deref(),
        ]
        .into_iter()
        .flatten()
        .fold(0usize, |total, value| total.saturating_add(value.len()));
        let mut nesting_depth = 0usize;

        let mut pending = vec![(&model.root, None, 0usize)];
        let mut visited = vec![false; model.nodes.len()];
        while let Some((node, index, depth)) = pending.pop() {
            nodes = nodes.saturating_add(1);
            nesting_depth = nesting_depth.max(depth);
            label_bytes = label_bytes
                .saturating_add(node.name.len())
                .saturating_add(node.class_selector.as_deref().map_or(0, str::len))
                .saturating_add(
                    styles(index)
                        .unwrap_or(&[])
                        .iter()
                        .map(String::len)
                        .sum::<usize>(),
                );
            if let Some(value) = node.value.as_ref() {
                let value = json_value_complexity(value.as_value(), depth);
                label_bytes = label_bytes.saturating_add(value.text_bytes);
                nodes = nodes.saturating_add(value.items);
                nesting_depth = nesting_depth.max(value.nesting_depth);
            }
            if let Some(children) = node.children.as_ref() {
                for child in children.iter().rev() {
                    if let Some(record) = model.nodes.get(*child)
                        && !visited[*child]
                    {
                        visited[*child] = true;
                        pending.push((record, Some(*child), depth.saturating_add(1)));
                    }
                }
            }
        }

        for (class_name, class_def) in &model.classes {
            classes = classes.saturating_add(1);
            label_bytes = label_bytes
                .saturating_add(class_name.len())
                .saturating_add(class_def.id.len())
                .saturating_add(class_def.styles.iter().map(String::len).sum::<usize>())
                .saturating_add(class_def.text_styles.iter().map(String::len).sum::<usize>());
        }

        Self {
            nodes,
            classes,
            label_bytes,
            nesting_depth,
        }
    }

    pub fn as_model_complexity(self) -> ModelComplexity {
        ModelComplexity::new(
            self.nodes.saturating_add(self.classes),
            self.label_bytes,
            self.nesting_depth,
        )
    }
}
