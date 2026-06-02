pub mod attr_sanitize;
pub mod css_override;
pub mod css_sanitize;
pub mod foreign_object;
pub mod root_background;
pub mod scoped_css;
pub(crate) mod util;

pub use css_override::{CssOverridePolicy, CssOverridePostprocessor};
pub use css_sanitize::SanitizeCssPostprocessor;
pub use foreign_object::{
    DropNativeDuplicateFallbacksPostprocessor, ForeignObjectFallbackPostprocessor,
    StripForeignObjectPostprocessor,
};
pub use root_background::RootBackgroundPostprocessor;
pub use scoped_css::ScopedCssPostprocessor;

pub use attr_sanitize::SanitizeSvgAttributesPostprocessor;
