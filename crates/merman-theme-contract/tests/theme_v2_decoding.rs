use merman_theme_contract::v2::{ThemeCompiler, ThemeDocumentV2};
use serde_json::json;

#[test]
fn reordered_fields_keep_the_same_theme_and_wire_shape() {
    let ordered = br##"{
        "schema_version": 2,
        "roles": {
            "canvas": {"kind": "linear_gradient", "stops": [
                {"offset": 0, "color": "#FFFFFF"}, {"offset": 1, "color": "#EEEEEE"}
            ]},
            "surface.container": "transparent"
        },
        "form": {"elevation": "flat"},
        "appearances": {"light": {
            "seed": {"canvas": "#FFFFFF", "ink": "#222222", "accent": "#334488"},
            "data": {"fill_ink": "auto"},
            "canvas": {"textures": [{"kind": "grid", "step_px": 16}]},
            "elevation": {"kind": "glow", "color": "#123456", "blur": 2}
        }},
        "overrides": [{"families": ["flowchart"], "elevation": {
            "kind": "layered_stack", "layers": [{"dx": 1, "dy": 2, "color": "#000000"}]
        }}]
    }"##;
    let reordered = br##"{
        "overrides": [{"elevation": {
            "layers": [{"color": "#000000", "dx": 1, "dy": 2}], "kind": "layered_stack"
        }, "families": ["flowchart"]}],
        "appearances": {"light": {
            "elevation": {"blur": 2, "color": "#123456", "kind": "glow"},
            "canvas": {"textures": [{"step_px": 16, "kind": "grid"}]},
            "data": {"fill_ink": "auto"},
            "seed": {"accent": "#334488", "ink": "#222222", "canvas": "#FFFFFF"}
        }},
        "form": {"elevation": "flat"},
        "roles": {
            "surface.container": "transparent",
            "canvas": {"stops": [
                {"color": "#FFFFFF", "offset": 0}, {"color": "#EEEEEE", "offset": 1}
            ], "kind": "linear_gradient"}
        },
        "schema_version": 2
    }"##;
    let compiler = ThemeCompiler::new();
    let document = compiler.parse_json(ordered).unwrap();
    assert_eq!(document, compiler.parse_json(reordered).unwrap());
    assert_eq!(
        document,
        serde_json::from_slice::<ThemeDocumentV2>(ordered).unwrap()
    );

    let wire = serde_json::to_value(compiler.compile(&document, None).unwrap()).unwrap();
    assert_eq!(wire["roles"]["canvas"]["kind"], "linear_gradient");
    assert!(wire.get("base").is_none());
    assert_eq!(
        wire["canvas"]["textures"],
        json!([{"kind": "grid", "step_px": 16.0, "major_every": 5,
            "opacity": 0.1, "major_opacity": 0.2}]),
    );
    assert_eq!(
        wire["elevation"],
        json!({"kind": "glow", "color": "#123456", "blur": 2.0, "opacity": 0.35}),
    );
    assert_eq!(
        wire["family_overrides"]["flowchart"]["elevation"],
        json!({"kind": "shadows", "layers": [
            {"dx": 1.0, "dy": 2.0, "blur": 0.0, "color": "#000000", "opacity": 1.0}
        ]}),
    );
}

#[test]
fn duplicate_keys_keep_the_sdk_diagnostic_and_track_nested_objects() {
    let compiler = ThemeCompiler::new();
    let error = compiler
        .parse_json(b"{\n  \"schema_version\": 2,\n  \"schema_version\": 2\n}")
        .unwrap_err();
    assert_eq!(error.path(), Some("."));
    assert_eq!((error.line(), error.column()), (Some(3), Some(18)));
    assert_eq!(
        error.to_string(),
        "duplicate key `schema_version` at line 3 column 18"
    );

    for (input, path, key) in [
        (
            r#""roles":{"canvas":{"stops":[{"offset":0,"\u006fffset":1}],"kind":"linear_gradient"}}"#,
            "roles.canvas.stops[0]",
            "offset",
        ),
        (
            r#""appearances":{"light":{"canvas":{"textures":[{"step_px":16,"kind":"grid","kind":"dots"}]}}}"#,
            "appearances.light.canvas.textures[0]",
            "kind",
        ),
        (
            r#""form":{"elevation":{"layers":[{"dx":1,"dx":2}],"kind":"layered_stack"}}"#,
            "form.elevation.layers[0]",
            "dx",
        ),
    ] {
        let input = format!(r#"{{"schema_version":2,{input}}}"#);
        let error = compiler.parse_json(input.as_bytes()).unwrap_err();
        assert_eq!(error.path(), Some(path), "{error}");
        assert!(
            error
                .to_string()
                .starts_with(&format!("duplicate key `{key}`")),
            "{error}"
        );
    }
}

#[test]
fn malformed_nested_values_keep_original_paths_and_coordinates() {
    let compiler = ThemeCompiler::new();
    for (input, path, column) in [
        (r#""extends": ]"#, "extends", 14),
        (
            r#""roles":{"canvas":{"stops":[{"offset": ?}],"kind":"linear_gradient"}}"#,
            "roles.canvas.stops[0].offset",
            42,
        ),
        (
            r#""form":{"elevation":{"layers":[{"dx":1,"dy": !}],"kind":"layered_stack"}}"#,
            "form.elevation.layers[0].dy",
            48,
        ),
    ] {
        let input = format!("{{\n  \"schema_version\": 2,\n  {input}\n}}");
        let error = compiler.parse_json(input.as_bytes()).unwrap_err();
        assert_eq!(error.path(), Some(path), "{error}");
        assert_eq!((error.line(), error.column()), (Some(3), Some(column)));
        assert!(error.to_string().starts_with("expected value"), "{error}");
    }
}

#[test]
fn tagged_objects_reject_extras_nulls_and_missing_required_fields() {
    let compiler = ThemeCompiler::new();
    for (texture, message) in [
        (
            r#"{"kind":"grid","step_px":16,"unexpected":true}"#,
            "unknown field `unexpected`",
        ),
        (
            r#"{"kind":"grid","step_px":16,"radius_px":1}"#,
            "unknown field `radius_px`",
        ),
        (
            r#"{"kind":"none","opacity":0.2}"#,
            "unknown field `opacity`",
        ),
        (
            r#"{"color":null,"step_px":16,"kind":"grid"}"#,
            "invalid type: null",
        ),
        (r#"{"kind":"radial_wash"}"#, "missing field `color`"),
    ] {
        let input = format!(
            r#"{{"schema_version":2,"appearances":{{"light":{{"canvas":{{"textures":[{texture}]}}}}}}}}"#
        );
        let error = compiler.parse_json(input.as_bytes()).unwrap_err();
        assert!(error.to_string().starts_with(message), "{input}: {error}");
    }
    for (effect, message) in [
        (
            r#"{"layers":[],"opacity":0.2,"kind":"layered_stack"}"#,
            "unknown field `opacity`",
        ),
        (
            r#"{"layers":null,"kind":"layered_stack"}"#,
            "invalid type: null",
        ),
        (
            r##"{"kind":"glow","color":"#123456"}"##,
            "missing field `blur`",
        ),
    ] {
        let input = format!(r#"{{"schema_version":2,"form":{{"elevation":{effect}}}}}"#);
        let error = compiler.parse_json(input.as_bytes()).unwrap_err();
        assert!(error.to_string().starts_with(message), "{input}: {error}");
    }
}

#[test]
fn document_fields_require_json_objects_at_the_schema_paths() {
    let compiler = ThemeCompiler::new();
    for (input, path) in [
        (json!(["", 2, "x", "X", "", "custom", "studio"]), "."),
        (
            json!({"schema_version": 2, "extends": "studio", "form": []}),
            "form",
        ),
        (
            json!({"schema_version": 2, "form": {"chart": [true]}}),
            "form.chart",
        ),
        (
            json!({"schema_version": 2, "appearances": {"light": {"seed": []}}}),
            "appearances.light.seed",
        ),
        (
            json!({"schema_version": 2, "overrides": [[["pie"]]]}),
            "overrides[0]",
        ),
        (
            json!({"schema_version": 2, "roles": {"canvas": {
                "kind": "linear_gradient", "stops": [[0, "#FFFFFF"], [1, "#000000"]]
            }}}),
            "roles.canvas.stops[0]",
        ),
        (
            json!({"schema_version": 2, "form": {"elevation": {
                "kind": "layered_stack", "layers": [[1, 2, 0, "#000000"]]
            }}}),
            "form.elevation.layers[0]",
        ),
    ] {
        let encoded = input.to_string();
        assert!(
            serde_json::from_str::<ThemeDocumentV2>(&encoded).is_err(),
            "{input}"
        );
        let error = compiler.parse_json(encoded.as_bytes()).unwrap_err();
        assert_eq!(error.path(), Some(path), "{input}: {error}");
        assert!(
            error.to_string().contains("expected a JSON object"),
            "{error}"
        );
        assert_eq!(error.line(), Some(1));
    }
}

#[test]
fn document_enums_require_strings_at_the_schema_paths() {
    let compiler = ThemeCompiler::new();
    for (field, spelling, path) in [
        (
            r#""default_appearance": VALUE"#,
            "dark",
            "default_appearance",
        ),
        (r#""category": VALUE"#, "reading", "category"),
        (
            r#""typography": {"body_family": VALUE}"#,
            "sans",
            "typography.body_family",
        ),
        (
            r#""typography": {"title_family": VALUE}"#,
            "serif",
            "typography.title_family",
        ),
        (
            r#""typography": {"code_family": VALUE}"#,
            "mono",
            "typography.code_family",
        ),
        (r#""form": {"density": VALUE}"#, "compact", "form.density"),
        (
            r#""form": {"edge_style": VALUE}"#,
            "orthogonal",
            "form.edge_style",
        ),
        (
            r#""form": {"marker_style": VALUE}"#,
            "open",
            "form.marker_style",
        ),
        (
            r#""form": {"edge_label_bg": VALUE}"#,
            "transparent",
            "form.edge_label_bg",
        ),
        (
            r#""overrides": [{"families": ["flowchart"], "appearance": VALUE}]"#,
            "light",
            "overrides[0].appearance",
        ),
        (
            r##""roles": {"canvas": {"kind": VALUE, "stops": [
                {"offset": 0, "color": "#FFFFFF"}, {"offset": 1, "color": "#EEEEEE"}
            ]}}"##,
            "linear_gradient",
            "roles.canvas.kind",
        ),
    ] {
        let document = |value: &str| {
            let field = field.replace("VALUE", value);
            format!("{{\n  \"schema_version\": 2,\n  {field}\n}}")
        };
        let valid = document(&json!(spelling).to_string());
        assert_eq!(
            compiler.parse_json(valid.as_bytes()).unwrap(),
            serde_json::from_str::<ThemeDocumentV2>(&valid).unwrap()
        );

        let invalid = document(&json!({(spelling): null}).to_string());
        assert!(serde_json::from_str::<ThemeDocumentV2>(&invalid).is_err());
        assert!(
            serde_json::from_value::<ThemeDocumentV2>(serde_json::from_str(&invalid).unwrap())
                .is_err()
        );
        let error = compiler.parse_json(invalid.as_bytes()).unwrap_err();
        assert_eq!(error.path(), Some(path), "{error}");
        assert_eq!(error.line(), Some(3));
        assert!(error.to_string().contains("expected a string"), "{error}");
    }
}

#[test]
fn unknown_enum_spelling_keeps_the_source_diagnostic() {
    let error = ThemeCompiler::new()
        .parse_json(
            b"{\n  \"schema_version\": 2,\n  \"typography\": {\"title_family\": \"SERIF\"}\n}",
        )
        .unwrap_err();
    assert_eq!(error.path(), Some("typography.title_family"));
    assert_eq!((error.line(), error.column()), (Some(3), Some(40)));
    assert_eq!(
        error.to_string(),
        "unknown variant `SERIF`, expected one of `sans`, `serif`, `mono` at line 3 column 40"
    );
}

#[test]
fn trailing_input_is_rejected_after_a_valid_document() {
    let error = ThemeCompiler::new()
        .parse_json(b"{\"schema_version\": 2}\n[]")
        .unwrap_err();
    assert_eq!(error.path(), Some("."));
    assert_eq!((error.line(), error.column()), (Some(2), Some(1)));
    assert!(error.to_string().starts_with("trailing characters"));
}
