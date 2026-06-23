#![forbid(unsafe_code)]

pub mod completion;
pub mod diagnostics;
pub mod document_store;
pub mod server;

pub use server::MermanLanguageServer;
