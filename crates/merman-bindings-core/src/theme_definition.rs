use merman::diagram_theme::{
    ThemeDefinitionCompileError, ThemeMaterializationErrorV1, ThemeSupportQueryV1,
    ThemeSupportQueryV2,
    compile_theme_definition_json as compile_theme_definition_json_with_renderer,
    describe_theme_support as describe_theme_support_with_renderer,
    describe_theme_support_v2 as describe_theme_support_v2_with_renderer,
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
    if version.schema_version == merman::diagram_theme::THEME_SUPPORT_SCHEMA_VERSION_V2 {
        let query = serde_json::from_slice::<ThemeSupportQueryV2>(bytes).map_err(|error| {
            BindingError::invalid_options_json(format!("invalid theme support query JSON: {error}"))
        })?;
        let descriptor = describe_theme_support_v2_with_renderer(&query);
        serde_json::to_vec(&descriptor).map_err(crate::common::internal_json_error)
    } else if version.schema_version == merman::diagram_theme::THEME_SUPPORT_SCHEMA_VERSION_V1 {
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
    let compiler =
        DiagramThemeCompiler::new().with_resource_policy(general_binding_theme_resource_policy());
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
    compile_theme_definition_json_with_renderer(compiler, bytes)
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

fn materialization_error(
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
        MaterializedThemeWireV1, PresetExportV1, ThemeDefinitionV1, ThemeRuleFacetV1,
        ThemeSupportBaseTypographyPropertyV2, ThemeSupportOutputV1, ThemeSupportQueryV1,
        ThemeSupportQueryV2, ThemeTokensV1, describe_theme_support, describe_theme_support_v2,
        materialize_theme_with_resource_policy,
    };
    use merman::svg::{ThemeResourceLimitId, ThemeResourcePolicy};
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
    fn preset_export_uses_the_same_policy_aware_compiler_contract() {
        let policy = ThemeResourcePolicy::for_profile(
            merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE,
        );
        let compiler = merman::svg::DiagramThemeCompiler::new().with_resource_policy(policy);
        let actual = crate::export_theme_preset_json_with(&compiler, b"editor-light")
            .expect("a built-in preset should export under the active compiler policy");
        let export: PresetExportV1 =
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
        let known = ThemeSupportQueryV1::known(
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

        let known_v2 = ThemeSupportQueryV2::base_typography(
            "railroad",
            ThemeSupportOutputV1::StandaloneSvg,
            ThemeSupportBaseTypographyPropertyV2::FontSize,
        );
        let known_v2_input =
            serde_json::to_vec(&known_v2).expect("known V2 query should serialize");
        let known_v2_output = crate::describe_theme_support_json(&known_v2_input)
            .expect("known V2 query should succeed");
        assert_eq!(
            known_v2_output,
            serde_json::to_vec(&describe_theme_support_v2(&known_v2))
                .expect("known V2 descriptor should serialize")
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
                "target": "node",
                "facet": "radius"
            });
            query[field] = Value::String(value.to_owned());
            let input = serde_json::to_vec(&query).unwrap();
            let output = crate::describe_theme_support_json(&input)
                .expect("unknown catalog identifiers remain valid discovery input");
            let descriptor: Value = serde_json::from_slice(&output).unwrap();

            assert_eq!(descriptor["query"], query);
            assert_eq!(descriptor["state"], "unverified");
            assert_eq!(descriptor["reason_ids"], json!([reason_id]));
        }

        let unknown_v2_property = json!({
            "schema_version": 2,
            "family": "railroad",
            "output": "standalone-svg",
            "subject": {
                "kind": "base-typography",
                "property": "future-property"
            }
        });
        let output =
            crate::describe_theme_support_json(&serde_json::to_vec(&unknown_v2_property).unwrap())
                .expect("unknown V2 properties remain valid discovery input");
        let descriptor: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(descriptor["query"], unknown_v2_property);
        assert_eq!(descriptor["state"], "unverified");
        assert_eq!(
            descriptor["reason_ids"],
            json!(["theme-support.unknown-base-typography-property"])
        );

        let unknown_v2_subject = json!({
            "schema_version": 2,
            "family": "future-family",
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
            crate::describe_theme_support_json(&serde_json::to_vec(&unknown_v2_subject).unwrap())
                .expect("bounded unknown V2 subjects remain valid discovery input");
        let descriptor: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(descriptor["query"], unknown_v2_subject);
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
    fn support_query_rejects_unknown_schema_versions_instead_of_decoding_as_v1() {
        let error = crate::describe_theme_support_json(
            br#"{"schema_version":99,"family":"flowchart","output":"standalone-svg","target":"node","facet":"fill"}"#,
        )
        .expect_err("unknown support schema must fail closed");

        assert_eq!(error.status(), crate::BindingStatus::OptionsJsonError);
        assert!(error.message().contains("schema version 99"));
    }
}
