use super::*;
use crate::{MermaidConfig, ParseMetadata};

fn meta() -> ParseMetadata {
    ParseMetadata {
        diagram_type: "classDiagram".to_string(),
        config: MermaidConfig::default(),
        effective_config: MermaidConfig::default(),
        title: None,
    }
}

#[test]
fn fast_parser_matches_lalrpop_for_basic_class_diagram() {
    let code = r#"classDiagram
class C1 {
  +String field1
  +method1()
}
C1 <|-- C2 : inherits
"#;
    let meta = meta();
    let slow = parse::parse_class_via_lalrpop(code, &meta).expect("slow parse");
    let fast = fast::parse_class_fast_db(code, &meta)
        .expect("fast parse")
        .expect("fast parser applicable")
        .into_model(&meta);
    assert_eq!(fast, slow);
}
