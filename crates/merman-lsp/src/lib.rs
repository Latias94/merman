#![forbid(unsafe_code)]

mod client_profile;
mod code_actions;
mod completion;
mod diagnostics;
mod protocol;
mod refresh_coordinator;
mod refresh_transport;
mod semantic_tokens;
mod server;
mod session;
mod snapshot;
mod structure;
mod sync;
#[cfg(feature = "stdio")]
mod transport;

pub use protocol::{
    CONFIG_SCHEMA_METHOD, CONFIG_SCHEMA_RESPONSE_VERSION, ConfigSchemaResponse,
    EXPERIMENTAL_SCHEMA_VERSION, RULE_CATALOG_METHOD, RULE_CATALOG_RESPONSE_VERSION,
    RuleCatalogEntry, RuleCatalogResponse,
};
pub use refresh_transport::{
    MermanClientSocket, MermanClientSocketError, MermanRequestStream, MermanResponseSink,
};
pub use server::MermanLanguageServer;
pub use session::{
    LSP_MAX_MESSAGE_BYTES, LSP_ORDINARY_HANDLER_CONCURRENCY, LSP_REQUEST_BYTE_BUDGET,
    MermanLspService,
};
#[cfg(feature = "stdio")]
pub use transport::{StdioServer, StdioTermination, serve_stdio, stdio_server};

#[cfg(test)]
mod completion_tests;
#[cfg(test)]
mod diagnostics_tests;
