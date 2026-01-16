use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SvgDomNode {
    pub(crate) name: String,
    pub(crate) attrs: BTreeMap<String, String>,
    pub(crate) text: Option<String>,
    pub(crate) children: Vec<SvgDomNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DomMode {
    Strict,
    Structure,
    Parity,
}

impl DomMode {
    pub(crate) fn parse(s: &str) -> Self {
        match s {
            "strict" => Self::Strict,
            "parity" => Self::Parity,
            _ => Self::Structure,
        }
    }
}

fn round_f64(v: f64, decimals: u32) -> f64 {
    let p = 10_f64.powi(decimals as i32);
    (v * p).round() / p
}

fn re_num() -> &'static Regex {
    static ONCE: OnceLock<Regex> = OnceLock::new();
    ONCE.get_or_init(|| Regex::new(r"-?(?:\d+\.\d+|\d+\.|\.\d+|\d+)(?:[eE][+-]?\d+)?").unwrap())
}

fn normalize_numeric_tokens(s: &str, decimals: u32) -> String {
    re_num()
        .replace_all(s, |caps: &regex::Captures<'_>| {
            let raw = caps.get(0).map(|m| m.as_str()).unwrap_or_default();
            let Ok(v) = raw.parse::<f64>() else {
                return raw.to_string();
            };
            let r = round_f64(v, decimals);
            let r = if r == 0.0 { 0.0 } else { r };
            let mut out = format!("{r}");
            if out.contains('.') {
                while out.ends_with('0') {
                    out.pop();
                }
                if out.ends_with('.') {
                    out.pop();
                }
            }
            out
        })
        .to_string()
}

fn normalize_numeric_tokens_mode(s: &str, decimals: u32, mode: DomMode) -> String {
    match mode {
        DomMode::Strict | DomMode::Parity => normalize_numeric_tokens(s, decimals),
        DomMode::Structure => re_num().replace_all(s, "<n>").to_string(),
    }
}

fn is_identifier_like_attr(key: &str) -> bool {
    matches!(
        key,
        "id" | "data-id"
            | "href"
            | "xlink:href"
            | "title"
            | "aria-labelledby"
            | "aria-describedby"
            | "aria-label"
            | "aria-roledescription"
    )
}

fn re_trailing_counter() -> &'static Regex {
    static ONCE: OnceLock<Regex> = OnceLock::new();
    ONCE.get_or_init(|| Regex::new(r"([_-])\d+$").unwrap())
}

fn re_mermaid_generate_id() -> &'static Regex {
    static ONCE: OnceLock<Regex> = OnceLock::new();
    ONCE.get_or_init(|| Regex::new(r"id-[a-z0-9]+-\d+").unwrap())
}

fn normalize_identifier_tokens(s: &str) -> String {
    let s = re_mermaid_generate_id()
        .replace_all(s, "id-<id>-<n>")
        .to_string();
    re_trailing_counter().replace(&s, "$1<n>").to_string()
}

fn normalize_class_list(s: &str) -> String {
    let mut parts: Vec<&str> = s.split_whitespace().collect();
    parts.sort_unstable();
    parts.dedup();
    parts.join(" ")
}

fn is_geometry_attr(name: &str) -> bool {
    matches!(
        name,
        "transform"
            | "d"
            | "points"
            | "x"
            | "y"
            | "x1"
            | "y1"
            | "x2"
            | "y2"
            | "cx"
            | "cy"
            | "r"
            | "rx"
            | "ry"
            | "width"
            | "height"
    )
}

fn build_node(n: roxmltree::Node<'_, '_>, mode: DomMode, decimals: u32) -> SvgDomNode {
    let mut attrs: BTreeMap<String, String> = BTreeMap::new();
    if n.is_element() {
        for a in n.attributes() {
            let key = a.name().to_string();
            let mut val = a.value().to_string();

            let mut normalized_geom = false;
            if mode != DomMode::Strict {
                if key == "data-points" {
                    val = "<data-points>".to_string();
                    normalized_geom = true;
                }
                if key == "d" || key == "points" {
                    if mode == DomMode::Structure {
                        val = "<geom>".to_string();
                        normalized_geom = true;
                    } else if mode == DomMode::Parity {
                        if key == "d"
                            && n.tag_name().name() == "path"
                            && n.attribute("class")
                                .is_some_and(|c| c.split_whitespace().any(|t| t == "relation"))
                        {
                            // Edge routing geometry differs across layout engines; treat edge path `d`
                            // as geometry noise in parity mode.
                            val = "<geom>".to_string();
                            normalized_geom = true;
                        } else {
                            // Keep command letters but treat numeric payload as geometry noise.
                            // This enables parity checks to catch path/points structure changes while
                            // ignoring layout-specific numeric drift.
                            let v = val.replace(',', " ");
                            let v = normalize_numeric_tokens_mode(&v, decimals, DomMode::Structure);
                            val = v.chars().filter(|c| !c.is_whitespace()).collect();
                            normalized_geom = true;
                        }
                    }
                }
                if key == "style" || key == "viewBox" {
                    if n.tag_name().name() == "svg" {
                        continue;
                    }
                    if key == "style" {
                        continue;
                    }
                }
            }

            if key == "class" {
                val = normalize_class_list(&val);
            }
            if mode == DomMode::Structure && is_identifier_like_attr(&key) {
                val = normalize_identifier_tokens(&val);
            } else if !normalized_geom && mode == DomMode::Parity && is_geometry_attr(&key) {
                val = normalize_numeric_tokens_mode(&val, decimals, DomMode::Structure);
            } else {
                val = normalize_numeric_tokens_mode(&val, decimals, mode);
            }
            attrs.insert(key, val);
        }
    }

    let text = n
        .text()
        .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|t| !t.is_empty());

    let mut children: Vec<SvgDomNode> = Vec::new();
    for c in n.children().filter(|c| c.is_element()) {
        children.push(build_node(c, mode, decimals));
    }

    if mode != DomMode::Strict {
        fn is_cluster_class_token(c: &str) -> bool {
            c == "cluster" || c.ends_with("-cluster") || c.ends_with("_cluster")
        }

        fn find_first_cluster_id(n: &SvgDomNode) -> Option<&str> {
            if n.name == "g" {
                if let Some(class) = n.attrs.get("class") {
                    if class.split_whitespace().any(is_cluster_class_token) {
                        if let Some(id) = n.attrs.get("id") {
                            return Some(id.as_str());
                        }
                    }
                }
            }
            for c in &n.children {
                if let Some(id) = find_first_cluster_id(c) {
                    return Some(id);
                }
            }
            None
        }

        fn sort_hint(n: &SvgDomNode) -> &str {
            if let Some(id) = n.attrs.get("id") {
                return id.as_str();
            }
            if let Some(id) = n.attrs.get("data-id") {
                return id.as_str();
            }
            if n.name == "g" {
                fn find_first_data_id(n: &SvgDomNode) -> Option<&str> {
                    if let Some(id) = n.attrs.get("data-id") {
                        return Some(id.as_str());
                    }
                    for c in &n.children {
                        if let Some(id) = find_first_data_id(c) {
                            return Some(id);
                        }
                    }
                    None
                }

                if let Some(class) = n.attrs.get("class") {
                    if class.split_whitespace().any(|c| c == "root") {
                        if let Some(id) = find_first_cluster_id(n) {
                            return id;
                        }
                    }
                    if class.split_whitespace().any(|c| c == "edgeLabel") {
                        if let Some(id) = find_first_data_id(n) {
                            return id;
                        }
                    }
                }
            }
            ""
        }

        children.sort_by(|a, b| {
            let aclass = a.attrs.get("class").map(|s| s.as_str()).unwrap_or("");
            let bclass = b.attrs.get("class").map(|s| s.as_str()).unwrap_or("");
            let ahint = sort_hint(a);
            let bhint = sort_hint(b);
            a.name
                .cmp(&b.name)
                .then_with(|| ahint.cmp(bhint))
                .then_with(|| aclass.cmp(bclass))
        });
    }

    SvgDomNode {
        name: n.tag_name().name().to_string(),
        attrs,
        text: if mode == DomMode::Strict { text } else { None },
        children,
    }
}

pub(crate) fn dom_signature(svg: &str, mode: DomMode, decimals: u32) -> Result<SvgDomNode, String> {
    let doc = roxmltree::Document::parse(svg).map_err(|e| e.to_string())?;
    let root = doc
        .descendants()
        .find(|n| n.has_tag_name("svg"))
        .ok_or_else(|| "missing <svg> root".to_string())?;
    Ok(build_node(root, mode, decimals))
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }
    let mut out = s
        .chars()
        .take(max_len.saturating_sub(1))
        .collect::<String>();
    out.push('…');
    out
}

pub(crate) fn dom_diff_path(
    upstream: &SvgDomNode,
    local: &SvgDomNode,
    path: &mut Vec<String>,
) -> Option<String> {
    if upstream.name != local.name {
        return Some(format!(
            "{}: element name mismatch upstream={} local={}",
            path.join("/"),
            upstream.name,
            local.name
        ));
    }

    if upstream.attrs != local.attrs {
        for (k, v_up) in &upstream.attrs {
            match local.attrs.get(k) {
                None => return Some(format!("{}: missing attr `{k}`", path.join("/"))),
                Some(v_lo) if v_lo != v_up => {
                    return Some(format!(
                        "{}: attr `{k}` mismatch upstream=`{}` local=`{}`",
                        path.join("/"),
                        truncate(v_up, 120),
                        truncate(v_lo, 120)
                    ));
                }
                _ => {}
            }
        }
        for k in local.attrs.keys() {
            if !upstream.attrs.contains_key(k) {
                return Some(format!("{}: extra attr `{k}`", path.join("/")));
            }
        }
    }

    if upstream.text != local.text {
        return Some(format!(
            "{}: text mismatch upstream=`{}` local=`{}`",
            path.join("/"),
            truncate(upstream.text.as_deref().unwrap_or(""), 120),
            truncate(local.text.as_deref().unwrap_or(""), 120)
        ));
    }

    let n = upstream.children.len().min(local.children.len());
    for i in 0..n {
        path.push(format!("{}[{}]", upstream.children[i].name, i));
        if let Some(d) = dom_diff_path(&upstream.children[i], &local.children[i], path) {
            return Some(d);
        }
        path.pop();
    }

    if upstream.children.len() != local.children.len() {
        return Some(format!(
            "{}: child count mismatch upstream={} local={}",
            path.join("/"),
            upstream.children.len(),
            local.children.len()
        ));
    }

    None
}

pub(crate) fn dom_diff(upstream: &SvgDomNode, local: &SvgDomNode) -> Option<String> {
    let mut path = vec![upstream.name.clone()];
    dom_diff_path(upstream, local, &mut path)
}
