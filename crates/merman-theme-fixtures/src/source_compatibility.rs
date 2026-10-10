use crate::model::{ReferenceDiagramFamily, ReferenceThemeMechanism};
use cssparser::{
    AtRuleParser, CowRcStr, ParseError, Parser, ParserState, QualifiedRuleParser, StyleSheetParser,
};
use lol_html::Selector;
use merman_core::{style::parse_safe_style_decl, theme_color::ThemeColor};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(crate) struct ThemeCssEvidence {
    pub(crate) has_qualified_rule: bool,
    pub(crate) mechanisms: BTreeSet<ReferenceThemeMechanism>,
    pub(crate) invalid: bool,
}

struct QualifiedThemeCssRule {
    selector: String,
    declarations: String,
}

struct ThemeCssRuleParser {
    invalid: bool,
}

impl<'i> QualifiedRuleParser<'i> for ThemeCssRuleParser {
    type Prelude = String;
    type QualifiedRule = Option<QualifiedThemeCssRule>;
    type Error = ();

    fn parse_prelude(
        &mut self,
        input: &mut Parser<'i>,
    ) -> Result<Self::Prelude, ParseError<Self::Error>> {
        let start = input.position();
        while !input.is_exhausted() {
            input.next_including_whitespace_and_comments()?;
        }
        let selector = input.slice_from(start).trim().to_string();
        if selector.is_empty() {
            Err(ParseError::custom(()))
        } else {
            Ok(selector)
        }
    }

    fn parse_block(
        &mut self,
        selector: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<Self::QualifiedRule, ParseError<Self::Error>> {
        let start = input.position();
        while !input.is_exhausted() {
            input.next_including_whitespace_and_comments()?;
        }
        Ok(Some(QualifiedThemeCssRule {
            selector,
            declarations: input.slice_from(start).trim().to_string(),
        }))
    }
}

impl<'i> AtRuleParser<'i> for ThemeCssRuleParser {
    type Prelude = ();
    type AtRule = Option<QualifiedThemeCssRule>;
    type Error = ();

    fn parse_prelude(
        &mut self,
        _name: CowRcStr<'i>,
        input: &mut Parser<'i>,
    ) -> Result<Self::Prelude, ParseError<Self::Error>> {
        self.invalid = true;
        while !input.is_exhausted() {
            input.next_including_whitespace_and_comments()?;
        }
        Ok(())
    }

    fn rule_without_block(
        &mut self,
        _prelude: Self::Prelude,
        _start: &ParserState,
    ) -> Result<Self::AtRule, Self::Error> {
        self.invalid = true;
        Ok(None)
    }

    fn parse_block(
        &mut self,
        _prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<Self::AtRule, ParseError<Self::Error>> {
        self.invalid = true;
        while !input.is_exhausted() {
            input.next_including_whitespace_and_comments()?;
        }
        Ok(None)
    }
}

pub(crate) fn collect_theme_css_evidence(config: &Value) -> ThemeCssEvidence {
    let Some(theme_css) = config
        .get("themeCSS")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    else {
        return ThemeCssEvidence::default();
    };

    let mut evidence = ThemeCssEvidence::default();
    let mut input = Parser::new(theme_css);
    let mut rule_parser = ThemeCssRuleParser { invalid: false };
    for parsed in StyleSheetParser::new(&mut input, &mut rule_parser) {
        let rule = match parsed {
            Ok(Some(rule)) => rule,
            Ok(None) | Err(_) => {
                evidence.invalid = true;
                continue;
            }
        };
        if !is_valid_theme_selector(&rule.selector) {
            evidence.invalid = true;
            continue;
        }
        let Ok(properties) = collect_valid_declaration_properties(&rule.declarations) else {
            evidence.invalid = true;
            continue;
        };
        let has_paint = properties
            .iter()
            .any(|(property, value)| is_safe_paint_property(property, value));
        if has_paint {
            evidence.has_qualified_rule = true;
        }
        collect_selector_mechanisms(&rule.selector, &mut evidence.mechanisms);
        if properties.contains_key("text-transform") {
            evidence
                .mechanisms
                .insert(ReferenceThemeMechanism::CssTextTransform);
        }
        if properties.contains_key("letter-spacing") {
            evidence
                .mechanisms
                .insert(ReferenceThemeMechanism::CssLetterSpacing);
        }
    }
    evidence.invalid |= rule_parser.invalid;
    evidence
}

pub(crate) fn collect_source_compatibility_mechanisms(
    family: ReferenceDiagramFamily,
    config: &Value,
    effective_config: &Value,
    css: &ThemeCssEvidence,
) -> BTreeSet<ReferenceThemeMechanism> {
    let mut mechanisms = BTreeSet::new();
    if family == ReferenceDiagramFamily::Flowchart
        && has_flowchart_node_theme_variable(config, effective_config)
    {
        mechanisms.insert(ReferenceThemeMechanism::ThemeVariables);
    }
    mechanisms.extend(css.mechanisms.iter().copied());
    mechanisms
}

pub(crate) fn has_flowchart_node_theme_variable(config: &Value, effective_config: &Value) -> bool {
    const FLOWCHART_NODE_VARIABLES: [&str; 8] = [
        "mainBkg",
        "nodeBorder",
        "nodeTextColor",
        "primaryBorderColor",
        "primaryColor",
        "primaryTextColor",
        "strokeWidth",
        "textColor",
    ];
    let Some(source_variables) = config.get("themeVariables").and_then(Value::as_object) else {
        return false;
    };
    let Some(effective_variables) = effective_config
        .get("themeVariables")
        .and_then(Value::as_object)
    else {
        return false;
    };
    FLOWCHART_NODE_VARIABLES.iter().any(|name| {
        let Some(source_value) = source_variables.get(*name) else {
            return false;
        };
        let Some(effective_value) = effective_variables.get(*name) else {
            return false;
        };
        valid_flowchart_theme_variable(*name, source_value)
            && valid_flowchart_theme_variable(*name, effective_value)
            && source_value == effective_value
    })
}

pub(crate) fn is_paint_declaration(style: &str) -> bool {
    merman_core::style::is_safe_paint_declaration(style)
}

pub(crate) fn is_typography_declaration(style: &str) -> bool {
    merman_core::style::is_safe_typography_declaration(style)
}

pub(crate) fn has_state_node_theme_variable(config: &Value, effective_config: &Value) -> bool {
    const STATE_NODE_VARIABLES: [&str; 8] = [
        "labelColor",
        "mainBkg",
        "nodeBorder",
        "primaryBorderColor",
        "primaryColor",
        "primaryTextColor",
        "secondaryColor",
        "tertiaryColor",
    ];
    let Some(source_variables) = config.get("themeVariables").and_then(Value::as_object) else {
        return false;
    };
    let Some(effective_variables) = effective_config
        .get("themeVariables")
        .and_then(Value::as_object)
    else {
        return false;
    };
    STATE_NODE_VARIABLES.iter().any(|name| {
        let Some(source_value) = source_variables.get(*name) else {
            return false;
        };
        let Some(effective_value) = effective_variables.get(*name) else {
            return false;
        };
        source_value == effective_value
            && source_value
                .as_str()
                .is_some_and(|value| ThemeColor::parse(value.trim()).is_ok())
    })
}

fn valid_flowchart_theme_variable(name: &str, value: &Value) -> bool {
    if name == "strokeWidth" {
        return value
            .as_f64()
            .is_some_and(|number| number.is_finite() && number >= 0.0)
            || value.as_str().is_some_and(valid_nonnegative_css_length);
    }
    value.as_str().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "none" | "transparent"
        ) || ThemeColor::parse(value.trim()).is_ok()
    })
}

fn valid_theme_css_declaration(property: &str, value: &str) -> bool {
    let value = value
        .trim()
        .strip_suffix("!important")
        .map_or(value.trim(), str::trim);
    match property {
        "color" | "fill" | "stroke" => valid_paint_color(value),
        "fill-opacity" | "opacity" | "stroke-opacity" => valid_opacity(value),
        "stroke-width" => valid_nonnegative_css_length(value),
        "stroke-dasharray" => valid_dash_array(value),
        "stroke-linecap" => matches!(
            value.to_ascii_lowercase().as_str(),
            "butt" | "round" | "square" | "inherit" | "initial" | "unset"
        ),
        "stroke-linejoin" => matches!(
            value.to_ascii_lowercase().as_str(),
            "arcs" | "bevel" | "miter" | "miter-clip" | "round" | "inherit" | "initial" | "unset"
        ),
        "text-transform" => matches!(
            value.to_ascii_lowercase().as_str(),
            "none"
                | "capitalize"
                | "uppercase"
                | "lowercase"
                | "full-width"
                | "full-size-kana"
                | "math-auto"
        ),
        "letter-spacing" => value.eq_ignore_ascii_case("normal") || valid_css_length(value),
        _ => false,
    }
}

fn is_safe_paint_property(property: &str, value: &str) -> bool {
    matches!(
        property,
        "color"
            | "fill"
            | "fill-opacity"
            | "opacity"
            | "stroke"
            | "stroke-dasharray"
            | "stroke-linecap"
            | "stroke-linejoin"
            | "stroke-opacity"
            | "stroke-width"
    ) && valid_theme_css_declaration(property, value)
}

fn valid_paint_color(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "none" | "transparent" | "currentcolor" | "inherit" | "initial" | "unset"
    ) || ThemeColor::parse(value).is_ok()
}

fn valid_opacity(value: &str) -> bool {
    let value = value.trim();
    if let Some(number) = value.strip_suffix('%') {
        return number
            .trim()
            .parse::<f64>()
            .ok()
            .is_some_and(|number| number.is_finite() && (0.0..=100.0).contains(&number));
    }
    value
        .parse::<f64>()
        .ok()
        .is_some_and(|number| number.is_finite() && (0.0..=1.0).contains(&number))
}

fn valid_dash_array(value: &str) -> bool {
    let value = value.trim();
    if matches!(
        value.to_ascii_lowercase().as_str(),
        "none" | "inherit" | "initial" | "unset"
    ) {
        return true;
    }
    !value.is_empty()
        && value.split_ascii_whitespace().all(|component| {
            valid_nonnegative_css_length(component)
                || component
                    .parse::<f64>()
                    .ok()
                    .is_some_and(|number| number.is_finite() && number >= 0.0)
        })
}

fn valid_nonnegative_css_length(value: &str) -> bool {
    let value = value.trim();
    if let Ok(number) = value.parse::<f64>() {
        return number.is_finite() && number >= 0.0;
    }
    ["px", "em", "rem", "pt", "%"].iter().any(|unit| {
        value
            .strip_suffix(unit)
            .and_then(|number| number.trim().parse::<f64>().ok())
            .is_some_and(|number| number.is_finite() && number >= 0.0)
    })
}

fn valid_css_length(value: &str) -> bool {
    valid_nonnegative_css_length(value)
        || value
            .strip_prefix('-')
            .is_some_and(valid_nonnegative_css_length)
}

fn collect_valid_declaration_properties(
    declarations: &str,
) -> Result<BTreeMap<String, String>, ()> {
    let mut properties = BTreeMap::new();
    let mut input = Parser::new(declarations);
    while !input.is_exhausted() {
        let property = input.parse_until_after(cssparser::Delimiter::Semicolon, |declaration| {
            let property = declaration.expect_ident_cloned()?;
            declaration.expect_colon()?;
            let value_start = declaration.position();
            while !declaration.is_exhausted() {
                declaration.next_including_whitespace_and_comments()?;
            }
            let value = declaration.slice_from(value_start).trim();
            let property = property.as_ref().to_ascii_lowercase();
            if !is_modeled_theme_css_property(&property)
                || parse_safe_style_decl(&format!("{property}:{value}"))
                    .is_none_or(|(_, value)| !valid_theme_css_declaration(&property, value))
            {
                return Err(ParseError::custom(()));
            }
            Ok::<_, ParseError<()>>((property, value.to_string()))
        });
        let Ok((property, value)) = property else {
            return Err(());
        };
        properties.insert(property, value);
    }
    if properties.is_empty() {
        Err(())
    } else {
        Ok(properties)
    }
}

fn is_modeled_theme_css_property(property: &str) -> bool {
    matches!(
        property,
        "color"
            | "fill"
            | "fill-opacity"
            | "opacity"
            | "stroke"
            | "stroke-dasharray"
            | "stroke-linecap"
            | "stroke-linejoin"
            | "stroke-opacity"
            | "stroke-width"
            | "text-transform"
            | "letter-spacing"
    )
}

fn collect_selector_mechanisms(selector: &str, mechanisms: &mut BTreeSet<ReferenceThemeMechanism>) {
    let selector = css_code_bytes(selector);
    for (needle, mechanism) in [
        (
            b":nth-child(".as_slice(),
            ReferenceThemeMechanism::NthChildSelector,
        ),
        (b":has(".as_slice(), ReferenceThemeMechanism::HasSelector),
        (b":not(".as_slice(), ReferenceThemeMechanism::NotSelector),
    ] {
        if selector
            .windows(needle.len())
            .any(|window| window == needle)
        {
            mechanisms.insert(mechanism);
        }
    }
}

fn is_valid_theme_selector(selector: &str) -> bool {
    if !selector_delimiters_are_balanced(selector) {
        return false;
    }
    let Some(rewritten) = rewrite_has_for_selector_validation(selector) else {
        return false;
    };
    rewritten.parse::<Selector>().is_ok()
}

fn selector_delimiters_are_balanced(selector: &str) -> bool {
    let bytes = selector.as_bytes();
    let mut parentheses = 0_usize;
    let mut brackets = 0_usize;
    let mut quote = None;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active_quote) = quote {
            if byte == b'\\' {
                index = match index.checked_add(2) {
                    Some(index) => index,
                    None => return false,
                };
                continue;
            }
            if byte == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        match byte {
            b'\'' | b'"' => quote = Some(byte),
            b'(' => {
                parentheses = match parentheses.checked_add(1) {
                    Some(value) => value,
                    None => return false,
                }
            }
            b')' => {
                parentheses = match parentheses.checked_sub(1) {
                    Some(value) => value,
                    None => return false,
                }
            }
            b'[' => {
                brackets = match brackets.checked_add(1) {
                    Some(value) => value,
                    None => return false,
                }
            }
            b']' => {
                brackets = match brackets.checked_sub(1) {
                    Some(value) => value,
                    None => return false,
                }
            }
            _ => {}
        }
        index += 1;
    }
    quote.is_none() && parentheses == 0 && brackets == 0
}

fn rewrite_has_for_selector_validation(selector: &str) -> Option<String> {
    let lower = selector.to_ascii_lowercase();
    let mut output = String::with_capacity(selector.len());
    let mut cursor = 0;
    while let Some(relative) = lower[cursor..].find(":has(") {
        let start = cursor + relative;
        output.push_str(&selector[cursor..start]);
        let open = start.checked_add(5)?;
        let close = matching_parenthesis(selector, open.checked_sub(1)?)?;
        let argument = selector.get(open..close)?.trim();
        if argument.is_empty() || argument.to_ascii_lowercase().contains(":has(") {
            return None;
        }
        output.push_str(":not(");
        output.push_str(argument);
        output.push(')');
        cursor = close.checked_add(1)?;
    }
    output.push_str(&selector[cursor..]);
    Some(output)
}

fn matching_parenthesis(value: &str, open: usize) -> Option<usize> {
    let bytes = value.as_bytes();
    if bytes.get(open) != Some(&b'(') {
        return None;
    }
    let mut depth = 0_usize;
    let mut quote = None;
    let mut index = open;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active_quote) = quote {
            if byte == b'\\' {
                index = index.checked_add(2)?;
                continue;
            }
            if byte == active_quote {
                quote = None;
            }
        } else {
            match byte {
                b'\'' | b'"' => quote = Some(byte),
                b'(' => depth = depth.checked_add(1)?,
                b')' => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    None
}

fn css_code_bytes(css: &str) -> Vec<u8> {
    let source = css.as_bytes();
    let mut code = css.to_ascii_lowercase().into_bytes();
    let mut index = 0;
    while index < source.len() {
        if source[index..].starts_with(b"/*") {
            code[index] = b' ';
            code[index + 1] = b' ';
            index += 2;
            while index < source.len() && !source[index..].starts_with(b"*/") {
                code[index] = b' ';
                index += 1;
            }
            if index < source.len() {
                code[index] = b' ';
                code[index + 1] = b' ';
                index += 2;
            }
            continue;
        }
        if matches!(source[index], b'\'' | b'"') {
            let quote = source[index];
            code[index] = b' ';
            index += 1;
            while index < source.len() {
                code[index] = b' ';
                if source[index] == b'\\' {
                    index += 1;
                    if index < source.len() {
                        code[index] = b' ';
                        index += 1;
                    }
                    continue;
                }
                let done = source[index] == quote;
                index += 1;
                if done {
                    break;
                }
            }
            continue;
        }
        index += 1;
    }
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modeled_theme_css_declarations_are_retained() {
        let properties = collect_valid_declaration_properties(
            "stroke: #7c3aed; text-transform: uppercase; letter-spacing: 0.04em;",
        )
        .expect("valid declarations");
        assert_eq!(
            properties.get("stroke").map(String::as_str),
            Some("#7c3aed")
        );
        assert_eq!(
            properties.get("text-transform").map(String::as_str),
            Some("uppercase")
        );
        assert_eq!(
            properties.get("letter-spacing").map(String::as_str),
            Some("0.04em")
        );
    }

    #[test]
    fn unmodeled_or_invalid_theme_css_declarations_are_rejected() {
        assert!(collect_valid_declaration_properties("unknown-prop: value;").is_err());
        assert!(collect_valid_declaration_properties("fill: not-a-color;").is_err());
        assert!(collect_valid_declaration_properties("letter-spacing: bogus;").is_err());
    }

    #[test]
    fn selector_parser_accepts_the_admitted_selector_forms() {
        for selector in [
            ".node:nth-child(2):not(.disabled) rect",
            "svg:has(.node) .label",
        ] {
            assert!(is_valid_theme_selector(selector), "{selector}");
        }
    }

    #[test]
    fn selector_validation_rejects_dynamic_or_malformed_forms() {
        for selector in [
            "svg:has() .label",
            "svg:has(.node:has(.nested)) .label",
            ".node:nth-child(bogus) rect",
            ".node:not( rect",
        ] {
            assert!(!is_valid_theme_selector(selector), "{selector}");
        }
    }
}
