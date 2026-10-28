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
    #[cfg(feature = "native-fonts")]
    native_font_context: Option<merman::svg::export::NativeFontContext>,
    #[cfg(feature = "svg")]
    icon_registry: Option<BindingIconRegistry>,
}

impl std::fmt::Debug for BindingEngineServices {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = formatter.debug_struct("BindingEngineServices");
        #[cfg(feature = "svg")]
        debug.field("host_text_measurer", &self.host_text_measurer.is_some());
        #[cfg(feature = "native-fonts")]
        debug.field("native_font_context", &self.native_font_context.is_some());
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
            #[cfg(feature = "native-fonts")]
            native_font_context: None,
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

    /// Installs already loaded fonts for operation-local measurement and native painting.
    ///
    /// Engine construction rejects a simultaneous host text measurer or explicit JSON text
    /// measurement selector. No font source is acquired while installing this service.
    #[cfg(feature = "native-fonts")]
    #[must_use]
    pub fn with_native_font_context(
        mut self,
        context: merman::svg::export::NativeFontContext,
    ) -> Self {
        self.native_font_context = Some(context);
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
            let empty = self.host_text_measurer.is_none() && self.icon_registry.is_none();
            #[cfg(feature = "native-fonts")]
            let empty = empty && self.native_font_context.is_none();
            empty
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
        #[cfg(feature = "native-fonts")]
        if self.native_font_context.is_some() {
            if self.host_text_measurer.is_some() {
                return Err(BindingError::invalid_argument(
                    "constructor service `native-font-context` conflicts with constructor service `host-text-measurement`",
                ));
            }
            if options.text_measurement_selector_explicit {
                return Err(BindingError::invalid_argument(
                    "constructor service `native-font-context` conflicts with explicit option `environment.text_measurement`",
                ));
            }
        }
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

    #[cfg(feature = "native-fonts")]
    pub(crate) fn native_font_context(&self) -> Option<merman::svg::export::NativeFontContext> {
        self.native_font_context.clone()
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
                #[cfg(feature = "native-fonts")]
                self.native_font_context
                    .is_some()
                    .then_some(ConstructorServiceKey::NativeFontContext),
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

/// Admits already loaded packs from `[{"json":"...","registrationName":"optional"}]`.
///
/// This transport envelope does not parse Iconify content or acquire pack sources. All pack
/// content and fixed constructor limits remain owned by [`build_icon_registry`].
#[cfg(feature = "svg")]
pub fn build_icon_registry_from_json(
    packs_json: &[u8],
) -> Result<BindingIconRegistry, BindingError> {
    use crate::IconRegistryResourceLimitId as Limit;
    use serde::de::{SeqAccess, Visitor};
    use serde::{Deserialize, Deserializer};

    // JSON escaping needs at most six bytes per input byte. Include only the bounded array,
    // field names, registration names, and separators around the existing pack byte budget.
    const MAX_PACKS: usize = Limit::MaxPacks.fixed_value() as usize;
    const MAX_ENVELOPE_BYTES: usize = 6
        * (Limit::MaxInputBytes.fixed_value() as usize
            + MAX_PACKS * Limit::MaxPrefixBytes.fixed_value() as usize)
        + MAX_PACKS * br#"{"json":"","registrationName":""},"#.len()
        + 2;
    if packs_json.len() > MAX_ENVELOPE_BYTES {
        return Err(BindingError::invalid_argument(
            "icon pack JSON envelope exceeds the capacity derived from fixed registry limits",
        ));
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    struct JsonPack {
        json: String,
        registration_name: Option<String>,
    }

    struct PacksVisitor<'a> {
        exceeded: &'a std::cell::Cell<bool>,
    }
    impl<'de> Visitor<'de> for PacksVisitor<'_> {
        type Value = Vec<JsonPack>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an array of already loaded icon packs")
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
            let mut packs = Vec::new();
            while let Some(pack) = sequence.next_element()? {
                packs.push(pack);
                if packs.len() > MAX_PACKS {
                    self.exceeded.set(true);
                    return Err(serde::de::Error::custom(
                        "icon pack count exceeds its fixed ceiling",
                    ));
                }
            }
            Ok(packs)
        }
    }

    let invalid_envelope = |_| {
        BindingError::invalid_argument(
            "invalid icon pack JSON envelope; expected an array of {json, registrationName?}",
        )
    };
    let mut deserializer = serde_json::Deserializer::from_slice(packs_json);
    let exceeded = std::cell::Cell::new(false);
    let packs = deserializer.deserialize_seq(PacksVisitor {
        exceeded: &exceeded,
    });
    if exceeded.get() {
        return Err(BindingError::icon_registry_resource_limit(
            Limit::MaxPacks,
            (MAX_PACKS + 1) as u64,
            Some(MAX_PACKS),
            "icon pack count exceeds the fixed registry ceiling",
        ));
    }
    let packs = packs.map_err(invalid_envelope)?;
    deserializer.end().map_err(invalid_envelope)?;
    build_icon_registry(packs.iter().map(|pack| {
        let icon_pack = crate::IconPack::new(pack.json.as_bytes());
        match pack.registration_name.as_deref() {
            Some(name) => icon_pack.with_registration_name(name),
            None => icon_pack,
        }
    }))
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
    fn json_icon_admission_rejects_envelope_shape_and_preserves_pack_errors() {
        for invalid in [
            r#"{}"#,
            r#"[{"json":{}}]"#,
            r#"[{"json":"{}","url":"https://example.invalid/pack.json"}]"#,
            r#"[{"json":"{}","json":"{}"}]"#,
            r#"[] true"#,
        ] {
            let error = build_icon_registry_from_json(invalid.as_bytes()).unwrap_err();
            assert_eq!(error.status(), crate::BindingStatus::InvalidArgument);
        }
        assert!(build_icon_registry_from_json(b"[]").unwrap().is_empty());

        for json in [
            r#"{"prefix":"test","icons":{"bad":{"body":"<path>"}}}"#,
            r#"{"prefix":"test","icons":{},"icons":{}}"#,
        ] {
            let wire = serde_json::to_vec(&serde_json::json!([{
                "json": json, "registrationName": "fleet",
            }]))
            .unwrap();
            let from_json = build_icon_registry_from_json(&wire).unwrap_err();
            let direct = build_icon_registry([
                crate::IconPack::new(json.as_bytes()).with_registration_name("fleet")
            ])
            .unwrap_err();
            assert_eq!(from_json, direct);
        }
    }

    #[cfg(feature = "svg")]
    #[test]
    fn json_icon_admission_bounds_pack_count_and_preserves_prefix_limits() {
        let maximum = crate::IconRegistryResourceLimitId::MaxPacks.fixed_value() as usize;
        let empty = serde_json::json!({"json": r#"{"prefix":"test","icons":{}}"#});
        let exact = serde_json::to_vec(&vec![empty.clone(); maximum]).unwrap();
        assert!(build_icon_registry_from_json(&exact).unwrap().is_empty());
        for count in [maximum + 1, maximum + 2, maximum * 2] {
            let too_many = serde_json::to_vec(&vec![empty.clone(); count]).unwrap();
            let error = build_icon_registry_from_json(&too_many).unwrap_err();
            assert_eq!(error.status(), crate::BindingStatus::ResourceLimitExceeded);
            let resource = error.resource_details().unwrap();
            assert_eq!(resource.limit_id, "max_icon_registry_packs");
            assert_eq!(resource.actual, (maximum + 1) as u64);
            assert_eq!(resource.max, maximum as u64);
            assert_eq!(resource.profile, "constructor-fixed");
            assert_eq!(
                error.icon_registry_details().unwrap().pack_index,
                Some(maximum as u64)
            );
        }

        let maximum = crate::IconRegistryResourceLimitId::MaxPrefixBytes.fixed_value() as usize;
        for length in [maximum, maximum + 1] {
            let wire = serde_json::to_vec(&serde_json::json!([{
                "json": r#"{"icons":{}}"#, "registrationName": "a".repeat(length),
            }]))
            .unwrap();
            let result = build_icon_registry_from_json(&wire);
            if length == maximum {
                assert!(result.unwrap().is_empty());
            } else {
                let error = result.unwrap_err();
                assert_eq!(
                    error.resource_details().unwrap().limit_id,
                    "max_icon_registry_prefix_bytes"
                );
                assert_eq!(
                    error.icon_registry_details().unwrap().registration_name,
                    None
                );
            }
        }
    }

    #[cfg(all(feature = "svg", feature = "diagram-flowchart"))]
    #[test]
    fn json_icon_services_render_with_registration_override_and_engine_isolation() {
        let json = r#"{"prefix":"original","icons":{"rocket":{"body":"<path data-icon=\"json-service\" d=\"M0 0H16V16H0z\"/>"}}}"#;
        let wire = serde_json::to_vec(&serde_json::json!([{
            "json": json, "registrationName": "fleet",
        }]))
        .unwrap();
        let registry = build_icon_registry_from_json(&wire).unwrap();
        drop(wire);
        let engine = crate::BindingEngine::from_options_and_services(
            b"",
            BindingEngineServices::new().with_icon_registry(registry),
        )
        .unwrap();
        let plain = crate::BindingEngine::new(b"").unwrap();
        let source = b"flowchart TD\nA@{ icon: \"fleet:rocket\", label: \"A\" }";
        let render = |engine: &crate::BindingEngine, options: &[u8]| {
            String::from_utf8(
                engine
                    .execute(
                        crate::BindingOperationRequest::new("svg", source)
                            .with_options_json(options),
                    )
                    .unwrap()
                    .into_data(),
            )
            .unwrap()
        };
        assert!(render(&engine, b"").contains("json-service"));
        assert!(!render(&plain, b"").contains("json-service"));
        assert!(render(&engine, br#"{"svg":{"diagram_id":"override"}}"#).contains("json-service"));
        drop(engine);
        assert!(!render(&plain, b"").contains("json-service"));
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
