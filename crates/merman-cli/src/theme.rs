use crate::error::CliError;
use crate::input::InputLimit;
use crate::invocation::ResolvedThemeInspect;
use crate::runtime::ExecutionContext;
use merman::svg::{MAX_THEME_ENCODED_BYTES_HARD_CAP, ThemeResourceLimitId, ThemeResourcePolicy};

pub(crate) fn inspect(
    args: ResolvedThemeInspect,
    control: &merman::OperationControl,
    context: &mut ExecutionContext,
) -> Result<(), CliError> {
    let policy = ThemeResourcePolicy::for_profile(args.resources.profile());
    let max_bytes = policy
        .value(ThemeResourceLimitId::MaxThemeEncodedBytes)
        .unwrap_or(MAX_THEME_ENCODED_BYTES_HARD_CAP);
    let limit = InputLimit::new("max_theme_encoded_bytes", Some(max_bytes));
    let recipe = if let Some(path) = args.input.file() {
        crate::io::read_named_bytes_file_controlled(path, "theme recipe file", limit, control)?
    } else {
        crate::input::read_bytes_with_limit_controlled(
            context.stdin.as_mut(),
            "stdin",
            limit,
            None,
            control,
        )
        .map_err(|error| match error {
            crate::input::InputReadError::Cancelled(cancelled) => {
                CliError::Render(merman::RenderError::Cancelled(cancelled))
            }
            error => CliError::primary_input(error),
        })?
    };
    crate::operation::checkpoint(control, merman::OperationPhase::Admission)?;
    let mut request = br#"{"schema_version":1,"action":"inspect","recipe":"#.to_vec();
    let request_len = request
        .len()
        .checked_add(recipe.len())
        .and_then(|length| length.checked_add(1))
        .ok_or_else(|| CliError::InvalidInput("theme recipe input size overflow".to_string()))?;
    policy
        .check_theme_encoded_bytes(request_len)
        .map_err(|error| CliError::InvalidInput(format!("invalid theme recipe: {error}")))?;
    request
        .try_reserve_exact(recipe.len() + 1)
        .map_err(|error| {
            CliError::Io(std::io::Error::other(format!(
                "failed to allocate theme inspection request: {error}"
            )))
        })?;
    request.extend_from_slice(&recipe);
    request.push(b'}');
    let mut result = merman_bindings_core::materialize_theme_design_json_with_resource_policy(
        &request, &policy,
    )
    .map_err(|error| {
        CliError::InvalidInput(format!(
            "invalid portable theme recipe (expected schema 1; render-selection wrappers belong to render --theme-file): {}",
            error.message()
        ))
    })?;
    crate::operation::checkpoint(control, merman::OperationPhase::Emit)?;
    result.push(b'\n');
    crate::io::write_stdout(&result, &context.stdout)
}
