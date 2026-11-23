use std::borrow::Cow;
use unicode_segmentation::UnicodeSegmentation;

use super::{CssScalar, CssValuePolicy, parse_css_value};

const MAX_TEXT_LENGTH_PX: f64 = 1_000_000.0;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) enum HtmlLineHeight {
    #[default]
    Normal,
    Number(f64),
    Pixels(f64),
}

impl HtmlLineHeight {
    pub(crate) fn pixels(self, font_size: f64) -> f64 {
        match self {
            Self::Normal => font_size * 1.5,
            Self::Number(scale) => font_size * scale,
            Self::Pixels(pixels) => pixels,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum HtmlTextTransform {
    #[default]
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}

impl HtmlTextTransform {
    pub(crate) fn apply<'a>(self, text: &'a str, in_word: &mut bool) -> Cow<'a, str> {
        if text.is_empty() {
            return Cow::Borrowed(text);
        }
        let starts_in_word = *in_word;
        *in_word = text.chars().next_back().is_some_and(char::is_alphanumeric);
        match self {
            Self::None => Cow::Borrowed(text),
            Self::Uppercase => Cow::Owned(text.to_uppercase()),
            Self::Lowercase => Cow::Owned(text.to_lowercase()),
            Self::Capitalize => {
                let mut transformed = String::with_capacity(text.len());
                for (index, word) in text.split_word_bounds().enumerate() {
                    let mut characters = word.chars();
                    if let Some(first) = characters.next() {
                        if index == 0 && starts_in_word {
                            transformed.push(first);
                        } else {
                            transformed.extend(first.to_uppercase());
                        }
                        transformed.push_str(characters.as_str());
                    }
                }
                Cow::Owned(transformed)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct HtmlTextSpacing {
    pub(crate) letters: f64,
    pub(crate) words: f64,
}

impl HtmlTextSpacing {
    pub(crate) fn advance(self, text: &str) -> f64 {
        let mut advance = 0.0;
        if self.letters != 0.0 {
            let letters = text
                .graphemes(true)
                .filter(|part| *part != "\u{00ad}")
                .count();
            advance += self.letters * letters as f64;
        }
        if self.words != 0.0 {
            let words = text
                .chars()
                .filter(|ch| {
                    matches!(
                        ch,
                        ' ' | '\u{00a0}' | '\u{1680}' | '\u{2000}'
                            ..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
                    )
                })
                .count();
            advance += self.words * words as f64;
        }
        advance
    }

    pub(crate) fn is_normal(self) -> bool {
        self == Self::default()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct HtmlTextGeometry {
    pub(crate) line_height: HtmlLineHeight,
    pub(crate) spacing: HtmlTextSpacing,
    pub(crate) transform: HtmlTextTransform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HtmlTextProperty {
    LineHeight,
    LetterSpacing,
    WordSpacing,
    Transform,
}

impl HtmlTextProperty {
    pub(crate) const ALL: [Self; 4] = [
        Self::LineHeight,
        Self::LetterSpacing,
        Self::WordSpacing,
        Self::Transform,
    ];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::LineHeight => "line-height",
            Self::LetterSpacing => "letter-spacing",
            Self::WordSpacing => "word-spacing",
            Self::Transform => "text-transform",
        }
    }

    pub(crate) fn named(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|property| property.name() == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Value {
    Initial,
    Inherit,
    Absolute(f64),
    Em(f64),
    Rem(f64),
    Number(f64),
    Transform(HtmlTextTransform),
}

impl Value {
    fn parse(property: HtmlTextProperty, value: &str) -> Option<Self> {
        let analysis = parse_css_value(value, CssValuePolicy::HtmlLabelMeasurement)?.analysis;
        if !analysis.is_single_component() {
            return None;
        }
        use CssScalar::*;
        use HtmlTextProperty as Property;
        match (property, analysis.scalar?) {
            (_, Ident(value)) if matches!(value.as_str(), "inherit" | "unset") => {
                Some(Self::Inherit)
            }
            (_, Ident(value)) if value == "initial" => Some(Self::Initial),
            (Property::Transform, Ident(value)) => Some(Self::Transform(match value.as_str() {
                "none" => HtmlTextTransform::None,
                "uppercase" => HtmlTextTransform::Uppercase,
                "lowercase" => HtmlTextTransform::Lowercase,
                "capitalize" => HtmlTextTransform::Capitalize,
                _ => return None,
            })),
            (Property::Transform, _) => None,
            (_, Ident(value)) if value == "normal" => Some(Self::Initial),
            (Property::LineHeight, Number(value))
                if (0.0..=MAX_TEXT_LENGTH_PX).contains(&value) =>
            {
                Some(Self::Number(value))
            }
            (Property::LineHeight, Percentage(value))
                if (0.0..=MAX_TEXT_LENGTH_PX).contains(&value) =>
            {
                Some(Self::Em(value))
            }
            (_, Number(0.0)) => Some(Self::Absolute(0.0)),
            (_, scalar @ (Px(value) | AbsoluteLengthPx(value) | Em(value) | Rem(value)))
                if value.abs() <= MAX_TEXT_LENGTH_PX
                    && (property != Property::LineHeight || value >= 0.0) =>
            {
                match scalar {
                    Em(_) => Some(Self::Em(value)),
                    Rem(_) => Some(Self::Rem(value)),
                    _ => Some(Self::Absolute(value)),
                }
            }
            _ => None,
        }
    }

    fn pixels(self, font_size: f64, root_font_size: f64) -> Option<f64> {
        let pixels = match self {
            Self::Initial => 0.0,
            Self::Absolute(pixels) => pixels,
            Self::Em(scale) => scale * font_size,
            Self::Rem(scale) => scale * root_font_size,
            _ => return None,
        };
        (pixels.abs() <= MAX_TEXT_LENGTH_PX).then_some(pixels)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct HtmlTextGeometryDeclarations([Option<Value>; 4]);

impl HtmlTextGeometryDeclarations {
    pub(crate) fn is_supported(property: HtmlTextProperty, value: &str) -> bool {
        Value::parse(property, value).is_some()
    }

    #[cfg(feature = "diagram-flowchart")]
    pub(crate) fn is_source_owned(property: HtmlTextProperty, value: &str) -> bool {
        matches!(
            Value::parse(property, value),
            Some(Value::Absolute(_) | Value::Em(_) | Value::Number(_) | Value::Transform(_))
        )
    }

    pub(crate) fn set(&mut self, property: HtmlTextProperty, value: &str) {
        self.0[property as usize] = Value::parse(property, value);
    }

    pub(crate) fn is_empty(self) -> bool {
        self.0.iter().all(Option::is_none)
    }

    pub(crate) fn resolve(
        self,
        mut inherited: HtmlTextGeometry,
        font_size: f64,
        root_font_size: f64,
    ) -> HtmlTextGeometry {
        for (property, value) in HtmlTextProperty::ALL.into_iter().zip(self.0) {
            let Some(value) = value else { continue };
            match (property, value) {
                (_, Value::Inherit) => {}
                (HtmlTextProperty::LineHeight, Value::Initial) => {
                    inherited.line_height = HtmlLineHeight::Normal;
                }
                (HtmlTextProperty::LineHeight, Value::Number(scale)) => {
                    if scale * font_size <= MAX_TEXT_LENGTH_PX {
                        inherited.line_height = HtmlLineHeight::Number(scale);
                    }
                }
                (HtmlTextProperty::LineHeight, value) => {
                    if let Some(pixels) = value.pixels(font_size, root_font_size) {
                        inherited.line_height = HtmlLineHeight::Pixels(pixels);
                    }
                }
                (HtmlTextProperty::Transform, Value::Transform(transform)) => {
                    inherited.transform = transform;
                }
                (HtmlTextProperty::Transform, Value::Initial) => {
                    inherited.transform = HtmlTextTransform::None;
                }
                (HtmlTextProperty::Transform, _) => {}
                (HtmlTextProperty::LetterSpacing, value) => {
                    inherited.spacing.letters = value
                        .pixels(font_size, root_font_size)
                        .unwrap_or(inherited.spacing.letters);
                }
                (HtmlTextProperty::WordSpacing, value) => {
                    inherited.spacing.words = value
                        .pixels(font_size, root_font_size)
                        .unwrap_or(inherited.spacing.words);
                }
            }
        }
        inherited
    }
}
