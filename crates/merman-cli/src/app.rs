use crate::cli::Cli;
use crate::error::CliError;
use crate::invocation::{ColorEnvironment, InvocationFacts};
use clap::error::ErrorKind;
use clap::{CommandFactory, FromArgMatches};
use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;

pub(crate) struct CliApp;

impl CliApp {
    pub(crate) fn system() -> Self {
        Self
    }

    pub(crate) fn execute(self) -> ExitCode {
        let facts = match system_facts() {
            Ok(facts) => facts,
            Err(error) => return report_error(CliError::Io(error)),
        };
        self.execute_from(env::args_os(), facts)
    }

    fn execute_from(
        self,
        args: impl IntoIterator<Item = OsString>,
        facts: InvocationFacts,
    ) -> ExitCode {
        let args = args.into_iter().collect::<Vec<_>>();
        let cli = match parse_cli(&args) {
            Ok(cli) => cli,
            Err(error) => return print_clap_error(error),
        };
        let invocation = match crate::invocation::resolve(cli, &facts) {
            Ok(invocation) => invocation,
            Err(error) => {
                if let Some(command) = error.missing_input_command() {
                    write_command_help_to_stderr(command);
                }
                return report_error(error);
            }
        };
        match crate::commands::run(invocation) {
            Ok(exit_code) => exit_code_from_i32(exit_code),
            Err(error) => report_error(error),
        }
    }
}

pub(crate) fn cli_command() -> clap::Command {
    let command = Cli::command();
    let help = root_help(&command);
    command.override_help(help)
}

fn parse_cli(args: &[OsString]) -> Result<Cli, clap::Error> {
    let mut command = cli_command();
    match command.try_get_matches_from_mut(args) {
        Ok(mut matches) => Cli::from_arg_matches_mut(&mut matches),
        Err(error) => {
            if let Some(argument) = removed_root_render_flag(error.kind(), args) {
                return Err(clap::Error::raw(
                    ErrorKind::UnknownArgument,
                    removed_root_render_message(argument),
                )
                .with_cmd(&command));
            }
            Err(error)
        }
    }
}

fn removed_root_render_flag(kind: ErrorKind, args: &[OsString]) -> Option<&str> {
    if !matches!(
        kind,
        ErrorKind::UnknownArgument | ErrorKind::InvalidSubcommand
    ) || !cfg!(any(feature = "svg", feature = "ascii"))
    {
        return None;
    }
    let Some(argument) = args.get(1).and_then(|value| value.to_str()) else {
        return None;
    };
    let long_name = argument.split_once('=').map_or(argument, |(name, _)| name);
    const LONG_FLAGS: &[&str] = &[
        "--input",
        "--output",
        "--out",
        "--outputFormat",
        "--output-format",
        "--format",
        "--artefacts",
        "--artifacts",
        "--jobs",
        "--scale",
        "--theme",
        "--width",
        "--height",
        "--svgId",
        "--svg-id",
        "--hand-drawn-seed",
        "--resource-profile",
        "--text-measurer",
        "--math-renderer",
        "--backgroundColor",
        "--background-color",
        "--background",
        "--configFile",
        "--config-file",
        "--cssFile",
        "--css-file",
        "--pdfFit",
        "--pdf-fit",
        "--svg-pipeline",
        "--suppress-errors",
        "--sequence-mirror-actors",
        "--ascii-charset",
        "--ascii-direction",
        "--ascii-color",
        "--xychart-vertical-plot-height",
        "--xychart-category-band-width",
        "--xychart-horizontal-plot-width",
        "--ascii-max-grid-cells",
        "--quiet",
        "--puppeteerConfigFile",
        "--puppeteer-config-file",
        "--raster-fit-width",
        "--raster-fit-height",
        "--raster-max-width",
        "--raster-max-height",
        "--raster-max-pixels",
        "--raster-unbounded",
        "--encoding-parallel-budget-mib",
        "--encoding-memory-budget-mib",
        "--pdf-filter-scale",
        "--pdf-max-filter-image-pixels",
        "--pdf-max-filter-pixels",
        "--pdf-filter-images-unbounded",
        "--pdf-filter-unbounded",
        "--embedded-image-max-bytes",
        "--embedded-image-max-total-bytes",
        "--embedded-image-max-pixels",
        "--embedded-image-max-total-pixels",
        "--embedded-images-unbounded",
        "--allow-network",
        "--iconPacks",
        "--iconPacksNamesAndUrls",
        "--runtime",
        "--system-clock",
        "--system-timezone",
        "--system-random",
        "--system-timing",
        "--fixed-today",
        "--fixed-local-offset-minutes",
    ];
    const SHORT_FLAGS: &[&str] = &[
        "-i", "-o", "-a", "-j", "-e", "-s", "-t", "-w", "-H", "-I", "-b", "-c", "-C", "-f", "-q",
        "-p",
    ];
    if LONG_FLAGS.contains(&long_name)
        || SHORT_FLAGS
            .iter()
            .any(|flag| argument == *flag || argument.starts_with(flag) && argument.len() > 2)
        || looks_like_removed_root_input(argument)
    {
        Some(argument)
    } else {
        None
    }
}

fn removed_root_render_message(argument: &str) -> String {
    let mut message = format!("root-level rendering syntax `{argument}` was removed");
    if cfg!(feature = "svg") {
        message.push_str("\nuse `merman-cli mmdc ...` for the pinned mmdc@11.16.0 interface");
    }
    if cfg!(any(feature = "svg", feature = "ascii")) {
        message.push_str(
            "\nuse `merman-cli render ...` for native single-diagram, JPEG, or text output",
        );
    }
    if cfg!(feature = "markdown") {
        message.push_str("\nuse `merman-cli batch ...` for native Markdown documents");
    }
    message
}

fn looks_like_removed_root_input(argument: &str) -> bool {
    if argument.starts_with('-') {
        return false;
    }
    std::path::Path::new(argument)
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "mmd" | "mermaid" | "md" | "markdown" | "mdx" | "svg"
            )
        })
}

fn root_help(command: &clap::Command) -> String {
    const GROUPS: [(&str, &[&str]); 4] = [
        ("Native rendering:", &["render", "batch"]),
        ("Analysis:", &["lint", "fix"]),
        ("Compatibility:", &["mmdc"]),
        (
            "Capabilities and tooling:",
            &[
                "detect",
                "parse",
                "layout",
                "capabilities",
                "lint-rules",
                "completion",
            ],
        ),
    ];

    let mut output = format!(
        "{}\n\nUsage: merman-cli <COMMAND> [ARGS]\n",
        command
            .get_about()
            .map(ToString::to_string)
            .unwrap_or_default()
    );
    for (heading, names) in GROUPS {
        let commands = names
            .iter()
            .filter_map(|name| command.find_subcommand(name))
            .collect::<Vec<_>>();
        if commands.is_empty() {
            continue;
        }
        output.push('\n');
        output.push_str(heading);
        output.push('\n');
        for subcommand in commands {
            let about = subcommand
                .get_about()
                .map(ToString::to_string)
                .unwrap_or_default();
            output.push_str(&format!("  {:<14} {about}\n", subcommand.get_name()));
        }
    }
    output.push_str("\nOptions:\n  -h, --help     Print help\n  -V, --version  Print version\n");
    output
}

fn system_facts() -> io::Result<InvocationFacts> {
    Ok(InvocationFacts {
        cwd: env::current_dir()?,
        stdin_is_terminal: io::stdin().is_terminal(),
        stdout_is_terminal: io::stdout().is_terminal(),
        color: ColorEnvironment {
            no_color: env::var_os("NO_COLOR").is_some(),
            force_color: env::var_os("CLICOLOR_FORCE")
                .is_some_and(|value| value != OsStr::new("") && value != OsStr::new("0")),
            colorterm: env::var("COLORTERM").ok(),
            term: env::var("TERM").ok(),
        },
    })
}

fn write_command_help_to_stderr(name: &str) {
    let mut command = cli_command();
    let Some(subcommand) = command.find_subcommand_mut(name) else {
        return;
    };
    let mut help = Vec::new();
    if subcommand.write_help(&mut help).is_ok() {
        let mut stderr = io::stderr().lock();
        let _ = stderr.write_all(&help);
        let _ = stderr.write_all(b"\n\n");
    }
}

fn print_clap_error(error: clap::Error) -> ExitCode {
    let exit_code = error.exit_code();
    let _ = error.print();
    exit_code_from_i32(exit_code)
}

fn report_error(error: CliError) -> ExitCode {
    if error.is_broken_stdout_pipe() {
        return ExitCode::SUCCESS;
    }
    let exit_code = error.exit_code();
    let mut stderr = io::stderr().lock();
    let _ = writeln!(stderr, "{error}");
    exit_code
}

fn exit_code_from_i32(exit_code: i32) -> ExitCode {
    u8::try_from(exit_code)
        .map(ExitCode::from)
        .unwrap_or(ExitCode::FAILURE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_help_groups_commands_by_user_task() {
        let help = root_help(&Cli::command());
        #[cfg(any(feature = "svg", feature = "ascii"))]
        assert!(help.contains("Native rendering:\n  render"));
        #[cfg(feature = "analysis")]
        assert!(help.contains("Analysis:\n  lint"));
        #[cfg(feature = "svg")]
        assert!(help.contains("Compatibility:\n  mmdc"));
        assert!(help.contains("Capabilities and tooling:"));
    }
}
