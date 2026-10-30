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
        let valid = inherits
            || match property {
                "font-family" => is_static_css_font_family_list(value),
                "font-size" => analysis.resolve_font_size_px(self.context).is_some(),
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
                style.font_weight = if inherits {
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
                style.font_style = if inherits {
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
