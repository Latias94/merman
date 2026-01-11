use crate::{Error, ParseMetadata, Result};
use serde_json::Value;

pub type DiagramSemanticParser = fn(code: &str, meta: &ParseMetadata) -> Result<Value>;

#[derive(Debug, Clone, Default)]
pub struct DiagramRegistry {
    parsers: std::collections::HashMap<&'static str, DiagramSemanticParser>,
}

impl DiagramRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, diagram_type: &'static str, parser: DiagramSemanticParser) {
        self.parsers.insert(diagram_type, parser);
    }

    pub fn get(&self, diagram_type: &str) -> Option<DiagramSemanticParser> {
        self.parsers.get(diagram_type).copied()
    }

    pub fn default_mermaid_11_12_2() -> Self {
        let mut reg = Self::new();

        reg.insert("flowchart-v2", crate::diagrams::flowchart::parse_flowchart);
        reg.insert("flowchart", crate::diagrams::flowchart::parse_flowchart);
        reg.insert("flowchart-elk", crate::diagrams::flowchart::parse_flowchart);

        reg.insert("info", crate::diagrams::info::parse_info);
        reg.insert("pie", crate::diagrams::pie::parse_pie);
        reg.insert("sequence", crate::diagrams::sequence::parse_sequence);

        reg.insert("classDiagram", crate::diagrams::class::parse_class);
        reg.insert("class", crate::diagrams::class::parse_class);

        reg.insert("er", crate::diagrams::er::parse_er);
        reg.insert("erDiagram", crate::diagrams::er::parse_er);

        reg.insert("stateDiagram", crate::diagrams::state::parse_state);
        reg.insert("state", crate::diagrams::state::parse_state);

        reg.insert("mindmap", crate::diagrams::mindmap::parse_mindmap);
        reg.insert("gantt", crate::diagrams::gantt::parse_gantt);
        reg.insert("timeline", crate::diagrams::timeline::parse_timeline);

        reg
    }
}

#[derive(Debug, Clone)]
pub struct ParsedDiagram {
    pub meta: ParseMetadata,
    pub model: Value,
}

pub fn parse_or_unsupported(
    registry: &DiagramRegistry,
    diagram_type: &str,
    code: &str,
    meta: &ParseMetadata,
) -> Result<Value> {
    let Some(parser) = registry.get(diagram_type) else {
        return Err(Error::UnsupportedDiagram {
            diagram_type: diagram_type.to_string(),
        });
    };
    parser(code, meta)
}
