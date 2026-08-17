use merman::diagram_theme::{
    ThemeDefinitionCompileError, ThemeMaterializationErrorV1,
    compile_theme_definition_json as compile_theme_definition_json_with_renderer,
};
use merman::svg::{DiagramTheme, DiagramThemeCompiler, ThemeResourceLimitId, ThemeResourcePolicy};

use crate::common::BindingError;

/// Compiles one versioned authoring definition with the general binding resource profile.
pub fn compile_theme_definition_json(bytes: &[u8]) -> Result<DiagramTheme, BindingError> {
    let compiler =
        DiagramThemeCompiler::new().with_resource_policy(ThemeResourcePolicy::for_profile(
            merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE,
        ));
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

fn definition_compile_error(
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
    if diagnostic.code() == "theme-authoring.invalid-definition-json" {
        return BindingError::invalid_options_json(error.to_string());
    }
    if diagnostic.code() == "theme-authoring.resource-limit-exceeded"
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
        return BindingError::resource_limit(
            descriptor.phase.as_str(),
            descriptor.stable_id,
            actual,
            max,
            profile,
            error.to_string(),
        );
    }
    BindingError::invalid_argument(format!("invalid theme definition: {error}"))
}
