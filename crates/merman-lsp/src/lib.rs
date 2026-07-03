#![forbid(unsafe_code)]

mod code_actions;
mod completion;
mod diagnostics;
mod document_store;
mod protocol;
mod semantic_tokens;
mod server;
mod snapshot;
mod structure;

pub use protocol::{
    CONFIG_SCHEMA_METHOD, CONFIG_SCHEMA_RESPONSE_VERSION, ConfigSchemaResponse,
    EXPERIMENTAL_SCHEMA_VERSION, LspRuleCatalogEntry, RULE_CATALOG_METHOD,
    RULE_CATALOG_RESPONSE_VERSION, RuleCatalogResponse,
};
pub use server::MermanLanguageServer;

#[cfg(test)]
mod completion_tests;
#[cfg(test)]
mod diagnostics_tests;
#[cfg(test)]
mod document_store_tests;
