use regex::Regex;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[cfg(test)]
use std::cell::Cell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SvgDomNode {
    pub(crate) name: String,
    pub(crate) attrs: BTreeMap<String, String>,
    pub(crate) text: Option<String>,
    pub(crate) children: Vec<SvgDomNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DomMode {
    Strict,
    Structure,
    Parity,
    ParityRoot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DomComparisonProfile {
    descendants: DomMode,
    root_contract: bool,
    normalize_browser_text_wrapping: bool,
    normalize_browser_text_word_boundaries: bool,
    normalize_browser_text_length: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct DomSignatureKey {
    descendants: DomMode,
    decimals: u32,
    normalize_browser_text_wrapping: bool,
    normalize_browser_text_word_boundaries: bool,
    normalize_browser_text_length: bool,
}

pub(crate) struct ParsedSvgDom<'input> {
    document: roxmltree::Document<'input>,
    signatures: BTreeMap<DomSignatureKey, SvgDomNode>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CanonicalLocalSvgSignature {
    decimals: u32,
    canonical: String,
}

impl CanonicalLocalSvgSignature {
    pub(crate) const fn decimals(&self) -> u32 {
        self.decimals
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        self.canonical.as_bytes()
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DomComparatorWorkCounts {
    pub(crate) parses: usize,
    pub(crate) signature_builds: usize,
}

#[cfg(test)]
std::thread_local! {
    static DOM_COMPARATOR_WORK_COUNTS: Cell<DomComparatorWorkCounts> = const {
        Cell::new(DomComparatorWorkCounts {
            parses: 0,
            signature_builds: 0,
        })
    };
}

impl DomComparisonProfile {
    pub(crate) const fn from_mode(mode: DomMode) -> Self {
        match mode {
            DomMode::ParityRoot => Self {
                descendants: DomMode::Parity,
                root_contract: true,
                normalize_browser_text_wrapping: false,
                normalize_browser_text_word_boundaries: false,
                normalize_browser_text_length: false,
            },
            descendants => Self {
                descendants,
                root_contract: false,
                normalize_browser_text_wrapping: false,
                normalize_browser_text_word_boundaries: false,
                normalize_browser_text_length: false,
            },
        }
    }

    pub(crate) const fn with_root_contract(descendants: DomMode) -> Self {
        debug_assert!(!matches!(descendants, DomMode::ParityRoot));
        Self {
            descendants,
            root_contract: true,
            normalize_browser_text_wrapping: false,
            normalize_browser_text_word_boundaries: false,
            normalize_browser_text_length: false,
        }
    }

    pub(crate) const fn with_browser_text_wrapping_normalized(mut self) -> Self {
        if !matches!(self.descendants, DomMode::Strict) {
            self.normalize_browser_text_wrapping = true;
        }
        self
    }

    pub(crate) const fn with_browser_text_word_boundaries_normalized(mut self) -> Self {
        if !matches!(self.descendants, DomMode::Strict) {
            self.normalize_browser_text_wrapping = true;
            self.normalize_browser_text_word_boundaries = true;
        }
        self
    }

    pub(crate) const fn with_browser_text_length_normalized(mut self) -> Self {
        if !matches!(self.descendants, DomMode::Strict) {
            self.normalize_browser_text_length = true;
        }
        self
    }

    pub(crate) const fn descendants(self) -> DomMode {
        self.descendants
    }

    pub(crate) const fn validates_root_contract(self) -> bool {
        self.root_contract
    }

    pub(crate) const fn normalizes_browser_text_wrapping(self) -> bool {
        self.normalize_browser_text_wrapping
    }

    pub(crate) const fn normalizes_browser_text_length(self) -> bool {
        self.normalize_browser_text_length
    }

    const fn signature_key(self, decimals: u32) -> DomSignatureKey {
        DomSignatureKey {
            descendants: self.descendants,
            decimals,
            normalize_browser_text_wrapping: self.normalize_browser_text_wrapping,
            normalize_browser_text_word_boundaries: self.normalize_browser_text_word_boundaries,
            normalize_browser_text_length: self.normalize_browser_text_length,
        }
    }
}

impl<'input> ParsedSvgDom<'input> {
    /// Parse an SVG that has already passed through [`normalize_xml_entities`].
    ///
    /// Keeping normalization ownership at the call site lets this document borrow either the
    /// original SVG or the normalized allocation without a self-referential container.
    pub(crate) fn parse_normalized(svg: &'input str) -> Result<Self, String> {
        #[cfg(test)]
        DOM_COMPARATOR_WORK_COUNTS.with(|counts| {
            let mut next = counts.get();
            next.parses += 1;
            counts.set(next);
        });

        let document = roxmltree::Document::parse(svg).map_err(|error| error.to_string())?;
        if !document.descendants().any(|node| node.has_tag_name("svg")) {
            return Err("missing <svg> root".to_string());
        }
        Ok(Self {
            document,
            signatures: BTreeMap::new(),
        })
    }

    pub(crate) fn root_element(&self) -> roxmltree::Node<'_, '_> {
        self.document.root_element()
    }

    pub(crate) fn svg_root(&self) -> roxmltree::Node<'_, '_> {
        self.document
            .descendants()
            .find(|node| node.has_tag_name("svg"))
            .expect("parsed SVG document invariant")
    }

    pub(crate) fn signature_for_comparison(
        &mut self,
        profile: DomComparisonProfile,
        decimals: u32,
    ) -> &SvgDomNode {
        self.signature(profile.signature_key(decimals))
    }

    fn signature_for_mode(&mut self, mode: DomMode, decimals: u32) -> &SvgDomNode {
        self.signature(DomSignatureKey {
            descendants: mode,
            decimals,
            normalize_browser_text_wrapping: false,
            normalize_browser_text_word_boundaries: false,
            normalize_browser_text_length: false,
        })
    }

    fn signature(&mut self, key: DomSignatureKey) -> &SvgDomNode {
        let document = &self.document;
        match self.signatures.entry(key) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                #[cfg(test)]
                DOM_COMPARATOR_WORK_COUNTS.with(|counts| {
                    let mut next = counts.get();
                    next.signature_builds += 1;
                    counts.set(next);
                });

                let root = document
                    .descendants()
                    .find(|node| node.has_tag_name("svg"))
                    .expect("parsed SVG document invariant");
                let eventmodeling_root_fill = eventmodeling_root_fill_value(root);
                let mut signature = build_node(
                    root,
                    key.descendants,
                    key.decimals,
                    key.normalize_browser_text_wrapping,
                    eventmodeling_root_fill.as_deref(),
                );
                if key.normalize_browser_text_wrapping {
                    normalize_browser_text_wrapping(
                        &mut signature,
                        key.normalize_browser_text_word_boundaries,
                    );
                }
                if key.normalize_browser_text_length {
                    normalize_browser_text_length(&mut signature);
                }
                entry.insert(signature)
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn reset_dom_comparator_work_counts() {
    DOM_COMPARATOR_WORK_COUNTS.with(|counts| counts.set(DomComparatorWorkCounts::default()));
}

#[cfg(test)]
pub(crate) fn dom_comparator_work_counts() -> DomComparatorWorkCounts {
    DOM_COMPARATOR_WORK_COUNTS.with(Cell::get)
}

impl DomMode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Structure => "structure",
            Self::Parity => "parity",
            Self::ParityRoot => "parity-root",
        }
    }
}

impl std::str::FromStr for DomMode {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw.trim() {
            "strict" => Ok(Self::Strict),
            "structure" => Ok(Self::Structure),
            "parity" => Ok(Self::Parity),
            "parity-root" | "parity_root" => Ok(Self::ParityRoot),
            other => Err(format!(
                "unknown DOM mode {other:?}; expected strict, structure, parity, or parity-root"
            )),
        }
    }
}

impl std::fmt::Display for DomMode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
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

fn extract_style_prop_value(style: &str, prop_name: &str) -> Option<String> {
    let target = prop_name.trim().to_ascii_lowercase();
    if target.is_empty() {
        return None;
    }

    for decl in style.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        let (k, v) = decl.split_once(':')?;
        if k.trim().to_ascii_lowercase() != target {
            continue;
        }
        let v = v.trim();
        if v.is_empty() {
            return None;
        }
        return Some(v.to_string());
    }
    None
}

/// Returns the fill from Event Modeling's exact root rule, when the stylesheet proves one.
///
/// Event Modeling's upstream renderer keeps swimlane text color in the diagram root rule while
/// Merman emits the same effective value directly on each text terminal. This deliberately narrow
/// lookup aligns those two representations without turning the comparator into a CSS engine.
fn eventmodeling_root_fill_value(root: roxmltree::Node<'_, '_>) -> Option<String> {
    if root.attribute("aria-roledescription") != Some("eventmodeling") {
        return None;
    }
    let diagram_id = root.attribute("id")?;
    let selector = format!("#{diagram_id}");
    let mut fill = None;

    for style in root
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "style")
    {
        let css = style.text()?;
        let mut input = cssparser::ParserInput::new(css);
        let mut parser = cssparser::Parser::new(&mut input);
        while !parser.is_exhausted() {
            let start = parser.position();
            let selectors_end = loop {
                let position = parser.position();
                if matches!(parser.next().ok()?, cssparser::Token::CurlyBracketBlock) {
                    break position;
                }
            };
            let selectors = parser.slice(start..selectors_end).trim();
            let declarations = parser
                .parse_nested_block(|body| {
                    let start = body.position();
                    while body.next().is_ok() {}
                    Ok::<_, cssparser::ParseError<'_, ()>>(body.slice_from(start))
                })
                .ok()?;
            // These pinned upstream rules only animate stroke offset. Unknown nested rules fail
            // closed; their inner selectors must never be reinterpreted as unconditional rules.
            if matches!(
                (selectors, declarations.trim()),
                (
                    "@keyframes edge-animation-frame",
                    "from{stroke-dashoffset:0;}"
                ) | ("@keyframes dash", "to{stroke-dashoffset:0;}")
            ) {
                continue;
            }
            if selectors.starts_with('@') || declarations.contains('{') {
                return None;
            }
            let mut rule_fill = None;
            eventmodeling_collect_unique_fill(declarations, &mut rule_fill)?;
            if rule_fill.is_none() {
                continue;
            }
            let mut matches_root = false;
            for candidate in selectors.split(',').map(str::trim) {
                if candidate == selector {
                    matches_root = true;
                } else if !eventmodeling_selector_excludes_swimlane_inheritance(root, candidate) {
                    return None;
                }
            }
            if matches_root {
                eventmodeling_collect_unique_fill(declarations, &mut fill)?;
            }
        }
    }
    fill
}

// Only prove exclusion for descendant selectors ending in a class requirement. Pseudo-classes,
// functions, combinators and other unknown syntax fail closed without resolving their semantics.
fn eventmodeling_selector_excludes_swimlane_inheritance(
    root: roxmltree::Node<'_, '_>,
    selector: &str,
) -> bool {
    let mut input = cssparser::ParserInput::new(selector);
    let mut parser = cssparser::Parser::new(&mut input);
    while !parser.is_exhausted() {
        if let Ok(class) = parser.try_parse(|input| {
            input.expect_delim('.')?;
            let class = input.expect_ident_cloned()?;
            input.expect_exhausted()?;
            Ok::<_, cssparser::BasicParseError<'_>>(class)
        }) {
            return root
                .descendants()
                .filter(|node| is_eventmodeling_swimlane_text(*node))
                .all(|node| {
                    node.ancestors()
                        .all(|ancestor| !has_class_token(ancestor, &class))
                });
        }
        if !matches!(
            parser.next(),
            Ok(cssparser::Token::IDHash(_)
                | cssparser::Token::Ident(_)
                | cssparser::Token::Delim('.')
                | cssparser::Token::SquareBracketBlock)
        ) {
            return false;
        }
    }
    false
}

// Only enumerate declarations: a competing fill or an `all` reset makes inheritance unproven.
// CSS tokenization keeps quoted semicolons and escaped property names out of the decision logic.
fn eventmodeling_collect_unique_fill(style: &str, fill: &mut Option<String>) -> Option<()> {
    let mut input = cssparser::ParserInput::new(style);
    let mut parser = cssparser::Parser::new(&mut input);
    while !parser.is_exhausted() {
        if parser.try_parse(|input| input.expect_semicolon()).is_ok() {
            continue;
        }
        parser
            .parse_until_after(cssparser::Delimiter::Semicolon, |declaration| {
                let property = declaration.expect_ident_cloned()?;
                declaration.expect_colon()?;
                let start = declaration.position();
                while declaration.next_including_whitespace_and_comments().is_ok() {}
                let value = declaration.slice_from(start).trim();
                if value.is_empty() || property.eq_ignore_ascii_case("all") {
                    return Err(declaration.new_custom_error(()));
                }
                if property.eq_ignore_ascii_case("fill") {
                    if fill.is_some() {
                        return Err(declaration.new_custom_error(()));
                    }
                    *fill = Some(value.to_owned());
                }
                Ok::<_, cssparser::ParseError<'_, ()>>(())
            })
            .ok()?;
    }
    Some(())
}

fn eventmodeling_has_unoverridden_inheritance(node: roxmltree::Node<'_, '_>) -> bool {
    for ancestor in node.ancestors().filter(|node| node.is_element()) {
        if ancestor.attribute("fill").is_some() {
            return false;
        }
        if let Some(style) = ancestor.attribute("style") {
            let mut fill = None;
            if eventmodeling_collect_unique_fill(style, &mut fill).is_none() || fill.is_some() {
                return false;
            }
        }
        if ancestor.tag_name().name() == "svg" {
            return true;
        }
    }
    false
}

fn normalize_style_font_size_for_parity(style: &str, decimals: u32) -> Option<String> {
    let v = extract_style_prop_value(style, "font-size")?;
    let v = normalize_numeric_tokens(&v, decimals);
    Some(format!("font-size:{v}"))
}

fn normalize_svg_root_style_parity_root(style: &str, decimals: u32) -> String {
    let re = {
        static ONCE: OnceLock<Regex> = OnceLock::new();
        ONCE.get_or_init(|| {
            Regex::new(r#"max-width:\s*(-?(?:\d+\.\d+|\d+\.|\.\d+|\d+)(?:[eE][+-]?\d+)?)px"#)
                .unwrap()
        })
    };

    re.replace_all(style, |caps: &regex::Captures<'_>| {
        let raw = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
        format!("max-width: {}px", normalize_numeric_tokens(raw, decimals))
    })
    .to_string()
}

fn normalize_svg_root_viewbox_parity_root(view_box: &str, decimals: u32) -> String {
    let parts: Vec<&str> = view_box
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|t| !t.is_empty())
        .collect();
    if parts.len() != 4 {
        return normalize_numeric_tokens(view_box, decimals);
    }

    let mut out_parts = Vec::with_capacity(4);
    for part in parts {
        if part.parse::<f64>().is_err() {
            return normalize_numeric_tokens(view_box, decimals);
        }
        out_parts.push(normalize_numeric_tokens(part, decimals));
    }

    out_parts.join(" ")
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

fn normalize_transform_attr(s: &str) -> String {
    // SVG transform lists allow arbitrary whitespace and optional commas between numeric
    // arguments. For DOM comparisons we treat whitespace around commas as non-semantic.
    //
    // Examples that should compare equal:
    // - `translate(1,2)` vs `translate(1, 2)`
    // - `translate(1 ,  2)` vs `translate(1,2)`
    // - `matrix(1 0 0 1 0 0)` (whitespace separators must be preserved, just normalized)
    let mut out = String::with_capacity(s.len());
    let mut pending_space = false;
    let mut it = s.chars().peekable();
    while let Some(ch) = it.next() {
        if ch.is_whitespace() {
            pending_space = true;
            continue;
        }
        if ch == '(' {
            if out.ends_with(' ') {
                out.pop();
            }
            out.push('(');
            pending_space = false;
            while it.peek().is_some_and(|c| c.is_whitespace()) {
                it.next();
            }
            continue;
        }
        if ch == ')' {
            if out.ends_with(' ') {
                out.pop();
            }
            out.push(')');
            pending_space = false;
            continue;
        }
        if ch == ',' {
            if out.ends_with(' ') {
                out.pop();
            }
            out.push(',');
            pending_space = false;
            while it.peek().is_some_and(|c| c.is_whitespace()) {
                it.next();
            }
            continue;
        }
        if pending_space {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
            pending_space = false;
        }
        out.push(ch);
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

fn re_class_edge_note_id() -> &'static Regex {
    static ONCE: OnceLock<Regex> = OnceLock::new();
    ONCE.get_or_init(|| Regex::new(r"\bedgeNote\d+\b").unwrap())
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

fn normalize_class_edge_note_ids(s: &str) -> String {
    re_class_edge_note_id()
        .replace_all(s, "edgeNote<n>")
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

fn flowchart_svg_root<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
) -> Option<roxmltree::Node<'a, 'input>> {
    fn is_flowchart_root(candidate: roxmltree::Node<'_, '_>) -> bool {
        candidate.is_element()
            && candidate.tag_name().name() == "svg"
            && (candidate
                .attribute("aria-roledescription")
                .is_some_and(|value| value.starts_with("flowchart"))
                || candidate.attribute("class").is_some_and(|class| {
                    class.split_whitespace().any(|token| token == "flowchart")
                }))
    }

    is_flowchart_root(node).then_some(node).or_else(|| {
        node.ancestors()
            .find(|&ancestor| is_flowchart_root(ancestor))
    })
}

fn is_flowchart_diagram(node: roxmltree::Node<'_, '_>) -> bool {
    flowchart_svg_root(node).is_some()
}

fn has_class_token(node: roxmltree::Node<'_, '_>, token: &str) -> bool {
    node.attribute("class")
        .is_some_and(|class| class.split_whitespace().any(|value| value == token))
}

fn is_flowchart_node_shell(node: roxmltree::Node<'_, '_>) -> bool {
    is_flowchart_diagram(node)
        && node.tag_name().name() == "g"
        && ["node", "rough-node", "icon-shape", "image-shape"]
            .into_iter()
            .any(|class| has_class_token(node, class))
}

fn is_flowchart_cluster(node: roxmltree::Node<'_, '_>) -> bool {
    is_flowchart_diagram(node) && node.tag_name().name() == "g" && has_class_token(node, "cluster")
}

fn is_eventmodeling_swimlane_text(node: roxmltree::Node<'_, '_>) -> bool {
    if !(node.tag_name().name() == "text"
        && node.parent().is_some_and(|parent| {
            parent.is_element()
                && parent.tag_name().name() == "g"
                && parent
                    .attribute("class")
                    .is_some_and(|class| has_class_token_value(class, "em-swimlane"))
        }))
    {
        return false;
    }

    node.ancestors().any(|ancestor| {
        ancestor.is_element()
            && ancestor.tag_name().name() == "svg"
            && ancestor.attribute("aria-roledescription") == Some("eventmodeling")
    })
}

fn is_ishikawa_text_bbox_attr(node: roxmltree::Node<'_, '_>, key: &str) -> bool {
    key == "data-merman-text-bbox"
        && node.is_element()
        && node.tag_name().name() == "text"
        && node.ancestors().any(|ancestor| {
            ancestor.is_element()
                && ancestor.tag_name().name() == "svg"
                && ancestor.attribute("aria-roledescription") == Some("ishikawa")
        })
}

fn is_tree_view_renderer_paint_marker_id(
    node: roxmltree::Node<'_, '_>,
    key: &str,
    value: &str,
) -> bool {
    key == "id"
        && node.is_element()
        && node.tag_name().name() == "line"
        && has_class_token(node, "treeView-node-line")
        && value.starts_with("treeView-edge-")
        && value.ends_with("-merman-fill-stroke-paint")
        && node.ancestors().any(|ancestor| {
            ancestor.is_element()
                && ancestor.tag_name().name() == "svg"
                && ancestor.attribute("aria-roledescription") == Some("treeView")
        })
}

fn has_class_token_value(class: &str, token: &str) -> bool {
    class.split_whitespace().any(|value| value == token)
}

fn root_layout_group_rank(parent: roxmltree::Node<'_, '_>, child: &SvgDomNode) -> u8 {
    let is_root = is_flowchart_diagram(parent)
        && parent.tag_name().name() == "g"
        && parent
            .attribute("class")
            .is_some_and(|class| has_class_token_value(class, "root"));
    if !is_root {
        return 4;
    }

    let Some(class) = child.attrs.get("class") else {
        return 4;
    };
    if has_class_token_value(class, "clusters") {
        0
    } else if has_class_token_value(class, "edgePaths") || has_class_token_value(class, "edgePath")
    {
        1
    } else if has_class_token_value(class, "edgeLabels") {
        2
    } else if has_class_token_value(class, "nodes") {
        3
    } else {
        4
    }
}

fn flowchart_cluster_child_rank(parent: roxmltree::Node<'_, '_>, child: &SvgDomNode) -> u8 {
    if !is_flowchart_cluster(parent) {
        return 2;
    }

    if child.name == "g"
        && child
            .attrs
            .get("class")
            .is_none_or(|class| class.is_empty())
    {
        return 0;
    }
    if child.name == "g"
        && child
            .attrs
            .get("class")
            .is_some_and(|class| has_class_token_value(class, "cluster-label"))
    {
        return 1;
    }
    2
}

fn flowchart_nodes_child_rank(parent: roxmltree::Node<'_, '_>, child: &SvgDomNode) -> u8 {
    if !(is_flowchart_diagram(parent)
        && parent.tag_name().name() == "g"
        && parent
            .attribute("class")
            .is_some_and(|class| has_class_token_value(class, "nodes")))
    {
        return 2;
    }

    let class = child.attrs.get("class").map(String::as_str).unwrap_or("");
    if ["node", "rough-node", "icon-shape", "image-shape"]
        .into_iter()
        .any(|token| has_class_token_value(class, token))
    {
        return 0;
    }
    if has_class_token_value(class, "edgeLabel") {
        return 1;
    }
    2
}

fn is_non_elk_flowchart_nodes_group(node: roxmltree::Node<'_, '_>) -> bool {
    is_flowchart_diagram(node)
        && flowchart_svg_root(node)
            .is_some_and(|root| root.attribute("aria-roledescription") != Some("flowchart-elk"))
        && node.tag_name().name() == "g"
        && node
            .attribute("class")
            .is_some_and(|class| has_class_token_value(class, "nodes"))
}

fn flowchart_descendant_text(node: roxmltree::Node<'_, '_>) -> String {
    let mut text = String::new();
    for segment in node
        .descendants()
        .filter(|candidate| candidate.is_text())
        .filter_map(|candidate| candidate.text())
    {
        for word in segment.split_whitespace() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(word);
        }
    }
    text
}

fn flowchart_role_label_text(node: roxmltree::Node<'_, '_>, role_class: &str) -> Option<String> {
    node.descendants()
        .find(|candidate| {
            candidate.is_element()
                && candidate.tag_name().name() == "g"
                && has_class_token(*candidate, role_class)
        })
        .map(flowchart_descendant_text)
        .filter(|text| !text.is_empty())
}

fn flowchart_role_label_matches(
    node: roxmltree::Node<'_, '_>,
    role_class: &str,
    expected: &str,
) -> bool {
    flowchart_role_label_text(node, role_class).is_some_and(|text| text == expected)
}

fn flowchart_compound_subgraph_node_semantic_id<'a>(
    node: roxmltree::Node<'_, '_>,
    value: &'a str,
) -> Option<&'a str> {
    // Mermaid v2 uses a directly scoped ID for an external node only when the containing `nodes`
    // group also owns a nested subgraph root. The visible label closes that scoped ID back to the
    // author semantic; ordinary authored node IDs remain untouched.
    let root = flowchart_svg_root(node)?;
    let role = root.attribute("aria-roledescription")?;
    if !role.starts_with("flowchart") || role == "flowchart-elk" {
        return None;
    }
    if node.attribute("data-et").is_some() || node.attribute("data-look") != Some("classic") {
        return None;
    }

    let nodes_group = node.parent().filter(|parent| {
        parent.is_element() && parent.tag_name().name() == "g" && has_class_token(*parent, "nodes")
    })?;
    let contains_subgraph_root = nodes_group.children().any(|candidate| {
        candidate.is_element()
            && candidate.tag_name().name() == "g"
            && has_class_token(candidate, "root")
            && candidate.descendants().any(is_flowchart_cluster)
    });
    if !contains_subgraph_root {
        return None;
    }

    let diagram_id = root.attribute("id")?;
    let semantic_id = value.strip_prefix(diagram_id)?.strip_prefix('-')?;
    if semantic_id.is_empty()
        || semantic_id.chars().any(char::is_whitespace)
        || !flowchart_role_label_matches(node, "label", semantic_id)
    {
        return None;
    }
    Some(semantic_id)
}

fn flowchart_edge_endpoint_pair(value: &str) -> Option<(&str, &str)> {
    let value = value.strip_prefix("L_")?;
    let (endpoints, ordinal) = value.rsplit_once('_')?;
    if ordinal.is_empty() || !ordinal.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let mut parts = endpoints.split('_');
    let source = parts.next()?.trim();
    let target = parts.next()?.trim();
    if parts.next().is_some()
        || source.is_empty()
        || target.is_empty()
        || source.chars().any(char::is_whitespace)
        || target.chars().any(char::is_whitespace)
    {
        return None;
    }
    Some((source, target))
}

fn flowchart_has_unique_node_terminal(node: roxmltree::Node<'_, '_>, semantic_id: &str) -> bool {
    let Some(root) = flowchart_svg_root(node) else {
        return false;
    };
    let expected_id = format!("node-{semantic_id}");
    let mut matches = root.descendants().filter(|candidate| {
        if !is_flowchart_node_shell(*candidate)
            || candidate
                .attribute("data-et")
                .is_some_and(|kind| kind != "node")
        {
            return false;
        }
        candidate.attribute("data-id") == Some(semantic_id)
            || candidate
                .attribute("id")
                .is_some_and(|value| canonical_flowchart_fragment(*candidate, value) == expected_id)
    });
    matches.next().is_some() && matches.next().is_none()
}

fn flowchart_has_unique_edge_label(node: roxmltree::Node<'_, '_>, semantic_id: &str) -> bool {
    let Some(edge_paths) = node.parent().filter(|parent| {
        parent.is_element()
            && parent.tag_name().name() == "g"
            && has_class_token(*parent, "edgePaths")
    }) else {
        return false;
    };
    let Some(graph_root) = edge_paths.parent().filter(|parent| {
        parent.is_element() && parent.tag_name().name() == "g" && has_class_token(*parent, "root")
    }) else {
        return false;
    };

    let mut matches = graph_root
        .children()
        .filter(|candidate| {
            candidate.is_element()
                && candidate.tag_name().name() == "g"
                && has_class_token(*candidate, "edgeLabels")
        })
        .flat_map(|labels| labels.descendants())
        .filter(|candidate| {
            candidate.is_element()
                && candidate.tag_name().name() == "g"
                && has_class_token(*candidate, "label")
                && candidate.attribute("data-id") == Some(semantic_id)
        });
    matches.next().is_some() && matches.next().is_none()
}

fn is_flowchart_closed_invisible_edge(node: roxmltree::Node<'_, '_>) -> bool {
    // Invisible links omit the normal `flowchart-link` class. Admit their producer ID only after
    // the semantic edge ID, sibling edge label, and unique source/target terminals close the same
    // route; generated ordinals alone are not evidence.
    if !(is_flowchart_diagram(node)
        && node.tag_name().name() == "path"
        && has_class_token(node, "edge-thickness-invisible")
        && has_class_token(node, "edge-pattern-solid")
        && node.attribute("data-edge") == Some("true")
        && node.attribute("data-et") == Some("edge")
        && node.attribute("data-look") == Some("classic"))
    {
        return false;
    }
    let Some(root) = flowchart_svg_root(node) else {
        return false;
    };
    if root.attribute("aria-roledescription") != Some("flowchart-v2") {
        return false;
    }
    let Some(semantic_id) = node
        .attribute("data-id")
        .filter(|value| !value.is_empty() && flowchart_edge_endpoint_pair(value).is_some())
    else {
        return false;
    };
    let Some(id) = node.attribute("id") else {
        return false;
    };
    let producer_id = strip_flowchart_document_scope(node, id);
    if producer_id != semantic_id
        && !producer_id.strip_prefix("edge-").is_some_and(|ordinal| {
            !ordinal.is_empty() && ordinal.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return false;
    }
    let Some((source, target)) = flowchart_edge_endpoint_pair(semantic_id) else {
        return false;
    };
    flowchart_has_unique_edge_label(node, semantic_id)
        && flowchart_has_unique_node_terminal(node, source)
        && flowchart_has_unique_node_terminal(node, target)
}

fn is_flowchart_edge(node: roxmltree::Node<'_, '_>) -> bool {
    is_flowchart_diagram(node)
        && node.tag_name().name() == "path"
        && (has_class_token(node, "flowchart-link") || is_flowchart_closed_invisible_edge(node))
}

fn is_flowchart_self_loop_label_id(value: &str) -> bool {
    let Some((endpoints, ordinal)) = value.rsplit_once("---") else {
        return false;
    };
    let Some((source, target)) = endpoints.split_once("---") else {
        return false;
    };
    !source.is_empty()
        && source == target
        && !source.chars().any(char::is_whitespace)
        && !ordinal.is_empty()
        && ordinal.bytes().all(|byte| byte.is_ascii_digit())
}

fn flowchart_synthetic_label_semantic_id<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
) -> Option<&'a str> {
    if !(is_flowchart_diagram(node)
        && node.tag_name().name() == "g"
        && has_class_token(node, "label")
        && has_class_token(node, "edgeLabel"))
    {
        return None;
    }

    fn upstream_edge_label_semantic_id(value: &str) -> Option<&str> {
        let offset = value.rfind("-L_")?.saturating_add(1);
        let semantic_id = value.get(offset..)?;
        flowchart_edge_endpoint_pair(semantic_id).map(|_| semantic_id)
    }

    match (node.attribute("data-et"), node.attribute("data-id")) {
        (None, None) => node.attribute("id").and_then(|value| {
            upstream_edge_label_semantic_id(value)
                .or_else(|| is_flowchart_self_loop_label_id(value).then_some(value))
        }),
        (Some("edge-label"), Some(data_id))
            if flowchart_edge_endpoint_pair(data_id).is_some()
                || is_flowchart_self_loop_label_id(data_id) =>
        {
            let generated_id = strip_flowchart_document_scope(node, node.attribute("id")?);
            generated_id
                .strip_prefix("synthetic-label-")
                .is_some_and(|ordinal| {
                    !ordinal.is_empty() && ordinal.bytes().all(|byte| byte.is_ascii_digit())
                })
                .then_some(data_id)
        }
        _ => None,
    }
}

fn is_flowchart_synthetic_label(node: roxmltree::Node<'_, '_>) -> bool {
    flowchart_synthetic_label_semantic_id(node).is_some()
}

fn is_flowchart_typed_synthetic_label(node: roxmltree::Node<'_, '_>) -> bool {
    node.attribute("data-et") == Some("edge-label")
        && flowchart_synthetic_label_semantic_id(node).is_some()
}

fn has_flowchart_typed_duplicate_identity(node: roxmltree::Node<'_, '_>) -> bool {
    if is_flowchart_typed_synthetic_label(node) {
        return true;
    }
    if !node
        .attribute("data-id")
        .is_some_and(|value| !value.is_empty())
    {
        return false;
    }
    match node.attribute("data-et") {
        Some("node") => is_flowchart_node_shell(node),
        Some("cluster") => is_flowchart_cluster(node),
        _ => false,
    }
}

fn is_flowchart_semantic_element(node: roxmltree::Node<'_, '_>) -> bool {
    is_flowchart_node_shell(node)
        || is_flowchart_cluster(node)
        || is_flowchart_edge(node)
        || is_flowchart_synthetic_label(node)
        || (is_flowchart_diagram(node)
            && node.tag_name().name() == "g"
            && has_class_token(node, "edgeLabel"))
}

fn is_flowchart_nested_edge_label_data_id(node: roxmltree::Node<'_, '_>) -> bool {
    is_flowchart_diagram(node)
        && node.tag_name().name() == "g"
        && has_class_token(node, "label")
        && node.attribute("data-id").is_some()
        && node.ancestors().any(|ancestor| {
            ancestor.is_element()
                && (ancestor
                    .attribute("data-et")
                    .is_some_and(|kind| kind == "edge-label")
                    || has_class_token(ancestor, "edgeLabel"))
        })
}

fn flowchart_diagram_id<'a, 'input>(node: roxmltree::Node<'a, 'input>) -> Option<&'a str> {
    flowchart_svg_root(node)?.attribute("id")
}

fn strip_flowchart_scope<'a>(node: roxmltree::Node<'_, '_>, value: &'a str) -> &'a str {
    let Some(diagram_id) = flowchart_diagram_id(node) else {
        return value;
    };
    value
        .strip_prefix(diagram_id)
        .and_then(|suffix| {
            suffix
                .strip_prefix('-')
                .or_else(|| suffix.strip_prefix('_'))
        })
        .unwrap_or(value)
}

fn strip_flowchart_document_scope<'a>(node: roxmltree::Node<'_, '_>, value: &'a str) -> &'a str {
    let value = strip_flowchart_scope(node, value);
    value
        .strip_prefix("merman-flowchart-document-")
        .or_else(|| value.strip_prefix("merman-flowchart-document_"))
        .unwrap_or(value)
}

fn canonical_flowchart_fragment(node: roxmltree::Node<'_, '_>, value: &str) -> String {
    let value = strip_flowchart_document_scope(node, value);

    if matches!(value, "drop-shadow" | "drop-shadow-small") {
        return format!("filter-{value}");
    }

    if let Some(raw) = value.strip_prefix("flowchart-")
        && let Some((node_id, ordinal)) = raw.rsplit_once('-')
        && !node_id.is_empty()
        && !ordinal.is_empty()
        && ordinal.bytes().all(|byte| byte.is_ascii_digit())
    {
        return format!("node-{node_id}");
    }

    if value.starts_with("L_") {
        return format!("edge-{value}");
    }
    if value.starts_with("edge-")
        || value.starts_with("node-")
        || value.starts_with("cluster-")
        || value.starts_with("filter-")
        || value.starts_with("gradient-")
        || value.starts_with("a11y-")
        || value.starts_with("flowchart-")
    {
        return value.to_string();
    }

    value.to_string()
}

fn is_flowchart_elk_cluster_container(node: roxmltree::Node<'_, '_>) -> bool {
    let Some(root) = flowchart_svg_root(node) else {
        return false;
    };
    if root.attribute("aria-roledescription") != Some("flowchart-elk") {
        return false;
    }

    let Some(subgraph) = node.parent().filter(|parent| {
        parent.is_element()
            && parent.tag_name().name() == "g"
            && has_class_token(*parent, "subgraph")
    }) else {
        return false;
    };
    subgraph.parent().is_some_and(|parent| {
        parent.is_element()
            && parent.tag_name().name() == "g"
            && has_class_token(parent, "subgraphs")
    })
}

fn canonical_flowchart_elk_cluster_identifier(
    node: roxmltree::Node<'_, '_>,
    value: &str,
) -> Option<String> {
    if !is_flowchart_cluster(node) || !is_flowchart_elk_cluster_container(node) {
        return None;
    }
    if node.attribute("data-look") != Some("classic")
        || flowchart_role_label_text(node, "cluster-label").is_none()
    {
        return None;
    }
    // ELK collapses the upstream producer ID to the JavaScript object sentinel. Restrict this to
    // the exact subgraph producer role and keep the independently rendered cluster label in the
    // signature so author semantics remain observable.
    if value == "[object Object]"
        && node.attribute("data-et").is_none()
        && node.attribute("data-id").is_none()
    {
        return Some("cluster-<elk-object-sentinel>".to_string());
    }

    let data_id = node
        .attribute("data-id")
        .filter(|value| !value.is_empty())?;
    if flowchart_role_label_text(node, "cluster-label").as_deref() != Some(data_id) {
        return None;
    }
    let generated_cluster_id = strip_flowchart_document_scope(node, value)
        .strip_prefix("cluster-")
        .is_some_and(|ordinal| {
            !ordinal.is_empty() && ordinal.bytes().all(|byte| byte.is_ascii_digit())
        });
    (node.attribute("data-et") == Some("cluster")
        && !data_id.trim().is_empty()
        && generated_cluster_id)
        .then(|| "cluster-<elk-object-sentinel>".to_string())
}

fn canonical_flowchart_a11y_identifier(
    node: roxmltree::Node<'_, '_>,
    value: &str,
) -> Option<String> {
    let root = flowchart_svg_root(node)?;
    if node.parent() != Some(root) {
        return None;
    }
    let diagram_id = root.attribute("id")?;
    let (reference_attr, upstream_id, local_id, canonical) = match node.tag_name().name() {
        "title" => (
            "aria-labelledby",
            format!("chart-title-{diagram_id}"),
            "a11y-title",
            "a11y-title",
        ),
        "desc" => (
            "aria-describedby",
            format!("chart-desc-{diagram_id}"),
            "a11y-description",
            "a11y-description",
        ),
        _ => return None,
    };
    if root.attribute(reference_attr) != Some(value) {
        return None;
    }

    (value == upstream_id.as_str() || strip_flowchart_document_scope(node, value) == local_id)
        .then(|| canonical.to_string())
}

fn canonical_flowchart_a11y_reference(
    node: roxmltree::Node<'_, '_>,
    key: &str,
    value: &str,
) -> Option<String> {
    let root = flowchart_svg_root(node)?;
    if node != root {
        return None;
    }
    let expected_tag = match key {
        "aria-labelledby" => "title",
        "aria-describedby" => "desc",
        _ => return None,
    };
    let mut targets = root
        .children()
        .filter(|candidate| candidate.is_element() && candidate.attribute("id") == Some(value));
    let target = targets.next()?;
    if targets.next().is_some() || target.tag_name().name() != expected_tag {
        return None;
    }
    canonical_flowchart_a11y_identifier(target, value)
}

fn canonical_flowchart_root_gradient_identifier(
    node: roxmltree::Node<'_, '_>,
    value: &str,
) -> Option<String> {
    let root = flowchart_svg_root(node)?;
    if node.tag_name().name() != "linearGradient" || node.parent() != Some(root) {
        return None;
    }

    matches!(
        strip_flowchart_document_scope(node, value),
        "gradient" | "gradient-root"
    )
    .then(|| "gradient-root".to_string())
}

/// Returns a canonical identity only for a recognized Flowchart producer role.
///
/// `None` is intentional: callers must retain the original value so authored IDs and malformed
/// producer output remain observable.
pub(crate) fn canonical_flowchart_identifier(
    node: roxmltree::Node<'_, '_>,
    key: &str,
    value: &str,
) -> Option<String> {
    if key != "id" || !is_flowchart_diagram(node) {
        return None;
    }

    if let Some(canonical) = canonical_flowchart_a11y_identifier(node, value) {
        return Some(canonical);
    }
    if let Some(canonical) = canonical_flowchart_elk_cluster_identifier(node, value) {
        return Some(canonical);
    }

    if is_flowchart_node_shell(node) && node.attribute("data-et").is_none_or(|kind| kind == "node")
    {
        if let Some(semantic_id) = flowchart_compound_subgraph_node_semantic_id(node, value) {
            return Some(format!("node-{semantic_id}"));
        }
        if node.attribute("data-et") == Some("node")
            && let Some(data_id) = node.attribute("data-id").filter(|value| !value.is_empty())
        {
            return Some(format!("node-{data_id}"));
        }
        let canonical = canonical_flowchart_fragment(node, value);
        if canonical.starts_with("node-") {
            return Some(canonical);
        }
    }

    if is_flowchart_cluster(node)
        && node
            .attribute("data-et")
            .is_none_or(|kind| kind == "cluster")
    {
        if node.attribute("data-et") == Some("cluster")
            && let Some(data_id) = node.attribute("data-id").filter(|value| !value.is_empty())
        {
            return Some(format!("cluster-{data_id}"));
        }
        let fragment = canonical_flowchart_fragment(node, value);
        if !fragment.is_empty() {
            return Some(if fragment.starts_with("cluster-") {
                fragment
            } else {
                format!("cluster-{fragment}")
            });
        }
    }

    if is_flowchart_edge(node) && node.attribute("data-et").is_none_or(|kind| kind == "edge") {
        if node.attribute("data-et") == Some("edge")
            && let Some(data_id) = node.attribute("data-id").filter(|value| !value.is_empty())
        {
            return Some(format!("edge-{data_id}"));
        }
        let canonical = canonical_flowchart_fragment(node, value);
        if canonical.starts_with("edge-") {
            return Some(canonical);
        }
    }

    if is_flowchart_synthetic_label(node) {
        let data_id = flowchart_synthetic_label_semantic_id(node)?;
        return Some(format!("edge-label-{data_id}"));
    }

    match node.tag_name().name() {
        "marker" | "filter" | "clipPath" => Some(canonical_flowchart_fragment(node, value)),
        "linearGradient" => canonical_flowchart_root_gradient_identifier(node, value),
        _ => None,
    }
}

/// Canonicalizes a closed reference to the expected producer-owned resource kind.
///
/// Missing, ambiguous, wrong-kind, and unrecognized targets retain their original reference.
pub(crate) fn canonical_flowchart_reference(
    node: roxmltree::Node<'_, '_>,
    key: &str,
    value: &str,
) -> String {
    let expected_target = match key {
        "marker-start" | "marker-mid" | "marker-end" => "marker",
        "clip-path" => "clipPath",
        "filter" => "filter",
        "fill" | "stroke" => "linearGradient",
        _ => return value.to_string(),
    };
    let Some(fragment) = value
        .strip_prefix("url(#")
        .and_then(|value| value.strip_suffix(')'))
    else {
        return value.to_string();
    };
    let Some(root) = flowchart_svg_root(node) else {
        return value.to_string();
    };
    let mut targets = root
        .descendants()
        .filter(|candidate| candidate.is_element() && candidate.attribute("id") == Some(fragment));
    let Some(target) = targets.next() else {
        return value.to_string();
    };
    if targets.next().is_some() || target.tag_name().name() != expected_target {
        return value.to_string();
    }
    let Some(canonical) = canonical_flowchart_identifier(target, "id", fragment) else {
        return value.to_string();
    };
    format!("url(#{canonical})")
}

fn preserves_flowchart_identity_attribute(
    node: roxmltree::Node<'_, '_>,
    key: &str,
    value: &str,
) -> bool {
    if !is_flowchart_diagram(node) {
        return false;
    }
    match key {
        "data-id" => {
            is_flowchart_semantic_element(node) || is_flowchart_nested_edge_label_data_id(node)
        }
        "aria-labelledby" | "aria-describedby" => flowchart_svg_root(node) == Some(node),
        "id" => {
            let fragment = strip_flowchart_document_scope(node, value);
            canonical_flowchart_identifier(node, key, value).is_some()
                || is_flowchart_semantic_element(node)
                || matches!(
                    node.tag_name().name(),
                    "title" | "desc" | "marker" | "filter" | "clipPath" | "linearGradient"
                )
                || fragment.starts_with("flowchart-")
                || fragment.starts_with("L_")
        }
        _ => false,
    }
}

fn build_node(
    n: roxmltree::Node<'_, '_>,
    mode: DomMode,
    decimals: u32,
    preserve_browser_text_rows: bool,
    eventmodeling_root_fill: Option<&str>,
) -> SvgDomNode {
    let mut attrs: BTreeMap<String, String> = BTreeMap::new();

    fn is_block_diagram(n: roxmltree::Node<'_, '_>) -> bool {
        for a in n.ancestors() {
            if a.is_element() && a.tag_name().name() == "svg" {
                return a
                    .attribute("aria-roledescription")
                    .is_some_and(|v| v == "block");
            }
        }
        false
    }

    fn is_architecture_diagram(n: roxmltree::Node<'_, '_>) -> bool {
        for a in n.ancestors() {
            if a.is_element() && a.tag_name().name() == "svg" {
                return a
                    .attribute("aria-roledescription")
                    .is_some_and(|v| v == "architecture");
            }
        }
        false
    }

    fn architecture_diagram_id(n: roxmltree::Node<'_, '_>) -> Option<String> {
        for a in n.ancestors() {
            if a.is_element()
                && a.tag_name().name() == "svg"
                && a.attribute("aria-roledescription")
                    .is_some_and(|v| v == "architecture")
            {
                return a.attribute("id").map(str::to_string);
            }
        }
        None
    }

    fn normalize_architecture_scoped_dom_id(n: roxmltree::Node<'_, '_>, val: &str) -> String {
        let Some(diagram_id) = architecture_diagram_id(n) else {
            return val.to_string();
        };
        for kind in ["service", "node", "group"] {
            let prefix = format!("{diagram_id}-{kind}-");
            if let Some(rest) = val.strip_prefix(&prefix) {
                return format!("{kind}-{rest}");
            }
        }
        val.to_string()
    }

    fn is_sankey_diagram(n: roxmltree::Node<'_, '_>) -> bool {
        for a in n.ancestors() {
            if a.is_element() && a.tag_name().name() == "svg" {
                return a
                    .attribute("aria-roledescription")
                    .is_some_and(|v| v == "sankey");
            }
        }
        false
    }

    fn sankey_diagram_id(n: roxmltree::Node<'_, '_>) -> Option<String> {
        for a in n.ancestors() {
            if a.is_element()
                && a.tag_name().name() == "svg"
                && a.attribute("aria-roledescription")
                    .is_some_and(|v| v == "sankey")
            {
                return a.attribute("id").map(str::to_string);
            }
        }
        None
    }

    fn normalize_sankey_scoped_dom_value(n: roxmltree::Node<'_, '_>, val: &str) -> String {
        let Some(diagram_id) = sankey_diagram_id(n) else {
            return val.to_string();
        };
        let node_prefix = format!("{diagram_id}-node-");
        let gradient_prefix = format!("{diagram_id}-linearGradient-");
        val.replace(&node_prefix, "node-")
            .replace(&gradient_prefix, "linearGradient-")
    }

    fn vertical_timeline_diagram_id(n: roxmltree::Node<'_, '_>) -> Option<String> {
        let svg = n.ancestors().find(|ancestor| {
            ancestor.is_element()
                && ancestor.tag_name().name() == "svg"
                && ancestor
                    .attribute("aria-roledescription")
                    .is_some_and(|value| value == "timeline")
        })?;
        let activity_line = svg.descendants().find(|descendant| {
            descendant.is_element()
                && descendant.tag_name().name() == "line"
                && descendant
                    .parent()
                    .and_then(|parent| parent.attribute("class"))
                    .is_some_and(|class| {
                        class.split_whitespace().any(|token| token == "lineWrapper")
                    })
                && descendant.attribute("stroke-width") == Some("4")
        })?;
        (activity_line.attribute("x1") == activity_line.attribute("x2"))
            .then(|| svg.attribute("id").map(str::to_string))
            .flatten()
    }

    fn normalize_vertical_timeline_broken_node_id(
        n: roxmltree::Node<'_, '_>,
        key: &str,
        val: &str,
    ) -> Option<String> {
        let diagram_id = vertical_timeline_diagram_id(n)?;
        // Marker definitions and references are rendering semantics; only the unreferenced node
        // IDs are safe to align with Merman's diagram-scoped spelling.
        let scoped_node_prefix = format!("{diagram_id}-node-");
        if n.tag_name().name() == "path"
            && key == "id"
            && n.attribute("class").is_some_and(|class| {
                class.split_whitespace().any(|token| token == "node-bkg")
                    && class
                        .split_whitespace()
                        .any(|token| token == "node-undefined")
            })
            && let Some(ordinal) = val
                .strip_prefix("undefined-node-")
                .or_else(|| val.strip_prefix(&scoped_node_prefix))
            && !ordinal.is_empty()
            && ordinal.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Some(format!("<timeline-node-{ordinal}>"));
        }
        None
    }

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

        fn is_class_diagram(n: roxmltree::Node<'_, '_>) -> bool {
            for a in n.ancestors() {
                if a.is_element() && a.tag_name().name() == "svg" {
                    return a
                        .attribute("aria-roledescription")
                        .is_some_and(|v| v == "class")
                        || a.attribute("class")
                            .is_some_and(|c| c.split_whitespace().any(|t| t == "classDiagram"));
                }
            }
            false
        }

        fn is_architecture_icon_content(n: roxmltree::Node<'_, '_>) -> bool {
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
                    && a.attribute("class").is_some_and(|c| {
                        c.split_whitespace().any(|t| {
                            t == "architecture-service"
                                || t == "architecture-groups"
                                || t == "architecture-group"
                        })
                    })
            })
        }

        fn is_flowchart_label_container_path(n: roxmltree::Node<'_, '_>) -> bool {
            n.tag_name().name() == "path"
                && is_flowchart_diagram(n)
                && has_class_token(n, "label-container")
        }

        fn is_class_rough_node_path(n: roxmltree::Node<'_, '_>) -> bool {
            n.tag_name().name() == "path"
                && is_class_diagram(n)
                && n.ancestors().any(|a| {
                    a.is_element() && a.tag_name().name() == "g" && has_class_token(a, "rough-node")
                })
        }

        fn is_flowchart_rough_node_path(n: roxmltree::Node<'_, '_>) -> bool {
            n.tag_name().name() == "path"
                && is_flowchart_diagram(n)
                && n.ancestors().any(|a| {
                    a.is_element() && a.tag_name().name() == "g" && has_class_token(a, "rough-node")
                })
        }

        fn is_flowchart_hand_drawn_cluster_path(n: roxmltree::Node<'_, '_>) -> bool {
            n.tag_name().name() == "path"
                && is_flowchart_diagram(n)
                && n.ancestors().any(|a| {
                    a.is_element()
                        && a.tag_name().name() == "g"
                        && has_class_token(a, "cluster")
                        && a.attribute("data-look").is_some_and(|v| v == "handDrawn")
                })
        }

        fn is_flowchart_renderer_paint_marker_id(n: roxmltree::Node<'_, '_>, value: &str) -> bool {
            is_flowchart_hand_drawn_cluster_path(n)
                && (value.ends_with("-merman-fill-paint")
                    || value.ends_with("-merman-fill-stroke-paint"))
        }

        fn is_architecture_edge_arrow(n: roxmltree::Node<'_, '_>) -> bool {
            if !(n.tag_name().name() == "polygon" && has_class_token(n, "arrow")) {
                return false;
            }

            let mut in_architecture_svg = false;
            let mut in_architecture_edges = false;
            for a in n.ancestors() {
                if !a.is_element() {
                    continue;
                }
                if a.tag_name().name() == "svg"
                    && a.attribute("aria-roledescription")
                        .is_some_and(|v| v == "architecture")
                {
                    in_architecture_svg = true;
                }
                if a.tag_name().name() == "g" && has_class_token(a, "architecture-edges") {
                    in_architecture_edges = true;
                }
            }

            in_architecture_svg && in_architecture_edges
        }

        fn is_architecture_service_node_bkg_path(n: roxmltree::Node<'_, '_>) -> bool {
            n.tag_name().name() == "path"
                && has_class_token(n, "node-bkg")
                && n.ancestors().any(|a| {
                    a.is_element()
                        && a.tag_name().name() == "g"
                        && has_class_token(a, "architecture-service")
                })
        }

        fn is_xychart_bar_data_label_text(n: roxmltree::Node<'_, '_>) -> bool {
            if !(n.is_element() && n.tag_name().name() == "text") {
                return false;
            }
            let mut has_plot = false;
            let mut has_bar_plot = false;
            for a in n.ancestors() {
                if !(a.is_element() && a.tag_name().name() == "g") {
                    continue;
                }
                let Some(class) = a.attribute("class") else {
                    continue;
                };
                for token in class.split_whitespace() {
                    if token == "plot" {
                        has_plot = true;
                    } else if token.starts_with("bar-plot-") {
                        has_bar_plot = true;
                    }
                }
            }
            has_plot && has_bar_plot
        }

        for a in n.attributes() {
            let key = a.name().to_string();
            let mut val = a.value().to_string();

            if mode != DomMode::Strict && is_flowchart_diagram(n) {
                if matches!(key.as_str(), "data-et" | "data-id")
                    && has_flowchart_typed_duplicate_identity(n)
                {
                    // Typed node, cluster, and verified synthetic-label metadata duplicates
                    // the canonical role identity. Edge-path and unrecognized edge-label
                    // metadata remains observable and fails closed.
                    continue;
                }
                if matches!(
                    key.as_str(),
                    "marker-start"
                        | "marker-mid"
                        | "marker-end"
                        | "clip-path"
                        | "filter"
                        | "fill"
                        | "stroke"
                ) {
                    val = canonical_flowchart_reference(n, &key, &val);
                }
                if matches!(key.as_str(), "aria-labelledby" | "aria-describedby")
                    && let Some(canonical) = canonical_flowchart_a11y_reference(n, &key, &val)
                {
                    val = canonical;
                }
                if key == "id"
                    && let Some(normalized) = canonical_flowchart_identifier(n, &key, &val)
                {
                    val = normalized;
                }
            }

            if mode != DomMode::Strict
                && key == "id"
                && is_flowchart_renderer_paint_marker_id(n, &val)
            {
                // Renderer-owned paint markers are an internal evidence channel. Mermaid's
                // reference path has no corresponding id, so keep the marker out of non-strict
                // DOM parity while strict mode still detects its presence exactly.
                continue;
            }

            if mode != DomMode::Strict && is_tree_view_renderer_paint_marker_id(n, &key, &val) {
                // Tree View's line carries a renderer-owned dual-paint marker so native raster
                // evidence can bind both semantic edge facets to the final stroke terminal.
                // Mermaid's reference DOM has no equivalent id; strict mode still compares it.
                continue;
            }

            if mode != DomMode::Strict && is_ishikawa_text_bbox_attr(n, &key) {
                // Ishikawa's renderer-owned text bbox is evidence metadata for the local
                // receipt, not an upstream Mermaid DOM semantic. Keep it in strict signatures
                // and final SVG output, but do not make non-strict upstream DOM parity depend on
                // this private attribute.
                continue;
            }

            if matches!(mode, DomMode::Parity | DomMode::ParityRoot)
                && n.tag_name().name() == "foreignObject"
                && key == "overflow"
                && val.trim().eq_ignore_ascii_case("visible")
            {
                // Merman keeps headless HTML label boxes non-clipping to absorb host font
                // fallback drift. Treat that explicit browser-safety marker as non-semantic in
                // parity DOM gates while preserving it in strict signatures.
                continue;
            }

            if key == "transform" {
                val = normalize_transform_attr(&val);
                if matches!(mode, DomMode::Parity | DomMode::ParityRoot) {
                    let lower = val.to_ascii_lowercase();
                    // Upstream Mermaid can emit invalid transforms for certain edge-label corner
                    // cases (e.g. blank labels): `translate(undefined,NaN)`. Treat these as
                    // non-semantic in parity modes so we don't fail DOM comparisons on upstream
                    // renderer quirks.
                    if lower.contains("undefined") || lower.contains("nan") {
                        continue;
                    }
                }
            }

            if mode == DomMode::Strict && matches!(key.as_str(), "d" | "points") {
                val = normalize_attr_whitespace_strict(&val);
            }

            if matches!(mode, DomMode::Parity | DomMode::ParityRoot)
                && key == "font-size"
                && is_xychart_bar_data_label_text(n)
            {
                val = normalize_numeric_tokens_mode(&val, decimals, DomMode::Structure);
                attrs.insert(key, val);
                continue;
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
                if key == "transform" && is_architecture_edge_arrow(n) {
                    // Mermaid Architecture emits edge arrowheads as translated polygons, so
                    // upstream diagonal arrows can point away from their edge segment. Merman
                    // rotates those standalone polygons from actual routed geometry; parity
                    // modes should treat that as a visual-geometry delta, not a DOM regression.
                    val = "<geom>".to_string();
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
                        } else if key == "d" && is_class_rough_node_path(n) {
                            // Mermaid's hand-drawn class nodes are emitted by RoughJS. The exact
                            // bezier payload changes with rough generation details even when the
                            // semantic node structure and styling match, so keep this scoped to
                            // classDiagram rough-node geometry only.
                            val = "<class-rough-node-geom>".to_string();
                            normalized_geom = true;
                        } else if key == "d" && is_flowchart_rough_node_path(n) {
                            // Mermaid's hand-drawn flowchart nodes are also RoughJS-generated.
                            // Keep parity focused on DOM structure and semantic attributes while
                            // leaving non-flowchart rough paths untouched.
                            val = "<flowchart-rough-node-geom>".to_string();
                            normalized_geom = true;
                        } else if key == "d" && is_flowchart_hand_drawn_cluster_path(n) {
                            // Hand-drawn flowchart subgraphs use RoughJS paths for cluster
                            // backgrounds/borders. Their bezier payload is geometry noise; the
                            // cluster DOM, styling, and labels remain compared.
                            val = "<flowchart-rough-cluster-geom>".to_string();
                            normalized_geom = true;
                        } else if key == "d" && is_architecture_service_node_bkg_path(n) {
                            // Architecture fallback service background paths differ between
                            // historical fixtures and the pinned Mermaid `svgDraw.ts`
                            // spelling. The path is pure geometry; root gates still compare the
                            // rendered viewport separately.
                            val = "<architecture-node-bkg>".to_string();
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
                        } else if key == "d"
                            && n.tag_name().name() == "path"
                            && n.attribute("class").is_some_and(|c| {
                                c.split_whitespace().any(|t| t == "relationshipLine")
                            })
                        {
                            // Mermaid ER relationship routes are layout-engine-dependent (notably
                            // when `layout=elk` is enabled). Treat the `d` payload as geometry
                            // noise in parity mode so comparisons focus on DOM structure and
                            // semantic attributes.
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
                        } else if matches!(mode, DomMode::Parity | DomMode::ParityRoot)
                            && matches!(n.tag_name().name(), "text" | "tspan")
                        {
                            let Some(filtered) =
                                normalize_style_font_size_for_parity(&val, decimals)
                            else {
                                continue;
                            };
                            val = filtered;
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
                DomMode::Structure | DomMode::Parity | DomMode::ParityRoot
            ) && let Some(normalized) = normalize_vertical_timeline_broken_node_id(n, &key, &val)
            {
                val = normalized;
            }
            if matches!(
                mode,
                DomMode::Strict | DomMode::Structure | DomMode::Parity | DomMode::ParityRoot
            ) && key == "id"
                && is_architecture_icon_content(n)
                && (val.starts_with("IconifyId") || val.len() <= 2)
            {
                val = "<icon-id>".to_string();
            }
            if matches!(mode, DomMode::Parity | DomMode::ParityRoot)
                && key == "id"
                && is_architecture_diagram(n)
                && !is_architecture_icon_content(n)
            {
                val = normalize_architecture_scoped_dom_id(n, &val);
            }
            if matches!(
                mode,
                DomMode::Structure | DomMode::Parity | DomMode::ParityRoot
            ) && is_sankey_diagram(n)
            {
                val = normalize_sankey_scoped_dom_value(n, &val);
            }
            if matches!(mode, DomMode::Parity | DomMode::ParityRoot)
                && is_class_diagram(n)
                && matches!(key.as_str(), "id" | "data-id")
            {
                val = normalize_class_edge_note_ids(&val);
            }
            if mode == DomMode::Structure && is_identifier_like_attr(&key) {
                if !preserves_flowchart_identity_attribute(n, &key, &val) {
                    val = normalize_identifier_tokens(&val);
                }
            } else if matches!(mode, DomMode::Parity | DomMode::ParityRoot)
                && is_identifier_like_attr(&key)
            {
                if !preserves_flowchart_identity_attribute(n, &key, &val) {
                    val = normalize_mermaid_generated_id_only(&val);
                }
            } else if mode == DomMode::Strict && is_identifier_like_attr(&key) {
                if is_block_diagram(n) {
                    val = normalize_mermaid_generated_id_only(&val);
                }
                // In strict mode, keep identifier-like attributes byte-for-byte (aside from XML
                // escaping). Applying numeric token normalization here is unsafe, as it can
                // accidentally rewrite IDs like `flowchart-A-0` into `flowchart-A0` by treating
                // `-0` as a number.
            } else if !normalized_geom
                && matches!(mode, DomMode::Parity | DomMode::ParityRoot)
                && (is_geometry_attr(&key)
                    || (key == "label-offset-y" && is_flowchart_label_container_path(n)))
                && !(mode == DomMode::ParityRoot
                    && n.tag_name().name() == "svg"
                    && (key == "width" || key == "height"))
            {
                val = normalize_numeric_tokens_mode(&val, decimals, DomMode::Structure);
            } else {
                val = normalize_numeric_tokens_mode(&val, decimals, mode);
            }

            if key == "style" && mode == DomMode::ParityRoot && n.tag_name().name() == "svg" {
                val = normalize_svg_root_style_parity_root(&val, decimals);
            }
            if key == "viewBox" && mode == DomMode::ParityRoot && n.tag_name().name() == "svg" {
                val = normalize_svg_root_viewbox_parity_root(&val, decimals);
            }

            attrs.insert(key, val);
        }

        if mode != DomMode::Strict
            && is_eventmodeling_swimlane_text(n)
            && !attrs.contains_key("fill")
            && let Some(fill) = eventmodeling_root_fill
            && eventmodeling_has_unoverridden_inheritance(n)
        {
            attrs.insert(
                "fill".to_string(),
                normalize_numeric_tokens_mode(fill, decimals, mode),
            );
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
                    children.push(build_node(
                        c,
                        mode,
                        decimals,
                        preserve_browser_text_rows,
                        eventmodeling_root_fill,
                    ));
                } else if c.is_text()
                    && let Some(t) = c.text().and_then(normalize_text_node_text)
                {
                    children.push(SvgDomNode {
                        name: "#text".to_string(),
                        attrs: BTreeMap::new(),
                        text: Some(t),
                        children: Vec::new(),
                    });
                }
            }
        } else {
            text = n.text().and_then(normalize_text_node_text);
            for c in n.children().filter(|c| c.is_element()) {
                children.push(build_node(
                    c,
                    mode,
                    decimals,
                    preserve_browser_text_rows,
                    eventmodeling_root_fill,
                ));
            }
        }
    } else {
        // Non-strict modes treat text as non-semantic and only track element structure.
        for c in n.children().filter(|c| c.is_element()) {
            children.push(build_node(
                c,
                mode,
                decimals,
                preserve_browser_text_rows,
                eventmodeling_root_fill,
            ));
        }
        if preserve_browser_text_rows
            && n.tag_name().name() == "tspan"
            && n.attribute("class").is_some_and(|class| {
                class
                    .split_whitespace()
                    .any(|token| token == "text-outer-tspan")
            })
        {
            let raw = n
                .descendants()
                .filter(|descendant| descendant.is_text())
                .filter_map(|descendant| descendant.text())
                .collect::<String>();
            text = normalize_text_node_text(&raw);
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

        fn find_first_attr<'a>(n: &'a SvgDomNode, tag: &str, attr: &str) -> Option<&'a str> {
            if n.name == tag
                && let Some(v) = n.attrs.get(attr)
            {
                return Some(v.as_str());
            }
            for c in &n.children {
                if let Some(v) = find_first_attr(c, tag, attr) {
                    return Some(v);
                }
            }
            None
        }

        fn count_tag(n: &SvgDomNode, tag: &str) -> usize {
            let mut out = 0usize;
            if n.name == tag {
                out += 1;
            }
            for c in &n.children {
                out += count_tag(c, tag);
            }
            out
        }

        fn min_tspan_dy(n: &SvgDomNode) -> Option<f64> {
            let mut best: Option<f64> = None;
            if n.name == "tspan"
                && let Some(dy) = n.attrs.get("dy").and_then(|s| s.parse::<f64>().ok())
            {
                best = Some(best.map(|v| v.min(dy)).unwrap_or(dy));
            }
            for c in &n.children {
                if let Some(v) = min_tspan_dy(c) {
                    best = Some(best.map(|b| b.min(v)).unwrap_or(v));
                }
            }
            best
        }

        fn find_first_cluster_id(n: &SvgDomNode) -> Option<&str> {
            if n.name == "g"
                && let Some(class) = n.attrs.get("class")
                && class.split_whitespace().any(is_cluster_class_token)
                && let Some(id) = n.attrs.get("id")
            {
                return Some(id.as_str());
            }
            for c in &n.children {
                if let Some(id) = find_first_cluster_id(c) {
                    return Some(id);
                }
            }
            None
        }

        fn sort_hint(n: &SvgDomNode) -> &str {
            fn find_first_path_id(n: &SvgDomNode) -> Option<&str> {
                if n.name == "path"
                    && let Some(id) = n.attrs.get("id")
                {
                    return Some(id.as_str());
                }
                for c in &n.children {
                    if let Some(id) = find_first_path_id(c) {
                        return Some(id);
                    }
                }
                None
            }

            fn find_first_c4_node_id(n: &SvgDomNode) -> Option<&str> {
                if n.name == "g"
                    && n.attrs
                        .get("class")
                        .is_some_and(|class| class.split_whitespace().any(|token| token == "node"))
                    && let Some(id) = n.attrs.get("id")
                {
                    return Some(id.as_str());
                }
                for c in &n.children {
                    if let Some(id) = find_first_c4_node_id(c) {
                        return Some(id);
                    }
                }
                None
            }

            if let Some(id) = n.attrs.get("id") {
                return id.as_str();
            }
            if let Some(id) = n.attrs.get("data-id") {
                return id.as_str();
            }
            if n.name == "text" {
                for c in &n.children {
                    if c.name != "tspan" {
                        continue;
                    }
                    if let Some(dy) = c.attrs.get("dy") {
                        return dy.as_str();
                    }
                }
            }
            if n.name == "g" {
                // Mermaid often emits anonymous wrapper `<g>` elements (notably in edge groups)
                // without `id`/`class`. In parity modes we sort children to make DOM comparisons
                // deterministic. If we can't derive a stable hint, wrappers can be re-ordered based
                // on incidental content differences (e.g. label wrapping changes the `tspan`
                // structure), causing spurious diffs. Prefer the first descendant edge path id as
                // a stable semantic key.
                if !n.attrs.contains_key("id")
                    && !n.attrs.contains_key("data-id")
                    && let Some(id) = find_first_path_id(n)
                {
                    return id;
                }

                // C4 paints each node inside an anonymous translated wrapper. The wrapper's
                // transform is geometry-dependent, so use the nested node id as its stable
                // semantic key instead of letting browser/font-driven coordinates reorder peers.
                if let Some(id) = find_first_c4_node_id(n) {
                    return id;
                }

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
                    if class.split_whitespace().any(|c| c == "root")
                        && let Some(id) = find_first_cluster_id(n)
                    {
                        return id;
                    }
                    if class.split_whitespace().any(|c| c == "edgeLabel")
                        && let Some(id) = find_first_data_id(n)
                    {
                        return id;
                    }
                }
            }
            ""
        }

        if matches!(mode, DomMode::Parity | DomMode::ParityRoot)
            && n.tag_name().name() == "g"
            && children.iter().any(|c| {
                c.name == "rect"
                    && c.attrs.get("class").is_some_and(|class| {
                        let mut has_actor = false;
                        let mut has_bottom = false;
                        for token in class.split_whitespace() {
                            has_actor |= token == "actor";
                            has_bottom |= token == "actor-bottom";
                        }
                        has_actor && has_bottom
                    })
            })
            && children.iter().any(|c| c.name == "switch")
        {
            // Mermaid Sequence calls the async KaTeX actor-label renderer from a synchronous
            // footer actor path. Exported SVGs can therefore contain missing or half-populated
            // bottom actor `<switch>` labels depending on timing. Treat those footer switches as
            // non-semantic in parity modes; top actor labels still compare normally.
            children.retain(|c| c.name != "switch");
        }

        fn is_ishikawa_nonsemantic_wrapper(n: &SvgDomNode) -> bool {
            if n.name != "g" {
                return false;
            }
            n.attrs.get("class").is_some_and(|class| {
                class.split_whitespace().any(|token| {
                    matches!(
                        token,
                        "ishikawa-pair" | "ishikawa-label-group" | "ishikawa-sub-group"
                    )
                })
            })
        }

        fn flatten_ishikawa_nonsemantic_wrappers(children: &mut Vec<SvgDomNode>) {
            let mut flattened = Vec::with_capacity(children.len());
            for mut child in std::mem::take(children) {
                flatten_ishikawa_nonsemantic_wrappers(&mut child.children);
                if is_ishikawa_nonsemantic_wrapper(&child) {
                    flattened.extend(child.children);
                } else {
                    flattened.push(child);
                }
            }
            *children = flattened;
        }

        if matches!(mode, DomMode::Parity | DomMode::ParityRoot) {
            flatten_ishikawa_nonsemantic_wrappers(&mut children);
        }

        if is_non_elk_flowchart_nodes_group(n) {
            // Dagre Flowchart and Swimlane emit node shells in source order. Keep that order
            // within each semantic role and only move synthetic edge labels after node shells;
            // sorting by generated IDs would otherwise make `answer` precede `request` in
            // Swimlane DOM. ELK intentionally remains in the generic semantic-id order below:
            // its layout adapter owns render order rather than the source sequence.
            children.sort_by_key(|child| flowchart_nodes_child_rank(n, child));
        } else {
            children.sort_by(|a, b| {
                let aclass = a.attrs.get("class").map(|s| s.as_str()).unwrap_or("");
                let bclass = b.attrs.get("class").map(|s| s.as_str()).unwrap_or("");
                let ahint = sort_hint(a);
                let bhint = sort_hint(b);
                let a_root_rank = root_layout_group_rank(n, a);
                let b_root_rank = root_layout_group_rank(n, b);
                let a_cluster_rank = flowchart_cluster_child_rank(n, a);
                let b_cluster_rank = flowchart_cluster_child_rank(n, b);
                let a_nodes_rank = flowchart_nodes_child_rank(n, a);
                let b_nodes_rank = flowchart_nodes_child_rank(n, b);
                let a_is_c4_shape_group =
                    a.name == "g" && aclass.split_whitespace().any(|c| c == "person-man");
                let b_is_c4_shape_group =
                    b.name == "g" && bclass.split_whitespace().any(|c| c == "person-man");
                a_root_rank
                    .cmp(&b_root_rank)
                    .then_with(|| a_cluster_rank.cmp(&b_cluster_rank))
                    .then_with(|| a_nodes_rank.cmp(&b_nodes_rank))
                    .then_with(|| a.name.cmp(&b.name))
                    .then_with(|| ahint.cmp(bhint))
                    .then_with(|| aclass.cmp(bclass))
                    .then_with(|| a.attrs.cmp(&b.attrs))
                    .then_with(|| {
                        if !(a_is_c4_shape_group && b_is_c4_shape_group) {
                            return std::cmp::Ordering::Equal;
                        }
                        let a_fill = find_first_attr(a, "rect", "fill").unwrap_or("");
                        let b_fill = find_first_attr(b, "rect", "fill").unwrap_or("");
                        a_fill.cmp(b_fill)
                    })
                    .then_with(|| {
                        if !(a_is_c4_shape_group && b_is_c4_shape_group) {
                            return std::cmp::Ordering::Equal;
                        }
                        let a_len = find_first_attr(a, "text", "textLength").unwrap_or("");
                        let b_len = find_first_attr(b, "text", "textLength").unwrap_or("");
                        a_len.cmp(b_len)
                    })
                    .then_with(|| {
                        if a.name != "g"
                            || b.name != "g"
                            || !aclass.is_empty()
                            || !bclass.is_empty()
                        {
                            return std::cmp::Ordering::Equal;
                        }
                        let at = count_tag(a, "tspan");
                        let bt = count_tag(b, "tspan");
                        at.cmp(&bt)
                    })
                    .then_with(|| {
                        if a.name != "g"
                            || b.name != "g"
                            || !aclass.is_empty()
                            || !bclass.is_empty()
                        {
                            return std::cmp::Ordering::Equal;
                        }
                        let ady = min_tspan_dy(a).unwrap_or(0.0);
                        let bdy = min_tspan_dy(b).unwrap_or(0.0);
                        ady.partial_cmp(&bdy).unwrap_or(std::cmp::Ordering::Equal)
                    })
            });
        }
    }

    SvgDomNode {
        name: n.tag_name().name().to_string(),
        attrs,
        text,
        children,
    }
}

fn normalize_unclosed_img_tags(s: &str) -> String {
    fn re_unquoted_attr_value() -> &'static Regex {
        static ONCE: OnceLock<Regex> = OnceLock::new();
        // In HTML it is legal to omit quotes for attribute values (e.g. `src=x`), but
        // `roxmltree` parses XML and requires quotes. Normalize the common case we see in
        // Mermaid's `<foreignObject>` XHTML.
        ONCE.get_or_init(|| Regex::new(r#"(\s[\w:-]+)=([^\s"'<>]+)"#).unwrap())
    }

    let mut out = String::with_capacity(s.len());
    let mut idx = 0usize;
    while let Some(rel) = s[idx..].find("<img") {
        let start = idx + rel;
        out.push_str(&s[idx..start]);
        let Some(gt_rel) = s[start..].find('>') else {
            out.push_str(&s[start..]);
            return out;
        };
        let end = start + gt_rel;

        // Include the closing `>` for easier normalization.
        let tag_full = &s[start..=end];
        let mut tag_norm = re_unquoted_attr_value()
            .replace_all(tag_full, r#"$1="$2""#)
            .to_string();

        // Ensure the void tag is self-closed so the surrounding SVG becomes valid XML.
        if tag_norm.ends_with('>') {
            let inner = tag_norm[..tag_norm.len() - 1].trim_end();
            if !inner.ends_with('/') {
                tag_norm.pop();
                tag_norm.push_str("/>");
            }
        }

        out.push_str(&tag_norm);
        idx = end + 1;
    }
    out.push_str(&s[idx..]);
    out
}

pub(crate) fn normalize_xml_entities(svg: &str) -> Cow<'_, str> {
    // Mermaid SVG output (especially `<foreignObject>` XHTML) can contain HTML-ish constructs
    // that are valid in a browser DOM, but not valid XML:
    //
    // - HTML entity references like `&nbsp;` (not predefined in XML), and
    // - void elements like `<img ...>` without a self-closing slash (`/>`).
    //
    // Normalize the most common cases so we can parse and compare DOM trees deterministically.
    // This only affects DOM comparison and report parsing; it does not change SVG rendering.
    if !(svg.contains("&nbsp;")
        || svg.contains("&#160;")
        || svg.contains("&#xA0;")
        || svg.contains("<img"))
    {
        return Cow::Borrowed(svg);
    }

    let mut out = svg.to_string();
    if out.contains("&nbsp;") || out.contains("&#160;") || out.contains("&#xA0;") {
        out = out.replace("&nbsp;", " ");
        out = out.replace("&#160;", " ").replace("&#xA0;", " ");
    }
    if out.contains("<img") {
        out = normalize_unclosed_img_tags(&out);
    }
    Cow::Owned(out)
}

fn normalize_xml_for_local_signature(svg: &str) -> Cow<'_, str> {
    if !(svg.contains("&nbsp;") || svg.contains("<img")) {
        return Cow::Borrowed(svg);
    }

    let mut out = svg.replace("&nbsp;", "&#160;");
    if out.contains("<img") {
        out = normalize_unclosed_img_tags(&out);
    }
    Cow::Owned(out)
}

pub(crate) fn dom_signature(svg: &str, mode: DomMode, decimals: u32) -> Result<SvgDomNode, String> {
    let svg = normalize_xml_entities(svg);
    let mut document = ParsedSvgDom::parse_normalized(svg.as_ref())?;
    Ok(document.signature_for_mode(mode, decimals).clone())
}

fn normalize_browser_text_wrapping(node: &mut SvgDomNode, word_boundaries: bool) {
    for child in &mut node.children {
        normalize_browser_text_wrapping(child, word_boundaries);
    }

    if node.name != "text" || node.children.is_empty() {
        return;
    }

    let is_generated_row = |child: &SvgDomNode| {
        child.name == "tspan"
            && child.attrs.get("class").is_some_and(|class| {
                class
                    .split_whitespace()
                    .any(|token| token == "text-outer-tspan")
            })
    };
    if !node.children.iter().all(is_generated_row) {
        return;
    }

    // Row boundaries are safe to remove only when doing so preserves the exact normalized text.
    // A boundary between `Hello` and `world` cannot prove whether the source contained a space, so
    // whitespace remains semantic and that browser-dependent case stays a bounded residual.
    let row_texts = node
        .children
        .iter()
        .filter_map(|child| child.text.as_deref())
        .collect::<Vec<_>>();
    let text = row_texts.join(if word_boundaries { " " } else { "" });
    node.children = vec![SvgDomNode {
        name: "tspan".to_string(),
        attrs: [("class".to_string(), "text-outer-tspan".to_string())].into(),
        text: (!text.is_empty()).then_some(text),
        children: Vec::new(),
    }];
}

fn normalize_browser_text_length(node: &mut SvgDomNode) {
    for child in &mut node.children {
        normalize_browser_text_length(child);
    }

    if node.name == "text"
        && let Some(value) = node.attrs.get_mut("textLength")
    {
        *value = "<browser-text-length>".to_string();
    }
}

#[cfg(test)]
pub(crate) fn dom_signature_for_comparison(
    svg: &str,
    profile: DomComparisonProfile,
    decimals: u32,
) -> Result<SvgDomNode, String> {
    let svg = normalize_xml_entities(svg);
    let mut document = ParsedSvgDom::parse_normalized(svg.as_ref())?;
    Ok(document.signature_for_comparison(profile, decimals).clone())
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

fn expanded_xml_name(namespace: Option<&str>, local_name: &str) -> String {
    match namespace {
        Some(namespace) => format!("{{{namespace}}}{local_name}"),
        None => local_name.to_string(),
    }
}

fn is_local_signature_quantized_attr(
    node: roxmltree::Node<'_, '_>,
    attribute: roxmltree::Attribute<'_, '_>,
) -> bool {
    // Cross-architecture evidence currently identifies only last-bit drift in generated path
    // coordinates. Keep every other attribute byte-exact and preserve path commands while
    // quantizing their numeric operands to the comparison precision.
    node.tag_name().name() == "path"
        && matches!(
            node.tag_name().namespace(),
            None | Some("http://www.w3.org/2000/svg")
        )
        && attribute.name() == "d"
        && attribute.namespace().is_none()
}

fn write_canonical_local_svg_node(out: &mut String, node: roxmltree::Node<'_, '_>, decimals: u32) {
    debug_assert!(node.is_element());
    let name = expanded_xml_name(node.tag_name().namespace(), node.tag_name().name());
    out.push('<');
    out.push_str(&name);

    let mut attrs = node
        .attributes()
        .map(|attribute| {
            let name = expanded_xml_name(attribute.namespace(), attribute.name());
            let value = if is_local_signature_quantized_attr(node, attribute) {
                normalize_numeric_tokens(attribute.value(), decimals)
            } else {
                attribute.value().to_string()
            };
            (name, value)
        })
        .collect::<Vec<_>>();
    attrs.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    for (name, value) in attrs {
        out.push(' ');
        out.push_str(&name);
        out.push_str("=\"");
        out.push_str(&escape_xml_attr(&value));
        out.push('"');
    }

    let has_content = node
        .children()
        .any(|child| child.is_element() || child.is_text());
    if !has_content {
        out.push_str("/>");
        return;
    }

    out.push('>');
    for child in node.children() {
        if child.is_element() {
            write_canonical_local_svg_node(out, child, decimals);
        } else if child.is_text()
            && let Some(text) = child.text()
        {
            out.push_str(&escape_xml_text(text));
        }
    }
    out.push_str("</");
    out.push_str(&name);
    out.push('>');
}

pub(crate) fn canonical_local_svg_signature(
    svg: &str,
    decimals: u32,
) -> Result<CanonicalLocalSvgSignature, String> {
    let document_start = svg.strip_prefix('\u{feff}').unwrap_or(svg);
    if document_start
        .strip_prefix("<?xml")
        .and_then(|rest| rest.as_bytes().first())
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        return Err(
            "XML declarations are not permitted in a canonical local SVG signature".to_string(),
        );
    }
    let normalized = normalize_xml_for_local_signature(svg);
    let document =
        roxmltree::Document::parse(normalized.as_ref()).map_err(|error| error.to_string())?;
    if document.descendants().any(|node| node.is_pi()) {
        return Err(
            "processing instructions are not permitted in a canonical local SVG signature"
                .to_string(),
        );
    }
    let root = document.root_element();
    if root.tag_name().name() != "svg" {
        return Err(format!(
            "expected <svg> document root, found <{}>",
            root.tag_name().name()
        ));
    }
    let mut canonical = String::new();
    write_canonical_local_svg_node(&mut canonical, root, decimals);
    Ok(CanonicalLocalSvgSignature {
        decimals,
        canonical,
    })
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

fn collect_dom_diffs_path(
    upstream: &SvgDomNode,
    local: &SvgDomNode,
    path: &mut Vec<String>,
    differences: &mut Vec<String>,
) {
    if upstream.name != local.name {
        differences.push(format!(
            "{}: element name mismatch upstream={} local={}",
            path.join("/"),
            upstream.name,
            local.name
        ));
        return;
    }

    if upstream.attrs != local.attrs {
        for (k, v_up) in &upstream.attrs {
            match local.attrs.get(k) {
                None => differences.push(format!("{}: missing attr `{k}`", path.join("/"))),
                Some(v_lo) if v_lo != v_up => {
                    differences.push(format!(
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
                differences.push(format!("{}: extra attr `{k}`", path.join("/")));
            }
        }
    }

    if upstream.text != local.text {
        differences.push(format!(
            "{}: text mismatch upstream=`{}` local=`{}`",
            path.join("/"),
            truncate(upstream.text.as_deref().unwrap_or(""), 120),
            truncate(local.text.as_deref().unwrap_or(""), 120)
        ));
    }

    let n = upstream.children.len().min(local.children.len());
    for i in 0..n {
        path.push(format!("{}[{}]", upstream.children[i].name, i));
        collect_dom_diffs_path(&upstream.children[i], &local.children[i], path, differences);
        path.pop();
    }

    if upstream.children.len() != local.children.len() {
        differences.push(format!(
            "{}: child count mismatch upstream={} local={}",
            path.join("/"),
            upstream.children.len(),
            local.children.len()
        ));
    }
}

/// Returns every structural, attribute, and text difference in deterministic traversal order.
///
/// A node with a different element name cannot be aligned with its counterpart, so that branch
/// contributes one name mismatch and is not recursively expanded. All other differences on a
/// node and its aligned descendants are retained instead of stopping at the first mismatch.
pub(crate) fn dom_diffs(upstream: &SvgDomNode, local: &SvgDomNode) -> Vec<String> {
    let mut path = vec![upstream.name.clone()];
    let mut differences = Vec::new();
    collect_dom_diffs_path(upstream, local, &mut path, &mut differences);
    differences
}

/// Marker included when a mismatch report contains more than its first detail.
///
/// Existing single-detail reports retain their exact spelling. The marker makes the complete
/// difference set explicit and gives residual policies a stable way to reject a known first
/// mismatch when a second mismatch was introduced for the same fixture.
pub(crate) const ADDITIONAL_DOM_DIFFS_MARKER: &str = "additional DOM differences";

pub(crate) fn format_dom_diffs(differences: &[String]) -> Option<String> {
    let first = differences.first()?;
    if differences.len() == 1 {
        return Some(first.clone());
    }

    Some(format!(
        "{first}; {ADDITIONAL_DOM_DIFFS_MARKER} ({}): {}",
        differences.len() - 1,
        differences[1..].join(" | "),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NON_STRICT_FLOWCHART_MODES: [DomMode; 3] =
        [DomMode::Structure, DomMode::Parity, DomMode::ParityRoot];

    fn assert_flowchart_non_strict_equivalent(upstream: &str, local: &str) {
        for mode in NON_STRICT_FLOWCHART_MODES {
            assert_eq!(
                dom_signature(upstream, mode, 3).unwrap(),
                dom_signature(local, mode, 3).unwrap(),
                "mode={mode:?}"
            );
        }
        assert_ne!(
            dom_signature(upstream, DomMode::Strict, 3).unwrap(),
            dom_signature(local, DomMode::Strict, 3).unwrap()
        );
    }

    #[test]
    fn canonical_local_svg_signature_rejects_xml_declarations() {
        for svg in [
            r#"<?xml version="1.0"?><svg/>"#,
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-16\"?><svg/>",
        ] {
            let error = canonical_local_svg_signature(svg, 3)
                .expect_err("XML declarations must not be omitted from the receipt contract");
            assert!(error.contains("XML declarations are not permitted"));
        }
    }

    #[test]
    fn dom_mode_parser_is_strict_and_display_is_canonical() {
        for (raw, expected, displayed) in [
            ("strict", DomMode::Strict, "strict"),
            ("structure", DomMode::Structure, "structure"),
            ("parity", DomMode::Parity, "parity"),
            ("parity_root", DomMode::ParityRoot, "parity-root"),
        ] {
            let parsed = raw.parse::<DomMode>().expect("known DOM mode");
            assert_eq!(parsed, expected);
            assert_eq!(parsed.to_string(), displayed);
        }
        for unknown in ["", "unknown", "structural", "PARITY"] {
            assert!(unknown.parse::<DomMode>().is_err(), "mode={unknown:?}");
        }
    }

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
    fn strict_normalizes_block_generated_ids_only() {
        let svg = r#"<svg aria-roledescription="block"><g id="id-abc123def456-1"/><g id="flowchart-A-0"/></svg>"#;
        let dom = dom_signature(svg, DomMode::Strict, 3).unwrap();
        assert_eq!(
            dom.children[0].attrs.get("id").map(|s| s.as_str()),
            Some("id-<id>-1")
        );
        assert_eq!(
            dom.children[1].attrs.get("id").map(|s| s.as_str()),
            Some("flowchart-A-0")
        );
    }

    #[test]
    fn flowchart_non_strict_normalizes_scoped_roles_but_preserves_authored_ids() {
        let legacy = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><g class="clusters"><g class="cluster" id="diagram-A"><rect/></g></g><g class="nodes"><g class="node" id="diagram-flowchart-A-0"><rect/></g></g><g class="edgePaths"><path class="flowchart-link" id="diagram-L_A_B_0" data-et="edge" data-id="L_A_B_0"/></g></svg>"#;
        let typed = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><g class="clusters"><g class="cluster" id="diagram-merman-flowchart-document-cluster-0" data-id="A" data-et="cluster"><rect/></g></g><g class="nodes"><g class="node" id="diagram-merman-flowchart-document-node-0" data-id="A" data-et="node"><rect/></g></g><g class="edgePaths"><path class="flowchart-link" id="diagram-merman-flowchart-document-edge-0" data-et="edge" data-id="L_A_B_0"/></g></svg>"#;

        assert_flowchart_non_strict_equivalent(legacy, typed);

        let authored_a = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><g class="legend" id="flowchart-custom-0"/></svg>"#;
        let authored_b = authored_a.replace("flowchart-custom-0", "flowchart-custom-1");
        for mode in NON_STRICT_FLOWCHART_MODES {
            assert_ne!(
                dom_signature(authored_a, mode, 3).unwrap(),
                dom_signature(&authored_b, mode, 3).unwrap(),
                "mode={mode:?}"
            );
        }
    }

    #[test]
    fn flowchart_root_layout_groups_have_stable_semantic_order() {
        let svg = r#"<svg aria-roledescription="flowchart-v2"><g class="root"><g class="nodes"/><g class="edgeLabels"/><g class="edgePath"/><g class="clusters"/></g></svg>"#;
        let dom = dom_signature(svg, DomMode::Structure, 3).unwrap();
        let root = &dom.children[0];
        let classes = root
            .children
            .iter()
            .map(|child| child.attrs.get("class").map(String::as_str))
            .collect::<Vec<_>>();
        assert_eq!(
            classes,
            vec![
                Some("clusters"),
                Some("edgePath"),
                Some("edgeLabels"),
                Some("nodes")
            ]
        );
    }

    #[test]
    fn flowchart_non_strict_ignores_only_renderer_paint_marker_ids() {
        let upstream = r#"<svg aria-roledescription="flowchart-v2"><g class="cluster" data-look="handDrawn"><g><path d="M0 0"/></g></g></svg>"#;
        let local = r#"<svg aria-roledescription="flowchart-v2"><g class="cluster" data-look="handDrawn"><g><path id="diagram-cluster-merman-fill-paint" d="M0 0"/></g></g></svg>"#;

        assert_eq!(
            dom_signature(upstream, DomMode::Structure, 3).unwrap(),
            dom_signature(local, DomMode::Structure, 3).unwrap()
        );
        assert_ne!(
            dom_signature(upstream, DomMode::Strict, 3).unwrap(),
            dom_signature(local, DomMode::Strict, 3).unwrap()
        );
    }

    #[test]
    fn tree_view_non_strict_ignores_only_renderer_dual_paint_marker_ids() {
        let upstream = r#"<svg aria-roledescription="treeView"><line x1="0" y1="0" x2="10" y2="0" class="treeView-node-line"/></svg>"#;
        let local = r#"<svg aria-roledescription="treeView"><line id="treeView-edge-0-merman-fill-stroke-paint" x1="0" y1="0" x2="10" y2="0" class="treeView-node-line"/></svg>"#;

        assert_eq!(
            dom_signature(upstream, DomMode::Structure, 3).unwrap(),
            dom_signature(local, DomMode::Structure, 3).unwrap()
        );
        assert_ne!(
            dom_signature(upstream, DomMode::Strict, 3).unwrap(),
            dom_signature(local, DomMode::Strict, 3).unwrap()
        );

        let wrong_class = local.replace("treeView-node-line", "treeView-node-icon");
        assert_ne!(
            dom_signature(upstream, DomMode::Structure, 3).unwrap(),
            dom_signature(&wrong_class, DomMode::Structure, 3).unwrap()
        );
        let wrong_family = local.replace(
            "aria-roledescription=\"treeView\"",
            "aria-roledescription=\"flowchart-v2\"",
        );
        assert_ne!(
            dom_signature(upstream, DomMode::Structure, 3).unwrap(),
            dom_signature(&wrong_family, DomMode::Structure, 3).unwrap()
        );
    }

    #[test]
    fn flowchart_nodes_keep_source_order_across_generated_id_shapes() {
        let upstream = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><g class="nodes"><g class="node default" id="diagram-flowchart-request-0"/><g class="node default" id="diagram-flowchart-receive-1"/><g class="label edgeLabel" id="edge-label-triage-answer-L_triage_answer_0"/></g></svg>"#;
        let local = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><g class="nodes"><g class="node default" id="diagram-merman-flowchart-document-node-0" data-id="request" data-et="node"/><g class="node default" id="diagram-merman-flowchart-document-node-1" data-id="receive" data-et="node"/><g class="label edgeLabel" id="diagram-merman-flowchart-document-synthetic-label-12" data-id="L_triage_answer_0" data-et="edge-label"/></g></svg>"#;

        assert_eq!(
            dom_signature(upstream, DomMode::Structure, 3).unwrap(),
            dom_signature(local, DomMode::Structure, 3).unwrap()
        );
    }

    #[test]
    fn flowchart_elk_nodes_compare_by_semantic_identity_not_layout_order() {
        let upstream = r#"<svg id="diagram" aria-roledescription="flowchart-elk"><g class="nodes"><g class="node default" id="node-a"/><g class="node default" id="node-b"/></g></svg>"#;
        let local = r#"<svg id="diagram" aria-roledescription="flowchart-elk"><g class="nodes"><g class="node default" id="diagram-merman-flowchart-document-node-1" data-id="b" data-et="node"/><g class="node default" id="diagram-merman-flowchart-document-node-0" data-id="a" data-et="node"/></g></svg>"#;

        assert_eq!(
            dom_signature(upstream, DomMode::Structure, 3).unwrap(),
            dom_signature(local, DomMode::Structure, 3).unwrap()
        );
    }

    #[test]
    fn parity_does_not_infer_eventmodeling_fill_from_competing_root_declarations() {
        for styles in [
            "<style>#event-diagram{fill:#333;fill:#f00;}</style>",
            "<style>#event-diagram{fill:#333;fill:#333;}</style>",
            "<style>#event-diagram{fill:#333;}#event-diagram{fill:#f00;}</style>",
            "<style>#event-diagram{fill:#333;}</style><style>#event-diagram{fill:#f00;}</style>",
            "<style>#event-diagram{fill:#333!important;fill:#f00!important;}</style>",
            "<style>#event-diagram{fill:#333;FILL:#f00;}</style>",
            r"<style>#event-diagram{fill:#333;f\69ll:#f00;}</style>",
            "<style>#event-diagram{fill:#333;all:initial;}</style>",
        ] {
            let upstream = format!(
                r#"<svg id="event-diagram" aria-roledescription="eventmodeling">{styles}<g class="em-swimlane"><text>Events</text></g></svg>"#
            );
            let local = upstream.replace("<text>", "<text fill=\"#333\">");
            for mode in [DomMode::Structure, DomMode::Parity, DomMode::ParityRoot] {
                assert_ne!(
                    dom_signature(&upstream, mode, 3).unwrap(),
                    dom_signature(&local, mode, 3).unwrap(),
                    "competing root declarations must remain unnormalized: {styles}/{mode:?}"
                );
            }
        }
    }

    #[test]
    fn parity_does_not_infer_eventmodeling_fill_through_stylesheet_overrides() {
        for rule in [
            "#event-diagram .em-swimlane{fill:#f00;}",
            "#event-diagram g{fill:#f00;}",
            "#event-diagram text{fill:#f00;}",
            "#event-diagram .absent,#event-diagram .em-swimlane{fill:#f00;}",
            "#event-diagram,#event-diagram .em-swimlane{fill:#333;}",
            "#event-diagram .em-swimlane{all:initial;}",
            "#event-diagram text{all:initial;}",
            r"#event-diagram .em-swimlane{f\69ll:#f00;}",
            "#event-diagram :is(.em-swimlane,.absent){fill:#f00;}",
            "#event-diagram :is(.em-swimlane) .absent{fill:#f00;}",
            "#event-diagram > .absent{fill:#f00;}",
            "@media screen{#event-diagram .em-swimlane{fill:#f00;}}",
        ] {
            let upstream = format!(
                r##"<svg id="event-diagram" aria-roledescription="eventmodeling"><style>#event-diagram{{fill:#333;}}{rule}</style><g class="em-swimlane"><text>Events</text></g></svg>"##
            );
            let local = upstream.replace("<text>", "<text fill=\"#333\">");
            for mode in [DomMode::Structure, DomMode::Parity, DomMode::ParityRoot] {
                assert_ne!(
                    dom_signature(&upstream, mode, 3).unwrap(),
                    dom_signature(&local, mode, 3).unwrap(),
                    "stylesheet override must block root inheritance: {rule}/{mode:?}"
                );
            }
        }
    }

    #[test]
    fn parity_does_not_infer_eventmodeling_fill_from_later_nested_root_rule() {
        for selectors in [
            "@media print",
            "@supports (display: grid)",
            "@keyframes dash",
            ".unused",
        ] {
            let upstream = format!(
                r##"<svg id="event-diagram" aria-roledescription="eventmodeling"><style>{selectors}{{.unused{{stroke:red;}}#event-diagram{{fill:#333;}}}}</style><g class="em-swimlane"><text>Events</text></g></svg>"##
            );
            let local = upstream.replace("<text>", "<text fill=\"#333\">");
            for mode in [DomMode::Structure, DomMode::Parity, DomMode::ParityRoot] {
                assert_ne!(
                    dom_signature(&upstream, mode, 3).unwrap(),
                    dom_signature(&local, mode, 3).unwrap(),
                    "a later nested rule must not become an unconditional root rule: {selectors}/{mode:?}"
                );
            }
        }
    }

    #[test]
    fn parity_infers_eventmodeling_fill_with_proven_unrelated_class_rules() {
        for rule in [
            "",
            "#event-diagram .em-swimlane{stroke:#f00;}",
            "#event-diagram .marker{fill:#f00;}",
            "#event-diagram .error-icon,#event-diagram .error-text{fill:#f00;}",
            "#event-diagram [data-look='neo'].node circle .state-start{fill:#f00;}",
            "@keyframes dash{to{stroke-dashoffset:0;}}",
        ] {
            let upstream = format!(
                r##"<svg id="event-diagram" aria-roledescription="eventmodeling"><style>#event-diagram{{fill:#333;}}{rule}</style><g class="em-swimlane"><text>Events</text></g><g class="marker"/></svg>"##
            );
            let local = upstream.replace("<text>", "<text fill=\"#333\">");
            for mode in [DomMode::Structure, DomMode::Parity, DomMode::ParityRoot] {
                assert_eq!(
                    dom_signature(&upstream, mode, 3).unwrap(),
                    dom_signature(&local, mode, 3).unwrap(),
                    "unrelated rules must preserve root inheritance: {rule}/{mode:?}"
                );
            }
        }
    }

    #[test]
    fn parity_infers_eventmodeling_fill_with_upstream_fixture_stylesheet() {
        let upstream = include_str!(
            "../../../fixtures/upstream-svgs/eventmodeling/upstream_docs_eventmodeling_minimum.svg"
        );
        let local = upstream.replace("<text ", "<text fill=\"#333\" ");
        assert_ne!(upstream, local);
        for mode in [DomMode::Structure, DomMode::Parity, DomMode::ParityRoot] {
            assert_eq!(
                dom_signature(upstream, mode, 3).unwrap(),
                dom_signature(&local, mode, 3).unwrap(),
                "upstream fixture must preserve root inheritance: {mode:?}"
            );
        }
    }

    #[test]
    fn parity_does_not_infer_eventmodeling_fill_through_source_overrides() {
        for (root_style, group_attributes, text_attributes) in [
            ("", "fill=\"#f00\"", ""),
            ("", "style=\"fill:#f00\"", ""),
            ("fill:#f00", "", ""),
            ("", "style=\"all:initial\"", ""),
            ("", "", "style=\"fill:#f00\""),
        ] {
            let upstream = format!(
                r##"<svg id="event-diagram" aria-roledescription="eventmodeling" style="{root_style}"><style>#event-diagram{{fill:#333;}}</style><g class="em-swimlane" {group_attributes}><text {text_attributes}>Events</text></g></svg>"##
            );
            let local = upstream.replace("<text ", "<text fill=\"#333\" ");
            for mode in [DomMode::Structure, DomMode::Parity, DomMode::ParityRoot] {
                assert_ne!(
                    dom_signature(&upstream, mode, 3).unwrap(),
                    dom_signature(&local, mode, 3).unwrap(),
                    "source override must block root inheritance: {upstream}/{mode:?}"
                );
            }
        }
    }

    #[test]
    fn parity_compares_eventmodeling_swimlane_effective_fill() {
        let upstream = r##"<svg id="event-diagram" aria-roledescription="eventmodeling"><style>#event-diagram{fill:#333;}</style><g class="em-swimlane"><text font-weight="bold">Events</text></g></svg>"##;
        let local = r##"<svg id="event-diagram" aria-roledescription="eventmodeling"><style>#event-diagram{fill:#333;}</style><g class="em-swimlane"><text fill="#333" font-weight="bold">Events</text></g></svg>"##;

        for mode in [DomMode::Structure, DomMode::Parity, DomMode::ParityRoot] {
            assert_eq!(
                dom_signature(upstream, mode, 3).unwrap(),
                dom_signature(local, mode, 3).unwrap(),
                "mode={mode:?}"
            );
        }
        assert_ne!(
            dom_signature(upstream, DomMode::Strict, 3).unwrap(),
            dom_signature(local, DomMode::Strict, 3).unwrap()
        );

        let wrong_fill = local.replace("fill=\"#333\"", "fill=\"#f00\"");
        assert_ne!(
            dom_signature(upstream, DomMode::Parity, 3).unwrap(),
            dom_signature(&wrong_fill, DomMode::Parity, 3).unwrap()
        );

        let unproven_upstream = upstream.replace("<style>#event-diagram{fill:#333;}</style>", "");
        assert_ne!(
            dom_signature(&unproven_upstream, DomMode::Parity, 3).unwrap(),
            dom_signature(local, DomMode::Parity, 3).unwrap()
        );

        let quoted_style = "font-family:'quoted;family';fill:#333;";
        let quoted_upstream = upstream.replace("fill:#333;", quoted_style);
        let quoted_local = local.replace("fill:#333;", quoted_style);
        assert_eq!(
            dom_signature(&quoted_upstream, DomMode::Parity, 3).unwrap(),
            dom_signature(&quoted_local, DomMode::Parity, 3).unwrap()
        );
    }

    #[test]
    fn parity_ignores_only_ishikawa_renderer_text_bbox_metadata() {
        let upstream = r#"<svg aria-roledescription="ishikawa"><text class="ishikawa-label">Cause</text></svg>"#;
        let local = r#"<svg aria-roledescription="ishikawa"><text class="ishikawa-label" data-merman-text-bbox="0,0,10,10">Cause</text></svg>"#;

        assert_eq!(
            dom_signature(upstream, DomMode::Structure, 3).unwrap(),
            dom_signature(local, DomMode::Structure, 3).unwrap()
        );
        assert_ne!(
            dom_signature(upstream, DomMode::Strict, 3).unwrap(),
            dom_signature(local, DomMode::Strict, 3).unwrap()
        );

        let unrelated = r#"<svg aria-roledescription="flowchart-v2"><text data-merman-text-bbox="0,0,10,10">Cause</text></svg>"#;
        assert_ne!(
            dom_signature(upstream, DomMode::Structure, 3).unwrap(),
            dom_signature(unrelated, DomMode::Structure, 3).unwrap()
        );
    }

    #[test]
    fn flowchart_non_strict_normalizes_all_node_shape_producer_ids() {
        for class in ["node", "rough-node", "icon-shape", "image-shape"] {
            let legacy = format!(
                r#"<svg id="diagram" aria-roledescription="flowchart-v2"><g class="nodes"><g class="{class} default" id="diagram-flowchart-A-0"><rect/></g></g></svg>"#
            );
            let typed = format!(
                r#"<svg id="diagram" aria-roledescription="flowchart-v2"><g class="nodes"><g class="{class} default" id="diagram-merman-flowchart-document-node-7" data-id="A" data-et="node"><rect/></g></g></svg>"#
            );
            assert_flowchart_non_strict_equivalent(&legacy, &typed);
        }

        let unrelated = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><g class="nodes"><g class="rough-node-copy" id="diagram-flowchart-A-0"><rect/></g></g></svg>"#;
        let falsely_typed = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><g class="nodes"><g class="rough-node-copy" id="diagram-merman-flowchart-document-node-7" data-id="A" data-et="node"><rect/></g></g></svg>"#;
        for mode in NON_STRICT_FLOWCHART_MODES {
            assert_ne!(
                dom_signature(unrelated, mode, 3).unwrap(),
                dom_signature(falsely_typed, mode, 3).unwrap(),
                "mode={mode:?}"
            );
        }
    }

    #[test]
    fn flowchart_non_strict_normalizes_closed_a11y_idrefs() {
        let upstream = r#"<svg id="diagram" aria-roledescription="flowchart-v2" aria-labelledby="chart-title-diagram" aria-describedby="chart-desc-diagram"><title id="chart-title-diagram">Title</title><desc id="chart-desc-diagram">Description</desc></svg>"#;
        let local = r#"<svg id="diagram" aria-roledescription="flowchart-v2" aria-labelledby="diagram-merman-flowchart-document-a11y-title" aria-describedby="diagram-merman-flowchart-document-a11y-description"><title id="diagram-merman-flowchart-document-a11y-title">Title</title><desc id="diagram-merman-flowchart-document-a11y-description">Description</desc></svg>"#;

        assert_flowchart_non_strict_equivalent(upstream, local);
    }

    #[test]
    fn flowchart_a11y_normalization_rejects_broken_or_wrong_kind_refs() {
        let upstream = r#"<svg id="diagram" aria-roledescription="flowchart-v2" aria-labelledby="chart-title-diagram"><title id="chart-title-diagram">Title</title></svg>"#;
        let broken = r#"<svg id="diagram" aria-roledescription="flowchart-v2" aria-labelledby="missing-title"><title id="diagram-merman-flowchart-document-a11y-title">Title</title></svg>"#;
        let wrong_kind = r#"<svg id="diagram" aria-roledescription="flowchart-v2" aria-labelledby="diagram-merman-flowchart-document-a11y-title"><desc id="diagram-merman-flowchart-document-a11y-title">Title</desc></svg>"#;

        for mode in NON_STRICT_FLOWCHART_MODES {
            let expected = dom_signature(upstream, mode, 3).unwrap();
            assert_ne!(expected, dom_signature(broken, mode, 3).unwrap());
            assert_ne!(expected, dom_signature(wrong_kind, mode, 3).unwrap());
        }
    }

    #[test]
    fn flowchart_non_strict_normalizes_root_gradient_id_and_reference() {
        let upstream = r##"<svg id="diagram" aria-roledescription="flowchart-v2"><linearGradient id="diagram-gradient"><stop/></linearGradient><rect fill="url(#diagram-gradient)"/></svg>"##;
        let local = r##"<svg id="diagram" aria-roledescription="flowchart-v2"><linearGradient id="diagram-merman-flowchart-document-gradient-root"><stop/></linearGradient><rect fill="url(#diagram-merman-flowchart-document-gradient-root)"/></svg>"##;

        assert_flowchart_non_strict_equivalent(upstream, local);
    }

    #[test]
    fn flowchart_gradient_normalization_rejects_nested_authored_and_broken_refs() {
        let nested_upstream = r##"<svg id="diagram" aria-roledescription="flowchart-v2"><defs><linearGradient id="diagram-gradient"><stop/></linearGradient></defs><rect fill="url(#diagram-gradient)"/></svg>"##;
        let nested_local = r##"<svg id="diagram" aria-roledescription="flowchart-v2"><defs><linearGradient id="diagram-merman-flowchart-document-gradient-root"><stop/></linearGradient></defs><rect fill="url(#diagram-merman-flowchart-document-gradient-root)"/></svg>"##;
        let authored_a = r##"<svg id="diagram" aria-roledescription="flowchart-v2"><linearGradient id="authored-one"><stop/></linearGradient><rect fill="url(#authored-one)"/></svg>"##;
        let authored_b = authored_a.replace("authored-one", "authored-two");
        let broken = r##"<svg id="diagram" aria-roledescription="flowchart-v2"><linearGradient id="diagram-merman-flowchart-document-gradient-root"><stop/></linearGradient><rect fill="url(#missing-gradient)"/></svg>"##;
        let valid = r##"<svg id="diagram" aria-roledescription="flowchart-v2"><linearGradient id="diagram-gradient"><stop/></linearGradient><rect fill="url(#diagram-gradient)"/></svg>"##;
        let wrong_role_upstream = r##"<svg id="diagram" aria-roledescription="flowchart-v2"><linearGradient id="diagram-gradient"><stop/></linearGradient><rect filter="url(#diagram-gradient)"/></svg>"##;
        let wrong_role_local = r##"<svg id="diagram" aria-roledescription="flowchart-v2"><linearGradient id="diagram-merman-flowchart-document-gradient-root"><stop/></linearGradient><rect filter="url(#diagram-merman-flowchart-document-gradient-root)"/></svg>"##;

        for mode in NON_STRICT_FLOWCHART_MODES {
            assert_ne!(
                dom_signature(nested_upstream, mode, 3).unwrap(),
                dom_signature(nested_local, mode, 3).unwrap()
            );
            assert_ne!(
                dom_signature(authored_a, mode, 3).unwrap(),
                dom_signature(&authored_b, mode, 3).unwrap()
            );
            assert_ne!(
                dom_signature(valid, mode, 3).unwrap(),
                dom_signature(broken, mode, 3).unwrap()
            );
            assert_ne!(
                dom_signature(wrong_role_upstream, mode, 3).unwrap(),
                dom_signature(wrong_role_local, mode, 3).unwrap()
            );
        }
    }

    #[test]
    fn flowchart_non_strict_normalizes_synthetic_label_producer_ids() {
        let upstream = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><g class="edgeLabels"><g class="label edgeLabel" id="A---A---1"/></g></svg>"#;
        let local = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><g class="edgeLabels"><g class="label edgeLabel" id="diagram-merman-flowchart-document-synthetic-label-2" data-id="A---A---1" data-et="edge-label"/></g></svg>"#;

        assert_flowchart_non_strict_equivalent(upstream, local);
        assert_flowchart_non_strict_equivalent(
            upstream,
            &local.replace("synthetic-label-2", "synthetic-label-9"),
        );

        let changed_id = local.replace("A---A---1", "B---B---1");
        let malformed_id = local.replace("A---A---1", "A---B---1");
        let malformed_producer = local.replace("synthetic-label-2", "synthetic-label-x");
        let changed_role = local.replace("data-et=\"edge-label\"", "data-et=\"edge\"");
        let unrelated = local.replace("label edgeLabel", "edgeLabel");
        for mode in NON_STRICT_FLOWCHART_MODES {
            let expected = dom_signature(upstream, mode, 3).unwrap();
            assert_ne!(expected, dom_signature(&changed_id, mode, 3).unwrap());
            assert_ne!(expected, dom_signature(&malformed_id, mode, 3).unwrap());
            assert_ne!(
                expected,
                dom_signature(&malformed_producer, mode, 3).unwrap()
            );
            assert_ne!(expected, dom_signature(&changed_role, mode, 3).unwrap());
            assert_ne!(expected, dom_signature(&unrelated, mode, 3).unwrap());
        }
    }

    #[test]
    fn flowchart_non_strict_normalizes_known_elk_cluster_sentinel() {
        let upstream = r#"<svg id="diagram" aria-roledescription="flowchart-elk"><g class="subgraphs"><g class="subgraph"><g class="cluster" id="[object Object]" data-look="classic"><rect/><g class="cluster-label"><foreignObject><div><span class="nodeLabel"><p>P1.5</p></span></div></foreignObject></g></g></g></g></svg>"#;
        let local = r#"<svg id="diagram" aria-roledescription="flowchart-elk"><g class="subgraphs"><g class="subgraph"><g class="cluster" id="diagram-merman-flowchart-document-cluster-0" data-id="P1.5" data-et="cluster" data-look="classic"><rect/><g class="cluster-label"><foreignObject><div><span class="nodeLabel"><p>P1.5</p></span></div></foreignObject></g></g></g></g></svg>"#;

        assert_flowchart_non_strict_equivalent(upstream, local);

        let authored_id = local.replace(
            "diagram-merman-flowchart-document-cluster-0",
            "diagram-authored-cluster",
        );
        let changed_label = local.replace("<p>P1.5</p>", "<p>Other</p>");
        let wrong_role = local.replace("data-et=\"cluster\"", "data-et=\"node\"");
        let sentinel_outside = upstream.replace(
            r#"<g class="subgraphs"><g class="subgraph">"#,
            r#"<g class="clusters"><g>"#,
        );
        let local_outside = local.replace(
            r#"<g class="subgraphs"><g class="subgraph">"#,
            r#"<g class="clusters"><g>"#,
        );
        for mode in NON_STRICT_FLOWCHART_MODES {
            let expected = dom_signature(upstream, mode, 3).unwrap();
            assert_ne!(expected, dom_signature(&authored_id, mode, 3).unwrap());
            assert_ne!(expected, dom_signature(&changed_label, mode, 3).unwrap());
            assert_ne!(expected, dom_signature(&wrong_role, mode, 3).unwrap());
            assert_ne!(
                dom_signature(&sentinel_outside, mode, 3).unwrap(),
                dom_signature(&local_outside, mode, 3).unwrap()
            );
        }
    }

    #[test]
    fn flowchart_non_strict_normalizes_compound_subgraph_node_shell_id() {
        let upstream = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><g class="root"><g class="clusters"/><g class="edgePaths"/><g class="edgeLabels"/><g class="nodes"><g class="node" id="diagram-B" data-look="classic"><g class="label"><span class="nodeLabel"><p>B</p></span></g></g><g class="root"><g class="clusters"><g class="cluster" id="diagram-A"><g class="cluster-label"><span class="nodeLabel"><p>A</p></span></g></g></g><g class="edgePaths"/><g class="edgeLabels"/><g class="nodes"/></g></g></g></svg>"#;
        let local = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><g class="root"><g class="clusters"/><g class="edgePaths"/><g class="edgeLabels"/><g class="nodes"><g class="node" id="diagram-merman-flowchart-document-node-subgraph-1" data-id="B" data-et="node" data-look="classic"><g class="label"><span class="nodeLabel"><p>B</p></span></g></g><g class="root"><g class="clusters"><g class="cluster" id="diagram-merman-flowchart-document-cluster-0" data-id="A" data-et="cluster"><g class="cluster-label"><span class="nodeLabel"><p>A</p></span></g></g></g><g class="edgePaths"/><g class="edgeLabels"/><g class="nodes"/></g></g></g></svg>"#;

        assert_flowchart_non_strict_equivalent(upstream, local);

        let ordinary_upstream = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><g class="root"><g class="nodes"><g class="node" id="diagram-B" data-look="classic"><g class="label"><span class="nodeLabel"><p>B</p></span></g></g></g></g></svg>"#;
        let ordinary_local = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><g class="root"><g class="nodes"><g class="node" id="diagram-merman-flowchart-document-node-0" data-id="B" data-et="node" data-look="classic"><g class="label"><span class="nodeLabel"><p>B</p></span></g></g></g></g></svg>"#;
        let unscoped_upstream = upstream.replace("id=\"diagram-B\"", "id=\"B\"");
        let wrong_label = upstream.replace("<p>B</p>", "<p>C</p>");
        for mode in NON_STRICT_FLOWCHART_MODES {
            assert_ne!(
                dom_signature(ordinary_upstream, mode, 3).unwrap(),
                dom_signature(ordinary_local, mode, 3).unwrap(),
                "mode={mode:?}"
            );
            assert_ne!(
                dom_signature(&unscoped_upstream, mode, 3).unwrap(),
                dom_signature(local, mode, 3).unwrap(),
                "mode={mode:?}"
            );
            assert_ne!(
                dom_signature(&wrong_label, mode, 3).unwrap(),
                dom_signature(local, mode, 3).unwrap(),
                "mode={mode:?}"
            );
        }
    }

    #[test]
    fn flowchart_non_strict_normalizes_closed_invisible_edge_id() {
        let upstream = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><g class="root"><g class="clusters"/><g class="edgePaths"><path id="diagram-L_A_B_0" class="edge-thickness-invisible edge-pattern-solid" data-edge="true" data-et="edge" data-id="L_A_B_0" data-look="classic"/></g><g class="edgeLabels"><g class="edgeLabel"><g class="label" data-id="L_A_B_0"/></g></g><g class="nodes"><g class="node" id="diagram-flowchart-A-0"><g class="label"><p>A</p></g></g><g class="node" id="diagram-flowchart-B-1"><g class="label"><p>B</p></g></g></g></g></svg>"#;
        let local = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><g class="root"><g class="clusters"/><g class="edgePaths"><path id="diagram-merman-flowchart-document-edge-0" class="edge-thickness-invisible edge-pattern-solid" data-edge="true" data-et="edge" data-id="L_A_B_0" data-look="classic"/></g><g class="edgeLabels"><g class="edgeLabel"><g class="label" data-id="L_A_B_0"/></g></g><g class="nodes"><g class="node" id="diagram-merman-flowchart-document-node-0" data-id="A" data-et="node"><g class="label"><p>A</p></g></g><g class="node" id="diagram-merman-flowchart-document-node-1" data-id="B" data-et="node"><g class="label"><p>B</p></g></g></g></g></svg>"#;

        assert_flowchart_non_strict_equivalent(upstream, local);
        assert_flowchart_non_strict_equivalent(
            upstream,
            &local.replace("document-edge-0", "document-edge-9"),
        );

        let missing_label_upstream = upstream.replace(
            r#"<g class="edgeLabels"><g class="edgeLabel"><g class="label" data-id="L_A_B_0"/></g></g>"#,
            r#"<g class="edgeLabels"/>"#,
        );
        let missing_label_local = local.replace(
            r#"<g class="edgeLabels"><g class="edgeLabel"><g class="label" data-id="L_A_B_0"/></g></g>"#,
            r#"<g class="edgeLabels"/>"#,
        );
        let missing_terminal_upstream = upstream.replace(
            r#"<g class="node" id="diagram-flowchart-B-1"><g class="label"><p>B</p></g></g>"#,
            "",
        );
        let missing_terminal_local = local.replace(
            r#"<g class="node" id="diagram-merman-flowchart-document-node-1" data-id="B" data-et="node"><g class="label"><p>B</p></g></g>"#,
            "",
        );
        let changed_role = local.replace("data-et=\"edge\"", "data-et=\"node\"");
        let changed_semantic_id = local.replace(
            "data-id=\"L_A_B_0\" data-look=\"classic\"",
            "data-id=\"L_A_C_0\" data-look=\"classic\"",
        );
        let authored_producer = local.replace(
            "diagram-merman-flowchart-document-edge-0",
            "diagram-authored-edge",
        );
        for mode in NON_STRICT_FLOWCHART_MODES {
            let expected = dom_signature(upstream, mode, 3).unwrap();
            assert_ne!(
                dom_signature(&missing_label_upstream, mode, 3).unwrap(),
                dom_signature(&missing_label_local, mode, 3).unwrap(),
                "mode={mode:?}"
            );
            assert_ne!(
                dom_signature(&missing_terminal_upstream, mode, 3).unwrap(),
                dom_signature(&missing_terminal_local, mode, 3).unwrap(),
                "mode={mode:?}"
            );
            assert_ne!(expected, dom_signature(&changed_role, mode, 3).unwrap());
            assert_ne!(
                expected,
                dom_signature(&changed_semantic_id, mode, 3).unwrap()
            );
            assert_ne!(
                expected,
                dom_signature(&authored_producer, mode, 3).unwrap()
            );
        }
    }

    #[test]
    fn flowchart_non_strict_keeps_edge_role_and_semantic_id_fail_closed() {
        let baseline = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><path class="flowchart-link" id="diagram-L_A_B_0" data-et="edge" data-id="L_A_B_0"/></svg>"#;
        let role_changed = baseline.replace("data-et=\"edge\"", "data-et=\"node\"");
        let id_changed = baseline.replace("data-id=\"L_A_B_0\"", "data-id=\"L_A_C_0\"");

        for mode in NON_STRICT_FLOWCHART_MODES {
            let expected = dom_signature(baseline, mode, 3).unwrap();
            assert_ne!(expected, dom_signature(&role_changed, mode, 3).unwrap());
            assert_ne!(expected, dom_signature(&id_changed, mode, 3).unwrap());
        }
    }

    #[test]
    fn flowchart_non_strict_preserves_semantic_edge_id_that_resembles_generated_id() {
        let baseline = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><path class="flowchart-link" id="diagram-L_id-a-1_B_0"/></svg>"#;
        let changed =
            baseline.replace("id=\"diagram-L_id-a-1_B_0\"", "id=\"diagram-L_id-b-1_B_0\"");

        for mode in NON_STRICT_FLOWCHART_MODES {
            assert_ne!(
                dom_signature(baseline, mode, 3).unwrap(),
                dom_signature(&changed, mode, 3).unwrap(),
                "mode={mode:?}"
            );
        }
    }

    #[test]
    fn flowchart_non_strict_preserves_semantic_edge_data_id_that_resembles_generated_id() {
        let baseline = r#"<svg id="diagram" aria-roledescription="flowchart-v2"><path class="flowchart-link" id="diagram-L_id-a-1_B_0" data-et="edge" data-id="L_id-a-1_B_0"/></svg>"#;
        let changed = baseline.replace("data-id=\"L_id-a-1_B_0\"", "data-id=\"L_id-b-1_B_0\"");

        for mode in NON_STRICT_FLOWCHART_MODES {
            assert_ne!(
                dom_signature(baseline, mode, 3).unwrap(),
                dom_signature(&changed, mode, 3).unwrap(),
                "mode={mode:?}"
            );
        }
    }

    #[test]
    fn flowchart_non_strict_preserves_nested_edge_label_data_id() {
        let baseline = r#"<svg aria-roledescription="flowchart-v2"><g class="edgeLabels"><g class="edgeLabel"><g class="label" data-id="L_id-a-1_B_0"/></g></g></svg>"#;
        let changed = baseline.replace("L_id-a-1_B_0", "L_id-b-1_B_0");

        for mode in NON_STRICT_FLOWCHART_MODES {
            assert_ne!(
                dom_signature(baseline, mode, 3).unwrap(),
                dom_signature(&changed, mode, 3).unwrap(),
                "mode={mode:?}"
            );
        }
    }

    #[test]
    fn parity_keeps_non_flowchart_generated_id_normalization() {
        let svg =
            r#"<svg aria-roledescription="block"><g class="node" id="id-abc123def456-1"/></svg>"#;
        let dom = dom_signature(svg, DomMode::Parity, 3).unwrap();

        assert_eq!(
            dom.children[0].attrs.get("id").map(String::as_str),
            Some("id-<id>-1")
        );
    }

    #[test]
    fn flowchart_parity_normalizes_nonsemantic_generated_ids_in_both_modes() {
        let svg = r#"<svg aria-roledescription="flowchart-v2"><g class="legend" id="id-abc123def456-1"/><rect class="node" id="id-fedcba654321-2"/></svg>"#;

        for mode in [DomMode::Parity, DomMode::ParityRoot] {
            let dom = dom_signature(svg, mode, 3).unwrap();
            assert_eq!(
                dom.children[0].attrs.get("id").map(String::as_str),
                Some("id-<id>-1")
            );
            assert_eq!(
                dom.children[1].attrs.get("id").map(String::as_str),
                Some("id-<id>-2")
            );
        }
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
    fn parity_root_keeps_distinct_svg_root_style_max_widths() {
        let a = r#"<svg width="100%" viewBox="0 0 560.391 10" style="max-width: 560.391px; background-color: white;"><path d="M 10 20 L 30 40"/></svg>"#;
        let b = r#"<svg width="100%" viewBox="0 0 560.375 10" style="max-width: 560.375px; background-color: white;"><path d="M 10 20 L 30 40"/></svg>"#;
        let dom_a = dom_signature(a, DomMode::ParityRoot, 3).unwrap();
        let dom_b = dom_signature(b, DomMode::ParityRoot, 3).unwrap();
        assert_ne!(dom_a.attrs.get("style"), dom_b.attrs.get("style"));
        assert_eq!(
            dom_a.attrs.get("style").map(|s| s.as_str()),
            Some("max-width: 560.391px; background-color: white;")
        );
        assert_eq!(
            dom_b.attrs.get("style").map(|s| s.as_str()),
            Some("max-width: 560.375px; background-color: white;")
        );
    }

    #[test]
    fn parity_root_does_not_hide_midpoint_straddling_widths() {
        let a = r#"<svg width="100%" viewBox="0 0 502.172 10" style="max-width: 502.172px; background-color: white;"><path d="M 10 20 L 30 40"/></svg>"#;
        let b = r#"<svg width="100%" viewBox="0 0 502.188 10" style="max-width: 502.188px; background-color: white;"><path d="M 10 20 L 30 40"/></svg>"#;
        let dom_a = dom_signature(a, DomMode::ParityRoot, 3).unwrap();
        let dom_b = dom_signature(b, DomMode::ParityRoot, 3).unwrap();
        assert_ne!(dom_a.attrs.get("style"), dom_b.attrs.get("style"));
    }

    #[test]
    fn parity_root_keeps_distinct_svg_root_viewbox_numbers() {
        let a = r#"<svg width="100%" viewBox="0 0 560.391 10.016" style="max-width: 560.391px; background-color: white;"><path d="M 10 20 L 30 40"/></svg>"#;
        let b = r#"<svg width="100%" viewBox="0 0 560.375 10" style="max-width: 560.375px; background-color: white;"><path d="M 10 20 L 30 40"/></svg>"#;
        let dom_a = dom_signature(a, DomMode::ParityRoot, 3).unwrap();
        let dom_b = dom_signature(b, DomMode::ParityRoot, 3).unwrap();
        assert_ne!(dom_a.attrs.get("viewBox"), dom_b.attrs.get("viewBox"));
        assert_eq!(
            dom_a.attrs.get("viewBox").map(|s| s.as_str()),
            Some("0 0 560.391 10.016")
        );
    }

    #[test]
    fn parity_root_keeps_distinct_svg_root_viewbox_origins() {
        let a = r#"<svg viewBox="-1.25 -2.5 100 100"/>"#;
        let b = r#"<svg viewBox="-1 -2.5 100 100"/>"#;

        let dom_a = dom_signature(a, DomMode::ParityRoot, 3).unwrap();
        let dom_b = dom_signature(b, DomMode::ParityRoot, 3).unwrap();

        assert_ne!(dom_a.attrs.get("viewBox"), dom_b.attrs.get("viewBox"));
    }

    #[test]
    fn dom_diff_reports_all_differences_for_one_fixture() {
        let upstream = dom_signature(
            r#"<svg data-root="upstream"><g data-node="upstream">upstream</g></svg>"#,
            DomMode::Strict,
            3,
        )
        .unwrap();
        let local = dom_signature(
            r#"<svg data-root="local"><g data-node="local">local</g></svg>"#,
            DomMode::Strict,
            3,
        )
        .unwrap();

        let differences = dom_diffs(&upstream, &local);

        assert_eq!(differences.len(), 3);
        assert!(differences.iter().any(|detail| {
            detail.contains("svg: attr `data-root` mismatch upstream=`upstream` local=`local`")
        }));
        assert!(differences.iter().any(|detail| {
            detail.contains("svg/g[0]: attr `data-node` mismatch upstream=`upstream` local=`local`")
        }));
        assert!(
            differences
                .iter()
                .any(|detail| detail.contains("svg/g[0]: text mismatch"))
        );

        let rendered = format_dom_diffs(&differences).expect("differences should render");
        assert!(rendered.contains(ADDITIONAL_DOM_DIFFS_MARKER));
        assert!(rendered.contains("svg/g[0]: text mismatch"));
    }

    #[test]
    fn parity_keeps_style_font_size_on_text_nodes() {
        let a = r#"<svg><text style="text-anchor: middle; font-size: 16px; font-weight: 400;">Hi</text></svg>"#;
        let b = r#"<svg><text style="text-anchor: middle; font-size: 18px; font-weight: 400;">Hi</text></svg>"#;
        let dom_a = dom_signature(a, DomMode::Parity, 3).unwrap();
        let dom_b = dom_signature(b, DomMode::Parity, 3).unwrap();
        let text_a = &dom_a.children[0];
        let text_b = &dom_b.children[0];
        assert_eq!(text_a.name, "text");
        assert_eq!(text_b.name, "text");
        assert_eq!(
            text_a.attrs.get("style").map(|s| s.as_str()),
            Some("font-size:16px")
        );
        assert_eq!(
            text_b.attrs.get("style").map(|s| s.as_str()),
            Some("font-size:18px")
        );
        assert_ne!(dom_a, dom_b);
    }

    #[test]
    fn evidence_profile_normalizes_only_generated_browser_text_rows() {
        let one_row = r#"<svg><text><tspan class="text-outer-tspan row" x="0"><tspan class="text-inner-tspan">FontSizeSvgProbe</tspan></tspan></text></svg>"#;
        let two_rows = r#"<svg><text><tspan class="text-outer-tspan row" x="0"><tspan class="text-inner-tspan">FontSizeSvgProb</tspan></tspan><tspan class="text-outer-tspan row" x="0"><tspan class="text-inner-tspan">e</tspan></tspan></text></svg>"#;
        let changed_text = r#"<svg><text><tspan class="text-outer-tspan row" x="0"><tspan class="text-inner-tspan">DifferentLabel</tspan></tspan></text></svg>"#;

        assert_ne!(
            dom_signature(one_row, DomMode::Parity, 3).unwrap(),
            dom_signature(two_rows, DomMode::Parity, 3).unwrap()
        );

        let profile = DomComparisonProfile::from_mode(DomMode::Parity)
            .with_browser_text_wrapping_normalized();
        assert_eq!(
            dom_signature_for_comparison(one_row, profile, 3).unwrap(),
            dom_signature_for_comparison(two_rows, profile, 3).unwrap()
        );
        assert_ne!(
            dom_signature_for_comparison(one_row, profile, 3).unwrap(),
            dom_signature_for_comparison(changed_text, profile, 3).unwrap()
        );

        let strict = DomComparisonProfile::from_mode(DomMode::Strict)
            .with_browser_text_wrapping_normalized();
        assert!(!strict.normalizes_browser_text_wrapping());
        assert_ne!(
            dom_signature_for_comparison(one_row, strict, 3).unwrap(),
            dom_signature_for_comparison(two_rows, strict, 3).unwrap()
        );
    }

    #[test]
    fn browser_text_profile_preserves_real_text_and_normalizes_formatting_whitespace() {
        let alpha = r#"<svg><text><tspan class="text-outer-tspan row"><tspan class="text-inner-tspan">Alpha</tspan></tspan></text></svg>"#;
        let alpha_with_formatting_whitespace = r#"<svg>
  <text>
    <tspan class="text-outer-tspan row">
      <tspan class="text-inner-tspan">
        Alpha
      </tspan>
    </tspan>
  </text>
</svg>"#;
        let beta = r#"<svg><text><tspan class="text-outer-tspan row"><tspan class="text-inner-tspan">Beta</tspan></tspan></text></svg>"#;
        let profile = DomComparisonProfile::from_mode(DomMode::Parity)
            .with_browser_text_wrapping_normalized();

        assert_eq!(
            dom_signature_for_comparison(alpha, profile, 3).unwrap(),
            dom_signature_for_comparison(alpha_with_formatting_whitespace, profile, 3).unwrap()
        );
        assert_ne!(
            dom_signature_for_comparison(alpha, profile, 3).unwrap(),
            dom_signature_for_comparison(beta, profile, 3).unwrap()
        );
    }

    #[test]
    fn browser_text_profile_preserves_architecture_label_content() {
        let alpha = r#"<svg aria-roledescription="architecture"><text><tspan class="text-outer-tspan row"><tspan class="text-inner-tspan">Alpha</tspan></tspan></text></svg>"#;
        let beta = r#"<svg aria-roledescription="architecture"><text><tspan class="text-outer-tspan row"><tspan class="text-inner-tspan">Beta</tspan></tspan></text></svg>"#;
        let profile = DomComparisonProfile::from_mode(DomMode::Parity)
            .with_browser_text_wrapping_normalized();

        assert_ne!(
            dom_signature_for_comparison(alpha, profile, 3).unwrap(),
            dom_signature_for_comparison(beta, profile, 3).unwrap()
        );
    }

    #[test]
    fn browser_text_profile_preserves_ambiguous_word_boundary_whitespace() {
        let spaced = r#"<svg><text><tspan class="text-outer-tspan row"><tspan class="text-inner-tspan">Hello</tspan><tspan class="text-inner-tspan"> world</tspan></tspan></text></svg>"#;
        let unspaced = r#"<svg><text><tspan class="text-outer-tspan row"><tspan class="text-inner-tspan">Helloworld</tspan></tspan></text></svg>"#;
        let two_rows = r#"<svg><text><tspan class="text-outer-tspan row"><tspan class="text-inner-tspan">Hello</tspan></tspan><tspan class="text-outer-tspan row"><tspan class="text-inner-tspan">world</tspan></tspan></text></svg>"#;
        let changed_word = r#"<svg><text><tspan class="text-outer-tspan row"><tspan class="text-inner-tspan">Goodbye</tspan></tspan><tspan class="text-outer-tspan row"><tspan class="text-inner-tspan">world</tspan></tspan></text></svg>"#;
        let profile = DomComparisonProfile::from_mode(DomMode::Parity)
            .with_browser_text_wrapping_normalized();

        assert_ne!(
            dom_signature_for_comparison(spaced, profile, 3).unwrap(),
            dom_signature_for_comparison(two_rows, profile, 3).unwrap()
        );
        assert_ne!(
            dom_signature_for_comparison(spaced, profile, 3).unwrap(),
            dom_signature_for_comparison(unspaced, profile, 3).unwrap()
        );
        assert_eq!(
            dom_signature_for_comparison(unspaced, profile, 3).unwrap(),
            dom_signature_for_comparison(two_rows, profile, 3).unwrap()
        );
        assert_ne!(
            dom_signature_for_comparison(unspaced, profile, 3).unwrap(),
            dom_signature_for_comparison(changed_word, profile, 3).unwrap()
        );
    }

    #[test]
    fn browser_text_profile_preserves_non_row_tspan_structure() {
        let generated = r#"<svg><text><tspan class="text-outer-tspan row"><tspan>Label</tspan></tspan></text></svg>"#;
        let authored =
            r#"<svg><text><tspan class="authored"><tspan>Label</tspan></tspan></text></svg>"#;
        let profile = DomComparisonProfile::from_mode(DomMode::Parity)
            .with_browser_text_wrapping_normalized();

        assert_ne!(
            dom_signature_for_comparison(generated, profile, 3).unwrap(),
            dom_signature_for_comparison(authored, profile, 3).unwrap()
        );
    }

    #[test]
    fn evidence_profile_normalizes_only_present_browser_text_length_values() {
        let measured_a = r#"<svg><text class="type" textLength="51">person</text></svg>"#;
        let measured_b = r#"<svg><text class="type" textLength="52">person</text></svg>"#;
        let missing = r#"<svg><text class="type">person</text></svg>"#;
        let changed_class = r#"<svg><text class="different" textLength="52">person</text></svg>"#;

        let profile =
            DomComparisonProfile::from_mode(DomMode::Parity).with_browser_text_length_normalized();
        assert_eq!(
            dom_signature_for_comparison(measured_a, profile, 3).unwrap(),
            dom_signature_for_comparison(measured_b, profile, 3).unwrap()
        );
        assert_ne!(
            dom_signature_for_comparison(measured_a, profile, 3).unwrap(),
            dom_signature_for_comparison(missing, profile, 3).unwrap()
        );
        assert_ne!(
            dom_signature_for_comparison(measured_a, profile, 3).unwrap(),
            dom_signature_for_comparison(changed_class, profile, 3).unwrap()
        );

        let strict =
            DomComparisonProfile::from_mode(DomMode::Strict).with_browser_text_length_normalized();
        assert!(!strict.normalizes_browser_text_length());
        assert_ne!(
            dom_signature_for_comparison(measured_a, strict, 3).unwrap(),
            dom_signature_for_comparison(measured_b, strict, 3).unwrap()
        );
    }

    #[test]
    fn parity_ignores_foreign_object_visible_overflow() {
        let upstream =
            r#"<svg><foreignObject width="10" height="20"><div>Hi</div></foreignObject></svg>"#;
        let local = r#"<svg><foreignObject width="10" height="20" overflow="visible" style="overflow: visible;"><div>Hi</div></foreignObject></svg>"#;

        let upstream_dom = dom_signature(upstream, DomMode::Parity, 3).unwrap();
        let local_dom = dom_signature(local, DomMode::Parity, 3).unwrap();
        assert_eq!(upstream_dom, local_dom);

        let strict_dom = dom_signature(local, DomMode::Strict, 3).unwrap();
        assert_eq!(
            strict_dom.children[0]
                .attrs
                .get("overflow")
                .map(String::as_str),
            Some("visible")
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
    fn parity_masks_class_hand_drawn_rough_node_path_geometry() {
        let upstream = r#"<svg class="classDiagram" aria-roledescription="class"><g class="rough-node default"><path d="M0,0 L10,10"/></g></svg>"#;
        let local = r#"<svg class="classDiagram" aria-roledescription="class"><g class="rough-node default"><path d="M0,0 C1,2 3,4 5,6"/></g></svg>"#;

        let upstream_dom = dom_signature(upstream, DomMode::Parity, 3).unwrap();
        let local_dom = dom_signature(local, DomMode::Parity, 3).unwrap();

        assert_eq!(upstream_dom, local_dom);
        assert_eq!(
            upstream_dom.children[0].children[0]
                .attrs
                .get("d")
                .map(|s| s.as_str()),
            Some("<class-rough-node-geom>")
        );
    }

    #[test]
    fn parity_normalizes_class_edge_note_dom_ids() {
        let upstream = r#"<svg class="classDiagram" aria-roledescription="class"><g><path id="edgeNote2" data-id="edgeNote2"/></g></svg>"#;
        let local = r#"<svg class="classDiagram" aria-roledescription="class"><g><path id="edgeNote1" data-id="edgeNote1"/></g></svg>"#;

        let upstream_dom = dom_signature(upstream, DomMode::Parity, 3).unwrap();
        let local_dom = dom_signature(local, DomMode::Parity, 3).unwrap();

        assert_eq!(upstream_dom, local_dom);
    }

    #[test]
    fn parity_masks_flowchart_hand_drawn_rough_node_path_geometry() {
        let upstream = r#"<svg class="flowchart" aria-roledescription="flowchart-v2"><g class="rough-node default"><path d="M0,0 L10,10"/></g></svg>"#;
        let local = r#"<svg class="flowchart" aria-roledescription="flowchart-v2"><g class="rough-node default"><path d="M0,0 C1,2 3,4 5,6"/></g></svg>"#;

        let upstream_dom = dom_signature(upstream, DomMode::Parity, 3).unwrap();
        let local_dom = dom_signature(local, DomMode::Parity, 3).unwrap();

        assert_eq!(upstream_dom, local_dom);
        assert_eq!(
            upstream_dom.children[0].children[0]
                .attrs
                .get("d")
                .map(|s| s.as_str()),
            Some("<flowchart-rough-node-geom>")
        );
    }

    #[test]
    fn parity_masks_flowchart_hand_drawn_cluster_path_geometry() {
        let upstream = r##"<svg class="flowchart" aria-roledescription="flowchart-v2"><g class="cluster" data-look="handDrawn"><path d="M0,0 L10,10" stroke="#000"/></g></svg>"##;
        let local = r##"<svg class="flowchart" aria-roledescription="flowchart-v2"><g class="cluster" data-look="handDrawn"><path d="M0,0 C1,2 3,4 5,6" stroke="#000"/></g></svg>"##;

        let upstream_dom = dom_signature(upstream, DomMode::Parity, 3).unwrap();
        let local_dom = dom_signature(local, DomMode::Parity, 3).unwrap();

        assert_eq!(upstream_dom, local_dom);
        assert_eq!(
            upstream_dom.children[0].children[0]
                .attrs
                .get("d")
                .map(|s| s.as_str()),
            Some("<flowchart-rough-cluster-geom>")
        );
    }

    #[test]
    fn parity_keeps_flowchart_classic_cluster_path_structure() {
        let upstream = r##"<svg class="flowchart" aria-roledescription="flowchart-v2"><g class="cluster" data-look="classic"><path d="M0,0 L10,10" stroke="#000"/></g></svg>"##;
        let local = r##"<svg class="flowchart" aria-roledescription="flowchart-v2"><g class="cluster" data-look="classic"><path d="M0,0 C1,2 3,4 5,6" stroke="#000"/></g></svg>"##;

        let upstream_dom = dom_signature(upstream, DomMode::Parity, 3).unwrap();
        let local_dom = dom_signature(local, DomMode::Parity, 3).unwrap();

        assert_ne!(upstream_dom, local_dom);
    }

    #[test]
    fn parity_keeps_non_class_flowchart_rough_node_path_structure() {
        let upstream = r#"<svg class="sequence" aria-roledescription="sequence"><g class="rough-node default"><path d="M0,0 L10,10"/></g></svg>"#;
        let local = r#"<svg class="sequence" aria-roledescription="sequence"><g class="rough-node default"><path d="M0,0 C1,2 3,4 5,6"/></g></svg>"#;

        let upstream_dom = dom_signature(upstream, DomMode::Parity, 3).unwrap();
        let local_dom = dom_signature(local, DomMode::Parity, 3).unwrap();

        assert_ne!(upstream_dom, local_dom);
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
    fn parity_masks_architecture_edge_arrow_transform_as_geom() {
        let svg = r#"<svg aria-roledescription="architecture"><g class="architecture-edges"><g><path class="edge" d="M 0,0 L 10,10"/><polygon class="arrow" points="0,0 10,0 5,10" transform="translate(1,2) rotate(45,5,10)"/></g></g></svg>"#;
        let dom = dom_signature(svg, DomMode::Parity, 3).unwrap();
        let arrow = &dom.children[0].children[0].children[1];

        assert_eq!(
            arrow.attrs.get("transform").map(|s| s.as_str()),
            Some("<geom>")
        );
    }

    #[test]
    fn parity_normalizes_architecture_diagram_scoped_dom_ids() {
        let prefixed = r#"<svg id="diag" aria-roledescription="architecture"><g class="architecture-services"><g id="diag-service-api"><g><path id="diag-node-api" class="node-bkg" d="M0,80 V5 Q0,0 5,0 H75 Q80,0 80,5 V80 Z"/></g></g></g><g class="architecture-groups"><rect id="diag-group-core" class="node-bkg" x="0" y="0" width="80" height="80"/></g></svg>"#;
        let bare = r#"<svg id="diag" aria-roledescription="architecture"><g class="architecture-services"><g id="service-api"><g><path id="node-api" class="node-bkg" d="M0,80 V5 Q0,0 5,0 H75 Q80,0 80,5 V80 Z"/></g></g></g><g class="architecture-groups"><rect id="group-core" class="node-bkg" x="0" y="0" width="80" height="80"/></g></svg>"#;

        let prefixed_dom = dom_signature(prefixed, DomMode::Parity, 3).unwrap();
        let bare_dom = dom_signature(bare, DomMode::Parity, 3).unwrap();

        assert_eq!(prefixed_dom, bare_dom);
    }

    #[test]
    fn parity_normalizes_sankey_diagram_scoped_dom_ids() {
        let prefixed = r##"<svg id="diag" aria-roledescription="sankey"><defs><linearGradient id="diag-linearGradient-5"><stop offset="0%" stop-color="#000"/></linearGradient></defs><g class="nodes"><g class="node" id="diag-node-1"/></g><g class="links"><path stroke="url(#diag-linearGradient-5)"/></g></svg>"##;
        let bare = r##"<svg id="diag" aria-roledescription="sankey"><defs><linearGradient id="linearGradient-5"><stop offset="0%" stop-color="#000"/></linearGradient></defs><g class="nodes"><g class="node" id="node-1"/></g><g class="links"><path stroke="url(#linearGradient-5)"/></g></svg>"##;

        let prefixed_dom = dom_signature(prefixed, DomMode::Parity, 3).unwrap();
        let bare_dom = dom_signature(bare, DomMode::Parity, 3).unwrap();

        assert_eq!(prefixed_dom, bare_dom);
    }

    #[test]
    fn structure_and_parity_fail_closed_on_vertical_timeline_marker_reference_semantics() {
        let upstream = r#"<svg id="timeline" aria-roledescription="timeline"><g class="lineWrapper"><line x1="430" y1="18" x2="430" y2="495" stroke-width="4" marker-end="url(#arrowhead)"/></g><defs><marker id="undefined-arrowhead"/></defs><g><path id="undefined-node-7" class="node-bkg node-undefined"/></g></svg>"#;
        let local = r#"<svg id="timeline" aria-roledescription="timeline"><g class="lineWrapper"><line x1="430" y1="18" x2="430" y2="495" stroke-width="4" marker-end="url(#timeline-arrowhead)"/></g><defs><marker id="timeline-arrowhead"/></defs><g><path id="timeline-node-7" class="node-bkg node-undefined"/></g></svg>"#;

        for mode in [DomMode::Structure, DomMode::Parity, DomMode::ParityRoot] {
            assert_ne!(
                dom_signature(upstream, mode, 3).unwrap(),
                dom_signature(local, mode, 3).unwrap()
            );
        }
        assert_ne!(
            dom_signature(upstream, DomMode::Strict, 3).unwrap(),
            dom_signature(local, DomMode::Strict, 3).unwrap()
        );
    }

    #[test]
    fn structure_and_parity_normalize_only_vertical_timeline_broken_node_ids() {
        let upstream = r#"<svg id="timeline" aria-roledescription="timeline"><g class="lineWrapper"><line x1="430" y1="18" x2="430" y2="495" stroke-width="4" marker-end="url(#arrowhead)"/></g><defs><marker id="undefined-arrowhead"/></defs><g><path id="undefined-node-7" class="node-bkg node-undefined"/></g></svg>"#;
        let corrected_node_id = r#"<svg id="timeline" aria-roledescription="timeline"><g class="lineWrapper"><line x1="430" y1="18" x2="430" y2="495" stroke-width="4" marker-end="url(#arrowhead)"/></g><defs><marker id="undefined-arrowhead"/></defs><g><path id="timeline-node-7" class="node-bkg node-undefined"/></g></svg>"#;

        for mode in [DomMode::Structure, DomMode::Parity, DomMode::ParityRoot] {
            assert_eq!(
                dom_signature(upstream, mode, 3).unwrap(),
                dom_signature(corrected_node_id, mode, 3).unwrap()
            );
        }
    }

    #[test]
    fn structure_and_parity_preserve_horizontal_timeline_ids() {
        let left = r#"<svg id="timeline" aria-roledescription="timeline"><g class="lineWrapper"><line x1="18" y1="430" x2="495" y2="430" stroke-width="4" marker-end="url(#arrowhead)"/></g><defs><marker id="undefined-arrowhead"/></defs></svg>"#;
        let right = r#"<svg id="timeline" aria-roledescription="timeline"><g class="lineWrapper"><line x1="18" y1="430" x2="495" y2="430" stroke-width="4" marker-end="url(#timeline-arrowhead)"/></g><defs><marker id="timeline-arrowhead"/></defs></svg>"#;

        for mode in [DomMode::Structure, DomMode::Parity, DomMode::ParityRoot] {
            assert_ne!(
                dom_signature(left, mode, 3).unwrap(),
                dom_signature(right, mode, 3).unwrap()
            );
        }
    }

    #[test]
    fn parity_normalizes_architecture_service_background_path_spelling() {
        let old_path = r#"<svg aria-roledescription="architecture"><g class="architecture-services"><g class="architecture-service"><g><path class="node-bkg" id="node-api" d="M0 80 v-80 q0,-5 5,-5 h80 q5,0 5,5 v80 H0 Z"/></g></g></g></svg>"#;
        let new_path = r#"<svg aria-roledescription="architecture"><g class="architecture-services"><g class="architecture-service"><g><path class="node-bkg" id="node-api" d="M0,80 V5 Q0,0 5,0 H75 Q80,0 80,5 V80 Z"/></g></g></g></svg>"#;

        let old_dom = dom_signature(old_path, DomMode::Parity, 3).unwrap();
        let new_dom = dom_signature(new_path, DomMode::Parity, 3).unwrap();

        assert_eq!(old_dom, new_dom);
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
        let svg = r#"<svg class="flowchart" aria-roledescription="flowchart-v2"><path x="12.3" y="4.56" width="7" height="8" class="label-container" label-offset-y="9.19347190389631"/></svg>"#;
        let dom = dom_signature(svg, DomMode::Parity, 3).unwrap();
        let path = &dom.children[0];
        assert_eq!(path.attrs.get("x").map(|s| s.as_str()), Some("<n>"));
        assert_eq!(path.attrs.get("y").map(|s| s.as_str()), Some("<n>"));
        assert_eq!(path.attrs.get("width").map(|s| s.as_str()), Some("<n>"));
        assert_eq!(path.attrs.get("height").map(|s| s.as_str()), Some("<n>"));
        assert_eq!(
            path.attrs.get("label-offset-y").map(|s| s.as_str()),
            Some("<n>")
        );
    }

    #[test]
    fn parity_keeps_label_offset_y_outside_flowchart_label_container() {
        let svg = r#"<svg aria-roledescription="stateDiagram"><path class="label-container" label-offset-y="9.19347190389631"/></svg>"#;
        let dom = dom_signature(svg, DomMode::Parity, 3).unwrap();
        let path = &dom.children[0];
        assert_eq!(
            path.attrs.get("label-offset-y").map(|s| s.as_str()),
            Some("9.193")
        );
    }

    #[test]
    fn parity_does_not_equate_invalid_quadrant_color_with_browser_computed_black() {
        let raw_upstream = r#"<svg><g class="data-points"><g class="data-point"><circle fill="hsl(240, 100%, NaN%)" stroke="hsl(240, 100%, NaN%)"/></g></g></svg>"#;
        let browser_materialized = r#"<svg><g class="data-points"><g class="data-point"><circle fill="rgb(0, 0, 0)" stroke="none"/></g></g></svg>"#;

        let raw_dom = dom_signature(raw_upstream, DomMode::Parity, 3).unwrap();
        let materialized_dom = dom_signature(browser_materialized, DomMode::Parity, 3).unwrap();

        assert_ne!(
            raw_dom, materialized_dom,
            "raw/source parity must not use browser-computed color equivalence"
        );
    }
}
