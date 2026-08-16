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
fn authoritative_parser_preserves_the_former_fast_subset() {
    let code = r#"classDiagram
class C1 {
  +String field1
  +method1()
}
C1 <|-- C2 : inherits
"#;
    let meta = meta();
    let compat = parse::parse_class(code, &meta).expect("compat parse");
    let typed = parse::parse_class_typed(code, &meta).expect("typed parse");
    assert_eq!(compat, render_model_to_compat_json(&typed, &meta).unwrap());
}

#[test]
fn namespace_qualified_relation_endpoints_create_facade_classes_like_mermaid() {
    let code = r#"classDiagram
namespace Platform["Platform Layer"] {
  namespace FFI {
    class DartBinding
    class PythonBinding
  }
  namespace Core {
    class Renderer
  }
}
Platform.FFI.DartBinding --> Platform.Core.Renderer : calls
Platform.FFI.PythonBinding --> Platform.Core.Renderer : calls
"#;

    let model = parse::parse_class_typed(code, &meta()).expect("class diagram should parse");

    assert_eq!(
        model.classes.keys().cloned().collect::<Vec<_>>(),
        vec![
            "DartBinding",
            "PythonBinding",
            "Renderer",
            "Platform.FFI.DartBinding",
            "Platform.Core.Renderer",
            "Platform.FFI.PythonBinding"
        ]
    );
    assert_eq!(model.relations[0].id1, "Platform.FFI.DartBinding");
    assert_eq!(model.relations[0].id2, "Platform.Core.Renderer");
    assert_eq!(model.relations[1].id1, "Platform.FFI.PythonBinding");
    assert_eq!(model.relations[1].id2, "Platform.Core.Renderer");
    assert_eq!(
        model.namespaces["Platform.FFI"].class_ids,
        vec!["DartBinding", "PythonBinding"]
    );
    assert_eq!(
        model.namespaces["Platform.Core"].class_ids,
        vec!["Renderer"]
    );
    assert_eq!(
        model.namespace_facade_aliases,
        std::collections::BTreeMap::from([
            ("Platform.Core.Renderer".to_string(), "Renderer".to_string(),),
            (
                "Platform.FFI.DartBinding".to_string(),
                "DartBinding".to_string(),
            ),
            (
                "Platform.FFI.PythonBinding".to_string(),
                "PythonBinding".to_string(),
            ),
        ])
    );
}

#[test]
fn explicit_qualified_class_is_not_a_synthetic_namespace_facade() {
    let code = r#"classDiagram
namespace N {
  class C
}
class N.C["Distinct"]
class D
N.C --> D
"#;

    let model = parse::parse_class_typed(code, &meta()).expect("class diagram should parse");

    assert!(model.namespace_facade_aliases.is_empty());
    assert_eq!(model.classes["N.C"].text, "Distinct");
    assert_eq!(model.relations[0].id1, "N.C");
}
