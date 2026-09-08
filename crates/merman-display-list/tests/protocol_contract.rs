mod common;

use common::{extended_document, sample_document};
use merman_display_list::{
    DRAWING_LIST_MAX_EXTENSION_DEPTH, DrawingCommand, DrawingListDocument, DrawingListLimits,
    MeasurementProvenance, Paint, ResourceId, StrokeStyle, TextBaseline, TextPaintOrder,
};
use serde_json::{Map, Value, json};

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
fn canonical_json_preserves_exact_f64_coordinates() {
    for coordinate in [51.248178375505404, -51.248178375505404] {
        let mut document = sample_document();
        document.viewport.bounds.x = coordinate;

        let canonical = document
            .canonical_json_bytes()
            .expect("finite viewport coordinate serializes canonically");
        let decoded = DrawingListDocument::from_json_bytes(&canonical)
            .expect("canonical DrawingList JSON decodes");

        assert_eq!(
            decoded.viewport.bounds.x.to_bits(),
            coordinate.to_bits(),
            "decoding must preserve the exact f64 coordinate {coordinate}"
        );
        assert_eq!(
            decoded.canonical_json_bytes().unwrap(),
            canonical,
            "canonical bytes must remain identical after decoding coordinate {coordinate}"
        );
    }
}

#[test]
fn canonical_json_sorts_nested_extension_objects() {
    let mut document = sample_document();
    let mut nested = Map::new();
    nested.insert("z-last".to_string(), json!(1));
    nested.insert("a-first".to_string(), json!(2));
    document
        .extensions
        .insert("x-nested-order".to_string(), Value::Object(nested));

    let bytes = document
        .canonical_json_bytes()
        .expect("nested extension document serializes");
    let json = String::from_utf8(bytes).expect("canonical JSON is UTF-8");
    let first = json
        .find("\"a-first\"")
        .expect("canonical JSON retains the first nested key");
    let last = json
        .find("\"z-last\"")
        .expect("canonical JSON retains the last nested key");
    assert!(
        first < last,
        "nested extension keys must be emitted in canonical order: {json}"
    );
}

#[test]
fn path_style_requires_explicit_nullable_fill_and_stroke() {
    let document = serde_json::to_value(sample_document()).unwrap();
    for field in ["fill", "stroke"] {
        let mut missing = document.clone();
        missing["commands"][3]["style"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            DrawingListDocument::from_json_bytes(&serde_json::to_vec(&missing).unwrap()).is_err(),
            "wire decoder must require path style {field}"
        );
        assert!(serde_json::from_value::<DrawingListDocument>(missing).is_err());
    }

    let mut unpainted = document;
    unpainted["commands"][3]["style"]["fill"] = Value::Null;
    unpainted["commands"][3]["style"]["stroke"] = Value::Null;
    DrawingListDocument::from_json_bytes(&serde_json::to_vec(&unpainted).unwrap())
        .expect("explicit null disables paint without omitting required fields");
}

#[test]
fn close_segment_rejects_unknown_fields() {
    let mut value = serde_json::to_value(sample_document()).unwrap();
    value["resources"][0]["segments"][4]["future_geometry"] = json!(true);
    assert!(DrawingListDocument::from_json_bytes(&serde_json::to_vec(&value).unwrap()).is_err());
    assert!(serde_json::from_value::<DrawingListDocument>(value).is_err());
}

#[test]
fn glyph_geometry_provenance_rejects_unknown_fields() {
    let provenance = MeasurementProvenance::GlyphGeometry;
    let bytes = serde_json::to_vec(&provenance).unwrap();
    assert_eq!(
        serde_json::from_slice::<MeasurementProvenance>(&bytes).unwrap(),
        provenance
    );
    let mut value = serde_json::to_value(sample_document()).unwrap();
    value["commands"][5]["run"]["obligation"]["measurement"] =
        json!({"kind": "glyph_geometry", "future_measurement": true});
    assert!(DrawingListDocument::from_json_bytes(&serde_json::to_vec(&value).unwrap()).is_err());
    assert!(serde_json::from_value::<DrawingListDocument>(value).is_err());
}

#[test]
fn extension_depth_has_the_same_validation_encoding_and_decoding_boundary() {
    assert_eq!(DRAWING_LIST_MAX_EXTENSION_DEPTH, 125);
    for objects in [false, true] {
        for depth in [0, 1, 124, 125, 126, 128] {
            let mut nested = Value::Null;
            for _ in 0..depth {
                nested = if objects {
                    json!({"child": nested})
                } else {
                    Value::Array(vec![nested])
                };
            }
            let mut document = sample_document();
            document.extensions.insert("x-depth".into(), nested);
            let raw = serde_json::to_vec(&document).unwrap();
            let accepted = depth <= 125;
            assert_eq!(
                document.validate().is_ok(),
                accepted,
                "validation at extension depth {depth}, objects={objects}"
            );
            let canonical = document.canonical_json_bytes();
            assert_eq!(canonical.is_ok(), accepted, "encoding at depth {depth}");
            let decoded = DrawingListDocument::from_json_bytes(&raw);
            assert_eq!(decoded.is_ok(), accepted, "decoding at depth {depth}");
            let decoded = serde_json::from_value::<DrawingListDocument>(
                serde_json::to_value(&document).unwrap(),
            );
            assert_eq!(decoded.is_ok(), accepted, "value decoding at depth {depth}");
            if let Ok(canonical) = canonical {
                let decoded = DrawingListDocument::from_json_bytes(&canonical)
                    .expect("every validated canonical document decodes");
                assert_eq!(decoded.extensions, document.extensions);
                assert_eq!(decoded.canonical_json_bytes().unwrap(), canonical);
            }
        }
    }
}

#[test]
fn canonical_encoding_never_accepts_an_undecodable_128_layer_extension() {
    let mut nested = Value::Null;
    for _ in 0..128 {
        nested = Value::Array(vec![nested]);
    }
    let mut document = sample_document();
    document.extensions.insert("x-depth".into(), nested);
    if let Ok(bytes) = document.canonical_json_bytes() {
        DrawingListDocument::from_json_bytes(&bytes)
            .expect("successful canonical encoding must produce a decodable document");
    }
}

#[test]
fn published_draft_2020_12_schema_accepts_the_runtime_document() {
    let schema: Value = serde_json::from_str(include_str!("../schema/drawing-list-v1.json"))
        .expect("DrawingList schema is valid JSON");
    assert_eq!(
        schema["$defs"]["stroke_style"]["properties"]["dash_array"]["maxItems"],
        json!(DrawingListLimits::default().max_stroke_dash_entries),
        "the schema must publish the protocol hard maximum for one dash array"
    );
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&schema)
        .expect("DrawingList schema is valid Draft 2020-12");
    let value = serde_json::to_value(sample_document()).expect("fixture serializes");

    assert!(
        validator.is_valid(&value),
        "published schema rejected runtime document: {value}"
    );

    let mut unpainted = sample_document();
    for command in &mut unpainted.commands {
        if let DrawingCommand::DrawPath { style, .. } = command {
            style.fill = None;
            style.stroke = None;
        }
    }
    unpainted
        .validate()
        .expect("retained unpainted paths are valid");
    assert!(validator.is_valid(&serde_json::to_value(unpainted).unwrap()));

    let mut stroked_text = sample_document();
    let DrawingCommand::DrawText { run } = &mut stroked_text.commands[5] else {
        panic!("sample document command 5 is text");
    };
    run.style.stroke = Some(StrokeStyle {
        paint: Paint::resource(ResourceId::new("paint.node-fill")),
        width: 2.0,
        dash_array: vec![3.0, 1.0],
        dash_offset: 0.5,
        line_cap: merman_display_list::LineCap::Round,
        line_join: merman_display_list::LineJoin::Bevel,
        miter_limit: 4.0,
    });
    run.style.paint_order = TextPaintOrder::StrokeThenFill;
    stroked_text
        .validate()
        .expect("text stroke may reference a paint resource");
    let stroked_value = serde_json::to_value(&stroked_text).expect("stroked text serializes");
    assert!(
        validator.is_valid(&stroked_value),
        "published schema rejected a valid text stroke: {stroked_value}"
    );
    let decoded: DrawingListDocument =
        serde_json::from_value(stroked_value.clone()).expect("stroked text decodes");
    assert!(matches!(
        &decoded.commands[5],
        DrawingCommand::DrawText { run }
            if run.style.paint_order == TextPaintOrder::StrokeThenFill
                && run.style.stroke.is_some()
    ));

    for required_field in ["stroke", "paint_order"] {
        let mut missing_field = stroked_value.clone();
        missing_field["commands"][5]["run"]["style"]
            .as_object_mut()
            .expect("text style is an object")
            .remove(required_field);
        assert!(
            !validator.is_valid(&missing_field),
            "schema must require text style field {required_field}"
        );
        let decoded = serde_json::from_value::<DrawingListDocument>(missing_field);
        assert!(
            decoded.is_err(),
            "decoder must require text style field {required_field}"
        );
    }

    let mut central_baseline = value.clone();
    central_baseline["commands"][5]["run"]["baseline"] = json!("central");
    assert!(validator.is_valid(&central_baseline));
    let decoded: DrawingListDocument =
        serde_json::from_value(central_baseline).expect("central baseline decodes");
    assert!(matches!(
        &decoded.commands[5],
        DrawingCommand::DrawText { run } if run.baseline == TextBaseline::Central
    ));

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

    let mut invalid_image = serde_json::to_value(extended_document()).expect("fixture serializes");
    let image = invalid_image["resources"]
        .as_array_mut()
        .expect("resources are an array")
        .iter_mut()
        .find(|resource| resource["kind"] == "image")
        .expect("extended fixture has an image");
    image["image"]["media_type"] = json!("image/svg+xml");
    assert!(!validator.is_valid(&invalid_image));

    for media_type in [
        "image /png",
        "image/ png",
        "\u{a0}image/png",
        "image/png\u{a0}",
        "image/png\u{a0}; charset=binary",
    ] {
        let mut invalid_image = extended.clone();
        let image = invalid_image["resources"]
            .as_array_mut()
            .expect("resources are an array")
            .iter_mut()
            .find(|resource| resource["kind"] == "image")
            .expect("extended fixture has an image");
        image["image"]["media_type"] = json!(media_type);
        assert!(
            !validator.is_valid(&invalid_image),
            "schema must reject media type whitespace adjacent to '/': {media_type}"
        );
    }

    let mut invalid_u32 = serde_json::to_value(extended_document()).expect("fixture serializes");
    let image = invalid_u32["resources"]
        .as_array_mut()
        .expect("resources are an array")
        .iter_mut()
        .find(|resource| resource["kind"] == "image")
        .expect("extended fixture has an image");
    image["pixel_width"] = json!(4_294_967_296u64);
    assert!(!validator.is_valid(&invalid_u32));

    let mut invalid_glyph = serde_json::to_value(extended_document()).expect("fixture serializes");
    invalid_glyph["commands"]
        .as_array_mut()
        .expect("commands are an array")
        .iter_mut()
        .find_map(|command| command.get_mut("run"))
        .expect("extended fixture has text")["obligation"]["glyphs"][0]["glyph_id"] =
        json!(4_294_967_296u64);
    assert!(!validator.is_valid(&invalid_glyph));

    let mut invalid_base64 = serde_json::to_value(extended_document()).expect("fixture serializes");
    let image = invalid_base64["resources"]
        .as_array_mut()
        .expect("resources are an array")
        .iter_mut()
        .find(|resource| resource["kind"] == "image")
        .expect("extended fixture has an image");
    image["image"]["data"] = json!("not-base64!");
    assert!(!validator.is_valid(&invalid_base64));

    for non_canonical in ["Zh==", "Zm9="] {
        let mut invalid_pad_bits = extended.clone();
        let font = invalid_pad_bits["resources"]
            .as_array_mut()
            .expect("resources are an array")
            .iter_mut()
            .find(|resource| resource["kind"] == "font")
            .expect("extended fixture has a font");
        font["font"]["data"] = json!(non_canonical);
        assert!(
            !validator.is_valid(&invalid_pad_bits),
            "schema must reject non-zero Base64 pad bits: {non_canonical}"
        );
    }
}
