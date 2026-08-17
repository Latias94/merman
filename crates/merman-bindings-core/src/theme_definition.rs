use merman::diagram_theme::{
    ThemeDefinitionAdmissionError, ThemeDefinitionCompileError,
    compile_theme_definition_json as compile_theme_definition_json_with_renderer,
};
use merman::svg::{DiagramTheme, DiagramThemeCompiler, ThemeResourcePolicy};

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
    compile_theme_definition_json_with_renderer(compiler, bytes).map_err(definition_compile_error)
}

fn definition_compile_error(error: ThemeDefinitionCompileError) -> BindingError {
    match error {
        ThemeDefinitionCompileError::Admission(ThemeDefinitionAdmissionError::ResourceLimit(
            error,
        )) => crate::theme::theme_resource_error(error),
        ThemeDefinitionCompileError::Admission(ThemeDefinitionAdmissionError::InvalidJson {
            ..
        }) => BindingError::invalid_options_json(error.to_string()),
        ThemeDefinitionCompileError::Admission(
            error @ ThemeDefinitionAdmissionError::CollectionLimit { .. },
        ) => BindingError::invalid_argument(error.to_string()),
        ThemeDefinitionCompileError::Materialization(error) => {
            BindingError::invalid_argument(format!("invalid theme definition: {error}"))
        }
        ThemeDefinitionCompileError::Compilation(error) => {
            crate::theme::theme_definition_compile_error(error)
        }
        error => BindingError::invalid_argument(format!("invalid theme definition: {error}")),
    }
}
