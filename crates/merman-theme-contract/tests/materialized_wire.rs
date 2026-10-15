use merman_theme_contract::{
    CanonicalJsonErrorKind, DiagramThemeSpecWireV1, MaterializedThemeWireV1, SpecifiedWireV1,
    ThemeRuleSetWireV1, ThemeStylePatchWireV1,
};

fn valid_materialized_value() -> serde_json::Value {
    serde_json::to_value(
        MaterializedThemeWireV1::try_new(DiagramThemeSpecWireV1::default())
            .expect("a valid spec should create a materialized success envelope"),
    )
    .expect("a valid materialized success envelope should serialize")
}

#[test]
fn materialized_success_wire_round_trips_and_exposes_the_current_versions() {
    let spec = DiagramThemeSpecWireV1 {
        styles: Some(Vec::new()),
        ..DiagramThemeSpecWireV1::default()
    };
    let materialized = MaterializedThemeWireV1::try_new(spec.clone())
        .expect("a valid spec should create a materialized success envelope");

    assert_eq!(materialized.schema_version(), 1);
    assert_eq!(materialized.authoring_schema_version(), 1);
    assert_eq!(materialized.expansion_version(), 1);
    assert_eq!(materialized.spec_schema_version(), 1);
    assert_eq!(materialized.spec(), &spec);

    let encoded = serde_json::to_value(&materialized)
        .expect("a valid materialized success envelope should serialize");
    assert_eq!(
        encoded,
        serde_json::json!({
            "schema_version": 1,
            "authoring_schema_version": 1,
            "expansion_version": 1,
            "spec_schema_version": 1,
            "spec": {"styles": []}
        })
    );

    let decoded: MaterializedThemeWireV1 = serde_json::from_value(encoded)
        .expect("the serialized materialized success envelope should decode");
    assert_eq!(decoded, materialized);
    assert_eq!(decoded.into_spec(), spec);
}

#[test]
fn materialized_success_wire_rejects_unknown_fields() {
    let mut value = valid_materialized_value();
    value
        .as_object_mut()
        .expect("the envelope should be an object")
        .insert("unexpected".to_owned(), serde_json::Value::Bool(true));

    assert!(serde_json::from_value::<MaterializedThemeWireV1>(value).is_err());
}

#[test]
fn materialized_success_wire_requires_every_field() {
    for field in [
        "schema_version",
        "authoring_schema_version",
        "expansion_version",
        "spec_schema_version",
        "spec",
    ] {
        let mut value = valid_materialized_value();
        value
            .as_object_mut()
            .expect("the envelope should be an object")
            .remove(field);

        assert!(
            serde_json::from_value::<MaterializedThemeWireV1>(value).is_err(),
            "missing {field} must be rejected"
        );
    }
}

#[test]
fn materialized_success_wire_rejects_null_fields() {
    for field in [
        "schema_version",
        "authoring_schema_version",
        "expansion_version",
        "spec_schema_version",
        "spec",
    ] {
        let mut value = valid_materialized_value();
        value
            .as_object_mut()
            .expect("the envelope should be an object")
            .insert(field.to_owned(), serde_json::Value::Null);

        assert!(
            serde_json::from_value::<MaterializedThemeWireV1>(value).is_err(),
            "null {field} must be rejected"
        );
    }
}

#[test]
fn materialized_success_wire_rejects_unknown_versions() {
    for field in [
        "schema_version",
        "authoring_schema_version",
        "expansion_version",
        "spec_schema_version",
    ] {
        let mut value = valid_materialized_value();
        value
            .as_object_mut()
            .expect("the envelope should be an object")
            .insert(field.to_owned(), serde_json::Value::from(2));

        assert!(
            serde_json::from_value::<MaterializedThemeWireV1>(value).is_err(),
            "unknown {field} must be rejected"
        );
    }
}

#[test]
fn materialized_success_wire_rejects_non_finite_specs_before_construction() {
    let spec = DiagramThemeSpecWireV1 {
        styles: Some(vec![ThemeRuleSetWireV1::Rule {
            target: "node".to_owned(),
            family: None,
            variant: None,
            ordinal: None,
            style: ThemeStylePatchWireV1 {
                opacity: SpecifiedWireV1::Value(f32::INFINITY),
                ..ThemeStylePatchWireV1::default()
            },
        }]),
        ..DiagramThemeSpecWireV1::default()
    };

    let error = MaterializedThemeWireV1::try_new(spec)
        .expect_err("a success wire must be serializable when it is created");
    assert_eq!(error.kind(), CanonicalJsonErrorKind::InvalidInput);
}
