use std::io::{self, Read};

const READ_CHUNK_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InputLimit {
    pub(crate) stable_id: &'static str,
    pub(crate) max_bytes: Option<usize>,
}

impl InputLimit {
    pub(crate) const fn new(stable_id: &'static str, max_bytes: Option<usize>) -> Self {
        Self {
            stable_id,
            max_bytes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObservedSize {
    Exact(u64),
    AtLeast(u64),
}

impl std::fmt::Display for ObservedSize {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exact(bytes) => write!(formatter, "{bytes}"),
            Self::AtLeast(bytes) => write!(formatter, "at least {bytes}"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum InputReadError {
    #[error("{resource} doesn't exist")]
    NotFound { resource: String },
    #[error("{resource} is not a regular file")]
    NotRegularFile { resource: String },
    #[error("failed to read {resource}: {source}")]
    Io {
        resource: String,
        #[source]
        source: io::Error,
    },
    #[error("{resource} is {actual} bytes, exceeding the {limit}-byte limit ({limit_id})")]
    LimitExceeded {
        resource: String,
        actual: ObservedSize,
        limit: usize,
        limit_id: &'static str,
    },
    #[error("{resource} is not valid UTF-8 (invalid byte sequence at offset {valid_up_to})")]
    InvalidUtf8 {
        resource: String,
        valid_up_to: usize,
        error_len: Option<usize>,
    },
    #[error("failed to allocate memory while reading {resource}: {source}")]
    Allocation {
        resource: String,
        #[source]
        source: std::collections::TryReserveError,
    },
}

impl InputReadError {
    pub(crate) const fn is_operational(&self) -> bool {
        matches!(
            self,
            Self::NotRegularFile { .. } | Self::Io { .. } | Self::Allocation { .. }
        )
    }
}

/// Reads a resource without retaining more than `limit + 1` bytes.
///
/// `length_hint` is used only for an early limit check. It is deliberately not
/// used to reserve capacity because file metadata and HTTP `Content-Length`
/// values can be stale or controlled by an untrusted peer.
#[cfg(test)]
pub(crate) fn read_bytes(
    reader: impl Read,
    resource: impl Into<String>,
    limit: Option<usize>,
    length_hint: Option<u64>,
) -> Result<Vec<u8>, InputReadError> {
    let resource = resource.into();
    read_bytes_impl(
        reader,
        &resource,
        InputLimit::new("test_byte_limit", limit),
        length_hint,
    )
}

pub(crate) fn read_utf8(
    reader: impl Read,
    resource: impl Into<String>,
    limit: InputLimit,
    length_hint: Option<u64>,
) -> Result<String, InputReadError> {
    let resource = resource.into();
    let bytes = read_bytes_impl(reader, &resource, limit, length_hint)?;
    String::from_utf8(bytes).map_err(|error| {
        let utf8_error = error.utf8_error();
        InputReadError::InvalidUtf8 {
            resource,
            valid_up_to: utf8_error.valid_up_to(),
            error_len: utf8_error.error_len(),
        }
    })
}

fn read_bytes_impl(
    mut reader: impl Read,
    resource: &str,
    limit: InputLimit,
    length_hint: Option<u64>,
) -> Result<Vec<u8>, InputReadError> {
    if let (Some(max_bytes), Some(length_hint)) = (limit.max_bytes, length_hint)
        && u128::from(length_hint) > max_bytes as u128
    {
        return Err(InputReadError::LimitExceeded {
            resource: resource.to_owned(),
            actual: ObservedSize::Exact(length_hint),
            limit: max_bytes,
            limit_id: limit.stable_id,
        });
    }

    let read_ceiling = limit
        .max_bytes
        .and_then(|max_bytes| max_bytes.checked_add(1));
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; READ_CHUNK_BYTES];

    loop {
        let requested = match read_ceiling {
            Some(ceiling) => {
                let remaining = ceiling - bytes.len();
                if remaining == 0 {
                    break;
                }
                remaining.min(chunk.len())
            }
            None => chunk.len(),
        };

        let count = match reader.read(&mut chunk[..requested]) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(source) => {
                return Err(InputReadError::Io {
                    resource: resource.to_owned(),
                    source,
                });
            }
        };
        bytes
            .try_reserve(count)
            .map_err(|source| InputReadError::Allocation {
                resource: resource.to_owned(),
                source,
            })?;
        bytes.extend_from_slice(&chunk[..count]);
    }

    if let Some(max_bytes) = limit.max_bytes
        && bytes.len() > max_bytes
    {
        return Err(InputReadError::LimitExceeded {
            resource: resource.to_owned(),
            actual: ObservedSize::AtLeast(bytes.len() as u64),
            limit: max_bytes,
            limit_id: limit.stable_id,
        });
    }

    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::{InputLimit, InputReadError, ObservedSize, read_bytes, read_utf8};
    use std::io::{self, Cursor, Read};

    #[test]
    fn accepts_input_at_the_exact_limit() {
        let bytes = read_bytes(Cursor::new(b"1234"), "source", Some(4), Some(4))
            .expect("input at the limit should be accepted");

        assert_eq!(bytes, b"1234");
    }

    #[test]
    fn rejects_input_one_byte_over_the_limit() {
        let error = read_bytes(Cursor::new(b"12345"), "source", Some(4), None)
            .expect_err("input over the limit should be rejected");

        assert!(matches!(
            error,
            InputReadError::LimitExceeded {
                resource,
                actual: ObservedSize::AtLeast(5),
                limit: 4,
                ..
            } if resource == "source"
        ));
    }

    #[test]
    fn reports_invalid_utf8_without_retaining_the_input() {
        let error = read_utf8(
            Cursor::new([b'a', 0xff, b'b']),
            "config",
            InputLimit::new("max_config_bytes", None),
            None,
        )
        .expect_err("invalid UTF-8 should be rejected");

        assert!(matches!(
            error,
            InputReadError::InvalidUtf8 {
                resource,
                valid_up_to: 1,
                error_len: Some(1),
            } if resource == "config"
        ));
    }

    #[test]
    fn handles_readers_that_return_short_reads() {
        let reader = ShortReader {
            bytes: b"diagram".to_vec(),
            offset: 0,
        };

        let text = read_utf8(
            reader,
            "source",
            InputLimit::new("max_source_bytes", Some(7)),
            Some(7),
        )
        .expect("short reads should be retried");

        assert_eq!(text, "diagram");
    }

    #[test]
    fn catches_a_length_hint_that_understates_the_stream() {
        let error = read_bytes(Cursor::new(b"12345"), "response body", Some(4), Some(1))
            .expect_err("streaming must enforce the limit independently of the hint");

        assert!(matches!(
            error,
            InputReadError::LimitExceeded {
                actual: ObservedSize::AtLeast(5),
                limit: 4,
                ..
            }
        ));
    }

    #[test]
    fn rejects_an_oversized_hint_before_reading() {
        let error = read_bytes(PanicReader, "response body", Some(4), Some(5))
            .expect_err("an oversized hint should fail before reading");

        assert!(matches!(
            error,
            InputReadError::LimitExceeded {
                actual: ObservedSize::Exact(5),
                limit: 4,
                ..
            }
        ));
    }

    #[test]
    fn supports_explicitly_unbounded_reads() {
        let bytes = vec![b'x'; 3 * 8 * 1024 + 17];

        let actual = read_bytes(Cursor::new(&bytes), "trusted input", None, Some(u64::MAX))
            .expect("unbounded mode should ignore the length hint");

        assert_eq!(actual, bytes);
    }

    struct ShortReader {
        bytes: Vec<u8>,
        offset: usize,
    }

    impl Read for ShortReader {
        fn read(&mut self, destination: &mut [u8]) -> io::Result<usize> {
            if self.offset == self.bytes.len() {
                return Ok(0);
            }
            let count = destination.len().min(1);
            destination[..count]
                .copy_from_slice(&self.bytes[self.offset..self.offset.saturating_add(count)]);
            self.offset += count;
            Ok(count)
        }
    }

    struct PanicReader;

    impl Read for PanicReader {
        fn read(&mut self, _destination: &mut [u8]) -> io::Result<usize> {
            panic!("reader must not be polled after an oversized length hint");
        }
    }
}
