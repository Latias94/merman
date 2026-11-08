use std::borrow::Cow;

use merman_model::{OperationControl, OperationControlResult};

use super::{
    CssFontSizeContext, CssValueAnalysis, CssValuePolicy, ParsedStyleDeclaration,
    is_static_css_font_family_list, is_supported_css_font_style_value,
    is_supported_css_font_weight_value, parse_css_value,
};
use crate::text::TextStyle;

/// Finds the winning root-dependent size within one element's declaration lists.
///
/// Compatibility measurement still approximates rem using the inherited diagram size. This
/// detector supplies admission evidence only; it never changes measurement or emitted CSS.
#[cfg_attr(not(feature = "all-diagrams"), allow(dead_code))]
pub(crate) fn root_dependent_font_size<'a>(
    styles: impl IntoIterator<Item = &'a str>,
    control: &OperationControl,
) -> OperationControlResult<Option<(usize, &'a str)>> {
    let mut winner: Option<(bool, Option<(usize, &'a str)>)> = None;
    let mut ordinal = 0usize;
    for raw in styles {
        super::visit_style_declaration_boundaries_with_checkpoints(
            raw,
            &mut || control.checkpoint(),
            |boundary| {
                let declaration_ordinal = ordinal;
                ordinal = ordinal.saturating_add(1);
                if let Some(size) = super::css_font_size_override(boundary.raw())
                    && winner
                        .as_ref()
                        .is_none_or(|(important, _)| size.important() || !*important)
                {
                    let mut parser = cssparser::Parser::new(boundary.raw());
                    let root_dependent = parser
                        .expect_ident_cloned()
                        .is_ok_and(|property| property.eq_ignore_ascii_case("font-size"))
                        && parser.expect_colon().is_ok()
                        && super::CssValueAnalysis::font_size_depends_on_root(
                            &boundary.raw()[parser.position().byte_index()..],
                        );
                    let clears_root_dependency = super::parse_style_declaration(boundary.raw())
                        .is_some_and(|declaration| {
                            declaration.inherits_property_value()
                                || (size.ownership() == super::CssFontSizeOwnership::SourceOwned
                                    && (declaration
                                        .resolve_font_size_px(super::CssFontSizeContext::uniform(
                                            16.0,
                                        ))
                                        .is_some()
                                        || declaration.svg_number_or_px() == Some(0.0)))
                        });
                    if !root_dependent && !clears_root_dependency {
                        return Ok(true);
                    }
                    winner = Some((
                        size.important(),
                        root_dependent.then_some((declaration_ordinal, boundary.raw())),
                    ));
                }
                Ok(true)
            },
        )?;
    }
    Ok(winner.and_then(|(_, declaration)| declaration))
}

/// Resolves supported font longhands against one immutable inherited style.
pub(crate) struct TextStyleCascade<'a> {
    base: &'a TextStyle,
    style: Cow<'a, TextStyle>,
    context: CssFontSizeContext,
    importance: [Option<bool>; 4],
}

impl<'a> TextStyleCascade<'a> {
    pub(crate) fn new(base: &'a TextStyle, context: CssFontSizeContext) -> Self {
        Self {
            base,
            style: Cow::Borrowed(base),
            context,
            importance: [None; 4],
        }
    }

    pub(crate) fn observe_declaration(&mut self, declaration: &ParsedStyleDeclaration<'_>) {
        self.observe_analyzed(
            declaration.property(),
            declaration.value(),
            declaration.important(),
            declaration.analysis(),
        );
    }

    pub(crate) fn observe(&mut self, property: &str, value: &str, important: bool) {
        if !matches!(
            property,
            "font-family" | "font-size" | "font-weight" | "font-style"
        ) {
            return;
        }
        if let Some(parsed) = parse_css_value(value, CssValuePolicy::MermaidSourceStyle) {
            self.observe_analyzed(property, parsed.value, important, &parsed.analysis);
        }
    }

    fn observe_analyzed(
        &mut self,
        property: &str,
        value: &str,
        important: bool,
        analysis: &CssValueAnalysis,
    ) {
        let index = match property {
            "font-family" => 0,
            "font-size" => 1,
            "font-weight" => 2,
            "font-style" => 3,
            _ => return,
        };
        if self.importance[index].is_some_and(|previous| previous && !important) {
            return;
        }
        let inherits = analysis.inherits_property_value();
        let initial = matches!(property, "font-weight" | "font-style")
            && analysis.single_ident() == Some("initial");
        let valid = inherits
            || initial
            || match property {
                "font-family" => is_static_css_font_family_list(value),
                "font-size" => {
                    !matches!(analysis.scalar.as_ref(), Some(super::CssScalar::Number(value)) if *value != 0.0)
                        && analysis.resolve_font_size_px(self.context).is_some()
                }
                "font-weight" => is_supported_css_font_weight_value(value),
                "font-style" => is_supported_css_font_style_value(value),
                _ => unreachable!(),
            };
        if !valid {
            return;
        }
        self.importance[index] = Some(important);
        let style = self.style.to_mut();
        match property {
            "font-family" => {
                style.font_family = if inherits {
                    self.base.font_family.clone()
                } else {
                    Some(value.to_owned())
                };
            }
            "font-size" => {
                style.font_size = if inherits {
                    self.base.font_size
                } else {
                    analysis
                        .resolve_font_size_px(self.context)
                        .expect("validated font size")
                };
            }
            "font-weight" => {
                style.font_weight = if initial {
                    Some("400".to_owned())
                } else if inherits {
                    self.base.font_weight.clone()
                } else {
                    let inherited = self
                        .base
                        .font_weight
                        .as_deref()
                        .and_then(|value| crate::text::resolve_css_font_weight(value, 400))
                        .unwrap_or(400);
                    Some(
                        crate::text::resolve_css_font_weight(value, inherited)
                            .map_or_else(|| value.to_owned(), |weight| weight.to_string()),
                    )
                };
            }
            "font-style" => {
                style.font_style = if initial {
                    Some("normal".to_owned())
                } else if inherits {
                    self.base.font_style.clone()
                } else {
                    Some(value.to_owned())
                };
            }
            _ => unreachable!(),
        }
    }

    pub(crate) fn finish(self) -> Cow<'a, TextStyle> {
        if self.style.font_family == self.base.font_family
            && self.style.font_size == self.base.font_size
            && self.style.font_weight == self.base.font_weight
            && self.style.font_style == self.base.font_style
        {
            Cow::Borrowed(self.base)
        } else {
            self.style
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid_style::visit_parsed_style_declarations;

    fn parent_style() -> TextStyle {
        TextStyle {
            font_family: Some("Inter, sans-serif".to_owned()),
            font_size: 24.0,
            font_weight: Some("600".to_owned()),
            font_style: Some("oblique".to_owned()),
        }
    }

    #[test]
    fn font_initial_resets_override_primitive_defaults() {
        let base = parent_style();
        for value in ["initial", "INITIAL", r"\69nitial"] {
            let mut cascade = TextStyleCascade::new(&base, CssFontSizeContext::new(24.0, 16.0));
            cascade.observe("font-weight", "bolder", false);
            cascade.observe("font-style", "italic", false);
            visit_parsed_style_declarations(
                &format!("font-weight:{value};font-style:{value}"),
                |declaration| cascade.observe_declaration(&declaration),
            );
            let resolved = cascade.finish();
            assert_eq!(resolved.font_weight.as_deref(), Some("400"), "{value}");
            assert_eq!(resolved.font_style.as_deref(), Some("normal"), "{value}");
            assert_eq!(resolved.font_family, base.font_family);
            assert_eq!(resolved.font_size, base.font_size);
        }
    }

    #[test]
    fn font_initial_respects_importance_and_immutable_parent_resolution() {
        let base = parent_style();
        for (authored, weight, font_style) in [
            (
                "font-weight:initial!important;font-weight:900;\
                 font-style:initial!important;font-style:italic",
                "400",
                "normal",
            ),
            (
                "font-weight:900!important;font-weight:initial;\
                 font-style:italic!important;font-style:initial",
                "900",
                "italic",
            ),
            (
                "font-weight:900!important;font-weight:initial!important;\
                 font-style:italic!important;font-style:initial!important",
                "400",
                "normal",
            ),
            (
                "font-weight:initial;font-weight:bolder;\
                 font-style:initial;font-style:inherit",
                "900",
                "oblique",
            ),
            (
                "font-weight:initial;font-weight:inherit;\
                 font-style:initial;font-style:unset",
                "600",
                "oblique",
            ),
            (
                "font-weight:initial;font-weight:initial 900;\
                 font-style:initial;font-style:'initial'",
                "400",
                "normal",
            ),
        ] {
            let mut cascade = TextStyleCascade::new(&base, CssFontSizeContext::new(24.0, 16.0));
            cascade.observe("font-weight", "bolder", false);
            cascade.observe("font-style", "italic", false);
            visit_parsed_style_declarations(authored, |declaration| {
                cascade.observe_declaration(&declaration);
            });
            let resolved = cascade.finish();
            assert_eq!(resolved.font_weight.as_deref(), Some(weight), "{authored}");
            assert_eq!(
                resolved.font_style.as_deref(),
                Some(font_style),
                "{authored}"
            );
        }
    }

    #[test]
    fn already_initial_fonts_retain_the_borrowed_parent() {
        let base = TextStyle {
            font_size: 16.0,
            font_weight: Some("400".to_owned()),
            font_style: Some("normal".to_owned()),
            ..TextStyle::default()
        };
        let mut cascade = TextStyleCascade::new(&base, CssFontSizeContext::uniform(16.0));
        cascade.observe("font-weight", "bolder", false);
        cascade.observe("font-style", "italic", false);
        cascade.observe("font-weight", "initial", false);
        cascade.observe("font-style", "initial", false);
        assert!(matches!(cascade.finish(), Cow::Borrowed(style) if std::ptr::eq(style, &base)));
    }

    #[test]
    fn zero_sizes_keep_cascade_priority_and_relative_descendants() {
        let base = parent_style();
        for declarations in [
            "font-size:0px",
            "font-size:0pt!important;font-size:32px",
            "font-size:32px;font-size:0px!important",
            "font-size:0px;font-size:-1px!important",
            "font-size:0px;font-size:-1e-1000pt!important",
        ] {
            let mut cascade = TextStyleCascade::new(&base, CssFontSizeContext::new(24.0, 18.0));
            visit_parsed_style_declarations(declarations, |declaration| {
                cascade.observe_declaration(&declaration);
            });
            let zero = cascade.finish();
            assert_eq!(zero.font_size, 0.0, "{declarations}");
            for relative in ["150%", "2em", "smaller", "larger", "inherit", "unset"] {
                let mut child = TextStyleCascade::new(&zero, CssFontSizeContext::new(0.0, 18.0));
                child.observe("font-size", relative, false);
                assert_eq!(child.finish().font_size, 0.0, "{declarations}/{relative}");
            }
            let mut absolute = TextStyleCascade::new(&zero, CssFontSizeContext::new(0.0, 18.0));
            absolute.observe("font-size", "24pt", false);
            assert_eq!(absolute.finish().font_size, 32.0);
        }
        assert_eq!(base.font_size, 24.0);
        let mut invalid_numeric = TextStyleCascade::new(&base, CssFontSizeContext::uniform(24.0));
        invalid_numeric.observe("font-size", "32", false);
        assert_eq!(invalid_numeric.finish().font_size, 24.0);
        let zero = TextStyle {
            font_size: 0.0,
            ..base
        };
        for invalid in ["-1em", "-100%", "-1rem"] {
            let mut cascade = TextStyleCascade::new(&zero, CssFontSizeContext::uniform(0.0));
            cascade.observe("font-size", invalid, true);
            cascade.observe("font-size", "24px", false);
            assert_eq!(cascade.finish().font_size, 24.0, "{invalid}");
        }
    }
}
