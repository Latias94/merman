use crate::{MermaidConfig, Result};
use regex::Regex;
use std::borrow::Cow;
use std::sync::OnceLock;

macro_rules! cached_regex {
    ($fn_name:ident, $pat:literal) => {
        fn $fn_name() -> &'static Regex {
            static RE: OnceLock<Regex> = OnceLock::new();
            RE.get_or_init(|| Regex::new($pat).expect("detector regex must compile"))
        }
    };
}

#[derive(Debug, thiserror::Error)]
#[error("No diagram type detected matching given configuration for text: {text}")]
pub struct DetectTypeError {
    pub text: String,
}

pub type DetectorFn = fn(text: &str, config: &mut MermaidConfig) -> bool;

#[derive(Debug, Clone)]
pub struct Detector {
    pub id: &'static str,
    pub detector: DetectorFn,
}

#[derive(Debug, Clone)]
pub struct DetectorRegistry {
    detectors: Vec<Detector>,
    frontmatter_re: Regex,
    any_comment_re: Regex,
}

impl DetectorRegistry {
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
            // Mermaid accepts frontmatter even when the source is indented (common in JS template
            // literals used by Cypress snapshot tests). Match and strip it with optional leading
            // whitespace on both the opening and closing `---` lines.
            frontmatter_re: Regex::new(r"(?s)^\s*-{3}\s*[\n\r](.*?)[\n\r]\s*-{3}\s*[\n\r]+")
                .unwrap(),
            any_comment_re: Regex::new(r"(?m)\s*%%.*\n").unwrap(),
        }
    }

    pub fn add(&mut self, detector: Detector) {
        self.detectors.push(detector);
    }

    pub fn add_fn(&mut self, id: &'static str, detector: DetectorFn) {
        self.add(Detector { id, detector });
    }

    pub fn detect_type(&self, text: &str, config: &mut MermaidConfig) -> Result<&'static str> {
        let no_frontmatter = self.frontmatter_re.replace(text, "");
        let no_directives = remove_directives(no_frontmatter.as_ref());
        let cleaned = self
            .any_comment_re
            .replace_all(no_directives.as_ref(), "\n");

        if let Some(id) = fast_detect_by_leading_keyword(cleaned.as_ref()) {
            return Ok(id);
        }

        for det in &self.detectors {
            if (det.detector)(cleaned.as_ref(), config) {
                return Ok(det.id);
            }
        }

        Err(DetectTypeError {
            text: cleaned.into_owned(),
        }
        .into())
    }

    /// Detects a diagram type assuming the input is already pre-cleaned:
    /// no front-matter, no directives, and no Mermaid `%%` comments.
    pub fn detect_type_precleaned(
        &self,
        text: &str,
        config: &mut MermaidConfig,
    ) -> Result<&'static str> {
        if let Some(id) = fast_detect_by_leading_keyword(text) {
            return Ok(id);
        }

        for det in &self.detectors {
            if (det.detector)(text, config) {
                return Ok(det.id);
            }
        }

        Err(DetectTypeError {
            text: text.to_string(),
        }
        .into())
    }

    pub fn default_mermaid_11_12_2_full() -> Self {
        let mut reg = Self::new();

        // The detector order is significant and mirrors Mermaid's registration order.
        reg.add_fn("error", detector_error);
        reg.add_fn("---", detector_frontmatter_unparsed);

        // Mermaid's injected.includeLargeFeatures=true ordering.
        reg.add_fn("flowchart-elk", detector_flowchart_elk);
        reg.add_fn("mindmap", detector_mindmap);
        reg.add_fn("architecture", detector_architecture);
        reg.add_fn("zenuml", detector_zenuml);

        // Mermaid's base registration order.
        reg.add_fn("c4", detector_c4);
        reg.add_fn("kanban", detector_kanban);
        reg.add_fn("classDiagram", detector_class_v2);
        reg.add_fn("class", detector_class_dagre_d3);
        reg.add_fn("er", detector_er);
        reg.add_fn("gantt", detector_gantt);
        reg.add_fn("info", detector_info);
        reg.add_fn("pie", detector_pie);
        reg.add_fn("requirement", detector_requirement);
        reg.add_fn("sequence", detector_sequence);
        reg.add_fn("flowchart-v2", detector_flowchart_v2);
        reg.add_fn("flowchart", detector_flowchart_dagre_d3_graph);
        reg.add_fn("timeline", detector_timeline);
        reg.add_fn("gitGraph", detector_git_graph);
        reg.add_fn("stateDiagram", detector_state_v2);
        reg.add_fn("state", detector_state_dagre_d3);
        reg.add_fn("journey", detector_journey);
        reg.add_fn("quadrantChart", detector_quadrant);
        reg.add_fn("sankey", detector_sankey);
        reg.add_fn("packet", detector_packet);
        reg.add_fn("xychart", detector_xychart);
        reg.add_fn("block", detector_block);
        reg.add_fn("radar", detector_radar);
        reg.add_fn("treemap", detector_treemap);

        reg
    }

    pub fn default_mermaid_11_12_2_tiny() -> Self {
        let mut reg = Self::new();

        // The detector order is significant and mirrors Mermaid's registration order.
        reg.add_fn("error", detector_error);
        reg.add_fn("---", detector_frontmatter_unparsed);

        // Mermaid's base registration order.
        reg.add_fn("zenuml", detector_zenuml);
        reg.add_fn("c4", detector_c4);
        reg.add_fn("kanban", detector_kanban);
        reg.add_fn("classDiagram", detector_class_v2);
        reg.add_fn("class", detector_class_dagre_d3);
        reg.add_fn("er", detector_er);
        reg.add_fn("gantt", detector_gantt);
        reg.add_fn("info", detector_info);
        reg.add_fn("pie", detector_pie);
        reg.add_fn("requirement", detector_requirement);
        reg.add_fn("sequence", detector_sequence);
        reg.add_fn("flowchart-v2", detector_flowchart_v2);
        reg.add_fn("flowchart", detector_flowchart_dagre_d3_graph);
        reg.add_fn("timeline", detector_timeline);
        reg.add_fn("gitGraph", detector_git_graph);
        reg.add_fn("stateDiagram", detector_state_v2);
        reg.add_fn("state", detector_state_dagre_d3);
        reg.add_fn("journey", detector_journey);
        reg.add_fn("quadrantChart", detector_quadrant);
        reg.add_fn("sankey", detector_sankey);
        reg.add_fn("packet", detector_packet);
        reg.add_fn("xychart", detector_xychart);
        reg.add_fn("block", detector_block);
        reg.add_fn("radar", detector_radar);
        reg.add_fn("treemap", detector_treemap);

        reg
    }

    #[cfg(feature = "large-features")]
    pub fn default_mermaid_11_12_2() -> Self {
        Self::default_mermaid_11_12_2_full()
    }

    #[cfg(not(feature = "large-features"))]
    pub fn default_mermaid_11_12_2() -> Self {
        Self::default_mermaid_11_12_2_tiny()
    }
}

fn fast_detect_by_leading_keyword(text: &str) -> Option<&'static str> {
    fn has_boundary(rest: &str) -> bool {
        rest.is_empty()
            || rest
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace() || c == ';')
    }

    let t = text.trim_start();

    // Prefer a fast string-prefix check for common "keyword header" diagrams.
    // This avoids running dozens of regex detectors for tiny fixtures.
    if let Some(rest) = t.strip_prefix("sequenceDiagram") {
        return has_boundary(rest).then_some("sequence");
    }
    if let Some(rest) = t.strip_prefix("classDiagram") {
        return has_boundary(rest).then_some("classDiagram");
    }
    if let Some(rest) = t.strip_prefix("stateDiagram") {
        return has_boundary(rest).then_some("stateDiagram");
    }
    if let Some(rest) = t.strip_prefix("mindmap") {
        return has_boundary(rest).then_some("mindmap");
    }
    if let Some(rest) = t.strip_prefix("architecture") {
        return has_boundary(rest).then_some("architecture");
    }
    if let Some(rest) = t.strip_prefix("erDiagram") {
        return has_boundary(rest).then_some("er");
    }
    if let Some(rest) = t.strip_prefix("gantt") {
        return has_boundary(rest).then_some("gantt");
    }
    if let Some(rest) = t.strip_prefix("timeline") {
        return has_boundary(rest).then_some("timeline");
    }
    if let Some(rest) = t.strip_prefix("journey") {
        return has_boundary(rest).then_some("journey");
    }
    if let Some(rest) = t.strip_prefix("gitGraph") {
        return has_boundary(rest).then_some("gitGraph");
    }
    if let Some(rest) = t.strip_prefix("quadrantChart") {
        return has_boundary(rest).then_some("quadrantChart");
    }
    if let Some(rest) = t.strip_prefix("packet-beta") {
        return has_boundary(rest).then_some("packet");
    }
    if let Some(rest) = t.strip_prefix("xychart-beta") {
        return has_boundary(rest).then_some("xychart");
    }

    None
}

fn remove_directives(text: &str) -> Cow<'_, str> {
    if !text.contains("%%{") {
        return Cow::Borrowed(text);
    }

    let mut out = String::with_capacity(text.len());
    let mut pos = 0;
    while let Some(rel) = text[pos..].find("%%{") {
        let start = pos + rel;
        out.push_str(&text[pos..start]);
        let after_start = start + 3;
        if let Some(rel_end) = text[after_start..].find("}%%") {
            let end = after_start + rel_end + 3;
            pos = end;
        } else {
            return Cow::Owned(out);
        }
    }
    out.push_str(&text[pos..]);
    Cow::Owned(out)
}

cached_regex!(
    re_c4,
    r"^\s*C4Context|C4Container|C4Component|C4Dynamic|C4Deployment"
);

impl Default for DetectorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn detector_frontmatter_unparsed(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("---")
}

fn detector_error(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim().eq_ignore_ascii_case("error")
}

fn detector_c4(txt: &str, _config: &mut MermaidConfig) -> bool {
    // Matches Mermaid's upstream regex exactly (note the missing grouping in JS).
    re_c4().is_match(txt)
}

fn detector_kanban(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("kanban")
}

fn detector_class_dagre_d3(txt: &str, config: &mut MermaidConfig) -> bool {
    if config.get_str("class.defaultRenderer") == Some("dagre-wrapper") {
        return false;
    }
    txt.trim_start().starts_with("classDiagram")
}

fn detector_class_v2(txt: &str, config: &mut MermaidConfig) -> bool {
    if txt.trim_start().starts_with("classDiagram")
        && config.get_str("class.defaultRenderer") == Some("dagre-wrapper")
    {
        return true;
    }
    txt.trim_start().starts_with("classDiagram-v2")
}

fn detector_er(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("erDiagram")
}

fn detector_gantt(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("gantt")
}

fn detector_info(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("info")
}

fn detector_pie(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("pie")
}

fn detector_requirement(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("requirement")
}

fn detector_sequence(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("sequenceDiagram")
}

fn detector_flowchart_elk(txt: &str, config: &mut MermaidConfig) -> bool {
    let trimmed = txt.trim_start();
    if trimmed.starts_with("flowchart-elk")
        || ((trimmed.starts_with("flowchart") || trimmed.starts_with("graph"))
            && config.get_str("flowchart.defaultRenderer") == Some("elk"))
    {
        config.set_value("layout", serde_json::Value::String("elk".to_string()));
        return true;
    }
    false
}

fn detector_flowchart_v2(txt: &str, config: &mut MermaidConfig) -> bool {
    if config.get_str("flowchart.defaultRenderer") == Some("dagre-d3") {
        return false;
    }
    if config.get_str("flowchart.defaultRenderer") == Some("elk") {
        config.set_value("layout", serde_json::Value::String("elk".to_string()));
    }

    if txt.trim_start().starts_with("graph")
        && config.get_str("flowchart.defaultRenderer") == Some("dagre-wrapper")
    {
        return true;
    }
    txt.trim_start().starts_with("flowchart")
}

fn detector_flowchart_dagre_d3_graph(txt: &str, config: &mut MermaidConfig) -> bool {
    if matches!(
        config.get_str("flowchart.defaultRenderer"),
        Some("dagre-wrapper" | "elk")
    ) {
        return false;
    }
    txt.trim_start().starts_with("graph")
}

fn detector_timeline(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("timeline")
}

fn detector_git_graph(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("gitGraph")
}

fn detector_state_dagre_d3(txt: &str, config: &mut MermaidConfig) -> bool {
    if config.get_str("state.defaultRenderer") == Some("dagre-wrapper") {
        return false;
    }
    txt.trim_start().starts_with("stateDiagram")
}

fn detector_state_v2(txt: &str, config: &mut MermaidConfig) -> bool {
    let trimmed = txt.trim_start();
    if trimmed.starts_with("stateDiagram-v2") {
        return true;
    }
    trimmed.starts_with("stateDiagram")
        && config.get_str("state.defaultRenderer") == Some("dagre-wrapper")
}

fn detector_journey(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("journey")
}

fn detector_quadrant(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("quadrantChart")
}

fn detector_sankey(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("sankey")
}

fn detector_packet(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("packet")
}

fn detector_xychart(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("xychart")
}

fn detector_block(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("block")
}

fn detector_radar(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("radar-beta")
}

fn detector_treemap(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("treemap")
}

fn detector_mindmap(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("mindmap")
}

fn detector_architecture(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("architecture")
}

fn detector_zenuml(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.trim_start().starts_with("zenuml")
}

#[cfg(test)]
mod remove_directives_tests {
    use super::remove_directives;
    use std::borrow::Cow;

    #[test]
    fn no_directives_is_borrowed() {
        let s = "flowchart TD; A-->B;";
        assert!(matches!(remove_directives(s), Cow::Borrowed(_)));
    }

    #[test]
    fn removes_directive_block() {
        let s = "%%{init: {\"theme\": \"dark\"}}%%\nflowchart TD; A-->B;";
        let out = remove_directives(s);
        assert!(out.as_ref().contains("flowchart TD"));
        assert!(!out.as_ref().contains("init"));
    }

    #[test]
    fn unterminated_directive_truncates_at_start() {
        let s = "flowchart\n%%{init: {\"theme\": \"dark\"}}\nA-->B;";
        let out = remove_directives(s);
        assert_eq!(out.as_ref().trim(), "flowchart");
    }
}
