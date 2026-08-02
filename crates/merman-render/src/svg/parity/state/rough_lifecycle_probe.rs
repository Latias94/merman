use super::*;
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use std::cell::{Cell, RefCell};
use std::sync::{Mutex, MutexGuard, OnceLock};

pub(super) const STATE_ROUGH_LIFECYCLE_RECEIPT_MARKER: &str =
    "MERMAN_STATE_ROUGH_LIFECYCLE_RECEIPT_V1=";
const STATE_ROUGH_LIFECYCLE_SCHEMA: &str = "merman.state_rough_lifecycle.v1";
const OWNED_BYTES_DEFINITION: &str = "sum_of_cached_string_capacities";
const CONFIGURED_ZERO_CONTRACT: &str =
    "configured_hand_drawn_seed_zero_resolves_to_operation_seed_before_cache_bypass";
const LONG_LIVED_REQUEST_COUNT: usize = 2_048;
const LONG_LIVED_REQUEST_CHECKPOINTS: [usize; 6] = [1, 16, 64, 256, 1_024, 2_048];
const LONG_LIVED_GEOMETRY_LABEL_BYTES: [usize; 8] = [1, 2, 4, 8, 16, 32, 64, 128];

thread_local! {
    static STATE_ROUGH_LIFECYCLE_CAPTURE_DEPTH: Cell<u32> = const { Cell::new(0) };
    static STATE_ROUGH_LIFECYCLE_SNAPSHOTS: RefCell<Vec<StateRoughLifecycleOperationSnapshot>> =
        const { RefCell::new(Vec::new()) };
    static STATE_ROUGH_LIFECYCLE_PANIC_AFTER_CACHE_POPULATION: Cell<bool> = const { Cell::new(false) };
}

fn lifecycle_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn with_lifecycle_snapshots<R>(
    f: impl FnOnce(&mut Vec<StateRoughLifecycleOperationSnapshot>) -> R,
) -> R {
    STATE_ROUGH_LIFECYCLE_SNAPSHOTS.with(|snapshots| f(&mut snapshots.borrow_mut()))
}

fn lifecycle_capture_is_enabled() -> bool {
    STATE_ROUGH_LIFECYCLE_CAPTURE_DEPTH.with(|depth| depth.get() != 0)
}

pub(super) struct StateRoughLifecycleCaptureGuard;

impl Drop for StateRoughLifecycleCaptureGuard {
    fn drop(&mut self) {
        STATE_ROUGH_LIFECYCLE_CAPTURE_DEPTH.with(|depth| {
            depth.set(
                depth
                    .get()
                    .checked_sub(1)
                    .expect("State Rough lifecycle capture depth should be balanced"),
            );
        });
    }
}

#[allow(dead_code)]
pub(super) struct StateRoughLifecyclePanicGuard;

impl Drop for StateRoughLifecyclePanicGuard {
    fn drop(&mut self) {
        STATE_ROUGH_LIFECYCLE_PANIC_AFTER_CACHE_POPULATION.with(|enabled| enabled.set(false));
    }
}

#[allow(dead_code)]
pub(super) fn state_rough_lifecycle_panic_after_cache_population() -> StateRoughLifecyclePanicGuard
{
    STATE_ROUGH_LIFECYCLE_PANIC_AFTER_CACHE_POPULATION.with(|enabled| {
        assert!(
            !enabled.replace(true),
            "State Rough panic hook must not nest"
        );
    });
    StateRoughLifecyclePanicGuard
}

pub(super) fn state_rough_lifecycle_capture() -> StateRoughLifecycleCaptureGuard {
    STATE_ROUGH_LIFECYCLE_CAPTURE_DEPTH.with(|depth| {
        depth.set(
            depth
                .get()
                .checked_add(1)
                .expect("State Rough lifecycle capture depth should remain bounded"),
        );
    });
    StateRoughLifecycleCaptureGuard
}

pub(super) fn state_rough_lifecycle_probe_reset() {
    with_lifecycle_snapshots(Vec::clear);
    state_rough_cache_clear_for_probe();
}

pub(super) fn state_rough_lifecycle_take_snapshots() -> Vec<StateRoughLifecycleOperationSnapshot> {
    with_lifecycle_snapshots(std::mem::take)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub(super) struct StateRoughGeometryCounters {
    pub(super) draw_requests: usize,
    pub(super) operation_lookups: usize,
    pub(super) operation_hits: usize,
    pub(super) operation_misses: usize,
    pub(super) operation_builds: usize,
    pub(super) tls_hits: usize,
    pub(super) global_hits: usize,
    pub(super) bypass_builds: usize,
}

impl StateRoughGeometryCounters {
    fn checked_add_assign(&mut self, other: Self) {
        self.draw_requests = self
            .draw_requests
            .checked_add(other.draw_requests)
            .expect("State Rough draw request rollup should remain bounded");
        self.operation_lookups = self
            .operation_lookups
            .checked_add(other.operation_lookups)
            .expect("State Rough operation lookup rollup should remain bounded");
        self.operation_hits = self
            .operation_hits
            .checked_add(other.operation_hits)
            .expect("State Rough operation hit rollup should remain bounded");
        self.operation_misses = self
            .operation_misses
            .checked_add(other.operation_misses)
            .expect("State Rough operation miss rollup should remain bounded");
        self.operation_builds = self
            .operation_builds
            .checked_add(other.operation_builds)
            .expect("State Rough operation build rollup should remain bounded");
        self.tls_hits = self
            .tls_hits
            .checked_add(other.tls_hits)
            .expect("State Rough TLS hit rollup should remain bounded");
        self.global_hits = self
            .global_hits
            .checked_add(other.global_hits)
            .expect("State Rough global hit rollup should remain bounded");
        self.bypass_builds = self
            .bypass_builds
            .checked_add(other.bypass_builds)
            .expect("State Rough bypass build rollup should remain bounded");
    }

    fn validate(self, geometry: &str) -> std::result::Result<(), String> {
        let classified_draws = self
            .operation_lookups
            .checked_add(self.bypass_builds)
            .ok_or_else(|| format!("{geometry} draw request identity overflowed"))?;
        if self.draw_requests != classified_draws {
            return Err(format!(
                "{geometry} draw request identity failed: draws={} lookups={} bypass_builds={}",
                self.draw_requests, self.operation_lookups, self.bypass_builds
            ));
        }
        if self.operation_lookups
            != self
                .operation_hits
                .checked_add(self.operation_misses)
                .ok_or_else(|| format!("{geometry} operation lookup identity overflowed"))?
        {
            return Err(format!(
                "{geometry} operation lookup identity failed: lookups={} hits={} misses={}",
                self.operation_lookups, self.operation_hits, self.operation_misses
            ));
        }

        let miss_sources = self
            .tls_hits
            .checked_add(self.global_hits)
            .and_then(|count| count.checked_add(self.operation_builds))
            .ok_or_else(|| format!("{geometry} operation miss source identity overflowed"))?;
        if self.operation_misses != miss_sources {
            return Err(format!(
                "{geometry} operation miss source identity failed: misses={} tls_hits={} global_hits={} builds={}",
                self.operation_misses, self.tls_hits, self.global_hits, self.operation_builds
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub(super) struct StateRoughOperationCounters {
    pub(super) circle: StateRoughGeometryCounters,
    pub(super) paths: StateRoughGeometryCounters,
}

impl StateRoughOperationCounters {
    fn checked_add_assign(&mut self, other: Self) {
        self.circle.checked_add_assign(other.circle);
        self.paths.checked_add_assign(other.paths);
    }

    fn validate(self) -> std::result::Result<(), String> {
        self.circle.validate("circle")?;
        self.paths.validate("paths")
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub(super) struct StateRoughCacheFootprint {
    pub(super) entries: usize,
    pub(super) owned_bytes: usize,
}

impl StateRoughCacheFootprint {
    fn observe_peak(&mut self, entries: usize, owned_bytes: usize) {
        self.entries = self.entries.max(entries);
        self.owned_bytes = self.owned_bytes.max(owned_bytes);
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub(super) struct StateRoughRetainedSnapshot {
    pub(super) global: StateRoughCacheFootprint,
    pub(super) tls: StateRoughCacheFootprint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum StateRoughSeedResolution {
    ConfiguredDeterministic,
    ConfiguredFallbackCapable,
    OperationResolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum StateRoughLifecycleOutcome {
    Success,
    Error,
    Unwind,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct StateRoughLifecycleOperationSnapshot {
    pub(super) configured_seed: f64,
    pub(super) resolved_seed: f64,
    pub(super) seed_resolution: StateRoughSeedResolution,
    pub(super) cache_allowed: bool,
    pub(super) outcome: StateRoughLifecycleOutcome,
    pub(super) counters: StateRoughOperationCounters,
    pub(super) operation_peak: StateRoughCacheFootprint,
    pub(super) post_operation_retained: StateRoughRetainedSnapshot,
}

#[derive(Clone, Copy)]
pub(super) enum StateRoughGeometryKind {
    Circle,
    Paths,
}

#[derive(Default)]
struct StateRoughGeometryCounterCells {
    draw_requests: Cell<usize>,
    operation_lookups: Cell<usize>,
    operation_hits: Cell<usize>,
    operation_misses: Cell<usize>,
    operation_builds: Cell<usize>,
    tls_hits: Cell<usize>,
    global_hits: Cell<usize>,
    bypass_builds: Cell<usize>,
}

impl StateRoughGeometryCounterCells {
    fn snapshot(&self) -> StateRoughGeometryCounters {
        StateRoughGeometryCounters {
            draw_requests: self.draw_requests.get(),
            operation_lookups: self.operation_lookups.get(),
            operation_hits: self.operation_hits.get(),
            operation_misses: self.operation_misses.get(),
            operation_builds: self.operation_builds.get(),
            tls_hits: self.tls_hits.get(),
            global_hits: self.global_hits.get(),
            bypass_builds: self.bypass_builds.get(),
        }
    }
}

pub(super) struct StateRoughLifecycleOperationProbe {
    enabled: bool,
    configured_seed: f64,
    resolved_seed: f64,
    seed_resolution: StateRoughSeedResolution,
    cache_allowed: bool,
    outcome: Cell<StateRoughLifecycleOutcome>,
    circle: StateRoughGeometryCounterCells,
    paths: StateRoughGeometryCounterCells,
    operation_peak_entries: Cell<usize>,
    operation_peak_owned_bytes: Cell<usize>,
}

impl StateRoughLifecycleOperationProbe {
    pub(super) fn new(configured_seed: f64, resolved_seed: f64, cache_allowed: bool) -> Self {
        let seed_resolution = if configured_seed == 0.0 {
            StateRoughSeedResolution::OperationResolved
        } else if cache_allowed {
            StateRoughSeedResolution::ConfiguredDeterministic
        } else {
            StateRoughSeedResolution::ConfiguredFallbackCapable
        };

        Self {
            enabled: lifecycle_capture_is_enabled(),
            configured_seed,
            resolved_seed,
            seed_resolution,
            cache_allowed,
            outcome: Cell::new(StateRoughLifecycleOutcome::Error),
            circle: StateRoughGeometryCounterCells::default(),
            paths: StateRoughGeometryCounterCells::default(),
            operation_peak_entries: Cell::new(0),
            operation_peak_owned_bytes: Cell::new(0),
        }
    }

    fn increment(counter: &Cell<usize>, label: &str) {
        counter.set(
            counter
                .get()
                .checked_add(1)
                .unwrap_or_else(|| panic!("State Rough {label} counter should remain bounded")),
        );
    }

    fn geometry(&self, kind: StateRoughGeometryKind) -> &StateRoughGeometryCounterCells {
        match kind {
            StateRoughGeometryKind::Circle => &self.circle,
            StateRoughGeometryKind::Paths => &self.paths,
        }
    }

    pub(super) fn record_draw_request(&self, kind: StateRoughGeometryKind) {
        if self.enabled {
            Self::increment(&self.geometry(kind).draw_requests, "draw request");
        }
    }

    pub(super) fn record_operation_lookup(&self, kind: StateRoughGeometryKind) {
        if self.enabled {
            Self::increment(&self.geometry(kind).operation_lookups, "operation lookup");
        }
    }

    pub(super) fn record_operation_hit(&self, kind: StateRoughGeometryKind) {
        if self.enabled {
            Self::increment(&self.geometry(kind).operation_hits, "operation hit");
        }
    }

    pub(super) fn record_operation_miss(&self, kind: StateRoughGeometryKind) {
        if self.enabled {
            Self::increment(&self.geometry(kind).operation_misses, "operation miss");
        }
    }

    pub(super) fn record_operation_build(&self, kind: StateRoughGeometryKind) {
        if self.enabled {
            Self::increment(&self.geometry(kind).operation_builds, "operation build");
        }
    }

    #[allow(dead_code)]
    pub(super) fn record_tls_hit(&self, kind: StateRoughGeometryKind) {
        if self.enabled {
            Self::increment(&self.geometry(kind).tls_hits, "TLS hit");
        }
    }

    #[allow(dead_code)]
    pub(super) fn record_global_hit(&self, kind: StateRoughGeometryKind) {
        if self.enabled {
            Self::increment(&self.geometry(kind).global_hits, "global hit");
        }
    }

    pub(super) fn record_bypass_build(&self, kind: StateRoughGeometryKind) {
        if self.enabled {
            Self::increment(&self.geometry(kind).bypass_builds, "bypass build");
        }
    }

    pub(super) fn observe_operation_cache(&self, entries: usize, owned_bytes: usize) {
        if !self.enabled {
            return;
        }
        self.operation_peak_entries
            .set(self.operation_peak_entries.get().max(entries));
        self.operation_peak_owned_bytes
            .set(self.operation_peak_owned_bytes.get().max(owned_bytes));
    }

    pub(super) fn mark_success(&self) {
        if self.enabled {
            self.outcome.set(StateRoughLifecycleOutcome::Success);
        }
    }

    fn counters(&self) -> StateRoughOperationCounters {
        StateRoughOperationCounters {
            circle: self.circle.snapshot(),
            paths: self.paths.snapshot(),
        }
    }
}

impl Drop for StateRoughLifecycleOperationProbe {
    fn drop(&mut self) {
        if !self.enabled {
            return;
        }

        let (global_entries, global_owned_bytes, tls_entries, tls_owned_bytes) =
            state_rough_cache_retained_counts();
        let outcome = if std::thread::panicking() {
            StateRoughLifecycleOutcome::Unwind
        } else {
            self.outcome.get()
        };
        let snapshot = StateRoughLifecycleOperationSnapshot {
            configured_seed: self.configured_seed,
            resolved_seed: self.resolved_seed,
            seed_resolution: self.seed_resolution,
            cache_allowed: self.cache_allowed,
            outcome,
            counters: self.counters(),
            operation_peak: StateRoughCacheFootprint {
                entries: self.operation_peak_entries.get(),
                owned_bytes: self.operation_peak_owned_bytes.get(),
            },
            post_operation_retained: StateRoughRetainedSnapshot {
                global: StateRoughCacheFootprint {
                    entries: global_entries,
                    owned_bytes: global_owned_bytes,
                },
                tls: StateRoughCacheFootprint {
                    entries: tls_entries,
                    owned_bytes: tls_owned_bytes,
                },
            },
        };
        with_lifecycle_snapshots(|snapshots| snapshots.push(snapshot));
    }
}

pub(super) fn state_rough_lifecycle_observe_operation_cache(ctx: &StateRenderCtx<'_>) {
    let (entries, owned_bytes) = ctx.rough_cache.footprint();
    ctx.rough_lifecycle_probe
        .observe_operation_cache(entries, owned_bytes);
    STATE_ROUGH_LIFECYCLE_PANIC_AFTER_CACHE_POPULATION.with(|enabled| {
        if enabled.replace(false) {
            panic!("State Rough lifecycle probe unwind after cache population");
        }
    });
}

#[derive(Debug, Clone, Serialize)]
struct StateRoughLifecycleContracts {
    owned_bytes: &'static str,
    configured_seed_zero: &'static str,
    fallback_capable_configured_seeds: [f64; 2],
}

#[derive(Debug, Clone, Serialize)]
struct StateRoughEngineLifecycleReceipt {
    engine_instances: usize,
    engine_reused_across_requests: bool,
    request_count: usize,
    detailed_request_count: usize,
    long_lived_request_count: usize,
    render_threads: usize,
}

#[derive(Debug, Clone, Serialize)]
struct StateRoughScheduleReceipt {
    same_seed_request_ordinals: Vec<usize>,
    distinct_seed_request_ordinals: Vec<usize>,
    fallback_bypass_request_ordinals: Vec<usize>,
    geometry_label_byte_checkpoints: Vec<usize>,
    request_count_checkpoints: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct StateRoughSvgIdentity {
    bytes: usize,
    elements: usize,
    identity: String,
}

#[derive(Debug, Clone, Serialize)]
struct StateRoughRequestReceipt {
    ordinal: usize,
    case: String,
    render_thread: String,
    geometry_label_bytes: usize,
    ordinary_nodes: usize,
    svg: StateRoughSvgIdentity,
    operation: StateRoughLifecycleOperationSnapshot,
}

#[derive(Debug, Clone, Serialize)]
struct StateRoughRetainedCheckpoint {
    request_count: usize,
    geometry_label_bytes: usize,
    configured_seed: f64,
    retained: StateRoughRetainedSnapshot,
}

#[derive(Debug, Clone, Serialize)]
struct StateRoughLongLivedReceipt {
    request_count: usize,
    request_count_checkpoints: Vec<usize>,
    checkpoints: Vec<StateRoughRetainedCheckpoint>,
    svg: StateRoughSvgIdentity,
    counters: StateRoughOperationCounters,
    max_operation_peak: StateRoughCacheFootprint,
    final_retained: StateRoughRetainedSnapshot,
}

#[derive(Debug, Clone, Serialize)]
struct StateRoughLifecycleRollup {
    svg: StateRoughSvgIdentity,
    counters: StateRoughOperationCounters,
    max_operation_peak: StateRoughCacheFootprint,
    initial_retained: StateRoughRetainedSnapshot,
    final_retained: StateRoughRetainedSnapshot,
    retained_growth: StateRoughRetainedSnapshot,
    operation_cache_reuse_observed: bool,
    legacy_cross_operation_cache_observed: bool,
    configured_zero_operation_resolution_observed: bool,
    fallback_bypass_observed: bool,
}

#[derive(Debug, Clone, Serialize)]
struct StateRoughLifecycleReceipt {
    schema: &'static str,
    contracts: StateRoughLifecycleContracts,
    engine_lifecycle: StateRoughEngineLifecycleReceipt,
    schedule: StateRoughScheduleReceipt,
    requests: Vec<StateRoughRequestReceipt>,
    checkpoints: Vec<StateRoughRetainedCheckpoint>,
    long_lived: StateRoughLongLivedReceipt,
    rollup: StateRoughLifecycleRollup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeRenderThread {
    Primary,
    Fresh,
}

impl ProbeRenderThread {
    fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Fresh => "fresh",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ProbeRequestSpec {
    case: &'static str,
    configured_seed: f64,
    geometry_label_bytes: usize,
    ordinary_nodes: usize,
    render_thread: ProbeRenderThread,
}

fn detailed_probe_specs() -> [ProbeRequestSpec; 9] {
    [
        ProbeRequestSpec {
            case: "seed-7-cold",
            configured_seed: 7.0,
            geometry_label_bytes: 4,
            ordinary_nodes: 6,
            render_thread: ProbeRenderThread::Primary,
        },
        ProbeRequestSpec {
            case: "seed-7-tls-warm",
            configured_seed: 7.0,
            geometry_label_bytes: 4,
            ordinary_nodes: 6,
            render_thread: ProbeRenderThread::Primary,
        },
        ProbeRequestSpec {
            case: "seed-7-global-warm",
            configured_seed: 7.0,
            geometry_label_bytes: 4,
            ordinary_nodes: 6,
            render_thread: ProbeRenderThread::Fresh,
        },
        ProbeRequestSpec {
            case: "seed-11-width-16",
            configured_seed: 11.0,
            geometry_label_bytes: 16,
            ordinary_nodes: 6,
            render_thread: ProbeRenderThread::Primary,
        },
        ProbeRequestSpec {
            case: "seed-12-width-32",
            configured_seed: 12.0,
            geometry_label_bytes: 32,
            ordinary_nodes: 6,
            render_thread: ProbeRenderThread::Primary,
        },
        ProbeRequestSpec {
            case: "seed-13-width-64",
            configured_seed: 13.0,
            geometry_label_bytes: 64,
            ordinary_nodes: 6,
            render_thread: ProbeRenderThread::Primary,
        },
        ProbeRequestSpec {
            case: "configured-zero-operation-seed",
            configured_seed: 0.0,
            geometry_label_bytes: 16,
            ordinary_nodes: 6,
            render_thread: ProbeRenderThread::Primary,
        },
        ProbeRequestSpec {
            case: "fallback-u32-wrap",
            configured_seed: 4_294_967_296.0,
            geometry_label_bytes: 4,
            ordinary_nodes: 6,
            render_thread: ProbeRenderThread::Primary,
        },
        ProbeRequestSpec {
            case: "fallback-second-stroke-wrap",
            configured_seed: -1.0,
            geometry_label_bytes: 4,
            ordinary_nodes: 6,
            render_thread: ProbeRenderThread::Primary,
        },
    ]
}

#[derive(Default)]
struct StateRoughSvgRollupBuilder {
    hasher: Sha256,
    bytes: usize,
    elements: usize,
}

impl StateRoughSvgRollupBuilder {
    fn record(&mut self, ordinal: usize, svg: &str, elements: usize) {
        let ordinal = u64::try_from(ordinal).expect("probe ordinal should fit u64");
        let svg_bytes = u64::try_from(svg.len()).expect("probe SVG length should fit u64");
        self.hasher.update(ordinal.to_le_bytes());
        self.hasher.update(svg_bytes.to_le_bytes());
        self.hasher.update(svg.as_bytes());
        self.bytes = self
            .bytes
            .checked_add(svg.len())
            .expect("probe SVG byte rollup should remain bounded");
        self.elements = self
            .elements
            .checked_add(elements)
            .expect("probe SVG element rollup should remain bounded");
    }

    fn finish(self) -> StateRoughSvgIdentity {
        let digest = self.hasher.finalize();
        StateRoughSvgIdentity {
            bytes: self.bytes,
            elements: self.elements,
            identity: format!("sha256:{digest:x}"),
        }
    }
}

fn svg_identity(svg: &str) -> String {
    let digest = Sha256::digest(svg.as_bytes());
    format!("sha256:{digest:x}")
}

fn validate_svg_identity(identity: &str) -> std::result::Result<(), String> {
    let digest = identity
        .strip_prefix("sha256:")
        .ok_or_else(|| "SVG identity must use the sha256 prefix".to_string())?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("SVG identity must contain 64 lowercase hexadecimal digits".to_string());
    }
    Ok(())
}

fn svg_element_count(svg: &str) -> usize {
    roxmltree::Document::parse(svg)
        .expect("State Rough lifecycle SVG should be valid XML")
        .descendants()
        .filter(|node| node.is_element())
        .count()
}

fn state_probe_source(spec: ProbeRequestSpec) -> String {
    let configured_seed =
        serde_json::to_string(&spec.configured_seed).expect("probe seed should serialize");
    let label = "x".repeat(spec.geometry_label_bytes);
    let mut source = format!(
        "%%{{init: {{\"look\":\"handDrawn\",\"handDrawnSeed\":{configured_seed}}}}}%%\nstateDiagram-v2\n"
    );
    for index in 0..spec.ordinary_nodes {
        let _ = writeln!(source, "state \"{label}\" as S{index}");
    }
    source.push_str("[*] --> S0\n");
    for index in 1..spec.ordinary_nodes {
        let _ = writeln!(source, "S{} --> S{index}", index - 1);
    }
    let _ = writeln!(source, "S{} --> [*]", spec.ordinary_nodes - 1);
    source.push_str("state CircleA {\n  [*] --> CircleAInner\n  CircleAInner --> [*]\n}\n");
    source.push_str("state CircleB {\n  [*] --> CircleBInner\n  CircleBInner --> [*]\n}\n");
    source.push_str("S0 --> CircleA\nCircleA --> CircleB\n");
    source.push_str("state F1 <<fork>>\nstate F2 <<join>>\nF1 --> F2\n");
    source.push_str("state C1 <<choice>>\nstate C2 <<choice>>\nC1 --> C2\n");
    source
}

fn prepare_state_probe_artifact(
    engine: &merman_core::Engine,
    environment: &crate::environment::RenderEnvironment,
    spec: ProbeRequestSpec,
) -> (crate::family::FamilyRenderArtifact, f64) {
    let source = state_probe_source(spec);
    let parsed = engine
        .parse_diagram_for_render_model_sync(&source, merman_core::ParseOptions::strict())
        .expect("State Rough lifecycle source should parse")
        .expect("State Rough lifecycle source should be detected");
    let session = environment
        .begin_session()
        .expect("State Rough lifecycle render session should start");
    let expected_resolved_seed = if spec.configured_seed == 0.0 {
        session.render_seed().get() as f64
    } else {
        spec.configured_seed
    };
    let artifact = crate::family::prepare(
        parsed,
        &crate::LayoutOptions::headless_svg_defaults(),
        session,
    )
    .expect("State Rough lifecycle artifact should prepare");
    (artifact, expected_resolved_seed)
}

fn render_state_probe_artifact(
    artifact: crate::family::FamilyRenderArtifact,
) -> (String, StateRoughLifecycleOperationSnapshot) {
    let capture = state_rough_lifecycle_capture();
    let rendered = artifact
        .render_svg(
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("state-rough-lifecycle-probe".to_string()),
                ..crate::svg::SvgRenderOptions::default()
            },
            &crate::svg::SvgDebugOptions::default(),
        )
        .expect("State Rough lifecycle artifact should render")
        .svg()
        .to_owned();
    drop(capture);

    let mut snapshots = state_rough_lifecycle_take_snapshots();
    assert_eq!(
        snapshots.len(),
        1,
        "one State render should produce exactly one lifecycle snapshot"
    );
    (rendered, snapshots.pop().expect("lifecycle snapshot"))
}

fn render_state_probe_request(
    engine: &merman_core::Engine,
    environment: &crate::environment::RenderEnvironment,
    ordinal: usize,
    spec: ProbeRequestSpec,
) -> (StateRoughRequestReceipt, String) {
    let (artifact, expected_resolved_seed) =
        prepare_state_probe_artifact(engine, environment, spec);
    let (svg, operation) = match spec.render_thread {
        ProbeRenderThread::Primary => render_state_probe_artifact(artifact),
        ProbeRenderThread::Fresh => {
            std::thread::spawn(move || render_state_probe_artifact(artifact))
                .join()
                .expect("fresh State Rough render thread should not panic")
        }
    };

    assert_eq!(operation.configured_seed, spec.configured_seed);
    assert_eq!(operation.resolved_seed, expected_resolved_seed);
    assert_eq!(
        operation.cache_allowed,
        !roughr::core::RoughJsSeed::new(expected_resolved_seed).may_use_math_random()
    );
    assert_eq!(operation.outcome, StateRoughLifecycleOutcome::Success);
    operation
        .counters
        .validate()
        .expect("State Rough operation counter identities should hold");
    for counters in [operation.counters.circle, operation.counters.paths] {
        assert!(counters.draw_requests > 0);
        if operation.cache_allowed {
            assert_eq!(counters.bypass_builds, 0);
        } else {
            assert_eq!(counters.operation_lookups, 0);
            assert_eq!(counters.operation_hits, 0);
            assert_eq!(counters.operation_misses, 0);
            assert_eq!(counters.operation_builds, 0);
            assert_eq!(counters.tls_hits, 0);
            assert_eq!(counters.global_hits, 0);
            assert!(counters.bypass_builds > 0);
        }
    }

    let svg_receipt = StateRoughSvgIdentity {
        bytes: svg.len(),
        elements: svg_element_count(&svg),
        identity: svg_identity(&svg),
    };
    (
        StateRoughRequestReceipt {
            ordinal,
            case: spec.case.to_string(),
            render_thread: spec.render_thread.as_str().to_string(),
            geometry_label_bytes: spec.geometry_label_bytes,
            ordinary_nodes: spec.ordinary_nodes,
            svg: svg_receipt,
            operation,
        },
        svg,
    )
}

fn footprint_growth(
    final_footprint: StateRoughCacheFootprint,
    initial_footprint: StateRoughCacheFootprint,
) -> StateRoughCacheFootprint {
    StateRoughCacheFootprint {
        entries: final_footprint
            .entries
            .checked_sub(initial_footprint.entries)
            .expect("State Rough retained entries must not fall below the initial snapshot"),
        owned_bytes: final_footprint
            .owned_bytes
            .checked_sub(initial_footprint.owned_bytes)
            .expect("State Rough retained bytes must not fall below the initial snapshot"),
    }
}

fn run_state_rough_long_lived_probe(
    engine: &merman_core::Engine,
    environment: &crate::environment::RenderEnvironment,
    first_ordinal: usize,
    total_svg_rollup: &mut StateRoughSvgRollupBuilder,
) -> StateRoughLongLivedReceipt {
    let mut svg_rollup = StateRoughSvgRollupBuilder::default();
    let mut counters = StateRoughOperationCounters::default();
    let mut max_operation_peak = StateRoughCacheFootprint::default();
    let mut checkpoints = Vec::with_capacity(LONG_LIVED_REQUEST_CHECKPOINTS.len());
    let mut final_retained = StateRoughRetainedSnapshot::default();

    for request_count in 1..=LONG_LIVED_REQUEST_COUNT {
        let geometry_label_bytes = LONG_LIVED_GEOMETRY_LABEL_BYTES
            [(request_count - 1) % LONG_LIVED_GEOMETRY_LABEL_BYTES.len()];
        let spec = ProbeRequestSpec {
            case: "long-lived-distinct-seed",
            configured_seed: 10_000.0 + request_count as f64,
            geometry_label_bytes,
            ordinary_nodes: 2 + ((request_count - 1) % 5),
            render_thread: ProbeRenderThread::Primary,
        };
        let ordinal = first_ordinal
            .checked_add(request_count - 1)
            .expect("long-lived request ordinal should remain bounded");
        let (request, svg) = render_state_probe_request(engine, environment, ordinal, spec);
        counters.checked_add_assign(request.operation.counters);
        max_operation_peak.observe_peak(
            request.operation.operation_peak.entries,
            request.operation.operation_peak.owned_bytes,
        );
        svg_rollup.record(ordinal, &svg, request.svg.elements);
        total_svg_rollup.record(ordinal, &svg, request.svg.elements);
        final_retained = request.operation.post_operation_retained;

        if LONG_LIVED_REQUEST_CHECKPOINTS.contains(&request_count) {
            checkpoints.push(StateRoughRetainedCheckpoint {
                request_count,
                geometry_label_bytes,
                configured_seed: request.operation.configured_seed,
                retained: final_retained,
            });
        }
    }

    StateRoughLongLivedReceipt {
        request_count: LONG_LIVED_REQUEST_COUNT,
        request_count_checkpoints: LONG_LIVED_REQUEST_CHECKPOINTS.to_vec(),
        checkpoints,
        svg: svg_rollup.finish(),
        counters,
        max_operation_peak,
        final_retained,
    }
}

fn build_state_rough_lifecycle_receipt(
    requests: Vec<StateRoughRequestReceipt>,
    rendered_svgs: &[String],
    long_lived: StateRoughLongLivedReceipt,
    svg_rollup: StateRoughSvgIdentity,
    initial_retained: StateRoughRetainedSnapshot,
) -> StateRoughLifecycleReceipt {
    assert_eq!(requests.len(), rendered_svgs.len());
    for (request, svg) in requests.iter().zip(rendered_svgs) {
        assert_eq!(request.svg.bytes, svg.len());
        assert_eq!(request.svg.elements, svg_element_count(svg));
        assert_eq!(request.svg.identity, svg_identity(svg));
    }
    let mut counters = StateRoughOperationCounters::default();
    let mut max_operation_peak = StateRoughCacheFootprint::default();
    let mut checkpoints = Vec::with_capacity(requests.len());

    for request in &requests {
        counters.checked_add_assign(request.operation.counters);
        max_operation_peak.observe_peak(
            request.operation.operation_peak.entries,
            request.operation.operation_peak.owned_bytes,
        );
        checkpoints.push(StateRoughRetainedCheckpoint {
            request_count: request.ordinal,
            geometry_label_bytes: request.geometry_label_bytes,
            configured_seed: request.operation.configured_seed,
            retained: request.operation.post_operation_retained,
        });
    }
    counters.checked_add_assign(long_lived.counters);
    max_operation_peak.observe_peak(
        long_lived.max_operation_peak.entries,
        long_lived.max_operation_peak.owned_bytes,
    );

    let final_retained = long_lived.final_retained;
    let retained_growth = StateRoughRetainedSnapshot {
        global: footprint_growth(final_retained.global, initial_retained.global),
        tls: footprint_growth(final_retained.tls, initial_retained.tls),
    };
    let retained_growth_observed = retained_growth.global.entries > 0
        || retained_growth.global.owned_bytes > 0
        || retained_growth.tls.entries > 0
        || retained_growth.tls.owned_bytes > 0;

    StateRoughLifecycleReceipt {
        schema: STATE_ROUGH_LIFECYCLE_SCHEMA,
        contracts: StateRoughLifecycleContracts {
            owned_bytes: OWNED_BYTES_DEFINITION,
            configured_seed_zero: CONFIGURED_ZERO_CONTRACT,
            fallback_capable_configured_seeds: [4_294_967_296.0, -1.0],
        },
        engine_lifecycle: StateRoughEngineLifecycleReceipt {
            engine_instances: 1,
            engine_reused_across_requests: true,
            request_count: requests
                .len()
                .checked_add(long_lived.request_count)
                .expect("probe request count should remain bounded"),
            detailed_request_count: requests.len(),
            long_lived_request_count: long_lived.request_count,
            render_threads: 2,
        },
        schedule: StateRoughScheduleReceipt {
            same_seed_request_ordinals: vec![1, 2, 3],
            distinct_seed_request_ordinals: vec![4, 5, 6],
            fallback_bypass_request_ordinals: vec![8, 9],
            geometry_label_byte_checkpoints: vec![4, 16, 32, 64],
            request_count_checkpoints: LONG_LIVED_REQUEST_CHECKPOINTS.to_vec(),
        },
        rollup: StateRoughLifecycleRollup {
            svg: svg_rollup,
            counters,
            max_operation_peak,
            initial_retained,
            final_retained,
            retained_growth,
            operation_cache_reuse_observed: counters.circle.operation_hits > 0
                && counters.paths.operation_hits > 0,
            legacy_cross_operation_cache_observed: counters.circle.tls_hits > 0
                && counters.circle.global_hits > 0
                && counters.paths.tls_hits > 0
                && counters.paths.global_hits > 0
                && retained_growth_observed,
            configured_zero_operation_resolution_observed: requests.iter().any(|request| {
                request.operation.configured_seed == 0.0
                    && request.operation.resolved_seed != 0.0
                    && request.operation.seed_resolution
                        == StateRoughSeedResolution::OperationResolved
            }),
            fallback_bypass_observed: {
                let fallback_requests = requests
                    .iter()
                    .filter(|request| {
                        request.operation.configured_seed == 4_294_967_296.0
                            || request.operation.configured_seed == -1.0
                    })
                    .collect::<Vec<_>>();
                fallback_requests.len() == 2
                    && fallback_requests.iter().all(|request| {
                        !request.operation.cache_allowed
                            && [
                                request.operation.counters.circle,
                                request.operation.counters.paths,
                            ]
                            .into_iter()
                            .all(|counters| {
                                counters.operation_lookups == 0 && counters.bypass_builds > 0
                            })
                    })
            },
        },
        requests,
        checkpoints,
        long_lived,
    }
}

fn validate_state_rough_lifecycle_receipt(
    receipt: &StateRoughLifecycleReceipt,
) -> std::result::Result<(), String> {
    if receipt.schema != STATE_ROUGH_LIFECYCLE_SCHEMA {
        return Err(format!("unexpected receipt schema: {}", receipt.schema));
    }
    if receipt.contracts.owned_bytes != OWNED_BYTES_DEFINITION {
        return Err("owned-byte definition drifted".to_string());
    }
    if receipt.contracts.configured_seed_zero != CONFIGURED_ZERO_CONTRACT {
        return Err("configured-zero contract drifted".to_string());
    }
    if receipt.engine_lifecycle.engine_instances != 1
        || !receipt.engine_lifecycle.engine_reused_across_requests
        || receipt.engine_lifecycle.render_threads != 2
    {
        return Err("receipt must describe one reused Engine lifecycle".to_string());
    }
    let expected_request_count = receipt
        .engine_lifecycle
        .detailed_request_count
        .checked_add(receipt.engine_lifecycle.long_lived_request_count)
        .ok_or_else(|| "engine request count overflowed".to_string())?;
    if receipt.engine_lifecycle.request_count != expected_request_count
        || receipt.engine_lifecycle.detailed_request_count != receipt.requests.len()
        || receipt.engine_lifecycle.long_lived_request_count != receipt.long_lived.request_count
        || receipt.checkpoints.len() != receipt.requests.len()
    {
        return Err("engine, detailed, and long-lived request cardinality must agree".to_string());
    }
    let detailed_specs = detailed_probe_specs();
    if receipt.requests.len() != detailed_specs.len() {
        return Err("detailed State Rough request schedule cardinality drifted".to_string());
    }
    if receipt.long_lived.request_count != LONG_LIVED_REQUEST_COUNT
        || receipt.long_lived.request_count_checkpoints != LONG_LIVED_REQUEST_CHECKPOINTS
        || receipt.schedule.request_count_checkpoints != LONG_LIVED_REQUEST_CHECKPOINTS
        || receipt.long_lived.checkpoints.len() != LONG_LIVED_REQUEST_CHECKPOINTS.len()
    {
        return Err("long-lived request schedule drifted".to_string());
    }
    if receipt.schedule.same_seed_request_ordinals != [1, 2, 3]
        || receipt.schedule.distinct_seed_request_ordinals != [4, 5, 6]
        || receipt.schedule.fallback_bypass_request_ordinals != [8, 9]
        || receipt.schedule.geometry_label_byte_checkpoints != [4, 16, 32, 64]
    {
        return Err("detailed request schedule drifted".to_string());
    }
    for (index, (request, checkpoint)) in receipt
        .requests
        .iter()
        .zip(&receipt.checkpoints)
        .enumerate()
    {
        let spec = detailed_specs[index];
        if request.ordinal != index + 1
            || request.case != spec.case
            || request.render_thread != spec.render_thread.as_str()
            || request.geometry_label_bytes != spec.geometry_label_bytes
            || request.ordinary_nodes != spec.ordinary_nodes
            || request.operation.configured_seed != spec.configured_seed
            || checkpoint.request_count != request.ordinal
            || checkpoint.geometry_label_bytes != request.geometry_label_bytes
            || checkpoint.configured_seed != request.operation.configured_seed
            || checkpoint.retained != request.operation.post_operation_retained
        {
            return Err(format!(
                "detailed request/checkpoint {} does not match its schedule",
                index + 1
            ));
        }
        validate_svg_identity(&request.svg.identity)?;
        request.operation.counters.validate()?;
        for counters in [
            request.operation.counters.circle,
            request.operation.counters.paths,
        ] {
            if counters.draw_requests == 0 {
                return Err(format!(
                    "request {} did not exercise both Rough geometry kinds",
                    request.ordinal
                ));
            }
            if request.operation.cache_allowed && counters.bypass_builds != 0 {
                return Err(format!(
                    "cache-eligible request {} recorded bypass builds",
                    request.ordinal
                ));
            }
            if !request.operation.cache_allowed
                && (counters.operation_lookups != 0
                    || counters.operation_hits != 0
                    || counters.operation_misses != 0
                    || counters.operation_builds != 0
                    || counters.tls_hits != 0
                    || counters.global_hits != 0)
            {
                return Err(format!(
                    "cache-bypassed request {} entered a cache layer",
                    request.ordinal
                ));
            }
        }
    }
    if receipt.requests[0].svg.identity != receipt.requests[1].svg.identity
        || receipt.requests[0].svg.identity != receipt.requests[2].svg.identity
        || receipt.requests[0].svg.bytes != receipt.requests[1].svg.bytes
        || receipt.requests[0].svg.bytes != receipt.requests[2].svg.bytes
        || receipt.requests[0].svg.elements != receipt.requests[1].svg.elements
        || receipt.requests[0].svg.elements != receipt.requests[2].svg.elements
    {
        return Err("same-seed detailed controls must have identical SVG output".to_string());
    }
    for (checkpoint, expected_request_count) in receipt
        .long_lived
        .checkpoints
        .iter()
        .zip(LONG_LIVED_REQUEST_CHECKPOINTS)
    {
        if checkpoint.request_count != expected_request_count {
            return Err(format!(
                "long-lived checkpoint {} does not match the registered schedule",
                checkpoint.request_count
            ));
        }
    }
    if receipt
        .long_lived
        .checkpoints
        .last()
        .map(|checkpoint| checkpoint.retained)
        != Some(receipt.long_lived.final_retained)
    {
        return Err("long-lived final retained snapshot drifted".to_string());
    }
    receipt.long_lived.counters.validate()?;
    validate_svg_identity(&receipt.long_lived.svg.identity)?;
    receipt.rollup.counters.validate()?;
    validate_svg_identity(&receipt.rollup.svg.identity)?;

    let expected_svg_bytes = receipt
        .requests
        .iter()
        .try_fold(receipt.long_lived.svg.bytes, |sum, request| {
            sum.checked_add(request.svg.bytes)
        })
        .ok_or_else(|| "SVG byte rollup overflowed".to_string())?;
    let expected_svg_elements = receipt
        .requests
        .iter()
        .try_fold(receipt.long_lived.svg.elements, |sum, request| {
            sum.checked_add(request.svg.elements)
        })
        .ok_or_else(|| "SVG element rollup overflowed".to_string())?;
    if receipt.rollup.svg.bytes != expected_svg_bytes
        || receipt.rollup.svg.elements != expected_svg_elements
    {
        return Err("SVG rollup totals do not match detailed and long-lived totals".to_string());
    }

    let mut expected_counters = StateRoughOperationCounters::default();
    let mut expected_peak = receipt.long_lived.max_operation_peak;
    for request in &receipt.requests {
        expected_counters.checked_add_assign(request.operation.counters);
        expected_peak.observe_peak(
            request.operation.operation_peak.entries,
            request.operation.operation_peak.owned_bytes,
        );
    }
    expected_counters.checked_add_assign(receipt.long_lived.counters);
    if receipt.rollup.counters != expected_counters
        || receipt.rollup.max_operation_peak != expected_peak
        || receipt.rollup.final_retained != receipt.long_lived.final_retained
    {
        return Err("lifecycle rollup does not match its component receipts".to_string());
    }
    Ok(())
}

fn serialize_state_rough_lifecycle_receipt(receipt: &StateRoughLifecycleReceipt) -> String {
    format!(
        "{STATE_ROUGH_LIFECYCLE_RECEIPT_MARKER}{}",
        serde_json::to_string(receipt).expect("State Rough lifecycle receipt should serialize")
    )
}

#[test]
fn state_rough_lifecycle_counter_identities_and_peak_tracking_are_exact() {
    let _test_lock = lifecycle_test_lock();
    state_rough_lifecycle_probe_reset();
    let capture = state_rough_lifecycle_capture();
    {
        let probe = StateRoughLifecycleOperationProbe::new(7.0, 7.0, true);
        probe.record_draw_request(StateRoughGeometryKind::Circle);
        probe.record_operation_lookup(StateRoughGeometryKind::Circle);
        probe.record_operation_miss(StateRoughGeometryKind::Circle);
        probe.record_tls_hit(StateRoughGeometryKind::Circle);
        probe.observe_operation_cache(2, 40);
        probe.observe_operation_cache(1, 20);
        probe.record_draw_request(StateRoughGeometryKind::Circle);
        probe.record_operation_lookup(StateRoughGeometryKind::Circle);
        probe.record_operation_hit(StateRoughGeometryKind::Circle);
        probe.record_draw_request(StateRoughGeometryKind::Paths);
        probe.record_operation_lookup(StateRoughGeometryKind::Paths);
        probe.record_operation_miss(StateRoughGeometryKind::Paths);
        probe.record_operation_build(StateRoughGeometryKind::Paths);
        probe.mark_success();
    }
    drop(capture);

    let snapshots = state_rough_lifecycle_take_snapshots();
    assert_eq!(snapshots.len(), 1);
    let snapshot = &snapshots[0];
    snapshot.counters.validate().expect("counter identities");
    assert_eq!(snapshot.counters.circle.draw_requests, 2);
    assert_eq!(snapshot.counters.circle.operation_lookups, 2);
    assert_eq!(snapshot.counters.circle.operation_hits, 1);
    assert_eq!(snapshot.counters.circle.operation_misses, 1);
    assert_eq!(snapshot.counters.circle.tls_hits, 1);
    assert_eq!(snapshot.counters.paths.draw_requests, 1);
    assert_eq!(snapshot.counters.paths.operation_builds, 1);
    assert_eq!(
        snapshot.operation_peak,
        StateRoughCacheFootprint {
            entries: 2,
            owned_bytes: 40,
        }
    );
    assert_eq!(snapshot.outcome, StateRoughLifecycleOutcome::Success);
}

#[test]
fn state_rough_lifecycle_marker_wraps_one_strict_json_schema() {
    let _test_lock = lifecycle_test_lock();
    let mut requests = Vec::new();
    let mut rendered_svgs = Vec::new();
    let mut svg_rollup = StateRoughSvgRollupBuilder::default();
    for (index, spec) in detailed_probe_specs().into_iter().enumerate() {
        let resolved_seed = if spec.configured_seed == 0.0 {
            42.0
        } else {
            spec.configured_seed
        };
        let cache_allowed = !roughr::core::RoughJsSeed::new(resolved_seed).may_use_math_random();
        let geometry_counters = if cache_allowed {
            StateRoughGeometryCounters {
                draw_requests: 1,
                operation_lookups: 1,
                operation_misses: 1,
                operation_builds: 1,
                ..StateRoughGeometryCounters::default()
            }
        } else {
            StateRoughGeometryCounters {
                draw_requests: 1,
                bypass_builds: 1,
                ..StateRoughGeometryCounters::default()
            }
        };
        let operation = StateRoughLifecycleOperationSnapshot {
            configured_seed: spec.configured_seed,
            resolved_seed,
            seed_resolution: if spec.configured_seed == 0.0 {
                StateRoughSeedResolution::OperationResolved
            } else if cache_allowed {
                StateRoughSeedResolution::ConfiguredDeterministic
            } else {
                StateRoughSeedResolution::ConfiguredFallbackCapable
            },
            cache_allowed,
            outcome: StateRoughLifecycleOutcome::Success,
            counters: StateRoughOperationCounters {
                circle: geometry_counters,
                paths: geometry_counters,
            },
            operation_peak: if cache_allowed {
                StateRoughCacheFootprint {
                    entries: 2,
                    owned_bytes: 16,
                }
            } else {
                StateRoughCacheFootprint::default()
            },
            post_operation_retained: StateRoughRetainedSnapshot::default(),
        };
        let svg = if index < 3 {
            "<svg/>".to_string()
        } else {
            format!(r#"<svg data-probe="{}"/>"#, index + 1)
        };
        let svg_receipt = StateRoughSvgIdentity {
            bytes: svg.len(),
            elements: 1,
            identity: svg_identity(&svg),
        };
        let ordinal = index + 1;
        svg_rollup.record(ordinal, &svg, svg_receipt.elements);
        requests.push(StateRoughRequestReceipt {
            ordinal,
            case: spec.case.to_string(),
            render_thread: spec.render_thread.as_str().to_string(),
            geometry_label_bytes: spec.geometry_label_bytes,
            ordinary_nodes: spec.ordinary_nodes,
            svg: svg_receipt,
            operation,
        });
        rendered_svgs.push(svg);
    }
    let long_lived_checkpoints = LONG_LIVED_REQUEST_CHECKPOINTS
        .into_iter()
        .map(|request_count| StateRoughRetainedCheckpoint {
            request_count,
            geometry_label_bytes: LONG_LIVED_GEOMETRY_LABEL_BYTES
                [(request_count - 1) % LONG_LIVED_GEOMETRY_LABEL_BYTES.len()],
            configured_seed: 10_000.0 + request_count as f64,
            retained: StateRoughRetainedSnapshot::default(),
        })
        .collect::<Vec<_>>();
    let long_lived = StateRoughLongLivedReceipt {
        request_count: LONG_LIVED_REQUEST_COUNT,
        request_count_checkpoints: LONG_LIVED_REQUEST_CHECKPOINTS.to_vec(),
        checkpoints: long_lived_checkpoints,
        svg: StateRoughSvgRollupBuilder::default().finish(),
        counters: StateRoughOperationCounters::default(),
        max_operation_peak: StateRoughCacheFootprint::default(),
        final_retained: StateRoughRetainedSnapshot::default(),
    };
    let receipt = build_state_rough_lifecycle_receipt(
        requests,
        &rendered_svgs,
        long_lived,
        svg_rollup.finish(),
        StateRoughRetainedSnapshot::default(),
    );
    validate_state_rough_lifecycle_receipt(&receipt).expect("valid receipt schema");
    let line = serialize_state_rough_lifecycle_receipt(&receipt);
    assert_eq!(
        line.matches(STATE_ROUGH_LIFECYCLE_RECEIPT_MARKER).count(),
        1
    );
    let json = line
        .strip_prefix(STATE_ROUGH_LIFECYCLE_RECEIPT_MARKER)
        .expect("receipt marker prefix");
    let value: serde_json::Value = serde_json::from_str(json).expect("strict JSON receipt");
    let keys = value
        .as_object()
        .expect("receipt object")
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        keys,
        [
            "checkpoints",
            "contracts",
            "engine_lifecycle",
            "long_lived",
            "requests",
            "rollup",
            "schedule",
            "schema",
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(value["schema"], STATE_ROUGH_LIFECYCLE_SCHEMA);
    assert_eq!(value["contracts"]["owned_bytes"], OWNED_BYTES_DEFINITION);
}

#[test]
#[ignore = "decision-grade State Rough lifecycle receipt; run explicitly with --ignored --nocapture"]
fn state_rough_lifecycle_probe_receipt() {
    let _test_lock = lifecycle_test_lock();
    state_rough_lifecycle_probe_reset();
    let (global_entries, global_owned_bytes, tls_entries, tls_owned_bytes) =
        state_rough_cache_retained_counts();
    let initial_retained = StateRoughRetainedSnapshot {
        global: StateRoughCacheFootprint {
            entries: global_entries,
            owned_bytes: global_owned_bytes,
        },
        tls: StateRoughCacheFootprint {
            entries: tls_entries,
            owned_bytes: tls_owned_bytes,
        },
    };
    assert_eq!(initial_retained, StateRoughRetainedSnapshot::default());

    let engine = merman_core::Engine::new();
    let environment = crate::environment::RenderEnvironment::deterministic();
    let specs = detailed_probe_specs();

    let mut requests = Vec::with_capacity(specs.len());
    let mut rendered_svgs = Vec::with_capacity(specs.len());
    let mut svg_rollup = StateRoughSvgRollupBuilder::default();
    for (index, spec) in specs.into_iter().enumerate() {
        let (request, svg) = render_state_probe_request(&engine, &environment, index + 1, spec);
        svg_rollup.record(request.ordinal, &svg, request.svg.elements);
        requests.push(request);
        rendered_svgs.push(svg);
    }

    assert_eq!(requests[0].svg.identity, requests[1].svg.identity);
    assert_eq!(requests[0].svg.identity, requests[2].svg.identity);
    assert_ne!(requests[0].svg.identity, requests[3].svg.identity);
    assert!(
        requests
            .iter()
            .all(|request| request.operation.outcome == StateRoughLifecycleOutcome::Success)
    );

    let long_lived = run_state_rough_long_lived_probe(
        &engine,
        &environment,
        requests.len() + 1,
        &mut svg_rollup,
    );
    let receipt = build_state_rough_lifecycle_receipt(
        requests,
        &rendered_svgs,
        long_lived,
        svg_rollup.finish(),
        initial_retained,
    );
    validate_state_rough_lifecycle_receipt(&receipt).expect("valid lifecycle receipt");
    assert!(receipt.rollup.operation_cache_reuse_observed);
    assert!(receipt.rollup.configured_zero_operation_resolution_observed);
    assert!(receipt.rollup.fallback_bypass_observed);
    assert_eq!(receipt.long_lived.request_count, LONG_LIVED_REQUEST_COUNT);
    assert_eq!(
        receipt.long_lived.request_count_checkpoints,
        LONG_LIVED_REQUEST_CHECKPOINTS
    );

    println!("{}", serialize_state_rough_lifecycle_receipt(&receipt));
}
