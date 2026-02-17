//! Shared helpers for state diagram layout.

use super::StateNode;
use crate::text::TextStyle;
use dugong::RankDir;
use serde_json::Value;

pub(super) fn state_node_is_effective_group(n: &StateNode) -> bool {
    n.is_group && n.shape != "note"
}

fn json_f64(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_i64().map(|n| n as f64))
        .or_else(|| v.as_u64().map(|n| n as f64))
}

pub(super) fn config_f64(cfg: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    json_f64(cur)
}

pub(super) fn config_bool(cfg: &Value, path: &[&str]) -> Option<bool> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_bool()
}

fn config_string(cfg: &Value, path: &[&str]) -> Option<String> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string()).or_else(|| {
        cur.as_array()
            .and_then(|a| a.first()?.as_str())
            .map(|s| s.to_string())
    })
}

pub(super) fn normalize_dir(direction: &str) -> String {
    match direction.trim().to_uppercase().as_str() {
        "TB" | "TD" => "TB".to_string(),
        "BT" => "BT".to_string(),
        "LR" => "LR".to_string(),
        "RL" => "RL".to_string(),
        other => other.to_string(),
    }
}

pub(super) fn rank_dir_from(direction: &str) -> RankDir {
    match normalize_dir(direction).as_str() {
        "TB" => RankDir::TB,
        "BT" => RankDir::BT,
        "LR" => RankDir::LR,
        "RL" => RankDir::RL,
        _ => RankDir::TB,
    }
}

#[allow(dead_code)]
fn toggle_rank_dir(dir: RankDir) -> RankDir {
    match dir {
        RankDir::TB => RankDir::LR,
        RankDir::LR => RankDir::TB,
        RankDir::BT => RankDir::RL,
        RankDir::RL => RankDir::BT,
    }
}

pub(super) fn value_to_label_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .first()
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        _ => "".to_string(),
    }
}

pub(super) fn decode_html_entities_once(text: &str) -> std::borrow::Cow<'_, str> {
    fn decode_html_entity(entity: &str) -> Option<char> {
        match entity {
            "nbsp" => Some(' '),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "#39" => Some('\''),
            "equals" => Some('='),
            _ => {
                if let Some(hex) = entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                {
                    u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                } else if let Some(dec) = entity.strip_prefix('#') {
                    dec.parse::<u32>().ok().and_then(char::from_u32)
                } else {
                    None
                }
            }
        }
    }

    if !text.contains('&') {
        return std::borrow::Cow::Borrowed(text);
    }

    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while let Some(rel) = text[i..].find('&') {
        let amp = i + rel;
        out.push_str(&text[i..amp]);
        let tail = &text[amp + 1..];
        if let Some(semi_rel) = tail.find(';') {
            let semi = amp + 1 + semi_rel;
            let entity = &text[amp + 1..semi];
            if let Some(decoded) = decode_html_entity(entity) {
                out.push(decoded);
            } else {
                out.push_str(&text[amp..=semi]);
            }
            i = semi + 1;
            continue;
        }
        out.push('&');
        i = amp + 1;
    }
    out.push_str(&text[i..]);
    std::borrow::Cow::Owned(out)
}

pub(crate) fn state_text_style(effective_config: &Value) -> TextStyle {
    // Mermaid state diagram v2 uses HTML labels (foreignObject) by default, inheriting the global
    // `#id{font-size: ...}` rule (defaults to 16px). The 10px `g.stateGroup text{font-size:10px}`
    // rule applies to SVG `<text>` elements, not HTML labels.
    let font_family = config_string(effective_config, &["fontFamily"])
        .or_else(|| config_string(effective_config, &["themeVariables", "fontFamily"]))
        .or_else(|| Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()));
    let font_size = config_f64(effective_config, &["fontSize"]).unwrap_or(16.0);
    TextStyle {
        font_family,
        font_size,
        font_weight: None,
    }
}
