use crate::error::CliError;
use crate::input::{InputLimit, InputReadError, read_utf8, read_utf8_controlled};
use crate::runtime::SharedWriter;
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[cfg(feature = "analysis")]
pub(crate) struct AcquiredFixSource {
    text: String,
    snapshot: Option<FixSourceSnapshot>,
}

#[cfg(feature = "analysis")]
struct FixSourceSnapshot {
    identity: same_file::Handle,
}

#[cfg(feature = "analysis")]
impl AcquiredFixSource {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn verify_unchanged(&self, path: &Path) -> Result<(), CliError> {
        use crate::error::FileOperation;

        let snapshot = self.snapshot.as_ref().ok_or_else(|| {
            CliError::InvalidInput("--write requires a file input, not stdin".to_string())
        })?;
        let target_metadata =
            std::fs::symlink_metadata(path).map_err(|source| snapshot_io_error(path, source))?;
        if target_metadata.file_type().is_symlink() {
            return Err(concurrent_change(path, "target became a symbolic link"));
        }
        if !target_metadata.file_type().is_file() {
            return Err(concurrent_change(
                path,
                "target is no longer a regular file",
            ));
        }

        let mut file = File::open(path).map_err(|source| snapshot_io_error(path, source))?;
        let metadata = file
            .metadata()
            .map_err(|source| CliError::file(FileOperation::VerifySourceSnapshot, path, source))?;
        if !metadata.file_type().is_file() {
            return Err(concurrent_change(
                path,
                "target is no longer a regular file",
            ));
        }
        let opened_identity =
            same_file::Handle::from_file(file.try_clone().map_err(|source| {
                CliError::file(FileOperation::VerifySourceSnapshot, path, source)
            })?)
            .map_err(|source| CliError::file(FileOperation::VerifySourceSnapshot, path, source))?;
        if opened_identity != snapshot.identity {
            return Err(concurrent_change(
                path,
                "file identity changed after acquisition",
            ));
        }

        let expected = self.text.as_bytes();
        let comparison_limit = expected.len().saturating_add(1);
        let mut offset = 0_usize;
        let mut buffer = [0_u8; 8 * 1024];
        loop {
            let remaining = comparison_limit.saturating_sub(offset);
            if remaining == 0 {
                return Err(concurrent_change(
                    path,
                    "complete source contents changed after acquisition",
                ));
            }
            let read_len = remaining.min(buffer.len());
            let read = loop {
                match file.read(&mut buffer[..read_len]) {
                    Ok(read) => break read,
                    Err(source) if source.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(source) => {
                        return Err(CliError::file(
                            FileOperation::VerifySourceSnapshot,
                            path,
                            source,
                        ));
                    }
                }
            };
            if read == 0 {
                break;
            }
            let Some(expected_end) = offset.checked_add(read) else {
                return Err(concurrent_change(
                    path,
                    "source length overflowed during snapshot verification",
                ));
            };
            if expected_end > expected.len() || buffer[..read] != expected[offset..expected_end] {
                return Err(concurrent_change(
                    path,
                    "complete source contents changed after acquisition",
                ));
            }
            offset = expected_end;
        }
        if offset != expected.len() {
            return Err(concurrent_change(
                path,
                "complete source contents changed after acquisition",
            ));
        }
        let current_identity =
            same_file::Handle::from_path(path).map_err(|source| snapshot_io_error(path, source))?;
        if current_identity != opened_identity {
            return Err(concurrent_change(
                path,
                "file identity changed during snapshot verification",
            ));
        }
        Ok(())
    }
}

#[cfg(feature = "analysis")]
fn concurrent_change(path: &Path, reason: impl Into<String>) -> CliError {
    CliError::ConcurrentModification {
        path: path.to_path_buf(),
        reason: reason.into(),
    }
}

#[cfg(feature = "analysis")]
fn snapshot_io_error(path: &Path, source: std::io::Error) -> CliError {
    if source.kind() == std::io::ErrorKind::NotFound {
        concurrent_change(path, "target disappeared after acquisition")
    } else {
        CliError::file(
            crate::error::FileOperation::VerifySourceSnapshot,
            path,
            source,
        )
    }
}

pub(crate) fn read_input(
    path: Option<&Path>,
    quiet: bool,
    limit: InputLimit,
    stdin: &mut dyn Read,
    stderr: &SharedWriter,
) -> Result<String, CliError> {
    read_primary_input(path, quiet, limit, stdin, stderr).map_err(CliError::primary_input)
}

#[cfg(any(feature = "svg", feature = "ascii"))]
pub(crate) fn read_input_controlled(
    path: Option<&Path>,
    quiet: bool,
    limit: InputLimit,
    stdin: &mut dyn Read,
    stderr: &SharedWriter,
    control: &merman::OperationControl,
) -> Result<String, CliError> {
    crate::operation::checkpoint(control, merman::OperationPhase::Admission)?;
    read_primary_input_impl(path, quiet, limit, stdin, stderr, Some(control))
        .map_err(controlled_primary_input_error)
}

pub(crate) fn read_primary_input(
    path: Option<&Path>,
    quiet: bool,
    limit: InputLimit,
    stdin: &mut dyn Read,
    stderr: &SharedWriter,
) -> Result<String, InputReadError> {
    read_primary_input_impl(path, quiet, limit, stdin, stderr, None)
}

fn read_primary_input_impl(
    path: Option<&Path>,
    quiet: bool,
    limit: InputLimit,
    stdin: &mut dyn Read,
    stderr: &SharedWriter,
    control: Option<&merman::OperationControl>,
) -> Result<String, InputReadError> {
    match path {
        None => {
            crate::diagnostics::DiagnosticSink::new(quiet, stderr).info(
                "No input file specified, reading from stdin. Use -i <input> to suppress this warning.",
            );
            read_utf8_with_control(stdin, "stdin", limit, None, control)
        }
        Some(path) if path == Path::new("-") => {
            read_utf8_with_control(stdin, "stdin", limit, None, control)
        }
        Some(path) => {
            let resource = format!("Input file {}", crate::error::safe_path(path));
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
            read_utf8_with_control(file, resource, limit, Some(metadata.len()), control)
        }
    }
}

#[cfg(feature = "analysis")]
pub(crate) fn read_fix_source(
    input: &crate::invocation::ResolvedInput,
    limit: InputLimit,
    stdin: &mut dyn Read,
) -> Result<AcquiredFixSource, InputReadError> {
    let Some(path) = input.file() else {
        let text = read_utf8(stdin, "stdin", limit, None)?;
        return Ok(AcquiredFixSource {
            text,
            snapshot: None,
        });
    };
    let resource = format!("Input file {}", crate::error::safe_path(path));
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
    let identity =
        same_file::Handle::from_file(file.try_clone().map_err(|source| InputReadError::Io {
            resource: resource.clone(),
            source,
        })?)
        .map_err(|source| InputReadError::Io {
            resource: resource.clone(),
            source,
        })?;
    let text = read_utf8(file, resource, limit, Some(metadata.len()))?;
    Ok(AcquiredFixSource {
        text,
        snapshot: Some(FixSourceSnapshot { identity }),
    })
}

#[cfg(feature = "svg")]
pub(crate) fn read_optional_text_file_controlled(
    path: Option<&Path>,
    label: &str,
    limit: InputLimit,
    control: &merman::OperationControl,
) -> Result<Option<String>, CliError> {
    path.map(|path| read_named_text_file_controlled(path, label, limit, control))
        .transpose()
}

pub(crate) fn read_named_text_file(
    path: impl AsRef<Path>,
    label: &str,
    limit: InputLimit,
) -> Result<String, CliError> {
    let (file, resource, length_hint) = open_named_regular_file(path.as_ref(), label)?;
    read_utf8(file, resource, limit, Some(length_hint)).map_err(CliError::auxiliary_input)
}

#[cfg(any(feature = "svg", feature = "ascii"))]
pub(crate) fn read_named_text_file_controlled(
    path: impl AsRef<Path>,
    label: &str,
    limit: InputLimit,
    control: &merman::OperationControl,
) -> Result<String, CliError> {
    crate::operation::checkpoint(control, merman::OperationPhase::Admission)?;
    let (file, resource, length_hint) = open_named_regular_file(path.as_ref(), label)?;
    read_utf8_controlled(file, resource, limit, Some(length_hint), control)
        .map_err(controlled_auxiliary_input_error)
}

#[cfg(feature = "icons")]
pub(crate) fn read_named_bytes_file_controlled(
    path: impl AsRef<Path>,
    label: &str,
    limit: InputLimit,
    control: &merman::OperationControl,
) -> Result<Vec<u8>, CliError> {
    crate::operation::checkpoint(control, merman::OperationPhase::Admission)?;
    let (file, resource, length_hint) = open_named_regular_file(path.as_ref(), label)?;
    crate::input::read_bytes_with_limit_controlled(
        file,
        resource,
        limit,
        Some(length_hint),
        control,
    )
    .map_err(controlled_auxiliary_input_error)
}

fn open_named_regular_file(path: &Path, label: &str) -> Result<(File, String, u64), CliError> {
    let resource = format!("{label} {}", crate::error::safe_path(path));
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
    Ok((file, resource, metadata.len()))
}

fn read_utf8_with_control(
    reader: impl Read,
    resource: impl Into<String>,
    limit: InputLimit,
    length_hint: Option<u64>,
    control: Option<&merman::OperationControl>,
) -> Result<String, InputReadError> {
    match control {
        Some(control) => read_utf8_controlled(reader, resource, limit, length_hint, control),
        None => read_utf8(reader, resource, limit, length_hint),
    }
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn controlled_primary_input_error(error: InputReadError) -> CliError {
    match error {
        InputReadError::Cancelled(cancelled) => {
            CliError::Render(merman::RenderError::Cancelled(cancelled))
        }
        error => CliError::primary_input(error),
    }
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn controlled_auxiliary_input_error(error: InputReadError) -> CliError {
    match error {
        InputReadError::Cancelled(cancelled) => {
            CliError::Render(merman::RenderError::Cancelled(cancelled))
        }
        error => CliError::auxiliary_input(error),
    }
}

#[cfg(any(feature = "svg", feature = "ascii"))]
pub(crate) fn write_output(
    target: &crate::invocation::ResolvedDestination,
    bytes: &[u8],
    control: &merman::OperationControl,
    publications: &crate::output::PublicationGuards,
    stdout: &SharedWriter,
    publication: &mut dyn crate::output::PublicationBackend,
) -> Result<(), CliError> {
    crate::operation::checkpoint(control, merman::OperationPhase::Emit)?;
    match target {
        crate::invocation::ResolvedDestination::Stdout => {
            write_stdout_controlled(bytes, stdout, control)?;
        }
        crate::invocation::ResolvedDestination::File(path) => {
            publication.publish_file_controlled(path, bytes, publications, control)?;
        }
    }
    Ok(())
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn write_stdout_controlled(
    bytes: &[u8],
    stdout: &SharedWriter,
    control: &merman::OperationControl,
) -> Result<(), CliError> {
    stdout.with_writer(|writer| {
        // Stdout cannot be rolled back. This checkpoint is the publication commit point;
        // cancellation requested after it must not turn a complete or partial write into
        // a cancellation failure.
        crate::operation::checkpoint(control, merman::OperationPhase::Emit)?;
        writer.write_all(bytes).map_err(stdout_error)
    })
}

pub(crate) fn write_stdout(bytes: &[u8], stdout: &SharedWriter) -> Result<(), CliError> {
    write_stdout_bytes(stdout, bytes)
}

pub(crate) fn write_stdout_line(line: &str, stdout: &SharedWriter) -> Result<(), CliError> {
    write_stdout_bytes(stdout, line.as_bytes())?;
    write_stdout_bytes(stdout, b"\n")?;
    Ok(())
}

fn write_stdout_bytes(stdout: &SharedWriter, bytes: &[u8]) -> Result<(), CliError> {
    stdout.write_all(bytes).map_err(stdout_error)
}

fn stdout_error(error: std::io::Error) -> CliError {
    if error.kind() == std::io::ErrorKind::BrokenPipe {
        CliError::BrokenStdoutPipe
    } else {
        CliError::stream("stdout", error)
    }
}

#[cfg(feature = "analysis")]
pub(crate) fn write_file(
    path: &Path,
    bytes: &[u8],
    publications: &crate::output::PublicationGuards,
    publication: &mut dyn crate::output::PublicationBackend,
) -> Result<(), CliError> {
    publication.publish_file_verified(path, bytes, publications, &mut |_| Ok(()))
}

#[cfg(feature = "analysis")]
pub(crate) fn write_file_verified(
    path: &Path,
    bytes: &[u8],
    publications: &crate::output::PublicationGuards,
    publication: &mut dyn crate::output::PublicationBackend,
    verify: &mut dyn FnMut(&Path) -> Result<(), CliError>,
) -> Result<(), CliError> {
    publication.publish_file_verified(path, bytes, publications, verify)
}

#[cfg(all(test, feature = "analysis"))]
mod tests {
    use super::*;
    use crate::invocation::ResolvedInput;

    fn acquire(path: &Path) -> AcquiredFixSource {
        let mut stdin = std::io::empty();
        read_fix_source(
            &ResolvedInput::File(path.to_path_buf()),
            InputLimit::new("max_source_bytes", Some(1024)),
            &mut stdin,
        )
        .expect("acquire source")
    }

    #[test]
    fn acquired_snapshot_accepts_unchanged_contents_and_identity() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.mmd");
        std::fs::write(&path, b"flowchart\nA-->B\n").unwrap();
        let snapshot = acquire(&path);

        snapshot.verify_unchanged(&path).expect("unchanged");
    }

    #[test]
    fn acquired_snapshot_rejects_same_inode_content_changes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.mmd");
        std::fs::write(&path, b"flowchart\nA-->B\n").unwrap();
        let snapshot = acquire(&path);
        std::fs::write(&path, b"flowchart\nB-->A\n").unwrap();

        assert!(matches!(
            snapshot.verify_unchanged(&path),
            Err(CliError::ConcurrentModification { .. })
        ));
        assert_eq!(std::fs::read(&path).unwrap(), b"flowchart\nB-->A\n");
    }

    #[test]
    fn acquired_snapshot_rejects_renamed_and_relinked_targets() {
        let directory = tempfile::tempdir().unwrap();
        let renamed = directory.path().join("renamed.mmd");
        let path = directory.path().join("source.mmd");
        std::fs::write(&path, b"flowchart\nA-->B\n").unwrap();
        let snapshot = acquire(&path);
        std::fs::rename(&path, &renamed).unwrap();
        std::fs::write(&path, b"newer path contents").unwrap();

        assert!(matches!(
            snapshot.verify_unchanged(&path),
            Err(CliError::ConcurrentModification { .. })
        ));
        assert_eq!(std::fs::read(&path).unwrap(), b"newer path contents");

        let relinked = directory.path().join("relinked.mmd");
        std::fs::write(&relinked, b"linked replacement").unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::hard_link(&relinked, &path).unwrap();
        assert!(matches!(
            snapshot.verify_unchanged(&path),
            Err(CliError::ConcurrentModification { .. })
        ));
        assert_eq!(std::fs::read(&path).unwrap(), b"linked replacement");
    }
}

#[cfg(all(test, any(feature = "svg", feature = "ascii")))]
mod controlled_stdout_tests {
    use super::*;
    use std::io;
    use std::sync::{Arc, Mutex};

    struct CancelAfterWrite {
        bytes: Arc<Mutex<Vec<u8>>>,
        control: merman::OperationControl,
    }

    impl io::Write for CancelAfterWrite {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.bytes
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .extend_from_slice(bytes);
            self.control.cancel();
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn stdout_cancellation_before_publication_writes_nothing() {
        let control = merman::OperationControl::new();
        control.cancel();
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let stdout = SharedWriter::new(CancelAfterWrite {
            bytes: Arc::clone(&bytes),
            control: control.clone(),
        });

        assert!(matches!(
            write_stdout_controlled(b"complete artifact", &stdout, &control),
            Err(CliError::Render(merman::RenderError::Cancelled(
                merman::OperationCancelled {
                    phase: merman::OperationPhase::Emit,
                    reason: merman::CancelReason::Requested,
                }
            )))
        ));
        assert!(
            bytes
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_empty()
        );
    }

    #[test]
    fn stdout_cancellation_after_publication_commit_returns_complete_artifact() {
        let control = merman::OperationControl::new();
        let written = Arc::new(Mutex::new(Vec::new()));
        let stdout = SharedWriter::new(CancelAfterWrite {
            bytes: Arc::clone(&written),
            control: control.clone(),
        });
        let artifact = vec![b'x'; crate::input::IO_CHUNK_BYTES + 1];

        write_stdout_controlled(&artifact, &stdout, &control)
            .expect("publication remains successful after its commit point");

        assert!(control.is_cancelled());
        assert_eq!(
            *written
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            artifact
        );
    }
}
