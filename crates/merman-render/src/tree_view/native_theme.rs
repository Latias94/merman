use crate::theme::{Color, FamilyThemeAdapter, ThemeCapabilityFlags};

pub(crate) struct TreeViewThemeAdapter;

impl FamilyThemeAdapter for TreeViewThemeAdapter {
    const FAMILY: crate::DiagramFamilyId = crate::DiagramFamilyId::TREE_VIEW;
    const CAPS: ThemeCapabilityFlags = ThemeCapabilityFlags::CANVAS
        .union(ThemeCapabilityFlags::NODE_SURFACE)
        .union(ThemeCapabilityFlags::EMPHASIS)
        .union(ThemeCapabilityFlags::CONNECTOR)
        .union(ThemeCapabilityFlags::TYPOGRAPHY);
}

#[derive(Debug)]
pub(crate) struct TreeViewNativeStyle {
    pub(crate) node_fill: Color,
    pub(crate) on_highlight: Color,
    pub(crate) label_weight: String,
    pub(crate) directory_weight: String,
}
