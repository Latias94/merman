#![forbid(unsafe_code)]

//! Ordered Mermaid language-server services and the optional bounded stdio transport.
//!
//! Embedded transports drive [`MermanLspService`] through Tower's `Service<Request>` contract and
//! own their own scheduling. With the `stdio` feature, `stdio_server` and `serve_stdio` provide
//! Merman's private admission policy, including exact small control handling, bounded ordinary
//! work, recoverable request overload, and `StdioTermination::InputOverloaded` when input
//! integrity cannot be preserved.

mod client_profile;
mod code_actions;
mod completion;
mod diagnostic_round_trip;
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
mod syntax_highlighting;
#[cfg(feature = "stdio")]
mod transport;
mod workspace_edit;

pub use protocol::{
    CONFIG_SCHEMA_METHOD, CONFIG_SCHEMA_RESPONSE_VERSION, ConfigSchemaResponse,
    EXPERIMENTAL_SCHEMA_VERSION, FIXED_TODAY_SCHEMA_PATTERN, RULE_CATALOG_METHOD,
    RULE_CATALOG_RESPONSE_VERSION, RuleCatalogEntry, RuleCatalogResponse,
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
