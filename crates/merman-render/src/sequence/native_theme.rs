use merman_theme_contract::v2::{Elevation, Form};

use crate::theme::{Color, FamilyThemeAdapter, ThemeCapabilityFlags};

pub(crate) struct SequenceThemeAdapter;

impl FamilyThemeAdapter for SequenceThemeAdapter {
    const FAMILY: crate::DiagramFamilyId = crate::DiagramFamilyId::SEQUENCE;
    const CAPS: ThemeCapabilityFlags = ThemeCapabilityFlags::CANVAS
        .union(ThemeCapabilityFlags::NODE_SURFACE)
        .union(ThemeCapabilityFlags::CONTAINER)
        .union(ThemeCapabilityFlags::CALLOUT)
        .union(ThemeCapabilityFlags::EMPHASIS)
        .union(ThemeCapabilityFlags::CONNECTOR)
        .union(ThemeCapabilityFlags::ELEVATION)
        .union(ThemeCapabilityFlags::RADIUS)
        .union(ThemeCapabilityFlags::MARKER_STYLE)
        .union(ThemeCapabilityFlags::TYPOGRAPHY);
}

#[derive(Debug)]
pub(crate) struct SequenceNativeStyle {
    pub(crate) form: Form,
    pub(crate) elevation: Elevation,
    pub(crate) container: Color,
    pub(crate) title: Color,
}
