//! Descriptor-owned capability vocabulary shared by every binding contract.
//!
//! The generated source is included exactly once so operation, artifact, and metadata APIs use
//! the same nominal Rust key types.

#![allow(dead_code)]

use crate::key_set::KeySet;

include!("generated/capability_surface.rs");

#[cfg(feature = "svg")]
const COMPILED_SVG_CAPABILITY_BITS: u64 = CapabilityKey::Svg.compact_bit()
    | if merman::svg::layout_cytoscape_available() {
        CapabilityKey::LayoutCytoscape.compact_bit()
    } else {
        0
    }
    | if merman::svg::layout_elk_available() {
        CapabilityKey::LayoutElk.compact_bit()
    } else {
        0
    }
    | if merman::svg::math_available() {
        CapabilityKey::Math.compact_bit()
    } else {
        0
    };

#[cfg(not(feature = "svg"))]
const COMPILED_SVG_CAPABILITY_BITS: u64 = 0;

const COMPILED_CAPABILITY_BITS: u64 = COMPILED_SVG_CAPABILITY_BITS
    | if cfg!(feature = "analysis") {
        CapabilityKey::Analysis.compact_bit()
    } else {
        0
    }
    | if cfg!(feature = "ascii") {
        CapabilityKey::Ascii.compact_bit()
    } else {
        0
    }
    | if cfg!(feature = "png") {
        CapabilityKey::Png.compact_bit()
    } else {
        0
    }
    | if cfg!(feature = "jpeg") {
        CapabilityKey::Jpeg.compact_bit()
    } else {
        0
    }
    | if cfg!(feature = "pdf") {
        CapabilityKey::Pdf.compact_bit()
    } else {
        0
    }
    | if cfg!(feature = "native-runtime") {
        CapabilityKey::SystemClock.compact_bit()
            | CapabilityKey::SystemRandom.compact_bit()
            | CapabilityKey::SystemTimezone.compact_bit()
    } else {
        0
    };

pub(crate) const fn compiled_capability_keys() -> KeySet<CapabilityKey> {
    KeySet::from_bits(COMPILED_CAPABILITY_BITS)
}

pub(crate) fn operation_is_compiled(key: OperationKey) -> bool {
    let compiled = compiled_capability_keys();
    let spec = key.spec();
    spec.capability
        .is_none_or(|capability| compiled.contains(capability))
        && spec
            .compiled_prerequisites
            .iter()
            .all(|capability| compiled.contains(*capability))
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

#[cfg(all(
    test,
    feature = "svg",
    feature = "layout-cytoscape",
    feature = "layout-elk",
    feature = "math"
))]
mod feature_unification_probe_tests {
    #[test]
    fn ambient_render_backends_are_really_compiled() {
        assert!(merman::svg::layout_cytoscape_available());
        assert!(merman::svg::layout_elk_available());
        assert!(merman::svg::math_available());
    }
}
