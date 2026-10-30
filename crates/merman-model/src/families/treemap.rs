//! Flat Treemap data, managed values, and full presentation facts.

mod complexity;
#[cfg(all(test, feature = "presentation-data"))]
mod tests;
pub use complexity::TreemapComplexity;

/// Declared compatibility output presence; manual values emit their model by default.
#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TreemapOutputPresence {
    #[default]
    Model,
    Empty,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TreemapNode {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<usize>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<crate::ManagedJson>,
    #[serde(
        default,
        rename = "classSelector",
        skip_serializing_if = "Option::is_none"
    )]
    pub class_selector: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TreemapDiagramModel {
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub title: Option<String>,
    pub root: TreemapNode,
    /// Flat records; child IDs are indices into this vector.
    pub nodes: Vec<TreemapNode>,
    pub classes: std::collections::BTreeMap<String, TreemapClassDef>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TreemapClassDef {
    pub id: String,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default, rename = "textStyles")]
    pub text_styles: Vec<String>,
}

#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Default)]
pub struct TreemapVisuals {
    pub root_styles: Option<Vec<String>>,
    /// Explicit full-model overrides, indexed by canonical node ID.
    pub node_styles: Vec<Option<Vec<String>>>,
}

#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Default)]
pub struct TreemapDiagramRenderModel {
    pub semantic: TreemapDiagramModel,
    pub visuals: TreemapVisuals,
    output_presence: TreemapOutputPresence,
}

#[cfg(feature = "presentation-data")]
impl std::ops::Deref for TreemapDiagramRenderModel {
    type Target = TreemapDiagramModel;
    fn deref(&self) -> &Self::Target {
        &self.semantic
    }
}

#[cfg(feature = "presentation-data")]
impl std::ops::DerefMut for TreemapDiagramRenderModel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.semantic
    }
}

#[cfg(feature = "presentation-data")]
impl TreemapDiagramRenderModel {
    pub fn from_semantic_model(semantic: TreemapDiagramModel) -> Self {
        Self {
            semantic,
            ..Self::default()
        }
    }

    pub fn compiled_styles(&self, index: Option<usize>) -> Option<&[String]> {
        match index {
            Some(index) => self
                .visuals
                .node_styles
                .get(index)
                .and_then(Option::as_deref),
            None => self.visuals.root_styles.as_deref(),
        }
    }

    /// Assembles declared full-output data without granting source provenance.
    pub fn from_parts(
        semantic: TreemapDiagramModel,
        visuals: TreemapVisuals,
        output_presence: TreemapOutputPresence,
    ) -> Self {
        Self {
            semantic,
            visuals,
            output_presence,
        }
    }

    pub fn output_presence(&self) -> TreemapOutputPresence {
        self.output_presence
    }

    pub fn validate_child_relationships_controlled(
        &self,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<std::result::Result<(), &'static str>> {
        let mut seen = vec![false; self.nodes.len()];
        let mut stack = vec![&self.root];
        while let Some(node) = stack.pop() {
            control.checkpoint()?;
            if let Some(children) = &node.children {
                for &child in children.iter().rev() {
                    control.checkpoint()?;
                    let Some(record) = self.nodes.get(child) else {
                        return Ok(Err("treemap child ID is out of range"));
                    };
                    if std::mem::replace(&mut seen[child], true) {
                        return Ok(Err("cyclic or repeated treemap child relationship"));
                    }
                    stack.push(record);
                }
            }
        }
        Ok(Ok(()))
    }
}
