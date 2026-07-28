use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};

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
            stdin: Box::new(io::stdin()),
            stdout: SharedWriter::new(io::stdout()),
            stderr: SharedWriter::new(io::stderr()),
            #[cfg(feature = "network-icons")]
            network: Box::new(crate::network::SystemNetworkAcquirer),
            #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
            publication: Box::new(crate::output::SystemPublicationBackend),
        }
    }
}
