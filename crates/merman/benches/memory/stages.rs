use super::ProbeAllocator;
use super::allocator::{AllocationCheckpoint, AllocationMetrics};
use merman_model::benchmark_stages::{Stage, observe_stage_changes};
use std::cell::RefCell;

#[derive(Clone, Copy, Default)]
pub(super) struct StageHeap {
    pub(super) allocations: u64,
    pub(super) allocated_bytes: u64,
    pub(super) peak_growth_bytes: u64,
}

#[derive(Default)]
pub(super) struct StageReport {
    pub(super) stages: [StageHeap; Stage::ALL.len()],
    pub(super) unattributed: StageHeap,
}

struct ActiveCapture {
    allocator: &'static ProbeAllocator,
    report: StageReport,
    stage: Option<Stage>,
    start: AllocationCheckpoint,
    peak_live_bytes: u64,
    counter_underflowed: bool,
}

impl ActiveCapture {
    fn close_interval(&mut self, end: AllocationCheckpoint) {
        if self.counter_underflowed {
            return;
        }
        let (Some(allocations), Some(allocated_bytes), Some(peak_growth_bytes)) = (
            end.allocation_count
                .checked_sub(self.start.allocation_count),
            end.allocated_bytes.checked_sub(self.start.allocated_bytes),
            end.peak_live_bytes.checked_sub(self.start.live_bytes),
        ) else {
            self.counter_underflowed = true;
            return;
        };
        let sample = match self.stage {
            Some(stage) => &mut self.report.stages[stage as usize],
            None => &mut self.report.unattributed,
        };
        sample.allocations += allocations;
        sample.allocated_bytes += allocated_bytes;
        // A phase can recur after another phase changes how much memory is live.
        sample.peak_growth_bytes = sample.peak_growth_bytes.max(peak_growth_bytes);
        self.peak_live_bytes = self.peak_live_bytes.max(end.peak_live_bytes);
        self.start = end;
    }
}

thread_local! {
    static ACTIVE: RefCell<Option<ActiveCapture>> = const { RefCell::new(None) };
}

fn switch_stage(stage: Option<Stage>) {
    ACTIVE.with_borrow_mut(|active| {
        let active = active.as_mut().expect("stage allocation capture is active");
        active.close_interval(active.allocator.checkpoint(true));
        active.stage = stage;
    });
}

struct StopCapture(&'static ProbeAllocator);

impl Drop for StopCapture {
    fn drop(&mut self) {
        self.0.stop_measurement();
        ACTIVE.take();
    }
}

/// Observes the synchronous workload with the process-wide counting allocator.
pub(super) fn capture<R>(
    allocator: &'static ProbeAllocator,
    operation: impl FnOnce() -> R,
) -> (R, AllocationMetrics, StageReport) {
    let snapshot = allocator.begin_measurement();
    let stop = StopCapture(allocator);
    ACTIVE.set(Some(ActiveCapture {
        allocator,
        report: StageReport::default(),
        stage: None,
        start: AllocationCheckpoint {
            allocation_count: 0,
            allocated_bytes: 0,
            live_bytes: snapshot,
            peak_live_bytes: snapshot,
        },
        peak_live_bytes: snapshot,
        counter_underflowed: false,
    }));
    let output = observe_stage_changes(switch_stage, operation);
    let mut active = ACTIVE.take().expect("stage allocation capture is active");
    // Read the final interval's counters with a zero baseline. This capture validates its own
    // baselines below, so a bad interval cannot set the allocator's sticky underflow flag.
    let mut heap = allocator.finish_measurement(0);
    active.close_interval(AllocationCheckpoint {
        allocation_count: heap.allocation_count,
        allocated_bytes: heap.allocated_bytes,
        live_bytes: heap.live_bytes_after,
        peak_live_bytes: heap.peak_live_bytes,
    });
    // Phase boundaries reset only the interval peak. Recover the batch peak from the absolute
    // high-water marks of all intervals. Allocation count and traffic are continuous counters.
    heap.snapshot_live_bytes = snapshot;
    heap.peak_live_bytes = active.peak_live_bytes;
    let growth = active.peak_live_bytes.checked_sub(snapshot);
    heap.peak_growth_bytes = growth.unwrap_or_default();
    heap.counter_underflowed |= active.counter_underflowed || growth.is_none();
    drop(stop);
    (output, heap, active.report)
}
