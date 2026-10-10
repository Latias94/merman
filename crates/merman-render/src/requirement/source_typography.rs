//! Requirement node typography shared by measurement and terminal evidence.

use super::RequirementMeasurementStyles;
use crate::Result;
use crate::mermaid_style::{
    CssFontFamilyOwnership, CssFontSizeOwnership, ParsedStyleDeclaration,
    is_static_css_font_family_list, is_supported_css_font_style_value,
    is_supported_css_font_weight_value, parse_style_declaration, visit_parsed_style_declarations,
};
use crate::resources::{OperationWorkMeter, PreparedTextRetainedReservation};
use crate::text::{TextMeasurer, TextMetrics, TextStyle, WrapMode};
use std::borrow::Cow;
use std::sync::Arc;

/// Source declarations are retained once, including when opaque render measurements replay them.
#[derive(Debug, Clone, Default)]
pub(crate) struct RequirementNodeTypography {
    source: Option<Arc<SourceTypography>>,
}

#[derive(Debug)]
struct SourceTypography {
    font_family: Option<String>,
    font_size: Option<String>,
    font_weight: Option<String>,
    font_style: Option<String>,
    font_faces_verified: bool,
    font_family_ownership: CssFontFamilyOwnership,
    font_size_ownership: CssFontSizeOwnership,
    font_size_px: Option<f64>,
    _retained: PreparedTextRetainedReservation,
}

#[derive(Default)]
struct SourceDeclarations<'a> {
    font_family: Option<&'a str>,
    font_size: Option<&'a str>,
    font_weight: Option<&'a str>,
    font_style: Option<&'a str>,
    font_family_inherits: bool,
    font_size_inherits: bool,
}

impl<'a> SourceDeclarations<'a> {
    fn observe(&mut self, declaration: ParsedStyleDeclaration<'a>) {
        // styles2String drops source importance and emits every value as !important.
        // Font shorthand is not a label style in this writer.
        match declaration.property() {
            "font-family" => {
                self.font_family = Some(declaration.value());
                self.font_family_inherits = declaration.inherits_property_value();
            }
            "font-size" => {
                self.font_size = Some(declaration.value());
                self.font_size_inherits = declaration.inherits_property_value();
            }
            "font-weight" => self.font_weight = Some(declaration.value()),
            "font-style" => self.font_style = Some(declaration.value()),
            _ => {}
        }
    }
}

fn static_font_weight(value: &str) -> bool {
    is_supported_css_font_weight_value(value)
        && !value.eq_ignore_ascii_case("bolder")
        && !value.eq_ignore_ascii_case("lighter")
}

fn static_font_size_px(value: &str) -> Option<f64> {
    let mut parser = cssparser::Parser::new(value);
    match parser.next().ok()? {
        cssparser::Token::Number { value, .. } if *value == 0.0 => {}
        cssparser::Token::Dimension { unit, .. } if unit.eq_ignore_ascii_case("px") => {}
        _ => return None,
    }
    parser.expect_exhausted().ok()?;
    crate::mermaid_style::parse_svg_number_or_px(value)
}

impl RequirementNodeTypography {
    pub(crate) fn node_measurer<'a>(
        &'a self,
        measurer: &'a dyn TextMeasurer,
        wrap_mode: WrapMode,
    ) -> RequirementNodeMeasurer<'a> {
        RequirementNodeMeasurer {
            inner: measurer,
            source: self.source.as_deref().filter(|source| {
                wrap_mode == WrapMode::SvgLike
                    && (source
                        .font_weight
                        .as_deref()
                        .is_some_and(static_font_weight)
                        || source
                            .font_style
                            .as_deref()
                            .is_some_and(is_supported_css_font_style_value))
            }),
        }
    }

    pub(super) fn resolve(
        declarations: &[String],
        work_meter: &Arc<OperationWorkMeter>,
    ) -> Result<Self> {
        let mut source = SourceDeclarations::default();
        for raw in declarations {
            work_meter.charge(raw.len().div_ceil(64).saturating_add(1))?;
            if let Some(declaration) = parse_style_declaration(raw) {
                source.observe(declaration);
            }
        }
        if source.font_family.is_none()
            && source.font_size.is_none()
            && source.font_weight.is_none()
            && source.font_style.is_none()
        {
            return Ok(Self::default());
        }
        let retained_bytes = std::mem::size_of::<SourceTypography>()
            .checked_add(2 * std::mem::size_of::<usize>())
            .and_then(|bytes| bytes.checked_add(source.font_family.map_or(0, str::len)))
            .and_then(|bytes| bytes.checked_add(source.font_size.map_or(0, str::len)))
            .and_then(|bytes| bytes.checked_add(source.font_weight.map_or(0, str::len)))
            .and_then(|bytes| bytes.checked_add(source.font_style.map_or(0, str::len)))
            .ok_or_else(|| work_meter.arithmetic_overflow())?;
        let retained = work_meter.reserve_prepared_text_retained_bytes(retained_bytes)?;
        let font_family_ownership = match source.font_family {
            None => CssFontFamilyOwnership::Inherited,
            Some(_) if source.font_family_inherits => CssFontFamilyOwnership::Inherited,
            Some(value) if is_static_css_font_family_list(value) => {
                CssFontFamilyOwnership::SourceOwned
            }
            Some(_) => CssFontFamilyOwnership::Unverified,
        };
        let font_size_px = source.font_size.and_then(static_font_size_px);
        let font_size_ownership = match source.font_size {
            None => CssFontSizeOwnership::Inherited,
            Some(_) if source.font_size_inherits => CssFontSizeOwnership::Inherited,
            Some(_) if font_size_px.is_some() => CssFontSizeOwnership::SourceOwned,
            Some(_) => CssFontSizeOwnership::Unverified,
        };
        Ok(Self {
            source: Some(Arc::new(SourceTypography {
                font_family: source.font_family.map(str::to_owned),
                font_size: source.font_size.map(str::to_owned),
                font_weight: source.font_weight.map(str::to_owned),
                font_style: source.font_style.map(str::to_owned),
                font_faces_verified: source.font_weight.is_none_or(static_font_weight)
                    && source
                        .font_style
                        .is_none_or(is_supported_css_font_style_value),
                font_family_ownership,
                font_size_ownership,
                font_size_px,
                _retained: retained,
            })),
        })
    }

    pub(crate) fn font_family_ownership(&self) -> CssFontFamilyOwnership {
        self.source
            .as_ref()
            .map_or(CssFontFamilyOwnership::Inherited, |source| {
                source.font_family_ownership
            })
    }

    pub(crate) fn font_size_ownership(&self) -> CssFontSizeOwnership {
        self.source
            .as_ref()
            .map_or(CssFontSizeOwnership::Inherited, |source| {
                source.font_size_ownership
            })
    }

    pub(crate) fn has_zero_font_size(&self) -> bool {
        self.source
            .as_ref()
            .is_some_and(|source| source.font_size_px == Some(0.0))
    }

    pub(super) fn apply_to<'a>(
        &self,
        base: &'a RequirementMeasurementStyles,
    ) -> Cow<'a, RequirementMeasurementStyles> {
        if !self.source.as_ref().is_some_and(|source| {
            source.font_family_ownership == CssFontFamilyOwnership::SourceOwned
                || source.font_size_px.is_some()
                || source
                    .font_weight
                    .as_deref()
                    .is_some_and(static_font_weight)
                || source
                    .font_style
                    .as_deref()
                    .is_some_and(is_supported_css_font_style_value)
        }) {
            return Cow::Borrowed(base);
        }
        Cow::Owned(RequirementMeasurementStyles {
            node_wrap_mode: base.node_wrap_mode,
            edge_wrap_mode: base.edge_wrap_mode,
            html_regular: self.resolve_text_style(&base.html_regular),
            html_bold: self.resolve_text_style(&base.html_bold),
            // calculateTextWidth's root/config probe intentionally remains independent.
            calculation: base.calculation.clone(),
        })
    }

    pub(crate) fn resolve_text_style(&self, base: &TextStyle) -> TextStyle {
        let source = self.source.as_ref();
        let font_family = source
            .filter(|source| source.font_family_ownership == CssFontFamilyOwnership::SourceOwned)
            .map_or(&base.font_family, |source| &source.font_family);
        TextStyle {
            font_family: font_family.clone(),
            font_size: source
                .and_then(|source| source.font_size_px)
                .unwrap_or(base.font_size),
            font_weight: source
                .and_then(|source| source.font_weight.as_deref())
                .filter(|value| static_font_weight(value))
                .map(|value| value.to_ascii_lowercase())
                .or_else(|| base.font_weight.clone()),
            font_style: source
                .and_then(|source| source.font_style.as_deref())
                .filter(|value| is_supported_css_font_style_value(value))
                .map(|value| value.to_ascii_lowercase())
                .or_else(|| base.font_style.clone()),
        }
    }

    pub(crate) fn matches_emitted_style(&self, label_styles: &str) -> bool {
        let mut emitted = SourceDeclarations::default();
        visit_parsed_style_declarations(label_styles, |declaration| emitted.observe(declaration));
        let expected = self.source.as_ref();
        // Unmodeled faces affect both family selection and measured font-size geometry.
        expected.is_none_or(|source| source.font_faces_verified)
            && emitted.font_weight == expected.and_then(|source| source.font_weight.as_deref())
            && emitted.font_style == expected.and_then(|source| source.font_style.as_deref())
            && emitted.font_family == expected.and_then(|source| source.font_family.as_deref())
            && emitted.font_size == expected.and_then(|source| source.font_size.as_deref())
    }
}

/// SVG source styles are emitted on each word, above Markdown presentation attributes.
/// HTML children retain their own faces; calculation probes retain their root/config style.
pub(crate) struct RequirementNodeMeasurer<'a> {
    inner: &'a dyn TextMeasurer,
    source: Option<&'a SourceTypography>,
}

impl RequirementNodeMeasurer<'_> {
    pub(crate) fn as_measurer(&self) -> &dyn TextMeasurer {
        if self.source.is_some() {
            self
        } else {
            self.inner
        }
    }

    fn final_style(&self, style: &TextStyle) -> TextStyle {
        let mut style = style.clone();
        if let Some(source) = self.source {
            if let Some(weight) = source
                .font_weight
                .as_deref()
                .filter(|value| static_font_weight(value))
            {
                style.font_weight = Some(weight.to_ascii_lowercase());
            }
            if let Some(face) = source
                .font_style
                .as_deref()
                .filter(|value| is_supported_css_font_style_value(value))
            {
                style.font_style = Some(face.to_ascii_lowercase());
            }
        }
        style
    }
}

impl TextMeasurer for RequirementNodeMeasurer<'_> {
    fn cancellation_requested(&self) -> bool {
        self.inner.cancellation_requested()
    }

    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.inner.measure(text, &self.final_style(style))
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.inner
            .measure_wrapped(text, &self.final_style(style), max_width, wrap_mode)
    }

    fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.inner
            .measure_svg_text_computed_length_px(text, &self.final_style(style))
    }

    fn measure_mermaid_calculate_text_dimensions(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> TextMetrics {
        self.inner
            .measure_mermaid_calculate_text_dimensions(text, style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::RenderResourcePolicy;

    fn resolve(declarations: &[&str]) -> RequirementNodeTypography {
        RequirementNodeTypography::resolve(
            &declarations
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>(),
            &Arc::new(OperationWorkMeter::new(RenderResourcePolicy::default())),
        )
        .unwrap()
    }

    #[test]
    fn requirement_source_typography_matches_writer_last_value_and_quoted_semicolon() {
        let source = resolve(&[
            "font-family:First!important",
            "FONT-FAMILY:'a;b',serif",
            "font-size:24px",
        ]);
        assert_eq!(
            source.font_family_ownership(),
            CssFontFamilyOwnership::SourceOwned
        );
        assert_eq!(
            source.font_size_ownership(),
            CssFontSizeOwnership::SourceOwned
        );
        assert!(
            source.matches_emitted_style(
                "font-family:'a;b',serif!important;font-size:24px!important"
            )
        );
        assert!(
            !source.matches_emitted_style("font-family:First!important;font-size:24px!important")
        );
    }

    #[test]
    fn requirement_source_typography_does_not_certify_relative_or_dynamic_values() {
        for value in [
            "2em",
            "125%",
            "var(--size)",
            "calc(16px + 2px)",
            "larger",
            "18",
            "-12px",
        ] {
            let source = resolve(&[&format!("font-size:{value}")]);
            assert_eq!(
                source.font_size_ownership(),
                CssFontSizeOwnership::Unverified,
                "{value}"
            );
        }
        let source = resolve(&[
            "font-family:serif",
            "font-family:unset",
            "font-size:24px",
            "font-size:inherit",
        ]);
        assert_eq!(
            source.font_family_ownership(),
            CssFontFamilyOwnership::Inherited
        );
        assert_eq!(
            source.font_size_ownership(),
            CssFontSizeOwnership::Inherited
        );
        assert_eq!(
            resolve(&["font:bold 48px serif"]).font_size_ownership(),
            CssFontSizeOwnership::Inherited
        );
    }

    #[test]
    fn requirement_source_typography_retains_source_bytes_once_across_replay() {
        let meter = Arc::new(OperationWorkMeter::new(RenderResourcePolicy::default()));
        let source = RequirementNodeTypography::resolve(
            &[
                "font-family:SourceFont,serif".to_owned(),
                "font-size:24px".to_owned(),
            ],
            &meter,
        )
        .unwrap();
        let retained = meter.prepared_text_retained_bytes();
        assert!(retained >= "SourceFont,serif".len() + "24px".len());
        let replay = source.clone();
        assert_eq!(meter.prepared_text_retained_bytes(), retained);
        drop(source);
        assert_eq!(meter.prepared_text_retained_bytes(), retained);
        drop(replay);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }
}
