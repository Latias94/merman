pub(crate) mod config;
mod document;
mod html;
mod publication;
mod receipt;
mod svg;

use crate::error::CliError;

pub(crate) use document::generate;
pub(crate) use publication::build;
pub(crate) use receipt::check;

fn operational_error(path: impl AsRef<std::path::Path>, reason: impl Into<String>) -> CliError {
    crate::transaction::TransactionError::InvalidState {
        evidence: path.as_ref().to_path_buf(),
        reason: reason.into(),
    }
    .into()
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let digest = Sha256::digest(bytes);
    encode_lower_hex(&digest)
}

fn encode_lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
