use merman::diagram_theme::{
    ThemeDefinitionCompileError, ThemeMaterializationErrorV1, ThemeSupportQueryV1,
    compile_theme_definition_json as compile_theme_definition_json_with_renderer,
    describe_theme_support as describe_theme_support_with_renderer,
    materialize_theme_json_with_resource_policy as materialize_theme_json_with_renderer_policy,
};
use merman::svg::{DiagramTheme, DiagramThemeCompiler, ThemeResourceLimitId, ThemeResourcePolicy};

use crate::common::BindingError;

fn general_binding_theme_resource_policy() -> ThemeResourcePolicy {
    ThemeResourcePolicy::for_profile(merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE)
}

/// Materializes one versioned authoring definition as the contract-owned JSON wire.
pub fn materialize_theme_definition_json(bytes: &[u8]) -> Result<Vec<u8>, BindingError> {
    let policy = general_binding_theme_resource_policy();
    materialize_theme_definition_json_with_resource_policy(bytes, &policy)
}

/// Materializes one versioned authoring definition under a caller-owned resource policy.
pub fn materialize_theme_definition_json_with_resource_policy(
    bytes: &[u8],
    policy: &ThemeResourcePolicy,
) -> Result<Vec<u8>, BindingError> {
    let materialized = materialize_theme_json_with_renderer_policy(bytes, policy)
        .map_err(|error| materialization_error(policy, error))?;
    serde_json::to_vec(&materialized).map_err(crate::common::internal_json_error)
}

/// Describes one coarse theme-support query using the general binding resource profile.
pub fn describe_theme_support_json(bytes: &[u8]) -> Result<Vec<u8>, BindingError> {
    let policy = general_binding_theme_resource_policy();
    describe_theme_support_json_with_resource_policy(bytes, &policy)
}

/// Describes one coarse theme-support query under a caller-owned encoded-byte ceiling.
pub fn describe_theme_support_json_with_resource_policy(
    bytes: &[u8],
    policy: &ThemeResourcePolicy,
) -> Result<Vec<u8>, BindingError> {
    policy
        .check_theme_encoded_bytes(bytes.len())
        .map_err(crate::theme::theme_resource_error)?;

    #[derive(serde::Deserialize)]
    struct SupportQueryVersion {
        schema_version: u32,
    }

    let version = serde_json::from_slice::<SupportQueryVersion>(bytes).map_err(|error| {
        BindingError::invalid_options_json(format!("invalid theme support query JSON: {error}"))
    })?;
    if version.schema_version == merman::diagram_theme::THEME_SUPPORT_SCHEMA_VERSION_V1 {
        let query = serde_json::from_slice::<ThemeSupportQueryV1>(bytes).map_err(|error| {
            BindingError::invalid_options_json(format!("invalid theme support query JSON: {error}"))
        })?;
        let descriptor = describe_theme_support_with_renderer(&query);
        serde_json::to_vec(&descriptor).map_err(crate::common::internal_json_error)
    } else {
        Err(BindingError::invalid_options_json(format!(
            "unsupported theme support query schema version {}",
            version.schema_version
        )))
    }
}

/// Exports one built-in preset as a closed, self-contained version 1 recipe envelope.
pub fn export_theme_preset_json(bytes: &[u8]) -> Result<Vec<u8>, BindingError> {
    export_theme_preset_json_with_resource_policy(bytes, &general_binding_theme_resource_policy())
}

/// Exports one built-in preset under a caller-owned theme resource policy.
pub fn export_theme_preset_json_with_resource_policy(
    bytes: &[u8],
    policy: &ThemeResourcePolicy,
) -> Result<Vec<u8>, BindingError> {
    let compiler = DiagramThemeCompiler::new().with_resource_policy(policy.clone());
    export_theme_preset_json_with(&compiler, bytes)
}

/// Exports one built-in preset under the caller-owned compiler resource policy.
pub fn export_theme_preset_json_with(
    compiler: &DiagramThemeCompiler,
    bytes: &[u8],
) -> Result<Vec<u8>, BindingError> {
    compiler
        .resource_policy()
        .check_theme_encoded_bytes(bytes.len())
        .map_err(crate::theme::theme_resource_error)?;
    let id = std::str::from_utf8(bytes)
        .map_err(|_| BindingError::invalid_argument("theme preset id must be valid UTF-8"))?;
    if id.is_empty() {
        return Err(BindingError::invalid_argument(
            "theme preset id must not be empty",
        ));
    }
    let preset = merman::svg::ThemePreset::from_id(id)
        .map_err(|error| BindingError::invalid_argument(error.to_string()))?;
    let export = compiler
        .export_preset(preset)
        .map_err(|error| materialization_error(compiler.resource_policy(), error))?;
    serde_json::to_vec(&export).map_err(crate::common::internal_json_error)
}

/// Compiles one versioned authoring definition with the general binding resource profile.
pub fn compile_theme_definition_json(bytes: &[u8]) -> Result<DiagramTheme, BindingError> {
    let compiler =
        DiagramThemeCompiler::new().with_resource_policy(general_binding_theme_resource_policy());
    compile_theme_definition_json_with(&compiler, bytes)
}

/// Compiles one versioned authoring definition under a caller-owned compiler policy.
pub fn compile_theme_definition_json_with(
    compiler: &DiagramThemeCompiler,
    bytes: &[u8],
) -> Result<DiagramTheme, BindingError> {
    let compiler = compiler
        .clone()
        .with_embedded_fonts_allowed(cfg!(feature = "embedded-fonts"));
    compile_theme_definition_json_with_renderer(&compiler, bytes)
        .map_err(|error| definition_compile_error(compiler.resource_policy(), error))
}

pub(crate) fn definition_compile_error(
    policy: &ThemeResourcePolicy,
    error: ThemeDefinitionCompileError,
) -> BindingError {
    match error {
        ThemeDefinitionCompileError::Materialization(error) => materialization_error(policy, error),
        ThemeDefinitionCompileError::Compilation(error) => {
            crate::theme::theme_definition_compile_error(error)
        }
        error => BindingError::invalid_argument(format!("invalid theme definition: {error}")),
    }
}

pub(crate) fn materialization_error(
    policy: &ThemeResourcePolicy,
    error: ThemeMaterializationErrorV1,
) -> BindingError {
    let diagnostic = error.diagnostic();
    let projected = if diagnostic.code() == "theme-authoring.invalid-definition-json" {
        BindingError::invalid_options_json(error.to_string())
    } else if diagnostic.code() == "theme-authoring.resource-limit-exceeded"
        && let (Some(limit), Some(actual), Some(max)) = (
            diagnostic
                .limit_id()
                .and_then(ThemeResourceLimitId::from_stable_id),
            diagnostic.actual(),
            diagnostic.max(),
        )
    {
        let descriptor = limit.descriptor();
        let profile = policy
            .profile()
            .map(merman::resources::ResourceProfile::id)
            .unwrap_or("custom-theme-policy");
        BindingError::resource_limit(
            descriptor.phase.as_str(),
            descriptor.stable_id,
            actual,
            max,
            profile,
            error.to_string(),
        )
    } else {
        BindingError::invalid_argument(format!("invalid theme definition: {error}"))
    };
    projected.with_theme_authoring_details(error)
}

#[cfg(test)]
mod tests {
    use crate::BindingError;
    use merman::diagram_theme::{
        MaterializedThemeWireV1, ThemeDefinitionV1, ThemeRecipeV1, ThemeRuleFacetV1,
        ThemeSupportBaseTypographyPropertyV1, ThemeSupportOutputV1, ThemeSupportQueryV1,
        ThemeTokensV1, describe_theme_support, materialize_theme_with_resource_policy,
    };
    use merman::svg::{ThemeResourceLimitId, ThemeResourcePolicy};
    use merman_theme_authoring_fixtures::THEME_AUTHORING_GOLDEN_VECTORS_V1;
    use serde_json::{Value, json};

    fn assert_theme_authoring_error(
        input: &[u8],
        policy: &ThemeResourcePolicy,
        expected_status: crate::BindingStatus,
        expected_code: &str,
        expected_path: &str,
        expected_details: Value,
    ) -> BindingError {
        let contract_error =
            merman::diagram_theme::materialize_theme_json_with_resource_policy(input, policy)
                .expect_err("the renderer contract should reject the test input");
        let binding_error =
            crate::materialize_theme_definition_json_with_resource_policy(input, policy)
                .expect_err("the binding operation should preserve the renderer failure");

        assert_eq!(binding_error.status(), expected_status);
        assert_eq!(
            binding_error.theme_authoring_details(),
            Some(&contract_error)
        );

        let payload: Value =
            serde_json::from_slice(&crate::binding_error_payload_json_bytes(&binding_error))
                .expect("binding error payload should be valid JSON");
        let expected_envelope =
            serde_json::to_value(&contract_error).expect("contract error should serialize");
        assert_eq!(payload["details"]["theme_authoring"], expected_envelope);
        assert_eq!(
            crate::binding_error_js_details_json(&binding_error)
                .expect("details projection should serialize")
                .expect("theme failures have details")["theme_authoring"],
            expected_envelope,
        );
        assert_eq!(payload["details"]["theme_authoring"]["schema_version"], 1);
        assert_eq!(
            payload["details"]["theme_authoring"]["diagnostics"][0]["code"],
            expected_code
        );
        assert_eq!(
            payload["details"]["theme_authoring"]["diagnostics"][0]["severity"],
            "error"
        );
        assert_eq!(
            payload["details"]["theme_authoring"]["diagnostics"][0]["path"],
            expected_path
        );
        assert_eq!(
            payload["details"]["theme_authoring"]["diagnostics"][0]["details"],
            expected_details
        );
        assert_eq!(
            payload["details"]["theme_authoring"]["diagnostics"][0]["message"],
            contract_error.diagnostic().message()
        );
        binding_error
    }

    #[test]
    fn binding_materialization_matches_the_typed_contract_wire() {
        let definition = ThemeDefinitionV1::new(
            ThemeTokensV1::default().with_series(vec!["#123456".to_owned()]),
        );
        let policy = ThemeResourcePolicy::for_profile(
            merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE,
        );
        let expected = materialize_theme_with_resource_policy(&definition, &policy)
            .expect("the typed definition should materialize");
        let input = definition
            .canonical_json_bytes()
            .expect("the typed definition should have canonical JSON");

        let actual = crate::materialize_theme_definition_json_with_resource_policy(&input, &policy)
            .expect("the binding JSON operation should materialize the same definition");

        assert_eq!(
            crate::materialize_theme_definition_json(&input)
                .expect("the default binding policy should admit the same definition"),
            actual
        );
        assert_eq!(
            actual,
            serde_json::to_vec(&expected).expect("materialized contract should serialize")
        );
        assert_eq!(
            serde_json::from_slice::<MaterializedThemeWireV1>(&actual)
                .expect("binding output should retain the complete contract wire"),
            expected
        );
    }

    #[test]
    fn binding_materialization_replays_the_shared_authoring_vectors_exactly() {
        for vector in THEME_AUTHORING_GOLDEN_VECTORS_V1 {
            let materialized = crate::materialize_theme_definition_json(
                vector.readable_definition_json(),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "{} shared definition should materialize through bindings-core: {error:?}",
                    vector.name(),
                )
            });
            let materialized: MaterializedThemeWireV1 = serde_json::from_slice(&materialized)
                .unwrap_or_else(|error| {
                    panic!(
                        "{} bindings-core materialized wire should decode: {error}",
                        vector.name(),
                    )
                });

            assert_eq!(
                materialized
                    .spec()
                    .canonical_json_bytes()
                    .expect("the bindings-core materialized spec should canonicalize"),
                vector.canonical_spec_json(),
                "{} bindings-core materialization drifted from the contract vector",
                vector.name(),
            );
        }
    }

    #[test]
    fn preset_export_uses_the_same_policy_aware_compiler_contract() {
        let policy = ThemeResourcePolicy::for_profile(
            merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE,
        );
        let compiler = merman::svg::DiagramThemeCompiler::new().with_resource_policy(policy);
        let actual = crate::export_theme_preset_json_with(&compiler, b"editor-light")
            .expect("a built-in preset should export under the active compiler policy");
        let export: ThemeRecipeV1 =
            serde_json::from_slice(&actual).expect("preset export should use the contract wire");

        assert_eq!(export.kind(), "complete_spec");
        assert_eq!(
            crate::export_theme_preset_json(b"editor-light")
                .expect("the default binding policy should export the same preset"),
            actual
        );

        let unknown = crate::export_theme_preset_json_with(&compiler, b"future-preset")
            .expect_err("unknown preset ids must fail closed");
        assert_eq!(unknown.status(), crate::BindingStatus::InvalidArgument);
    }

    #[test]
    fn preset_export_checks_the_active_encoded_input_ceiling_before_utf8() {
        let policy = ThemeResourcePolicy::default()
            .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, 1)
            .expect("one byte is a valid caller-owned ceiling");
        let compiler = merman::svg::DiagramThemeCompiler::new().with_resource_policy(policy);
        let error = crate::export_theme_preset_json_with(&compiler, &[0xff, 0xff])
            .expect_err("the byte ceiling must run before UTF-8 decoding");

        assert_eq!(error.status(), crate::BindingStatus::ResourceLimitExceeded);
        let resource = error
            .resource_details()
            .expect("preset-id admission should retain resource details");
        assert_eq!(resource.limit_id, "max_theme_encoded_bytes");
        assert_eq!(resource.actual, 2);
        assert_eq!(resource.max, 1);
    }

    #[test]
    fn binding_materialization_preserves_closed_error_envelopes() {
        let policy = ThemeResourcePolicy::for_profile(
            merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE,
        );

        assert_theme_authoring_error(
            br#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{"series":[]}}"#,
            &policy,
            crate::BindingStatus::InvalidArgument,
            "theme-authoring.empty-series",
            "/tokens/series",
            json!({}),
        );

        for (entry, path, domain) in [
            (
                json!({"kind":"ordinal-palette","target":"node","colors":[]}),
                "/styles/1/colors",
                "theme-color-series",
            ),
            (
                json!({"kind":"rule","target":"node","style":{"typography":{"font_stack":[]}}}),
                "/styles/1/style/typography/font_stack",
                "font-stack",
            ),
        ] {
            let input = json!({
                "authoring_schema_version": 1,
                "expansion_version": 1,
                "tokens": {},
                "styles": [{"kind":"rule","target":"text","style":{}}, entry]
            });
            assert_eq!(input.pointer(path), Some(&json!([])));
            assert_theme_authoring_error(
                &serde_json::to_vec(&input).expect("serialize definition"),
                &policy,
                crate::BindingStatus::InvalidArgument,
                "theme-authoring.invalid-token-value",
                path,
                json!({"expected_domain_id": domain}),
            );
        }

        assert_theme_authoring_error(
            br##"{
                "authoring_schema_version": 1,
                "expansion_version": 1,
                "tokens": {},
                "styles": [
                    {"kind":"ordinal-palette","target":"node","colors":["#111111"]},
                    {"kind":"ordinal-palette","target":"node","colors":["#222222"]}
                ]
            }"##,
            &policy,
            crate::BindingStatus::InvalidArgument,
            "theme-authoring.duplicate-palette-target",
            "/styles/1/target",
            json!({
                "target_id": "node",
                "first_authored_index": 0,
                "duplicate_authored_index": 1
            }),
        );

        assert_theme_authoring_error(
            br#"{"authoring_schema_version":1,"#,
            &policy,
            crate::BindingStatus::OptionsJsonError,
            "theme-authoring.invalid-definition-json",
            "",
            json!({"reason_id": "malformed-json"}),
        );
    }

    #[test]
    fn binding_materialization_collection_error_preserves_indexed_location() {
        let input = json!({
            "authoring_schema_version": 1,
            "expansion_version": 1,
            "tokens": {},
            "styles": [
                {"kind":"rule","target":"text","style":{}},
                {"kind":"ordinal-palette","target":"node","colors":vec!["#123456";257]}
            ]
        });
        let error = assert_theme_authoring_error(
            &serde_json::to_vec(&input).unwrap(),
            &ThemeResourcePolicy::default(),
            crate::BindingStatus::InvalidArgument,
            "theme-authoring.resource-limit-exceeded",
            "/styles/1/colors",
            json!({"limit_id":"max_theme_palette_colors", "actual":257, "max":256}),
        );
        // Fixed structural ceilings are not entries in the adjustable resource catalog.
        assert!(error.resource_details().is_none());
        assert!(input.pointer("/styles/1/colors").is_some());
    }

    #[test]
    fn binding_materialization_resource_error_keeps_both_detail_envelopes() {
        let policy = ThemeResourcePolicy::default()
            .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, 1)
            .expect("one byte is a valid caller-owned ceiling");
        let error = assert_theme_authoring_error(
            br#"{}"#,
            &policy,
            crate::BindingStatus::ResourceLimitExceeded,
            "theme-authoring.resource-limit-exceeded",
            "",
            json!({
                "limit_id": "max_theme_encoded_bytes",
                "actual": 2,
                "max": 1
            }),
        );

        let resource = error
            .resource_details()
            .expect("resource failures should retain the binding resource projection");
        assert_eq!(resource.limit_id, "max_theme_encoded_bytes");
        assert_eq!(resource.phase, "theme_input");
        assert_eq!(resource.actual, 2);
        assert_eq!(resource.max, 1);
        assert_eq!(resource.profile, "interactive");
        assert_eq!(resource.cause, crate::BindingResourceLimitCause::Ceiling);
        let payload: Value =
            serde_json::from_slice(&crate::binding_error_payload_json_bytes(&error)).unwrap();
        assert_eq!(
            payload["details"]["resource"],
            json!({
                "cause": "ceiling",
                "limit_id": "max_theme_encoded_bytes",
                "phase": "theme_input",
                "actual": 2,
                "max": 1,
                "profile": "interactive"
            })
        );
        assert!(payload["details"].get("theme_authoring").is_some());
    }

    #[test]
    fn support_query_round_trips_known_and_unknown_identifiers() {
        let known = ThemeSupportQueryV1::for_target(
            "flowchart",
            ThemeSupportOutputV1::StandaloneSvg,
            "node",
            ThemeRuleFacetV1::Radius,
        );
        let known_input = serde_json::to_vec(&known).expect("known query should serialize");
        let known_output =
            crate::describe_theme_support_json(&known_input).expect("known query should succeed");
        assert_eq!(
            known_output,
            serde_json::to_vec(&describe_theme_support(&known))
                .expect("known descriptor should serialize")
        );

        let known_base = ThemeSupportQueryV1::base_typography(
            "railroad",
            ThemeSupportOutputV1::StandaloneSvg,
            ThemeSupportBaseTypographyPropertyV1::FontSize,
        );
        let known_base_input =
            serde_json::to_vec(&known_base).expect("known subject query should serialize");
        let known_base_output = crate::describe_theme_support_json(&known_base_input)
            .expect("known subject query should succeed");
        assert_eq!(
            known_base_output,
            serde_json::to_vec(&describe_theme_support(&known_base))
                .expect("known subject descriptor should serialize")
        );

        for (field, value, reason_id) in [
            ("family", "future-family", "theme-support.unknown-family"),
            ("target", "future-target", "theme-support.unknown-target"),
            ("facet", "future-facet", "theme-support.unknown-facet"),
        ] {
            let mut query = json!({
                "schema_version": 1,
                "family": "flowchart",
                "output": "standalone-svg",
                "subject": {"kind": "rule", "target": "node", "facet": "radius"}
            });
            if field == "family" {
                query[field] = Value::String(value.to_owned());
            } else {
                query["subject"][field] = Value::String(value.to_owned());
            }
            let input = serde_json::to_vec(&query).unwrap();
            let output = crate::describe_theme_support_json(&input)
                .expect("unknown catalog identifiers remain valid discovery input");
            let descriptor: Value = serde_json::from_slice(&output).unwrap();

            assert_eq!(descriptor["query"], query);
            assert_eq!(descriptor["state"], "unverified");
            assert_eq!(descriptor["reason_ids"], json!([reason_id]));
        }

        let unknown_base_property = json!({
            "schema_version": 1,
            "family": "railroad",
            "output": "standalone-svg",
            "subject": {
                "kind": "base-typography",
                "property": "future-property"
            }
        });
        let output = crate::describe_theme_support_json(
            &serde_json::to_vec(&unknown_base_property).unwrap(),
        )
        .expect("unknown properties remain valid discovery input");
        let descriptor: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(descriptor["query"], unknown_base_property);
        assert_eq!(descriptor["state"], "unverified");
        assert_eq!(
            descriptor["reason_ids"],
            json!(["theme-support.unknown-base-typography-property"])
        );

        let unknown_base_subject = json!({
            "schema_version": 1,
            "family": "flowchart",
            "output": "standalone-svg",
            "subject": {
                "kind": "future-subject",
                "target": "future-target",
                "options": {
                    "enabled": true,
                    "weights": [3, 1, 2]
                }
            }
        });
        let output =
            crate::describe_theme_support_json(&serde_json::to_vec(&unknown_base_subject).unwrap())
                .expect("bounded unknown subjects remain valid discovery input");
        let descriptor: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(descriptor["query"], unknown_base_subject);
        assert_eq!(descriptor["state"], "unverified");
        assert_eq!(
            descriptor["reason_ids"],
            json!(["theme-support.unknown-subject"])
        );
    }

    #[test]
    fn support_query_checks_encoded_bytes_before_serde() {
        let policy = ThemeResourcePolicy::default()
            .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, 1)
            .expect("one byte is a valid caller-owned ceiling");
        let error = crate::describe_theme_support_json_with_resource_policy(b"not-json", &policy)
            .expect_err("the encoded-byte ceiling should fail before JSON decoding");

        assert_eq!(error.status(), crate::BindingStatus::ResourceLimitExceeded);
        let resource = error
            .resource_details()
            .expect("predecode failure should expose structured resource details");
        assert_eq!(resource.limit_id, "max_theme_encoded_bytes");
        assert_eq!(resource.phase, "theme_input");
        assert_eq!(resource.actual, 8);
        assert_eq!(resource.max, 1);
        assert!(error.theme_authoring_details().is_none());
    }

    #[test]
    fn support_query_rejects_invalid_json_and_shape_as_options_json() {
        for input in [
            br#"{"schema_version":1,"family":"flowchart""#.as_slice(),
            br#"{"schema_version":1,"family":"flowchart","output":"standalone-svg","target":"node"}"#
                .as_slice(),
        ] {
            let error = crate::describe_theme_support_json(input)
                .expect_err("invalid support query JSON should be a transport options error");
            assert_eq!(error.status(), crate::BindingStatus::OptionsJsonError);
            assert!(error.theme_authoring_details().is_none());
        }
    }

    #[test]
    fn support_query_rejects_duplicate_subject_fields_as_options_json() {
        let error = crate::describe_theme_support_json(
            br#"{"schema_version":1,"family":"flowchart","output":"standalone-svg","subject":{"kind":"rule","target":"node","target":"edge","facet":"fill"}}"#,
        )
        .expect_err("duplicate subject fields must fail before support projection");

        assert_eq!(error.status(), crate::BindingStatus::OptionsJsonError);
        assert!(
            error
                .message()
                .contains("theme support subject contains duplicate field `target`")
        );
        assert!(error.theme_authoring_details().is_none());
    }

    #[test]
    fn support_query_rejects_unpublished_shapes_and_unknown_versions() {
        for input in [
            br#"{"schema_version":1,"family":"flowchart","output":"standalone-svg","target":"node","facet":"fill"}"#.as_slice(),
            br#"{"schema_version":2,"family":"flowchart","output":"standalone-svg","subject":{"kind":"rule","target":"node","facet":"fill"}}"#.as_slice(),
            br#"{"schema_version":99,"family":"flowchart","output":"standalone-svg","subject":{"kind":"rule","target":"node","facet":"fill"}}"#.as_slice(),
        ] {
            let error = crate::describe_theme_support_json(input)
                .expect_err("only the first-release subject-based v1 contract is accepted");
            assert_eq!(error.status(), crate::BindingStatus::OptionsJsonError);
        }
    }
}
