//! Source-owned CSS font decisions.

use crate::diagram_theme::{FontStack, GenericFontFamily};
use cssparser::Parser;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedCssFontStack {
    font_stack: FontStack,
    generic_families: BTreeMap<usize, GenericFontFamily>,
}

impl ParsedCssFontStack {
    fn generic_family(&self, index: usize) -> Option<GenericFontFamily> {
        self.generic_families.get(&index).copied()
    }

    pub(crate) fn as_css(&self) -> String {
        let mut out = String::new();
        for (index, family) in self.font_stack.families().iter().enumerate() {
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

pub(crate) fn parse_css_font_stack(value: &str) -> Option<ParsedCssFontStack> {
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
        font_stack: FontStack::new(families).ok()?,
        generic_families,
    })
}

pub(crate) fn resolve_css_font_weight(value: &str, inherited: u16) -> Option<u16> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(400),
        "bold" => Some(700),
        "bolder" => Some(relative_font_weight(inherited, true)),
        "lighter" => Some(relative_font_weight(inherited, false)),
        value => value
            .parse::<u16>()
            .ok()
            .filter(|weight| (1..=1000).contains(weight)),
    }
}

fn relative_font_weight(inherited: u16, bolder: bool) -> u16 {
    match (inherited, bolder) {
        (1..=99, true) => 400,
        (1..=99, false) => inherited,
        (100..=349, true) => 400,
        (100..=349, false) => 100,
        (350..=549, true) => 700,
        (350..=549, false) => 100,
        (550..=749, true) => 900,
        (550..=749, false) => 400,
        (750..=899, true) => 900,
        (750..=899, false) => 700,
        (900..=1000, true) => inherited,
        (900..=1000, false) => 700,
        _ => inherited,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_legacy_font_weights_resolve_against_the_theme_weight() {
        for (inherited, bolder, lighter) in [
            (1, 400, 1),
            (99, 400, 99),
            (100, 400, 100),
            (349, 400, 100),
            (350, 700, 100),
            (549, 700, 100),
            (550, 900, 400),
            (749, 900, 400),
            (750, 900, 700),
            (899, 900, 700),
            (900, 900, 700),
            (1000, 1000, 700),
        ] {
            assert_eq!(resolve_css_font_weight("bolder", inherited), Some(bolder));
            assert_eq!(resolve_css_font_weight("lighter", inherited), Some(lighter));
        }
        assert_eq!(resolve_css_font_weight("calc(400)", 400), None);
    }
}
