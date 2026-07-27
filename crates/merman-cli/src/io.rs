use crate::error::CliError;
use crate::input::{InputLimit, InputReadError, read_utf8};
use std::fs::File;
use std::io::Write;
use std::path::Path;
#[cfg(any(feature = "svg", feature = "ascii"))]
use std::path::PathBuf;

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Debug, Clone)]
pub(crate) enum OutputTarget {
    Stdout,
    File(PathBuf),
}

pub(crate) fn read_input(
    path: Option<&Path>,
    quiet: bool,
    limit: InputLimit,
) -> Result<String, CliError> {
    read_primary_input(path, quiet, limit).map_err(CliError::primary_input)
}

pub(crate) fn read_primary_input(
    path: Option<&Path>,
    quiet: bool,
    limit: InputLimit,
) -> Result<String, InputReadError> {
    match path {
        None => {
            if !quiet {
                eprintln!(
                    "No input file specified, reading from stdin. Use -i <input> to suppress this warning."
                );
            }
            read_utf8(std::io::stdin().lock(), "stdin", limit, None)
        }
        Some(path) if path == Path::new("-") => {
            read_utf8(std::io::stdin().lock(), "stdin", limit, None)
        }
        Some(path) => {
            let resource = format!("Input file \"{}\"", path.display());
            let file = File::open(path).map_err(|source| {
                if source.kind() == std::io::ErrorKind::NotFound {
                    InputReadError::NotFound {
                        resource: resource.clone(),
                    }
                } else {
                    InputReadError::Io {
                        resource: resource.clone(),
                        source,
                    }
                }
            })?;
            let metadata = file.metadata().map_err(|source| InputReadError::Io {
                resource: resource.clone(),
                source,
            })?;
            if !metadata.file_type().is_file() {
                return Err(InputReadError::NotRegularFile { resource });
            }
            read_utf8(file, resource, limit, Some(metadata.len()))
        }
    }
}

#[cfg(feature = "svg")]
pub(crate) fn read_optional_text_file(
    path: Option<&Path>,
    label: &str,
    limit: InputLimit,
) -> Result<Option<String>, CliError> {
    path.map(|p| read_named_text_file(p, label, limit))
        .transpose()
}

pub(crate) fn read_named_text_file(
    path: impl AsRef<Path>,
    label: &str,
    limit: InputLimit,
) -> Result<String, CliError> {
    let path = path.as_ref();
    let resource = format!("{label} \"{}\"", path.display());
    let file = File::open(path).map_err(|source| {
        CliError::auxiliary_input(if source.kind() == std::io::ErrorKind::NotFound {
            InputReadError::NotFound {
                resource: resource.clone(),
            }
        } else {
            InputReadError::Io {
                resource: resource.clone(),
                source,
            }
        })
    })?;
    let metadata = file.metadata().map_err(|source| {
        CliError::auxiliary_input(InputReadError::Io {
            resource: resource.clone(),
            source,
        })
    })?;
    if !metadata.file_type().is_file() {
        return Err(CliError::auxiliary_input(InputReadError::NotRegularFile {
            resource,
        }));
    }
    read_utf8(file, resource, limit, Some(metadata.len())).map_err(CliError::auxiliary_input)
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
