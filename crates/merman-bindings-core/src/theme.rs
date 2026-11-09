use merman::svg::{DiagramTheme, DiagramThemeCompiler, ThemeResourcePolicy};
use merman_theme_contract::{
    DiagramThemeSpecWireV1, THEME_RECIPE_SCHEMA_VERSION_V1, ThemeAppearance, ThemeRecipeV1,
};
use serde::{Deserialize, Deserializer, de::Error as _};
use serde_json::{Value, value::RawValue};

use crate::common::{BindingError, BindingStatus};

#[derive(Debug)]
pub(crate) enum BindingThemeOptionsJson {
    Selection(ThemeSelectionJson),
    Recipe(ThemeRecipeV1),
}

#[derive(Debug)]
#[expect(
    clippy::large_enum_variant,
    reason = "One decoded selection retains its complete typed recipe or specification"
)]
pub(crate) enum ThemeSelectionJson {
    Preset {
        preset: String,
        appearance: Option<ThemeAppearance>,
    },
    Recipe {
        recipe: ThemeRecipeV1,
        appearance: Option<ThemeAppearance>,
    },
    Spec {
        spec: DiagramThemeSpecWireV1,
    },
}

pub(crate) fn deserialize_appearance<'de, D>(
    deserializer: D,
) -> Result<Option<ThemeAppearance>, D::Error>
where
    D: Deserializer<'de>,
{
    ThemeAppearance::deserialize(deserializer).map(Some)
}

/// Decodes one theme value after its tagged union has been validated.
fn decode_validated_theme_value(
    value: Value,
) -> Result<BindingThemeOptionsJson, serde_json::Error> {
    if value.get("kind").is_some() || value.get("schema_version").is_some() {
        serde_json::from_value(value).map(BindingThemeOptionsJson::Recipe)
    } else {
        let mut object: serde_json::Map<String, Value> = serde_json::from_value(value)?;
        let selection = if let Some(preset) = object.remove("preset") {
            ThemeSelectionJson::Preset {
                preset: serde_json::from_value(preset)?,
                appearance: object
                    .remove("appearance")
                    .map(serde_json::from_value)
                    .transpose()?,
            }
        } else if let Some(recipe) = object.remove("recipe") {
            ThemeSelectionJson::Recipe {
                recipe: serde_json::from_value(recipe)?,
                appearance: object
                    .remove("appearance")
                    .map(serde_json::from_value)
                    .transpose()?,
            }
        } else {
            ThemeSelectionJson::Spec {
                spec: serde_json::from_value(
                    object
                        .remove("spec")
                        .expect("validated specification selection"),
                )?,
            }
        };
        Ok(BindingThemeOptionsJson::Selection(selection))
    }
}

/// The options owner validates the union before decoding the remaining option groups.
/// Decode its payload at the ordinary Serde field position to preserve error precedence.
pub(crate) fn deserialize_validated_theme<'de, D>(
    deserializer: D,
) -> Result<Option<BindingThemeOptionsJson>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Value>::deserialize(deserializer)?
        .map(decode_validated_theme_value)
        .transpose()
        .map_err(D::Error::custom)
}

/// Validates and decodes the standalone theme selection once.
pub(crate) fn decode_theme_value(
    value: Value,
    error_context: &str,
) -> Result<BindingThemeOptionsJson, BindingError> {
    validate_theme_wire(Some(&value))?;
    decode_validated_theme_value(value)
        .map_err(|error| invalid_options(format!("invalid {error_context}: {error}")))
}

#[derive(Debug, Deserialize)]
struct BindingThemeInputProbe<'a> {
    #[serde(borrow)]
    theme: Option<&'a RawValue>,
}

/// Checks the exact encoded theme slice before Serde allocates the typed theme graph.
#[cfg(test)]
pub(crate) fn validate_theme_input_json(options_json: &[u8]) -> Result<(), BindingError> {
    let compiler =
        DiagramThemeCompiler::new().with_resource_policy(ThemeResourcePolicy::for_profile(
            merman::model::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE,
        ));
    validate_theme_input_json_with(&compiler, options_json)
}

/// Checks the exact encoded theme slice against a caller-owned host ceiling.
pub(crate) fn validate_theme_input_json_with(
    compiler: &DiagramThemeCompiler,
    options_json: &[u8],
) -> Result<(), BindingError> {
    if options_json.is_empty() {
        return Ok(());
    }
    let probe: BindingThemeInputProbe<'_> =
        serde_json::from_slice(options_json).map_err(|error| {
            BindingError::new(
                BindingStatus::OptionsJsonError,
                format!("invalid options_json: {error}"),
            )
        })?;
    let Some(theme) = probe.theme else {
        return Ok(());
    };
    compiler
        .check_encoded_input_bytes(theme.get().len())
        .map_err(theme_resource_error)?;
    check_theme_json_input(compiler, theme.get().as_bytes())
}

// Borrow payloads so version and authoring admission precede Value normalization. In particular,
// normalizing first would silently discard duplicate design, spec, or complete-spec fields.
#[derive(Deserialize)]
struct ThemeInputProbe<'a> {
    #[serde(default, borrow, deserialize_with = "present_raw_value")]
    schema_version: Option<&'a RawValue>,
    #[serde(default, borrow, deserialize_with = "present_raw_value")]
    kind: Option<&'a RawValue>,
    #[serde(default, borrow, deserialize_with = "present_raw_value")]
    complete_spec: Option<&'a RawValue>,
    #[serde(default, borrow, deserialize_with = "present_raw_value")]
    spec: Option<&'a RawValue>,
    #[serde(default, borrow, deserialize_with = "present_raw_value")]
    recipe: Option<&'a RawValue>,
}

pub(crate) fn present_raw_value<'de, D>(deserializer: D) -> Result<Option<&'de RawValue>, D::Error>
where
    D: Deserializer<'de>,
{
    <&RawValue>::deserialize(deserializer).map(Some)
}

fn check_theme_json_input(
    compiler: &DiagramThemeCompiler,
    bytes: &[u8],
) -> Result<(), BindingError> {
    check_theme_json_structure(compiler, bytes)?;
    let probe: ThemeInputProbe<'_> = serde_json::from_slice(bytes)
        .map_err(|error| invalid_options(format!("invalid theme JSON: {error}")))?;
    let spec = if probe.kind.is_none() && probe.schema_version.is_none() {
        if let Some(recipe) = probe.recipe {
            check_theme_recipe_json_input(compiler, recipe.get().as_bytes())?;
        }
        // Preserve the selection owner's null-payload diagnostic and error position.
        probe.spec.filter(|raw| raw.get() != "null")
    } else {
        check_recipe_version(
            probe
                .schema_version
                .and_then(|raw| serde_json::from_str::<u64>(raw.get()).ok()),
        )?;
        probe.complete_spec
    };
    if let Some(spec) = spec {
        serde_json::from_str::<DiagramThemeSpecWireV1>(spec.get())
            .map_err(|error| invalid_options(format!("invalid complete theme spec: {error}")))?;
    }
    Ok(())
}

pub(crate) fn check_theme_json_structure(
    compiler: &DiagramThemeCompiler,
    bytes: &[u8],
) -> Result<(), BindingError> {
    compiler
        .check_encoded_input_bytes(bytes.len())
        .map_err(theme_resource_error)?;
    compiler.check_design_json_input(bytes).map_err(|error| {
        crate::theme_design::materialization_error(compiler.resource_policy(), error)
    })
}

pub(crate) fn check_theme_recipe_json_input(
    compiler: &DiagramThemeCompiler,
    bytes: &[u8],
) -> Result<(), BindingError> {
    check_theme_json_structure(compiler, bytes)?;
    let probe: ThemeInputProbe<'_> = serde_json::from_slice(bytes)
        .map_err(|error| invalid_options(format!("invalid theme recipe JSON: {error}")))?;
    check_recipe_version(
        probe
            .schema_version
            .and_then(|raw| serde_json::from_str::<u64>(raw.get()).ok()),
    )?;
    if let Some(spec) = probe.complete_spec {
        serde_json::from_str::<DiagramThemeSpecWireV1>(spec.get())
            .map_err(|error| invalid_options(format!("invalid complete theme spec: {error}")))?;
    }
    Ok(())
}

fn check_recipe_version(version: Option<u64>) -> Result<(), BindingError> {
    if version == Some(u64::from(THEME_RECIPE_SCHEMA_VERSION_V1)) {
        return Ok(());
    }
    Err(invalid_options(format!(
        "unsupported theme recipe schema_version {}; expected integer {}",
        version
            .map(|value| value.to_string())
            .unwrap_or_else(|| "<missing or non-integer>".to_owned()),
        THEME_RECIPE_SCHEMA_VERSION_V1,
    )))
}

/// Compiles a versioned recipe or an exact preset/spec selection with default host policy.
pub fn compile_theme_selection_json(bytes: &[u8]) -> Result<DiagramTheme, BindingError> {
    let compiler =
        DiagramThemeCompiler::new().with_resource_policy(ThemeResourcePolicy::for_profile(
            merman::model::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE,
        ));
    compile_theme_selection_json_with(&compiler, bytes)
}

/// Compiles one exact theme selection with a caller-owned compiler policy.
pub fn compile_theme_selection_json_with(
    compiler: &DiagramThemeCompiler,
    bytes: &[u8],
) -> Result<DiagramTheme, BindingError> {
    compile_theme_selection_json_with_appearance(compiler, bytes, None)
}

/// Compiles an exact selection, requiring a file selector and caller selector to agree.
///
/// An omitted caller selector preserves the file's selection or deterministic document default.
/// Raw admission is shared with binding options and occurs before JSON normalization.
pub fn compile_theme_selection_json_with_appearance(
    compiler: &DiagramThemeCompiler,
    bytes: &[u8],
    appearance: Option<ThemeAppearance>,
) -> Result<DiagramTheme, BindingError> {
    compiler
        .check_encoded_input_bytes(bytes.len())
        .map_err(theme_resource_error)?;
    check_theme_json_input(compiler, bytes)?;
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid theme selection JSON: {error}"),
        )
    })?;
    let selection = decode_theme_value(value, "theme selection JSON")?;
    compile_theme_with_appearance(compiler, Some(&selection), appearance)?.ok_or_else(|| {
        BindingError::internal("validated theme selection did not produce a compiled theme")
    })
}

/// Validates the parts of the theme contract that must remain visible in the original JSON.
///
/// In particular, Serde's `Option` intentionally maps both an omitted field and JSON `null` to
/// `None`. The outer options overlay needs that behavior for `theme: null`, but the tagged union
/// itself must still reject `{}`, a null payload, or simultaneous `preset` and `spec` members.
pub(crate) fn validate_theme_wire(theme: Option<&Value>) -> Result<(), BindingError> {
    let Some(theme) = theme else {
        return Ok(());
    };
    if theme.is_null() {
        return Ok(());
    }
    let object = theme
        .as_object()
        .ok_or_else(|| invalid_options("options field `theme` must be an object or null"))?;
    let selections: Vec<_> = ["preset", "recipe", "spec"]
        .into_iter()
        .filter(|key| object.contains_key(*key))
        .collect();
    if object.contains_key("kind") || object.contains_key("schema_version") {
        if !selections.is_empty() || object.contains_key("appearance") {
            return Err(invalid_options(
                "a direct theme recipe must not contain `preset`, `recipe`, `spec`, or `appearance`; use a recipe selection wrapper for an appearance",
            ));
        }
        check_recipe_version(object.get("schema_version").and_then(Value::as_u64))?;
        return Ok(());
    }
    if selections.len() != 1 {
        if selections.len() > 1 {
            return Err(invalid_options(format!(
                "options field `theme` must not contain both `{}` and `{}`",
                selections[0], selections[1]
            )));
        }
        return Err(invalid_options(
            "options field `theme` must contain exactly one of `preset`, `recipe`, or `spec`",
        ));
    }
    let selection = selections[0];
    if object.get(selection).is_some_and(Value::is_null) {
        return Err(invalid_options(
            "options field `theme.preset`, `theme.recipe`, or `theme.spec` must not be null",
        ));
    }
    for key in object.keys() {
        if key != selection && !(key == "appearance" && selection != "spec") {
            return Err(invalid_options(format!(
                "unknown theme selection field `{key}`"
            )));
        }
    }
    if let Some(appearance) = object.get("appearance")
        && !matches!(appearance.as_str(), Some("light" | "dark"))
    {
        return Err(invalid_options(
            "theme.appearance must be `light` or `dark`; resolve host preferences before rendering",
        ));
    }
    if selection == "recipe" {
        let recipe = object.get("recipe").expect("the selected recipe exists");
        let recipe = recipe
            .as_object()
            .ok_or_else(|| invalid_options("theme.recipe must be a versioned recipe object"))?;
        check_recipe_version(recipe.get("schema_version").and_then(Value::as_u64))?;
    }
    Ok(())
}

pub(crate) fn compile_theme_with(
    compiler: &DiagramThemeCompiler,
    selection: Option<&BindingThemeOptionsJson>,
) -> Result<Option<DiagramTheme>, BindingError> {
    compile_theme_with_appearance(compiler, selection, None)
}

fn selected_appearance(
    file: Option<ThemeAppearance>,
    requested: Option<ThemeAppearance>,
) -> Result<Option<ThemeAppearance>, BindingError> {
    if let (Some(file), Some(requested)) = (file, requested)
        && file != requested
    {
        return Err(BindingError::invalid_argument(format!(
            "requested theme appearance `{requested}` conflicts with the theme selection appearance `{file}`"
        )));
    }
    Ok(requested.or(file))
}

fn compile_theme_with_appearance(
    compiler: &DiagramThemeCompiler,
    selection: Option<&BindingThemeOptionsJson>,
    requested_appearance: Option<ThemeAppearance>,
) -> Result<Option<DiagramTheme>, BindingError> {
    let Some(selection) = selection else {
        return Ok(None);
    };
    let selection = match selection {
        BindingThemeOptionsJson::Recipe(recipe) => {
            return compiler
                .compile_document(recipe.clone(), requested_appearance)
                .map(Some)
                .map_err(|error| {
                    crate::theme_design::design_compile_error(compiler.resource_policy(), error)
                });
        }
        BindingThemeOptionsJson::Selection(selection) => selection,
    };
    let theme = match selection {
        ThemeSelectionJson::Preset { preset, appearance } => compiler
            .compile_preset_selection(
                preset,
                selected_appearance(*appearance, requested_appearance)?,
            )
            .map_err(|error| {
                crate::theme_design::design_compile_error(compiler.resource_policy(), error)
            })?,
        ThemeSelectionJson::Recipe { recipe, appearance } => compiler
            .compile_document(
                recipe.clone(),
                selected_appearance(*appearance, requested_appearance)?,
            )
            .map_err(|error| {
                crate::theme_design::design_compile_error(compiler.resource_policy(), error)
            })?,
        ThemeSelectionJson::Spec { spec } if requested_appearance.is_some() => compiler
            .compile_document(
                ThemeRecipeV1::CompleteSpec {
                    complete_spec: spec.clone(),
                },
                requested_appearance,
            )
            .map_err(|error| {
                crate::theme_design::design_compile_error(compiler.resource_policy(), error)
            })?,
        ThemeSelectionJson::Spec { spec } => compiler
            .compile_spec_wire(spec.clone())
            .map_err(theme_compile_error)?,
    };
    Ok(Some(theme))
}

pub(crate) fn theme_compile_error(error: merman::svg::ThemeCompileError) -> BindingError {
    match error {
        merman::svg::ThemeCompileError::FontCatalog(
            merman::svg::FontCatalogError::EmbeddedFontsUnavailable,
        ) => invalid_theme("theme", "embedded theme font resources are not supported"),
        merman::svg::ThemeCompileError::ResourceLimit(error)
        | merman::svg::ThemeCompileError::FontCatalog(
            merman::svg::FontCatalogError::ResourceLimit(error),
        ) => theme_resource_error(error),
        merman::svg::ThemeCompileError::Validation(error) => theme_validation_error(error),
        error => invalid_theme("theme", error.to_string()),
    }
}

pub(crate) fn theme_design_compile_error(error: merman::svg::ThemeCompileError) -> BindingError {
    match error {
        merman::svg::ThemeCompileError::FontCatalog(
            merman::svg::FontCatalogError::EmbeddedFontsUnavailable,
        ) => invalid_theme("theme", "embedded theme font resources are not supported"),
        merman::svg::ThemeCompileError::ResourceLimit(error)
        | merman::svg::ThemeCompileError::FontCatalog(
            merman::svg::FontCatalogError::ResourceLimit(error),
        ) => theme_resource_error(error),
        merman::svg::ThemeCompileError::Validation(error) => {
            let field = match &error {
                merman::svg::ThemeCompileValidationError::EmptyValue { field }
                | merman::svg::ThemeCompileValidationError::InvalidValue { field }
                | merman::svg::ThemeCompileValidationError::InvalidNumber { field }
                | merman::svg::ThemeCompileValidationError::InvalidCollection { field }
                | merman::svg::ThemeCompileValidationError::LimitExceeded { field }
                | merman::svg::ThemeCompileValidationError::DuplicateId { field }
                | merman::svg::ThemeCompileValidationError::UnknownId { field }
                | merman::svg::ThemeCompileValidationError::UnsupportedMermaidTheme {
                    field, ..
                } => *field,
                _ => "design",
            };
            invalid_theme(format!("theme design {field}"), error.to_string())
        }
        error => invalid_theme("theme design", error.to_string()),
    }
}

fn theme_validation_error(error: merman::svg::ThemeCompileValidationError) -> BindingError {
    let message = error.to_string();
    let field = match &error {
        merman::svg::ThemeCompileValidationError::EmptyValue { field }
        | merman::svg::ThemeCompileValidationError::InvalidValue { field }
        | merman::svg::ThemeCompileValidationError::InvalidNumber { field }
        | merman::svg::ThemeCompileValidationError::InvalidCollection { field }
        | merman::svg::ThemeCompileValidationError::LimitExceeded { field }
        | merman::svg::ThemeCompileValidationError::DuplicateId { field }
        | merman::svg::ThemeCompileValidationError::UnknownId { field }
        | merman::svg::ThemeCompileValidationError::UnsupportedMermaidTheme { field, .. } => {
            let field = match *field {
                "styles.rule.target" => "styles.target",
                "styles.rule.family" => "styles.family",
                "styles.rule.variant" => "styles.variant",
                field => field,
            };
            format!("theme.spec.{field}")
        }
        merman::svg::ThemeCompileValidationError::InvalidColor { .. } => {
            "theme.spec.paint".to_string()
        }
        merman::svg::ThemeCompileValidationError::InvalidTargetFamily { .. } => {
            "theme.spec.styles".to_string()
        }
        merman::svg::ThemeCompileValidationError::InvalidCanvasFamilyScope { .. } => {
            "theme.spec.canvas".to_string()
        }
        _ => "theme.spec".to_string(),
    };
    invalid_theme(field, message)
}

pub(crate) fn theme_resource_error(error: merman::svg::ThemeResourceLimitExceeded) -> BindingError {
    let actual = u64::try_from(error.actual).unwrap_or(u64::MAX);
    let max = u64::try_from(error.max).unwrap_or(u64::MAX);
    let profile = error
        .profile
        .map(merman::model::resources::ResourceProfile::id)
        .unwrap_or("custom-theme-policy");
    BindingError::resource_limit(
        error.phase.as_str(),
        error.limit,
        actual,
        max,
        profile,
        error.to_string(),
    )
}

fn invalid_theme(field: impl Into<String>, message: impl Into<String>) -> BindingError {
    let field = field.into();
    BindingError::new(
        BindingStatus::InvalidArgument,
        format!("invalid {field}: {}", message.into()),
    )
}

fn invalid_options(message: impl Into<String>) -> BindingError {
    BindingError::new(BindingStatus::OptionsJsonError, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman::diagram_theme::{ThemeDesignV1, materialize_theme_design};
    use merman::svg::{ThemeCapability, ThemeResourceLimitId};

    const PAIRED_RECIPE: &[u8] = include_bytes!(
        "../../merman-theme-authoring-fixtures/fixtures/authoring-v1/paired.recipe.json"
    );

    fn paired_recipe() -> Value {
        serde_json::from_slice(PAIRED_RECIPE).unwrap()
    }

    #[test]
    fn theme_wire_requires_exactly_one_selection() {
        for invalid in [
            serde_json::json!({}),
            serde_json::json!({"preset": "editor-dark", "spec": {}}),
            serde_json::json!({"preset": null}),
            serde_json::json!({"spec": null}),
            serde_json::json!({"schema_version": 1, "kind": "complete_spec", "complete_spec": {}, "preset": "editor-dark"}),
            serde_json::json!({"schema_version": 1, "kind": "complete_spec", "complete_spec": {}, "spec": {}}),
            serde_json::json!({"appearance": "dark"}),
            serde_json::json!({"preset": "editor", "appearance": null}),
            serde_json::json!({"preset": "editor", "appearance": "auto"}),
            serde_json::json!({"preset": "editor", "appearance": "future"}),
            serde_json::json!({"preset": "editor", "appearance": {}}),
            serde_json::json!({"preset": "editor", "recipe": paired_recipe()}),
            serde_json::json!({"recipe": null}),
            serde_json::json!({"recipe": paired_recipe(), "spec": {}}),
            serde_json::json!({"spec": {}, "appearance": "light"}),
            serde_json::json!({"recipe": paired_recipe(), "schema_version": 2}),
            serde_json::json!({"preset": "editor", "unexpected": true}),
            serde_json::json!({"schema_version": 2, "kind": "design", "design": {}, "appearance": "dark"}),
        ] {
            let error = validate_theme_wire(Some(&invalid)).unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        }

        validate_theme_wire(Some(&Value::Null)).unwrap();
        validate_theme_wire(Some(&serde_json::json!({"preset": "editor-light"}))).unwrap();
        validate_theme_wire(Some(&serde_json::json!({"spec": {}}))).unwrap();
        validate_theme_wire(Some(&serde_json::json!({
            "preset": "editor", "appearance": "dark"
        })))
        .unwrap();
        validate_theme_wire(Some(&serde_json::json!({
            "recipe": paired_recipe(), "appearance": "light"
        })))
        .unwrap();
    }

    #[test]
    fn exported_recipe_is_directly_importable_by_a_fresh_compiler() {
        let exporter = DiagramThemeCompiler::new();
        for preset in [
            merman::svg::ThemePreset::EditorDark,
            merman::svg::ThemePreset::Cyberpunk,
        ] {
            let recipe = exporter.export_preset(preset).unwrap();
            let bytes = serde_json::to_vec(&recipe).unwrap();
            let expected = exporter.compile_preset(preset).unwrap();
            let imported = compile_theme_selection_json_with(&DiagramThemeCompiler::new(), &bytes)
                .expect("an exported recipe must be accepted without envelope reconstruction");
            assert_eq!(imported.recipe_fingerprint(), expected.recipe_fingerprint());
        }
    }

    #[test]
    fn recipe_design_uses_typed_materialization_and_rejects_legacy_definition() {
        let design: ThemeDesignV1 = serde_json::from_value(serde_json::json!({
            "tokens": {"brand": {"type":"color", "value":"#123456"}},
            "styles": {"node": {"fill": {"kind":"solid", "color":{"ref":"brand"}}}}
        }))
        .unwrap();
        let bytes = serde_json::to_vec(&ThemeRecipeV1::Design {
            design: design.clone(),
        })
        .unwrap();
        let compiler = DiagramThemeCompiler::new();
        let expected = compiler
            .compile_spec_wire(materialize_theme_design(&design).unwrap())
            .unwrap();
        let imported = compile_theme_selection_json_with(&compiler, &bytes).unwrap();
        assert_eq!(imported.recipe_fingerprint(), expected.recipe_fingerprint());
        let legacy = br#"{"schema_version":1,"kind":"definition","definition":{"authoring_schema_version":1,"expansion_version":1,"tokens":{}}}"#;
        assert_eq!(
            compile_theme_selection_json(legacy).unwrap_err().status(),
            BindingStatus::OptionsJsonError
        );
    }

    #[test]
    fn recipe_design_and_complete_spec_render_through_both_engine_forms() {
        let recipe = serde_json::json!({
            "schema_version": 1, "kind": "design",
            "design": {
                "tokens": {"brand": {"type":"color", "value":"#123456"}},
                "styles": {"node": {"fill": {"kind":"solid", "color":{"ref":"brand"}}}}
            }
        });
        let complete_spec = merman::diagram_theme::materialize_theme_design(
            &serde_json::from_value::<ThemeDesignV1>(recipe["design"].clone()).unwrap(),
        )
        .unwrap();
        let source = b"flowchart LR\nA-->B\n";
        let options = serde_json::to_vec(&serde_json::json!({
            "svg": {"diagram_id":"authored-design"}, "theme": recipe,
        }))
        .unwrap();
        let complete_options = serde_json::to_vec(&serde_json::json!({
            "svg": {"diagram_id":"authored-design"},
            "theme": {"schema_version":1, "kind":"complete_spec", "complete_spec":complete_spec},
        }))
        .unwrap();
        let actual = crate::render_svg(source, &options).unwrap();
        assert_eq!(
            actual,
            crate::render_svg(source, &complete_options).unwrap()
        );
        assert_eq!(
            actual,
            crate::BindingEngine::new(&options)
                .unwrap()
                .render_svg(source)
                .unwrap()
        );

        let mut invalid: Value = serde_json::from_slice(&options).unwrap();
        invalid["theme"]["design"]["styles"]["node"]["fill"]["color"]["ref"] =
            Value::String("missing".to_owned());
        let error = crate::BindingEngine::new(&serde_json::to_vec(&invalid).unwrap())
            .err()
            .expect("a missing design reference must fail");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert_eq!(
            error.theme_authoring_details().unwrap().diagnostic().path(),
            "/design/styles/node/fill/color"
        );
    }

    #[test]
    fn recipe_version_errors_precede_future_payload_shape_errors() {
        let bytes =
            br#"{"kind":"complete_spec","complete_spec":{"future_field":true},"schema_version":3}"#;
        let error = compile_theme_selection_json(bytes).unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(
            error.message().contains("schema_version"),
            "{}",
            error.message()
        );
        assert!(error.message().contains('3'), "{}", error.message());
    }

    #[test]
    fn theme_raw_admission_rejects_duplicates_and_invalid_versions_in_both_entries() {
        let design = r#"{}"#;
        let mut invalid = vec![
            r#"{"schema_version":1,"kind":"complete_spec","complete_spec":{"future":true},"complete_spec":{}}"#.to_owned(),
            format!(r#"{{"kind":"design","schema_version":null,"schema_version":1,"design":{design}}}"#),
            format!(r#"{{"kind":"design","schema_version":2,"schema_\u0076ersion":1,"design":{design}}}"#),
            r#"{"kind":"design","schema_version":1,"design":{"tokens":{},"tokens":{}}}"#.to_owned(),
            r##"{"kind":"design","schema_version":1,"design":{"tokens":{"accent":{"type":"color","value":"#111111"},"acc\u0065nt":{"type":"color","value":"#222222"}}}}"##.to_owned(),
        ];
        for spec in [
            r##"{"styles":[{"kind":"rule","target":"node","style":{"fill":"#ff0000"}}],"styles":[]}"##,
            r##"{"canvas":{"base":"#ff0000"},"canvas":{"base":"#0000ff"}}"##,
            r##"{"canvas":{"base":"#ff0000","base":"#0000ff"}}"##,
            r##"{"styles":[{"kind":"rule","target":"node","style":{"fill":null,"fill":"#ff0000"}}]}"##,
        ] {
            invalid.push(format!(r#"{{"spec":{spec}}}"#));
            invalid.push(format!(
                r#"{{"schema_version":1,"kind":"complete_spec","complete_spec":{spec}}}"#
            ));
        }
        invalid.push(r#"{"spec":null,"sp\u0065c":{}}"#.to_owned());
        invalid.push(r#"{"schema_version":1,"kind":"complete_spec","complete_spec":null,"complete_\u0073pec":{}}"#.to_owned());
        for version in ["null", "0", "3", "1.0", "1e0", "\"1\"", "true"] {
            invalid.push(format!(
                r#"{{"design":{design},"kind":"design","schema_version":{version}}}"#
            ));
        }
        invalid.push(format!(r#"{{"kind":"design","design":{design}}}"#));
        let nested = format!("{}0{}", "[".repeat(40), "]".repeat(40));
        invalid.push(format!(
            r#"{{"kind":"design","schema_version":1,"design":{{"tokens":{{"accent":{nested}}}}}}}"#
        ));
        let tiny = DiagramThemeCompiler::new().with_resource_policy(
            ThemeResourcePolicy::default()
                .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, 1)
                .unwrap(),
        );
        for input in invalid {
            let options = format!(r#"{{"theme":{input}}}"#);
            for error in [
                compile_theme_selection_json_with(&tiny, input.as_bytes()).unwrap_err(),
                validate_theme_input_json_with(&tiny, options.as_bytes()).unwrap_err(),
            ] {
                assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
            }
            assert!(
                compile_theme_selection_json(input.as_bytes()).is_err(),
                "{input}"
            );
            assert!(
                crate::common::parse_options(options.as_bytes()).is_err(),
                "{input}"
            );
        }
    }

    #[test]
    fn paired_recipe_default_and_explicit_selections_share_the_compiler_route() {
        let compiler = DiagramThemeCompiler::new();
        let document: ThemeRecipeV1 = serde_json::from_slice(PAIRED_RECIPE).unwrap();
        let direct = compile_theme_selection_json(PAIRED_RECIPE).unwrap();
        for (selector, appearance) in [
            (None, None),
            (Some("light"), Some(ThemeAppearance::Light)),
            (Some("dark"), Some(ThemeAppearance::Dark)),
        ] {
            let mut selection = serde_json::json!({"recipe": paired_recipe()});
            if let Some(selector) = selector {
                selection["appearance"] = serde_json::json!(selector);
            }
            let actual =
                compile_theme_selection_json(&serde_json::to_vec(&selection).unwrap()).unwrap();
            let expected = compiler
                .compile_document(document.clone(), appearance)
                .unwrap();
            assert_eq!(actual.recipe_fingerprint(), expected.recipe_fingerprint());
            if appearance != Some(ThemeAppearance::Dark) {
                assert_eq!(actual.recipe_fingerprint(), direct.recipe_fingerprint());
            } else {
                assert_ne!(actual.recipe_fingerprint(), direct.recipe_fingerprint());
            }
        }
    }

    #[test]
    fn external_appearance_uses_the_same_raw_selection_admission() {
        let compiler = DiagramThemeCompiler::new();
        let dark = compile_theme_selection_json_with_appearance(
            &compiler,
            PAIRED_RECIPE,
            Some(ThemeAppearance::Dark),
        )
        .unwrap();
        assert_eq!(dark.selected_appearance(), Some(ThemeAppearance::Dark));
        for selection in [
            serde_json::json!({"recipe": paired_recipe()}),
            serde_json::json!({"recipe": paired_recipe(), "appearance": "dark"}),
        ] {
            let bytes = serde_json::to_vec(&selection).unwrap();
            let selected = compile_theme_selection_json_with_appearance(
                &compiler,
                &bytes,
                Some(ThemeAppearance::Dark),
            )
            .unwrap();
            assert_eq!(selected.recipe_fingerprint(), dark.recipe_fingerprint());
            assert_eq!(selected.selected_appearance(), Some(ThemeAppearance::Dark));
            if selection.get("appearance").is_some() {
                assert_eq!(
                    compile_theme_selection_json_with_appearance(&compiler, &bytes, None)
                        .unwrap()
                        .recipe_fingerprint(),
                    dark.recipe_fingerprint()
                );
                let error = compile_theme_selection_json_with_appearance(
                    &compiler,
                    &bytes,
                    Some(ThemeAppearance::Light),
                )
                .unwrap_err();
                assert_eq!(error.status(), BindingStatus::InvalidArgument);
                assert!(error.message().contains("conflicts"));
            }
        }
        let explicit =
            compile_theme_selection_json(br#"{"preset":"editor","appearance":"dark"}"#).unwrap();
        for selection in [
            br#"{"preset":"editor"}"#.as_slice(),
            br#"{"preset":"editor","appearance":"dark"}"#,
        ] {
            let selected = compile_theme_selection_json_with_appearance(
                &compiler,
                selection,
                Some(ThemeAppearance::Dark),
            )
            .unwrap();
            assert_eq!(selected.recipe_fingerprint(), explicit.recipe_fingerprint());
        }
        assert!(
            compile_theme_selection_json_with_appearance(
                &compiler,
                br#"{"preset":"editor","appearance":"light"}"#,
                Some(ThemeAppearance::Dark),
            )
            .unwrap_err()
            .message()
            .contains("conflicts")
        );
        for input in [
            br#"{"spec":{}}"#.as_slice(),
            br#"{"schema_version":1,"kind":"design","design":{}}"#,
        ] {
            let error = compile_theme_selection_json_with_appearance(
                &compiler,
                input,
                Some(ThemeAppearance::Dark),
            )
            .unwrap_err();
            assert_eq!(error.status(), BindingStatus::InvalidArgument);
        }
        let error = compile_theme_selection_json_with_appearance(
            &compiler,
            br#"{"preset":"editor","appearance":"light","appearance":"dark"}"#,
            Some(ThemeAppearance::Dark),
        )
        .unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    }

    #[test]
    fn appearance_requires_a_declared_recipe_appearance() {
        for recipe in [
            serde_json::json!({"schema_version": 1, "kind": "design", "design": {}}),
            serde_json::json!({"schema_version": 1, "kind": "complete_spec", "complete_spec": {}}),
        ] {
            compile_theme_selection_json(&serde_json::to_vec(&recipe).unwrap()).unwrap();
            for appearance in ["light", "dark"] {
                let selection = serde_json::json!({
                    "recipe": recipe, "appearance": appearance
                });
                let error = compile_theme_selection_json(&serde_json::to_vec(&selection).unwrap())
                    .unwrap_err();
                assert_eq!(error.status(), BindingStatus::InvalidArgument);
            }
        }
        let mut single = paired_recipe();
        single["design"]["appearances"]
            .as_object_mut()
            .unwrap()
            .remove("dark");
        compile_theme_selection_json(
            &serde_json::to_vec(&serde_json::json!({
                "recipe": single, "appearance": "light"
            }))
            .unwrap(),
        )
        .unwrap();
        let error = compile_theme_selection_json(
            &serde_json::to_vec(&serde_json::json!({
                "recipe": single, "appearance": "dark"
            }))
            .unwrap(),
        )
        .unwrap_err();
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
    }

    #[test]
    fn preset_selections_preserve_concrete_identity() {
        let fingerprint = |selection: Value| {
            compile_theme_selection_json(&serde_json::to_vec(&selection).unwrap())
                .unwrap()
                .recipe_fingerprint()
        };
        for (appearance, concrete) in [("light", "editor-light"), ("dark", "editor-dark")] {
            assert_eq!(
                fingerprint(serde_json::json!({"preset": "editor", "appearance": appearance})),
                fingerprint(serde_json::json!({"preset": concrete}))
            );
            assert_eq!(
                fingerprint(serde_json::json!({"preset": concrete, "appearance": appearance})),
                fingerprint(serde_json::json!({"preset": concrete}))
            );
        }
        let error =
            compile_theme_selection_json(br#"{"preset":"editor-dark","appearance":"light"}"#)
                .unwrap_err();
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
    }

    #[test]
    fn preset_selection_and_export_require_exact_catalog_ids() {
        let compiler = DiagramThemeCompiler::new();
        for id in [
            " editor",
            "editor ",
            " editor ",
            "\teditor\n",
            "\u{00a0}editor\u{00a0}",
            " editor-dark ",
        ] {
            let selection = serde_json::to_vec(&serde_json::json!({
                "preset": id, "appearance": "dark"
            }))
            .unwrap();
            for error in [
                compile_theme_selection_json_with(&compiler, &selection).unwrap_err(),
                crate::export_theme_preset_json_with(&compiler, id.as_bytes()).unwrap_err(),
            ] {
                assert_eq!(error.status(), BindingStatus::InvalidArgument, "{id:?}");
                assert_eq!(
                    error.theme_authoring_details().unwrap().diagnostic().path(),
                    "/preset",
                    "{id:?}"
                );
            }
        }
    }

    #[test]
    fn paired_theme_raw_admission_checks_wrappers_and_inactive_payloads() {
        for input in [
            r#"{"preset":"editor","preset":"editor","appearance":"dark"}"#,
            r#"{"preset":"editor","appearance":"light","appe\u0061rance":"dark"}"#,
            r#"{"recipe":{"schema_version":1,"kind":"design","design":{}},"rec\u0069pe":{"schema_version":1,"kind":"design","design":{}}}"#,
            r#"{"recipe":{"schema_version":1,"schema_\u0076ersion":1,"kind":"design","design":{}}}"#,
            r#"{"recipe":{"schema_version":1,"kind":"design","design":{"appearances":{"light":{},"dark":{},"d\u0061rk":{}},"default_appearance":"light"}}}"#,
            r##"{"recipe":{"schema_version":1,"kind":"design","design":{"appearances":{"light":{},"dark":{"tokens":{"ink":{"type":"color","value":"#123456"},"i\u006ek":{"type":"color","value":"#654321"}}}},"default_appearance":"light"}},"appearance":"light"}"##,
            r#"{"recipe":{"schema_version":1.0,"kind":"design","design":{}}}"#,
            r#"{"recipe":{"schema_version":1e0,"kind":"design","design":{}}}"#,
            r#"{"recipe":{"schema_version":"1","kind":"design","design":{}}}"#,
            r#"{"recipe":{"schema_version":2,"kind":"design","design":{}}}"#,
            r#"{"recipe":{"schema_version":3,"kind":"design","design":{}}}"#,
            r#"{"recipe":{"preset":"editor"}}"#,
        ] {
            let options = format!(r#"{{"theme":{input}}}"#);
            for error in [
                compile_theme_selection_json(input.as_bytes()).unwrap_err(),
                crate::common::parse_options(options.as_bytes()).unwrap_err(),
            ] {
                assert_eq!(error.status(), BindingStatus::OptionsJsonError, "{input}");
            }
        }
        let mut inactive_invalid = paired_recipe();
        inactive_invalid["design"]["appearances"]["dark"]["styles"] =
            serde_json::json!({"node": {"opacity": 2}});
        let selection = serde_json::json!({
            "recipe": inactive_invalid, "appearance": "light"
        });
        let error =
            compile_theme_selection_json(&serde_json::to_vec(&selection).unwrap()).unwrap_err();
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
    }

    #[test]
    fn paired_recipe_uses_whole_selection_replacement_in_reusable_engines() {
        let source = b"flowchart LR\nA-->B\n";
        let base = serde_json::json!({
            "svg": {"diagram_id": "paired-selection"},
            "theme": {"recipe": paired_recipe(), "appearance": "light"}
        });
        let engine = crate::BindingEngine::new(&serde_json::to_vec(&base).unwrap()).unwrap();
        let light = engine.render_svg(source).unwrap();
        for appearance in ["dark", "light", "dark"] {
            let request_options = serde_json::json!({
                "theme": {"recipe": paired_recipe(), "appearance": appearance}
            });
            let request_options = serde_json::to_vec(&request_options).unwrap();
            let actual = engine
                .execute_data(
                    crate::BindingOperationRequest::new("svg", source)
                        .with_options_json(&request_options),
                )
                .unwrap();
            let mut standalone = base.clone();
            standalone["theme"]["appearance"] = serde_json::json!(appearance);
            let expected =
                crate::render_svg(source, &serde_json::to_vec(&standalone).unwrap()).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(engine.render_svg(source).unwrap(), light);
        }
        for request in [
            br#"{"theme":{"appearance":"dark"}}"#.as_slice(),
            br#"{"theme":{"appearance":null}}"#,
        ] {
            let error = engine
                .execute_data(
                    crate::BindingOperationRequest::new("svg", source).with_options_json(request),
                )
                .unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        }
        let cleared = engine
            .execute_data(
                crate::BindingOperationRequest::new("svg", source)
                    .with_options_json(br#"{"theme":null}"#),
            )
            .unwrap();
        assert_eq!(
            cleared,
            crate::render_svg(source, br#"{"svg":{"diagram_id":"paired-selection"}}"#).unwrap()
        );
        assert_eq!(engine.render_svg(source).unwrap(), light);
    }

    #[test]
    fn complete_spec_raw_admission_and_recipe_envelope_keep_distinct_error_positions() {
        const INVALID_PAYLOAD: &str =
            r#"{"schema_version":1,"kind":"complete_spec","complete_spec":{"future_field":true}}"#;
        const INVALID_ENVELOPE: &str =
            r#"{"schema_version":1,"kind":"future_recipe","complete_spec":{}}"#;
        for (theme, suffix, expected) in [
            (
                INVALID_PAYLOAD,
                r#", "version":2"#,
                "invalid complete theme spec:",
            ),
            (
                INVALID_PAYLOAD,
                r#", "layout":{"text_measurer":"removed"}"#,
                "invalid complete theme spec:",
            ),
            (
                INVALID_PAYLOAD,
                r#", "site_config":{"themeCSS":".node {}"}"#,
                "invalid complete theme spec:",
            ),
            (
                INVALID_ENVELOPE,
                r#", "version":2"#,
                "unsupported options_json schema version",
            ),
            (
                INVALID_ENVELOPE,
                r#", "layout":{"text_measurer":"removed"}"#,
                "layout.text_measurer was removed",
            ),
            (
                INVALID_ENVELOPE,
                r#", "site_config":{"themeCSS":".node {}"}"#,
                "site_config.themeCSS",
            ),
        ] {
            let input = format!(r#"{{"theme":{theme}{suffix}}}"#);
            let error = crate::common::parse_options(input.as_bytes()).unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
            assert!(error.message().contains(expected), "{input}: {error:?}");
        }
    }

    #[test]
    fn saved_recipe_renders_identically_in_one_shot_and_reusable_engines() {
        let recipe = crate::export_theme_preset_json(b"editor-dark").unwrap();
        let restored: Value = serde_json::from_slice(&recipe).unwrap();
        let options = serde_json::to_vec(&serde_json::json!({
            "svg": { "diagram_id": "shared-recipe" }, "theme": restored,
        }))
        .unwrap();
        let source = b"flowchart LR\nA-->B\n";
        let expected = crate::render_svg(
            source,
            br#"{"svg":{"diagram_id":"shared-recipe"},"theme":{"preset":"editor-dark"}}"#,
        )
        .unwrap();
        assert_eq!(crate::render_svg(source, &options).unwrap(), expected);
        let engine = crate::BindingEngine::new(&options).unwrap();
        assert_eq!(engine.render_svg(source).unwrap(), expected);
    }

    #[test]
    fn preset_and_structured_spec_compile_through_the_renderer_wire_authority() {
        let preset: BindingThemeOptionsJson = decode_theme_value(
            serde_json::json!({"preset": "editor-dark"}),
            "theme selection JSON",
        )
        .unwrap();
        let preset = compile_theme_with(&DiagramThemeCompiler::new(), Some(&preset))
            .unwrap()
            .unwrap();
        assert!(
            preset
                .report()
                .requires_capability(ThemeCapability::SemanticRules)
        );

        let spec = decode_theme_value(
            serde_json::json!({
                "spec": {
                    "typography": {
                        "default": {
                            "font_stack": ["Inter", "sans-serif"],
                            "font_size_px": 18.0,
                            "line_height": 1.4
                        }
                    },
                    "styles": [{
                        "kind": "rule",
                        "target": "node",
                        "family": "flowchart",
                        "style": {
                            "fill": "#101820",
                            "stroke": { "paint": "#f2aa4c", "width": 2.0 },
                            "radius": 6.0
                        }
                    }],
                    "canvas": { "base": "#0b0f14", "bleed": 8.0 },
                    "requirements": { "capabilities": ["rounded-geometry"] }
                }
            }),
            "theme selection JSON",
        )
        .unwrap();
        let theme = compile_theme_with(&DiagramThemeCompiler::new(), Some(&spec))
            .unwrap()
            .unwrap();

        assert!(
            theme
                .report()
                .requires_capability(ThemeCapability::Typography)
        );
        assert!(
            theme
                .report()
                .requires_capability(ThemeCapability::RoundedGeometry)
        );
        assert!(
            theme
                .report()
                .requires_capability(ThemeCapability::SemanticRules)
        );
    }

    #[test]
    fn renderer_wire_errors_are_mapped_to_binding_status() {
        let selection = decode_theme_value(
            serde_json::json!({
                "spec": {
                    "styles": [{
                        "kind": "rule",
                        "target": "future-node",
                        "style": {}
                    }]
                }
            }),
            "theme selection JSON",
        )
        .unwrap();

        let error = compile_theme_with(&DiagramThemeCompiler::new(), Some(&selection)).unwrap_err();
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("styles.rule.target"));
    }

    #[test]
    fn raw_theme_input_limit_precedes_typed_json_allocation() {
        let policy = ThemeResourcePolicy::default();
        let max = policy
            .value(ThemeResourceLimitId::MaxThemeEncodedBytes)
            .expect("encoded-theme ceiling");
        let padding = " ".repeat(max);
        let options = format!(r#"{{"theme":{{{padding}"preset":"editor-light"}}}}"#);

        let error = validate_theme_input_json(options.as_bytes()).unwrap_err();
        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
        let details = error
            .resource_details()
            .expect("structured theme limit details");
        assert_eq!(details.limit_id, "max_theme_encoded_bytes");
        assert_eq!(details.phase, "theme_input");
        assert_eq!(details.max, u64::try_from(max).unwrap());
        assert!(details.actual > details.max);
        assert_eq!(details.profile, "interactive");
    }

    #[test]
    fn nested_contract_shape_errors_remain_transport_errors() {
        for value in [
            serde_json::json!({
                "spec": {
                    "styles": [{
                        "kind": "rule",
                        "target": "node",
                        "style": {},
                        "extra": true
                    }]
                }
            }),
            serde_json::json!({
                "spec": {
                    "styles": [{
                        "kind": "rule",
                        "target": null,
                        "style": {}
                    }]
                }
            }),
        ] {
            let error = serde_json::to_vec(&value)
                .ok()
                .and_then(|bytes| compile_theme_selection_json(&bytes).err())
                .expect("invalid contract shape must fail");
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        }
    }
}
