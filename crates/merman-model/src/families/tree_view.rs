#[cfg(feature = "compat-json")]
pub mod compat;

/// Node type for a TreeView file.
pub const TREE_VIEW_FILE_NODE_TYPE: &str = "file";
/// Node type for a TreeView directory, including the default root.
pub const TREE_VIEW_DIRECTORY_NODE_TYPE: &str = "directory";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TreeViewNode {
    pub id: i64,
    pub level: i64,
    pub name: String,
    #[serde(rename = "nodeType")]
    pub node_type: String,
    #[serde(default, rename = "cssClass", skip_serializing_if = "Option::is_none")]
    pub css_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub children: Vec<TreeViewNode>,
}

impl Default for TreeViewNode {
    fn default() -> Self {
        Self {
            id: 0,
            level: -1,
            name: "/".to_string(),
            node_type: TREE_VIEW_DIRECTORY_NODE_TYPE.to_string(),
            css_class: None,
            icon: None,
            description: None,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TreeViewDiagramModel {
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    pub root: TreeViewNode,
}
