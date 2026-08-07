use crate::common::BINDING_RESULT_PAYLOAD_VERSION;

/// Stable schema version for binding operation metadata and per-operation options.
pub const BINDING_OPERATION_SCHEMA_VERSION: u32 = 1;

/// One binding-owned payload schema actually returned by a transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum BindingPayloadSchemaKey {
    BindingResult,
    OperationMetadata,
}

impl BindingPayloadSchemaKey {
    pub const ALL: &'static [Self] = &[Self::BindingResult, Self::OperationMetadata];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::BindingResult => "binding-result",
            Self::OperationMetadata => "operation-metadata",
        }
    }

    #[must_use]
    pub const fn version(self) -> u32 {
        match self {
            Self::BindingResult => BINDING_RESULT_PAYLOAD_VERSION,
            Self::OperationMetadata => BINDING_OPERATION_SCHEMA_VERSION,
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|key| key.id() == id)
    }
}
