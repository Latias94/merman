use crate::{BindingPayloadSchemaKey, ConstructorServiceKey, TargetKey};

/// Feature-independent facade exposure projected into one concrete artifact selection.
///
/// The registry owns the payload schemas and constructor-service candidates that a maintained
/// facade can represent. A concrete artifact may select a subset of the service candidates when
/// its feature or target recipe does not compile the required pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct BindingTransportExposureSpec {
    key: BindingTransportKey,
    targets: &'static [TargetKey],
    payload_schemas: &'static [BindingPayloadSchemaKey],
    constructor_service_candidates: &'static [ConstructorServiceKey],
}

impl BindingTransportExposureSpec {
    #[must_use]
    pub const fn key(&self) -> BindingTransportKey {
        self.key
    }

    #[must_use]
    pub const fn targets(&self) -> &'static [TargetKey] {
        self.targets
    }

    #[must_use]
    pub const fn payload_schemas(&self) -> &'static [BindingPayloadSchemaKey] {
        self.payload_schemas
    }

    #[must_use]
    pub const fn constructor_service_candidates(&self) -> &'static [ConstructorServiceKey] {
        self.constructor_service_candidates
    }

    pub(crate) const fn supports_target(&self, target: TargetKey) -> bool {
        let mut index = 0;
        while index < self.targets.len() {
            if target_discriminant(self.targets[index]) == target_discriminant(target) {
                return true;
            }
            index += 1;
        }
        false
    }
}

macro_rules! define_binding_transports {
    (
        $(
            $variant:ident => {
                id: $id:literal,
                targets: $targets:expr,
                payload_schemas: $payload_schemas:expr,
                constructor_service_candidates: $constructor_service_candidates:expr,
            }
        ),+ $(,)?
    ) => {
        /// One maintained binding transport whose artifact exposure is projected into host SDKs.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[non_exhaustive]
        #[repr(u8)]
        pub enum BindingTransportKey {
            $($variant),+
        }

        impl BindingTransportKey {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            #[must_use]
            pub const fn id(self) -> &'static str {
                match self {
                    $(Self::$variant => $id),+
                }
            }

            #[must_use]
            pub const fn spec(self) -> &'static BindingTransportExposureSpec {
                &TRANSPORT_EXPOSURE_SPECS[self as usize]
            }
        }

        const TRANSPORT_EXPOSURE_SPECS: &[BindingTransportExposureSpec] = &[
            $(
                BindingTransportExposureSpec {
                    key: BindingTransportKey::$variant,
                    targets: $targets,
                    payload_schemas: $payload_schemas,
                    constructor_service_candidates: $constructor_service_candidates,
                }
            ),+
        ];
    };
}

const NATIVE_TARGET: &[TargetKey] = &[TargetKey::Native];
const NODE_TARGETS: &[TargetKey] = &[TargetKey::Native, TargetKey::Web];
const TYPST_TARGET: &[TargetKey] = &[TargetKey::Typst];
const WEB_TARGET: &[TargetKey] = &[TargetKey::Web];
const COMMON_JSON_PAYLOAD_SCHEMAS: &[BindingPayloadSchemaKey] = &[
    BindingPayloadSchemaKey::BindingResult,
    BindingPayloadSchemaKey::OperationMetadata,
];
const BINDING_RESULT_PAYLOAD_SCHEMA: &[BindingPayloadSchemaKey] =
    &[BindingPayloadSchemaKey::BindingResult];
const HOST_TEXT_MEASUREMENT_SERVICE: &[ConstructorServiceKey] =
    &[ConstructorServiceKey::HostTextMeasurement];
const NATIVE_C_SERVICES: &[ConstructorServiceKey] = &[
    ConstructorServiceKey::HostTextMeasurement,
    ConstructorServiceKey::IconRegistry,
];
const RUST_SERVICES: &[ConstructorServiceKey] = &[
    ConstructorServiceKey::HostTextMeasurement,
    ConstructorServiceKey::IconRegistry,
];

define_binding_transports! {
    AndroidJni => {
        id: "android-jni",
        targets: NATIVE_TARGET,
        payload_schemas: COMMON_JSON_PAYLOAD_SCHEMAS,
        constructor_service_candidates: HOST_TEXT_MEASUREMENT_SERVICE,
    },
    NativeC => {
        id: "native-c",
        targets: NATIVE_TARGET,
        payload_schemas: COMMON_JSON_PAYLOAD_SCHEMAS,
        constructor_service_candidates: NATIVE_C_SERVICES,
    },
    Node => {
        id: "node",
        targets: NODE_TARGETS,
        payload_schemas: COMMON_JSON_PAYLOAD_SCHEMAS,
        constructor_service_candidates: &[],
    },
    Rust => {
        id: "rust",
        targets: NATIVE_TARGET,
        payload_schemas: COMMON_JSON_PAYLOAD_SCHEMAS,
        constructor_service_candidates: RUST_SERVICES,
    },
    Typst => {
        id: "typst",
        targets: TYPST_TARGET,
        payload_schemas: &[],
        constructor_service_candidates: &[],
    },
    UniFfi => {
        id: "uniffi",
        targets: NATIVE_TARGET,
        payload_schemas: COMMON_JSON_PAYLOAD_SCHEMAS,
        constructor_service_candidates: RUST_SERVICES,
    },
    Web => {
        id: "web",
        targets: WEB_TARGET,
        payload_schemas: BINDING_RESULT_PAYLOAD_SCHEMA,
        constructor_service_candidates: HOST_TEXT_MEASUREMENT_SERVICE,
    },
}

const fn target_discriminant(target: TargetKey) -> u8 {
    match target {
        TargetKey::Native => 0,
        TargetKey::Typst => 1,
        TargetKey::Web => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn transport_exposure_registry_is_bijective_and_sorted() {
        assert_eq!(
            BindingTransportKey::ALL.len(),
            TRANSPORT_EXPOSURE_SPECS.len()
        );
        let ids = BindingTransportKey::ALL
            .iter()
            .copied()
            .map(BindingTransportKey::id)
            .collect::<Vec<_>>();
        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));

        for (index, key) in BindingTransportKey::ALL.iter().copied().enumerate() {
            assert_eq!(key as usize, index);
            assert_eq!(key.spec().key(), key);
        }
    }

    #[test]
    fn transport_exposure_slices_are_sorted_and_unique() {
        for key in BindingTransportKey::ALL {
            let spec = key.spec();
            assert_sorted_unique(spec.targets().iter().map(|target| target.id()));
            assert_sorted_unique(spec.payload_schemas().iter().map(|schema| schema.id()));
            assert_sorted_unique(
                spec.constructor_service_candidates()
                    .iter()
                    .map(|service| service.id()),
            );
        }
    }

    fn assert_sorted_unique<'a>(ids: impl Iterator<Item = &'a str>) {
        let ids = ids.collect::<Vec<_>>();
        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            ids.len(),
            ids.iter().copied().collect::<BTreeSet<_>>().len()
        );
    }
}
