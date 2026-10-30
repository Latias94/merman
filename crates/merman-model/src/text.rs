//! Pure text value helpers shared by family data and consumers.

/// Decodes the exact minimal entity subset used by Mermaid's visible class text.
///
/// Replacement is deliberately split into two stages to preserve the historical order:
/// `&lt;`, `&gt;`, and `&amp;` first, followed by `&quot;` and `&#39;`. As a result,
/// `&amp;quot;` decodes fully while `&amp;lt;` stops at `&lt;`.
pub fn decode_entities_minimal(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }

    let stage1 = decode_stage1_lt_gt_amp(text);
    if !stage1.contains('&') {
        return stage1;
    }
    if !stage1.contains("&quot;") && !stage1.contains("&#39;") {
        return stage1;
    }
    decode_stage2_quot_apos(&stage1)
}

fn decode_stage1_lt_gt_amp(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        if let Some(stripped) = tail.strip_prefix("&lt;") {
            out.push('<');
            rest = stripped;
        } else if let Some(stripped) = tail.strip_prefix("&gt;") {
            out.push('>');
            rest = stripped;
        } else if let Some(stripped) = tail.strip_prefix("&amp;") {
            out.push('&');
            rest = stripped;
        } else {
            out.push('&');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

fn decode_stage2_quot_apos(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        if let Some(stripped) = tail.strip_prefix("&quot;") {
            out.push('"');
            rest = stripped;
        } else if let Some(stripped) = tail.strip_prefix("&#39;") {
            out.push('\'');
            rest = stripped;
        } else {
            out.push('&');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

#[cfg(feature = "html-entities")]
mod html;
#[cfg(feature = "html-entities")]
pub use html::*;
#[cfg(feature = "terminal-text")]
pub mod terminal;
