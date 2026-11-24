use crate::flat::Expanded;
use crate::work::{LayoutError, LayoutResult};

pub(crate) fn clear_reserve<T>(values: &mut Vec<T>, len: usize) -> LayoutResult<()> {
    values.clear();
    values
        .try_reserve_exact(len)
        .map_err(|_| LayoutError::AllocationLimit)
}

pub(crate) fn capacity_bytes<T>(values: &Vec<T>) -> usize {
    values.capacity() * size_of::<T>()
}

/// Reusable component-local arrays. The byte cap applies between solves, including after errors.
#[derive(Debug)]
pub struct LayoutScratch {
    max_retained_bytes: usize,
    expanded: Expanded,
}

impl Default for LayoutScratch {
    fn default() -> Self {
        Self::with_max_retained_bytes(2 * 1024 * 1024)
    }
}
impl LayoutScratch {
    pub fn with_max_retained_bytes(max_retained_bytes: usize) -> Self {
        Self {
            max_retained_bytes,
            expanded: Expanded::default(),
        }
    }
    pub fn retained_bytes(&self) -> usize {
        self.expanded.retained_bytes()
    }
    pub(crate) fn frame(&mut self) -> Frame<'_> {
        Frame {
            expanded: &mut self.expanded,
            limit: self.max_retained_bytes,
        }
    }
}
pub(crate) struct Frame<'a> {
    pub(crate) expanded: &'a mut Expanded,
    limit: usize,
}
impl Drop for Frame<'_> {
    fn drop(&mut self) {
        if self.expanded.retained_bytes() > self.limit {
            *self.expanded = Expanded::default();
        }
    }
}
