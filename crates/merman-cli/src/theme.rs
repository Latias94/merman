use crate::cli::NativeThemeCliArgs;
use crate::error::CliError;
use crate::input::{InputLimit, InputReadError};
use crate::invocation::ResolvedInput;
use merman::theme::{BuiltinPreset, ThemeCompiler, ThemeError, ThemeSelection};
use merman::{OperationControl, OperationPhase};
use std::io::Read;
use std::path::{Path, PathBuf};

#[cfg(feature = "svg")]
use crate::invocation::{ResolvedDestination, ResolvedThemeCheck};
#[cfg(feature = "svg")]
use crate::runtime::{ExecutionContext, SharedWriter};
#[cfg(feature = "svg")]
use merman::theme::{
    Category, Color, Paint, ResolvedThemeV2, ThemeNotice, ThemeOutcome, ThemeOutcomeNotice,
};
#[cfg(feature = "svg")]
use serde::Serialize;
#[cfg(feature = "svg")]
use std::fmt::Write as _;

#[derive(Debug, Clone)]
pub(crate) enum ThemeInput {
    Preset(BuiltinPreset),
    Mermaid(&'static str),
    Document(ResolvedInput),
}

impl ThemeInput {
    pub(crate) fn parse(value: PathBuf) -> Result<Self, String> {
        if value == Path::new("-") {
            return Ok(Self::Document(ResolvedInput::Stdin));
        }
        if value
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            return Ok(Self::Document(ResolvedInput::File(value)));
        }
        if let Some(id) = value.to_str() {
            if let Some(preset) = BuiltinPreset::from_id(id) {
                return Ok(Self::Preset(preset));
            }
            if let Some(&theme) = merman::supported_themes()
                .iter()
                .find(|&&theme| theme == id)
            {
                return Ok(Self::Mermaid(theme));
            }
        }
        let presets = BuiltinPreset::ALL
            .iter()
            .map(|preset| preset.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        Err(format!(
            "expected a .json file, '-' for stdin, a Merman preset ({presets}), or a Mermaid theme ({})",
            merman::supported_themes().join(", ")
        ))
    }
}

#[cfg(feature = "svg")]
pub(crate) fn parse_background(value: &str) -> Result<Color, String> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("transparent") {
        return Ok(Color::TRANSPARENT);
    }
    if let Ok(color) = Color::parse(value) {
        return Ok(color);
    }
    let svgtypes::Color {
        red,
        green,
        blue,
        alpha,
    } = value
        .parse()
        .map_err(|_| "expected a CSS color or 'transparent'".to_owned())?;
    Color::parse(&format!("#{red:02X}{green:02X}{blue:02X}{alpha:02X}"))
        .map_err(|error| error.to_string())
}

pub(crate) fn prepare(
    args: &NativeThemeCliArgs,
    stdin: &mut dyn Read,
    control: &OperationControl,
) -> Result<Option<ThemeSelection>, CliError> {
    let theme = match &args.theme {
        Some(ThemeInput::Preset(preset)) => ThemeSelection::preset(*preset, args.appearance),
        Some(ThemeInput::Document(input)) => {
            let compiler = ThemeCompiler::new();
            let bytes = read_document(input.file(), stdin, &compiler, control)?;
            let resolved = compiler
                .compile_json(&bytes, args.appearance)
                .map_err(|error| {
                    CliError::InvalidInput(format!(
                        "invalid theme document {}",
                        format_error(&error)
                    ))
                })?;
            ThemeSelection::compiled(resolved)
        }
        Some(ThemeInput::Mermaid(_)) | None => return Ok(None),
    };
    crate::operation::checkpoint(control, OperationPhase::Admission)?;
    Ok(Some(theme.with_precedence(args.theme_precedence)))
}

fn read_document(
    path: Option<&Path>,
    stdin: &mut dyn Read,
    compiler: &ThemeCompiler,
    control: &OperationControl,
) -> Result<Vec<u8>, CliError> {
    let limit = InputLimit::new("max_theme_encoded_bytes", Some(compiler.max_bytes()));
    if let Some(path) = path {
        crate::io::read_named_bytes_file_controlled(path, "theme document", limit, control)
    } else {
        crate::input::read_bytes_with_limit_controlled(stdin, "theme stdin", limit, None, control)
            .map_err(|error| match error {
                InputReadError::Cancelled(cancelled) => {
                    CliError::Render(merman::RenderError::Cancelled(cancelled))
                }
                error => CliError::auxiliary_input(error),
            })
    }
}

fn format_error(error: &ThemeError) -> String {
    let message = match error.path() {
        Some(path) => format!("at {path}: {error}"),
        None => error.to_string(),
    };
    merman::normalize_terminal_diagnostic(&message)
}

#[cfg(feature = "svg")]
pub(crate) fn list(json: bool, color: bool, stdout: &SharedWriter) -> Result<(), CliError> {
    let catalog =
        merman::theme::catalog().map_err(|error| CliError::Io(std::io::Error::other(error)))?;
    if json {
        return crate::diagnostics::write_json_stdout(catalog, true, stdout);
    }

    let mut output = String::new();
    for preset in &catalog.presets {
        let category = match preset.category {
            Category::Reading => "reading",
            Category::Editor => "editor",
            Category::Art => "art",
            Category::Custom => "custom",
        };
        let appearances = preset
            .appearances
            .iter()
            .map(|appearance| appearance.as_str())
            .collect::<Vec<_>>()
            .join("/");
        let _ = write!(
            output,
            "{:<12} {} · {} · {} (default {})",
            preset.id.as_str(),
            preset.name,
            category,
            appearances,
            preset.default_appearance
        );
        for swatch in preset.swatches {
            if color {
                let [r, g, b, _] = swatch.channels();
                let _ = write!(output, " \x1b[48;2;{r};{g};{b}m  \x1b[0m");
            } else {
                let _ = write!(output, " {swatch}");
            }
        }
        let _ = writeln!(output, "\n             {}", preset.description);
    }
    let _ = writeln!(
        output,
        "\nMermaid themes: {}",
        catalog.mermaid_themes.join(", ")
    );
    crate::io::write_stdout(output.as_bytes(), stdout)
}

#[cfg(feature = "svg")]
pub(crate) fn export(
    preset: BuiltinPreset,
    minimal: bool,
    output: ResolvedDestination,
    publications: &crate::output::PublicationGuards,
    control: &OperationControl,
    context: &mut ExecutionContext,
) -> Result<(), CliError> {
    let document = if minimal {
        serde_json::to_string_pretty(&serde_json::json!({
            "$schema": "https://merman.dev/schemas/theme-v2.schema.json",
            "schema_version": 2,
            "extends": preset,
            "seed": {},
            "typography": {},
            "form": {}
        }))
        .map_err(CliError::json_output)?
    } else {
        preset.json().trim_end().to_owned()
    };
    crate::io::write_output(
        &output,
        format!("{document}\n").as_bytes(),
        control,
        publications,
        &context.stdout,
        context.publication.as_mut(),
    )
}

#[cfg(feature = "svg")]
pub(crate) fn check(
    args: ResolvedThemeCheck,
    control: &OperationControl,
    context: &mut ExecutionContext,
) -> Result<i32, CliError> {
    let compiler = ThemeCompiler::new();
    let document = read_document(
        args.input.file(),
        context.stdin.as_mut(),
        &compiler,
        control,
    )?;
    let resolved = compiler.compile_json(&document, args.appearance);
    crate::operation::checkpoint(control, OperationPhase::Emit)?;
    let resolved = match resolved {
        Ok(resolved) => resolved,
        Err(error) if args.json => {
            crate::diagnostics::write_json_stdout(
                &serde_json::json!({
                    "ok": false,
                    "line": error.line(),
                    "column": error.column(),
                    "path": error.path(),
                    "message": error.to_string()
                }),
                true,
                &context.stdout,
            )?;
            return Ok(2);
        }
        Err(error) => {
            return Err(CliError::InvalidInput(format!(
                "invalid theme document {}",
                format_error(&error)
            )));
        }
    };
    let contrast = contrast_rows(&resolved);
    let notices = resolved.notices();
    let ok = !args.strict_contrast
        || !notices
            .iter()
            .any(|notice| matches!(notice, ThemeNotice::LowContrast { .. }));
    if args.json {
        #[derive(Serialize)]
        struct CheckResult<'a> {
            ok: bool,
            resolved: &'a ResolvedThemeV2,
            contrast: &'a [ContrastRow],
            notices: &'a [ThemeNotice],
        }
        crate::diagnostics::write_json_stdout(
            &CheckResult {
                ok,
                resolved: &resolved,
                contrast: &contrast,
                notices,
            },
            true,
            &context.stdout,
        )?;
    } else {
        let mut output = format!("Theme appearance: {}\n\n", resolved.appearance());
        let _ = writeln!(
            output,
            "{:<22} {:<20} WCAG ratio",
            "Foreground", "Background"
        );
        for row in contrast {
            let _ = writeln!(
                output,
                "{:<22} {:<20} {:.2}:1",
                row.foreground, row.background, row.ratio
            );
        }
        for notice in notices {
            let _ = writeln!(output, "Warning: {}", format_notice(notice));
        }
        crate::io::write_stdout(output.as_bytes(), &context.stdout)?;
    }
    Ok(i32::from(!ok))
}

#[cfg(feature = "svg")]
#[derive(Serialize)]
struct ContrastRow {
    foreground: &'static str,
    background: &'static str,
    ratio: f64,
}

#[cfg(feature = "svg")]
fn contrast_rows(theme: &ResolvedThemeV2) -> [ContrastRow; 13] {
    let roles = theme.roles();
    let appearance = theme.appearance();
    let pie = theme.family("pie");
    let on_canvas = |foreground, paint: &Paint| ContrastRow {
        foreground,
        background: "canvas",
        ratio: paint.minimum_contrast_over(&roles.canvas, 1.0, None, appearance),
    };
    let solid_on_canvas = |foreground, color| on_canvas(foreground, &Paint::Solid(color));
    let on_surface =
        |foreground, color, background, paint: &Paint, background_opacity| ContrastRow {
            foreground,
            background,
            ratio: Paint::Solid(color).minimum_contrast_over(
                paint,
                background_opacity,
                Some(&roles.canvas),
                appearance,
            ),
        };
    [
        solid_on_canvas("text.primary", roles.text_primary),
        on_surface(
            "text.primary",
            roles.text_primary,
            "surface.node",
            &roles.surface_node,
            1.0,
        ),
        on_surface(
            "text.primary",
            roles.text_primary,
            "surface.container",
            &roles.surface_container,
            1.0,
        ),
        solid_on_canvas("text.muted", roles.text_muted),
        on_surface(
            "text.muted",
            roles.text_muted,
            "surface.node",
            &roles.surface_node,
            1.0,
        ),
        solid_on_canvas("text.title", roles.text_title),
        on_surface(
            "text.title",
            roles.text_title,
            "surface.container",
            &roles.surface_container,
            1.0,
        ),
        on_surface(
            "text.callout",
            roles.text_callout,
            "surface.callout",
            &roles.surface_callout,
            1.0,
        ),
        on_surface(
            "text.on_emphasis",
            roles.text_on_emphasis,
            "surface.emphasis",
            &roles.surface_emphasis,
            1.0,
        ),
        on_canvas("stroke.connector", &roles.stroke_connector),
        solid_on_canvas("data.ink[0]", theme.data().ink(0)),
        on_surface(
            "data.fill_ink[0]",
            theme.data().fill_ink(0),
            "data.fill",
            &Paint::Solid(theme.data().fill(0)),
            1.0,
        ),
        ContrastRow {
            foreground: "data.fill_ink[0]",
            background: "pie.data.fill",
            ratio: Paint::Solid(pie.data.fill_ink(0)).minimum_contrast_over(
                &Paint::Solid(pie.data.fill(0)),
                pie.form.chart.pie_opacity,
                Some(&pie.roles.canvas),
                appearance,
            ),
        },
    ]
}

#[cfg(feature = "svg")]
fn format_notice(notice: &ThemeNotice) -> String {
    match notice {
        ThemeNotice::LowContrast {
            foreground,
            background,
            ratio,
            minimum,
        } => format!("{foreground} on {background} has {ratio:.2}:1 contrast (target {minimum}:1)"),
        ThemeNotice::ContrastAdjusted { foreground, color } => {
            format!("adjusted {foreground} to {color} for contrast")
        }
    }
}

#[cfg(feature = "svg")]
pub(crate) fn report_outcome(
    outcome: &ThemeOutcome,
    quiet: bool,
    chart: Option<u64>,
    stderr: &SharedWriter,
) {
    if quiet || outcome.notices.is_empty() {
        return;
    }
    let location = chart.map_or_else(
        || "theme".to_owned(),
        |chart| format!("chart {chart} theme"),
    );
    let sink = crate::diagnostics::DiagnosticSink::new(false, stderr);
    for notice in outcome.notices.iter().take(3) {
        let message = match notice {
            ThemeOutcomeNotice::Compilation(notice) => format_notice(notice),
            ThemeOutcomeNotice::Rendering(message) => (*message).to_owned(),
        };
        sink.info(merman::normalize_terminal_diagnostic(&format!(
            "{location}: {message}"
        )));
    }
    if outcome.notices.len() > 3 {
        sink.info(format!(
            "{location}: {} more notices",
            outcome.notices.len() - 3
        ));
    }
}
