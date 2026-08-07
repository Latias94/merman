use crate::capability::{CapabilityKey, OperationKey, OutputKey};
use crate::metadata_registry::MetadataKey;
use crate::option_contract::BindingOptionGroupKey;
use crate::payload_contract::BindingPayloadSchemaKey;
use crate::service_contract::{ConstructorServiceKey, TextMeasurementProviderKey};
use std::fmt;
use std::marker::PhantomData;

pub(crate) trait CompactKey: Copy + Eq + 'static {
    const ALL: &'static [Self];

    fn bit(self) -> u64;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct KeySet<K> {
    bits: u64,
    marker: PhantomData<fn() -> K>,
}

impl<K> KeySet<K> {
    pub(crate) const fn empty() -> Self {
        Self {
            bits: 0,
            marker: PhantomData,
        }
    }

    pub(crate) const fn bits(self) -> u64 {
        self.bits
    }
}

impl<K: CompactKey> KeySet<K> {
    pub(crate) const fn from_bits(bits: u64) -> Self {
        let valid_bits = if K::ALL.len() == u64::BITS as usize {
            u64::MAX
        } else {
            (1_u64 << K::ALL.len()) - 1
        };
        if bits & !valid_bits != 0 {
            panic!("compact key set contains unknown bits");
        }
        Self {
            bits,
            marker: PhantomData,
        }
    }

    pub(crate) fn insert(&mut self, key: K) -> bool {
        let bit = key.bit();
        let inserted = self.bits & bit == 0;
        self.bits |= bit;
        inserted
    }

    pub(crate) fn contains(&self, key: K) -> bool {
        self.bits & key.bit() != 0
    }

    pub(crate) fn extend(&mut self, values: impl IntoIterator<Item = K>) {
        for value in values {
            self.insert(value);
        }
    }

    pub(crate) fn iter(&self) -> KeySetIter<K> {
        KeySetIter {
            remaining: self.bits,
            index: 0,
            marker: PhantomData,
        }
    }
}

impl<K> Default for KeySet<K> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<K: CompactKey> FromIterator<K> for KeySet<K> {
    fn from_iter<T: IntoIterator<Item = K>>(iter: T) -> Self {
        let mut keys = Self::empty();
        keys.extend(iter);
        keys
    }
}

impl<K: CompactKey> IntoIterator for &KeySet<K> {
    type Item = K;
    type IntoIter = KeySetIter<K>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<K: CompactKey + fmt::Debug> fmt::Debug for KeySet<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_set().entries(self.iter()).finish()
    }
}

pub(crate) struct KeySetIter<K> {
    remaining: u64,
    index: usize,
    marker: PhantomData<fn() -> K>,
}

impl<K: CompactKey> Iterator for KeySetIter<K> {
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < K::ALL.len() {
            let key = K::ALL[self.index];
            self.index += 1;
            let bit = key.bit();
            if self.remaining & bit != 0 {
                self.remaining &= !bit;
                return Some(key);
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.remaining.count_ones() as usize;
        (remaining, Some(remaining))
    }
}

impl<K: CompactKey> ExactSizeIterator for KeySetIter<K> {}

macro_rules! impl_compact_key {
    ($key:ty) => {
        impl $key {
            pub(crate) const fn compact_bit(self) -> u64 {
                1_u64 << (self as u32)
            }
        }

        impl CompactKey for $key {
            const ALL: &'static [Self] = <$key>::ALL;

            fn bit(self) -> u64 {
                self.compact_bit()
            }
        }
    };
}

impl_compact_key!(CapabilityKey);
impl_compact_key!(OperationKey);
impl_compact_key!(OutputKey);
impl_compact_key!(MetadataKey);
impl_compact_key!(BindingPayloadSchemaKey);
impl_compact_key!(BindingOptionGroupKey);
impl_compact_key!(TextMeasurementProviderKey);
impl_compact_key!(ConstructorServiceKey);

const _: () = {
    assert!(CapabilityKey::ALL.len() <= 64);
    assert!(OperationKey::ALL.len() <= 64);
    assert!(OutputKey::ALL.len() <= 64);
    assert!(MetadataKey::ALL.len() <= 64);
    assert!(BindingPayloadSchemaKey::ALL.len() <= 64);
    assert!(BindingOptionGroupKey::ALL.len() <= 64);
    assert!(TextMeasurementProviderKey::ALL.len() <= 64);
    assert!(ConstructorServiceKey::ALL.len() <= 64);
};

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_compact_key_contract<K>(id: impl Fn(K) -> &'static str)
    where
        K: CompactKey + fmt::Debug,
    {
        assert!(K::ALL.len() <= u64::BITS as usize);
        for (index, key) in K::ALL.iter().copied().enumerate() {
            assert_eq!(key.bit(), 1_u64 << index, "{}", id(key));
            if index > 0 {
                let previous_id = id(K::ALL[index - 1]);
                let current_id = id(key);
                assert!(
                    previous_id < current_id,
                    "stable IDs must be strictly ordered: {previous_id:?} >= {current_id:?}"
                );
            }
        }
    }

    #[test]
    fn iteration_uses_descriptor_order() {
        let keys = [
            CapabilityKey::Svg,
            CapabilityKey::Analysis,
            CapabilityKey::Png,
        ]
        .into_iter()
        .collect::<KeySet<_>>();

        assert_eq!(
            keys.iter().collect::<Vec<_>>(),
            [
                CapabilityKey::Analysis,
                CapabilityKey::Png,
                CapabilityKey::Svg,
            ]
        );
    }

    #[test]
    fn compact_keys_follow_stable_descriptor_order() {
        assert_compact_key_contract(CapabilityKey::id);
        assert_compact_key_contract(OperationKey::id);
        assert_compact_key_contract(OutputKey::id);
        assert_compact_key_contract(MetadataKey::id);
        assert_compact_key_contract(BindingPayloadSchemaKey::id);
        assert_compact_key_contract(BindingOptionGroupKey::id);
        assert_compact_key_contract(TextMeasurementProviderKey::id);
        assert_compact_key_contract(ConstructorServiceKey::id);
    }

    #[test]
    fn raw_bits_cannot_break_exact_size_iteration() {
        let unknown_bit = 1_u64 << CapabilityKey::ALL.len();
        assert!(
            std::panic::catch_unwind(|| KeySet::<CapabilityKey>::from_bits(unknown_bit)).is_err()
        );
    }
}
