use merman_theme_contract::v2::Color;

#[derive(Debug)]
pub(crate) struct TreeViewNativeStyle {
    pub(crate) node_fill: Color,
    pub(crate) on_highlight: Color,
    pub(crate) label_weight: String,
    pub(crate) directory_weight: String,
}
