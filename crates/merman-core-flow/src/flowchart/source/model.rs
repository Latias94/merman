use super::{FlowNodeProvenance, LinkToken, TitleKind};

#[derive(Debug, Clone)]
pub(super) struct Node {
    pub id: String,
    pub provenance: FlowNodeProvenance,
    pub label: Option<String>,
    pub label_type: TitleKind,
    pub shape: Option<String>,
    pub icon: Option<String>,
    pub form: Option<String>,
    pub pos: Option<String>,
    pub img: Option<String>,
    pub constraint: Option<String>,
    pub asset_width: Option<f64>,
    pub asset_height: Option<f64>,
    pub styles: Vec<String>,
    pub classes: Vec<String>,
    pub link: Option<String>,
    pub link_target: Option<String>,
    pub have_callback: bool,
}

#[derive(Debug, Clone)]
pub(super) struct Edge {
    pub from: String,
    pub to: String,
    pub id: Option<String>,
    pub link: LinkToken,
    pub label: Option<String>,
    pub label_type: TitleKind,
    pub style: Vec<String>,
    pub classes: Vec<String>,
    pub interpolate: Option<String>,
    pub is_user_defined_id: bool,
    pub animate: Option<bool>,
    pub animation: Option<String>,
    pub step: Option<f64>,
}

#[derive(Debug, Clone)]
pub(super) struct EdgeDefaults {
    pub style: Vec<String>,
    pub interpolate: Option<String>,
}
