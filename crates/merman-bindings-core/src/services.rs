use crate::{BindingError, ConstructorServiceKey, common};
#[cfg(feature = "svg")]
use std::sync::Arc;

/// Immutable binding-owned icon registry constructed through [`build_icon_registry`].
///
/// The renderer registry stays behind a private field so every binding transport receives the
/// same structured construction errors and cannot bypass the canonical admission seam.
#[cfg(feature = "svg")]
#[derive(Clone)]
pub struct BindingIconRegistry {
    inner: merman::svg::IconRegistry,
}

#[cfg(feature = "svg")]
impl BindingIconRegistry {
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub(crate) fn renderer_registry(&self) -> merman::svg::IconRegistry {
        self.inner.clone()
    }
}

#[cfg(feature = "svg")]
impl std::fmt::Debug for BindingIconRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BindingIconRegistry")
            .field("entry_count", &self.len())
            .finish_non_exhaustive()
    }
}

/// Immutable constructor-owned services shared by a reusable binding engine.
///
/// Foreign transports remain responsible for callback admission, retention, quiescence, and
/// out-of-lock destruction. This value only carries transport-neutral Rust service objects into
/// the engine's single materialization path.
#[derive(Clone, Default)]
#[non_exhaustive]
pub struct BindingEngineServices {
    #[cfg(feature = "svg")]
    host_text_measurer: Option<Arc<dyn crate::HostTextMeasurer>>,
    #[cfg(feature = "svg")]
    icon_registry: Option<BindingIconRegistry>,
}

impl std::fmt::Debug for BindingEngineServices {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = formatter.debug_struct("BindingEngineServices");
        #[cfg(feature = "svg")]
        debug.field("host_text_measurer", &self.host_text_measurer.is_some());
        #[cfg(feature = "svg")]
        debug.field("icon_registry", &self.icon_registry.is_some());
        debug.finish_non_exhaustive()
    }
}

impl BindingEngineServices {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            #[cfg(feature = "svg")]
            host_text_measurer: None,
            #[cfg(feature = "svg")]
            icon_registry: None,
        }
    }

    #[cfg(feature = "svg")]
    #[must_use]
    pub fn with_host_text_measurer(mut self, measurer: Arc<dyn crate::HostTextMeasurer>) -> Self {
        self.host_text_measurer = Some(measurer);
        self
    }

    #[cfg(feature = "svg")]
    #[must_use]
    pub fn with_icon_registry(mut self, registry: BindingIconRegistry) -> Self {
        self.icon_registry = (!registry.is_empty()).then_some(registry);
        self
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        #[cfg(feature = "svg")]
        {
            self.host_text_measurer.is_none() && self.icon_registry.is_none()
        }
        #[cfg(not(feature = "svg"))]
        {
            true
        }
    }

    pub(crate) fn validate_options(
        &self,
        options: &common::BindingOptions,
    ) -> Result<(), BindingError> {
        #[cfg(feature = "svg")]
        if self.host_text_measurer.is_some() && options.text_measurement_selector_explicit {
            return Err(BindingError::invalid_argument(
                "constructor service `host-text-measurement` conflicts with explicit option `environment.text_measurement`",
            ));
        }
        #[cfg(not(feature = "svg"))]
        let _ = options;
        Ok(())
    }

    #[cfg(feature = "svg")]
    pub(crate) fn host_text_measurer(&self) -> Option<Arc<dyn crate::HostTextMeasurer>> {
        self.host_text_measurer.as_ref().map(Arc::clone)
    }

    #[cfg(feature = "svg")]
    pub(crate) fn icon_registry(&self) -> Option<merman::svg::IconRegistry> {
        self.icon_registry
            .as_ref()
            .map(BindingIconRegistry::renderer_registry)
    }

    pub(crate) fn service_keys(&self) -> impl Iterator<Item = ConstructorServiceKey> {
        #[cfg(feature = "svg")]
        {
            [
                self.host_text_measurer
                    .is_some()
                    .then_some(ConstructorServiceKey::HostTextMeasurement),
                self.icon_registry
                    .is_some()
                    .then_some(ConstructorServiceKey::IconRegistry),
            ]
            .into_iter()
            .flatten()
        }
        #[cfg(not(feature = "svg"))]
        {
            std::iter::empty()
        }
    }
}

/// Builds one immutable registry through the canonical binding error-mapping seam.
#[cfg(feature = "svg")]
pub fn build_icon_registry<'a>(
    packs: impl IntoIterator<Item = crate::IconPack<'a>>,
) -> Result<BindingIconRegistry, BindingError> {
    merman::svg::IconRegistry::from_packs(packs)
        .map(|inner| BindingIconRegistry { inner })
        .map_err(BindingError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_services_exist_without_optional_features() {
        let services = BindingEngineServices::new();
        assert!(services.is_empty());
        assert!(services.clone().is_empty());
    }

    #[cfg(feature = "svg")]
    #[test]
    fn empty_icon_registry_is_normalized_to_no_service() {
        let registry = build_icon_registry(std::iter::empty::<crate::IconPack<'_>>())
            .expect("an empty registry is a valid immutable value");
        let services = BindingEngineServices::new().with_icon_registry(registry);

        assert!(services.is_empty());
        assert!(services.icon_registry().is_none());
        assert_eq!(services.service_keys().count(), 0);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn canonical_icon_factory_preserves_structured_limit_details() {
        let maximum =
            usize::try_from(crate::IconRegistryResourceLimitId::MaxPrefixBytes.fixed_value())
                .expect("fixed prefix limit fits usize");
        let registration_name = "a".repeat(maximum + 1);
        let error = build_icon_registry([
            crate::IconPack::new(br#"{"icons":{}}"#).with_registration_name(&registration_name)
        ])
        .expect_err("registration-name limit + 1 must fail through the binding seam");

        assert_eq!(error.status(), crate::BindingStatus::ResourceLimitExceeded);
        let resource = error.resource_details().expect("resource details");
        assert_eq!(
            resource.limit_id,
            crate::IconRegistryResourceLimitId::MaxPrefixBytes.stable_id()
        );
        assert_eq!(resource.actual, u64::try_from(maximum + 1).unwrap());
        assert_eq!(resource.max, u64::try_from(maximum).unwrap());
        assert_eq!(resource.profile, "constructor-fixed");

        let icon = error.icon_registry_details().expect("icon details");
        assert_eq!(icon.kind_id, "resource_limit_exceeded");
        assert_eq!(icon.pack_index, Some(0));
        assert_eq!(icon.registration_name, None);

        let payload: serde_json::Value =
            serde_json::from_slice(&crate::binding_error_payload_json_bytes(&error))
                .expect("binding error JSON");
        assert_eq!(
            payload["details"]["resource"]["limit_id"],
            resource.limit_id
        );
        assert_eq!(payload["details"]["icon_registry"]["kind_id"], icon.kind_id);
        for duplicate in ["limit_id", "actual", "maximum"] {
            assert!(
                payload["details"]["icon_registry"].get(duplicate).is_none(),
                "icon-specific details must not duplicate resource field {duplicate:?}"
            );
        }
        assert!(!payload.to_string().contains(&registration_name));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn canonical_icon_factory_maps_content_and_allocation_failures_by_ownership() {
        let invalid = build_icon_registry([crate::IconPack::new(
            br#"{"prefix":"test","icons":{"bad":{"body":"<path>"}}}"#,
        )])
        .expect_err("invalid XML must fail through the canonical factory");
        assert_eq!(invalid.status(), crate::BindingStatus::InvalidArgument);
        assert!(invalid.resource_details().is_none());
        assert_eq!(
            invalid
                .icon_registry_details()
                .map(|details| details.kind_id),
            Some("invalid_xml")
        );

        assert_eq!(
            crate::common::icon_registry_error_status(
                merman::svg::IconRegistryBuildErrorKind::AllocationFailed,
            ),
            crate::BindingStatus::InternalError
        );
        assert_eq!(
            crate::common::icon_registry_error_status(
                merman::svg::IconRegistryBuildErrorKind::ArithmeticOverflow,
            ),
            crate::BindingStatus::InternalError
        );

        let invalid_utf8 = build_icon_registry([crate::IconPack::new(&[0xff])])
            .expect_err("invalid pack UTF-8 must retain the transport-wide UTF-8 status");
        assert_eq!(invalid_utf8.status(), crate::BindingStatus::Utf8Error);
        assert_eq!(
            invalid_utf8
                .icon_registry_details()
                .map(|details| details.kind_id),
            Some("invalid_utf8")
        );
    }
}
