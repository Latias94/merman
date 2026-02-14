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
cached_regex!(re_kanban, r"^\s*kanban");
cached_regex!(re_class_diagram, r"^\s*classDiagram");
cached_regex!(re_class_diagram_v2, r"^\s*classDiagram-v2");
cached_regex!(re_er_diagram, r"^\s*erDiagram");
cached_regex!(re_gantt, r"^\s*gantt");
cached_regex!(re_info, r"^\s*info");
cached_regex!(re_pie, r"^\s*pie");
cached_regex!(re_requirement, r"^\s*requirement(Diagram)?");
cached_regex!(re_sequence_diagram, r"^\s*sequenceDiagram");
cached_regex!(re_flowchart_elk, r"^\s*flowchart-elk");
cached_regex!(re_flowchart_or_graph, r"^\s*(flowchart|graph)");
cached_regex!(re_graph, r"^\s*graph");
cached_regex!(re_flowchart, r"^\s*flowchart");
cached_regex!(re_timeline, r"^\s*timeline");
cached_regex!(re_git_graph, r"^\s*gitGraph");
cached_regex!(re_state_diagram_v2, r"^\s*stateDiagram-v2");
cached_regex!(re_state_diagram, r"^\s*stateDiagram");
cached_regex!(re_journey, r"^\s*journey");
cached_regex!(re_quadrant_chart, r"^\s*quadrantChart");
cached_regex!(re_sankey, r"^\s*sankey(-beta)?");
cached_regex!(re_packet, r"^\s*packet(-beta)?");
cached_regex!(re_xychart, r"^\s*xychart(-beta)?");
cached_regex!(re_block, r"^\s*block(-beta)?");
cached_regex!(re_radar, r"^\s*radar-beta");
cached_regex!(re_treemap, r"^\s*treemap");
cached_regex!(re_mindmap, r"^\s*mindmap");
cached_regex!(re_architecture, r"^\s*architecture");
cached_regex!(re_zenuml, r"^\s*zenuml");

impl Default for DetectorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn detector_frontmatter_unparsed(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.to_lowercase().trim_start().starts_with("---")
}

fn detector_error(txt: &str, _config: &mut MermaidConfig) -> bool {
    txt.to_lowercase().trim() == "error"
}

fn detector_c4(txt: &str, _config: &mut MermaidConfig) -> bool {
    // Matches Mermaid's upstream regex exactly (note the missing grouping in JS).
    re_c4().is_match(txt)
}

fn detector_kanban(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_kanban().is_match(txt)
}

fn detector_class_dagre_d3(txt: &str, config: &mut MermaidConfig) -> bool {
    if config.get_str("class.defaultRenderer") == Some("dagre-wrapper") {
        return false;
    }
    re_class_diagram().is_match(txt)
}

fn detector_class_v2(txt: &str, config: &mut MermaidConfig) -> bool {
    if re_class_diagram().is_match(txt)
        && config.get_str("class.defaultRenderer") == Some("dagre-wrapper")
    {
        return true;
    }
    re_class_diagram_v2().is_match(txt)
}

fn detector_er(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_er_diagram().is_match(txt)
}

fn detector_gantt(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_gantt().is_match(txt)
}

fn detector_info(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_info().is_match(txt)
}

fn detector_pie(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_pie().is_match(txt)
}

fn detector_requirement(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_requirement().is_match(txt)
}

fn detector_sequence(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_sequence_diagram().is_match(txt)
}

fn detector_flowchart_elk(txt: &str, config: &mut MermaidConfig) -> bool {
    if re_flowchart_elk().is_match(txt)
        || (re_flowchart_or_graph().is_match(txt)
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

    if re_graph().is_match(txt)
        && config.get_str("flowchart.defaultRenderer") == Some("dagre-wrapper")
    {
        return true;
    }
    re_flowchart().is_match(txt)
}

fn detector_flowchart_dagre_d3_graph(txt: &str, config: &mut MermaidConfig) -> bool {
    if matches!(
        config.get_str("flowchart.defaultRenderer"),
        Some("dagre-wrapper" | "elk")
    ) {
        return false;
    }
    re_graph().is_match(txt)
}

fn detector_timeline(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_timeline().is_match(txt)
}

fn detector_git_graph(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_git_graph().is_match(txt)
}

fn detector_state_dagre_d3(txt: &str, config: &mut MermaidConfig) -> bool {
    if config.get_str("state.defaultRenderer") == Some("dagre-wrapper") {
        return false;
    }
    re_state_diagram().is_match(txt)
}

fn detector_state_v2(txt: &str, config: &mut MermaidConfig) -> bool {
    if re_state_diagram_v2().is_match(txt) {
        return true;
    }
    re_state_diagram().is_match(txt)
        && config.get_str("state.defaultRenderer") == Some("dagre-wrapper")
}

fn detector_journey(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_journey().is_match(txt)
}

fn detector_quadrant(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_quadrant_chart().is_match(txt)
}

fn detector_sankey(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_sankey().is_match(txt)
}

fn detector_packet(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_packet().is_match(txt)
}

fn detector_xychart(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_xychart().is_match(txt)
}

fn detector_block(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_block().is_match(txt)
}

fn detector_radar(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_radar().is_match(txt)
}

fn detector_treemap(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_treemap().is_match(txt)
}

fn detector_mindmap(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_mindmap().is_match(txt)
}

fn detector_architecture(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_architecture().is_match(txt)
}

fn detector_zenuml(txt: &str, _config: &mut MermaidConfig) -> bool {
    re_zenuml().is_match(txt)
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
