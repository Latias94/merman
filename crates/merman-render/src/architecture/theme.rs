use merman_model::presentation::PresentationConfig;

use crate::theme::FamilyTheme;

mod typography;
pub(crate) use typography::ArchitectureTypographyThemePlan;

#[derive(Debug)]
pub(crate) struct ArchitectureGroupThemePlan {
    typography: ArchitectureTypographyThemePlan,
    native: Option<Box<super::ArchitectureNativeStyle>>,
}

impl ArchitectureGroupThemePlan {
    pub(crate) fn resolve(native: Option<&FamilyTheme>, config: &PresentationConfig) -> Self {
        let native_style =
            native.map(|theme| super::ArchitectureNativeStyle::resolve(theme, config));
        let typography =
            ArchitectureTypographyThemePlan::resolve(config, native.zip(native_style.as_ref()));
        Self {
            typography,
            native: native_style.map(Box::new),
        }
    }

    pub(crate) fn native_style(&self) -> Option<&super::ArchitectureNativeStyle> {
        self.native.as_deref()
    }

    pub(crate) const fn typography_theme(&self) -> &ArchitectureTypographyThemePlan {
        &self.typography
    }
}
