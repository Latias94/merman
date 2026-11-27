use merman_render_core::Bounds;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitGraphBranchLayout {
    pub name: String,
    pub index: i64,
    pub pos: f64,
    pub bbox_width: f64,
    pub bbox_height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitGraphCommitLayout {
    pub id: String,
    pub message: String,
    pub seq: i64,
    pub commit_type: i64,
    #[serde(default)]
    pub custom_type: Option<i64>,
    #[serde(default)]
    pub custom_id: Option<bool>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub parents: Vec<String>,
    pub branch: String,
    pub pos: f64,
    pub pos_with_offset: f64,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitGraphArrowLayout {
    pub from: String,
    pub to: String,
    pub class_index: i64,
    pub d: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitGraphDiagramLayout {
    pub bounds: Option<Bounds>,
    pub direction: String,
    pub rotate_commit_label: bool,
    pub show_branches: bool,
    pub show_commit_label: bool,
    pub parallel_commits: bool,
    pub diagram_padding: f64,
    pub max_pos: f64,
    pub branches: Vec<GitGraphBranchLayout>,
    pub commits: Vec<GitGraphCommitLayout>,
    pub arrows: Vec<GitGraphArrowLayout>,
}
