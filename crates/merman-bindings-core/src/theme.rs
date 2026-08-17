use merman::svg::{DiagramTheme, DiagramThemeCompiler, ThemeResourcePolicy};
use merman_theme_contract::DiagramThemeSpecWireV1;
use serde::Deserialize;
use serde_json::{Value, value::RawValue};

use crate::common::{BindingError, BindingStatus};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BindingThemeOptionsJson {
    preset: Option<String>,
    spec: Option<DiagramThemeSpecWireV1>,
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
        .map_err(theme_resource_error)
}

/// Compiles one exact `{"preset": ...}` or `{"spec": ...}` selection with default host policy.
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
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid theme selection JSON: {error}"),
        )
    })?;
    validate_theme_wire(Some(&value))?;
    let selection = serde_json::from_value::<BindingThemeOptionsJson>(value).map_err(|error| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid theme selection JSON: {error}"),
        )
    })?;
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
    let theme = match (&selection.preset, &selection.spec) {
        (Some(id), None) => {
            let preset = merman::svg::ThemePreset::from_id(id.trim()).map_err(|_| {
                invalid_theme("theme.preset", format!("unknown theme preset `{id}`"))
            })?;
            compiler
                .compile_preset(preset)
                .map_err(theme_compile_error)?
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
        merman::svg::ThemeCompileError::ResourceLimit(error)
        | merman::svg::ThemeCompileError::FontCatalog(
            merman::svg::FontCatalogError::ResourceLimit(error),
        ) => theme_resource_error(error),
        merman::svg::ThemeCompileError::Validation(error) => theme_validation_error(error),
        error => invalid_theme("theme", error.to_string()),
    }
}

pub(crate) fn theme_definition_compile_error(
    error: merman::svg::ThemeCompileError,
) -> BindingError {
    match error {
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
                _ => "definition",
            };
            invalid_theme(format!("theme definition {field}"), error.to_string())
        }
        error => invalid_theme("theme definition", error.to_string()),
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
    use merman::diagram_theme::{
        ThemeAuthoringTypographyV1, ThemeColorTokenV1, ThemeDefinitionV1, ThemeMaterializer,
        ThemeTokensV1, compile_theme_definition,
    };
    use merman::svg::{ThemeCapability, ThemeResourceLimitId};

    #[test]
    fn theme_wire_requires_exactly_one_selection() {
        for invalid in [
            serde_json::json!({}),
            serde_json::json!({"preset": "editor-dark", "spec": {}}),
            serde_json::json!({"preset": null}),
            serde_json::json!({"spec": null}),
        ] {
            let error = validate_theme_wire(Some(&invalid)).unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        }

        validate_theme_wire(Some(&Value::Null)).unwrap();
        validate_theme_wire(Some(&serde_json::json!({"preset": "editor-light"}))).unwrap();
        validate_theme_wire(Some(&serde_json::json!({"spec": {}}))).unwrap();
    }

    #[test]
    fn preset_and_structured_spec_compile_through_the_renderer_wire_authority() {
        let preset: BindingThemeOptionsJson =
            serde_json::from_value(serde_json::json!({"preset": "editor-dark"})).unwrap();
        let preset = compile_theme_with(&DiagramThemeCompiler::new(), Some(&preset))
            .unwrap()
            .unwrap();
        assert!(
            preset
                .report()
                .requires_capability(ThemeCapability::SemanticRules)
        );

        let spec: BindingThemeOptionsJson = serde_json::from_value(serde_json::json!({
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
        }))
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
    fn authoring_definition_json_matches_the_typed_rust_definition() {
        const DEFINITION_JSON: &[u8] = br##"{
                "authoring_schema_version": 1,
                "expansion_version": 1,
                "tokens": {
                    "canvas": "#0f172a",
                    "surface": "#111827",
                    "surface_alt": "#1f2937",
                    "surface_muted": "#334155",
                    "text": "#e5e7eb",
                    "border": "#475569",
                    "line": "#94a3b8",
                    "accent": "#60a5fa",
                    "series": ["#60a5fa", "#34d399", "#f59e0b"],
                    "typography": {
                        "font_stack": ["Inter", "system-ui", "sans-serif"],
                        "font_size_px": 15,
                        "font_weight": 500
                    }
                }
            }"##;
        let typed_definition = ThemeDefinitionV1::new(
            ThemeTokensV1::default()
                .with_color(ThemeColorTokenV1::Canvas, "#0f172a")
                .with_color(ThemeColorTokenV1::Surface, "#111827")
                .with_color(ThemeColorTokenV1::SurfaceAlt, "#1f2937")
                .with_color(ThemeColorTokenV1::SurfaceMuted, "#334155")
                .with_color(ThemeColorTokenV1::Text, "#e5e7eb")
                .with_color(ThemeColorTokenV1::Border, "#475569")
                .with_color(ThemeColorTokenV1::Line, "#94a3b8")
                .with_color(ThemeColorTokenV1::Accent, "#60a5fa")
                .with_series(vec![
                    "#60a5fa".to_owned(),
                    "#34d399".to_owned(),
                    "#f59e0b".to_owned(),
                ])
                .with_typography(
                    ThemeAuthoringTypographyV1::default()
                        .with_font_stack(vec![
                            "Inter".to_owned(),
                            "system-ui".to_owned(),
                            "sans-serif".to_owned(),
                        ])
                        .with_font_size_px(15.0)
                        .with_font_weight(500),
                ),
        );
        let decoded_definition: ThemeDefinitionV1 = serde_json::from_slice(DEFINITION_JSON)
            .expect("the readable binding definition should decode through the shared contract");

        assert_eq!(
            decoded_definition
                .canonical_json_bytes()
                .expect("the JSON definition should canonicalize"),
            typed_definition
                .canonical_json_bytes()
                .expect("the typed definition should canonicalize"),
        );

        let materializer = ThemeMaterializer::new();
        let decoded_materialized = materializer
            .materialize_theme(&decoded_definition)
            .expect("the JSON definition should materialize");
        let typed_materialized = materializer
            .materialize_theme(&typed_definition)
            .expect("the typed definition should materialize");
        assert_eq!(decoded_materialized.spec(), typed_materialized.spec());

        let binding_theme = crate::compile_theme_definition_json(DEFINITION_JSON)
            .expect("the binding definition should materialize and compile");
        let typed_theme = compile_theme_definition(&DiagramThemeCompiler::new(), &typed_definition)
            .expect("the typed definition should materialize and compile");
        assert_eq!(
            binding_theme.recipe_fingerprint(),
            typed_theme.recipe_fingerprint(),
        );

        assert!(
            binding_theme
                .report()
                .requires_capability(ThemeCapability::SemanticRules)
        );
        assert!(
            binding_theme
                .report()
                .requires_capability(ThemeCapability::SolidPaint)
        );
    }

    #[test]
    fn authoring_definition_preserves_binding_error_classification() {
        let materialization_error = crate::compile_theme_definition_json(
            br#"{
                "authoring_schema_version": 1,
                "expansion_version": 1,
                "tokens": { "series": [] }
            }"#,
        )
        .expect_err("an empty authored series should fail during materialization");
        assert_eq!(
            materialization_error.status(),
            BindingStatus::InvalidArgument
        );
        assert!(
            materialization_error
                .message()
                .contains("invalid theme definition")
        );
        assert!(materialization_error.message().contains("series"));

        let compilation_error = crate::compile_theme_definition_json(
            br#"{
                "authoring_schema_version": 1,
                "expansion_version": 1,
                "tokens": {},
                "styles": [{
                    "kind": "rule",
                    "target": "future-node",
                    "style": {}
                }]
            }"#,
        )
        .expect_err("an unknown semantic target should fail during compilation");
        assert_eq!(compilation_error.status(), BindingStatus::InvalidArgument);
        assert!(
            compilation_error
                .message()
                .contains("theme definition styles.rule.target")
        );
        assert!(!compilation_error.message().contains("theme.spec"));
    }

    #[test]
    fn authoring_resource_errors_only_project_discoverable_theme_limits() {
        let policy = ThemeResourcePolicy::default()
            .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, 1)
            .expect("one byte is a valid encoded-theme ceiling");
        let compiler = DiagramThemeCompiler::new().with_resource_policy(policy);
        let error = crate::compile_theme_definition_json_with(&compiler, br#"{}"#)
            .expect_err("the caller-owned encoded byte limit must fail before decoding");
        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
        let details = error
            .resource_details()
            .expect("catalogued theme limit details");
        assert_eq!(details.limit_id, "max_theme_encoded_bytes");
        assert_eq!(details.phase, "theme_input");
        assert_eq!(details.actual, 2);
        assert_eq!(details.max, 1);
        assert_eq!(details.profile, "interactive");

        let colors = std::iter::repeat_n(r##""#123456""##, 257)
            .collect::<Vec<_>>()
            .join(",");
        let json = format!(
            r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{"series":[{colors}]}}}}"#
        );
        let error = crate::compile_theme_definition_json(json.as_bytes())
            .expect_err("an authoring-only palette ceiling must remain an authoring diagnostic");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert_eq!(error.resource_details(), None);
        assert!(error.message().contains("invalid theme definition"));
    }

    #[test]
    fn authoring_definition_rejects_duplicate_members_before_typed_projection() {
        let duplicate = br##"{
            "authoring_schema_version": 1,
            "expansion_version": 1,
            "tokens": {
                "canvas": "#ffffff",
                "canv\u0061s": "#000000"
            }
        }"##;
        let error = crate::compile_theme_definition_json(duplicate)
            .expect_err("decoded-equivalent duplicate keys must fail before typed projection");
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("duplicate object key"));
    }

    #[test]
    fn renderer_wire_errors_are_mapped_to_binding_status() {
        let selection: BindingThemeOptionsJson = serde_json::from_value(serde_json::json!({
            "spec": {
                "styles": [{
                    "kind": "rule",
                    "target": "future-node",
                    "style": {}
                }]
            }
        }))
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
