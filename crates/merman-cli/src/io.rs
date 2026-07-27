use crate::error::CliError;
use std::io::{Read, Write};
use std::path::Path;
#[cfg(any(feature = "svg", feature = "ascii"))]
use std::path::PathBuf;

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Debug, Clone)]
pub(crate) enum OutputTarget {
    Stdout,
    File(PathBuf),
}

pub(crate) fn read_input(path: Option<&Path>, quiet: bool) -> Result<String, CliError> {
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
        Some(path) if path == Path::new("-") => {
            std::io::stdin().read_to_string(&mut buf)?;
        }
        Some(path) => {
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

#[cfg(feature = "svg")]
pub(crate) fn read_optional_text_file(
    path: Option<&Path>,
    label: &str,
) -> Result<Option<String>, CliError> {
    path.map(|p| read_named_text_file(p, label)).transpose()
}

pub(crate) fn read_named_text_file(
    path: impl AsRef<Path>,
    label: &str,
) -> Result<String, CliError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(CliError::InvalidInput(format!(
            "{label} \"{}\" does not exist",
            path.display()
        )));
    }
    Ok(std::fs::read_to_string(path)?)
}

#[cfg(any(feature = "svg", feature = "ascii"))]
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
    let mut stdout = std::io::stdout();
    write_stdout_bytes(&mut stdout, bytes)
}

pub(crate) fn write_stdout_line(line: &str) -> Result<(), CliError> {
    let mut stdout = std::io::stdout();
    write_stdout_bytes(&mut stdout, line.as_bytes())?;
    write_stdout_bytes(&mut stdout, b"\n")?;
    Ok(())
}

fn write_stdout_bytes(stdout: &mut impl Write, bytes: &[u8]) -> Result<(), CliError> {
    stdout.write_all(bytes).map_err(|err| {
        if err.kind() == std::io::ErrorKind::BrokenPipe {
            CliError::BrokenStdoutPipe
        } else {
            CliError::Io(err)
        }
    })
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
pub(crate) fn write_file(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    ensure_output_dir(path)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
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
