use crate::*;
use futures::executor::block_on;
use serde_json::json;

#[test]
fn parse_diagram_pie_basic() {
    let engine = Engine::new();
    let text = r#"pie showData
 "Cats": 2
 'Dogs': 3
 "#;
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.meta.diagram_type, "pie");
    assert_eq!(
        res.model,
        json!({
            "type": "pie",
            "showData": true,
            "title": null,
            "accTitle": null,
            "accDescr": null,
            "sections": [
                { "label": "Cats", "value": 2.0 },
                { "label": "Dogs", "value": 3.0 }
            ]
        })
    );
}

#[test]
fn parse_diagram_pie_rejects_negative_slice_values_like_upstream() {
    let engine = Engine::new();
    let text = r#"pie title Default text position: Animal adoption
         "dogs" : -60.67
        "rats" : 40.12
        "#;
    let err = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap_err()
        .to_string();
    assert_eq!(
        err,
        "Diagram parse error (pie): \"dogs\" has invalid value: -60.67. Negative values are not allowed in pie charts. All slice values must be >= 0."
    );
}
