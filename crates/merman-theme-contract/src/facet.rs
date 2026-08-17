use serde::{Deserialize, Serialize};

/// An atomic version 1 rule facet shared by typed authoring and support discovery.
///
/// This is a closed V1 vocabulary. Adding a new public facet requires a new contract version rather
/// than silently changing exhaustive matches against this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeRuleFacetV1 {
    /// Fill paint.
    Fill,
    /// Overall opacity.
    Opacity,
    /// Fill-specific opacity.
    FillOpacity,
    /// Stroke paint.
    StrokePaint,
    /// Stroke width.
    StrokeWidth,
    /// Stroke dash lengths.
    StrokeDasharray,
    /// Stroke line cap.
    StrokeLineCap,
    /// Stroke line join.
    StrokeLineJoin,
    /// Stroke-specific opacity.
    StrokeOpacity,
    /// Corner radius.
    Radius,
    /// Content padding.
    Padding,
    /// Font-family stack.
    FontStack,
    /// Font size.
    FontSize,
    /// Font weight.
    FontWeight,
    /// Font style.
    FontStyle,
    /// Line height.
    LineHeight,
    /// Letter spacing.
    LetterSpacing,
    /// Word spacing.
    WordSpacing,
    /// Text transform.
    TextTransform,
    /// Text decoration.
    TextDecoration,
    /// Text alignment.
    TextAlign,
    /// White-space behavior.
    WhiteSpace,
    /// Text wrapping mode.
    Wrap,
    /// Effect graph reference.
    Effect,
}

impl ThemeRuleFacetV1 {
    /// Stable enumeration view for catalogs and contract round-trip checks.
    pub const ALL: &'static [Self] = &[
        Self::Fill,
        Self::Opacity,
        Self::FillOpacity,
        Self::StrokePaint,
        Self::StrokeWidth,
        Self::StrokeDasharray,
        Self::StrokeLineCap,
        Self::StrokeLineJoin,
        Self::StrokeOpacity,
        Self::Radius,
        Self::Padding,
        Self::FontStack,
        Self::FontSize,
        Self::FontWeight,
        Self::FontStyle,
        Self::LineHeight,
        Self::LetterSpacing,
        Self::WordSpacing,
        Self::TextTransform,
        Self::TextDecoration,
        Self::TextAlign,
        Self::WhiteSpace,
        Self::Wrap,
        Self::Effect,
    ];

    /// Stable wire identifier for this atomic facet.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Fill => "fill",
            Self::Opacity => "opacity",
            Self::FillOpacity => "fill-opacity",
            Self::StrokePaint => "stroke-paint",
            Self::StrokeWidth => "stroke-width",
            Self::StrokeDasharray => "stroke-dasharray",
            Self::StrokeLineCap => "stroke-line-cap",
            Self::StrokeLineJoin => "stroke-line-join",
            Self::StrokeOpacity => "stroke-opacity",
            Self::Radius => "radius",
            Self::Padding => "padding",
            Self::FontStack => "font-stack",
            Self::FontSize => "font-size",
            Self::FontWeight => "font-weight",
            Self::FontStyle => "font-style",
            Self::LineHeight => "line-height",
            Self::LetterSpacing => "letter-spacing",
            Self::WordSpacing => "word-spacing",
            Self::TextTransform => "text-transform",
            Self::TextDecoration => "text-decoration",
            Self::TextAlign => "text-align",
            Self::WhiteSpace => "white-space",
            Self::Wrap => "wrap",
            Self::Effect => "effect",
        }
    }

    /// Parses one known atomic facet identifier.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|facet| facet.id() == id)
    }
}
