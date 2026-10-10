//! Allocation growth checks run on the public scanner, including diagram-free input.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::time::Instant;

struct CountingAllocator;
thread_local! {
    static ALLOCATED: Cell<Option<usize>> = const { Cell::new(None) };
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn record(bytes: usize) {
    let _ = ALLOCATED.try_with(|count| {
        if let Some(current) = count.get() {
            count.set(Some(current + bytes));
        }
    });
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let pointer = unsafe { System.realloc(pointer, layout, size) };
        if !pointer.is_null() {
            record(size);
        }
        pointer
    }
}

#[test]
fn deeply_nested_prose_has_linear_allocation_growth() {
    let mut previous = None;
    for depth in [1_000, 2_000, 4_000] {
        let source = format!("{}plain prose\n", "> ".repeat(depth));
        ALLOCATED.set(Some(0));
        let blocks = merman_doc::scan(&source);
        let allocated = ALLOCATED.replace(None).unwrap();
        assert!(blocks.is_empty());
        eprintln!(
            "depth={depth} input_bytes={} allocated_bytes={allocated}",
            source.len()
        );
        // Allows parser bookkeeping and allocator growth, but not retained prefixes per level.
        assert!(
            allocated < source.len() * 512,
            "excessive allocation: {allocated}"
        );
        if let Some(previous) = previous {
            assert!(allocated < previous * 3, "superlinear allocation growth");
        }
        previous = Some(allocated);
    }
}

#[test]
fn repeated_inline_prose_with_trailing_spaces_is_not_an_include() {
    for count in [1_000, 2_000, 4_000] {
        let source = format!("> {}{}\n", "x *y* ".repeat(count), " ".repeat(count * 8));
        let start = Instant::now();
        assert!(merman_doc::scan(&source).is_empty());
        // Timing is diagnostic only; correctness must not depend on host speed.
        eprintln!("input_bytes={} elapsed={:?}", source.len(), start.elapsed());
    }
}
