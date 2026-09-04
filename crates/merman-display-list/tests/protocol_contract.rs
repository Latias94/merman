mod common;

use common::{extended_document, sample_document};
use merman_display_list::DrawingListDocument;
use serde_json::{Value, json};

#[test]
fn canonical_json_round_trips_and_uses_a_stable_resource_order() {
    let document = sample_document();
    document.validate().expect("fixture document is valid");

    let canonical = document
        .canonical_json_bytes()
        .expect("fixture document serializes canonically");
    let decoded = DrawingListDocument::from_json_bytes(&canonical)
        .expect("canonical DrawingList JSON decodes");
    assert_eq!(decoded.canonical_json_bytes().unwrap(), canonical);

    let mut differently_inserted = document.clone();
    differently_inserted.resources.reverse();
    assert_eq!(
        differently_inserted.canonical_json_bytes().unwrap(),
        canonical,
        "resource insertion order is not wire-significant"
    );
}

#[test]
fn published_draft_2020_12_schema_accepts_the_runtime_document() {
    let schema: Value =
        serde_json::from_str(include_str!("../../../contracts/drawing-list-v1.json"))
            .expect("DrawingList schema is valid JSON");
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&schema)
        .expect("DrawingList schema is valid Draft 2020-12");
    let value = serde_json::to_value(sample_document()).expect("fixture serializes");

    assert!(
        validator.is_valid(&value),
        "published schema rejected runtime document: {value}"
    );

    let mut unknown_command = value.clone();
    unknown_command["commands"][0] = json!({ "kind": "future_visual_command" });
    assert!(!validator.is_valid(&unknown_command));

    let mut unknown_extension = value;
    unknown_extension["extensions"] = json!({ "future": true });
    assert!(!validator.is_valid(&unknown_extension));

    let extended = serde_json::to_value(extended_document()).expect("extended fixture serializes");
    assert!(
        validator.is_valid(&extended),
        "published schema rejected extended runtime document: {extended}"
    );

    let mut invalid_path = serde_json::to_value(sample_document()).expect("fixture serializes");
    invalid_path["resources"][0]["segments"][0] =
        json!({ "kind": "line_to", "to": { "x": 0.0, "y": 0.0 } });
    assert!(!validator.is_valid(&invalid_path));

    let mut invalid_pattern = extended.clone();
    let pattern = invalid_pattern["resources"]
        .as_array_mut()
        .expect("resources are an array")
        .iter_mut()
        .find(|resource| resource["kind"] == "pattern")
        .expect("extended fixture has a pattern");
    pattern["tile"]["width"] = json!(0.0);
    assert!(!validator.is_valid(&invalid_pattern));

    let mut invalid_text = serde_json::to_value(sample_document()).expect("fixture serializes");
    invalid_text["commands"][5]["run"]["language"] = json!("");
    assert!(!validator.is_valid(&invalid_text));

    let mut invalid_postscript =
        serde_json::to_value(sample_document()).expect("fixture serializes");
    invalid_postscript["commands"][5]["run"]["style"]["font"]["postscript_name"] = json!("");
    assert!(!validator.is_valid(&invalid_postscript));

    let mut invalid_font = serde_json::to_value(extended_document()).expect("fixture serializes");
    let font = invalid_font["resources"]
        .as_array_mut()
        .expect("resources are an array")
        .iter_mut()
        .find(|resource| resource["kind"] == "font")
        .expect("extended fixture has a font");
    font["font"]["media_type"] = json!("application/octet-stream");
    assert!(!validator.is_valid(&invalid_font));

    let mut uppercase_font = serde_json::to_value(extended_document()).expect("fixture serializes");
    let font = uppercase_font["resources"]
        .as_array_mut()
        .expect("resources are an array")
        .iter_mut()
        .find(|resource| resource["kind"] == "font")
        .expect("extended fixture has a font");
    font["font"]["media_type"] = json!("FONT/WoFf2; charset=binary");
    assert!(validator.is_valid(&uppercase_font));
}
