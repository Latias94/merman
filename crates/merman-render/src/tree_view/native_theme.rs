use crate::theme::Color;

#[derive(Debug)]
pub(crate) struct TreeViewNativeStyle {
    pub(crate) node_fill: Color,
    pub(crate) on_highlight: Color,
    pub(crate) label_weight: String,
    pub(crate) directory_weight: String,
}
