use crate::session::LSP_MAX_MESSAGE_BYTES;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::io;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tower_lsp_server::jsonrpc::{Error, Request, Response};

const MAX_HEADER_BYTES: usize = 8 * 1024;

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum IncomingMessage {
    Response(Response),
    Request(Request),
}

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum OutgoingMessage {
    Response(Response),
    Request(Request),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Recovery {
    Continue,
    Stop,
}

#[derive(Debug)]
pub(super) enum FrameError {
    HeaderTooLarge { limit: usize },
    BodyTooLarge { length: usize, limit: usize },
    InvalidHeader(String),
    InvalidContentLength,
    MissingContentLength,
    EmptyBody,
    InvalidContentType,
    InvalidUtf8(std::str::Utf8Error),
    InvalidJson(serde_json::Error),
}

impl FrameError {
    pub(super) fn jsonrpc_error(&self) -> Error {
        match self {
            Self::InvalidJson(error) if error.is_data() => Error::invalid_request(),
            _ => Error::parse_error(),
        }
    }
}

impl Display for FrameError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::HeaderTooLarge { limit } => {
                write!(formatter, "LSP headers exceed the {limit}-byte limit")
            }
            Self::BodyTooLarge { length, limit } => write!(
                formatter,
                "LSP message body is {length} bytes, exceeding the {limit}-byte limit"
            ),
            Self::InvalidHeader(message) => write!(formatter, "invalid LSP header: {message}"),
            Self::InvalidContentLength => formatter.write_str("invalid Content-Length header"),
            Self::MissingContentLength => {
                formatter.write_str("missing required Content-Length header")
            }
            Self::EmptyBody => formatter.write_str("LSP message body is empty"),
            Self::InvalidContentType => formatter
                .write_str("Content-Type must declare the utf-8 or utf8 character encoding"),
            Self::InvalidUtf8(error) => write!(formatter, "message body is not UTF-8: {error}"),
            Self::InvalidJson(error) => write!(formatter, "invalid JSON-RPC message: {error}"),
        }
    }
}

pub(super) enum FrameRead {
    Eof,
    Message {
        message: IncomingMessage,
        body_length: usize,
    },
    Error(FrameError, Recovery),
}

struct HeaderBlock {
    prefix: Vec<u8>,
    too_large: bool,
    boundary_complete: bool,
}

struct HeaderParseError {
    error: FrameError,
    content_length: Option<usize>,
}

pub(super) struct LspFrameReader<I> {
    input: BufReader<I>,
    max_header_bytes: usize,
    max_message_bytes: usize,
}

impl<I> LspFrameReader<I>
where
    I: AsyncRead + Unpin,
{
    pub(super) fn new(input: I) -> Self {
        Self::with_limits(input, MAX_HEADER_BYTES, LSP_MAX_MESSAGE_BYTES)
    }

    pub(super) fn with_limits(input: I, max_header_bytes: usize, max_message_bytes: usize) -> Self {
        Self {
            input: BufReader::new(input),
            max_header_bytes,
            max_message_bytes,
        }
    }

    pub(super) async fn read_frame(&mut self) -> io::Result<FrameRead> {
        let Some(headers) = self.read_header_block().await? else {
            return Ok(FrameRead::Eof);
        };

        if headers.too_large {
            let content_length = content_length_from_prefix(&headers.prefix);
            let recovery = if headers.boundary_complete {
                self.recover_body(content_length).await?
            } else {
                Recovery::Stop
            };
            return Ok(FrameRead::Error(
                FrameError::HeaderTooLarge {
                    limit: self.max_header_bytes,
                },
                recovery,
            ));
        }

        let content_length = match parse_headers(&headers.prefix) {
            Ok(content_length) => content_length,
            Err(error) => {
                let recovery = self.recover_body(error.content_length).await?;
                return Ok(FrameRead::Error(error.error, recovery));
            }
        };

        if content_length > self.max_message_bytes {
            let recovery = self.recover_body(Some(content_length)).await?;
            return Ok(FrameRead::Error(
                FrameError::BodyTooLarge {
                    length: content_length,
                    limit: self.max_message_bytes,
                },
                recovery,
            ));
        }

        if content_length == 0 {
            return Ok(FrameRead::Error(FrameError::EmptyBody, Recovery::Continue));
        }

        let mut body = vec![0; content_length];
        self.input.read_exact(&mut body).await?;
        let body = match std::str::from_utf8(&body) {
            Ok(body) => body,
            Err(error) => {
                return Ok(FrameRead::Error(
                    FrameError::InvalidUtf8(error),
                    Recovery::Continue,
                ));
            }
        };
        tracing::trace!("<- {}", body);
        match serde_json::from_str(body) {
            Ok(message) => Ok(FrameRead::Message {
                message,
                body_length: content_length,
            }),
            Err(error) => Ok(FrameRead::Error(
                FrameError::InvalidJson(error),
                Recovery::Continue,
            )),
        }
    }

    async fn read_header_block(&mut self) -> io::Result<Option<HeaderBlock>> {
        let mut prefix = Vec::with_capacity(self.max_header_bytes.min(256));
        let mut too_large = false;
        let mut bytes_seen = 0usize;
        let recoverable_limit = self.max_header_bytes.saturating_mul(2);
        let mut previous_3 = 0u8;
        let mut previous_2 = 0u8;
        let mut previous_1 = 0u8;

        loop {
            let available = self.input.fill_buf().await?;
            if available.is_empty() {
                if bytes_seen == 0 {
                    return Ok(None);
                }
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "stdin closed inside an LSP header block",
                ));
            }

            let mut consumed = 0usize;
            let mut complete = false;
            for byte in available.iter().copied() {
                consumed += 1;
                bytes_seen += 1;
                if prefix.len() < self.max_header_bytes {
                    prefix.push(byte);
                } else {
                    too_large = true;
                }

                let crlf_terminated = bytes_seen >= 4
                    && previous_3 == b'\r'
                    && previous_2 == b'\n'
                    && previous_1 == b'\r'
                    && byte == b'\n';
                let lf_terminated = bytes_seen >= 2 && previous_1 == b'\n' && byte == b'\n';
                previous_3 = previous_2;
                previous_2 = previous_1;
                previous_1 = byte;

                if crlf_terminated || lf_terminated {
                    complete = true;
                    break;
                }
                if bytes_seen > recoverable_limit {
                    break;
                }
            }
            self.input.consume(consumed);

            if complete {
                return Ok(Some(HeaderBlock {
                    prefix,
                    too_large,
                    boundary_complete: true,
                }));
            }
            if bytes_seen > recoverable_limit {
                return Ok(Some(HeaderBlock {
                    prefix,
                    too_large: true,
                    boundary_complete: false,
                }));
            }
        }
    }

    async fn recover_body(&mut self, content_length: Option<usize>) -> io::Result<Recovery> {
        let Some(content_length) = content_length else {
            return Ok(Recovery::Stop);
        };
        let recoverable_limit = self.max_message_bytes.saturating_mul(2);
        if content_length > recoverable_limit {
            return Ok(Recovery::Stop);
        }

        let mut remaining = content_length;
        let mut buffer = [0u8; 8 * 1024];
        while remaining > 0 {
            let chunk_length = remaining.min(buffer.len());
            self.input.read_exact(&mut buffer[..chunk_length]).await?;
            remaining -= chunk_length;
        }
        Ok(Recovery::Continue)
    }
}

fn content_length_from_prefix(headers: &[u8]) -> Option<usize> {
    let headers = std::str::from_utf8(headers).ok()?;
    let mut content_length = None;
    for line in headers.split_inclusive('\n') {
        if !line.ends_with('\n') {
            continue;
        }
        let line = line.trim_end_matches(['\r', '\n']);
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if !name.eq_ignore_ascii_case("Content-Length") {
            continue;
        }
        if content_length.is_some() {
            return None;
        }
        content_length = parse_content_length(value);
        content_length?;
    }
    content_length
}

fn parse_headers(headers: &[u8]) -> Result<usize, HeaderParseError> {
    let headers = std::str::from_utf8(headers).map_err(|error| HeaderParseError {
        error: FrameError::InvalidHeader(error.to_string()),
        content_length: None,
    })?;
    let mut content_length = None;
    let mut saw_content_length = false;
    let mut first_error = None;

    for line in headers.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            first_error.get_or_insert_with(|| {
                FrameError::InvalidHeader("header line lacks ':' separator".to_owned())
            });
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("Content-Length") {
            if saw_content_length {
                return Err(HeaderParseError {
                    error: FrameError::InvalidHeader(
                        "duplicate Content-Length headers are not allowed".to_owned(),
                    ),
                    content_length: None,
                });
            }
            saw_content_length = true;
            match parse_content_length(value) {
                Some(length) => content_length = Some(length),
                None => {
                    first_error.get_or_insert(FrameError::InvalidContentLength);
                }
            };
        } else if name.eq_ignore_ascii_case("Content-Type") {
            if !valid_content_type(value) {
                first_error.get_or_insert(FrameError::InvalidContentType);
            }
        } else {
            tracing::warn!(header = name, "ignoring unsupported LSP header");
        }
    }

    if let Some(error) = first_error {
        return Err(HeaderParseError {
            error,
            content_length,
        });
    }
    content_length.ok_or(HeaderParseError {
        error: FrameError::MissingContentLength,
        content_length: None,
    })
}

fn parse_content_length(value: &str) -> Option<usize> {
    let value = value.trim();
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn valid_content_type(value: &str) -> bool {
    value.split(';').skip(1).any(|parameter| {
        let Some((name, value)) = parameter.trim().split_once('=') else {
            return false;
        };
        name.trim().eq_ignore_ascii_case("charset")
            && matches!(value.trim().to_ascii_lowercase().as_str(), "utf-8" | "utf8")
    })
}

pub(super) async fn write_message<O>(output: &mut O, message: &OutgoingMessage) -> io::Result<()>
where
    O: AsyncWrite + Unpin,
{
    let body = serde_json::to_vec(message).map_err(io::Error::other)?;
    tracing::trace!("-> {}", String::from_utf8_lossy(&body));
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    output.write_all(header.as_bytes()).await?;
    output.write_all(&body).await?;
    output.flush().await
}
