use crate::svg::pipeline::{
    SvgTagScanner, checkpoint_loop, end_tag_name,
    extract_exact_double_quoted_attr_with_checkpoints, find_with_checkpoints, start_tag_name,
    trim_with_checkpoints,
};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default)]
struct StyleDeclarations<'a> {
    fill: Option<&'a str>,
    color: Option<&'a str>,
    font_size: Option<&'a str>,
    font_family: Option<&'a str>,
    font_weight: Option<&'a str>,
    font_style: Option<&'a str>,
}

impl<'a> StyleDeclarations<'a> {
    fn property(self, property: &str) -> Option<&'a str> {
        match property {
            "fill" => self.fill,
            "color" => self.color,
            "font-size" => self.font_size,
            "font-family" => self.font_family,
            "font-weight" => self.font_weight,
            "font-style" => self.font_style,
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ClassStyle<'a> {
    background_color: Option<&'a str>,
    text_fill: Option<&'a str>,
    font_size: Option<&'a str>,
    font_family: Option<&'a str>,
    font_weight: Option<&'a str>,
    font_style: Option<&'a str>,
}

impl<'a> ClassStyle<'a> {
    fn property(self, property: &str) -> Option<&'a str> {
        match property {
            "font-size" => self.font_size,
            "font-family" => self.font_family,
            "font-weight" => self.font_weight,
            "font-style" => self.font_style,
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct SelectorTargets {
    text_like: bool,
    inherited: bool,
}

/// Borrowed, operation-controlled CSS facts used by foreign-object fallback text.
///
/// The SVG stylesheet is scanned once. Class and root lookups performed for every fallback label
/// are then constant-time and retain borrowed values from the admitted SVG source.
#[derive(Debug)]
pub(super) struct FallbackStyleIndex<'a> {
    root_declarations: Option<StyleDeclarations<'a>>,
    class_styles: HashMap<&'a str, ClassStyle<'a>>,
}

impl<'a> FallbackStyleIndex<'a> {
    pub(super) fn new<E>(
        svg: &'a str,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Self, E> {
        checkpoint()?;
        let root_id = root_id(svg, checkpoint)?;
        let mut root_declarations = None;
        let mut class_styles = HashMap::new();
        let mut scanner = SvgTagScanner::new(svg);
        let mut style_iteration = 0usize;
        while let Some(tag) = scanner.next_with_checkpoints(checkpoint)? {
            checkpoint_loop(style_iteration, checkpoint)?;
            style_iteration = style_iteration.saturating_add(1);
            if start_tag_name(tag.raw()) != Some("style") || tag.is_self_closing() {
                continue;
            }

            let content_start = scanner.cursor();
            let mut close_tag = None;
            let mut content_iteration = 0usize;
            while let Some(candidate) = scanner.next_with_checkpoints(checkpoint)? {
                checkpoint_loop(content_iteration, checkpoint)?;
                content_iteration = content_iteration.saturating_add(1);
                if end_tag_name(candidate.raw()) == Some("style") {
                    close_tag = Some(candidate);
                    break;
                }
            }
            let Some(close_tag) = close_tag else {
                break;
            };
            index_style_rules(
                &svg[content_start..close_tag.start()],
                root_id,
                &mut root_declarations,
                &mut class_styles,
                checkpoint,
            )?;
        }

        checkpoint()?;
        Ok(Self {
            root_declarations,
            class_styles,
        })
    }

    pub(super) fn background_color_for_class(&self, class_name: &str) -> Option<&'a str> {
        self.class_styles
            .get(class_name)
            .and_then(|style| style.background_color)
    }

    pub(super) fn text_fill_for_class(&self, class_name: &str) -> Option<&'a str> {
        self.class_styles
            .get(class_name)
            .and_then(|style| style.text_fill)
    }

    pub(super) fn style_property_for_class(
        &self,
        class_name: &str,
        property: &str,
    ) -> Option<&'a str> {
        self.class_styles
            .get(class_name)
            .and_then(|style| style.property(property))
    }

    pub(super) fn root_text_fill(&self) -> Option<&'a str> {
        self.root_declarations
            .and_then(|declarations| declarations.fill.or(declarations.color))
    }

    pub(super) fn root_style_property(&self, property: &str) -> Option<&'a str> {
        self.root_declarations
            .and_then(|declarations| declarations.property(property))
    }
}

fn root_id<'a, E>(
    svg: &'a str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<&'a str>, E> {
    let mut scanner = SvgTagScanner::new(svg);
    while let Some(tag) = scanner.next_with_checkpoints(checkpoint)? {
        let Some(name) = start_tag_name(tag.raw()) else {
            continue;
        };
        if name != "svg" {
            return Ok(None);
        }
        let id = extract_exact_double_quoted_attr_with_checkpoints(tag.raw(), "id", checkpoint)?;
        checkpoint()?;
        return Ok(id);
    }
    Ok(None)
}

fn index_style_rules<'a, E>(
    css: &'a str,
    root_id: Option<&str>,
    root_declarations: &mut Option<StyleDeclarations<'a>>,
    class_styles: &mut HashMap<&'a str, ClassStyle<'a>>,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), E> {
    let mut search = 0usize;
    let mut rule_iteration = 0usize;
    while let Some(open_relative) = find_with_checkpoints(&css[search..], "{", checkpoint)? {
        checkpoint_loop(rule_iteration, checkpoint)?;
        rule_iteration = rule_iteration.saturating_add(1);
        let open = search + open_relative;
        let Some(close_relative) = find_with_checkpoints(&css[open + 1..], "}", checkpoint)? else {
            break;
        };
        let close = open + 1 + close_relative;
        let raw_selector = &css[search..open];
        let raw_declarations = &css[open + 1..close];
        let declarations = parse_style_declarations_with_checkpoints(raw_declarations, checkpoint)?;

        if root_declarations.is_none()
            && let Some(root_id) = root_id
            && selector_ends_with_root_id(raw_selector, root_id, checkpoint)?
        {
            *root_declarations = Some(declarations);
        }

        let selector_list = trim_with_checkpoints(raw_selector, checkpoint)?;
        for (class_index, (class_name, target)) in
            index_rule_class_targets(selector_list, checkpoint)?
                .into_iter()
                .enumerate()
        {
            checkpoint_loop(class_index, checkpoint)?;
            let style = class_styles.entry(class_name).or_default();
            if style.text_fill.is_none() {
                style.text_fill = if target.text_like {
                    declarations.fill.or(declarations.color)
                } else if target.inherited {
                    declarations.color
                } else {
                    None
                };
            }
            if target.inherited {
                style.font_size = style.font_size.or(declarations.font_size);
                style.font_family = style.font_family.or(declarations.font_family);
                style.font_weight = style.font_weight.or(declarations.font_weight);
                style.font_style = style.font_style.or(declarations.font_style);
            }
        }

        if let Some(class_name) = trailing_exact_class_with_checkpoints(raw_selector, checkpoint)? {
            let style = class_styles.entry(class_name).or_default();
            if style.background_color.is_none() {
                style.background_color =
                    extract_legacy_background_color_with_checkpoints(raw_declarations, checkpoint)?;
            }
        }

        if let Some(class_name) =
            trailing_compact_text_class_with_checkpoints(raw_selector, checkpoint)?
        {
            let style = class_styles.entry(class_name).or_default();
            if style.text_fill.is_none()
                && let Some(after_fill) = raw_declarations.strip_prefix("fill:")
            {
                let end =
                    find_with_checkpoints(after_fill, ";", checkpoint)?.unwrap_or(after_fill.len());
                let value = trim_with_checkpoints(&after_fill[..end], checkpoint)?;
                if !value.is_empty() {
                    style.text_fill = Some(value);
                }
            }
        }

        search = close + 1;
    }
    checkpoint()?;
    Ok(())
}

fn selector_ends_with_root_id<E>(
    selector: &str,
    root_id: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<bool, E> {
    let Some(required_len) = root_id.len().checked_add(1) else {
        checkpoint()?;
        return Ok(false);
    };
    let Some(hash_index) = selector.len().checked_sub(required_len) else {
        checkpoint()?;
        return Ok(false);
    };
    if selector.as_bytes().get(hash_index) != Some(&b'#') {
        checkpoint()?;
        return Ok(false);
    }
    for (iteration, (actual, expected)) in selector.as_bytes()[hash_index + 1..]
        .iter()
        .zip(root_id.as_bytes())
        .enumerate()
    {
        checkpoint_loop(iteration, checkpoint)?;
        if actual != expected {
            return Ok(false);
        }
    }
    checkpoint()?;
    Ok(true)
}

fn index_rule_class_targets<'a, E>(
    selector_list: &'a str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<HashMap<&'a str, SelectorTargets>, E> {
    let mut targets = HashMap::new();
    let mut search = 0usize;
    let mut selector_index = 0usize;
    loop {
        checkpoint_loop(selector_index, checkpoint)?;
        selector_index = selector_index.saturating_add(1);
        let comma = find_with_checkpoints(&selector_list[search..], ",", checkpoint)?;
        let end = comma.map_or(selector_list.len(), |relative| search + relative);
        let selector = trim_with_checkpoints(&selector_list[search..end], checkpoint)?;
        if selector.is_empty() {
            if comma.is_none() {
                break;
            }
            search = end + 1;
            continue;
        }
        let target = SelectorTargets {
            text_like: selector_targets_any_element_with_checkpoints(
                selector,
                &["text", "tspan", "span", "p"],
                checkpoint,
            )?,
            inherited: !selector_targets_any_element_with_checkpoints(
                selector,
                &["rect", "circle", "ellipse", "polygon", "path", "line"],
                checkpoint,
            )?,
        };
        for (class_index, class_name) in selector_class_tokens(selector, checkpoint)?
            .into_iter()
            .enumerate()
        {
            checkpoint_loop(class_index, checkpoint)?;
            let indexed = targets
                .entry(class_name)
                .or_insert(SelectorTargets::default());
            indexed.text_like |= target.text_like;
            indexed.inherited |= target.inherited;
        }
        if comma.is_none() {
            break;
        }
        search = end + 1;
    }
    checkpoint()?;
    Ok(targets)
}

fn selector_class_tokens<'a, E>(
    selector: &'a str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<&'a str>, E> {
    let mut classes = Vec::new();
    let mut characters = selector.char_indices().peekable();
    let mut iteration = 0usize;
    while let Some((_, character)) = characters.next() {
        checkpoint_loop(iteration, checkpoint)?;
        iteration = iteration.saturating_add(1);
        if character != '.' {
            continue;
        }

        let Some(&(start, first)) = characters.peek() else {
            break;
        };
        if !is_css_class_name_char(first) {
            continue;
        }
        let mut end = start + first.len_utf8();
        characters.next();
        while let Some(&(index, next)) = characters.peek() {
            checkpoint_loop(iteration, checkpoint)?;
            iteration = iteration.saturating_add(1);
            if !is_css_class_name_char(next) {
                break;
            }
            end = index + next.len_utf8();
            characters.next();
        }
        classes.push(&selector[start..end]);
    }
    checkpoint()?;
    Ok(classes)
}

fn selector_targets_any_element_with_checkpoints<E>(
    selector: &str,
    elements: &[&str],
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<bool, E> {
    for (index, element) in elements.iter().enumerate() {
        checkpoint_loop(index, checkpoint)?;
        if selector_targets_element_with_checkpoints(selector, element, checkpoint)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn selector_targets_element_with_checkpoints<E>(
    selector: &str,
    element: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<bool, E> {
    let selector_bytes = selector.as_bytes();
    let element_bytes = element.as_bytes();
    let Some(last_start) = selector_bytes.len().checked_sub(element_bytes.len()) else {
        checkpoint()?;
        return Ok(false);
    };

    for start in 0..=last_start {
        checkpoint_loop(start, checkpoint)?;
        let end = start + element_bytes.len();
        if !selector.is_char_boundary(start)
            || !selector.is_char_boundary(end)
            || !selector_bytes[start..end].eq_ignore_ascii_case(element_bytes)
        {
            continue;
        }

        let before = selector[..start].chars().next_back();
        let after = selector[end..].chars().next();
        let before_ok = before.is_none_or(|ch| !is_css_class_name_char(ch));
        let after_ok = after.is_none_or(|ch| !is_css_class_name_char(ch));
        if before_ok && after_ok {
            return Ok(true);
        }
    }
    checkpoint()?;
    Ok(false)
}

fn trailing_exact_class_with_checkpoints<'a, E>(
    selector: &'a str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<&'a str>, E> {
    let mut class_start = None;
    for (iteration, (index, character)) in selector.char_indices().rev().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        if character == '.' {
            class_start = Some(index + 1);
            break;
        }
    }
    let Some(start) = class_start else {
        checkpoint()?;
        return Ok(None);
    };
    let class_name = &selector[start..];
    if class_name.is_empty() {
        checkpoint()?;
        return Ok(None);
    }
    for (iteration, character) in class_name.chars().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        if !is_css_class_name_char(character) {
            return Ok(None);
        }
    }
    checkpoint()?;
    Ok(Some(class_name))
}

fn trailing_compact_text_class_with_checkpoints<'a, E>(
    selector: &'a str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<&'a str>, E> {
    let Some(selector) = selector.strip_suffix(" text") else {
        checkpoint()?;
        return Ok(None);
    };
    trailing_exact_class_with_checkpoints(selector, checkpoint)
}

fn is_css_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
}

fn is_css_class_name_char(ch: char) -> bool {
    is_css_identifier_char(ch) || !ch.is_ascii()
}

fn parse_style_declarations_with_checkpoints<'a, E>(
    style: &'a str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<StyleDeclarations<'a>, E> {
    let mut declarations = StyleDeclarations::default();
    let mut search = 0usize;
    let mut declaration_index = 0usize;
    loop {
        checkpoint_loop(declaration_index, checkpoint)?;
        declaration_index = declaration_index.saturating_add(1);
        let semicolon = find_with_checkpoints(&style[search..], ";", checkpoint)?;
        let end = semicolon.map_or(style.len(), |relative| search + relative);
        let declaration = &style[search..end];
        if let Some(colon) = find_with_checkpoints(declaration, ":", checkpoint)? {
            let name = trim_with_checkpoints(&declaration[..colon], checkpoint)?;
            let value =
                strip_important_borrowed_with_checkpoints(&declaration[colon + 1..], checkpoint)?;
            if !value.is_empty() {
                if declarations.fill.is_none() && name.eq_ignore_ascii_case("fill") {
                    declarations.fill = Some(value);
                } else if declarations.color.is_none() && name.eq_ignore_ascii_case("color") {
                    declarations.color = Some(value);
                } else if declarations.font_size.is_none() && name.eq_ignore_ascii_case("font-size")
                {
                    declarations.font_size = Some(value);
                } else if declarations.font_family.is_none()
                    && name.eq_ignore_ascii_case("font-family")
                {
                    declarations.font_family = Some(value);
                } else if declarations.font_weight.is_none()
                    && name.eq_ignore_ascii_case("font-weight")
                {
                    declarations.font_weight = Some(value);
                } else if declarations.font_style.is_none()
                    && name.eq_ignore_ascii_case("font-style")
                {
                    declarations.font_style = Some(value);
                }
            }
        }
        if semicolon.is_none() {
            break;
        }
        search = end + 1;
    }
    checkpoint()?;
    Ok(declarations)
}

fn extract_legacy_background_color_with_checkpoints<'a, E>(
    declarations: &'a str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<&'a str>, E> {
    let Some(start) = find_with_checkpoints(declarations, "background-color:", checkpoint)? else {
        return Ok(None);
    };
    let after = &declarations[start + "background-color:".len()..];
    let end = find_with_checkpoints(after, ";", checkpoint)?.unwrap_or(after.len());
    let value = trim_with_checkpoints(&after[..end], checkpoint)?;
    Ok((!value.is_empty()).then_some(value))
}

pub(super) fn extract_style_property_with_checkpoints<E>(
    style: &str,
    property: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<String>, E> {
    checkpoint()?;
    let mut search = 0usize;
    let mut declaration_index = 0usize;
    loop {
        checkpoint_loop(declaration_index, checkpoint)?;
        declaration_index = declaration_index.saturating_add(1);
        let semicolon = find_with_checkpoints(&style[search..], ";", checkpoint)?;
        let end = semicolon.map_or(style.len(), |relative| search + relative);
        let declaration = &style[search..end];
        if let Some(colon) = find_with_checkpoints(declaration, ":", checkpoint)? {
            let name = trim_with_checkpoints(&declaration[..colon], checkpoint)?;
            if name.eq_ignore_ascii_case(property) {
                let value = strip_important_borrowed_with_checkpoints(
                    &declaration[colon + 1..],
                    checkpoint,
                )?;
                if !value.is_empty() {
                    return Ok(Some(value.to_owned()));
                }
            }
        }
        if semicolon.is_none() {
            break;
        }
        search = end + 1;
    }
    checkpoint()?;
    Ok(None)
}

fn strip_important_borrowed_with_checkpoints<'a, E>(
    value: &'a str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<&'a str, E> {
    let value = trim_with_checkpoints(value, checkpoint)?;
    let Some(value) = value.strip_suffix("!important") else {
        return Ok(value);
    };
    trim_with_checkpoints(value, checkpoint)
}

fn strip_important_borrowed(value: &str) -> &str {
    let value = value.trim();
    value
        .strip_suffix("!important")
        .map(str::trim)
        .unwrap_or(value)
}

fn strip_important(value: &str) -> String {
    strip_important_borrowed(value).to_owned()
}

pub(super) fn parse_css_px_value(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    let trimmed = strip_important(trimmed);
    let number = trimmed.strip_suffix("px").unwrap_or(&trimmed).trim();
    number
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
}
