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
        let mut typography = ArchitectureTypographyThemePlan::resolve(config);
        let native = native.map(|theme| {
            let native = super::ArchitectureNativeStyle::resolve(theme, config);
            typography.apply_native(theme, &native, config);
            Box::new(native)
        });
        Self { typography, native }
    }

    pub(crate) fn native_style(&self) -> Option<&super::ArchitectureNativeStyle> {
        self.native.as_deref()
    }

    pub(crate) const fn typography_theme(&self) -> &ArchitectureTypographyThemePlan {
        &self.typography
    }
}
