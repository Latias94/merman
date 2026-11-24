use merman_theme_contract::v2::MarkerStyle;

/// An end-facing arrowhead in a 10 by 10 view box. A start marker mirrors it horizontally.
pub(crate) fn arrow_path(style: MarkerStyle) -> Option<&'static str> {
    match style {
        MarkerStyle::Stealth => Some("M0 1L10 5L0 9L2.5 5Z"),
        MarkerStyle::Filled => Some("M0 0.75L10 5L0 9.25Z"),
        MarkerStyle::Open => Some("M1 1L10 5L1 9"),
        MarkerStyle::Classic => None,
    }
}
