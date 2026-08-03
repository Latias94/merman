use std::collections::HashMap;
use std::hash::Hash;
use std::mem::size_of;

#[derive(Debug)]
pub(crate) struct WeightedLru<K, V> {
    budget: usize,
    retained: u128,
    next_access: u64,
    entries: HashMap<K, WeightedEntry<V>>,
    #[cfg(test)]
    statistics: WeightedCacheStatistics,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct WeightedCacheStatistics {
    pub(crate) hits: usize,
    pub(crate) misses: usize,
    pub(crate) evictions: usize,
    pub(crate) oversized_entries: usize,
    pub(crate) current_weight: usize,
    pub(crate) high_water_weight: usize,
}

#[derive(Debug)]
struct WeightedEntry<V> {
    value: V,
    weight: usize,
    last_access: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WeightedReplaceOutcome {
    Replaced,
    MissingOrMismatch,
    Oversized,
    ReplacementWouldBeEvicted,
}

impl<K, V> WeightedLru<K, V>
where
    K: Clone + Eq + Hash + Ord,
{
    pub(crate) fn new(budget: usize) -> Self {
        Self {
            budget,
            retained: 0,
            next_access: 0,
            entries: HashMap::new(),
            #[cfg(test)]
            statistics: WeightedCacheStatistics::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn get(&mut self, key: &K) -> Option<&V> {
        self.get_if(key, |_| true)
    }

    pub(crate) fn get_if(&mut self, key: &K, matches: impl FnOnce(&V) -> bool) -> Option<&V> {
        if !self
            .entries
            .get(key)
            .is_some_and(|entry| matches(&entry.value))
        {
            #[cfg(test)]
            {
                self.statistics.misses = self.statistics.misses.saturating_add(1);
            }
            return None;
        }
        #[cfg(test)]
        {
            self.statistics.hits = self.statistics.hits.saturating_add(1);
        }
        let access = self.next_access();
        let entry = self
            .entries
            .get_mut(key)
            .expect("confirmed cache entry must still exist");
        entry.last_access = access;
        Some(&entry.value)
    }

    pub(crate) fn peek(&self, key: &K) -> Option<&V> {
        self.entries.get(key).map(|entry| &entry.value)
    }

    #[cfg(test)]
    pub(crate) fn contains_key(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    pub(crate) fn insert(&mut self, key: K, value: V, weight: usize) -> bool {
        if self.is_oversized(weight) {
            self.record_oversized_entry();
            return false;
        }
        self.remove(&key);

        let weight_u128 = weight as u128;
        while self.retained.saturating_add(weight_u128) > self.budget as u128 {
            self.evict_lru();
        }
        let last_access = self.next_access();
        let previous = self.entries.insert(
            key,
            WeightedEntry {
                value,
                weight,
                last_access,
            },
        );
        debug_assert!(previous.is_none());
        self.retained = self
            .retained
            .checked_add(weight_u128)
            .expect("admitted cache weight must not overflow u128");
        self.record_retained_weight();
        self.assert_within_budget();
        true
    }

    pub(crate) fn replace_if_preserving_recency(
        &mut self,
        key: &K,
        matches: impl FnOnce(&V) -> bool,
        value: V,
        weight: usize,
    ) -> WeightedReplaceOutcome {
        let Some(previous) = self.entries.get(key) else {
            return WeightedReplaceOutcome::MissingOrMismatch;
        };
        if !matches(&previous.value) {
            return WeightedReplaceOutcome::MissingOrMismatch;
        }
        if self.is_oversized(weight) {
            self.record_oversized_entry();
            return WeightedReplaceOutcome::Oversized;
        }

        let previous_weight = previous.weight;
        let mut projected_retained = self
            .retained
            .checked_sub(previous_weight as u128)
            .expect("replacement weight must match retained entries")
            .checked_add(weight as u128)
            .expect("replacement cache weight must not overflow u128");
        let mut victims = Vec::new();
        if projected_retained > self.budget as u128 {
            let mut candidates = self
                .entries
                .iter()
                .map(|(candidate, entry)| (entry.last_access, candidate.clone(), entry.weight))
                .collect::<Vec<_>>();
            candidates.sort_unstable_by(|left, right| (left.0, &left.1).cmp(&(right.0, &right.1)));
            for (_, victim, victim_weight) in candidates {
                if projected_retained <= self.budget as u128 {
                    break;
                }
                if victim == *key {
                    return WeightedReplaceOutcome::ReplacementWouldBeEvicted;
                }
                victims.push(victim);
                projected_retained = projected_retained
                    .checked_sub(victim_weight as u128)
                    .expect("replacement victim weight must be retained");
            }
        }
        debug_assert!(projected_retained <= self.budget as u128);

        for victim in victims {
            self.remove(&victim);
            #[cfg(test)]
            {
                self.statistics.evictions = self.statistics.evictions.saturating_add(1);
            }
        }
        let previous = self
            .entries
            .get_mut(key)
            .expect("matched replacement entry must still exist");
        previous.value = value;
        previous.weight = weight;
        self.retained = self
            .retained
            .checked_sub(previous_weight as u128)
            .expect("replacement weight must match retained entries")
            .checked_add(weight as u128)
            .expect("replacement cache weight must not overflow u128");
        self.record_retained_weight();
        self.assert_within_budget();
        WeightedReplaceOutcome::Replaced
    }

    pub(crate) fn remove(&mut self, key: &K) -> Option<V> {
        let entry = self.entries.remove(key)?;
        self.subtract_weight(entry.weight);
        self.record_retained_weight();
        Some(entry.value)
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.retained = 0;
        self.record_retained_weight();
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(key, entry)| (key, &entry.value))
    }

    #[cfg(test)]
    pub(crate) fn total_weight(&self) -> usize {
        usize::try_from(self.retained).expect("retained cache weight is bounded by usize budget")
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn statistics(&self) -> WeightedCacheStatistics {
        self.statistics
    }

    fn is_oversized(&self, weight: usize) -> bool {
        self.budget == 0 || weight == usize::MAX || weight > self.budget
    }

    fn next_access(&mut self) -> u64 {
        if self.next_access == u64::MAX {
            let mut order = self
                .entries
                .iter()
                .map(|(key, entry)| (entry.last_access, key.clone()))
                .collect::<Vec<_>>();
            order.sort_unstable();
            for (ordinal, (_, key)) in order.into_iter().enumerate() {
                self.entries
                    .get_mut(&key)
                    .expect("renormalized cache entry must still exist")
                    .last_access = u64::try_from(ordinal)
                    .expect("cache entry count must fit in the recency counter");
            }
            self.next_access = u64::try_from(self.entries.len())
                .expect("cache entry count must fit in the recency counter");
        }
        let access = self.next_access;
        self.next_access += 1;
        access
    }

    fn evict_lru(&mut self) {
        let victim = self
            .entries
            .iter()
            .min_by(|(left_key, left), (right_key, right)| {
                (left.last_access, *left_key).cmp(&(right.last_access, *right_key))
            })
            .map(|(key, _)| key.clone())
            .expect("an over-budget cache must have an eviction victim");
        self.remove(&victim);
        #[cfg(test)]
        {
            self.statistics.evictions = self.statistics.evictions.saturating_add(1);
        }
    }

    fn subtract_weight(&mut self, weight: usize) {
        self.retained = self
            .retained
            .checked_sub(weight as u128)
            .expect("cache weight must match retained entries");
    }

    fn assert_within_budget(&self) {
        debug_assert!(self.retained <= self.budget as u128);
    }

    #[cfg(test)]
    fn record_oversized_entry(&mut self) {
        self.statistics.oversized_entries = self.statistics.oversized_entries.saturating_add(1);
    }

    #[cfg(not(test))]
    fn record_oversized_entry(&mut self) {}

    #[cfg(test)]
    fn record_retained_weight(&mut self) {
        let current_weight = usize::try_from(self.retained)
            .expect("retained cache weight is bounded by usize budget");
        self.statistics.current_weight = current_weight;
        self.statistics.high_water_weight = self.statistics.high_water_weight.max(current_weight);
    }

    #[cfg(not(test))]
    fn record_retained_weight(&mut self) {}
}

pub(crate) fn conservative_weighted_entry_bytes<K, V>(key_heap_bytes: usize) -> usize {
    2usize
        .saturating_mul(size_of::<(K, WeightedEntry<V>)>())
        .saturating_add(key_heap_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_changes_the_next_victim_without_changing_weight() {
        let mut cache = WeightedLru::new(20);
        assert!(cache.insert("a", 1, 10));
        assert!(cache.insert("b", 2, 10));
        assert_eq!(cache.get(&"a"), Some(&1));
        let before = cache.total_weight();

        assert!(cache.insert("c", 3, 10));

        assert_eq!(cache.total_weight(), before);
        assert!(cache.contains_key(&"a"));
        assert!(!cache.contains_key(&"b"));
        assert!(cache.contains_key(&"c"));
    }

    #[test]
    fn zero_budget_and_saturated_weight_are_never_retained() {
        let mut zero = WeightedLru::new(0);
        assert!(!zero.insert("zero", 1, 0));
        assert_eq!(zero.len(), 0);

        let mut maximum = WeightedLru::new(usize::MAX);
        assert!(!maximum.insert("max", 1, usize::MAX));
        assert_eq!(maximum.total_weight(), 0);
    }

    #[test]
    fn exact_budget_is_retained_and_one_byte_more_is_not() {
        let mut cache = WeightedLru::new(10);
        assert!(cache.insert("exact", 1, 10));
        assert!(!cache.insert("large", 2, 11));
        assert!(cache.contains_key(&"exact"));
        assert_eq!(cache.total_weight(), 10);
    }

    #[test]
    fn oversized_insert_does_not_evict_healthy_residents() {
        let mut cache = WeightedLru::new(20);
        cache.insert("a", 1, 10);
        cache.insert("b", 2, 10);
        let evictions = cache.statistics().evictions;

        assert!(!cache.insert("large", 3, 21));

        assert!(cache.contains_key(&"a"));
        assert!(cache.contains_key(&"b"));
        assert_eq!(cache.statistics().evictions, evictions);
        assert_eq!(cache.statistics().oversized_entries, 1);
    }

    #[test]
    fn oversized_same_key_insert_preserves_the_resident_entry() {
        let mut cache = WeightedLru::new(10);
        assert!(cache.insert("a", 1, 10));
        let before = cache.statistics();

        assert!(!cache.insert("a", 2, 11));

        assert_eq!(cache.peek(&"a"), Some(&1));
        assert_eq!(cache.total_weight(), 10);
        assert_eq!(cache.statistics().evictions, before.evictions);
        assert_eq!(cache.statistics().current_weight, before.current_weight);
        assert_eq!(
            cache.statistics().high_water_weight,
            before.high_water_weight
        );
        assert_eq!(
            cache.statistics().oversized_entries,
            before.oversized_entries + 1
        );
    }

    #[test]
    fn statistics_cover_lookup_admission_and_retained_high_water() {
        let mut cache = WeightedLru::new(20);
        assert_eq!(cache.get(&"missing"), None);
        assert!(cache.insert("a", 1, 10));
        assert_eq!(cache.get(&"a"), Some(&1));
        assert!(cache.insert("b", 2, 10));
        assert!(cache.insert("c", 3, 10));
        assert!(!cache.insert("oversized", 4, 21));

        assert_eq!(
            cache.statistics(),
            WeightedCacheStatistics {
                hits: 1,
                misses: 1,
                evictions: 1,
                oversized_entries: 1,
                current_weight: 20,
                high_water_weight: 20,
            }
        );
    }

    #[test]
    fn conditional_replacement_preserves_recency_and_evicts_only_older_entries() {
        let mut cache = WeightedLru::new(25);
        assert!(cache.insert("a", 1, 5));
        assert!(cache.insert("b", 2, 10));
        assert!(cache.insert("c", 3, 10));
        assert_eq!(cache.get(&"a"), Some(&1));

        assert_eq!(
            cache.replace_if_preserving_recency(&"a", |value| *value == 1, 10, 15),
            WeightedReplaceOutcome::Replaced
        );

        assert!(cache.contains_key(&"a"));
        assert!(!cache.contains_key(&"b"));
        assert!(cache.contains_key(&"c"));
        assert_eq!(cache.get(&"a"), Some(&10));
        assert_eq!(cache.total_weight(), 25);
    }

    #[test]
    fn replacement_that_would_evict_itself_leaves_all_entries_unchanged() {
        let mut cache = WeightedLru::new(20);
        assert!(cache.insert("a", 1, 10));
        assert!(cache.insert("b", 2, 10));
        let before = cache.statistics();

        assert_eq!(
            cache.replace_if_preserving_recency(&"a", |value| *value == 1, 10, 15),
            WeightedReplaceOutcome::ReplacementWouldBeEvicted
        );

        assert_eq!(cache.peek(&"a"), Some(&1));
        assert_eq!(cache.peek(&"b"), Some(&2));
        assert_eq!(cache.total_weight(), 20);
        assert_eq!(cache.statistics(), before);
    }

    #[test]
    fn oversized_or_mismatched_replacement_leaves_the_resident_unchanged() {
        let mut cache = WeightedLru::new(10);
        assert!(cache.insert("a", 1, 10));

        assert_eq!(
            cache.replace_if_preserving_recency(&"a", |value| *value == 2, 2, 5),
            WeightedReplaceOutcome::MissingOrMismatch
        );
        assert_eq!(
            cache.replace_if_preserving_recency(&"a", |value| *value == 1, 2, 11),
            WeightedReplaceOutcome::Oversized
        );

        assert_eq!(cache.get(&"a"), Some(&1));
        assert_eq!(cache.total_weight(), 10);
    }

    #[test]
    fn recency_renormalization_is_stable() {
        let mut cache = WeightedLru::new(20);
        cache.insert("b", 2, 10);
        cache.insert("a", 1, 10);
        cache.next_access = u64::MAX;

        assert_eq!(cache.get(&"b"), Some(&2));
        cache.insert("c", 3, 10);

        assert!(cache.contains_key(&"b"));
        assert!(!cache.contains_key(&"a"));
        assert!(cache.contains_key(&"c"));
    }
}
