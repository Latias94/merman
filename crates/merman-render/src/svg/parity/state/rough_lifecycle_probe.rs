use super::*;
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use std::cell::{Cell, RefCell};
use std::sync::{Mutex, MutexGuard, OnceLock, mpsc};
use std::time::Duration;

pub(super) const STATE_ROUGH_LIFECYCLE_RECEIPT_MARKER: &str =
    "MERMAN_STATE_ROUGH_LIFECYCLE_RECEIPT_V2=";
pub(super) const STATE_ROUGH_LIFECYCLE_CONTROLS_MARKER: &str =
    "MERMAN_STATE_ROUGH_LIFECYCLE_CONTROLS_V2=";
const STATE_ROUGH_LIFECYCLE_SCHEMA: &str = "merman.state_rough_lifecycle.v2";
const STATE_ROUGH_LIFECYCLE_CONTROLS_SCHEMA: &str = "merman.state_rough_lifecycle_controls.v2";
const OWNED_BYTES_DEFINITION: &str = "sum_of_cached_string_capacities";
const RELEASE_PROOF_DEFINITION: &str =
    "weak_string_allocation_witnesses_sampled_after_operation_cache_drop";
const RENDER_CANCELLATION_CONTRACT: &str = "not_applicable_no_render_control_or_checkpoint";
const EARLY_TERMINATION_PROOF: &str = "result_error_after_nonempty_operation_cache";
const CONFIGURED_ZERO_CONTRACT: &str =
    "configured_hand_drawn_seed_zero_resolves_to_operation_seed_before_cache_bypass";
const AFTER_ROOT_ERROR_SENTINEL: &str = "State Rough lifecycle control error after root render";
const AFTER_ROOT_PANIC_SENTINEL: &str = "State Rough lifecycle control unwind after root render";
const LONG_LIVED_REQUEST_COUNT: usize = 2_048;
const LONG_LIVED_REQUEST_CHECKPOINTS: [usize; 6] = [1, 16, 64, 256, 1_024, 2_048];
const LONG_LIVED_GEOMETRY_LABEL_BYTES: [usize; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
const CONTROL_TIMEOUT: Duration = Duration::from_secs(10);

thread_local! {
    static STATE_ROUGH_LIFECYCLE_CAPTURE_DEPTH: Cell<u32> = const { Cell::new(0) };
    static STATE_ROUGH_LIFECYCLE_SNAPSHOTS: RefCell<Vec<StateRoughLifecycleOperationSnapshot>> =
        const { RefCell::new(Vec::new()) };
    static STATE_ROUGH_LIFECYCLE_AFTER_ROOT_ACTION: RefCell<Option<(u64, StateRoughLifecycleAfterRootAction)>> =
        const { RefCell::new(None) };
    static STATE_ROUGH_LIFECYCLE_AFTER_ROOT_TOKEN: Cell<u64> = const { Cell::new(1) };
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

enum StateRoughLifecycleAfterRootAction {
    ReturnError {
        completed_svg: mpsc::Sender<String>,
    },
    Panic {
        completed_svg: mpsc::Sender<String>,
    },
    Rendezvous {
        worker: usize,
        reached: mpsc::SyncSender<usize>,
        release: mpsc::Receiver<()>,
    },
}

struct StateRoughLifecycleAfterRootGuard {
    token: u64,
}

impl Drop for StateRoughLifecycleAfterRootGuard {
    fn drop(&mut self) {
        STATE_ROUGH_LIFECYCLE_AFTER_ROOT_ACTION.with(|action| {
            let should_clear = action
                .borrow()
                .as_ref()
                .is_some_and(|(token, _)| *token == self.token);
            if should_clear {
                action.borrow_mut().take();
            }
        });
    }
}

fn state_rough_lifecycle_install_after_root_action(
    action: StateRoughLifecycleAfterRootAction,
) -> StateRoughLifecycleAfterRootGuard {
    let token = STATE_ROUGH_LIFECYCLE_AFTER_ROOT_TOKEN.with(|next| {
        let token = next.get();
        next.set(
            token
                .checked_add(1)
                .expect("State Rough lifecycle action token should remain bounded"),
        );
        token
    });
    STATE_ROUGH_LIFECYCLE_AFTER_ROOT_ACTION.with(|slot| {
        assert!(
            slot.borrow().is_none(),
            "State Rough after-root lifecycle actions must not nest"
        );
        *slot.borrow_mut() = Some((token, action));
    });
    StateRoughLifecycleAfterRootGuard { token }
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

impl StateRoughRetainedSnapshot {
    fn observe_peak(&mut self, other: Self) {
        self.global
            .observe_peak(other.global.entries, other.global.owned_bytes);
        self.tls
            .observe_peak(other.tls.entries, other.tls.owned_bytes);
    }
}

impl StateRoughReleaseProof {
    fn validate(
        self,
        counters: StateRoughOperationCounters,
        operation_peak: StateRoughCacheFootprint,
        cache_allowed: bool,
    ) -> std::result::Result<(), String> {
        if !self.cache_drop_observed {
            return Err(
                "operation cache drop was not observed before release sampling".to_string(),
            );
        }

        let expected_geometry_witnesses = counters
            .circle
            .operation_misses
            .checked_add(counters.paths.operation_misses)
            .ok_or_else(|| "release geometry witness identity overflowed".to_string())?;
        if self.geometry_witnesses != expected_geometry_witnesses {
            return Err(format!(
                "release geometry witness identity failed: witnesses={} expected={expected_geometry_witnesses}",
                self.geometry_witnesses
            ));
        }

        let expected_allocation_witnesses = counters
            .circle
            .operation_misses
            .checked_add(
                counters
                    .paths
                    .operation_misses
                    .checked_mul(2)
                    .ok_or_else(|| "release allocation witness identity overflowed".to_string())?,
            )
            .ok_or_else(|| "release allocation witness identity overflowed".to_string())?;
        if self.allocation_witnesses != expected_allocation_witnesses {
            return Err(format!(
                "release allocation witness identity failed: witnesses={} expected={expected_allocation_witnesses}",
                self.allocation_witnesses
            ));
        }
        if self.allocation_witnesses > 0 && self.witnessed_owned_bytes == 0 {
            return Err("non-empty release witnesses must own String capacity".to_string());
        }
        if self.live_allocation_witnesses > self.allocation_witnesses
            || self.live_owned_bytes > self.witnessed_owned_bytes
        {
            return Err("live release witnesses exceed the witnessed allocation set".to_string());
        }

        if cache_allowed {
            if operation_peak.entries != self.geometry_witnesses
                || operation_peak.owned_bytes != self.witnessed_owned_bytes
            {
                return Err(
                    "cache-eligible release witnesses must equal the operation-cache peak"
                        .to_string(),
                );
            }
        } else if operation_peak != StateRoughCacheFootprint::default() {
            return Err(
                "cache-bypassed release proof recorded an operation-cache peak".to_string(),
            );
        }

        Ok(())
    }
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
    pub(super) release_proof: StateRoughReleaseProof,
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
    release_tracker: StateRoughReleaseTracker,
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
            release_tracker: StateRoughReleaseTracker::default(),
        }
    }

    pub(super) fn release_tracker(&self) -> StateRoughReleaseTracker {
        self.release_tracker.clone()
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
        let release_proof = self.release_tracker.snapshot();
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
            release_proof,
        };
        with_lifecycle_snapshots(|snapshots| snapshots.push(snapshot));
    }
}

pub(super) fn state_rough_lifecycle_observe_operation_cache(ctx: &StateRenderCtx<'_>) {
    let (entries, owned_bytes) = ctx.rough_cache.footprint();
    ctx.rough_lifecycle_probe
        .observe_operation_cache(entries, owned_bytes);
}

pub(super) fn state_rough_lifecycle_after_root(completed_svg: &str) -> Result<()> {
    let action = STATE_ROUGH_LIFECYCLE_AFTER_ROOT_ACTION
        .with(|slot| slot.borrow_mut().take().map(|(_, action)| action));
    match action {
        None => Ok(()),
        Some(StateRoughLifecycleAfterRootAction::ReturnError {
            completed_svg: sender,
        }) => {
            sender
                .send(completed_svg.to_owned())
                .expect("error control SVG receiver should remain alive");
            Err(Error::InvalidModel {
                message: AFTER_ROOT_ERROR_SENTINEL.to_string(),
            })
        }
        Some(StateRoughLifecycleAfterRootAction::Panic {
            completed_svg: sender,
        }) => {
            sender
                .send(completed_svg.to_owned())
                .expect("unwind control SVG receiver should remain alive");
            panic!("{AFTER_ROOT_PANIC_SENTINEL}")
        }
        Some(StateRoughLifecycleAfterRootAction::Rendezvous {
            worker,
            reached,
            release,
        }) => {
            reached.send(worker).map_err(|_| Error::InvalidModel {
                message: format!(
                    "State Rough lifecycle worker {worker} could not report after-root arrival"
                ),
            })?;
            release
                .recv_timeout(CONTROL_TIMEOUT)
                .map_err(|error| Error::InvalidModel {
                    message: format!(
                        "State Rough lifecycle worker {worker} release failed: {error}"
                    ),
                })?;
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct StateRoughLifecycleContracts {
    owned_bytes: &'static str,
    release_proof: &'static str,
    render_cancellation: &'static str,
    early_termination_proof: &'static str,
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
struct StateRoughLongLivedReleaseProof {
    request_count: usize,
    cache_allowed: bool,
    counters: StateRoughOperationCounters,
    operation_peak: StateRoughCacheFootprint,
    release_proof: StateRoughReleaseProof,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
struct StateRoughReleaseRollup {
    operation_count: usize,
    total_geometry_witnesses: usize,
    total_allocation_witnesses: usize,
    total_witnessed_owned_bytes: usize,
    max_live_allocation_witnesses_after_operation: usize,
    max_live_owned_bytes_after_operation: usize,
    all_cache_drops_observed: bool,
}

fn state_rough_release_rollup(
    proofs: impl IntoIterator<Item = StateRoughReleaseProof>,
) -> StateRoughReleaseRollup {
    let mut rollup = StateRoughReleaseRollup {
        all_cache_drops_observed: true,
        ..StateRoughReleaseRollup::default()
    };
    for proof in proofs {
        rollup.operation_count = rollup
            .operation_count
            .checked_add(1)
            .expect("release proof operation count should remain bounded");
        rollup.total_geometry_witnesses = rollup
            .total_geometry_witnesses
            .checked_add(proof.geometry_witnesses)
            .expect("release geometry witness rollup should remain bounded");
        rollup.total_allocation_witnesses = rollup
            .total_allocation_witnesses
            .checked_add(proof.allocation_witnesses)
            .expect("release allocation witness rollup should remain bounded");
        rollup.total_witnessed_owned_bytes = rollup
            .total_witnessed_owned_bytes
            .checked_add(proof.witnessed_owned_bytes)
            .expect("release witnessed byte rollup should remain bounded");
        rollup.max_live_allocation_witnesses_after_operation = rollup
            .max_live_allocation_witnesses_after_operation
            .max(proof.live_allocation_witnesses);
        rollup.max_live_owned_bytes_after_operation = rollup
            .max_live_owned_bytes_after_operation
            .max(proof.live_owned_bytes);
        rollup.all_cache_drops_observed &= proof.cache_drop_observed;
    }
    if rollup.operation_count == 0 {
        rollup.all_cache_drops_observed = false;
    }
    rollup
}

#[derive(Debug, Clone, Serialize)]
struct StateRoughLongLivedReceipt {
    request_count: usize,
    request_count_checkpoints: Vec<usize>,
    checkpoints: Vec<StateRoughRetainedCheckpoint>,
    svg: StateRoughSvgIdentity,
    counters: StateRoughOperationCounters,
    max_operation_peak: StateRoughCacheFootprint,
    max_post_operation_retained: StateRoughRetainedSnapshot,
    final_retained: StateRoughRetainedSnapshot,
    release_proofs: Vec<StateRoughLongLivedReleaseProof>,
    release_rollup: StateRoughReleaseRollup,
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
    release: StateRoughReleaseRollup,
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

#[derive(Debug, Clone, Serialize)]
struct StateRoughControlEngineLifecycle {
    engine_instances: usize,
    engine_reused_across_requests: bool,
    request_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct StateRoughLifecycleFailureControl {
    sentinel: &'static str,
    engine_lifecycle: StateRoughControlEngineLifecycle,
    reference_svg: StateRoughSvgIdentity,
    failure_svg: StateRoughSvgIdentity,
    recovery_svg: StateRoughSvgIdentity,
    reference_operation: StateRoughLifecycleOperationSnapshot,
    operation: StateRoughLifecycleOperationSnapshot,
    recovery_operation: StateRoughLifecycleOperationSnapshot,
}

#[derive(Debug, Clone, Serialize)]
struct StateRoughLifecycleConcurrencyControl {
    engine_lifecycle: StateRoughControlEngineLifecycle,
    workers: usize,
    overlap_observed: bool,
    serial_svg: StateRoughSvgIdentity,
    worker_svgs: Vec<StateRoughSvgIdentity>,
    recovery_svg: StateRoughSvgIdentity,
    serial_operation: StateRoughLifecycleOperationSnapshot,
    operations: Vec<StateRoughLifecycleOperationSnapshot>,
    recovery_operation: StateRoughLifecycleOperationSnapshot,
}

#[derive(Debug, Clone, Serialize)]
struct StateRoughLifecycleControlsReceipt {
    schema: &'static str,
    error: StateRoughLifecycleFailureControl,
    unwind: StateRoughLifecycleFailureControl,
    concurrency: StateRoughLifecycleConcurrencyControl,
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

fn state_rough_svg_receipt(svg: &str) -> StateRoughSvgIdentity {
    StateRoughSvgIdentity {
        bytes: svg.len(),
        elements: svg_element_count(svg),
        identity: svg_identity(svg),
    }
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

fn state_probe_svg_options() -> crate::svg::SvgRenderOptions {
    crate::svg::SvgRenderOptions {
        diagram_id: Some("state-rough-lifecycle-probe".to_string()),
        ..crate::svg::SvgRenderOptions::default()
    }
}

fn render_state_probe_artifact(
    artifact: crate::family::FamilyRenderArtifact,
) -> (String, StateRoughLifecycleOperationSnapshot) {
    let capture = state_rough_lifecycle_capture();
    let rendered = artifact
        .render_svg(
            &state_probe_svg_options(),
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
    operation
        .release_proof
        .validate(
            operation.counters,
            operation.operation_peak,
            operation.cache_allowed,
        )
        .expect("State Rough operation release proof should hold");
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
    let mut max_post_operation_retained = StateRoughRetainedSnapshot::default();
    let mut checkpoints = Vec::with_capacity(LONG_LIVED_REQUEST_CHECKPOINTS.len());
    let mut final_retained = StateRoughRetainedSnapshot::default();
    let mut release_proofs = Vec::with_capacity(LONG_LIVED_REQUEST_COUNT);

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
        max_post_operation_retained.observe_peak(final_retained);
        release_proofs.push(StateRoughLongLivedReleaseProof {
            request_count,
            cache_allowed: request.operation.cache_allowed,
            counters: request.operation.counters,
            operation_peak: request.operation.operation_peak,
            release_proof: request.operation.release_proof,
        });

        if LONG_LIVED_REQUEST_CHECKPOINTS.contains(&request_count) {
            checkpoints.push(StateRoughRetainedCheckpoint {
                request_count,
                geometry_label_bytes,
                configured_seed: request.operation.configured_seed,
                retained: final_retained,
            });
        }
    }

    let release_rollup =
        state_rough_release_rollup(release_proofs.iter().map(|proof| proof.release_proof));
    StateRoughLongLivedReceipt {
        request_count: LONG_LIVED_REQUEST_COUNT,
        request_count_checkpoints: LONG_LIVED_REQUEST_CHECKPOINTS.to_vec(),
        checkpoints,
        svg: svg_rollup.finish(),
        counters,
        max_operation_peak,
        max_post_operation_retained,
        final_retained,
        release_proofs,
        release_rollup,
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
    let release = state_rough_release_rollup(
        requests
            .iter()
            .map(|request| request.operation.release_proof)
            .chain(
                long_lived
                    .release_proofs
                    .iter()
                    .map(|proof| proof.release_proof),
            ),
    );

    StateRoughLifecycleReceipt {
        schema: STATE_ROUGH_LIFECYCLE_SCHEMA,
        contracts: StateRoughLifecycleContracts {
            owned_bytes: OWNED_BYTES_DEFINITION,
            release_proof: RELEASE_PROOF_DEFINITION,
            render_cancellation: RENDER_CANCELLATION_CONTRACT,
            early_termination_proof: EARLY_TERMINATION_PROOF,
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
            release,
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
    if receipt.contracts.release_proof != RELEASE_PROOF_DEFINITION {
        return Err("release-proof definition drifted".to_string());
    }
    if receipt.contracts.render_cancellation != RENDER_CANCELLATION_CONTRACT {
        return Err("render-cancellation claim boundary drifted".to_string());
    }
    if receipt.contracts.early_termination_proof != EARLY_TERMINATION_PROOF {
        return Err("early-termination proof definition drifted".to_string());
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
        request.operation.release_proof.validate(
            request.operation.counters,
            request.operation.operation_peak,
            request.operation.cache_allowed,
        )?;
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
    if receipt.long_lived.release_proofs.len() != receipt.long_lived.request_count {
        return Err("long-lived release-proof cardinality drifted".to_string());
    }
    let mut expected_long_lived_counters = StateRoughOperationCounters::default();
    let mut expected_long_lived_peak = StateRoughCacheFootprint::default();
    for (index, proof) in receipt.long_lived.release_proofs.iter().enumerate() {
        if proof.request_count != index + 1 || !proof.cache_allowed {
            return Err(format!(
                "long-lived release proof {} does not match the seeded schedule",
                index + 1
            ));
        }
        proof.counters.validate()?;
        proof
            .release_proof
            .validate(proof.counters, proof.operation_peak, proof.cache_allowed)?;
        for counters in [proof.counters.circle, proof.counters.paths] {
            if counters.draw_requests == 0
                || counters.operation_hits == 0
                || counters.operation_misses == 0
            {
                return Err(format!(
                    "long-lived release proof {} did not exercise and reuse both geometry kinds",
                    proof.request_count
                ));
            }
        }
        expected_long_lived_counters.checked_add_assign(proof.counters);
        expected_long_lived_peak.observe_peak(
            proof.operation_peak.entries,
            proof.operation_peak.owned_bytes,
        );
    }
    if receipt.long_lived.counters != expected_long_lived_counters
        || receipt.long_lived.max_operation_peak != expected_long_lived_peak
        || receipt.long_lived.release_rollup
            != state_rough_release_rollup(
                receipt
                    .long_lived
                    .release_proofs
                    .iter()
                    .map(|proof| proof.release_proof),
            )
    {
        return Err("long-lived operation or release rollup drifted".to_string());
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
        || receipt.rollup.release
            != state_rough_release_rollup(
                receipt
                    .requests
                    .iter()
                    .map(|request| request.operation.release_proof)
                    .chain(
                        receipt
                            .long_lived
                            .release_proofs
                            .iter()
                            .map(|proof| proof.release_proof),
                    ),
            )
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
fn consumed_after_root_guard_cannot_clear_a_new_action() {
    let _test_lock = lifecycle_test_lock();
    let (first_svg_tx, _first_svg_rx) = mpsc::channel();
    let first = state_rough_lifecycle_install_after_root_action(
        StateRoughLifecycleAfterRootAction::ReturnError {
            completed_svg: first_svg_tx,
        },
    );
    let first_error =
        state_rough_lifecycle_after_root("<svg/>").expect_err("first action should fire");
    assert!(first_error.to_string().contains(AFTER_ROOT_ERROR_SENTINEL));

    let (second_svg_tx, _second_svg_rx) = mpsc::channel();
    let second = state_rough_lifecycle_install_after_root_action(
        StateRoughLifecycleAfterRootAction::ReturnError {
            completed_svg: second_svg_tx,
        },
    );
    drop(first);
    let second_error =
        state_rough_lifecycle_after_root("<svg/>").expect_err("second action should survive");
    assert!(second_error.to_string().contains(AFTER_ROOT_ERROR_SENTINEL));
    drop(second);
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
            release_proof: if cache_allowed {
                StateRoughReleaseProof {
                    cache_drop_observed: true,
                    geometry_witnesses: 2,
                    allocation_witnesses: 3,
                    witnessed_owned_bytes: 16,
                    live_allocation_witnesses: 0,
                    live_owned_bytes: 0,
                }
            } else {
                StateRoughReleaseProof {
                    cache_drop_observed: true,
                    ..StateRoughReleaseProof::default()
                }
            },
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
    let long_lived_geometry_counters = StateRoughGeometryCounters {
        draw_requests: 2,
        operation_lookups: 2,
        operation_hits: 1,
        operation_misses: 1,
        operation_builds: 1,
        ..StateRoughGeometryCounters::default()
    };
    let long_lived_operation_counters = StateRoughOperationCounters {
        circle: long_lived_geometry_counters,
        paths: long_lived_geometry_counters,
    };
    let long_lived_operation_peak = StateRoughCacheFootprint {
        entries: 2,
        owned_bytes: 16,
    };
    let long_lived_operation_release = StateRoughReleaseProof {
        cache_drop_observed: true,
        geometry_witnesses: 2,
        allocation_witnesses: 3,
        witnessed_owned_bytes: 16,
        live_allocation_witnesses: 0,
        live_owned_bytes: 0,
    };
    let long_lived_release_proofs = (1..=LONG_LIVED_REQUEST_COUNT)
        .map(|request_count| StateRoughLongLivedReleaseProof {
            request_count,
            cache_allowed: true,
            counters: long_lived_operation_counters,
            operation_peak: long_lived_operation_peak,
            release_proof: long_lived_operation_release,
        })
        .collect::<Vec<_>>();
    let mut long_lived_counters = StateRoughOperationCounters::default();
    for _ in 0..LONG_LIVED_REQUEST_COUNT {
        long_lived_counters.checked_add_assign(long_lived_operation_counters);
    }
    let long_lived = StateRoughLongLivedReceipt {
        request_count: LONG_LIVED_REQUEST_COUNT,
        request_count_checkpoints: LONG_LIVED_REQUEST_CHECKPOINTS.to_vec(),
        checkpoints: long_lived_checkpoints,
        svg: StateRoughSvgRollupBuilder::default().finish(),
        counters: long_lived_counters,
        max_operation_peak: long_lived_operation_peak,
        max_post_operation_retained: StateRoughRetainedSnapshot::default(),
        final_retained: StateRoughRetainedSnapshot::default(),
        release_rollup: state_rough_release_rollup(
            long_lived_release_proofs
                .iter()
                .map(|proof| proof.release_proof),
        ),
        release_proofs: long_lived_release_proofs,
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
    assert_eq!(
        value["contracts"]["release_proof"],
        RELEASE_PROOF_DEFINITION
    );
    assert_eq!(
        value["contracts"]["render_cancellation"],
        RENDER_CANCELLATION_CONTRACT
    );
    assert_eq!(
        value["contracts"]["early_termination_proof"],
        EARLY_TERMINATION_PROOF
    );
}

fn state_rough_lifecycle_control_spec() -> ProbeRequestSpec {
    ProbeRequestSpec {
        case: "release-control",
        configured_seed: 23.0,
        geometry_label_bytes: 32,
        ordinary_nodes: 6,
        render_thread: ProbeRenderThread::Primary,
    }
}

fn take_one_control_snapshot(label: &str) -> StateRoughLifecycleOperationSnapshot {
    let mut snapshots = state_rough_lifecycle_take_snapshots();
    assert_eq!(
        snapshots.len(),
        1,
        "{label} should produce exactly one lifecycle snapshot"
    );
    snapshots.pop().expect("one lifecycle snapshot")
}

fn validate_control_operation(
    label: &str,
    operation: &StateRoughLifecycleOperationSnapshot,
    expected_outcome: StateRoughLifecycleOutcome,
) {
    assert_eq!(operation.outcome, expected_outcome, "{label} outcome");
    assert!(operation.cache_allowed, "{label} should use seeded caching");
    operation
        .counters
        .validate()
        .unwrap_or_else(|error| panic!("{label} counters should be exact: {error}"));
    operation
        .release_proof
        .validate(
            operation.counters,
            operation.operation_peak,
            operation.cache_allowed,
        )
        .unwrap_or_else(|error| panic!("{label} release proof should be exact: {error}"));
    for (geometry, counters) in [
        ("circle", operation.counters.circle),
        ("paths", operation.counters.paths),
    ] {
        assert!(
            counters.draw_requests > 0,
            "{label} should render {geometry} geometry"
        );
        assert!(
            counters.operation_lookups > 0,
            "{label} should look up {geometry} geometry"
        );
        assert!(
            counters.operation_hits > 0,
            "{label} should reuse {geometry} geometry within the operation"
        );
        assert!(
            counters.operation_misses > 0,
            "{label} should populate {geometry} geometry within the operation"
        );
        assert_eq!(
            counters.bypass_builds, 0,
            "{label} should not bypass seeded {geometry} geometry"
        );
    }
    assert!(
        operation.operation_peak.entries > 0,
        "{label} should populate the operation cache"
    );
    assert!(
        operation.operation_peak.owned_bytes > 0,
        "{label} should retain operation-owned path bytes while rendering"
    );
    assert!(
        operation.release_proof.geometry_witnesses > 0,
        "{label} should witness cached geometry"
    );
    assert!(
        operation.release_proof.allocation_witnesses > 0,
        "{label} should witness cached String allocations"
    );
}

fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> Option<&str> {
    payload
        .downcast_ref::<&'static str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
}

fn render_successful_control_operation(
    engine: &merman_core::Engine,
    environment: &crate::environment::RenderEnvironment,
    spec: ProbeRequestSpec,
    label: &str,
) -> (String, StateRoughLifecycleOperationSnapshot) {
    let (artifact, _) = prepare_state_probe_artifact(engine, environment, spec);
    let (svg, operation) = render_state_probe_artifact(artifact);
    validate_control_operation(label, &operation, StateRoughLifecycleOutcome::Success);
    (svg, operation)
}

fn receive_completed_control_svg(receiver: mpsc::Receiver<String>, label: &str) -> String {
    receiver
        .recv_timeout(CONTROL_TIMEOUT)
        .unwrap_or_else(|error| panic!("{label} did not capture its completed SVG: {error}"))
}

fn state_rough_lifecycle_error_control() -> StateRoughLifecycleFailureControl {
    state_rough_lifecycle_probe_reset();
    let engine = merman_core::Engine::new();
    let environment = crate::environment::RenderEnvironment::deterministic();
    let spec = state_rough_lifecycle_control_spec();
    let (reference_svg, reference_operation) =
        render_successful_control_operation(&engine, &environment, spec, "error reference");

    state_rough_lifecycle_probe_reset();
    let (artifact, _) = prepare_state_probe_artifact(&engine, &environment, spec);

    let (failure_svg_tx, failure_svg_rx) = mpsc::channel();
    let capture = state_rough_lifecycle_capture();
    let action = state_rough_lifecycle_install_after_root_action(
        StateRoughLifecycleAfterRootAction::ReturnError {
            completed_svg: failure_svg_tx,
        },
    );
    let error = match artifact.render_svg(
        &state_probe_svg_options(),
        &crate::svg::SvgDebugOptions::default(),
    ) {
        Ok(_) => panic!("error control should fail after State root rendering"),
        Err(error) => error,
    };
    drop(action);
    drop(capture);
    let failure_svg = receive_completed_control_svg(failure_svg_rx, "error control");
    assert!(
        error.to_string().contains(AFTER_ROOT_ERROR_SENTINEL),
        "unexpected error control result: {error}"
    );

    let operation = take_one_control_snapshot("error control");
    validate_control_operation(
        "error control",
        &operation,
        StateRoughLifecycleOutcome::Error,
    );
    let (recovery_svg, recovery_operation) =
        render_successful_control_operation(&engine, &environment, spec, "error recovery");
    assert_eq!(
        reference_svg.as_bytes(),
        failure_svg.as_bytes(),
        "error control should fail only after producing the reference SVG"
    );
    assert_eq!(
        reference_svg.as_bytes(),
        recovery_svg.as_bytes(),
        "error control should recover on the same Engine without output drift"
    );
    StateRoughLifecycleFailureControl {
        sentinel: AFTER_ROOT_ERROR_SENTINEL,
        engine_lifecycle: StateRoughControlEngineLifecycle {
            engine_instances: 1,
            engine_reused_across_requests: true,
            request_count: 3,
        },
        reference_svg: state_rough_svg_receipt(&reference_svg),
        failure_svg: state_rough_svg_receipt(&failure_svg),
        recovery_svg: state_rough_svg_receipt(&recovery_svg),
        reference_operation,
        operation,
        recovery_operation,
    }
}

fn state_rough_lifecycle_unwind_control() -> StateRoughLifecycleFailureControl {
    state_rough_lifecycle_probe_reset();
    let engine = merman_core::Engine::new();
    let environment = crate::environment::RenderEnvironment::deterministic();
    let spec = state_rough_lifecycle_control_spec();
    let (reference_svg, reference_operation) =
        render_successful_control_operation(&engine, &environment, spec, "unwind reference");

    state_rough_lifecycle_probe_reset();
    let (artifact, _) = prepare_state_probe_artifact(&engine, &environment, spec);

    let (failure_svg_tx, failure_svg_rx) = mpsc::channel();
    let capture = state_rough_lifecycle_capture();
    let action = state_rough_lifecycle_install_after_root_action(
        StateRoughLifecycleAfterRootAction::Panic {
            completed_svg: failure_svg_tx,
        },
    );
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _ = artifact.render_svg(
            &state_probe_svg_options(),
            &crate::svg::SvgDebugOptions::default(),
        );
    }))
    .expect_err("unwind control should panic after State root rendering");
    drop(action);
    drop(capture);
    let failure_svg = receive_completed_control_svg(failure_svg_rx, "unwind control");
    assert_eq!(
        panic_payload_message(unwind.as_ref()),
        Some(AFTER_ROOT_PANIC_SENTINEL)
    );

    let operation = take_one_control_snapshot("unwind control");
    validate_control_operation(
        "unwind control",
        &operation,
        StateRoughLifecycleOutcome::Unwind,
    );
    let (recovery_svg, recovery_operation) =
        render_successful_control_operation(&engine, &environment, spec, "unwind recovery");
    assert_eq!(
        reference_svg.as_bytes(),
        failure_svg.as_bytes(),
        "unwind control should panic only after producing the reference SVG"
    );
    assert_eq!(
        reference_svg.as_bytes(),
        recovery_svg.as_bytes(),
        "unwind control should recover on the same Engine without output drift"
    );
    StateRoughLifecycleFailureControl {
        sentinel: AFTER_ROOT_PANIC_SENTINEL,
        engine_lifecycle: StateRoughControlEngineLifecycle {
            engine_instances: 1,
            engine_reused_across_requests: true,
            request_count: 3,
        },
        reference_svg: state_rough_svg_receipt(&reference_svg),
        failure_svg: state_rough_svg_receipt(&failure_svg),
        recovery_svg: state_rough_svg_receipt(&recovery_svg),
        reference_operation,
        operation,
        recovery_operation,
    }
}

fn state_rough_lifecycle_concurrency_control() -> StateRoughLifecycleConcurrencyControl {
    state_rough_lifecycle_probe_reset();
    let spec = state_rough_lifecycle_control_spec();

    let engine = merman_core::Engine::new();
    let environment = crate::environment::RenderEnvironment::deterministic();
    let (serial_svg, serial_operation) = render_successful_control_operation(
        &engine,
        &environment,
        spec,
        "concurrency serial control",
    );

    state_rough_lifecycle_probe_reset();
    let worker_artifacts = (0..2)
        .map(|_| prepare_state_probe_artifact(&engine, &environment, spec).0)
        .collect::<Vec<_>>();
    let (reached_tx, reached_rx) = mpsc::sync_channel(2);
    let mut releases = Vec::with_capacity(2);
    let mut workers = Vec::with_capacity(2);
    for (worker, artifact) in worker_artifacts.into_iter().enumerate() {
        let reached = reached_tx.clone();
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        releases.push(release_tx);
        workers.push(std::thread::spawn(move || {
            assert!(
                state_rough_lifecycle_take_snapshots().is_empty(),
                "worker lifecycle snapshots should start empty"
            );
            let capture = state_rough_lifecycle_capture();
            let action = state_rough_lifecycle_install_after_root_action(
                StateRoughLifecycleAfterRootAction::Rendezvous {
                    worker,
                    reached,
                    release: release_rx,
                },
            );
            let svg = artifact
                .render_svg(
                    &state_probe_svg_options(),
                    &crate::svg::SvgDebugOptions::default(),
                )
                .expect("concurrent State Rough render should succeed")
                .svg()
                .to_owned();
            drop(action);
            drop(capture);
            let operation = take_one_control_snapshot("concurrent worker");
            (worker, svg, operation)
        }));
    }
    drop(reached_tx);

    let mut reached_workers = std::collections::BTreeSet::new();
    let mut coordination_errors = Vec::new();
    for _ in 0..2 {
        match reached_rx.recv_timeout(CONTROL_TIMEOUT) {
            Ok(worker) => {
                if !reached_workers.insert(worker) {
                    coordination_errors.push(format!(
                        "concurrent worker {worker} reported the rendezvous twice"
                    ));
                }
            }
            Err(error) => {
                coordination_errors.push(format!(
                    "concurrent workers did not both reach the rendezvous: {error}"
                ));
                break;
            }
        }
    }
    let expected_workers = [0, 1]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let overlap_observed = reached_workers == expected_workers;
    if !overlap_observed {
        coordination_errors.push(format!(
            "concurrent rendezvous workers drifted: {reached_workers:?}"
        ));
    }
    for (worker, release) in releases.into_iter().enumerate() {
        if let Err(error) = release.send(()) {
            coordination_errors.push(format!(
                "concurrent worker {worker} release channel failed: {error}"
            ));
        }
    }

    let mut results = Vec::with_capacity(workers.len());
    for (worker, handle) in workers.into_iter().enumerate() {
        match handle.join() {
            Ok(result) => results.push(result),
            Err(payload) => coordination_errors.push(format!(
                "concurrent State Rough worker {worker} panicked: {}",
                panic_payload_message(payload.as_ref()).unwrap_or("non-string panic payload")
            )),
        }
    }
    assert!(
        coordination_errors.is_empty(),
        "concurrent State Rough control failed after joining every worker: {}",
        coordination_errors.join("; ")
    );
    results.sort_by_key(|(worker, _, _)| *worker);

    let mut worker_svgs = Vec::with_capacity(results.len());
    let mut operations = Vec::with_capacity(results.len());
    for (worker, svg, operation) in results {
        assert_eq!(
            svg.as_bytes(),
            serial_svg.as_bytes(),
            "worker {worker} SVG should match serial byte-for-byte"
        );
        validate_control_operation(
            &format!("concurrent worker {worker}"),
            &operation,
            StateRoughLifecycleOutcome::Success,
        );
        worker_svgs.push(state_rough_svg_receipt(&svg));
        operations.push(operation);
    }

    let (recovery_svg, recovery_operation) = render_successful_control_operation(
        &engine,
        &environment,
        spec,
        "post-concurrency recovery",
    );
    assert_eq!(
        serial_svg.as_bytes(),
        recovery_svg.as_bytes(),
        "concurrent control should recover on the same Engine without output drift"
    );

    StateRoughLifecycleConcurrencyControl {
        engine_lifecycle: StateRoughControlEngineLifecycle {
            engine_instances: 1,
            engine_reused_across_requests: true,
            request_count: 4,
        },
        workers: 2,
        overlap_observed,
        serial_svg: state_rough_svg_receipt(&serial_svg),
        worker_svgs,
        recovery_svg: state_rough_svg_receipt(&recovery_svg),
        serial_operation,
        operations,
        recovery_operation,
    }
}

#[test]
#[ignore = "State Rough lifecycle error, unwind, and concurrency controls; run explicitly"]
fn state_rough_lifecycle_release_controls() {
    let _test_lock = lifecycle_test_lock();
    let receipt = StateRoughLifecycleControlsReceipt {
        schema: STATE_ROUGH_LIFECYCLE_CONTROLS_SCHEMA,
        error: state_rough_lifecycle_error_control(),
        unwind: state_rough_lifecycle_unwind_control(),
        concurrency: state_rough_lifecycle_concurrency_control(),
    };
    assert_eq!(receipt.concurrency.worker_svgs.len(), 2);
    assert_eq!(receipt.concurrency.operations.len(), 2);
    assert!(receipt.concurrency.overlap_observed);
    println!(
        "{STATE_ROUGH_LIFECYCLE_CONTROLS_MARKER}{}",
        serde_json::to_string(&receipt).expect("lifecycle controls receipt should serialize")
    );
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
