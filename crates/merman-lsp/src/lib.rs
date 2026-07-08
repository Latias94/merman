#![forbid(unsafe_code)]

mod code_actions;
mod completion;
mod diagnostics;
mod document_store;
mod protocol;
mod semantic_tokens;
mod server;
mod snapshot;
mod snapshot_context;
mod structure;
mod transport;

pub use protocol::{
    CONFIG_SCHEMA_METHOD, CONFIG_SCHEMA_RESPONSE_VERSION, ConfigSchemaResponse,
    EXPERIMENTAL_SCHEMA_VERSION, RULE_CATALOG_METHOD, RULE_CATALOG_RESPONSE_VERSION,
    RuleCatalogEntry, RuleCatalogResponse,
};
pub use server::MermanLanguageServer;
pub use transport::{LSP_HANDLER_CONCURRENCY, stdio_server};

#[cfg(test)]
mod completion_tests;
#[cfg(test)]
mod diagnostics_tests;
#[cfg(test)]
mod document_store_tests;
#[cfg(test)]
mod snapshot_context_tests;
