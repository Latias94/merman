use super::{
    NODE_TYPE_BANG, NODE_TYPE_CIRCLE, NODE_TYPE_CLOUD, NODE_TYPE_DEFAULT, NODE_TYPE_HEXAGON,
    NODE_TYPE_RECT, NODE_TYPE_ROUNDED_RECT,
};
use crate::MermaidConfig;

const MINDMAP_SECTION_COUNT: usize = 11;

/// Shape and section rules shared by graphical and terminal Mindmap consumers.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MindmapDisplayPolicy {
    default_shape: &'static str,
    color_by_depth: bool,
}

impl MindmapDisplayPolicy {
    /// Reads the already-admitted configuration without resolving a theme or dimensions.
    pub fn from_config(config: &MermaidConfig) -> Self {
        let redux = config
            .get_str("theme")
            .unwrap_or_default()
            .as_bytes()
            .windows(b"redux".len())
            .any(|part| part.eq_ignore_ascii_case(b"redux"));
        Self {
            default_shape: if redux {
                "rounded"
            } else {
                "defaultMindmapNode"
            },
            color_by_depth: config.get_str("mindmap.colorBy") == Some("depth"),
        }
    }

    pub fn shape(self, node_type: i32) -> &'static str {
        match node_type {
            NODE_TYPE_CIRCLE => "mindmapCircle",
            NODE_TYPE_RECT => "rect",
            NODE_TYPE_ROUNDED_RECT => "rounded",
            NODE_TYPE_CLOUD => "cloud",
            NODE_TYPE_BANG => "bang",
            NODE_TYPE_HEXAGON => "hexagon",
            NODE_TYPE_DEFAULT => self.default_shape,
            _ => "rect",
        }
    }

    /// Resolves one visited node's section. Depth is zero at its traversal root.
    pub fn section(self, level: i32, inherited: Option<i32>, depth: usize) -> Option<i32> {
        if level == 0 {
            None
        } else if self.color_by_depth {
            Some((depth.saturating_sub(1) % MINDMAP_SECTION_COUNT) as i32)
        } else {
            inherited
        }
    }

    pub fn child_section(
        self,
        parent_level: i32,
        child_index: usize,
        inherited: Option<i32>,
    ) -> Option<i32> {
        if parent_level == 0 {
            Some((child_index % MINDMAP_SECTION_COUNT) as i32)
        } else {
            inherited
        }
    }
}
