mod error;
mod ingest;
mod limits;
mod lookup;
mod pack;
mod xml;

pub use error::{IconRegistryBuildError, IconRegistryBuildErrorKind};
pub use ingest::ResolvedIcon;
pub use limits::{
    IconRegistryResourceLimitDescriptor, IconRegistryResourceLimitId,
    icon_registry_resource_limit_descriptors,
};
pub use lookup::parse_icon_reference;
pub use pack::IconPack;
pub use xml::{IconDefsPlacement, ParsedIconBody, UntrustedIconXml};

use crate::resources::{OperationWorkError, OperationWorkMeter};
use ingest::{BuildUsage, ParsedPack};
use limits::IconRegistryBuildLimits;
use merman_model::OperationPhase;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

const ICON_ID_SCOPE_HASH_OFFSET: u64 = 0xcbf29ce484222325;
const ICON_ID_SCOPE_HASH_PRIME: u64 = 0x100000001b3;
const ICON_ID_SCOPE_CHECKPOINT_BYTES: usize = 4 * 1024;
const MAX_ICON_ID_SCOPE_PARTS: usize = 4;

/// Precomputed identity used to scope internal IDs in one rendered icon.
///
/// Callers cannot provide an arbitrary string: the scope must first pass through the controlled
/// prefix builder, which charges and checkpoints every scanned byte before icon materialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconIdScope(u64);

impl IconIdScope {
    const fn hash(self) -> u64 {
        self.0
    }
}

/// Incremental FNV-1a state for a family-owned icon scope prefix.
///
/// Reusing this value preserves the historical hash of the concatenated textual scope while
/// avoiding a per-icon clone and rescan of the complete diagram ID.
/// Each call accepts at most four parts. An empty suffix still observes the work meter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconIdScopePrefix(u64);

impl IconIdScopePrefix {
    pub fn from_parts<const N: usize>(
        parts: &[&str; N],
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        controlled_hash_icon_scope_parts(ICON_ID_SCOPE_HASH_OFFSET, parts, work_meter).map(Self)
    }

    pub fn extend_parts<const N: usize>(
        self,
        parts: &[&str; N],
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        controlled_hash_icon_scope_parts(self.0, parts, work_meter).map(Self)
    }

    pub fn scope_parts<const N: usize>(
        self,
        parts: &[&str; N],
        work_meter: &OperationWorkMeter,
    ) -> Result<IconIdScope, OperationWorkError> {
        self.extend_parts(parts, work_meter)
            .map(|prefix| IconIdScope(prefix.0))
    }
}

fn controlled_hash_icon_scope_parts<const N: usize>(
    mut hash: u64,
    parts: &[&str; N],
    work_meter: &OperationWorkMeter,
) -> Result<u64, OperationWorkError> {
    const {
        assert!(
            N <= MAX_ICON_ID_SCOPE_PARTS,
            "an icon ID scope accepts at most four parts"
        )
    };
    let scan_bytes = parts.iter().try_fold(0usize, |total, part| {
        total
            .checked_add(part.len())
            .ok_or_else(|| work_meter.arithmetic_overflow())
    })?;

    // Reject the complete scan before touching caller-sized bytes. No scope buffer is allocated.
    work_meter.charge(scan_bytes)?;
    work_meter.checkpoint(OperationPhase::Emit)?;
    for part in parts {
        for chunk in part.as_bytes().chunks(ICON_ID_SCOPE_CHECKPOINT_BYTES) {
            for byte in chunk {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(ICON_ID_SCOPE_HASH_PRIME);
            }
            work_meter.checkpoint(OperationPhase::Emit)?;
        }
    }
    Ok(hash)
}

#[cfg(test)]
fn icon_id_scope_for_test(value: &str) -> IconIdScope {
    let hash = value
        .as_bytes()
        .iter()
        .fold(ICON_ID_SCOPE_HASH_OFFSET, |mut hash, byte| {
            hash ^= u64::from(*byte);
            hash.wrapping_mul(ICON_ID_SCOPE_HASH_PRIME)
        });
    IconIdScope(hash)
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
        let mut prefixes = HashSet::new();
        icons.try_reserve(self.usage.entries).map_err(|_| {
            build_allocation_error(None, "resolved icon registry allocation failed")
        })?;

        for (pack_index, pack) in resolved_packs.into_iter().enumerate() {
            let ingest::ResolvedPack {
                prefix,
                registration_name,
                icons: pack_icons,
            } = pack;
            prefixes.insert(prefix.clone());
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
            inner: Arc::new(IconRegistryInner { icons, prefixes }),
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
    prefixes: HashSet<String>,
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

    pub fn is_pack_registered(&self, prefix: &str) -> bool {
        self.inner.prefixes.contains(prefix)
    }

    /// Looks up structurally admitted icon data for the renderer's terminal SVG policy.
    /// The parsed body still requires the renderer's sanitizer and output validation.
    #[doc(hidden)]
    pub fn resolve_icon(
        &self,
        icon_name: &str,
        fallback_prefix: Option<&str>,
    ) -> Option<&ResolvedIcon> {
        let (prefix, name) = parse_icon_reference(icon_name, fallback_prefix)?;
        self.inner.icons.get(&lookup::canonical_key(prefix, name))
    }
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
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};

    #[test]
    fn incremental_icon_scope_preserves_the_legacy_concatenated_hash() {
        assert_eq!(
            icon_id_scope_for_test("diagram-a").hash(),
            0xb5cb_5676_c393_2910
        );
        let diagram_id = "diagram-".repeat(1_024);
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let prefix =
            IconIdScopePrefix::from_parts(&[diagram_id.as_str(), "-flowchart-icon-"], &work_meter)
                .expect("scope prefix is admitted");
        let first = prefix
            .scope_parts(&["node-a"], &work_meter)
            .expect("first node scope is admitted");
        let second = prefix
            .scope_parts(&["node-b"], &work_meter)
            .expect("second node scope is admitted");

        assert_eq!(
            first,
            icon_id_scope_for_test(&format!("{diagram_id}-flowchart-icon-node-a"))
        );
        assert_eq!(
            second,
            icon_id_scope_for_test(&format!("{diagram_id}-flowchart-icon-node-b"))
        );
        assert_eq!(
            work_meter.used(),
            diagram_id.len() + "-flowchart-icon-".len() + "node-a".len() + "node-b".len()
        );
    }

    #[test]
    fn icon_scope_scan_admits_the_complete_fragment_before_hashing() {
        let parts = ["diagram", "-service-", "node", "-icon"];
        let exact_work = parts.iter().map(|part| part.len()).sum::<usize>();
        let exact_meter = OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, exact_work)
                .expect("configure exact work ceiling"),
        );
        IconIdScopePrefix::from_parts(&parts, &exact_meter)
            .expect("the exact scope scan is admitted");
        assert_eq!(exact_meter.used(), exact_work);

        let rejected_meter = OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
                .expect("configure N-1 work ceiling"),
        );
        IconIdScopePrefix::from_parts(&parts, &rejected_meter)
            .expect_err("N-1 must fail before the scope bytes are scanned");
        assert_eq!(rejected_meter.used(), 0);
    }

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
