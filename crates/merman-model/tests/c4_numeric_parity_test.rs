#![cfg(feature = "diagram-c4")]

use merman_model::families::c4::{C4DiagramModel, C4LayoutConfig, C4Relation, C4RelationOffset};
use serde_json::{Value, json};

fn model_with_number(value: f64) -> C4DiagramModel {
    C4DiagramModel {
        c4_type: "C4Context".to_string(),
        title: None,
        acc_title: None,
        acc_descr: None,
        wrap: false,
        layout: C4LayoutConfig {
            c4_shape_in_row: value,
            c4_boundary_in_row: value,
        },
        shapes: Vec::new(),
        boundaries: Vec::new(),
        rels: vec![C4Relation {
            from_alias: "a".to_string(),
            to_alias: "b".to_string(),
            rel_type: "Rel".to_string(),
            offset_x: Some(C4RelationOffset::Number(value)),
            offset_y: Some(C4RelationOffset::Number(value)),
            ..C4Relation::default()
        }],
    }
}

#[test]
fn c4_canonical_numbers_survive_value_and_text_round_trips() {
    for value in [
        0.0,
        -0.0,
        1.0,
        18_446_744_073_709_551_616.0,
        f64::MAX,
        f64::from_bits(1),
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ] {
        let model = model_with_number(value);
        let wire = serde_json::to_value(&model).unwrap();
        for actual in [
            &wire["layout"]["c4ShapeInRow"],
            &wire["layout"]["c4BoundaryInRow"],
        ] {
            if value.is_nan() {
                assert_eq!(actual, "NaN");
            } else if value == f64::INFINITY {
                assert_eq!(actual, "Infinity");
            } else if value == f64::NEG_INFINITY {
                assert_eq!(actual, "-Infinity");
            } else {
                assert_eq!(actual.as_f64().unwrap().to_bits(), value.to_bits());
            }
        }
        for field in ["offsetX", "offsetY"] {
            let actual = &wire["rels"][0][field];
            if value.is_finite() {
                assert_eq!(actual, &serde_json::to_value(value).unwrap());
                assert_eq!(actual.as_f64().unwrap().to_bits(), value.to_bits());
            } else {
                assert_eq!(actual, &json!({"number": wire["layout"]["c4ShapeInRow"]}));
            }
        }
        let text = serde_json::to_string(&model).unwrap();
        for restored in [
            serde_json::from_value::<C4DiagramModel>(wire).unwrap(),
            serde_json::from_str::<C4DiagramModel>(&text).unwrap(),
        ] {
            let relation = &restored.rels[0];
            let (Some(C4RelationOffset::Number(x)), Some(C4RelationOffset::Number(y))) =
                (&relation.offset_x, &relation.offset_y)
            else {
                panic!("numeric offsets must retain their variant and presence");
            };
            for actual in [
                restored.layout.c4_shape_in_row,
                restored.layout.c4_boundary_in_row,
                *x,
                *y,
            ] {
                if value.is_nan() {
                    assert!(actual.is_nan());
                } else {
                    assert_eq!(actual.to_bits(), value.to_bits());
                }
            }
        }
    }
    assert_eq!(
        serde_json::to_string(&C4RelationOffset::Number(1.0)).unwrap(),
        "1.0"
    );
}

#[test]
fn c4_literal_offsets_keep_strings_including_nonfinite_number_names() {
    for literal in [
        "NaN",
        "Infinity",
        "-Infinity",
        "1",
        "x",
        r#"{"number":"NaN"}"#,
    ] {
        let offset = C4RelationOffset::String(literal.to_string());
        let wire = serde_json::to_value(&offset).unwrap();
        assert_eq!(wire, literal);
        let text = serde_json::to_string(&offset).unwrap();
        for restored in [
            serde_json::from_value::<C4RelationOffset>(wire).unwrap(),
            serde_json::from_str::<C4RelationOffset>(&text).unwrap(),
        ] {
            assert_eq!(restored, offset);
        }
    }
}

#[test]
fn c4_optional_offsets_retain_absence_and_layout_defaults_remain_available() {
    for wire in [
        json!({"from": "a", "to": "b", "type": "Rel"}),
        json!({"from": "a", "to": "b", "type": "Rel", "offsetX": null, "offsetY": null}),
    ] {
        let text = wire.to_string();
        for relation in [
            serde_json::from_value::<C4Relation>(wire).unwrap(),
            serde_json::from_str::<C4Relation>(&text).unwrap(),
        ] {
            assert_eq!(relation.offset_x, None);
            assert_eq!(relation.offset_y, None);
            let wire = serde_json::to_value(relation).unwrap();
            assert!(wire.get("offsetX").is_none());
            assert!(wire.get("offsetY").is_none());
        }
    }
    assert_eq!(
        serde_json::from_str::<C4LayoutConfig>("{}").unwrap(),
        C4LayoutConfig::default()
    );
}

#[test]
fn c4_canonical_imports_reject_null_required_numbers_and_invalid_number_objects() {
    for field in ["c4ShapeInRow", "c4BoundaryInRow"] {
        for invalid in [Value::Null, json!(true), json!("1"), json!("inf")] {
            let mut wire = json!({});
            wire[field] = invalid;
            assert!(serde_json::from_str::<C4LayoutConfig>(&wire.to_string()).is_err());
            assert!(serde_json::from_value::<C4LayoutConfig>(wire).is_err());
        }
    }
    for invalid in [
        Value::Null,
        json!(true),
        json!(["NaN"]),
        json!({}),
        json!({"number": null}),
        json!({"number": 1}),
        json!({"number": "1"}),
        json!({"number": "+Infinity"}),
        json!({"number": "inf"}),
        json!({"number": "NaN", "extra": 1}),
    ] {
        assert!(serde_json::from_str::<C4RelationOffset>(&invalid.to_string()).is_err());
        assert!(serde_json::from_value::<C4RelationOffset>(invalid).is_err());
    }
}

#[cfg(feature = "compat-json")]
#[test]
fn c4_compatibility_json_projects_numeric_states_to_null_and_preserves_strings() {
    use merman_model::MermaidConfig;
    use merman_model::families::c4::compat;
    use merman_model::presentation::compat::CompatJsonMetadata;

    let config = MermaidConfig::empty_object();
    let metadata = CompatJsonMetadata {
        diagram_type: "c4",
        config: &config,
        authored_config: &config,
        title: None,
    };
    for (value, literal) in [
        (1.0, "1"),
        (f64::INFINITY, "Infinity"),
        (f64::NEG_INFINITY, "-Infinity"),
        (f64::NAN, "NaN"),
    ] {
        let mut model = model_with_number(value);
        model.rels.push(C4Relation {
            offset_x: Some(C4RelationOffset::String(literal.to_string())),
            offset_y: Some(C4RelationOffset::String(literal.to_string())),
            ..C4Relation::default()
        });
        let projected = compat::project(&model, &metadata).unwrap();
        for actual in [
            &projected["layout"]["c4ShapeInRow"],
            &projected["layout"]["c4BoundaryInRow"],
            &projected["rels"][0]["offsetX"],
            &projected["rels"][0]["offsetY"],
        ] {
            if value.is_finite() {
                assert_eq!(actual.as_f64().unwrap().to_bits(), value.to_bits());
            } else {
                assert!(actual.is_null());
            }
        }
        assert_eq!(projected["rels"][1]["offsetX"], literal);
        assert_eq!(projected["rels"][1]["offsetY"], literal);
    }
}
