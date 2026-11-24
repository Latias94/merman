use merman_theme_contract::v2::{Elevation, Form};

use crate::theme::Color;

#[derive(Debug)]
pub(crate) struct SequenceNativeStyle {
    pub(crate) form: Form,
    pub(crate) elevation: Elevation,
    pub(crate) container: Color,
    pub(crate) title: Color,
}
