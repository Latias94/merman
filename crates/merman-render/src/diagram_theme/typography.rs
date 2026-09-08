use std::collections::BTreeMap;

use super::ThemeCompileValidationError;
use crate::DiagramFamilyId;

pub(crate) const MAX_FONT_STACK_ENTRIES: usize = 32;
pub(crate) const MAX_FONT_FAMILY_BYTES: usize = 256;

/// A property value which distinguishes omission from an explicit clear.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Specified<T> {
    #[default]
    Unspecified,
    Clear,
    Value(T),
}

impl<T> Specified<T> {
    pub const fn is_unspecified(&self) -> bool {
        matches!(self, Self::Unspecified)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum LineHeight {
    #[default]
    Normal,
    Multiplier(f32),
    Px(f32),
}

impl LineHeight {
    pub(crate) fn validate(self) -> Result<(), ThemeCompileValidationError> {
        match self {
            Self::Normal => Ok(()),
            Self::Multiplier(value) if value.is_finite() && value > 0.0 => Ok(()),
            Self::Px(value) if value.is_finite() && value > 0.0 => Ok(()),
            _ => Err(ThemeCompileValidationError::InvalidNumber {
                field: "typography.line_height",
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum TextTransform {
    #[default]
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum TextDecoration {
    #[default]
    None,
    Underline,
    Overline,
    LineThrough,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum WhiteSpace {
    #[default]
    Normal,
    Pre,
    NoWrap,
    PreWrap,
    PreLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum WrapMode {
    #[default]
    Normal,
    BreakWord,
    Anywhere,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontStack {
    families: Vec<String>,
}

impl FontStack {
    pub fn new(
        families: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, ThemeCompileValidationError> {
        let families = families
            .into_iter()
            .map(Into::into)
            .map(|family| validate_font_family(family, "typography.font_stack"))
            .collect::<Result<Vec<_>, _>>()?;
        if families.is_empty() || families.len() > MAX_FONT_STACK_ENTRIES {
            return Err(ThemeCompileValidationError::InvalidCollection {
                field: "typography.font_stack",
            });
        }
        Ok(Self { families })
    }

    pub fn single(family: impl Into<String>) -> Result<Self, ThemeCompileValidationError> {
        Self::new([family])
    }

    pub fn families(&self) -> &[String] {
        &self.families
    }

    pub fn as_css(&self) -> String {
        self.families
            .iter()
            .map(|family| {
                if is_css_generic_family(family) {
                    family.to_ascii_lowercase()
                } else if is_unquoted_css_font_family(family) && !is_css_wide_keyword(family) {
                    family.clone()
                } else {
                    let mut serialized = String::new();
                    cssparser::serialize_string(family, &mut serialized)
                        .expect("serializing a CSS string into String cannot fail");
                    serialized
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn is_unquoted_css_font_family(value: &str) -> bool {
    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return false;
    }

    let mut serialized = String::new();
    cssparser::serialize_identifier(value, &mut serialized)
        .expect("serializing a CSS identifier into String cannot fail");
    serialized == value
}

fn is_css_generic_family(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "serif" | "sans-serif" | "monospace" | "cursive" | "fantasy" | "system-ui"
    )
}

pub(crate) fn is_css_wide_keyword(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "inherit" | "initial" | "revert" | "revert-layer" | "unset"
    )
}

impl Default for FontStack {
    fn default() -> Self {
        Self {
            families: vec![
                "trebuchet ms".to_string(),
                "verdana".to_string(),
                "arial".to_string(),
                "sans-serif".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    font_stack: FontStack,
    font_size_px: f32,
    font_weight: u16,
    font_style: crate::diagram_theme::FontStyle,
    line_height: LineHeight,
    letter_spacing_px: f32,
    word_spacing_px: f32,
    transform: TextTransform,
    decoration: TextDecoration,
    text_align: TextAlign,
    white_space: WhiteSpace,
    wrap: WrapMode,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_stack: FontStack::default(),
            font_size_px: 16.0,
            font_weight: 400,
            font_style: crate::diagram_theme::FontStyle::Normal,
            line_height: LineHeight::Normal,
            letter_spacing_px: 0.0,
            word_spacing_px: 0.0,
            transform: TextTransform::None,
            decoration: TextDecoration::None,
            text_align: TextAlign::Start,
            white_space: WhiteSpace::Normal,
            wrap: WrapMode::Normal,
        }
    }
}

impl TextStyle {
    pub fn with_font_stack(mut self, font_stack: FontStack) -> Self {
        self.font_stack = font_stack;
        self
    }

    pub fn with_font_size_px(mut self, value: f32) -> Result<Self, ThemeCompileValidationError> {
        validate_positive(value, "typography.font_size_px")?;
        self.font_size_px = value;
        Ok(self)
    }

    pub fn with_font_weight(mut self, value: u16) -> Result<Self, ThemeCompileValidationError> {
        if !(1..=1000).contains(&value) {
            return Err(ThemeCompileValidationError::InvalidNumber {
                field: "typography.font_weight",
            });
        }
        self.font_weight = value;
        Ok(self)
    }

    pub fn with_font_style(mut self, value: crate::diagram_theme::FontStyle) -> Self {
        self.font_style = value;
        self
    }

    pub fn with_line_height(
        mut self,
        value: LineHeight,
    ) -> Result<Self, ThemeCompileValidationError> {
        value.validate()?;
        self.line_height = value;
        Ok(self)
    }

    pub fn with_letter_spacing_px(
        mut self,
        value: f32,
    ) -> Result<Self, ThemeCompileValidationError> {
        validate_finite(value, "typography.letter_spacing_px")?;
        self.letter_spacing_px = value;
        Ok(self)
    }

    pub fn with_word_spacing_px(mut self, value: f32) -> Result<Self, ThemeCompileValidationError> {
        validate_finite(value, "typography.word_spacing_px")?;
        self.word_spacing_px = value;
        Ok(self)
    }

    pub fn with_transform(mut self, value: TextTransform) -> Self {
        self.transform = value;
        self
    }

    pub fn with_decoration(mut self, value: TextDecoration) -> Self {
        self.decoration = value;
        self
    }

    pub fn with_text_align(mut self, value: TextAlign) -> Self {
        self.text_align = value;
        self
    }

    pub fn with_white_space(mut self, value: WhiteSpace) -> Self {
        self.white_space = value;
        self
    }

    pub fn with_wrap(mut self, value: WrapMode) -> Self {
        self.wrap = value;
        self
    }

    pub const fn font_stack(&self) -> &FontStack {
        &self.font_stack
    }

    pub const fn font_size_px(&self) -> f32 {
        self.font_size_px
    }

    pub const fn font_weight(&self) -> u16 {
        self.font_weight
    }

    pub const fn font_style(&self) -> crate::diagram_theme::FontStyle {
        self.font_style
    }

    pub const fn line_height(&self) -> LineHeight {
        self.line_height
    }

    pub const fn letter_spacing_px(&self) -> f32 {
        self.letter_spacing_px
    }

    pub const fn word_spacing_px(&self) -> f32 {
        self.word_spacing_px
    }

    pub const fn transform(&self) -> TextTransform {
        self.transform
    }

    pub const fn decoration(&self) -> TextDecoration {
        self.decoration
    }

    pub const fn text_align(&self) -> TextAlign {
        self.text_align
    }

    pub const fn white_space(&self) -> WhiteSpace {
        self.white_space
    }

    pub const fn wrap(&self) -> WrapMode {
        self.wrap
    }

    pub(crate) fn validate(&self) -> Result<(), ThemeCompileValidationError> {
        self.font_size_px.is_finite().then_some(()).ok_or(
            ThemeCompileValidationError::InvalidNumber {
                field: "typography.font_size_px",
            },
        )?;
        validate_positive(self.font_size_px, "typography.font_size_px")?;
        if !(1..=1000).contains(&self.font_weight) {
            return Err(ThemeCompileValidationError::InvalidNumber {
                field: "typography.font_weight",
            });
        }
        self.line_height.validate()?;
        validate_finite(self.letter_spacing_px, "typography.letter_spacing_px")?;
        validate_finite(self.word_spacing_px, "typography.word_spacing_px")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextStylePatch {
    pub font_stack: Specified<FontStack>,
    pub font_size_px: Specified<f32>,
    pub font_weight: Specified<u16>,
    pub font_style: Specified<crate::diagram_theme::FontStyle>,
    pub line_height: Specified<LineHeight>,
    pub letter_spacing_px: Specified<f32>,
    pub word_spacing_px: Specified<f32>,
    pub transform: Specified<TextTransform>,
    pub decoration: Specified<TextDecoration>,
    pub text_align: Specified<TextAlign>,
    pub white_space: Specified<WhiteSpace>,
    pub wrap: Specified<WrapMode>,
}

impl Default for TextStylePatch {
    fn default() -> Self {
        Self {
            font_stack: Specified::Unspecified,
            font_size_px: Specified::Unspecified,
            font_weight: Specified::Unspecified,
            font_style: Specified::Unspecified,
            line_height: Specified::Unspecified,
            letter_spacing_px: Specified::Unspecified,
            word_spacing_px: Specified::Unspecified,
            transform: Specified::Unspecified,
            decoration: Specified::Unspecified,
            text_align: Specified::Unspecified,
            white_space: Specified::Unspecified,
            wrap: Specified::Unspecified,
        }
    }
}

impl TextStylePatch {
    pub(crate) fn validate(&self) -> Result<(), ThemeCompileValidationError> {
        if let Specified::Value(value) = &self.font_stack
            && value.families.is_empty()
        {
            return Err(ThemeCompileValidationError::InvalidCollection {
                field: "style.typography.font_stack",
            });
        }
        if let Specified::Value(value) = self.font_size_px {
            validate_positive(value, "style.typography.font_size_px")?;
        }
        if let Specified::Value(value) = self.font_weight
            && !(1..=1000).contains(&value)
        {
            return Err(ThemeCompileValidationError::InvalidNumber {
                field: "style.typography.font_weight",
            });
        }
        if let Specified::Value(value) = self.line_height {
            value.validate()?;
        }
        if let Specified::Value(value) = self.letter_spacing_px {
            validate_finite(value, "style.typography.letter_spacing_px")?;
        }
        if let Specified::Value(value) = self.word_spacing_px {
            validate_finite(value, "style.typography.word_spacing_px")?;
        }
        Ok(())
    }

    pub(crate) fn apply_to(&self, style: &mut TextStyle, base: &TextStyle) {
        apply_specified(&self.font_stack, &mut style.font_stack, &base.font_stack);
        apply_specified(
            &self.font_size_px,
            &mut style.font_size_px,
            &base.font_size_px,
        );
        apply_specified(&self.font_weight, &mut style.font_weight, &base.font_weight);
        apply_specified(&self.font_style, &mut style.font_style, &base.font_style);
        apply_specified(&self.line_height, &mut style.line_height, &base.line_height);
        apply_specified(
            &self.letter_spacing_px,
            &mut style.letter_spacing_px,
            &base.letter_spacing_px,
        );
        apply_specified(
            &self.word_spacing_px,
            &mut style.word_spacing_px,
            &base.word_spacing_px,
        );
        apply_specified(&self.transform, &mut style.transform, &base.transform);
        apply_specified(&self.decoration, &mut style.decoration, &base.decoration);
        apply_specified(&self.text_align, &mut style.text_align, &base.text_align);
        apply_specified(&self.white_space, &mut style.white_space, &base.white_space);
        apply_specified(&self.wrap, &mut style.wrap, &base.wrap);
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TypographySpec {
    default: TextStyle,
    family_overrides: BTreeMap<String, TextStyle>,
}

impl TypographySpec {
    pub fn with_default(mut self, default: TextStyle) -> Self {
        self.default = default;
        self
    }

    pub fn with_family_style(mut self, family: DiagramFamilyId, style: TextStyle) -> Self {
        self.family_overrides
            .insert(family.as_str().to_string(), style);
        self
    }

    pub const fn default_style(&self) -> &TextStyle {
        &self.default
    }

    pub fn family_style(&self, family: DiagramFamilyId) -> &TextStyle {
        self.family_overrides
            .get(family.as_str())
            .unwrap_or(&self.default)
    }

    pub(crate) fn family_overrides(&self) -> impl ExactSizeIterator<Item = (&str, &TextStyle)> {
        self.family_overrides
            .iter()
            .map(|(family, style)| (family.as_str(), style))
    }

    pub(crate) fn validate(&self) -> Result<(), ThemeCompileValidationError> {
        self.default.validate()?;
        for style in self.family_overrides.values() {
            style.validate()?;
        }
        Ok(())
    }
}

fn validate_font_family(
    family: String,
    field: &'static str,
) -> Result<String, ThemeCompileValidationError> {
    let family = family.trim().to_string();
    if family.is_empty()
        || family.len() > MAX_FONT_FAMILY_BYTES
        || family.chars().any(|character| {
            character.is_control() || matches!(character, ';' | '{' | '}' | '<' | '>')
        })
    {
        return Err(ThemeCompileValidationError::InvalidValue { field });
    }
    Ok(family)
}

fn validate_positive(value: f32, field: &'static str) -> Result<(), ThemeCompileValidationError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(ThemeCompileValidationError::InvalidNumber { field });
    }
    Ok(())
}

fn validate_finite(value: f32, field: &'static str) -> Result<(), ThemeCompileValidationError> {
    if !value.is_finite() {
        return Err(ThemeCompileValidationError::InvalidNumber { field });
    }
    Ok(())
}

fn apply_specified<T: Clone>(value: &Specified<T>, target: &mut T, base: &T) {
    match value {
        Specified::Unspecified => {}
        Specified::Clear => *target = base.clone(),
        Specified::Value(value) => *target = value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::FontStack;

    #[test]
    fn font_stack_quotes_names_that_are_not_valid_unquoted_css_identifiers() {
        let stack = FontStack::new([
            "123Radar",
            "-1Radar",
            "-",
            "Radar2",
            "-apple-system",
            "--radar",
        ])
        .expect("valid bounded font stack");

        assert_eq!(
            stack.as_css(),
            "\"123Radar\", \"-1Radar\", \"-\", Radar2, -apple-system, --radar"
        );
    }
}
