use std::mem::size_of;

pub(crate) const ARC_ALLOCATION_OVERHEAD: usize = 2 * size_of::<usize>();

// Charge every BTree entry as though it owned a sparsely populated node. This deliberately
// overestimates dense trees while remaining independent from the standard library's private node
// layout and allocator bookkeeping.
const CONSERVATIVE_BTREE_ENTRY_SLOTS: usize = 16;
const CONSERVATIVE_BTREE_NODE_METADATA_BYTES: usize = 256;

#[derive(Debug, Default)]
pub(crate) struct RetainedWeight {
    bytes: usize,
}

impl RetainedWeight {
    pub(crate) const fn new(bytes: usize) -> Self {
        Self { bytes }
    }

    pub(crate) fn add(&mut self, bytes: usize) {
        self.bytes = self.bytes.saturating_add(bytes);
    }

    pub(crate) fn add_array<T>(&mut self, capacity: usize) {
        self.add(capacity.saturating_mul(size_of::<T>()));
    }

    pub(crate) fn add_string(&mut self, value: &String) {
        self.add(value.capacity());
    }

    pub(crate) fn add_optional_string(&mut self, value: &Option<String>) {
        if let Some(value) = value {
            self.add_string(value);
        }
    }

    pub(crate) const fn finish(self) -> usize {
        self.bytes
    }
}

pub(crate) const fn conservative_btree_entry_bytes<K, V>() -> usize {
    CONSERVATIVE_BTREE_ENTRY_SLOTS
        .saturating_mul(size_of::<(K, V)>())
        .saturating_add(CONSERVATIVE_BTREE_NODE_METADATA_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulation_and_multiplication_saturate() {
        let mut weight = RetainedWeight::new(usize::MAX);
        weight.add(1);
        weight.add_array::<usize>(usize::MAX);
        assert_eq!(weight.finish(), usize::MAX);
    }

    #[test]
    fn strings_are_charged_by_capacity() {
        let mut value = String::with_capacity(128);
        value.push('x');
        let mut weight = RetainedWeight::default();
        weight.add_string(&value);
        assert_eq!(weight.finish(), 128);
    }

    #[test]
    fn btree_entries_are_charged_as_independent_sparse_nodes() {
        let pair_bytes = size_of::<(String, Vec<String>)>();
        assert_eq!(
            conservative_btree_entry_bytes::<String, Vec<String>>(),
            pair_bytes
                .saturating_mul(CONSERVATIVE_BTREE_ENTRY_SLOTS)
                .saturating_add(CONSERVATIVE_BTREE_NODE_METADATA_BYTES)
        );
        assert!(
            conservative_btree_entry_bytes::<String, Vec<String>>() > pair_bytes.saturating_mul(2)
        );
    }
}
