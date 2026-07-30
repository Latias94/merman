use crate::cli::{Cli, RawCommand};
use crate::error::CliError;
#[cfg(feature = "ascii")]
use crate::invocation::ColorEnvironment;
use crate::invocation::InvocationFacts;
use crate::runtime::{ExecutionContext, SharedWriter};
#[cfg(any(feature = "svg", feature = "ascii"))]
use clap::ValueEnum;
use clap::error::ErrorKind;
use clap::{CommandFactory, FromArgMatches};
use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{self, IsTerminal};
use std::process::ExitCode;

#[cfg(all(
    test,
    feature = "analysis",
    feature = "ascii",
    feature = "icons",
    feature = "jpeg",
    feature = "layout-cytoscape",
    feature = "layout-elk",
    feature = "markdown",
    feature = "math",
    feature = "network-icons",
    feature = "parallel-markdown",
    feature = "pdf",
    feature = "png",
    feature = "shell-completions",
    feature = "svg",
    feature = "system-clock",
    feature = "system-random",
    feature = "system-timezone",
    feature = "system-timing"
))]
mod distribution_assets;

struct ProcessSnapshot {
    args: Vec<OsString>,
    cwd: io::Result<std::path::PathBuf>,
    stdin_is_terminal: bool,
    #[cfg(feature = "ascii")]
    stdout_is_terminal: bool,
    #[cfg(feature = "ascii")]
    color: ColorEnvironment,
}

impl ProcessSnapshot {
    fn system() -> Self {
        Self {
            args: env::args_os().collect(),
            cwd: env::current_dir(),
            stdin_is_terminal: io::stdin().is_terminal(),
            #[cfg(feature = "ascii")]
            stdout_is_terminal: io::stdout().is_terminal(),
            #[cfg(feature = "ascii")]
            color: ColorEnvironment {
                no_color: nonempty_environment_value(env::var_os("NO_COLOR").as_deref()),
                force_color: env::var_os("CLICOLOR_FORCE")
                    .is_some_and(|value| value != OsStr::new("") && value != OsStr::new("0")),
                colorterm: env::var("COLORTERM").ok(),
                term: env::var("TERM").ok(),
            },
        }
    }

    fn into_facts(self, require_working_directory: bool) -> io::Result<InvocationFacts> {
        let cwd = match self.cwd {
            Ok(cwd) => Some(cwd),
            Err(_) if !require_working_directory => None,
            Err(error) => return Err(error),
        };
        Ok(InvocationFacts {
            cwd,
            stdin_is_terminal: self.stdin_is_terminal,
            #[cfg(feature = "ascii")]
            stdout_is_terminal: self.stdout_is_terminal,
            #[cfg(feature = "ascii")]
            color: self.color,
        })
    }
}

pub(crate) struct CliApp {
    process: ProcessSnapshot,
    execution: ExecutionContext,
}

pub(crate) fn run_system() -> ExitCode {
    CliApp {
        process: ProcessSnapshot::system(),
        execution: ExecutionContext::system(),
    }
    .execute()
}

impl CliApp {
    fn execute(self) -> ExitCode {
        let Self {
            process,
            mut execution,
        } = self;
        let parsed = match parse_cli(&process.args) {
            Ok(parsed) => parsed,
            Err(error) => {
                return print_clap_error(error, &execution.stdout, &execution.stderr);
            }
        };
        if parsed.should_warn_about_deprecated_root_mmdc()
            && let Err(source) = execution.stderr.write_all(DEPRECATED_ROOT_MMDC_WARNING)
        {
            return CliError::stream("stderr", source).exit_code();
        }
        if let Some(warning) = parsed.deprecated_native_format_warning()
            && let Err(source) = execution.stderr.write_all(warning.as_bytes())
        {
            return CliError::stream("stderr", source).exit_code();
        }
        let cli = parsed.cli;
        let facts = match process.into_facts(command_needs_working_directory(&cli.command)) {
            Ok(facts) => facts,
            Err(error) => return report_error(CliError::Io(error), &execution.stderr),
        };
        Self::execute_parsed(cli, facts, &mut execution)
    }

    fn execute_parsed(
        cli: Cli,
        facts: InvocationFacts,
        execution: &mut ExecutionContext,
    ) -> ExitCode {
        let invocation = match crate::invocation::resolve(cli, &facts) {
            Ok(invocation) => invocation,
            Err(error) => {
                if let Some(command) = error.missing_input_command()
                    && let Err(source) = write_command_help_to_stderr(command, &execution.stderr)
                {
                    return CliError::stream("stderr", source).exit_code();
                }
                return report_error(error, &execution.stderr);
            }
        };
        let Some(cwd) = facts.cwd.as_deref() else {
            return match crate::output::LocalPreflight::path_free(invocation) {
                Ok(preflight) => run_preflight(preflight, execution),
                Err(error) => report_error(error, &execution.stderr),
            };
        };
        let preflight = match crate::output::preflight(invocation, cwd) {
            Ok(preflight) => preflight,
            Err(error) => return report_error(error, &execution.stderr),
        };
        run_preflight(preflight, execution)
    }
}

fn run_preflight(
    preflight: crate::output::LocalPreflight,
    execution: &mut ExecutionContext,
) -> ExitCode {
    match crate::commands::run(preflight, execution) {
        Ok(exit_code) => exit_code_from_i32(exit_code),
        Err(error) => report_error(error, &execution.stderr),
    }
}

fn command_needs_working_directory(command: &RawCommand) -> bool {
    match command {
        RawCommand::Capabilities(_) => false,
        #[cfg(feature = "analysis")]
        RawCommand::LintRules(_) => false,
        #[cfg(feature = "shell-completions")]
        RawCommand::Completion(_) => false,
        _ => true,
    }
}

pub(crate) fn cli_command() -> clap::Command {
    let command = Cli::command()
        .mut_subcommand("detect", |subcommand| {
            subcommand.after_help(
                "More options: `merman-cli detect --help` shows source resource controls.",
            )
        })
        .mut_subcommand("parse", |subcommand| {
            subcommand.after_help(
                "More options: `merman-cli parse --help` shows configuration, runtime, and resource controls.",
            )
        });
    #[cfg(any(feature = "svg", feature = "ascii"))]
    let command = command.mut_subcommand("render", |subcommand| {
        subcommand.after_help(
            "Examples:\n  merman-cli render diagram.mmd\n\nMore options: `merman-cli render --help` shows advanced renderer, resource, and security controls.",
        )
    });
    #[cfg(feature = "markdown")]
    let command = command.mut_subcommand("batch", |subcommand| {
        subcommand.after_help(
            "Examples:\n  merman-cli batch README.md\n\nMore options: `merman-cli batch --help` shows advanced renderer, resource, and security controls.",
        )
    });
    #[cfg(feature = "analysis")]
    let command = command
        .mut_subcommand("lint", |subcommand| {
            subcommand.after_help(
                "Examples:\n  merman-cli lint diagram.mmd\n  merman-cli lint --format json diagram.mmd\n\nMore options: `merman-cli lint --help` shows advanced rule, runtime, and resource controls.",
            )
        })
        .mut_subcommand("fix", |subcommand| {
            subcommand.after_help(
                "Examples:\n  merman-cli fix --check diagram.mmd\n  merman-cli fix --diff diagram.mmd\n\nMore options: `merman-cli fix --help` shows advanced rule, runtime, and resource controls.",
            )
        });
    #[cfg(feature = "shell-completions")]
    let command = command.mut_subcommand("completion", |subcommand| {
        subcommand
            .after_help("Examples:\n  merman-cli completion bash\n  merman-cli completion zsh")
    });
    #[cfg(feature = "svg")]
    let command = command
        .mut_subcommand("layout", |subcommand| {
            subcommand.after_help(
                "More options: `merman-cli layout --help` shows layout, configuration, runtime, and resource controls.",
            )
        })
        .mut_subcommand("mmdc", |subcommand| {
            subcommand.after_help(
                "Examples:\n  merman-cli mmdc -i diagram.mmd -o diagram.svg\n\nMore options: `merman-cli mmdc --help` shows advanced compatibility, renderer, resource, and security controls.",
            )
        });
    let help = root_help(&command);
    command.override_help(help)
}

#[cfg(feature = "shell-completions")]
pub(crate) fn completion_script(shell: clap_complete::aot::Shell) -> Vec<u8> {
    let mut command = cli_command();
    if shell == clap_complete::aot::Shell::Bash {
        // The Bash generator uses different separators for the binary name and
        // command path. Give its internal root an already-normalized name so a
        // hyphenated executable reaches the generated subcommand branches.
        command = command.name("merman__cli");
        command.build();
    }
    let mut output = Vec::new();
    clap_complete::aot::generate(shell, &mut command, "merman-cli", &mut output);
    output
}

const DEPRECATED_ROOT_MMDC_WARNING: &[u8] = b"warning: root-level mmdc-compatible options are deprecated and will be removed in v0.9.0\nhelp: use `merman-cli mmdc ...`; the explicit `mmdc` subcommand remains supported\n";

struct ParsedCli {
    cli: Cli,
    deprecated_root_mmdc: bool,
    deprecated_native_format: bool,
}

impl ParsedCli {
    fn should_warn_about_deprecated_root_mmdc(&self) -> bool {
        #[cfg(feature = "svg")]
        {
            self.deprecated_root_mmdc
                && matches!(&self.cli.command, RawCommand::Mmdc(args) if !args.quiet)
        }
        #[cfg(not(feature = "svg"))]
        {
            let _ = self.deprecated_root_mmdc;
            false
        }
    }

    fn deprecated_native_format_warning(&self) -> Option<String> {
        if !self.deprecated_native_format {
            return None;
        }
        match &self.cli.command {
            #[cfg(any(feature = "svg", feature = "ascii"))]
            RawCommand::Render(args) => args.options.format.and_then(|format| {
                format.to_possible_value().map(|value| {
                    format!(
                        "warning: native `render -e {}` is deprecated and will be removed in v0.9.0\nhelp: use `merman-cli render -f {}`\n",
                        value.get_name(),
                        value.get_name()
                    )
                })
            }),
            #[cfg(feature = "markdown")]
            RawCommand::Batch(args) => args.options.format.and_then(|format| {
                format.to_possible_value().map(|value| {
                    format!(
                        "warning: native `batch -e {}` is deprecated and will be removed in v0.9.0\nhelp: use `merman-cli batch -f {}`\n",
                        value.get_name(),
                        value.get_name()
                    )
                })
            }),
            _ => None,
        }
    }
}

fn parse_cli(args: &[OsString]) -> Result<ParsedCli, clap::Error> {
    let mut command = cli_command();
    let deprecated_native_format = uses_deprecated_native_format(args);
    let normalized_args = deprecated_root_mmdc_args(&command, args);
    let deprecated_root_mmdc = normalized_args.is_some();
    let parse_args = normalized_args.as_deref().unwrap_or(args);
    match command.try_get_matches_from_mut(parse_args) {
        Ok(mut matches) => Cli::from_arg_matches_mut(&mut matches).map(|cli| ParsedCli {
            cli,
            deprecated_root_mmdc,
            deprecated_native_format,
        }),
        Err(error) => {
            if let Some(argument) = superseded_resource_flag(error.kind(), args) {
                return Err(clap::Error::raw(
                    ErrorKind::UnknownArgument,
                    superseded_resource_message(argument),
                )
                .with_cmd(&command));
            }
            if !deprecated_root_mmdc
                && let Some(argument) = removed_root_render_flag(error.kind(), args)
            {
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

fn uses_deprecated_native_format(args: &[OsString]) -> bool {
    if !matches!(
        args.get(1).and_then(|argument| argument.to_str()),
        Some("render" | "batch")
    ) {
        return false;
    }
    args.iter()
        .skip(2)
        .take_while(|argument| argument.as_os_str() != OsStr::new("--"))
        .filter_map(|argument| argument.to_str())
        .any(|argument| {
            argument == "-e"
                || argument
                    .strip_prefix("-e")
                    .is_some_and(|value| !value.is_empty())
        })
}

fn deprecated_root_mmdc_args(command: &clap::Command, args: &[OsString]) -> Option<Vec<OsString>> {
    let argument = args.get(1)?.to_string_lossy();
    if !is_mmdc_option(command, &argument) {
        return None;
    }

    let mut normalized = Vec::with_capacity(args.len() + 1);
    normalized.push(args.first()?.clone());
    normalized.push(OsString::from("mmdc"));
    normalized.extend(args.iter().skip(1).cloned());
    Some(normalized)
}

fn is_mmdc_option(command: &clap::Command, argument: &str) -> bool {
    if matches!(argument, "--help" | "--version" | "-h" | "-V") {
        return false;
    }
    let Some(option) = argument
        .strip_prefix('-')
        .filter(|option| !option.is_empty())
    else {
        return false;
    };
    let Some(mmdc) = command.find_subcommand("mmdc") else {
        return false;
    };
    if let Some(long) = option.strip_prefix('-') {
        let name = long.split_once('=').map_or(long, |(name, _)| name);
        return !name.is_empty()
            && mmdc.get_arguments().any(|candidate| {
                candidate.get_long() == Some(name)
                    || candidate
                        .get_all_aliases()
                        .is_some_and(|aliases| aliases.contains(&name))
            });
    }
    let Some(short) = option.chars().next() else {
        return false;
    };
    mmdc.get_arguments().any(|candidate| {
        candidate.get_short() == Some(short)
            || candidate
                .get_all_short_aliases()
                .is_some_and(|aliases| aliases.contains(&short))
    })
}

fn superseded_resource_flag(kind: ErrorKind, args: &[OsString]) -> Option<&str> {
    if kind != ErrorKind::UnknownArgument {
        return None;
    }
    args.iter()
        .filter_map(|argument| argument.to_str())
        .find_map(
            |argument| match argument.split_once('=').map_or(argument, |(name, _)| name) {
                "--max-source-bytes" => Some("--max-source-bytes"),
                "--encoding-parallel-budget-mib" => Some("--encoding-parallel-budget-mib"),
                "--encoding-memory-budget-mib" => Some("--encoding-memory-budget-mib"),
                _ => None,
            },
        )
}

fn superseded_resource_message(argument: &str) -> String {
    match argument {
        "--max-source-bytes" => {
            format!("`{argument}` was removed; use `--resource-limit max_source_bytes=BYTES`")
        }
        _ => format!(
            "`{argument}` was removed; use `--resource-limit max_scheduling_weight_bytes=BYTES` and `--jobs` to control raster/PDF concurrency"
        ),
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
    let argument = args.get(1).and_then(|value| value.to_str())?;
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
        "--id",
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
        message.push_str(&format!(
            "\nuse `merman-cli mmdc ...` for the pinned mmdc@{} interface",
            merman::baseline::PINNED_MERMAID_CLI_VERSION
        ));
    }
    if cfg!(any(feature = "svg", feature = "ascii")) {
        message.push_str("\nuse `merman-cli render ...` for native single-diagram output");
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

#[cfg(feature = "ascii")]
fn nonempty_environment_value(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| !value.is_empty())
}

fn write_command_help_to_stderr(name: &str, stderr: &SharedWriter) -> io::Result<()> {
    let mut command = cli_command();
    let Some(subcommand) = command.find_subcommand_mut(name) else {
        return Ok(());
    };
    let mut help = Vec::new();
    subcommand.write_help(&mut help)?;
    stderr.write_all(&help)?;
    stderr.write_all(b"\n\n")
}

fn print_clap_error(error: clap::Error, stdout: &SharedWriter, stderr: &SharedWriter) -> ExitCode {
    let exit_code = error.exit_code();
    let (stream, destination) = if error.use_stderr() {
        ("stderr", stderr)
    } else {
        ("stdout", stdout)
    };
    match destination.write_all(error.render().to_string().as_bytes()) {
        Ok(()) => exit_code_from_i32(exit_code),
        Err(source) => CliError::stream(stream, source).exit_code(),
    }
}

fn report_error(error: CliError, stderr: &SharedWriter) -> ExitCode {
    if error.is_broken_stdout_pipe() {
        return ExitCode::SUCCESS;
    }
    let exit_code = error.exit_code();
    match stderr.with_writer(|stderr| writeln!(stderr, "{error}")) {
        Ok(()) => exit_code,
        Err(source) => CliError::stream("stderr", source).exit_code(),
    }
}

fn exit_code_from_i32(exit_code: i32) -> ExitCode {
    u8::try_from(exit_code)
        .map(ExitCode::from)
        .unwrap_or(ExitCode::FAILURE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read, Write};
    use std::path::PathBuf;
    #[cfg(any(
        feature = "analysis",
        feature = "svg",
        feature = "ascii",
        feature = "markdown",
        feature = "network-icons"
    ))]
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for CaptureWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct BrokenPipeWriter;

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("injected stream failure"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[cfg(any(feature = "svg", feature = "ascii"))]
    struct CountingReader {
        reads: Arc<AtomicUsize>,
        source: Cursor<Vec<u8>>,
    }

    #[cfg(any(feature = "svg", feature = "ascii"))]
    impl Read for CountingReader {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            self.source.read(bytes)
        }
    }

    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    struct RejectingPublication {
        calls: Arc<AtomicUsize>,
    }

    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    impl crate::output::PublicationBackend for RejectingPublication {
        #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
        fn publish_file(
            &mut self,
            _path: &std::path::Path,
            _bytes: &[u8],
            _publications: &crate::output::PublicationGuards,
        ) -> Result<(), CliError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(CliError::Io(io::Error::other(
                "injected publication failure",
            )))
        }

        #[cfg(feature = "analysis")]
        fn publish_file_verified(
            &mut self,
            _path: &std::path::Path,
            _bytes: &[u8],
            _publications: &crate::output::PublicationGuards,
            _verify: &mut dyn FnMut(&std::path::Path) -> Result<(), CliError>,
        ) -> Result<(), CliError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(CliError::Io(io::Error::other(
                "injected publication failure",
            )))
        }

        #[cfg(feature = "markdown")]
        fn acquire_transaction(
            &mut self,
            _publications: &crate::output::PublicationGuards,
        ) -> Result<crate::output::AcquiredTransaction, CliError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(CliError::Io(io::Error::other(
                "injected publication failure",
            )))
        }
    }

    #[cfg(feature = "markdown")]
    struct RejectingCommitPublication {
        system: crate::output::SystemPublicationBackend,
        commits: Arc<AtomicUsize>,
    }

    #[cfg(feature = "markdown")]
    impl crate::output::PublicationBackend for RejectingCommitPublication {
        fn publish_file(
            &mut self,
            path: &std::path::Path,
            bytes: &[u8],
            publications: &crate::output::PublicationGuards,
        ) -> Result<(), CliError> {
            self.system.publish_file(path, bytes, publications)
        }

        #[cfg(feature = "analysis")]
        fn publish_file_verified(
            &mut self,
            path: &std::path::Path,
            bytes: &[u8],
            publications: &crate::output::PublicationGuards,
            verify: &mut dyn FnMut(&std::path::Path) -> Result<(), CliError>,
        ) -> Result<(), CliError> {
            self.system
                .publish_file_verified(path, bytes, publications, verify)
        }

        fn acquire_transaction(
            &mut self,
            publications: &crate::output::PublicationGuards,
        ) -> Result<crate::output::AcquiredTransaction, CliError> {
            self.system.acquire_transaction(publications)
        }

        fn commit_transaction(
            &mut self,
            _ready: crate::transaction::ReadyTransaction,
        ) -> Result<(), CliError> {
            self.commits.fetch_add(1, Ordering::SeqCst);
            Err(CliError::Io(io::Error::other(
                "injected transaction commit failure",
            )))
        }
    }

    #[cfg(feature = "network-icons")]
    struct FixtureNetwork {
        calls: Arc<AtomicUsize>,
    }

    #[cfg(feature = "network-icons")]
    impl crate::network::NetworkAcquirer for FixtureNetwork {
        fn fetch(
            &mut self,
            _raw_url: &str,
            _policy: crate::network::NetworkPolicy,
        ) -> Result<Vec<u8>, crate::network::NetworkError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(br#"{"prefix":"test","icons":{"rocket":{"body":"<path data-injected-network=\"true\"/>"}}}"#.to_vec())
        }
    }

    fn capture() -> (SharedWriter, Arc<Mutex<Vec<u8>>>) {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        (SharedWriter::new(CaptureWriter(Arc::clone(&bytes))), bytes)
    }

    fn snapshot(
        args: &[&str],
        cwd: io::Result<PathBuf>,
        stdin_is_terminal: bool,
    ) -> ProcessSnapshot {
        ProcessSnapshot {
            args: std::iter::once(OsString::from("merman-cli"))
                .chain(args.iter().map(|argument| OsString::from(*argument)))
                .collect(),
            cwd,
            stdin_is_terminal,
            #[cfg(feature = "ascii")]
            stdout_is_terminal: false,
            #[cfg(feature = "ascii")]
            color: ColorEnvironment {
                no_color: false,
                force_color: false,
                colorterm: None,
                term: None,
            },
        }
    }

    fn execution(
        stdin: impl Read + Send + 'static,
        stdout: SharedWriter,
        stderr: SharedWriter,
    ) -> ExecutionContext {
        ExecutionContext {
            stdin: Box::new(stdin),
            stdout,
            stderr,
            #[cfg(feature = "network-icons")]
            network: Box::new(crate::network::SystemNetworkAcquirer),
            #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
            publication: Box::new(crate::output::SystemPublicationBackend),
        }
    }

    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    fn execution_with_publication(
        stdin: impl Read + Send + 'static,
        stdout: SharedWriter,
        stderr: SharedWriter,
        publication: Box<dyn crate::output::PublicationBackend>,
    ) -> ExecutionContext {
        ExecutionContext {
            stdin: Box::new(stdin),
            stdout,
            stderr,
            #[cfg(feature = "network-icons")]
            network: Box::new(crate::network::SystemNetworkAcquirer),
            publication,
        }
    }

    fn bytes(buffer: &Arc<Mutex<Vec<u8>>>) -> String {
        String::from_utf8(buffer.lock().unwrap().clone()).unwrap()
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn no_color_requires_a_non_empty_value() {
        assert!(!nonempty_environment_value(None));
        assert!(!nonempty_environment_value(Some(OsStr::new(""))));
        assert!(nonempty_environment_value(Some(OsStr::new("0"))));
        assert!(nonempty_environment_value(Some(OsStr::new("1"))));
    }

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

    #[test]
    fn only_commands_that_resolve_paths_require_a_working_directory() {
        assert!(!command_needs_working_directory(&RawCommand::Capabilities(
            crate::cli::CapabilitiesArgs { json: true }
        )));
    }

    #[test]
    fn injected_streams_preserve_help_and_error_channels() {
        let (stdout, stdout_bytes) = capture();
        let (stderr, stderr_bytes) = capture();
        let app = CliApp {
            process: snapshot(&["--help"], Ok(PathBuf::from(".")), true),
            execution: execution(io::empty(), stdout, stderr),
        };
        assert_eq!(app.execute(), ExitCode::SUCCESS);
        assert!(bytes(&stdout_bytes).contains("Usage: merman-cli <COMMAND>"));
        assert!(bytes(&stderr_bytes).is_empty());

        let (stdout, stdout_bytes) = capture();
        let (stderr, stderr_bytes) = capture();
        let app = CliApp {
            process: snapshot(&["--not-a-command"], Ok(PathBuf::from(".")), true),
            execution: execution(io::empty(), stdout, stderr),
        };
        assert_eq!(app.execute(), ExitCode::from(2));
        assert!(bytes(&stdout_bytes).is_empty());
        assert!(bytes(&stderr_bytes).contains("unexpected argument"));
    }

    #[test]
    fn piped_input_and_payload_use_the_injected_streams() {
        let (stdout, stdout_bytes) = capture();
        let (stderr, stderr_bytes) = capture();
        let app = CliApp {
            process: snapshot(&["detect"], Ok(PathBuf::from(".")), false),
            execution: execution(
                Cursor::new(b"flowchart LR\nA-->B\n".to_vec()),
                stdout,
                stderr,
            ),
        };

        assert_eq!(app.execute(), ExitCode::SUCCESS);
        assert_eq!(bytes(&stdout_bytes), "flowchart-v2\n");
        assert!(bytes(&stderr_bytes).is_empty());
    }

    #[test]
    fn injected_working_directory_anchors_relative_input_acquisition() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("injected-cwd.mmd"),
            "flowchart LR\nA-->B\n",
        )
        .unwrap();
        let (stdout, stdout_bytes) = capture();
        let (stderr, stderr_bytes) = capture();
        let app = CliApp {
            process: snapshot(
                &["detect", "injected-cwd.mmd"],
                Ok(directory.path().to_path_buf()),
                true,
            ),
            execution: execution(io::empty(), stdout, stderr),
        };

        assert_eq!(app.execute(), ExitCode::SUCCESS);
        assert_eq!(bytes(&stdout_bytes), "flowchart-v2\n");
        assert!(bytes(&stderr_bytes).is_empty());
    }

    #[test]
    fn injected_help_and_usage_stream_failures_are_operational() {
        let (stderr, _) = capture();
        let help = CliApp {
            process: snapshot(&["--help"], Ok(PathBuf::from(".")), true),
            execution: execution(io::empty(), SharedWriter::new(FailingWriter), stderr),
        };
        assert_eq!(help.execute(), ExitCode::from(3));

        let (stdout, _) = capture();
        let usage = CliApp {
            process: snapshot(&["--not-a-command"], Ok(PathBuf::from(".")), true),
            execution: execution(io::empty(), stdout, SharedWriter::new(FailingWriter)),
        };
        assert_eq!(usage.execute(), ExitCode::from(3));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn deprecated_root_warning_failure_stops_before_input_acquisition() {
        let reads = Arc::new(AtomicUsize::new(0));
        let (stdout, stdout_bytes) = capture();
        let app = CliApp {
            process: snapshot(&["-i", "-", "-o", "-"], Ok(PathBuf::from(".")), false),
            execution: execution(
                CountingReader {
                    reads: Arc::clone(&reads),
                    source: Cursor::new(b"flowchart LR\nA-->B\n".to_vec()),
                },
                stdout,
                SharedWriter::new(FailingWriter),
            ),
        };

        assert_eq!(app.execute(), ExitCode::from(3));
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert!(bytes(&stdout_bytes).is_empty());
    }

    #[cfg(any(feature = "svg", feature = "ascii"))]
    #[test]
    fn terminal_missing_input_stops_before_acquisition_or_publication() {
        let reads = Arc::new(AtomicUsize::new(0));
        let publication_calls = Arc::new(AtomicUsize::new(0));
        let (stdout, stdout_bytes) = capture();
        let (stderr, stderr_bytes) = capture();
        let app = CliApp {
            process: snapshot(&["render"], Ok(PathBuf::from(".")), true),
            execution: execution_with_publication(
                CountingReader {
                    reads: Arc::clone(&reads),
                    source: Cursor::new(b"flowchart LR\nA-->B\n".to_vec()),
                },
                stdout,
                stderr,
                Box::new(RejectingPublication {
                    calls: Arc::clone(&publication_calls),
                }),
            ),
        };

        assert_eq!(app.execute(), ExitCode::from(2));
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert_eq!(publication_calls.load(Ordering::SeqCst), 0);
        assert!(bytes(&stdout_bytes).is_empty());
        let stderr = bytes(&stderr_bytes);
        assert!(
            stderr.contains("Usage:") && stderr.contains("render"),
            "{stderr}"
        );
    }

    #[test]
    fn unavailable_working_directory_is_ignored_only_for_path_free_commands() {
        let (stdout, stdout_bytes) = capture();
        let (stderr, stderr_bytes) = capture();
        let app = CliApp {
            process: snapshot(
                &["capabilities", "--json"],
                Err(io::Error::new(io::ErrorKind::NotFound, "cwd unavailable")),
                true,
            ),
            execution: execution(io::empty(), stdout, stderr),
        };
        assert_eq!(app.execute(), ExitCode::SUCCESS);
        assert!(bytes(&stdout_bytes).contains("\"schema_version\": 2"));
        assert!(bytes(&stderr_bytes).is_empty());

        let (stdout, _) = capture();
        let (stderr, stderr_bytes) = capture();
        let app = CliApp {
            process: snapshot(
                &["detect", "-"],
                Err(io::Error::new(io::ErrorKind::NotFound, "cwd unavailable")),
                false,
            ),
            execution: execution(
                Cursor::new(b"flowchart LR\nA-->B\n".to_vec()),
                stdout,
                stderr,
            ),
        };
        assert_eq!(app.execute(), ExitCode::from(3));
        assert!(bytes(&stderr_bytes).contains("cwd unavailable"));
    }

    #[test]
    fn injected_broken_stdout_pipe_remains_successful() {
        let (stderr, stderr_bytes) = capture();
        let app = CliApp {
            process: snapshot(&["detect", "-"], Ok(PathBuf::from(".")), false),
            execution: execution(
                Cursor::new(b"flowchart LR\nA-->B\n".to_vec()),
                SharedWriter::new(BrokenPipeWriter),
                stderr,
            ),
        };

        assert_eq!(app.execute(), ExitCode::SUCCESS);
        assert!(bytes(&stderr_bytes).is_empty());
    }

    #[cfg(feature = "network-icons")]
    #[test]
    fn injected_network_acquirer_drives_the_icon_workflow() {
        let network_calls = Arc::new(AtomicUsize::new(0));
        let (stdout, stdout_bytes) = capture();
        let (stderr, stderr_bytes) = capture();
        let mut execution = execution(
            Cursor::new(b"flowchart TD\nA@{ icon: \"test:rocket\", label: \"Rocket\" }\n".to_vec()),
            stdout,
            stderr,
        );
        execution.network = Box::new(FixtureNetwork {
            calls: Arc::clone(&network_calls),
        });
        let app = CliApp {
            process: snapshot(
                &[
                    "render",
                    "--format",
                    "svg",
                    "--allow-network",
                    "--icon-pack-source",
                    "test#https://example.com/icons.json",
                    "-",
                ],
                Ok(PathBuf::from(".")),
                false,
            ),
            execution,
        };

        assert_eq!(app.execute(), ExitCode::SUCCESS);
        assert_eq!(network_calls.load(Ordering::SeqCst), 1);
        assert!(bytes(&stdout_bytes).contains("data-injected-network"));
        assert!(bytes(&stderr_bytes).is_empty());
    }

    #[cfg(feature = "svg")]
    #[test]
    fn injected_publication_failure_is_operational_and_preserves_the_target() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("out.svg");
        let publication_calls = Arc::new(AtomicUsize::new(0));
        let (stdout, stdout_bytes) = capture();
        let (stderr, stderr_bytes) = capture();
        let app = CliApp {
            process: snapshot(
                &["render", "-", "--output", "out.svg"],
                Ok(directory.path().to_path_buf()),
                false,
            ),
            execution: execution_with_publication(
                Cursor::new(b"flowchart LR\nA-->B\n".to_vec()),
                stdout,
                stderr,
                Box::new(RejectingPublication {
                    calls: Arc::clone(&publication_calls),
                }),
            ),
        };

        assert_eq!(app.execute(), ExitCode::from(3));
        assert_eq!(publication_calls.load(Ordering::SeqCst), 1);
        assert!(!target.exists());
        assert!(bytes(&stdout_bytes).is_empty());
        assert!(bytes(&stderr_bytes).contains("injected publication failure"));
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn injected_batch_commit_failure_is_recovered_by_the_next_run() {
        let directory = tempfile::tempdir().unwrap();
        let commits = Arc::new(AtomicUsize::new(0));
        let source = b"# Document\n\n```mermaid\nflowchart LR\nA-->B\n```\n".to_vec();
        let args = [
            "batch",
            "-",
            "--stdin-file-name",
            "README.md",
            "--output-dir",
            "generated",
        ];
        let (stdout, stdout_bytes) = capture();
        let (stderr, stderr_bytes) = capture();
        let first = CliApp {
            process: snapshot(&args, Ok(directory.path().to_path_buf()), false),
            execution: execution_with_publication(
                Cursor::new(source.clone()),
                stdout,
                stderr,
                Box::new(RejectingCommitPublication {
                    system: crate::output::SystemPublicationBackend,
                    commits: Arc::clone(&commits),
                }),
            ),
        };

        assert_eq!(first.execute(), ExitCode::from(3));
        assert_eq!(commits.load(Ordering::SeqCst), 1);
        assert!(bytes(&stdout_bytes).is_empty());
        assert!(bytes(&stderr_bytes).contains("injected transaction commit failure"));
        assert!(!directory.path().join("generated/README.md").exists());
        assert!(
            directory
                .path()
                .join("generated/.merman.transaction")
                .is_dir()
        );

        let (stdout, stdout_bytes) = capture();
        let (stderr, stderr_bytes) = capture();
        let second = CliApp {
            process: snapshot(&args, Ok(directory.path().to_path_buf()), false),
            execution: execution(Cursor::new(source), stdout, stderr),
        };

        assert_eq!(second.execute(), ExitCode::SUCCESS);
        assert!(bytes(&stdout_bytes).is_empty());
        assert!(bytes(&stderr_bytes).contains("Found 1 mermaid charts"));
        assert!(directory.path().join("generated/README.md").is_file());
        assert!(
            !directory
                .path()
                .join("generated/.merman.transaction")
                .exists()
        );
    }
}
