use merman_theme_contract::{ThemeAppearance, ThemeRecipeV1};
use serde_json::json;

#[test]
fn one_recipe_contract_round_trips_concrete_and_paired_designs() {
    let inputs = [
        json!({"schema_version":1,"kind":"design","design":{}}),
        json!({"schema_version":1,"kind":"complete_spec","complete_spec":{}}),
        json!({
            "schema_version":1,"kind":"design","design":{
                "appearances":{"light":{},"dark":{"tokens":{
                    "surface":{"type":"paint","value":"#20242c"}
                }}},
                "default_appearance":"light"
            }
        }),
    ];
    for input in inputs {
        let recipe: ThemeRecipeV1 = serde_json::from_value(input.clone()).unwrap();
        assert_eq!(serde_json::to_value(&recipe).unwrap(), input);
        assert_eq!(recipe.schema_version(), input["schema_version"]);
    }
}

#[test]
fn appearance_layers_are_closed_objects_without_resource_or_policy_overrides() {
    for local in [
        json!([]),
        json!(null),
        json!({"assets":{}}),
        json!({"requirements":{}}),
        json!({"canvas":null}),
        json!({"typography":null}),
        json!({"mermaid":null}),
        json!({"future_field":true}),
    ] {
        let input = json!({
            "schema_version":1,"kind":"design","design":{
                "appearances":{"light":local},"default_appearance":"light"
            }
        });
        assert!(serde_json::from_value::<ThemeRecipeV1>(input).is_err());
    }
}

#[test]
fn appearance_declarations_belong_to_the_design_and_reject_null_or_unknown_values() {
    for fragment in [
        json!({"appearances":null}),
        json!({"default_appearance":null}),
        json!({"appearances":{"auto":{}},"default_appearance":"auto"}),
        json!({"appearances":{"light":{}},"default_appearance":"system"}),
        json!({"appearance":"dark"}),
        json!({"complete_spec":{}}),
    ] {
        let mut input = json!({"schema_version":1,"kind":"design","design":{}});
        input["design"]
            .as_object_mut()
            .unwrap()
            .extend(fragment.as_object().unwrap().clone());
        assert!(serde_json::from_value::<ThemeRecipeV1>(input).is_err());
    }
    let input = json!({
        "schema_version":1,"kind":"design","design":{},
        "appearances":{"light":{}},"default_appearance":"light"
    });
    assert!(serde_json::from_value::<ThemeRecipeV1>(input).is_err());
}

#[test]
fn recipe_uses_an_integer_version_and_rejects_duplicate_sections() {
    for version in [json!(0), json!(2), json!(1.0), json!("1"), json!(null)] {
        let input = json!({"schema_version":version,"kind":"design","design":{}});
        assert!(serde_json::from_value::<ThemeRecipeV1>(input).is_err());
    }
    for input in [
        r#"{"schema_version":1,"schema_version":1,"kind":"design","design":{}}"#,
        r#"{"schema_version":1,"kind":"design","design":{},"design":{}}"#,
        r#"{"schema_version":1,"kind":"design","design":{"appearances":{"light":{}},"appearances":{"dark":{}},"default_appearance":"dark"}}"#,
    ] {
        assert!(serde_json::from_str::<ThemeRecipeV1>(input).is_err());
    }
    assert_eq!(
        "light".parse::<ThemeAppearance>().unwrap(),
        ThemeAppearance::Light
    );
    assert!("auto".parse::<ThemeAppearance>().is_err());
}

#[test]
fn appearance_values_accept_only_the_two_string_identifiers() {
    for appearance in [json!("light"), json!("dark")] {
        let decoded: ThemeAppearance = serde_json::from_value(appearance.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), appearance);
    }
    for invalid in [
        json!({"light":null}),
        json!({"dark":null}),
        json!(["light"]),
        json!(true),
        json!(1),
        json!(null),
        json!("auto"),
    ] {
        assert!(serde_json::from_value::<ThemeAppearance>(invalid.clone()).is_err());
        let input = json!({
            "schema_version":1,"kind":"design","design":{
                "appearances":{"light":{},"dark":{}},
                "default_appearance":invalid
            }
        });
        assert!(serde_json::from_value::<ThemeRecipeV1>(input).is_err());
    }
}
