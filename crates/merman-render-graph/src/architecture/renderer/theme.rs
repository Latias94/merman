use merman_model::presentation::PresentationConfig;

use merman_theme_contract::v2::FamilyTheme;

mod typography;
pub use typography::ArchitectureTypographyThemePlan;

#[derive(Debug)]
pub struct ArchitectureGroupThemePlan {
    typography: ArchitectureTypographyThemePlan,
    native: Option<Box<super::ArchitectureNativeStyle>>,
}

impl ArchitectureGroupThemePlan {
    pub fn resolve(native: Option<&FamilyTheme>, config: &PresentationConfig) -> Self {
        let native_style =
            native.map(|theme| super::ArchitectureNativeStyle::resolve(theme, config));
        let typography =
            ArchitectureTypographyThemePlan::resolve(config, native.zip(native_style.as_ref()));
        Self {
            typography,
            native: native_style.map(Box::new),
        }
    }

    pub fn native_style(&self) -> Option<&super::ArchitectureNativeStyle> {
        self.native.as_deref()
    }

    pub const fn typography_theme(&self) -> &ArchitectureTypographyThemePlan {
        &self.typography
    }
}
