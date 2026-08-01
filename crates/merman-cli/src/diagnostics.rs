use crate::error::CliError;
use crate::runtime::SharedWriter;
use serde::Serialize;
use std::io::Write;

#[derive(Clone)]
pub(crate) struct DiagnosticSink {
    quiet: bool,
    stderr: SharedWriter,
}

impl DiagnosticSink {
    pub(crate) fn new(quiet: bool, stderr: &SharedWriter) -> Self {
        Self {
            quiet,
            stderr: stderr.clone(),
        }
    }

    pub(crate) fn info(&self, message: impl std::fmt::Display) {
        if self.quiet {
            return;
        }
        self.stderr.with_writer(|stderr| {
            let _ = writeln!(stderr, "{message}");
        });
    }
}

pub(crate) fn write_json_stdout<T: Serialize>(
    value: &T,
    pretty: bool,
    stdout: &SharedWriter,
) -> Result<(), CliError> {
    stdout.with_writer(|writer| write_json_to(writer, value, pretty))
}

fn write_json_to(
    writer: &mut dyn Write,
    value: &impl Serialize,
    pretty: bool,
) -> Result<(), CliError> {
    if pretty {
        serde_json::to_writer_pretty(&mut *writer, value).map_err(CliError::json_output)?;
    } else {
        serde_json::to_writer(&mut *writer, value).map_err(CliError::json_output)?;
    }
    writer
        .write_all(b"\n")
        .map_err(|source| CliError::stream("stdout", source))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::ser::SerializeSeq;
    use std::cell::Cell;
    use std::io;

    #[test]
    fn quiet_sink_suppresses_messages() {
        let written = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let bytes = SharedWriter::new(TestBuffer(std::sync::Arc::clone(&written)));
        DiagnosticSink::new(true, &bytes).info("hidden");
        assert!(written.lock().unwrap().is_empty());
    }

    #[test]
    fn active_sink_writes_one_line() {
        let written = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let bytes = SharedWriter::new(TestBuffer(std::sync::Arc::clone(&written)));
        DiagnosticSink::new(false, &bytes).info("visible");
        assert_eq!(*written.lock().unwrap(), b"visible\n");
    }

    #[test]
    fn json_serialization_stops_when_the_destination_closes() {
        struct Probe<'a>(&'a Cell<usize>);

        impl Serialize for Probe<'_> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let mut sequence = serializer.serialize_seq(Some(10_000))?;
                for value in 0..10_000_u32 {
                    self.0.set(self.0.get() + 1);
                    sequence.serialize_element(&value)?;
                }
                sequence.end()
            }
        }

        struct ClosedWriter {
            writes_left: usize,
        }

        impl Write for ClosedWriter {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if self.writes_left == 0 {
                    return Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"));
                }
                self.writes_left -= 1;
                Ok(bytes.len())
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let serialized = Cell::new(0);
        let error = write_json_to(
            &mut ClosedWriter { writes_left: 8 },
            &Probe(&serialized),
            false,
        )
        .unwrap_err();

        assert!(error.is_broken_stdout_pipe());
        assert!(
            serialized.get() < 10_000,
            "serialization must stop instead of materializing a complete second string"
        );
    }

    struct TestBuffer(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl Write for TestBuffer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}
