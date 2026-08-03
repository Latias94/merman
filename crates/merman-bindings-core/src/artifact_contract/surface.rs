use super::validation::invalid_artifact_contract;
use crate::BindingError;
use crate::capability::{CapabilityKey, OperationKey, TargetKey};
use crate::metadata_registry::MetadataKey;
use crate::payload_contract::BindingPayloadSchemaKey;
use crate::service_contract::{ConstructorServiceKey, RuntimePolicyExposure};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CompiledSelection<T> {
    Explicit(BTreeSet<T>),
    AllCompiled,
}

impl<T> Default for CompiledSelection<T> {
    fn default() -> Self {
        Self::Explicit(BTreeSet::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MetadataSelection {
    Explicit(BTreeSet<MetadataKey>),
    AllAvailable,
}

impl Default for MetadataSelection {
    fn default() -> Self {
        Self::Explicit(BTreeSet::new())
    }
}

/// Typed endpoint and service choices owned by one concrete transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportExposure {
    pub(super) target: TargetKey,
    pub(super) capabilities: CompiledSelection<CapabilityKey>,
    pub(super) operations: CompiledSelection<OperationKey>,
    pub(super) metadata: MetadataSelection,
    pub(super) payload_schemas: BTreeSet<BindingPayloadSchemaKey>,
    pub(super) constructor_services: BTreeSet<ConstructorServiceKey>,
    pub(super) runtime_policy: RuntimePolicyExposure,
}

impl TransportExposure {
    #[must_use]
    pub fn for_target(target: TargetKey) -> Self {
        Self {
            target,
            capabilities: CompiledSelection::default(),
            operations: CompiledSelection::default(),
            metadata: MetadataSelection::default(),
            payload_schemas: BTreeSet::new(),
            constructor_services: BTreeSet::new(),
            runtime_policy: RuntimePolicyExposure::default(),
        }
    }

    pub fn with_operations(
        mut self,
        operations: impl IntoIterator<Item = OperationKey>,
    ) -> Result<Self, BindingError> {
        insert_selection("transport operation", &mut self.operations, operations)?;
        Ok(self)
    }

    pub fn with_all_compiled_operations(mut self) -> Result<Self, BindingError> {
        select_all("transport operations", &mut self.operations)?;
        Ok(self)
    }

    pub fn with_supplemental_capabilities(
        mut self,
        capabilities: impl IntoIterator<Item = CapabilityKey>,
    ) -> Result<Self, BindingError> {
        insert_selection(
            "transport supplemental capability",
            &mut self.capabilities,
            capabilities,
        )?;
        Ok(self)
    }

    pub fn with_all_compiled_supplemental_capabilities(mut self) -> Result<Self, BindingError> {
        select_all(
            "transport supplemental capabilities",
            &mut self.capabilities,
        )?;
        Ok(self)
    }

    pub fn with_metadata(
        mut self,
        metadata: impl IntoIterator<Item = MetadataKey>,
    ) -> Result<Self, BindingError> {
        let MetadataSelection::Explicit(selected) = &mut self.metadata else {
            return Err(invalid_artifact_contract(
                "transport metadata already selects the complete available registry",
            ));
        };
        for key in metadata {
            if !selected.insert(key) {
                return Err(invalid_artifact_contract(format!(
                    "transport metadata `{}` was declared more than once",
                    key.id()
                )));
            }
        }
        Ok(self)
    }

    pub fn with_all_available_metadata(mut self) -> Result<Self, BindingError> {
        match &self.metadata {
            MetadataSelection::Explicit(selected) if selected.is_empty() => {
                self.metadata = MetadataSelection::AllAvailable;
                Ok(self)
            }
            _ => Err(invalid_artifact_contract(
                "transport metadata exposure was configured more than once",
            )),
        }
    }

    pub fn with_payload_schemas(
        mut self,
        schemas: impl IntoIterator<Item = BindingPayloadSchemaKey>,
    ) -> Result<Self, BindingError> {
        for schema in schemas {
            if !self.payload_schemas.insert(schema) {
                return Err(invalid_artifact_contract(format!(
                    "payload schema `{}` was declared more than once",
                    schema.id()
                )));
            }
        }
        Ok(self)
    }

    pub fn with_constructor_services(
        mut self,
        services: impl IntoIterator<Item = ConstructorServiceKey>,
    ) -> Result<Self, BindingError> {
        for service in services {
            if !self.constructor_services.insert(service) {
                return Err(invalid_artifact_contract(format!(
                    "constructor service `{}` was declared more than once",
                    service.id()
                )));
            }
        }
        Ok(self)
    }

    #[must_use]
    pub fn with_runtime_policy_exposure(mut self, exposure: RuntimePolicyExposure) -> Self {
        self.runtime_policy = exposure;
        self
    }
}

fn insert_selection<T>(
    label: &str,
    selection: &mut CompiledSelection<T>,
    values: impl IntoIterator<Item = T>,
) -> Result<(), BindingError>
where
    T: Copy + Ord + StableKey,
{
    let CompiledSelection::Explicit(selected) = selection else {
        return Err(invalid_artifact_contract(format!(
            "{label} selection already includes every compiled value"
        )));
    };
    for value in values {
        if !selected.insert(value) {
            return Err(invalid_artifact_contract(format!(
                "{label} `{}` was declared more than once",
                value.stable_id()
            )));
        }
    }
    Ok(())
}

fn select_all<T>(label: &str, selection: &mut CompiledSelection<T>) -> Result<(), BindingError>
where
    T: Ord,
{
    match selection {
        CompiledSelection::Explicit(selected) if selected.is_empty() => {
            *selection = CompiledSelection::AllCompiled;
            Ok(())
        }
        _ => Err(invalid_artifact_contract(format!(
            "{label} exposure was configured more than once"
        ))),
    }
}

trait StableKey {
    fn stable_id(self) -> &'static str;
}

impl StableKey for CapabilityKey {
    fn stable_id(self) -> &'static str {
        self.id()
    }
}

impl StableKey for OperationKey {
    fn stable_id(self) -> &'static str {
        self.id()
    }
}
