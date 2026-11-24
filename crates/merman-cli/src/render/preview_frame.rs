use crate::error::CliError;
use crate::input::InputLimit;
use merman::OperationControl;
use merman::svg::ShoalLayoutFrame;
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

pub(crate) const MAX_BYTES: usize = 4 * 1024 * 1024;

pub(super) fn read(
    path: &Path,
    control: &OperationControl,
) -> Result<Option<Arc<ShoalLayoutFrame>>, CliError> {
    let bytes = crate::io::read_named_bytes_file_controlled(
        path,
        "preview layout frame input",
        InputLimit::new("max_preview_layout_frame_bytes", Some(MAX_BYTES)),
        control,
    )?;
    Ok(serde_json::from_slice(&bytes).ok().map(Arc::new))
}

pub(super) fn encode(frame: &ShoalLayoutFrame) -> Result<Option<Vec<u8>>, CliError> {
    let mut bytes = FrameBytes(Vec::new());
    match serde_json::to_writer(&mut bytes, frame) {
        Ok(()) => Ok(Some(bytes.0)),
        Err(error) if error.io_error_kind() == Some(io::ErrorKind::StorageFull) => Ok(None),
        Err(error) => Err(CliError::json_output(error)),
    }
}

struct FrameBytes(Vec<u8>);

impl Write for FrameBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_BYTES - self.0.len() {
            return Err(io::ErrorKind::StorageFull.into());
        }
        self.0.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
