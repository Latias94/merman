use merman::svg::{DiagramTheme, DiagramThemeCompiler, ThemeResourcePolicy};
use merman_theme_contract::{
    DiagramThemeSpecWireV1, THEME_RECIPE_SCHEMA_VERSION_V1, ThemeRecipeV1,
};
use serde::{Deserialize, Deserializer, de::Error as _};
use serde_json::{Value, value::RawValue};

use crate::common::{BindingError, BindingStatus};

#[derive(Debug)]
pub(crate) enum BindingThemeOptionsJson {
    Selection(ThemeSelectionJson),
    Recipe(ThemeRecipeV1),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ThemeSelectionJson {
    preset: Option<String>,
    spec: Option<DiagramThemeSpecWireV1>,
}

/// Decodes one theme value after its tagged union has been validated.
fn decode_validated_theme_value(
    value: Value,
) -> Result<BindingThemeOptionsJson, serde_json::Error> {
    if value.get("kind").is_some() || value.get("schema_version").is_some() {
        serde_json::from_value(value).map(BindingThemeOptionsJson::Recipe)
    } else {
        serde_json::from_value(value).map(BindingThemeOptionsJson::Selection)
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
            merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE,
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
    design: Option<&'a RawValue>,
    #[serde(default, borrow, deserialize_with = "present_raw_value")]
    complete_spec: Option<&'a RawValue>,
    #[serde(default, borrow, deserialize_with = "present_raw_value")]
    spec: Option<&'a RawValue>,
}

fn present_raw_value<'de, D>(deserializer: D) -> Result<Option<&'de RawValue>, D::Error>
where
    D: Deserializer<'de>,
{
    <&RawValue>::deserialize(deserializer).map(Some)
}

fn check_theme_json_input(
    compiler: &DiagramThemeCompiler,
    bytes: &[u8],
) -> Result<(), BindingError> {
    let probe: ThemeInputProbe<'_> = serde_json::from_slice(bytes)
        .map_err(|error| invalid_options(format!("invalid theme JSON: {error}")))?;
    let spec = if probe.kind.is_none() && probe.schema_version.is_none() {
        // Preserve the selection owner's null-payload diagnostic and error position.
        probe.spec.filter(|raw| raw.get() != "null")
    } else {
        let version = probe
            .schema_version
            .and_then(|raw| serde_json::from_str::<u64>(raw.get()).ok());
        check_recipe_version(version)?;
        if let Some(design) = probe.design {
            compiler
                .check_design_json_input(design.get().as_bytes())
                .map_err(|error| {
                    crate::theme_design::materialization_error(compiler.resource_policy(), error)
                })?;
        }
        probe.complete_spec
    };
    if let Some(spec) = spec {
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
            merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE,
        ));
    compile_theme_selection_json_with(&compiler, bytes)
}

/// Compiles one exact theme selection with a caller-owned compiler policy.
pub fn compile_theme_selection_json_with(
    compiler: &DiagramThemeCompiler,
    bytes: &[u8],
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
    compile_theme_with(compiler, Some(&selection))?.ok_or_else(|| {
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
    let has_preset = object.contains_key("preset");
    let has_spec = object.contains_key("spec");
    if object.contains_key("kind") || object.contains_key("schema_version") {
        if has_preset || has_spec {
            return Err(invalid_options(
                "theme recipe must not also contain `preset` or `spec`",
            ));
        }
        check_recipe_version(object.get("schema_version").and_then(Value::as_u64))?;
        return Ok(());
    }
    match (has_preset, has_spec) {
        (true, false) | (false, true) => {}
        (false, false) => {
            return Err(invalid_options(
                "options field `theme` must contain exactly one of `preset` or `spec`",
            ));
        }
        (true, true) => {
            return Err(invalid_options(
                "options field `theme` must not contain both `preset` and `spec`",
            ));
        }
    }
    if object
        .get(if has_preset { "preset" } else { "spec" })
        .is_some_and(Value::is_null)
    {
        return Err(invalid_options(
            "options field `theme.preset` or `theme.spec` must not be null",
        ));
    }

    Ok(())
}

pub(crate) fn compile_theme_with(
    compiler: &DiagramThemeCompiler,
    selection: Option<&BindingThemeOptionsJson>,
) -> Result<Option<DiagramTheme>, BindingError> {
    let Some(selection) = selection else {
        return Ok(None);
    };
    let selection = match selection {
        BindingThemeOptionsJson::Recipe(recipe) => {
            return compiler
                .compile_recipe(recipe.clone())
                .map(Some)
                .map_err(|error| {
                    crate::theme_design::design_compile_error(compiler.resource_policy(), error)
                });
        }
        BindingThemeOptionsJson::Selection(selection) => selection,
    };
    let theme = match (&selection.preset, &selection.spec) {
        (Some(id), None) => {
            let preset = merman::svg::ThemePreset::from_id(id.trim()).map_err(|_| {
                invalid_theme("theme.preset", format!("unknown theme preset `{id}`"))
            })?;
            compiler.compile_preset(preset).map_err(|error| {
                crate::theme_design::design_compile_error(compiler.resource_policy(), error)
            })?
        }
        (None, Some(spec)) => compiler
            .compile_spec_wire(spec.clone())
            .map_err(theme_compile_error)?,
        _ => {
            return Err(invalid_options(
                "options field `theme` must contain exactly one of `preset` or `spec`",
            ));
        }
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
        .map(merman::resources::ResourceProfile::id)
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

    #[test]
    fn theme_wire_requires_exactly_one_selection() {
        for invalid in [
            serde_json::json!({}),
            serde_json::json!({"preset": "editor-dark", "spec": {}}),
            serde_json::json!({"preset": null}),
            serde_json::json!({"spec": null}),
            serde_json::json!({"schema_version": 1, "kind": "complete_spec", "complete_spec": {}, "preset": "editor-dark"}),
            serde_json::json!({"schema_version": 1, "kind": "complete_spec", "complete_spec": {}, "spec": {}}),
        ] {
            let error = validate_theme_wire(Some(&invalid)).unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        }

        validate_theme_wire(Some(&Value::Null)).unwrap();
        validate_theme_wire(Some(&serde_json::json!({"preset": "editor-light"}))).unwrap();
        validate_theme_wire(Some(&serde_json::json!({"spec": {}}))).unwrap();
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
            "/styles/node/fill/color"
        );
    }

    #[test]
    fn recipe_version_errors_precede_future_payload_shape_errors() {
        let bytes =
            br#"{"kind":"complete_spec","complete_spec":{"future_field":true},"schema_version":2}"#;
        let error = compile_theme_selection_json(bytes).unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(
            error.message().contains("schema_version"),
            "{}",
            error.message()
        );
        assert!(error.message().contains('2'), "{}", error.message());
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
        for version in ["null", "0", "2", "1.0", "1e0", "\"1\"", "true"] {
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
