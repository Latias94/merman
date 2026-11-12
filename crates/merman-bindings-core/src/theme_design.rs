use merman::diagram_theme::{
    ThemeDesignCompileError, ThemeMaterializationErrorV1, ThemeSupportQueryV1,
    describe_theme_support as describe_theme_support_with_renderer,
    materialize_theme_design_json_with_bindings,
};
use merman::svg::{DiagramTheme, DiagramThemeCompiler, ThemeResourceLimitId, ThemeResourcePolicy};
use merman_theme_contract::{ThemeAppearance, ThemeRecipeV1};
use serde::Deserialize;
use serde_json::value::RawValue;

use crate::common::BindingError;

pub(crate) const THEME_AUTHORING_REQUEST_SCHEMA_VERSION_V1: u32 = 1;
pub(crate) const THEME_AUTHORING_RESULT_SCHEMA_VERSION_V1: u32 = 1;

fn general_binding_theme_resource_policy() -> ThemeResourcePolicy {
    ThemeResourcePolicy::for_profile(
        merman::model::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE,
    )
}

/// Materializes a raw design or executes a versioned complete-document authoring request.
pub fn materialize_theme_design_json(bytes: &[u8]) -> Result<Vec<u8>, BindingError> {
    let policy = general_binding_theme_resource_policy();
    materialize_theme_design_json_with_resource_policy(bytes, &policy)
}

/// Executes theme authoring under a caller-owned resource policy.
pub fn materialize_theme_design_json_with_resource_policy(
    bytes: &[u8],
    policy: &ThemeResourcePolicy,
) -> Result<Vec<u8>, BindingError> {
    let compiler = DiagramThemeCompiler::new().with_resource_policy(policy.clone());
    materialize_theme_json_with(&compiler, bytes)
}

#[derive(Deserialize)]
struct ThemeAuthoringInputProbe<'a> {
    #[serde(default, borrow, deserialize_with = "crate::theme::present_raw_value")]
    schema_version: Option<&'a RawValue>,
    #[serde(default, borrow, deserialize_with = "crate::theme::present_raw_value")]
    recipe: Option<&'a RawValue>,
}

#[derive(Deserialize)]
#[serde(try_from = "String")]
enum ThemeAuthoringAction {
    Inspect,
    Materialize,
}

impl TryFrom<String> for ThemeAuthoringAction {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "inspect" => Ok(Self::Inspect),
            "materialize" => Ok(Self::Materialize),
            _ => Err("theme authoring action must be `inspect` or `materialize`"),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeAuthoringRequestV1 {
    schema_version: u32,
    action: ThemeAuthoringAction,
    recipe: ThemeRecipeV1,
    #[serde(default, deserialize_with = "crate::theme::deserialize_appearance")]
    appearance: Option<ThemeAppearance>,
}

pub(crate) fn materialize_theme_json_with(
    compiler: &DiagramThemeCompiler,
    bytes: &[u8],
) -> Result<Vec<u8>, BindingError> {
    compiler
        .check_design_json_input(bytes)
        .map_err(|error| materialization_error(compiler.resource_policy(), error))?;
    let probe: ThemeAuthoringInputProbe<'_> = serde_json::from_slice(bytes).map_err(|error| {
        BindingError::invalid_options_json(format!("invalid theme authoring JSON: {error}"))
    })?;
    if let Some(version) = probe.schema_version {
        check_authoring_request_version(version)?;
        if let Some(recipe) = probe.recipe {
            crate::theme::check_theme_recipe_envelope_after_structure(recipe)?;
        }
        let request: ThemeAuthoringRequestV1 = serde_json::from_slice(bytes).map_err(|error| {
            BindingError::invalid_options_json(format!("invalid theme authoring request: {error}"))
        })?;
        if request.schema_version != THEME_AUTHORING_REQUEST_SCHEMA_VERSION_V1 {
            return Err(invalid_authoring_request_version());
        }
        return match request.action {
            ThemeAuthoringAction::Inspect => {
                if request.appearance.is_some() {
                    return Err(BindingError::invalid_options_json(
                        "theme authoring inspect must not select an appearance",
                    ));
                }
                let inspection = compiler
                    .inspect_document(&request.recipe)
                    .map_err(|error| design_compile_error(compiler.resource_policy(), error))?;
                serde_json::to_vec(&inspection).map_err(crate::common::internal_json_error)
            }
            ThemeAuthoringAction::Materialize => {
                let materialized = compiler
                    .materialize_document(&request.recipe, request.appearance)
                    .map_err(|error| design_compile_error(compiler.resource_policy(), error))?;
                serde_json::to_vec(&materialized).map_err(crate::common::internal_json_error)
            }
        };
    }
    let policy = compiler.resource_policy();
    let materialized = materialize_theme_design_json_with_bindings(bytes, policy)
        .map_err(|error| materialization_error(policy, error))?;
    serde_json::to_vec(&materialized).map_err(crate::common::internal_json_error)
}

fn check_authoring_request_version(version: &RawValue) -> Result<(), BindingError> {
    if serde_json::from_str::<u64>(version.get()).ok()
        == Some(u64::from(THEME_AUTHORING_REQUEST_SCHEMA_VERSION_V1))
    {
        Ok(())
    } else {
        Err(invalid_authoring_request_version())
    }
}

fn invalid_authoring_request_version() -> BindingError {
    BindingError::invalid_options_json(format!(
        "theme authoring request schema_version must be integer {THEME_AUTHORING_REQUEST_SCHEMA_VERSION_V1}"
    ))
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

/// Exports a canonical or concrete preset ID as a complete editable version 1 recipe.
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
    let export = compiler
        .export_preset_document(id)
        .map_err(|error| design_compile_error(compiler.resource_policy(), error))?;
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
        .compile_document(ThemeRecipeV1::Design { design }, None)
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
            .map(merman::model::resources::ResourceProfile::id)
            .unwrap_or("custom-theme-policy");
        BindingError::resource_limit(
            descriptor.phase.as_str(),
            descriptor.stable_id,
            actual,
            max,
            profile,
            error.to_string(),
        )
    } else if diagnostic.path() == "/preset" {
        BindingError::invalid_argument(format!("invalid theme.preset: {error}"))
    } else {
        BindingError::invalid_argument(format!("invalid theme design: {error}"))
    };
    projected.with_theme_authoring_details(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BindingStatus;
    use merman_theme_contract::{
        MaterializedThemeRecipeV1, MaterializedThemeWireV1, ThemeRecipeInspectionV1, ThemeRecipeV1,
    };
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

    fn paired_recipe() -> Value {
        serde_json::from_slice(include_bytes!(
            "../../merman-theme-authoring-fixtures/fixtures/authoring-v1/paired.recipe.json"
        ))
        .unwrap()
    }

    fn authoring_error_payload_without_messages(error: &BindingError) -> Value {
        let mut payload: Value =
            serde_json::from_slice(&crate::binding_error_payload_json_bytes(error)).unwrap();
        let message = payload.as_object_mut().unwrap().remove("message").unwrap();
        assert!(!message.as_str().unwrap().trim().is_empty());
        for diagnostic in payload["details"]["theme_authoring"]["diagnostics"]
            .as_array_mut()
            .expect("typed theme-authoring diagnostics")
        {
            let message = diagnostic
                .as_object_mut()
                .unwrap()
                .remove("message")
                .expect("theme-authoring diagnostic message");
            assert!(!message.as_str().unwrap().trim().is_empty());
        }
        payload
    }

    #[test]
    fn binding_design_materialization_replays_shared_error_contracts() {
        let vectors: Value = serde_json::from_str(include_str!(
            "../../merman-theme-authoring-fixtures/fixtures/authoring-v1/errors.json"
        ))
        .unwrap();
        for vector in vectors.as_array().unwrap() {
            let input = vector["source"].as_str().unwrap().as_bytes();
            let options = vector["options_json"].as_str().unwrap_or("").as_bytes();
            let (_, policy) = crate::prepare_theme_authoring_options_json(options, None).unwrap();
            let mut expected_details = json!({"theme_authoring": vector["theme_authoring"]});
            if let Some(resource) = vector.get("resource") {
                expected_details["resource"] = resource.clone();
            }
            let expected = json!({
                "version": crate::BINDING_RESULT_PAYLOAD_VERSION,
                "ok": false,
                "code": vector["typst_status_code"],
                "code_name": vector["code_name"],
                "kind": vector["typst_error_kind"],
                "capability_id": null,
                "details": expected_details
            });
            for (entry, error) in [
                (
                    "direct",
                    materialize_theme_design_json_with_resource_policy(input, &policy).unwrap_err(),
                ),
                (
                    "operation",
                    crate::execute_once(
                        crate::BindingOperationRequest::new("materialize-theme-json", input)
                            .with_options_json(options),
                    )
                    .unwrap_err(),
                ),
            ] {
                assert_eq!(
                    authoring_error_payload_without_messages(&error),
                    expected,
                    "{} via {entry}",
                    vector["id"]
                );
            }
        }
    }

    #[test]
    fn raw_paired_design_materialization_keeps_its_result_contract_and_uses_the_default() {
        let mut recipe = paired_recipe();
        for (appearance, color) in [("light", "#123456"), ("dark", "#93c5fd")] {
            recipe["design"]["default_appearance"] = json!(appearance);
            let input = serde_json::to_vec(&recipe["design"]).unwrap();
            let output = materialize_theme_design_json(&input).unwrap();
            let materialized: MaterializedThemeWireV1 = serde_json::from_slice(&output).unwrap();
            let spec = serde_json::to_value(materialized.spec()).unwrap();
            assert_eq!(spec["styles"][0]["style"]["fill"]["color"], color);
            let result: Value = serde_json::from_slice(&output).unwrap();
            assert_eq!(result["schema_version"], 1);
            assert!(result.get("effective_appearance").is_none());
            let raw = compile_theme_design_json(&input).unwrap();
            let document =
                crate::compile_theme_selection_json(&serde_json::to_vec(&recipe).unwrap()).unwrap();
            assert_eq!(raw.recipe_fingerprint(), document.recipe_fingerprint());
        }
        recipe["design"]["appearances"]["light"]["styles"] =
            json!({"node": {"fill": {"kind": "solid", "color": {"ref": "missing"}}}});
        let error = materialize_theme_design_json(&serde_json::to_vec(&recipe["design"]).unwrap())
            .unwrap_err();
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
    }

    #[test]
    fn versioned_authoring_inspects_the_complete_document_with_reference_provenance() {
        let request = json!({
            "schema_version": 1, "action": "inspect", "recipe": paired_recipe()
        });
        let result = materialize_theme_design_json(&serde_json::to_vec(&request).unwrap()).unwrap();
        let inspection: ThemeRecipeInspectionV1 = serde_json::from_slice(&result).unwrap();
        assert_eq!(inspection.schema_version(), 1);
        assert_eq!(
            inspection.supported_appearances,
            [ThemeAppearance::Light, ThemeAppearance::Dark]
        );
        assert_eq!(inspection.default_appearance, Some(ThemeAppearance::Light));
        assert_eq!(inspection.appearances.len(), 2);
        for (index, token_path) in [
            "/design/tokens/brand",
            "/design/appearances/dark/tokens/brand",
        ]
        .into_iter()
        .enumerate()
        {
            let brand = inspection.appearances[index]
                .references
                .iter()
                .find(|reference| reference.token == "brand")
                .unwrap();
            assert_eq!(brand.path, "/design/styles/node/fill/color");
            assert_eq!(brand.token_path, token_path);
        }
        let value: Value = serde_json::from_slice(&result).unwrap();
        assert!(value.get("spec").is_none());
        assert!(value.get("effective_appearance").is_none());
    }

    #[test]
    fn versioned_authoring_materializes_the_default_or_explicit_appearance() {
        for (appearance, effective, color, token_path) in [
            (None, "light", "#123456", "/design/tokens/brand"),
            (Some("light"), "light", "#123456", "/design/tokens/brand"),
            (
                Some("dark"),
                "dark",
                "#93c5fd",
                "/design/appearances/dark/tokens/brand",
            ),
        ] {
            let mut request = json!({
                "schema_version": 1, "action": "materialize", "recipe": paired_recipe()
            });
            if let Some(appearance) = appearance {
                request["appearance"] = json!(appearance);
            }
            let result =
                materialize_theme_design_json(&serde_json::to_vec(&request).unwrap()).unwrap();
            let materialized: MaterializedThemeRecipeV1 = serde_json::from_slice(&result).unwrap();
            assert_eq!(materialized.schema_version(), 1);
            let value: Value = serde_json::from_slice(&result).unwrap();
            assert_eq!(value["effective_appearance"], effective);
            assert_eq!(value["default_appearance"], "light");
            assert_eq!(value["spec"]["styles"][0]["style"]["fill"]["color"], color);
            let brand = materialized
                .references
                .iter()
                .find(|reference| reference.token == "brand")
                .unwrap();
            assert_eq!(brand.path, "/design/styles/node/fill/color");
            assert_eq!(brand.token_path, token_path);
            crate::compile_theme_selection_json(
                &serde_json::to_vec(&json!({"spec": materialized.spec})).unwrap(),
            )
            .unwrap();
        }
    }

    #[test]
    fn authoring_keeps_concrete_recipes_unclassified() {
        for recipe in [
            json!({"schema_version": 1, "kind": "design", "design": design()}),
            json!({"schema_version": 1, "kind": "complete_spec", "complete_spec": {}}),
        ] {
            let request = json!({
                "schema_version": 1, "action": "materialize", "recipe": recipe
            });
            let result: Value = serde_json::from_slice(
                &materialize_theme_design_json(&serde_json::to_vec(&request).unwrap()).unwrap(),
            )
            .unwrap();
            assert_eq!(result["schema_version"], 1);
            assert_eq!(result["supported_appearances"], json!([]));
            assert!(result["effective_appearance"].is_null());
        }
    }

    #[test]
    fn versioned_authoring_rejects_closed_shape_and_nonconcrete_selectors() {
        let base = json!({
            "schema_version": 1, "action": "materialize", "recipe": paired_recipe()
        });
        let mut requests = Vec::new();
        for appearance in [Value::Null, json!("auto"), json!("future"), json!(1)] {
            let mut request = base.clone();
            request["appearance"] = appearance;
            requests.push(request);
        }
        for appearance in ["light", "dark"] {
            let mut request = base.clone();
            request["action"] = json!("inspect");
            request["appearance"] = json!(appearance);
            requests.push(request);
        }
        for version in [Value::Null, json!(0), json!(2), json!(1.0), json!("1")] {
            let mut request = base.clone();
            request["schema_version"] = version;
            requests.push(request);
        }
        let mut unknown = base.clone();
        unknown["unknown"] = json!(true);
        requests.push(unknown);
        for action in [
            json!("compile"),
            json!({"inspect": null}),
            json!({"materialize": null}),
        ] {
            let mut request = base.clone();
            request["action"] = action;
            requests.push(request);
        }
        let mut missing_recipe = base;
        missing_recipe.as_object_mut().unwrap().remove("recipe");
        requests.push(missing_recipe);
        for request in requests {
            let error =
                materialize_theme_design_json(&serde_json::to_vec(&request).unwrap()).unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError, "{request}");
        }
    }

    #[test]
    fn versioned_authoring_admits_raw_wrappers_and_every_inactive_appearance() {
        for input in [
            r#"{"schema_version":1,"schema_\u0076ersion":1,"action":"inspect","recipe":{"schema_version":1,"kind":"design","design":{}}}"#,
            r#"{"schema_version":1,"action":"inspect","act\u0069on":"materialize","recipe":{"schema_version":1,"kind":"design","design":{}}}"#,
            r#"{"schema_version":1,"action":"inspect","recipe":{"schema_version":1,"kind":"design","design":{}},"rec\u0069pe":{"schema_version":1,"kind":"design","design":{}}}"#,
            r#"{"schema_version":1,"action":"inspect","recipe":{"schema_version":1,"schema_version":1,"kind":"design","design":{}}}"#,
            r#"{"schema_version":1,"action":"materialize","appearance":"light","recipe":{"schema_version":1,"kind":"design","design":{"appearances":{"light":{},"dark":{"styles":{},"styles":{}}},"default_appearance":"light"}}}"#,
            r#"{"schema_version":1,"action":"inspect","recipe":{"schema_version":1.0,"kind":"design","design":{}}}"#,
        ] {
            let error = materialize_theme_design_json(input.as_bytes()).unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError, "{input}");
        }
        let mut recipe = paired_recipe();
        recipe["design"]["appearances"]["dark"]["styles"] =
            json!({"node": {"fill": {"kind": "solid", "color": {"ref": "missing"}}}});
        for action in ["inspect", "materialize"] {
            let request = json!({
                "schema_version": 1, "action": action, "recipe": recipe
            });
            let error =
                materialize_theme_design_json(&serde_json::to_vec(&request).unwrap()).unwrap_err();
            assert_eq!(error.status(), BindingStatus::InvalidArgument);
            assert_eq!(
                error.theme_authoring_details().unwrap().diagnostic().path(),
                "/design/appearances/dark/styles/node/fill/color"
            );
        }
    }

    #[test]
    fn versioned_authoring_keeps_outer_structure_and_byte_error_precedence() {
        let recipe = r#"{"schema_version":2,"schema_\u0076ersion":1,"kind":"complete_spec","complete_spec":{"future_field":true}}"#;
        let input = format!(r#"{{"schema_version":2,"action":"inspect","recipe":{recipe}}}"#);
        for (limit, status) in [
            (input.len(), BindingStatus::OptionsJsonError),
            (input.len() - 1, BindingStatus::ResourceLimitExceeded),
        ] {
            assert!(
                recipe.len() <= limit,
                "the nested recipe fits the byte limit"
            );
            let compiler = DiagramThemeCompiler::new().with_resource_policy(
                ThemeResourcePolicy::constrained()
                    .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, limit)
                    .unwrap(),
            );
            let expected = materialization_error(
                compiler.resource_policy(),
                compiler
                    .check_design_json_input(input.as_bytes())
                    .unwrap_err(),
            );
            assert_eq!(expected.status(), status);
            assert_eq!(
                materialize_theme_json_with(&compiler, input.as_bytes()).unwrap_err(),
                expected
            );
        }
    }

    #[test]
    fn versioned_authoring_charges_the_entire_raw_request_before_decoding() {
        for action in ["inspect", "materialize"] {
            let request = serde_json::to_vec(&json!({
                "schema_version": 1, "action": action, "recipe": paired_recipe()
            }))
            .unwrap();
            let policy = ThemeResourcePolicy::default()
                .with_limit(
                    ThemeResourceLimitId::MaxThemeEncodedBytes,
                    request.len() - 1,
                )
                .unwrap();
            let error =
                materialize_theme_design_json_with_resource_policy(&request, &policy).unwrap_err();
            assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
            let resource = error.resource_details().unwrap();
            assert_eq!(resource.limit_id, "max_theme_encoded_bytes");
            assert_eq!(resource.actual, u64::try_from(request.len()).unwrap());
            assert_eq!(resource.max, u64::try_from(request.len() - 1).unwrap());
            let payload = authoring_error_payload_without_messages(&error);
            assert_eq!(
                payload["details"]["theme_authoring"],
                json!({
                    "schema_version": 1,
                    "diagnostics": [{
                        "code": "theme-authoring.resource-limit-exceeded",
                        "severity": "error",
                        "path": "",
                        "details": {
                            "limit_id": "max_theme_encoded_bytes",
                            "actual": request.len(),
                            "max": request.len() - 1
                        }
                    }]
                }),
                "{action}"
            );
        }
    }

    #[test]
    fn grouped_and_concrete_preset_ids_export_complete_version_one_recipes() {
        let exported = export_theme_preset_json(b"editor").unwrap();
        let recipe: ThemeRecipeV1 = serde_json::from_slice(&exported).unwrap();
        assert!(matches!(recipe, ThemeRecipeV1::Design { .. }));
        let value: Value = serde_json::from_slice(&exported).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert!(value["design"]["appearances"]["light"].is_object());
        assert!(value["design"]["appearances"]["dark"].is_object());
        for appearance in ["light", "dark"] {
            crate::compile_theme_selection_json(
                &serde_json::to_vec(&json!({
                    "recipe": value, "appearance": appearance
                }))
                .unwrap(),
            )
            .unwrap();
        }
        for preset in [b"editor-light".as_slice(), b"editor-dark"] {
            let concrete: Value =
                serde_json::from_slice(&export_theme_preset_json(preset).unwrap()).unwrap();
            assert_eq!(concrete["schema_version"], 1);
            crate::compile_theme_selection_json(&serde_json::to_vec(&concrete).unwrap()).unwrap();
        }
    }

    #[test]
    fn preset_export_accepts_only_plain_utf8_ids() {
        for request in [
            r#"{"preset":"editor"}"#,
            r#"{"schema_version":1,"preset":"editor"}"#,
            r#"{"schema_version":2.0,"preset":"editor"}"#,
            r#"{"schema_version":2,"schema_version":2,"preset":"editor"}"#,
            r#"{"schema_version":2,"preset":"editor","pres\u0065t":"editor"}"#,
            r#"{"schema_version":2,"preset":"editor","appearance":"dark"}"#,
            r#"{"schema_version":2,"preset":null}"#,
            r#"{"schema_version":2,"preset":"editor","unknown":true}"#,
        ] {
            let error = export_theme_preset_json(request.as_bytes()).unwrap_err();
            assert_eq!(error.status(), BindingStatus::InvalidArgument, "{request}");
        }
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
