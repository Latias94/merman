use merman::diagram_theme::{
    ThemeDesignCompileError, ThemeMaterializationErrorV1, ThemeSupportQueryV1,
    describe_theme_support as describe_theme_support_with_renderer,
    materialize_theme_design_json_with_bindings,
};
use merman::svg::{DiagramTheme, DiagramThemeCompiler, ThemeResourceLimitId, ThemeResourcePolicy};

use crate::common::BindingError;

fn general_binding_theme_resource_policy() -> ThemeResourcePolicy {
    ThemeResourcePolicy::for_profile(merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE)
}

/// Materializes one editable design as the contract-owned JSON wire.
pub fn materialize_theme_design_json(bytes: &[u8]) -> Result<Vec<u8>, BindingError> {
    let policy = general_binding_theme_resource_policy();
    materialize_theme_design_json_with_resource_policy(bytes, &policy)
}

/// Materializes one editable design under a caller-owned resource policy.
pub fn materialize_theme_design_json_with_resource_policy(
    bytes: &[u8],
    policy: &ThemeResourcePolicy,
) -> Result<Vec<u8>, BindingError> {
    let materialized = materialize_theme_design_json_with_bindings(bytes, policy)
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

/// Compiles one editable design with the general binding resource profile.
pub fn compile_theme_design_json(bytes: &[u8]) -> Result<DiagramTheme, BindingError> {
    let compiler =
        DiagramThemeCompiler::new().with_resource_policy(general_binding_theme_resource_policy());
    compile_theme_design_json_with(&compiler, bytes)
}

/// Compiles one editable design under a caller-owned compiler policy.
pub fn compile_theme_design_json_with(
    compiler: &DiagramThemeCompiler,
    bytes: &[u8],
) -> Result<DiagramTheme, BindingError> {
    compiler
        .check_encoded_input_bytes(bytes.len())
        .map_err(crate::theme::theme_resource_error)?;
    compiler
        .check_design_json_input(bytes)
        .map_err(|error| materialization_error(compiler.resource_policy(), error))?;
    let design = serde_json::from_slice(bytes).map_err(|error| {
        BindingError::invalid_options_json(format!("invalid theme design: {error}"))
    })?;
    compiler
        .compile_recipe(merman::diagram_theme::ThemeRecipeV1::Design { design })
        .map_err(|error| design_compile_error(compiler.resource_policy(), error))
}

pub(crate) fn design_compile_error(
    policy: &ThemeResourcePolicy,
    error: ThemeDesignCompileError,
) -> BindingError {
    match error {
        ThemeDesignCompileError::Materialization(error) => materialization_error(policy, error),
        ThemeDesignCompileError::Compilation(error) => {
            crate::theme::theme_design_compile_error(error)
        }
        error => BindingError::invalid_argument(format!("invalid theme design: {error}")),
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
        BindingError::invalid_argument(format!("invalid theme design: {error}"))
    };
    projected.with_theme_authoring_details(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BindingStatus;
    use merman_theme_contract::{MaterializedThemeWireV1, ThemeRecipeV1};
    use serde_json::{Value, json};

    fn design() -> Value {
        json!({
            "tokens": {
                "brand": {"type": "color", "value": "#123456"},
                "alpha": {"type": "number", "value": 0.5}
            },
            "styles": {
                "node": {"fill": {"kind": "solid", "color": {"ref": "brand"}}, "opacity": {"ref":"alpha"}},
                "edge": {"stroke": {"paint": {"kind": "solid", "color": {"ref": "brand"}}}}
            }
        })
    }

    #[test]
    fn binding_design_materialization_replays_shared_golden_contracts() {
        for vector in merman_theme_authoring_fixtures::THEME_AUTHORING_GOLDEN_VECTORS_V1 {
            let materialized: MaterializedThemeWireV1 = serde_json::from_slice(
                &materialize_theme_design_json(vector.readable_design_json()).unwrap(),
            )
            .unwrap();
            assert_eq!(
                materialized.spec().canonical_json_bytes().unwrap(),
                vector.canonical_spec_json(),
                "{}",
                vector.name()
            );
            assert_eq!(
                serde_json::to_value(materialized.references()).unwrap(),
                serde_json::from_str::<Value>(vector.references_json()).unwrap(),
                "{} authored bindings",
                vector.name()
            );
            let design: merman_theme_contract::ThemeDesignV1 =
                serde_json::from_slice(vector.readable_design_json()).unwrap();
            assert_eq!(
                design.canonical_json_bytes().unwrap(),
                vector.canonical_design_json(),
                "{}",
                vector.name()
            );
        }
    }

    #[test]
    fn binding_design_materialization_resolves_shared_values() {
        let input = serde_json::to_vec(&design()).unwrap();
        let result = materialize_theme_design_json(&input).unwrap();
        let result: MaterializedThemeWireV1 = serde_json::from_slice(&result).unwrap();
        let actual = serde_json::to_value(result.spec()).unwrap();
        assert_eq!(
            actual["styles"][0]["style"]["stroke"]["paint"],
            json!({"kind":"solid","color":"#123456"})
        );
        assert_eq!(
            actual["styles"][1]["style"]["fill"],
            json!({"kind":"solid","color":"#123456"})
        );
        let references = result.references();
        let colors: Vec<_> = references
            .iter()
            .filter(|reference| reference.token == "brand")
            .map(|reference| {
                (
                    reference.path.as_str(),
                    reference.domain.value_type.as_str(),
                )
            })
            .collect();
        assert_eq!(
            colors,
            vec![
                ("/styles/edge/stroke/paint/color", "color"),
                ("/styles/node/fill/color", "color")
            ]
        );
        let alpha = references
            .iter()
            .find(|reference| reference.token == "alpha")
            .unwrap();
        assert_eq!(alpha.path, "/styles/node/opacity");
        assert_eq!(alpha.domain.min, Some(0.0));
        assert_eq!(alpha.domain.max, Some(1.0));
        compile_theme_design_json(&input).expect("the materialized design must compile");
        materialize_theme_design_json(b"{}").expect("an empty design is a valid starting point");
    }

    #[test]
    fn design_failures_keep_authored_paths_and_closed_diagnostics() {
        let mut input = design();
        input["tokens"]["brand"]["type"] = json!("number");
        input["tokens"]["brand"]["value"] = json!(3);
        let error =
            materialize_theme_design_json(&serde_json::to_vec(&input).unwrap()).unwrap_err();
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        let payload: Value =
            serde_json::from_slice(&crate::binding_error_payload_json_bytes(&error)).unwrap();
        assert_eq!(payload["details"]["theme_authoring"]["schema_version"], 1);
        assert_eq!(
            payload["details"]["theme_authoring"]["diagnostics"][0]["path"],
            "/styles/edge/stroke/paint/color"
        );
    }

    #[test]
    fn design_json_rejects_old_contract_and_duplicate_members() {
        for input in [
            br#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{}}"#.as_slice(),
            br#"{"tokens":{"brand":{"type":"number","value":1},"brand":{"type":"number","value":2}}}"#,
        ] {
            let error = materialize_theme_design_json(input).unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        }
    }

    #[test]
    fn design_raw_byte_budget_precedes_json_decoding() {
        let policy = ThemeResourcePolicy::default()
            .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, 1)
            .unwrap();
        let error = materialize_theme_design_json_with_resource_policy(b"{}", &policy).unwrap_err();
        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
        assert_eq!(
            error.resource_details().unwrap().limit_id,
            "max_theme_encoded_bytes"
        );
    }

    #[test]
    fn preset_export_and_support_discovery_keep_existing_operations() {
        let bytes = export_theme_preset_json(b"editor-light").unwrap();
        let recipe: ThemeRecipeV1 = serde_json::from_slice(&bytes).unwrap();
        assert!(matches!(
            recipe,
            ThemeRecipeV1::Design { .. } | ThemeRecipeV1::CompleteSpec { .. }
        ));
        crate::compile_theme_selection_json(&bytes).unwrap();
        let support = describe_theme_support_json(br#"{"schema_version":1,"family":"sequence","output":"standalone-svg","subject":{"kind":"base-typography","property":"font-stack"}}"#).unwrap();
        let support: Value = serde_json::from_slice(&support).unwrap();
        assert_eq!(support["schema_version"], 1);
        assert!(export_theme_preset_json(b"future-preset").is_err());
    }
}
