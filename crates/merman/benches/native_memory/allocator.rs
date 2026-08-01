use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering, fence};

pub(crate) struct CountingSystemAllocator {
    live_bytes: AtomicU64,
    allocation_count: AtomicU64,
    allocated_bytes: AtomicU64,
    peak_live_bytes: AtomicU64,
    measuring: AtomicBool,
    overflowed: AtomicBool,
    underflowed: AtomicBool,
}

#[derive(Debug)]
pub(crate) struct AllocationMetrics {
    pub(crate) snapshot_live_bytes: u64,
    pub(crate) allocation_count: u64,
    pub(crate) allocated_bytes: u64,
    pub(crate) live_bytes_after: u64,
    pub(crate) peak_live_bytes: u64,
    pub(crate) peak_growth_bytes: u64,
    pub(crate) counter_overflowed: bool,
    pub(crate) counter_underflowed: bool,
}

impl CountingSystemAllocator {
    pub(crate) const fn new() -> Self {
        Self {
            live_bytes: AtomicU64::new(0),
            allocation_count: AtomicU64::new(0),
            allocated_bytes: AtomicU64::new(0),
            peak_live_bytes: AtomicU64::new(0),
            measuring: AtomicBool::new(false),
            overflowed: AtomicBool::new(false),
            underflowed: AtomicBool::new(false),
        }
    }

    fn checked_add(&self, counter: &AtomicU64, amount: u64, damage: &AtomicBool) -> Option<u64> {
        let mut current = counter.load(Ordering::Relaxed);
        loop {
            let Some(next) = current.checked_add(amount) else {
                damage.store(true, Ordering::Relaxed);
                return None;
            };
            match counter.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => return Some(next),
                Err(observed) => current = observed,
            }
        }
    }

    fn checked_sub(&self, counter: &AtomicU64, amount: u64, damage: &AtomicBool) -> Option<u64> {
        let mut current = counter.load(Ordering::Relaxed);
        loop {
            let Some(next) = current.checked_sub(amount) else {
                damage.store(true, Ordering::Relaxed);
                return None;
            };
            match counter.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => return Some(next),
                Err(observed) => current = observed,
            }
        }
    }

    fn update_peak(&self, live_bytes: u64) {
        let mut peak = self.peak_live_bytes.load(Ordering::Relaxed);
        while live_bytes > peak {
            match self.peak_live_bytes.compare_exchange_weak(
                peak,
                live_bytes,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(observed) => peak = observed,
            }
        }
    }

    fn record_successful_allocation(&self, size: usize, live_bytes: Option<u64>) {
        if !self.measuring.load(Ordering::Relaxed) {
            return;
        }

        self.checked_add(&self.allocation_count, 1, &self.overflowed);
        self.checked_add(&self.allocated_bytes, size as u64, &self.overflowed);
        if let Some(live_bytes) = live_bytes {
            self.update_peak(live_bytes);
        }
    }

    fn record_new_allocation(&self, size: usize) {
        let live_bytes = self.checked_add(&self.live_bytes, size as u64, &self.overflowed);
        self.record_successful_allocation(size, live_bytes);
    }

    fn record_deallocation(&self, size: usize) {
        self.checked_sub(&self.live_bytes, size as u64, &self.underflowed);
    }

    fn record_reallocation(&self, old_size: usize, new_size: usize) {
        let live_bytes = if new_size >= old_size {
            self.checked_add(
                &self.live_bytes,
                (new_size - old_size) as u64,
                &self.overflowed,
            )
        } else {
            self.checked_sub(
                &self.live_bytes,
                (old_size - new_size) as u64,
                &self.underflowed,
            )
        };
        self.record_successful_allocation(new_size, live_bytes);
    }

    pub(crate) fn begin_measurement(&self) -> u64 {
        self.measuring.store(false, Ordering::SeqCst);
        let snapshot_live_bytes = self.live_bytes.load(Ordering::SeqCst);
        self.allocation_count.store(0, Ordering::SeqCst);
        self.allocated_bytes.store(0, Ordering::SeqCst);
        self.peak_live_bytes
            .store(snapshot_live_bytes, Ordering::SeqCst);
        fence(Ordering::SeqCst);
        self.measuring.store(true, Ordering::SeqCst);
        snapshot_live_bytes
    }

    pub(crate) fn finish_measurement(&self, snapshot_live_bytes: u64) -> AllocationMetrics {
        self.measuring.store(false, Ordering::SeqCst);
        fence(Ordering::SeqCst);

        let allocation_count = self.allocation_count.load(Ordering::SeqCst);
        let allocated_bytes = self.allocated_bytes.load(Ordering::SeqCst);
        let live_bytes_after = self.live_bytes.load(Ordering::SeqCst);
        let peak_live_bytes = self.peak_live_bytes.load(Ordering::SeqCst);
        let peak_growth_bytes = match peak_live_bytes.checked_sub(snapshot_live_bytes) {
            Some(value) => value,
            None => {
                self.underflowed.store(true, Ordering::SeqCst);
                0
            }
        };

        AllocationMetrics {
            snapshot_live_bytes,
            allocation_count,
            allocated_bytes,
            live_bytes_after,
            peak_live_bytes,
            peak_growth_bytes,
            counter_overflowed: self.overflowed.load(Ordering::SeqCst),
            counter_underflowed: self.underflowed.load(Ordering::SeqCst),
        }
    }

    #[cfg_attr(test, allow(dead_code))]
    pub(crate) fn stop_measurement(&self) {
        self.measuring.store(false, Ordering::SeqCst);
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn force_live_bytes_for_test(&self, value: u64) {
        self.live_bytes.store(value, Ordering::SeqCst);
    }
}

// SAFETY: every operation delegates to `System` with the original allocation contract. The
// atomic bookkeeping neither dereferences nor changes the pointer returned by the system allocator.
unsafe impl GlobalAlloc for CountingSystemAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies the `GlobalAlloc` layout contract unchanged to `System`.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            self.record_new_allocation(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies the `GlobalAlloc` layout contract unchanged to `System`.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            self.record_new_allocation(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the caller guarantees that `pointer` and `layout` identify a live allocation.
        unsafe { System.dealloc(pointer, layout) };
        self.record_deallocation(layout.size());
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller supplies the original allocation and valid new size unchanged.
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if !new_pointer.is_null() {
            self.record_reallocation(layout.size(), new_size);
        }
        new_pointer
    }
}
