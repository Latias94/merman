use std::mem::size_of;

pub(crate) const ARC_ALLOCATION_OVERHEAD: usize = 2 * size_of::<usize>();

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
    2usize.saturating_mul(size_of::<(K, V)>())
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
}
