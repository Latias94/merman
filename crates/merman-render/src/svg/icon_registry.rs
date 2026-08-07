mod error;
mod ingest;
mod limits;
mod lookup;
mod pack;
mod render;
mod xml;

pub use error::{IconRegistryBuildError, IconRegistryBuildErrorKind};
pub use limits::{
    IconRegistryResourceLimitDescriptor, IconRegistryResourceLimitId,
    icon_registry_resource_limit_descriptors,
};
pub use pack::IconPack;

use ingest::{BuildUsage, ParsedPack, ResolvedIcon};
use limits::IconRegistryBuildLimits;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

const MERMAID_UNKNOWN_ICON_BODY: &str = r#"<g><rect width="80" height="80" style="fill: #087ebf; stroke-width: 0px;"/><text transform="translate(21.16 64.67)" style="fill: #fff; font-family: ArialMT, Arial; font-size: 67.75px;"><tspan x="0" y="0">?</tspan></text></g>"#;

pub(in crate::svg) fn mermaid_unknown_icon_svg(
    width: impl fmt::Display,
    height: impl fmt::Display,
) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 80 80">{MERMAID_UNKNOWN_ICON_BODY}</svg>"#
    )
}

/// Consuming transaction that validates borrowed IconifyJSON packs before publishing a registry.
///
/// `add_pack` consumes `self`: a failed pack cannot leave a partially reusable builder. A
/// successful add owns only parsed state, so the caller may release its pack buffer immediately.
pub struct IconRegistryBuilder {
    limits: IconRegistryBuildLimits,
    input_bytes: usize,
    usage: BuildUsage,
    packs: Vec<ParsedPack>,
}

impl fmt::Debug for IconRegistryBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IconRegistryBuilder")
            .field("pack_count", &self.packs.len())
            .field("input_bytes", &self.input_bytes)
            .field("usage", &self.usage)
            .finish_non_exhaustive()
    }
}

impl IconRegistryBuilder {
    pub fn new() -> Self {
        Self::with_limits(IconRegistryBuildLimits::fixed())
    }

    fn with_limits(limits: IconRegistryBuildLimits) -> Self {
        Self {
            limits,
            input_bytes: 0,
            usage: BuildUsage::default(),
            packs: Vec::new(),
        }
    }

    pub fn add_pack(mut self, pack: IconPack<'_>) -> Result<Self, IconRegistryBuildError> {
        let pack_index = self.packs.len();
        let registration_name =
            ingest::validate_registration_name(pack.registration_name(), pack_index, &self.limits)?;
        let next_pack_count = pack_index.checked_add(1).ok_or_else(|| {
            build_arithmetic_error(Some(pack_index), "icon pack count overflowed")
        })?;
        if next_pack_count > self.limits.max_packs {
            return Err(build_limit_error(
                Some(pack_index),
                IconRegistryResourceLimitId::MaxPacks,
                next_pack_count,
                self.limits.max_packs,
                "icon pack count exceeds the fixed registry ceiling",
            )
            .with_registration_name(registration_name));
        }

        let json = pack.json();
        if json.len() > self.limits.max_pack_bytes {
            return Err(build_limit_error(
                Some(pack_index),
                IconRegistryResourceLimitId::MaxPackBytes,
                json.len(),
                self.limits.max_pack_bytes,
                "icon pack bytes exceed the fixed per-pack ceiling",
            )
            .with_registration_name(registration_name));
        }
        let next_input_bytes = self.input_bytes.checked_add(json.len()).ok_or_else(|| {
            build_arithmetic_error(Some(pack_index), "aggregate icon pack bytes overflowed")
        })?;
        if next_input_bytes > self.limits.max_input_bytes {
            return Err(build_limit_error(
                Some(pack_index),
                IconRegistryResourceLimitId::MaxInputBytes,
                next_input_bytes,
                self.limits.max_input_bytes,
                "aggregate icon pack bytes exceed the fixed registry ceiling",
            )
            .with_registration_name(registration_name));
        }

        let parsed = ingest::parse_pack(
            json,
            registration_name,
            pack_index,
            self.limits,
            &mut self.usage,
        )
        .map_err(|error| error.with_registration_name(registration_name))?;
        self.packs.try_reserve(1).map_err(|_| {
            build_allocation_error(Some(pack_index), "icon pack staging allocation failed")
        })?;
        self.packs.push(parsed);
        self.input_bytes = next_input_bytes;
        Ok(self)
    }

    pub fn build(mut self) -> Result<IconRegistry, IconRegistryBuildError> {
        let resolved_packs = ingest::resolve_packs(self.packs, &mut self.usage)?;
        let mut icons = HashMap::new();
        icons.try_reserve(self.usage.entries).map_err(|_| {
            build_allocation_error(None, "resolved icon registry allocation failed")
        })?;

        for (pack_index, pack) in resolved_packs.into_iter().enumerate() {
            let ingest::ResolvedPack {
                prefix,
                registration_name,
                icons: pack_icons,
            } = pack;
            for (name, icon) in pack_icons {
                let key = lookup::canonical_key(&prefix, &name);
                if icons.insert(key, icon).is_some() {
                    return Err(IconRegistryBuildError::new(
                        IconRegistryBuildErrorKind::DuplicateIcon,
                        Some(pack_index),
                        "canonical icon name is defined by more than one pack",
                    )
                    .with_registration_name(registration_name.as_deref()));
                }
            }
        }

        Ok(IconRegistry {
            inner: Arc::new(IconRegistryInner { icons }),
        })
    }
}

impl Default for IconRegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

struct IconRegistryInner {
    icons: HashMap<String, ResolvedIcon>,
}

/// Immutable, cheaply cloneable registry of fully validated Iconify icons and aliases.
#[derive(Clone)]
pub struct IconRegistry {
    inner: Arc<IconRegistryInner>,
}

impl fmt::Debug for IconRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IconRegistry")
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

impl IconRegistry {
    pub fn from_packs<'a>(
        packs: impl IntoIterator<Item = IconPack<'a>>,
    ) -> Result<Self, IconRegistryBuildError> {
        let mut builder = IconRegistryBuilder::new();
        for pack in packs {
            builder = builder.add_pack(pack)?;
        }
        builder.build()
    }

    pub fn len(&self) -> usize {
        self.inner.icons.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.icons.is_empty()
    }

    pub(in crate::svg) fn render_icon(
        &self,
        request: IconRenderRequest<'_>,
    ) -> crate::Result<Option<String>> {
        let Some(key) = lookup::resolve_icon_key(request.icon_name, request.fallback_prefix) else {
            return Ok(None);
        };
        self.inner
            .icons
            .get(&key)
            .map(|icon| render::render_resolved_icon(icon, &request))
            .transpose()
    }
}

pub(in crate::svg) struct IconRenderRequest<'a> {
    pub(in crate::svg) icon_name: &'a str,
    pub(in crate::svg) width_px: f64,
    pub(in crate::svg) height_px: f64,
    pub(in crate::svg) fallback_prefix: Option<&'a str>,
    pub(in crate::svg) extra_class: Option<&'a str>,
    pub(in crate::svg) id_scope: &'a str,
    pub(in crate::svg) effective_config: &'a merman_core::MermaidConfig,
    pub(in crate::svg) work_meter: &'a crate::resources::OperationWorkMeter,
}

/// Strictly scopes IDs in a trusted built-in SVG fragment without a textual parse fallback.
#[cfg(feature = "layout-cytoscape")]
pub(in crate::svg) fn scope_svg_internal_ids(body: &str, scope: &str) -> crate::Result<String> {
    let validated =
        xml::ValidatedIconBody::parse(body.to_owned(), 0, &IconRegistryBuildLimits::fixed())
            .map_err(|_| crate::Error::icon_processing("built-in icon fragment is invalid"))?;
    validated
        .scope(scope)
        .map_err(|_| crate::Error::icon_processing("built-in icon ID scoping failed"))
}

fn build_limit_error(
    pack_index: Option<usize>,
    id: IconRegistryResourceLimitId,
    actual: usize,
    maximum: usize,
    message: &'static str,
) -> IconRegistryBuildError {
    IconRegistryBuildError::new(
        IconRegistryBuildErrorKind::ResourceLimitExceeded,
        pack_index,
        message,
    )
    .with_limit(id, actual, maximum)
}

fn build_arithmetic_error(
    pack_index: Option<usize>,
    message: &'static str,
) -> IconRegistryBuildError {
    IconRegistryBuildError::new(
        IconRegistryBuildErrorKind::ArithmeticOverflow,
        pack_index,
        message,
    )
}

fn build_allocation_error(
    pack_index: Option<usize>,
    message: &'static str,
) -> IconRegistryBuildError {
    IconRegistryBuildError::new(
        IconRegistryBuildErrorKind::AllocationFailed,
        pack_index,
        message,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_pack_and_input_byte_limits_fail_before_json_decoding() {
        let first = br#"{"prefix":"one","icons":{}}"#;
        let second = br#"{"prefix":"two","icons":{}}"#;

        let mut limits = IconRegistryBuildLimits::fixed();
        limits.max_pack_bytes = first.len();
        limits.max_input_bytes = first.len() + second.len();
        let exact = IconRegistryBuilder::with_limits(limits)
            .add_pack(IconPack::new(first))
            .expect("exact per-pack bytes")
            .add_pack(IconPack::new(second))
            .expect("exact aggregate bytes")
            .build()
            .expect("two empty valid packs build");
        assert!(exact.is_empty());

        let oversized_pack = vec![b'x'; first.len() + 1];
        let per_pack = IconRegistryBuilder::with_limits(limits)
            .add_pack(IconPack::new(&oversized_pack))
            .expect_err("per-pack bytes are checked before UTF-8 or JSON");
        assert_eq!(
            per_pack.limit_id(),
            Some(IconRegistryResourceLimitId::MaxPackBytes.stable_id())
        );

        let mut aggregate_limits = limits;
        aggregate_limits.max_input_bytes = first.len() + 1;
        let aggregate = IconRegistryBuilder::with_limits(aggregate_limits)
            .add_pack(IconPack::new(first))
            .expect("first pack is within the aggregate ceiling")
            .add_pack(IconPack::new(&[0xff, 0xff]))
            .expect_err("aggregate bytes are checked before second-pack UTF-8 or JSON");
        assert_eq!(
            aggregate.limit_id(),
            Some(IconRegistryResourceLimitId::MaxInputBytes.stable_id())
        );
        assert_eq!(aggregate.actual(), u64::try_from(first.len() + 2).ok());
        assert_eq!(aggregate.maximum(), u64::try_from(first.len() + 1).ok());
    }

    #[test]
    fn fixed_pack_count_accepts_exact_and_rejects_plus_one_before_decoding() {
        let maximum = usize::try_from(IconRegistryResourceLimitId::MaxPacks.fixed_value())
            .expect("fixed pack count fits usize");

        let mut exact = IconRegistryBuilder::new();
        for index in 0..maximum {
            let json = format!(r#"{{"prefix":"p{index}","icons":{{}}}}"#);
            exact = exact
                .add_pack(IconPack::new(json.as_bytes()))
                .expect("every pack through the exact fixed count is admitted");
        }
        assert!(
            exact
                .build()
                .expect("exact fixed pack count builds")
                .is_empty()
        );

        let mut overflow = IconRegistryBuilder::new();
        for index in 0..maximum {
            let json = format!(r#"{{"prefix":"q{index}","icons":{{}}}}"#);
            overflow = overflow
                .add_pack(IconPack::new(json.as_bytes()))
                .expect("every pack through the exact fixed count is admitted");
        }
        let error = overflow
            .add_pack(IconPack::new(&[0xff]))
            .expect_err("pack count plus one is rejected before UTF-8 or JSON decoding");
        assert_eq!(error.limit(), Some(IconRegistryResourceLimitId::MaxPacks));
        assert_eq!(error.actual(), u64::try_from(maximum + 1).ok());
        assert_eq!(error.maximum(), u64::try_from(maximum).ok());
    }
}
