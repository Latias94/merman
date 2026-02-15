use regex::Regex;
use std::borrow::Cow;
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
    ParityRoot,
}

impl DomMode {
    pub(crate) fn parse(s: &str) -> Self {
        match s {
            "strict" => Self::Strict,
            "parity" => Self::Parity,
            "parity-root" | "parity_root" => Self::ParityRoot,
            _ => Self::Structure,
        }
    }
}

fn round_f64(v: f64, decimals: u32) -> f64 {
    let p = 10_f64.powi(decimals as i32);
    (v * p).round() / p
}

fn normalize_data_points_base64_json(s: &str, decimals: u32) -> Option<String> {
    use base64::Engine as _;

    fn round_json_numbers(value: &mut serde_json::Value, decimals: u32) {
        match value {
            serde_json::Value::Number(n) => {
                let Some(v) = n.as_f64() else {
                    return;
                };
                let r = round_f64(v, decimals);
                if let Some(new_n) = serde_json::Number::from_f64(r) {
                    *n = new_n;
                }
            }
            serde_json::Value::Array(arr) => {
                for v in arr {
                    round_json_numbers(v, decimals);
                }
            }
            serde_json::Value::Object(map) => {
                for (_, v) in map.iter_mut() {
                    round_json_numbers(v, decimals);
                }
            }
            _ => {}
        }
    }

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(s.as_bytes())
        .ok()?;
    let mut json = serde_json::from_slice::<serde_json::Value>(&bytes).ok()?;
    round_json_numbers(&mut json, decimals);
    let out = serde_json::to_string(&json).ok()?;
    Some(base64::engine::general_purpose::STANDARD.encode(out.as_bytes()))
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
        DomMode::Strict | DomMode::Parity | DomMode::ParityRoot => {
            normalize_numeric_tokens(s, decimals)
        }
        DomMode::Structure => re_num().replace_all(s, "<n>").to_string(),
    }
}

fn normalize_attr_whitespace_strict(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = true;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
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

fn re_mermaid_generate_id_capture() -> &'static Regex {
    static ONCE: OnceLock<Regex> = OnceLock::new();
    ONCE.get_or_init(|| Regex::new(r"id-[a-z0-9]+-(\d+)").unwrap())
}

fn re_gitgraph_dynamic_commit_id() -> &'static Regex {
    static ONCE: OnceLock<Regex> = OnceLock::new();
    ONCE.get_or_init(|| Regex::new(r"\b(\d+)-[0-9a-f]{7}\b").unwrap())
}

fn normalize_gitgraph_dynamic_commit_ids(s: &str) -> String {
    re_gitgraph_dynamic_commit_id()
        .replace_all(s, "$1-<dynamic>")
        .to_string()
}

fn normalize_identifier_tokens(s: &str) -> String {
    let s = re_mermaid_generate_id()
        .replace_all(s, "id-<id>-<n>")
        .to_string();
    re_trailing_counter().replace(&s, "$1<n>").to_string()
}

fn normalize_mermaid_generated_id_only(s: &str) -> String {
    re_mermaid_generate_id_capture()
        .replace_all(s, "id-<id>-$1")
        .to_string()
}

fn normalize_class_list(s: &str, mode: DomMode) -> String {
    let mut parts: Vec<String> = s
        .split_whitespace()
        .map(|t| {
            if mode != DomMode::Strict && t.starts_with("width-") {
                let suffix = &t["width-".len()..];
                if suffix.parse::<f64>().is_ok() {
                    return "width-<n>".to_string();
                }
            }
            t.to_string()
        })
        .collect();
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
        fn is_mindmap_diagram(n: roxmltree::Node<'_, '_>) -> bool {
            for a in n.ancestors() {
                if a.is_element() && a.tag_name().name() == "svg" {
                    return a
                        .attribute("class")
                        .is_some_and(|c| c.split_whitespace().any(|t| t == "mindmapDiagram"));
                }
            }
            false
        }

        fn is_architecture_service_icon_content(n: roxmltree::Node<'_, '_>) -> bool {
            let mut svg_count = 0;
            for a in n.ancestors() {
                if a.is_element() && a.tag_name().name() == "svg" {
                    svg_count += 1;
                    if svg_count >= 2 {
                        break;
                    }
                }
            }
            if svg_count < 2 {
                return false;
            }
            n.ancestors().any(|a| {
                a.is_element()
                    && a.tag_name().name() == "g"
                    && a.attribute("class")
                        .is_some_and(|c| c.split_whitespace().any(|t| t == "architecture-service"))
            })
        }

        for a in n.attributes() {
            let key = a.name().to_string();
            let mut val = a.value().to_string();

            if mode == DomMode::Strict && matches!(key.as_str(), "d" | "points" | "transform") {
                val = normalize_attr_whitespace_strict(&val);
            }

            // `data-points` is a base64-encoded JSON payload (Mermaid uses `btoa(JSON.stringify(...))`).
            // In strict mode we want stable, precision-controlled DOM comparisons, so we normalize the
            // *decoded* JSON numbers and then re-encode. Importantly, we must not run generic numeric
            // token normalization on the base64 string itself (it can corrupt the payload).
            if mode == DomMode::Strict && key == "data-points" {
                if let Some(normalized) = normalize_data_points_base64_json(&val, decimals) {
                    val = normalized;
                }
                attrs.insert(key, val);
                continue;
            }

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
                    } else if matches!(mode, DomMode::Parity | DomMode::ParityRoot) {
                        if key == "d" && n.tag_name().name() == "path" && is_mindmap_diagram(n) {
                            // Mindmap node/edge paths are highly layout-dependent. Treat them as
                            // geometry noise in parity mode to focus checks on DOM structure and
                            // semantic attributes.
                            val = "<geom>".to_string();
                            normalized_geom = true;
                        } else if key == "d"
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
                    if n.tag_name().name() == "svg" && mode != DomMode::ParityRoot {
                        continue;
                    }
                    if key == "style" {
                        if n.tag_name().name() == "svg" && mode == DomMode::ParityRoot {
                        } else {
                            continue;
                        }
                    }
                }
            }

            if key == "class" {
                val = normalize_class_list(&val, mode);
                val = normalize_gitgraph_dynamic_commit_ids(&val);
            }
            if matches!(
                mode,
                DomMode::Strict | DomMode::Structure | DomMode::Parity | DomMode::ParityRoot
            ) && key == "id"
                && is_architecture_service_icon_content(n)
                && (val.starts_with("IconifyId") || val.len() <= 2)
            {
                val = "<icon-id>".to_string();
            }
            if mode == DomMode::Structure && is_identifier_like_attr(&key) {
                val = normalize_identifier_tokens(&val);
            } else if matches!(mode, DomMode::Parity | DomMode::ParityRoot)
                && is_identifier_like_attr(&key)
            {
                val = normalize_mermaid_generated_id_only(&val);
            } else if mode == DomMode::Strict && is_identifier_like_attr(&key) {
                // In strict mode, keep identifier-like attributes byte-for-byte (aside from XML
                // escaping). Applying numeric token normalization here is unsafe, as it can
                // accidentally rewrite IDs like `flowchart-A-0` into `flowchart-A0` by treating
                // `-0` as a number.
            } else if !normalized_geom
                && matches!(mode, DomMode::Parity | DomMode::ParityRoot)
                && is_geometry_attr(&key)
                && !(mode == DomMode::ParityRoot
                    && n.tag_name().name() == "svg"
                    && (key == "width" || key == "height"))
            {
                val = normalize_numeric_tokens_mode(&val, decimals, DomMode::Structure);
            } else {
                val = normalize_numeric_tokens_mode(&val, decimals, mode);
            }
            attrs.insert(key, val);
        }
    }

    fn normalize_text_node_text(t: &str) -> Option<String> {
        let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
        if t.is_empty() { None } else { Some(t) }
    }

    let mut text: Option<String> = None;
    let mut children: Vec<SvgDomNode> = Vec::new();

    if mode == DomMode::Strict {
        // In strict mode we must preserve mixed-content text (e.g. `foo<br />bar`), where
        // `roxmltree::Node::text()` would only return the first text segment.
        let has_element_child = n.children().any(|c| c.is_element());
        if has_element_child {
            for c in n.children() {
                if c.is_element() {
                    children.push(build_node(c, mode, decimals));
                } else if c.is_text() {
                    if let Some(t) = c.text().and_then(normalize_text_node_text) {
                        children.push(SvgDomNode {
                            name: "#text".to_string(),
                            attrs: BTreeMap::new(),
                            text: Some(t),
                            children: Vec::new(),
                        });
                    }
                }
            }
        } else {
            text = n.text().and_then(normalize_text_node_text);
            for c in n.children().filter(|c| c.is_element()) {
                children.push(build_node(c, mode, decimals));
            }
        }
    } else {
        // Non-strict modes treat text as non-semantic and only track element structure.
        for c in n.children().filter(|c| c.is_element()) {
            children.push(build_node(c, mode, decimals));
        }
    }

    if n.is_element() && n.tag_name().name() == "style" {
        // Stylesheets are large and may differ in whitespace, ordering, and numeric precision
        // even when the effective rendering is unchanged. Treat them as non-semantic for DOM
        // comparisons, including strict mode (we track DOM structure separately from CSS parity).
        text = None;
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
        text,
        children,
    }
}

pub(crate) fn dom_signature(svg: &str, mode: DomMode, decimals: u32) -> Result<SvgDomNode, String> {
    fn normalize_xml_entities(svg: &str) -> Cow<'_, str> {
        // Mermaid SVG output (especially `<foreignObject>` XHTML) can contain HTML entity references
        // like `&nbsp;`, which are not predefined in XML and thus fail strict parsers.
        //
        // Replace the most common cases with a plain space so we can parse and compare DOM trees
        // deterministically. This only affects DOM comparison; it does not change SVG rendering.
        if !(svg.contains("&nbsp;") || svg.contains("&#160;") || svg.contains("&#xA0;")) {
            return Cow::Borrowed(svg);
        }
        let mut out = svg.to_string();
        out = out.replace("&nbsp;", " ");
        out = out.replace("&#160;", " ").replace("&#xA0;", " ");
        Cow::Owned(out)
    }

    let svg = normalize_xml_entities(svg);
    let doc = roxmltree::Document::parse(svg.as_ref()).map_err(|e| e.to_string())?;
    let root = doc
        .descendants()
        .find(|n| n.has_tag_name("svg"))
        .ok_or_else(|| "missing <svg> root".to_string())?;
    Ok(build_node(root, mode, decimals))
}

fn escape_xml_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_xml_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

fn write_indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn write_canonical_node(out: &mut String, n: &SvgDomNode, depth: usize) {
    if n.name == "#text" {
        let Some(t) = n.text.as_deref().filter(|t| !t.is_empty()) else {
            return;
        };
        write_indent(out, depth);
        out.push_str(&escape_xml_text(t));
        out.push('\n');
        return;
    }

    write_indent(out, depth);
    out.push('<');
    out.push_str(&n.name);

    for (k, v) in &n.attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        out.push_str(&escape_xml_attr(v));
        out.push('"');
    }

    let has_children = !n.children.is_empty();
    let has_text = n.text.as_ref().is_some_and(|t| !t.is_empty());

    if !has_children && !has_text {
        out.push_str("/>\n");
        return;
    }

    out.push('>');

    if has_text {
        out.push_str(&escape_xml_text(n.text.as_deref().unwrap_or_default()));
    }

    if has_children {
        out.push('\n');
        for c in &n.children {
            write_canonical_node(out, c, depth + 1);
        }
        write_indent(out, depth);
    }

    out.push_str("</");
    out.push_str(&n.name);
    out.push_str(">\n");
}

pub(crate) fn canonical_xml(svg: &str, mode: DomMode, decimals: u32) -> Result<String, String> {
    let dom = dom_signature(svg, mode, decimals)?;
    let mut out = String::new();
    write_canonical_node(&mut out, &dom, 0);
    Ok(out)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_does_not_normalize_numbers_inside_identifier_like_attrs() {
        let svg = r#"<svg id="flowchart-A-0" aria-label="foo-0 bar" aria-roledescription="flowchart-v2"><g id="id-abc-0"/></svg>"#;
        let dom = dom_signature(svg, DomMode::Strict, 3).unwrap();
        assert_eq!(
            dom.attrs.get("id").map(|s| s.as_str()),
            Some("flowchart-A-0")
        );
        assert_eq!(
            dom.attrs.get("aria-label").map(|s| s.as_str()),
            Some("foo-0 bar")
        );
        assert_eq!(
            dom.attrs.get("aria-roledescription").map(|s| s.as_str()),
            Some("flowchart-v2")
        );
        assert_eq!(dom.children.len(), 1);
        assert_eq!(
            dom.children[0].attrs.get("id").map(|s| s.as_str()),
            Some("id-abc-0")
        );
    }

    #[test]
    fn strict_preserves_mixed_content_text_segments() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><foreignObject><div xmlns="http://www.w3.org/1999/xhtml"><p>This is a<br />multiline string</p></div></foreignObject></svg>"#;
        let dom = dom_signature(svg, DomMode::Strict, 3).unwrap();

        let p = dom
            .children
            .iter()
            .find(|n| n.name == "foreignObject")
            .and_then(|fo| fo.children.iter().find(|n| n.name == "div"))
            .and_then(|div| div.children.iter().find(|n| n.name == "p"))
            .expect("p exists");

        assert_eq!(p.children.len(), 3);
        assert_eq!(p.children[0].name, "#text");
        assert_eq!(p.children[0].text.as_deref(), Some("This is a"));
        assert_eq!(p.children[1].name, "br");
        assert_eq!(p.children[2].name, "#text");
        assert_eq!(p.children[2].text.as_deref(), Some("multiline string"));

        let xml = canonical_xml(svg, DomMode::Strict, 3).unwrap();
        assert!(xml.contains("This is a"));
        assert!(xml.contains("multiline string"));
    }

    #[test]
    fn parity_keeps_path_commands_but_masks_numbers() {
        let svg = r#"<svg width="100" height="100" viewBox="0 0 100 100"><path d="M 10 20 L 30 40"/></svg>"#;
        let dom = dom_signature(svg, DomMode::Parity, 3).unwrap();
        assert!(!dom.attrs.contains_key("viewBox"));
        assert_eq!(dom.children.len(), 1);
        assert_eq!(dom.children[0].name, "path");
        assert_eq!(
            dom.children[0].attrs.get("d").map(|s| s.as_str()),
            Some("M<n><n>L<n><n>")
        );
    }

    #[test]
    fn parity_root_keeps_svg_root_viewbox_and_style() {
        let svg = r#"<svg width="100%" viewBox="0 -5.9759 600 405.9759" style="max-width: 600px; background-color: white;"><path d="M 10 20 L 30 40"/></svg>"#;
        let dom = dom_signature(svg, DomMode::ParityRoot, 3).unwrap();
        assert_eq!(dom.attrs.get("width").map(|s| s.as_str()), Some("100%"));
        assert_eq!(
            dom.attrs.get("viewBox").map(|s| s.as_str()),
            Some("0 -5.976 600 405.976")
        );
        assert_eq!(
            dom.attrs.get("style").map(|s| s.as_str()),
            Some("max-width: 600px; background-color: white;")
        );
    }

    #[test]
    fn parity_masks_relation_path_as_geom() {
        let svg = r#"<svg><path class="relation" d="M0,0L1,1"/></svg>"#;
        let dom = dom_signature(svg, DomMode::Parity, 3).unwrap();
        assert_eq!(
            dom.children[0].attrs.get("d").map(|s| s.as_str()),
            Some("<geom>")
        );
    }

    #[test]
    fn structure_normalizes_identifier_tokens_and_ignores_text() {
        let svg = r#"<svg><g id="foo_12"><text> hi   there </text></g></svg>"#;
        let dom = dom_signature(svg, DomMode::Structure, 3).unwrap();
        assert_eq!(
            dom.children[0].attrs.get("id").map(|s| s.as_str()),
            Some("foo_<n>")
        );
        assert_eq!(dom.children[0].children[0].text, None);
    }

    #[test]
    fn structure_masks_architecture_icon_internal_ids() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><g class="architecture-service"><g><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><g><ellipse id="IconifyIddeadbeef" cx="5" cy="5" rx="4" ry="4"/></g></svg></g></g></svg>"#;
        let dom = dom_signature(svg, DomMode::Structure, 3).unwrap();
        fn find_ellipse_id(n: &SvgDomNode) -> Option<&str> {
            if n.name == "ellipse" {
                return n.attrs.get("id").map(|s| s.as_str());
            }
            for c in &n.children {
                if let Some(id) = find_ellipse_id(c) {
                    return Some(id);
                }
            }
            None
        }

        assert_eq!(find_ellipse_id(&dom), Some("<icon-id>"));
    }

    #[test]
    fn non_strict_sorts_children_deterministically() {
        let a = r#"<svg><g id="b"/><g id="a"/></svg>"#;
        let b = r#"<svg><g id="a"/><g id="b"/></svg>"#;
        let sig_a = dom_signature(a, DomMode::Parity, 3).unwrap();
        let sig_b = dom_signature(b, DomMode::Parity, 3).unwrap();
        assert_eq!(sig_a, sig_b);
    }

    #[test]
    fn parity_masks_geometry_attrs_as_n() {
        let svg = r#"<svg><rect x="12.3" y="4.56" width="7" height="8"/></svg>"#;
        let dom = dom_signature(svg, DomMode::Parity, 3).unwrap();
        let rect = &dom.children[0];
        assert_eq!(rect.attrs.get("x").map(|s| s.as_str()), Some("<n>"));
        assert_eq!(rect.attrs.get("y").map(|s| s.as_str()), Some("<n>"));
        assert_eq!(rect.attrs.get("width").map(|s| s.as_str()), Some("<n>"));
        assert_eq!(rect.attrs.get("height").map(|s| s.as_str()), Some("<n>"));
    }
}
