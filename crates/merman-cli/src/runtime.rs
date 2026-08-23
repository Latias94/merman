use std::io::{self, Read, Write};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::input::IO_CHUNK_BYTES;

const STDIN_CONTROL_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Clone)]
pub(crate) struct SharedWriter {
    inner: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl SharedWriter {
    pub(crate) fn new(writer: impl Write + Send + 'static) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Box::new(writer))),
        }
    }

    pub(crate) fn write_all(&self, bytes: &[u8]) -> io::Result<()> {
        self.with_writer(|writer| writer.write_all(bytes))
    }

    pub(crate) fn with_writer<T>(&self, operation: impl FnOnce(&mut dyn Write) -> T) -> T {
        let mut writer = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        operation(writer.as_mut())
    }
}

pub(crate) struct ExecutionContext {
    pub(crate) stdin: Box<dyn Read + Send>,
    pub(crate) stdout: SharedWriter,
    pub(crate) stderr: SharedWriter,
    #[cfg(feature = "network-icons")]
    pub(crate) network: Box<dyn crate::network::NetworkAcquirer>,
    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    pub(crate) publication: Box<dyn crate::output::PublicationBackend>,
}

impl ExecutionContext {
    pub(crate) fn system() -> Self {
        Self {
            stdin: Box::new(InterruptibleStdin::new()),
            stdout: SharedWriter::new(io::stdout()),
            stderr: SharedWriter::new(io::stderr()),
            #[cfg(feature = "network-icons")]
            network: Box::new(crate::network::SystemNetworkAcquirer),
            #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
            publication: Box::new(crate::output::SystemPublicationBackend),
        }
    }
}

/// Keeps the process stdin producer bounded while giving the command thread a
/// regular opportunity to observe cooperative cancellation and deadlines.
///
/// The pump is lazy so argument and preflight failures still perform no input
/// acquisition. Dropping the receiver stops a producer that is between reads;
/// a producer blocked in the operating-system read is detached and terminates
/// with the process.
struct InterruptibleStdin {
    source: Option<io::Stdin>,
    receiver: Option<Receiver<StdinEvent>>,
    pending: Vec<u8>,
    pending_offset: usize,
    start_failed: bool,
}

impl InterruptibleStdin {
    fn new() -> Self {
        Self {
            source: Some(io::stdin()),
            receiver: None,
            pending: Vec::new(),
            pending_offset: 0,
            start_failed: false,
        }
    }

    fn start(&mut self) -> io::Result<()> {
        if self.receiver.is_some() {
            return Ok(());
        }
        if self.start_failed {
            return Err(io::Error::other("stdin acquisition worker is unavailable"));
        }
        let source = self
            .source
            .take()
            .ok_or_else(|| io::Error::other("stdin acquisition worker has no source"))?;
        let (sender, receiver) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("merman-cli-stdin".to_string())
            .spawn(move || pump_stdin(source, &sender))
        {
            Ok(handle) => {
                drop(handle);
                self.receiver = Some(receiver);
                Ok(())
            }
            Err(error) => {
                self.start_failed = true;
                Err(error)
            }
        }
    }
}

impl Read for InterruptibleStdin {
    fn read(&mut self, destination: &mut [u8]) -> io::Result<usize> {
        if destination.is_empty() {
            return Ok(0);
        }
        if self.pending_offset < self.pending.len() {
            return Ok(copy_pending(
                &self.pending,
                &mut self.pending_offset,
                destination,
            ));
        }

        self.start()?;
        let receiver = self
            .receiver
            .as_ref()
            .expect("a successful stdin start installs its receiver");
        match receiver.recv_timeout(STDIN_CONTROL_POLL_INTERVAL) {
            Ok(StdinEvent::Bytes(bytes)) => {
                self.pending = bytes;
                self.pending_offset = 0;
                Ok(copy_pending(
                    &self.pending,
                    &mut self.pending_offset,
                    destination,
                ))
            }
            Ok(StdinEvent::End) | Err(RecvTimeoutError::Disconnected) => Ok(0),
            Ok(StdinEvent::Error(error)) => Err(error),
            Err(RecvTimeoutError::Timeout) => Err(io::ErrorKind::Interrupted.into()),
        }
    }
}

enum StdinEvent {
    Bytes(Vec<u8>),
    End,
    Error(io::Error),
}

fn pump_stdin(source: io::Stdin, sender: &SyncSender<StdinEvent>) {
    let mut source = source.lock();
    let mut chunk = [0_u8; IO_CHUNK_BYTES];
    loop {
        match source.read(&mut chunk) {
            Ok(0) => {
                let _ = sender.send(StdinEvent::End);
                break;
            }
            Ok(count) => {
                if sender
                    .send(StdinEvent::Bytes(chunk[..count].to_vec()))
                    .is_err()
                {
                    break;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                let _ = sender.send(StdinEvent::Error(error));
                break;
            }
        }
    }
}

fn copy_pending(pending: &[u8], offset: &mut usize, destination: &mut [u8]) -> usize {
    let count = destination.len().min(pending.len().saturating_sub(*offset));
    let end = offset.saturating_add(count);
    destination[..count].copy_from_slice(&pending[*offset..end]);
    *offset = end;
    count
}
