use super::surface::MetadataSelection;
use crate::BindingError;
use crate::capability::{CapabilityKey, OperationKey, TargetKey};
use crate::metadata_registry::{MetadataKey, metadata_spec};
use crate::service_contract::{ConstructorServiceKey, RuntimePolicyExposure};
use std::collections::BTreeSet;

pub(super) fn close_capability_implications(capabilities: &mut BTreeSet<CapabilityKey>) {
    loop {
        let implications = capabilities
            .iter()
            .flat_map(|capability| capability.spec().implications.iter().copied())
            .collect::<Vec<_>>();
        let old_len = capabilities.len();
        capabilities.extend(implications);
        if capabilities.len() == old_len {
            break;
        }
    }
}

pub(super) fn is_supplemental_capability(capability: CapabilityKey) -> bool {
    capability.spec().kind == "engine"
        || (capability.spec().kind == "api"
            && !OperationKey::ALL
                .iter()
                .any(|operation| operation.spec().capability == Some(capability)))
}

pub(super) fn validate_compiled_capabilities(
    target: TargetKey,
    selected: &BTreeSet<CapabilityKey>,
    compiled: &BTreeSet<CapabilityKey>,
) -> Result<(), BindingError> {
    for capability in selected {
        if !compiled.contains(capability) {
            return Err(invalid_artifact_contract(format!(
                "transport capability `{}` is not compiled",
                capability.id()
            )));
        }
        if !capability.spec().targets.contains(&target) {
            return Err(invalid_artifact_contract(format!(
                "transport capability `{}` is not valid for target `{}`",
                capability.id(),
                target.id()
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_compiled_operation_requirements(
    target: TargetKey,
    operations: &BTreeSet<OperationKey>,
    closed_requirements: &BTreeSet<CapabilityKey>,
    compiled: &BTreeSet<CapabilityKey>,
) -> Result<(), BindingError> {
    for capability in closed_requirements {
        if !compiled.contains(capability) {
            let operation_ids = operations
                .iter()
                .filter(|operation| {
                    let mut requirements = operation
                        .spec()
                        .compiled_prerequisites
                        .iter()
                        .copied()
                        .collect::<BTreeSet<_>>();
                    close_capability_implications(&mut requirements);
                    requirements.contains(capability)
                })
                .map(|operation| operation.id())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(invalid_artifact_contract(format!(
                "operation requirement `{}` for [{}] is not compiled",
                capability.id(),
                operation_ids
            )));
        }
        if !capability.spec().targets.contains(&target) {
            return Err(invalid_artifact_contract(format!(
                "operation requirement `{}` is not valid for target `{}`",
                capability.id(),
                target.id()
            )));
        }
    }
    Ok(())
}

pub(super) fn resolve_system_adapters(
    target: TargetKey,
    exposure: RuntimePolicyExposure,
    compiled: &BTreeSet<CapabilityKey>,
) -> BTreeSet<CapabilityKey> {
    if target != TargetKey::Native || exposure != RuntimePolicyExposure::BindingOptions {
        return BTreeSet::new();
    }

    let native = [
        CapabilityKey::SystemClock,
        CapabilityKey::SystemRandom,
        CapabilityKey::SystemTimezone,
    ];
    if native.iter().all(|key| compiled.contains(key)) {
        native.into_iter().collect()
    } else {
        BTreeSet::new()
    }
}

pub(super) fn resolve_metadata(
    selection: MetadataSelection,
    capabilities: &BTreeSet<CapabilityKey>,
    compiled: &BTreeSet<MetadataKey>,
) -> Result<BTreeSet<MetadataKey>, BindingError> {
    let metadata = match selection {
        MetadataSelection::AllAvailable => compiled
            .iter()
            .copied()
            .filter(|key| metadata_spec(*key).is_available(capabilities))
            .collect(),
        MetadataSelection::Explicit(metadata) => metadata,
    };
    for key in &metadata {
        if !compiled.contains(key) {
            return Err(invalid_artifact_contract(format!(
                "metadata `{}` is not compiled",
                key.id()
            )));
        }
        if !metadata_spec(*key).is_available(capabilities) {
            return Err(invalid_artifact_contract(format!(
                "metadata `{}` is unavailable for the selected capabilities",
                key.id()
            )));
        }
    }
    Ok(metadata)
}

pub(super) fn validate_services(
    selected: &BTreeSet<ConstructorServiceKey>,
    compiled: &BTreeSet<ConstructorServiceKey>,
    uses_svg_pipeline: bool,
) -> Result<(), BindingError> {
    for service in selected {
        if !compiled.contains(service) {
            return Err(invalid_artifact_contract(format!(
                "constructor service `{}` is not compiled",
                service.id()
            )));
        }
        let spec = service.spec();
        if !spec.is_available(uses_svg_pipeline) {
            return Err(invalid_artifact_contract(format!(
                "constructor service `{}` requires an exposed operation backed by the SVG rendering pipeline",
                service.id()
            )));
        }
    }
    Ok(())
}

pub(super) fn invalid_artifact_contract(message: impl Into<String>) -> BindingError {
    BindingError::invalid_argument(format!("invalid artifact contract: {}", message.into()))
}
