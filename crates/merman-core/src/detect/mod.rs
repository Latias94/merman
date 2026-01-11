use crate::{MermaidConfig, Result};
use regex::Regex;

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
            frontmatter_re: Regex::new(r"(?s)^-{3}\s*[\n\r](.*?)[\n\r]-{3}\s*[\n\r]+").unwrap(),
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
        let no_frontmatter = self.frontmatter_re.replace(text, "").to_string();
        let no_directives = remove_directives(&no_frontmatter);
        let cleaned = self
            .any_comment_re
            .replace_all(&no_directives, "\n")
            .to_string();

        for det in &self.detectors {
            if (det.detector)(&cleaned, config) {
                return Ok(det.id);
            }
        }

        Err(DetectTypeError { text: cleaned }.into())
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

        // Mermaid's base registration order.
        reg.add_fn("c4", detector_c4);
        reg.add_fn("kanban", detector_kanban);
        reg.add_fn("classDiagram", detector_class_v2);
        reg.add_fn("class", detector_class_legacy);
        reg.add_fn("er", detector_er);
        reg.add_fn("gantt", detector_gantt);
        reg.add_fn("info", detector_info);
        reg.add_fn("pie", detector_pie);
        reg.add_fn("requirement", detector_requirement);
        reg.add_fn("sequence", detector_sequence);
        reg.add_fn("flowchart-v2", detector_flowchart_v2);
        reg.add_fn("flowchart", detector_flowchart_legacy);
        reg.add_fn("timeline", detector_timeline);
        reg.add_fn("gitGraph", detector_git_graph);
        reg.add_fn("stateDiagram", detector_state_v2);
        reg.add_fn("state", detector_state_legacy);
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
        reg.add_fn("c4", detector_c4);
        reg.add_fn("kanban", detector_kanban);
        reg.add_fn("classDiagram", detector_class_v2);
        reg.add_fn("class", detector_class_legacy);
        reg.add_fn("er", detector_er);
        reg.add_fn("gantt", detector_gantt);
        reg.add_fn("info", detector_info);
        reg.add_fn("pie", detector_pie);
        reg.add_fn("requirement", detector_requirement);
        reg.add_fn("sequence", detector_sequence);
        reg.add_fn("flowchart-v2", detector_flowchart_v2);
        reg.add_fn("flowchart", detector_flowchart_legacy);
        reg.add_fn("timeline", detector_timeline);
        reg.add_fn("gitGraph", detector_git_graph);
        reg.add_fn("stateDiagram", detector_state_v2);
        reg.add_fn("state", detector_state_legacy);
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

fn remove_directives(text: &str) -> String {
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
            return out;
        }
    }
    out.push_str(&text[pos..]);
    out
}

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
    Regex::new(r"^\s*C4Context|C4Container|C4Component|C4Dynamic|C4Deployment")
        .unwrap()
        .is_match(txt)
}

fn detector_kanban(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*kanban").unwrap().is_match(txt)
}

fn detector_class_legacy(txt: &str, config: &mut MermaidConfig) -> bool {
    if config.get_str("class.defaultRenderer") == Some("dagre-wrapper") {
        return false;
    }
    Regex::new(r"^\s*classDiagram").unwrap().is_match(txt)
}

fn detector_class_v2(txt: &str, config: &mut MermaidConfig) -> bool {
    if Regex::new(r"^\s*classDiagram").unwrap().is_match(txt)
        && config.get_str("class.defaultRenderer") == Some("dagre-wrapper")
    {
        return true;
    }
    Regex::new(r"^\s*classDiagram-v2").unwrap().is_match(txt)
}

fn detector_er(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*erDiagram").unwrap().is_match(txt)
}

fn detector_gantt(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*gantt").unwrap().is_match(txt)
}

fn detector_info(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*info").unwrap().is_match(txt)
}

fn detector_pie(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*pie").unwrap().is_match(txt)
}

fn detector_requirement(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*requirement(Diagram)?")
        .unwrap()
        .is_match(txt)
}

fn detector_sequence(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*sequenceDiagram").unwrap().is_match(txt)
}

fn detector_flowchart_elk(txt: &str, config: &mut MermaidConfig) -> bool {
    if Regex::new(r"^\s*flowchart-elk").unwrap().is_match(txt)
        || (Regex::new(r"^\s*(flowchart|graph)").unwrap().is_match(txt)
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

    if Regex::new(r"^\s*graph").unwrap().is_match(txt)
        && config.get_str("flowchart.defaultRenderer") == Some("dagre-wrapper")
    {
        return true;
    }
    Regex::new(r"^\s*flowchart").unwrap().is_match(txt)
}

fn detector_flowchart_legacy(txt: &str, config: &mut MermaidConfig) -> bool {
    if matches!(
        config.get_str("flowchart.defaultRenderer"),
        Some("dagre-wrapper" | "elk")
    ) {
        return false;
    }
    Regex::new(r"^\s*graph").unwrap().is_match(txt)
}

fn detector_timeline(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*timeline").unwrap().is_match(txt)
}

fn detector_git_graph(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*gitGraph").unwrap().is_match(txt)
}

fn detector_state_legacy(txt: &str, config: &mut MermaidConfig) -> bool {
    if config.get_str("state.defaultRenderer") == Some("dagre-wrapper") {
        return false;
    }
    Regex::new(r"^\s*stateDiagram").unwrap().is_match(txt)
}

fn detector_state_v2(txt: &str, config: &mut MermaidConfig) -> bool {
    if Regex::new(r"^\s*stateDiagram-v2").unwrap().is_match(txt) {
        return true;
    }
    Regex::new(r"^\s*stateDiagram").unwrap().is_match(txt)
        && config.get_str("state.defaultRenderer") == Some("dagre-wrapper")
}

fn detector_journey(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*journey").unwrap().is_match(txt)
}

fn detector_quadrant(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*quadrantChart").unwrap().is_match(txt)
}

fn detector_sankey(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*sankey(-beta)?").unwrap().is_match(txt)
}

fn detector_packet(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*packet(-beta)?").unwrap().is_match(txt)
}

fn detector_xychart(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*xychart(-beta)?").unwrap().is_match(txt)
}

fn detector_block(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*block(-beta)?").unwrap().is_match(txt)
}

fn detector_radar(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*radar-beta").unwrap().is_match(txt)
}

fn detector_treemap(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*treemap").unwrap().is_match(txt)
}

fn detector_mindmap(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*mindmap").unwrap().is_match(txt)
}

fn detector_architecture(txt: &str, _config: &mut MermaidConfig) -> bool {
    Regex::new(r"^\s*architecture").unwrap().is_match(txt)
}
