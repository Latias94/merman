use super::css::{
    FallbackStyleIndex,
    extract_style_property_with_checkpoints as extract_css_style_property_with_checkpoints,
};
use crate::svg::pipeline::{
    checkpoint_loop, escape_xml_attr_with_checkpoints,
    extract_exact_double_quoted_attr_with_checkpoints, find_with_checkpoints,
};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Translate {
    pub(super) x: f64,
    pub(super) y: f64,
}

#[derive(Clone, Debug, Default)]
pub(super) struct GFrame {
    translate: Translate,
    class_tokens: Vec<String>,
    fill: Option<String>,
    font_size: Option<String>,
    font_family: Option<String>,
    font_weight: Option<String>,
    font_style: Option<String>,
}

impl GFrame {
    pub(super) fn from_g_tag<E>(
        tag: &str,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Self, E> {
        let style = extract_exact_double_quoted_attr_with_checkpoints(tag, "style", checkpoint)?;
        let translate = match extract_exact_double_quoted_attr_with_checkpoints(
            tag,
            "transform",
            checkpoint,
        )? {
            Some(transform) => parse_translate(transform, checkpoint)?,
            None => Translate::default(),
        };
        let fill = match extract_exact_double_quoted_attr_with_checkpoints(tag, "fill", checkpoint)?
        {
            Some(fill) => Some(fill.to_owned()),
            None => extract_style_property_with_checkpoints(style, "fill", checkpoint)?,
        };

        Ok(Self {
            translate,
            class_tokens: parse_class_tokens(tag, checkpoint)?,
            fill,
            font_size: extract_style_property_with_checkpoints(style, "font-size", checkpoint)?,
            font_family: extract_style_property_with_checkpoints(style, "font-family", checkpoint)?,
            font_weight: extract_style_property_with_checkpoints(style, "font-weight", checkpoint)?,
            font_style: extract_style_property_with_checkpoints(style, "font-style", checkpoint)?,
        })
    }
}

fn extract_style_property_with_checkpoints<E>(
    style: Option<&str>,
    property: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<String>, E> {
    let Some(style) = style else {
        return Ok(None);
    };
    extract_css_style_property_with_checkpoints(style, property, checkpoint)
}

fn find_ascii_case_insensitive_with_checkpoints<E>(
    haystack: &str,
    needle: &[u8],
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<usize>, E> {
    for (iteration, offset) in (0..haystack.len()).enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        let Some(candidate) = haystack
            .as_bytes()
            .get(offset..offset.saturating_add(needle.len()))
        else {
            break;
        };
        if candidate.eq_ignore_ascii_case(needle) {
            return Ok(Some(offset));
        }
    }
    checkpoint()?;
    Ok(None)
}

fn parse_translate<E>(
    transform: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Translate, E> {
    let Some(i) =
        find_ascii_case_insensitive_with_checkpoints(transform, b"translate(", checkpoint)?
    else {
        return Ok(Translate::default());
    };
    let after = &transform[i + "translate(".len()..];
    let Some(end) = find_with_checkpoints(after, ")", checkpoint)? else {
        return Ok(Translate::default());
    };
    let args = &after[..end];

    let mut nums = [0.0f64; 2];
    let mut nums_len = 0usize;
    let mut cur = String::new();
    for (iteration, ch) in args.chars().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        if ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+' || ch == 'e' || ch == 'E' {
            cur.push(ch);
        } else if !cur.is_empty() {
            checkpoint()?;
            if nums_len < nums.len()
                && let Ok(value) = cur.parse::<f64>()
            {
                nums[nums_len] = value;
                nums_len += 1;
            }
            checkpoint()?;
            cur.clear();
        }
    }
    if !cur.is_empty() && nums_len < nums.len() {
        checkpoint()?;
        if let Ok(value) = cur.parse::<f64>() {
            nums[nums_len] = value;
            nums_len += 1;
        }
        checkpoint()?;
    }

    Ok(Translate {
        x: nums
            .first()
            .copied()
            .filter(|_| nums_len > 0)
            .unwrap_or(0.0),
        y: nums.get(1).copied().filter(|_| nums_len > 1).unwrap_or(0.0),
    })
}

pub(super) fn sum_translate<E>(
    stack: &[GFrame],
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Translate, E> {
    let mut acc = Translate::default();
    for (iteration, t) in stack.iter().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        acc.x += t.translate.x;
        acc.y += t.translate.y;
    }
    checkpoint()?;
    Ok(acc)
}

pub(super) fn extract_svg_text_fill_from_ancestors<E>(
    style_index: &FallbackStyleIndex<'_>,
    g_stack: &[GFrame],
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<String>, E> {
    let mut iteration = 0usize;
    // Prefer the closest ancestor's classes (more specific) by scanning frames from inner -> outer.
    for frame in g_stack.iter().rev() {
        checkpoint_loop(iteration, checkpoint)?;
        iteration = iteration.saturating_add(1);
        for token in frame.class_tokens.iter().rev() {
            checkpoint_loop(iteration, checkpoint)?;
            iteration = iteration.saturating_add(1);
            if let Some(fill) = style_index.text_fill_for_class(token) {
                return Ok(Some(fill.to_owned()));
            }
        }
        if let Some(fill) = &frame.fill {
            return Ok(Some(fill.clone()));
        }
    }
    Ok(style_index.root_text_fill().map(str::to_owned))
}

fn extract_svg_font_style_from_ancestors<E>(
    g_stack: &[GFrame],
    property: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<String>, E> {
    for (iteration, frame) in g_stack.iter().rev().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        let value = match property {
            "font-size" => &frame.font_size,
            "font-family" => &frame.font_family,
            "font-weight" => &frame.font_weight,
            "font-style" => &frame.font_style,
            _ => return Ok(None),
        };
        if let Some(value) = value {
            return Ok(Some(value.clone()));
        }
    }
    checkpoint()?;
    Ok(None)
}

pub(super) fn extract_svg_font_style_from_context<E>(
    style_index: &FallbackStyleIndex<'_>,
    g_stack: &[GFrame],
    property: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<String>, E> {
    if let Some(value) = extract_svg_font_style_from_ancestors(g_stack, property, checkpoint)? {
        return Ok(Some(value));
    }
    let mut iteration = 0usize;
    for frame in g_stack.iter().rev() {
        checkpoint_loop(iteration, checkpoint)?;
        iteration = iteration.saturating_add(1);
        for token in frame.class_tokens.iter().rev() {
            checkpoint_loop(iteration, checkpoint)?;
            iteration = iteration.saturating_add(1);
            if let Some(value) = style_index.style_property_for_class(token, property) {
                return Ok(Some(value.to_owned()));
            }
        }
    }
    Ok(style_index.root_style_property(property).map(str::to_owned))
}

fn parse_class_tokens<E>(
    tag: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<String>, E> {
    let Some(value) = extract_exact_double_quoted_attr_with_checkpoints(tag, "class", checkpoint)?
    else {
        return Ok(Vec::new());
    };
    let mut tokens = Vec::new();
    let mut token_start = None;
    for (iteration, (offset, character)) in value.char_indices().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        if character.is_whitespace() {
            if let Some(start) = token_start.take() {
                tokens.push(value[start..offset].to_string());
            }
        } else if token_start.is_none() {
            token_start = Some(offset);
        }
    }
    if let Some(start) = token_start {
        tokens.push(value[start..].to_string());
    }
    checkpoint()?;
    Ok(tokens)
}

fn push_unique_token<E>(
    tokens: &mut Vec<String>,
    token: &str,
    iteration: &mut usize,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), E> {
    let mut exists = false;
    for existing in tokens.iter() {
        checkpoint_loop(*iteration, checkpoint)?;
        *iteration = iteration.saturating_add(1);
        if existing == token {
            exists = true;
            break;
        }
    }
    if !exists {
        tokens.push(token.to_string());
    }
    Ok(())
}

pub(super) fn class_attr_tokens<E>(
    g_stack: &[GFrame],
    inner: &str,
    base_class: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<String, E> {
    let mut tokens = vec![base_class.to_string()];
    let mut iteration = 0usize;
    for frame in g_stack {
        for token in &frame.class_tokens {
            push_unique_token(&mut tokens, token, &mut iteration, checkpoint)?;
        }
    }
    for token in parse_class_tokens(inner, checkpoint)? {
        push_unique_token(&mut tokens, &token, &mut iteration, checkpoint)?;
    }
    checkpoint()?;
    let joined = tokens.join(" ");
    checkpoint()?;
    escape_xml_attr_with_checkpoints(&joined, checkpoint)
}

pub(super) fn fallback_text_class_attr_tokens<E>(
    g_stack: &[GFrame],
    inner: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<String, E> {
    let mut tokens = vec!["merman-foreignobject-fallback-text".to_string()];
    let mut iteration = 0usize;
    for frame in g_stack {
        for token in &frame.class_tokens {
            if is_fallback_text_safe_class(token) {
                push_unique_token(&mut tokens, token, &mut iteration, checkpoint)?;
            }
        }
    }
    for token in parse_class_tokens(inner, checkpoint)? {
        if is_fallback_text_safe_class(&token) {
            push_unique_token(&mut tokens, &token, &mut iteration, checkpoint)?;
        }
    }
    checkpoint()?;
    let joined = tokens.join(" ");
    checkpoint()?;
    escape_xml_attr_with_checkpoints(&joined, checkpoint)
}

fn is_fallback_text_safe_class(class_name: &str) -> bool {
    !matches!(class_name, "label")
}
