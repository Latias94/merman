use crate::error::CliError;
use std::io::{Read, Write as _};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) enum OutputTarget {
    Stdout,
    File(PathBuf),
}

impl OutputTarget {
    pub(crate) fn from_cli(raw: String) -> Self {
        if raw == "-" {
            Self::Stdout
        } else {
            Self::File(PathBuf::from(raw))
        }
    }
}

pub(crate) fn read_input(path: Option<&str>, quiet: bool) -> Result<String, CliError> {
    let mut buf = String::new();
    match path {
        None => {
            if !quiet {
                eprintln!(
                    "No input file specified, reading from stdin. Use -i <input> to suppress this warning."
                );
            }
            std::io::stdin().read_to_string(&mut buf)?;
        }
        Some("-") => {
            std::io::stdin().read_to_string(&mut buf)?;
        }
        Some(p) => {
            let path = Path::new(p);
            if !path.exists() {
                return Err(CliError::InvalidInput(format!(
                    "Input file \"{}\" doesn't exist",
                    path.display()
                )));
            }
            std::fs::File::open(path)?.read_to_string(&mut buf)?;
        }
    }
    Ok(buf)
}

pub(crate) fn read_optional_text_file(
    path: Option<&str>,
    label: &str,
) -> Result<Option<String>, CliError> {
    path.map(|p| read_named_text_file(p, label)).transpose()
}

pub(crate) fn read_named_text_file(path: &str, label: &str) -> Result<String, CliError> {
    let path_ref = Path::new(path);
    if !path_ref.exists() {
        return Err(CliError::InvalidInput(format!(
            "{label} \"{}\" does not exist",
            path_ref.display()
        )));
    }
    Ok(std::fs::read_to_string(path_ref)?)
}

pub(crate) fn write_output(target: Option<&OutputTarget>, bytes: &[u8]) -> Result<(), CliError> {
    match target {
        None | Some(OutputTarget::Stdout) => {
            write_stdout(bytes)?;
        }
        Some(OutputTarget::File(path)) => {
            write_file(path, bytes)?;
        }
    }
    Ok(())
}

pub(crate) fn write_stdout(bytes: &[u8]) -> Result<(), CliError> {
    std::io::stdout().write_all(bytes)?;
    Ok(())
}

pub(crate) fn write_stdout_line(line: &str) -> Result<(), CliError> {
    let mut stdout = std::io::stdout();
    stdout.write_all(line.as_bytes())?;
    stdout.write_all(b"\n")?;
    Ok(())
}

pub(crate) fn write_file(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    ensure_output_dir(path)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

fn ensure_output_dir(path: &Path) -> Result<(), CliError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() || parent.exists() {
        return Ok(());
    }
    Err(CliError::InvalidOutput(format!(
        "Output directory \"{}\" does not exist",
        parent.display()
    )))
}
