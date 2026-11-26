//! Source-owned CSS font decisions.

use super::{GenericFontFamily, fonts::validate_font_stack_families};
use cssparser::Parser;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedCssFontStack {
    families: Vec<String>,
    generic_families: BTreeMap<usize, GenericFontFamily>,
}

impl ParsedCssFontStack {
    fn generic_family(&self, index: usize) -> Option<GenericFontFamily> {
        self.generic_families.get(&index).copied()
    }

    #[cfg(feature = "paint-scene")]
    #[doc(hidden)]
    pub fn resolved_families(
        &self,
    ) -> impl ExactSizeIterator<Item = (&str, Option<GenericFontFamily>)> + '_ {
        self.families
            .iter()
            .enumerate()
            .map(|(index, name)| (name.as_str(), self.generic_family(index)))
    }

    pub fn as_css(&self) -> String {
        let mut out = String::new();
        for (index, family) in self.families.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            if self.generic_family(index).is_some() {
                out.push_str(family);
            } else {
                cssparser::serialize_string(family, &mut out)
                    .expect("serializing a CSS string into String cannot fail");
            }
        }
        out
    }
}

pub fn parse_css_font_stack(value: &str) -> Option<ParsedCssFontStack> {
    let mut parser = Parser::new(value);
    let mut families = Vec::new();
    let mut generic_families = BTreeMap::new();

    while !parser.is_exhausted() {
        let family_index = families.len();
        let family = if let Ok(value) = parser.try_parse(|input| input.expect_string_cloned()) {
            value.to_string()
        } else {
            let first = parser.expect_ident_cloned().ok()?;
            let mut components = vec![first.to_string()];
            while let Ok(component) = parser.try_parse(|input| input.expect_ident_cloned()) {
                components.push(component.to_string());
            }
            if components.iter().any(|component| {
                matches!(
                    component.to_ascii_lowercase().as_str(),
                    "inherit" | "initial" | "revert" | "revert-layer" | "unset"
                )
            }) {
                return None;
            }
            if components.len() == 1
                && let Some(generic) = GenericFontFamily::from_css_keyword(&components[0])
            {
                generic_families.insert(family_index, generic);
            }
            components.join(" ")
        };
        families.push(family);
        if parser.is_exhausted() {
            break;
        }
        parser.expect_comma().ok()?;
        if parser.is_exhausted() {
            return None;
        }
    }

    Some(ParsedCssFontStack {
        families: validate_font_stack_families(families).ok()?,
        generic_families,
    })
}

/// User-agent font defaults for the inline HTML elements modeled by label measurement.
/// Author declarations override these values; relative values use the element's parent.
pub fn xhtml_font_default(element: &str, property: &str) -> Option<&'static str> {
    match (element, property) {
        ("strong" | "b", "font-weight") => Some("bolder"),
        ("cite" | "dfn" | "em" | "i" | "var", "font-style") => Some("italic"),
        ("code" | "kbd" | "samp" | "tt", "font-family") => Some("monospace"),
        ("big", "font-size") => Some("1.2em"),
        ("small", "font-size") => Some("0.8333333333333334em"),
        _ => None,
    }
}
