use crate::error::CliError;
use serde::Serialize;
use std::io::{self, Write};

#[derive(Debug, Clone, Copy)]
pub(crate) struct DiagnosticSink {
    quiet: bool,
}

impl DiagnosticSink {
    pub(crate) const fn new(quiet: bool) -> Self {
        Self { quiet }
    }

    pub(crate) fn info(&self, message: impl std::fmt::Display) {
        if self.quiet {
            return;
        }
        let mut stderr = io::stderr().lock();
        let _ = writeln!(stderr, "{message}");
    }
}

pub(crate) fn write_json_stdout<T: Serialize>(value: &T, pretty: bool) -> Result<(), CliError> {
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_json_to(&mut writer, value, pretty)
}

fn write_json_to(
    writer: &mut impl Write,
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

    #[test]
    fn quiet_sink_suppresses_messages() {
        let mut bytes = Vec::new();
        write_info_to(&mut bytes, DiagnosticSink::new(true), "hidden").unwrap();
        assert!(bytes.is_empty());
    }

    #[test]
    fn active_sink_writes_one_line() {
        let mut bytes = Vec::new();
        write_info_to(&mut bytes, DiagnosticSink::new(false), "visible").unwrap();
        assert_eq!(bytes, b"visible\n");
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

    fn write_info_to(
        writer: &mut impl Write,
        sink: DiagnosticSink,
        message: &str,
    ) -> io::Result<()> {
        if sink.quiet {
            return Ok(());
        }
        writeln!(writer, "{message}")
    }
}
