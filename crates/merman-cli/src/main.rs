use futures::executor::block_on;
use merman::{Engine, ParseOptions};
use serde::Serialize;
use serde_json::Value;
use std::io::Read;

#[derive(Debug)]
enum CliError {
    Usage(&'static str),
    Io(std::io::Error),
    Mermaid(merman::Error),
    Json(serde_json::Error),
    NoDiagram,
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::Usage(msg) => write!(f, "{msg}"),
            CliError::Io(err) => write!(f, "I/O error: {err}"),
            CliError::Mermaid(err) => write!(f, "{err}"),
            CliError::Json(err) => write!(f, "JSON error: {err}"),
            CliError::NoDiagram => write!(f, "No Mermaid diagram detected"),
        }
    }
}

impl From<std::io::Error> for CliError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<merman::Error> for CliError {
    fn from(value: merman::Error) -> Self {
        Self::Mermaid(value)
    }
}

impl From<serde_json::Error> for CliError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Debug, Clone, Copy, Default)]
enum Command {
    #[default]
    Parse,
    Detect,
}

#[derive(Debug, Default)]
struct Args {
    command: Command,
    input: Option<String>,
    pretty: bool,
    with_meta: bool,
    suppress_errors: bool,
}

#[derive(Serialize)]
struct MetaOut<'a> {
    diagram_type: &'a str,
    config: &'a Value,
    effective_config: &'a Value,
    title: Option<&'a str>,
}

#[derive(Serialize)]
struct ParseOut<'a> {
    meta: MetaOut<'a>,
    model: &'a Value,
}

fn usage() -> &'static str {
    "merman-cli\n\
\n\
USAGE:\n\
  merman-cli [parse] [--pretty] [--meta] [--suppress-errors] [<path>|-]\n\
  merman-cli detect [<path>|-]\n\
\n\
NOTES:\n\
  - If <path> is omitted or '-', input is read from stdin.\n\
  - parse prints the semantic JSON model by default; --meta wraps it with parse metadata.\n\
"
}

fn parse_args(argv: &[String]) -> Result<Args, CliError> {
    let mut args = Args {
        command: Command::Parse,
        ..Default::default()
    };

    let mut it = argv.iter().skip(1).peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--help" | "-h" => return Err(CliError::Usage(usage())),
            "parse" => args.command = Command::Parse,
            "detect" => args.command = Command::Detect,
            "--pretty" => args.pretty = true,
            "--meta" => args.with_meta = true,
            "--suppress-errors" => args.suppress_errors = true,
            "--" => {
                if let Some(rest) = it.next() {
                    if args.input.is_some() {
                        return Err(CliError::Usage(usage()));
                    }
                    args.input = Some(rest.clone());
                }
                while it.next().is_some() {
                    return Err(CliError::Usage(usage()));
                }
            }
            other if other.starts_with('-') => return Err(CliError::Usage(usage())),
            path => {
                if args.input.is_some() {
                    return Err(CliError::Usage(usage()));
                }
                args.input = Some(path.to_string());
            }
        }
    }

    Ok(args)
}

fn read_input(input: Option<&str>) -> Result<String, CliError> {
    match input {
        None | Some("-") => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
        Some(path) => Ok(std::fs::read_to_string(path)?),
    }
}

fn write_json(value: &impl Serialize, pretty: bool) -> Result<(), CliError> {
    if pretty {
        serde_json::to_writer_pretty(std::io::stdout().lock(), value)?;
    } else {
        serde_json::to_writer(std::io::stdout().lock(), value)?;
    }
    Ok(())
}

fn run(args: Args) -> Result<(), CliError> {
    let text = read_input(args.input.as_deref())?;
    let engine = Engine::new();
    let options = ParseOptions {
        suppress_errors: args.suppress_errors,
    };

    match args.command {
        Command::Detect => {
            let Some(meta) = block_on(engine.parse_metadata(&text, options))? else {
                return Err(CliError::NoDiagram);
            };
            println!("{}", meta.diagram_type);
            Ok(())
        }
        Command::Parse => {
            let Some(parsed) = block_on(engine.parse_diagram(&text, options))? else {
                return Err(CliError::NoDiagram);
            };

            if args.with_meta {
                let out = ParseOut {
                    meta: MetaOut {
                        diagram_type: &parsed.meta.diagram_type,
                        config: parsed.meta.config.as_value(),
                        effective_config: parsed.meta.effective_config.as_value(),
                        title: parsed.meta.title.as_deref(),
                    },
                    model: &parsed.model,
                };
                write_json(&out, args.pretty)?;
            } else {
                write_json(&parsed.model, args.pretty)?;
            }
            Ok(())
        }
    }
}

fn main() {
    let args = match parse_args(&std::env::args().collect::<Vec<_>>()) {
        Ok(v) => v,
        Err(CliError::Usage(msg)) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };

    match run(args) {
        Ok(()) => {}
        Err(CliError::NoDiagram) => {
            eprintln!("{}", CliError::NoDiagram);
            std::process::exit(3);
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}
