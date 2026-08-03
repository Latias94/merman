//! Descriptor-owned capability vocabulary shared by every binding contract.
//!
//! The generated source is included exactly once so operation, artifact, and metadata APIs use
//! the same nominal Rust key types.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::sync::OnceLock;

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../capabilities/generated/capability_surface.rs"
));

pub(crate) fn compiled_capability_keys() -> &'static BTreeSet<CapabilityKey> {
    static COMPILED: OnceLock<BTreeSet<CapabilityKey>> = OnceLock::new();
    COMPILED.get_or_init(|| {
        let mut capabilities = BTreeSet::new();

        #[cfg(feature = "svg")]
        {
            capabilities.insert(CapabilityKey::Svg);
            if merman::svg::layout_cytoscape_available() {
                capabilities.insert(CapabilityKey::LayoutCytoscape);
            }
            if merman::svg::layout_elk_available() {
                capabilities.insert(CapabilityKey::LayoutElk);
            }
            if merman::svg::math_available() {
                capabilities.insert(CapabilityKey::Math);
            }
        }
        #[cfg(feature = "analysis")]
        capabilities.insert(CapabilityKey::Analysis);
        #[cfg(feature = "ascii")]
        capabilities.insert(CapabilityKey::Ascii);
        #[cfg(feature = "png")]
        capabilities.insert(CapabilityKey::Png);
        #[cfg(feature = "jpeg")]
        capabilities.insert(CapabilityKey::Jpeg);
        #[cfg(feature = "pdf")]
        capabilities.insert(CapabilityKey::Pdf);

        for id in merman::runtime::compiled_system_adapter_ids() {
            let key = CapabilityKey::from_id(id)
                .expect("runtime adapter IDs must be owned by the capability descriptor");
            capabilities.insert(key);
        }

        capabilities
    })
}

pub(crate) fn compiled_operation_keys() -> BTreeSet<OperationKey> {
    OperationKey::ALL
        .iter()
        .copied()
        .filter(|key| operation_is_compiled(*key))
        .collect()
}

pub(crate) fn operation_is_compiled(key: OperationKey) -> bool {
    let compiled = compiled_capability_keys();
    let spec = key.spec();
    spec.capability
        .is_none_or(|capability| compiled.contains(&capability))
        && spec
            .compiled_prerequisites
            .iter()
            .all(|capability| compiled.contains(capability))
}

/// One capability implemented by a transport crate rather than binding-core itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum TransportCompiledExtensionKey {
    Editor,
}

impl TransportCompiledExtensionKey {
    #[must_use]
    pub const fn capability(self) -> CapabilityKey {
        match self {
            Self::Editor => CapabilityKey::Editor,
        }
    }
}
