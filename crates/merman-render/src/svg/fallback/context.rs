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
}

impl GFrame {
    pub(super) fn from_g_tag<E>(
        tag: &str,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Self, E> {
        let translate = match extract_exact_double_quoted_attr_with_checkpoints(
            tag,
            "transform",
            checkpoint,
        )? {
            Some(transform) => parse_translate(transform, checkpoint)?,
            None => Translate::default(),
        };
        Ok(Self {
            translate,
            class_tokens: parse_class_tokens(tag, checkpoint)?,
        })
    }
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
            if nums_len < nums.len()
                && let Ok(value) = cur.parse::<f64>()
            {
                nums[nums_len] = value;
                nums_len += 1;
            }
            cur.clear();
        }
    }
    if !cur.is_empty()
        && nums_len < nums.len()
        && let Ok(value) = cur.parse::<f64>()
    {
        nums[nums_len] = value;
        nums_len += 1;
    }
    checkpoint()?;
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
    for (iteration, frame) in stack.iter().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        acc.x += frame.translate.x;
        acc.y += frame.translate.y;
    }
    checkpoint()?;
    Ok(acc)
}

fn parse_class_tokens<E>(
    tag: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<String>, E> {
    let Some(value) = extract_exact_double_quoted_attr_with_checkpoints(tag, "class", checkpoint)?
    else {
        return Ok(Vec::new());
    };
    checkpoint()?;
    Ok(value.split_whitespace().map(str::to_owned).collect())
}

fn push_unique_token<E>(
    tokens: &mut Vec<String>,
    token: &str,
    iteration: &mut usize,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), E> {
    for existing in tokens.iter() {
        checkpoint_loop(*iteration, checkpoint)?;
        *iteration = iteration.saturating_add(1);
        if existing == token {
            return Ok(());
        }
    }
    tokens.push(token.to_string());
    Ok(())
}

pub(super) fn source_class_attr_tokens<E>(
    g_stack: &[GFrame],
    inner: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<String>, E> {
    let mut tokens = Vec::new();
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
    if tokens.is_empty() {
        return Ok(None);
    }
    escape_xml_attr_with_checkpoints(&tokens.join(" "), checkpoint).map(Some)
}
