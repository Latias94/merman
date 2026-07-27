use crate::cli::{RenderFormat, TextCharset, TextColorMode, TextDirection, TextOutputCliArgs};
use crate::config::{engine_for, parse_options};
use crate::error::CliError;
use crate::input::InputLimit;
use crate::invocation::{ResolvedDestination, ResolvedOutput, ResolvedSingleRender};
use crate::io::{OutputTarget, read_input, write_output};
use std::env;
use std::io::{self, IsTerminal};

pub(crate) fn run_ascii_render(args: ResolvedSingleRender) -> Result<(), CliError> {
    let input = args.input.to_path_buf();
    let (format, destination, text_options) = match args.output {
        ResolvedOutput::Text {
            format,
            destination,
            options,
        } => (format, destination, options.into_legacy_cli_args()),
    };
    let output = match destination {
        ResolvedDestination::Stdout => Some(OutputTarget::Stdout),
        ResolvedDestination::File(path) => Some(OutputTarget::File(path)),
    };
    let quiet = args.common.quiet;
    let resources = args.common.resources;
    let parse = args.common.parse.into_legacy_cli_args();
    let source_limit = resources
        .input_policy()
        .value(merman::resources::InputResourceLimitId::MaxSourceBytes);
    let text = read_input(
        Some(&input),
        quiet,
        InputLimit::new(
            merman::resources::InputResourceLimitId::MaxSourceBytes.as_str(),
            source_limit,
        ),
    )?;
    let options = apply_text_options(
        text_options_for_output(text_options, output.as_ref()),
        format,
    )?;
    let renderer = merman::ascii::HeadlessAsciiRenderer::new()
        .with_engine(engine_for(&parse, &resources)?)
        .with_parse_options(parse_options(&parse))
        .with_ascii_options(options)
        .with_resource_policy(*resources.input_policy());
    let Some(rendered) = renderer.render_ascii_sync(&text)? else {
        return Err(CliError::NoDiagram);
    };
    write_output(output.as_ref(), rendered.as_bytes())
}

fn text_options_for_output(
    mut text: TextOutputCliArgs,
    output: Option<&OutputTarget>,
) -> TextOutputCliArgs {
    if text.ascii_color == Some(TextColorMode::Auto) {
        text.ascii_color = Some(resolve_auto_text_color(output));
    }
    text
}

fn resolve_auto_text_color(output: Option<&OutputTarget>) -> TextColorMode {
    if env::var_os("NO_COLOR").is_some() {
        return TextColorMode::Plain;
    }

    let color_term = env::var("COLORTERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();
    let truecolor = color_term.contains("truecolor") || color_term.contains("24bit");
    let force_color =
        env::var("CLICOLOR_FORCE").is_ok_and(|value| !value.is_empty() && value != "0");
    if force_color {
        return if truecolor {
            TextColorMode::Truecolor
        } else {
            TextColorMode::Ansi256
        };
    }

    let stdout_target = matches!(output, None | Some(OutputTarget::Stdout));
    if !stdout_target || !io::stdout().is_terminal() {
        return TextColorMode::Plain;
    }
    if truecolor {
        TextColorMode::Truecolor
    } else if term.contains("256color") {
        TextColorMode::Ansi256
    } else if !term.is_empty() && term != "dumb" {
        TextColorMode::Ansi16
    } else {
        TextColorMode::Plain
    }
}

fn apply_text_options(
    text: TextOutputCliArgs,
    format: RenderFormat,
) -> Result<merman::ascii::AsciiRenderOptions, CliError> {
    let mut options = match format {
        RenderFormat::Ascii => merman::ascii::AsciiRenderOptions::ascii(),
        RenderFormat::Unicode => merman::ascii::AsciiRenderOptions::unicode(),
    };
    if let Some(charset) = text.ascii_charset {
        options.charset = match charset {
            TextCharset::Ascii => merman::ascii::AsciiCharset::Ascii,
            TextCharset::Unicode => merman::ascii::AsciiCharset::Unicode,
        };
    }
    if let Some(direction) = text.ascii_direction {
        options.default_direction = match direction {
            TextDirection::LeftRight => merman::ascii::AsciiDirection::LeftRight,
            TextDirection::TopDown => merman::ascii::AsciiDirection::TopDown,
        };
    }
    if let Some(color_mode) = text.ascii_color {
        options.color_mode = match color_mode {
            TextColorMode::Plain => merman::ascii::AsciiColorMode::Plain,
            TextColorMode::Auto => {
                return Err(CliError::InvalidInput(
                    "ASCII color mode `auto` was not resolved when the render request was created"
                        .to_string(),
                ));
            }
            TextColorMode::Ansi16 => merman::ascii::AsciiColorMode::Ansi16,
            TextColorMode::Ansi256 => merman::ascii::AsciiColorMode::Ansi256,
            TextColorMode::Truecolor => merman::ascii::AsciiColorMode::TrueColor,
            TextColorMode::Html => merman::ascii::AsciiColorMode::Html,
        };
    }
    options.sequence_mirror_actors = text.sequence_mirror_actors;
    if let Some(height) = text.xychart_vertical_plot_height {
        options.xychart_vertical_plot_height = height;
    }
    if let Some(width) = text.xychart_category_band_width {
        options.xychart_category_band_width = width;
    }
    if let Some(width) = text.xychart_horizontal_plot_width {
        options.xychart_horizontal_plot_width = width;
    }
    if let Some(max_grid_cells) = text.ascii_max_grid_cells {
        options.max_grid_cells = max_grid_cells;
    }
    options
        .validate()
        .map_err(|err| CliError::InvalidInput(format!("invalid ASCII options: {err}")))?;
    Ok(options)
}
