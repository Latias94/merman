use super::analysis::AnalysisJobGeneration;
#[cfg(test)]
use super::cache::WeightedCacheStatistics;
use super::cache::{WeightedLru, WeightedReplaceOutcome, conservative_weighted_entry_bytes};
use crate::snapshot::{
    DiagnosticGeneration, DocumentAnalysisContext, DocumentEpoch, DocumentSnapshot,
    SnapshotGeneration,
};
use std::sync::Arc;
use tower_lsp_server::ls_types::Uri;

#[derive(Debug)]
pub(in crate::session) struct AnalysisCache {
    entries: WeightedLru<Uri, AnalysisCacheEntry>,
    next_incarnation: u64,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct AnalysisCacheIncarnation(u64);

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(in crate::session) struct AnalysisCacheAuthority(AnalysisCacheIncarnation);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::session) struct AnalysisCacheStamp {
    pub(in crate::session) document_epoch: DocumentEpoch,
    pub(in crate::session) snapshot_generation: SnapshotGeneration,
    pub(in crate::session) analysis_job_generation: AnalysisJobGeneration,
}

#[derive(Debug, Clone)]
pub(in crate::session) struct AnalysisCacheLease {
    snapshot: Arc<DocumentSnapshot>,
    context: Option<Arc<DocumentAnalysisContext>>,
    authority: AnalysisCacheAuthority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::session) enum AnalysisCachePromotion {
    Committed,
    SnapshotRetained,
    Stale,
}

#[derive(Debug, Clone)]
struct AnalysisCacheEntry {
    state: AnalysisCacheEntryState,
    stamp: AnalysisCacheStamp,
    incarnation: AnalysisCacheIncarnation,
}

#[derive(Debug, Clone)]
enum AnalysisCacheEntryState {
    SnapshotOnly(Arc<DocumentSnapshot>),
    Complete(Arc<DocumentAnalysisContext>),
}

impl AnalysisCache {
    pub(in crate::session) fn new(budget: usize) -> Self {
        Self {
            entries: WeightedLru::new(budget),
            next_incarnation: 0,
        }
    }

    pub(in crate::session) fn lookup(
        &mut self,
        uri: &Uri,
        stamp: AnalysisCacheStamp,
    ) -> Option<AnalysisCacheLease> {
        let lease = self
            .entries
            .get_if(uri, |entry| entry.stamp == stamp)
            .map(AnalysisCacheEntry::lease);
        if lease.is_none() {
            self.entries.remove(uri);
        }
        lease
    }

    pub(in crate::session) fn contains(&self, uri: &Uri, stamp: AnalysisCacheStamp) -> bool {
        self.entries
            .peek(uri)
            .is_some_and(|entry| entry.stamp == stamp)
    }

    pub(in crate::session) fn insert_snapshot(
        &mut self,
        uri: Uri,
        stamp: AnalysisCacheStamp,
        snapshot: Arc<DocumentSnapshot>,
    ) -> Option<AnalysisCacheAuthority> {
        let incarnation = self.issue_incarnation();
        let weight = snapshot_entry_weight(&uri, &snapshot);
        self.entries
            .insert(
                uri,
                AnalysisCacheEntry {
                    state: AnalysisCacheEntryState::SnapshotOnly(snapshot),
                    stamp,
                    incarnation,
                },
                weight,
            )
            .then_some(AnalysisCacheAuthority(incarnation))
    }

    pub(in crate::session) fn promote(
        &mut self,
        uri: &Uri,
        authority: AnalysisCacheAuthority,
        target_diagnostic_generation: DiagnosticGeneration,
        context: Arc<DocumentAnalysisContext>,
    ) -> AnalysisCachePromotion {
        let Some(existing) = self.entries.peek(uri) else {
            return AnalysisCachePromotion::Stale;
        };
        if existing.incarnation != authority.0
            || context.diagnostic_generation() != target_diagnostic_generation
            || existing.snapshot().analysis_result_identity() != context.analysis_result_identity()
            || !Arc::ptr_eq(existing.snapshot(), &context.snapshot)
        {
            return AnalysisCachePromotion::Stale;
        }

        let replacement = AnalysisCacheEntry {
            state: AnalysisCacheEntryState::Complete(Arc::clone(&context)),
            stamp: existing.stamp,
            incarnation: existing.incarnation,
        };
        let weight = complete_entry_weight(uri, &context);
        match self.entries.replace_if_preserving_recency(
            uri,
            |entry| entry.incarnation == authority.0,
            replacement,
            weight,
        ) {
            WeightedReplaceOutcome::Replaced => AnalysisCachePromotion::Committed,
            WeightedReplaceOutcome::Oversized
            | WeightedReplaceOutcome::ReplacementWouldBeEvicted => {
                AnalysisCachePromotion::SnapshotRetained
            }
            WeightedReplaceOutcome::MissingOrMismatch => AnalysisCachePromotion::Stale,
        }
    }

    pub(in crate::session) fn downgrade_complete_entries(&mut self) {
        let mut replacements = self
            .entries
            .iter()
            .filter_map(|(uri, entry)| {
                let AnalysisCacheEntryState::Complete(context) = &entry.state else {
                    return None;
                };
                let snapshot = Arc::clone(&context.snapshot);
                Some((
                    uri.clone(),
                    entry.incarnation,
                    AnalysisCacheEntry {
                        state: AnalysisCacheEntryState::SnapshotOnly(Arc::clone(&snapshot)),
                        stamp: entry.stamp,
                        incarnation: entry.incarnation,
                    },
                    snapshot_entry_weight(uri, &snapshot),
                ))
            })
            .collect::<Vec<_>>();
        replacements.sort_by(|left, right| left.0.cmp(&right.0));
        for (uri, incarnation, replacement, weight) in replacements {
            let outcome = self.entries.replace_if_preserving_recency(
                &uri,
                |entry| entry.incarnation == incarnation,
                replacement,
                weight,
            );
            debug_assert_eq!(outcome, WeightedReplaceOutcome::Replaced);
        }
    }

    pub(in crate::session) fn remove(&mut self, uri: &Uri) {
        self.entries.remove(uri);
    }

    pub(in crate::session) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(in crate::session) fn current_without_touch(
        &self,
        uri: &Uri,
        stamp: AnalysisCacheStamp,
    ) -> Option<AnalysisCacheLease> {
        self.entries
            .peek(uri)
            .filter(|entry| entry.stamp == stamp)
            .map(AnalysisCacheEntry::lease)
    }

    #[cfg(test)]
    pub(in crate::session) fn total_weight(&self) -> usize {
        self.entries.total_weight()
    }

    #[cfg(test)]
    pub(in crate::session) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(in crate::session) fn statistics(&self) -> WeightedCacheStatistics {
        self.entries.statistics()
    }

    #[cfg(test)]
    pub(in crate::session) fn snapshot_entry_weight(
        uri: &Uri,
        snapshot: &DocumentSnapshot,
    ) -> usize {
        snapshot_entry_weight(uri, snapshot)
    }

    #[cfg(test)]
    pub(in crate::session) fn complete_entry_weight(
        uri: &Uri,
        context: &DocumentAnalysisContext,
    ) -> usize {
        complete_entry_weight(uri, context)
    }

    fn issue_incarnation(&mut self) -> AnalysisCacheIncarnation {
        self.next_incarnation = self
            .next_incarnation
            .checked_add(1)
            .expect("analysis cache incarnation exhausted");
        AnalysisCacheIncarnation(self.next_incarnation)
    }
}

impl AnalysisCacheLease {
    pub(in crate::session) fn snapshot(&self) -> &Arc<DocumentSnapshot> {
        &self.snapshot
    }

    pub(in crate::session) fn context(&self) -> Option<&Arc<DocumentAnalysisContext>> {
        self.context.as_ref()
    }

    pub(in crate::session) fn authority(&self) -> AnalysisCacheAuthority {
        self.authority
    }
}

impl AnalysisCacheEntry {
    fn snapshot(&self) -> &Arc<DocumentSnapshot> {
        match &self.state {
            AnalysisCacheEntryState::SnapshotOnly(snapshot) => snapshot,
            AnalysisCacheEntryState::Complete(context) => &context.snapshot,
        }
    }

    fn lease(&self) -> AnalysisCacheLease {
        AnalysisCacheLease {
            snapshot: Arc::clone(self.snapshot()),
            context: match &self.state {
                AnalysisCacheEntryState::SnapshotOnly(_) => None,
                AnalysisCacheEntryState::Complete(context) => Some(Arc::clone(context)),
            },
            authority: AnalysisCacheAuthority(self.incarnation),
        }
    }
}

fn snapshot_entry_weight(uri: &Uri, snapshot: &DocumentSnapshot) -> usize {
    snapshot
        .estimated_owned_weight()
        .saturating_add(
            conservative_weighted_entry_bytes::<Uri, AnalysisCacheEntry>(uri.as_str().len()),
        )
}

fn complete_entry_weight(uri: &Uri, context: &DocumentAnalysisContext) -> usize {
    context
        .estimated_owned_weight()
        .total()
        .saturating_add(
            conservative_weighted_entry_bytes::<Uri, AnalysisCacheEntry>(uri.as_str().len()),
        )
}

#[cfg(test)]
mod tests {
    use super::super::analysis::request::{AnalysisBuildKey, AnalysisBuildRequest};
    use super::*;
    use merman_analysis::{AnalysisCancellationToken, Analyzer};
    use merman_editor_core::DocumentKind;
    use std::str::FromStr;

    fn test_uri(name: &str) -> Uri {
        Uri::from_str(&format!("file:///tmp/{name}.mmd")).unwrap()
    }

    fn test_stamp() -> AnalysisCacheStamp {
        AnalysisCacheStamp {
            document_epoch: DocumentEpoch(1),
            snapshot_generation: SnapshotGeneration(1),
            analysis_job_generation: AnalysisJobGeneration(1),
        }
    }

    fn test_snapshot(uri: &Uri) -> Arc<DocumentSnapshot> {
        AnalysisBuildRequest::new(
            AnalysisBuildKey::new(
                uri.clone(),
                1,
                AnalysisJobGeneration(1),
                SnapshotGeneration(1),
                DocumentEpoch(1),
            ),
            Arc::<str>::from("flowchart TD\nA-->B\n"),
            DocumentKind::Diagram,
            Analyzer::new(),
        )
        .build_cancellable(&AnalysisCancellationToken::new())
        .expect("test snapshot should build")
    }

    fn test_context(snapshot: Arc<DocumentSnapshot>) -> Arc<DocumentAnalysisContext> {
        test_context_for_generation(snapshot, DiagnosticGeneration(1))
    }

    fn test_context_for_generation(
        snapshot: Arc<DocumentSnapshot>,
        diagnostic_generation: DiagnosticGeneration,
    ) -> Arc<DocumentAnalysisContext> {
        let analyzer = Analyzer::new();
        Arc::new(
            DocumentAnalysisContext::project_cancellable(
                snapshot,
                analyzer.options().diagnostic_policy(),
                DocumentEpoch(1),
                diagnostic_generation,
                &AnalysisCancellationToken::new(),
            )
            .expect("test diagnostic projection should complete"),
        )
    }

    #[test]
    fn promotion_that_would_evict_its_snapshot_retains_the_true_lru_order() {
        let a = test_uri("strict-lru-a");
        let b = test_uri("strict-lru-b");
        let a_snapshot = test_snapshot(&a);
        let b_snapshot = test_snapshot(&b);
        let complete = test_context(Arc::clone(&a_snapshot));
        let snapshot_total = snapshot_entry_weight(&a, &a_snapshot)
            .checked_add(snapshot_entry_weight(&b, &b_snapshot))
            .unwrap();
        let complete_weight = complete_entry_weight(&a, &complete);
        let budget = snapshot_total.max(complete_weight);
        assert!(complete_weight.saturating_add(snapshot_entry_weight(&b, &b_snapshot)) > budget);
        let mut cache = AnalysisCache::new(budget);
        let authority = cache
            .insert_snapshot(a.clone(), test_stamp(), Arc::clone(&a_snapshot))
            .unwrap();
        cache
            .insert_snapshot(b.clone(), test_stamp(), b_snapshot)
            .unwrap();
        let before_weight = cache.total_weight();
        let before_statistics = cache.statistics();

        assert_eq!(
            cache.promote(&a, authority, DiagnosticGeneration(1), complete),
            AnalysisCachePromotion::SnapshotRetained
        );

        let a_lease = cache
            .current_without_touch(&a, test_stamp())
            .expect("the true LRU snapshot must remain");
        assert_eq!(a_lease.authority(), authority);
        assert!(a_lease.context().is_none());
        assert!(cache.current_without_touch(&b, test_stamp()).is_some());
        assert_eq!(cache.total_weight(), before_weight);
        assert_eq!(cache.statistics(), before_statistics);
    }

    #[test]
    fn evicted_projection_cannot_promote_a_reinserted_incarnation() {
        let a = test_uri("incarnation-a");
        let b = test_uri("incarnation-b");
        let snapshot = test_snapshot(&a);
        let filler = test_snapshot(&b);
        let budget = snapshot_entry_weight(&a, &snapshot).max(snapshot_entry_weight(&b, &filler));
        let mut cache = AnalysisCache::new(budget);
        let old_authority = cache
            .insert_snapshot(a.clone(), test_stamp(), Arc::clone(&snapshot))
            .unwrap();
        cache
            .insert_snapshot(b, test_stamp(), filler)
            .expect("filler snapshot should evict the original entry");
        assert!(cache.current_without_touch(&a, test_stamp()).is_none());
        let new_authority = cache
            .insert_snapshot(a.clone(), test_stamp(), Arc::clone(&snapshot))
            .unwrap();
        assert_ne!(old_authority, new_authority);
        let before_weight = cache.total_weight();
        let before_statistics = cache.statistics();

        assert_eq!(
            cache.promote(
                &a,
                old_authority,
                DiagnosticGeneration(1),
                test_context(Arc::clone(&snapshot)),
            ),
            AnalysisCachePromotion::Stale
        );

        let current = cache
            .current_without_touch(&a, test_stamp())
            .expect("the reinserted snapshot must remain");
        assert_eq!(current.authority(), new_authority);
        assert!(Arc::ptr_eq(current.snapshot(), &snapshot));
        assert!(current.context().is_none());
        assert_eq!(cache.total_weight(), before_weight);
        assert_eq!(cache.statistics(), before_statistics);
    }

    #[test]
    fn promotion_requires_the_exact_resident_snapshot_arc() {
        let uri = test_uri("snapshot-identity");
        let resident = test_snapshot(&uri);
        let equivalent = Arc::new(
            DocumentSnapshot::try_from_editor(resident.as_editor().clone())
                .expect("cloned editor snapshot should preserve its URI"),
        );
        assert_ne!(
            resident.analysis_result_identity(),
            equivalent.analysis_result_identity()
        );
        assert!(!Arc::ptr_eq(&resident, &equivalent));
        let mut cache = AnalysisCache::new(complete_entry_weight(
            &uri,
            &test_context(Arc::clone(&resident)),
        ));
        let authority = cache
            .insert_snapshot(uri.clone(), test_stamp(), resident)
            .unwrap();

        assert_eq!(
            cache.promote(
                &uri,
                authority,
                DiagnosticGeneration(1),
                test_context(equivalent),
            ),
            AnalysisCachePromotion::Stale
        );
        assert!(
            cache
                .current_without_touch(&uri, test_stamp())
                .unwrap()
                .context()
                .is_none()
        );
    }

    #[test]
    fn promotion_requires_the_target_diagnostic_generation() {
        let uri = test_uri("diagnostic-generation");
        let snapshot = test_snapshot(&uri);
        let stale_context =
            test_context_for_generation(Arc::clone(&snapshot), DiagnosticGeneration(1));
        let mut cache = AnalysisCache::new(complete_entry_weight(&uri, &stale_context));
        let authority = cache
            .insert_snapshot(uri.clone(), test_stamp(), Arc::clone(&snapshot))
            .unwrap();
        let before_weight = cache.total_weight();
        let before_statistics = cache.statistics();

        assert_eq!(
            cache.promote(&uri, authority, DiagnosticGeneration(2), stale_context,),
            AnalysisCachePromotion::Stale
        );

        let current = cache
            .current_without_touch(&uri, test_stamp())
            .expect("the snapshot-only entry must remain resident");
        assert!(Arc::ptr_eq(current.snapshot(), &snapshot));
        assert!(current.context().is_none());
        assert_eq!(cache.total_weight(), before_weight);
        assert_eq!(cache.statistics(), before_statistics);
    }

    #[test]
    fn diagnostic_downgrade_preserves_residency_and_releases_payload() {
        let uri = test_uri("diagnostic-downgrade");
        let snapshot = test_snapshot(&uri);
        let context = test_context(Arc::clone(&snapshot));
        let snapshot_weight = snapshot_entry_weight(&uri, &snapshot);
        let complete_weight = complete_entry_weight(&uri, &context);
        let payload = Arc::downgrade(&context.payload);
        let mut cache = AnalysisCache::new(complete_weight);
        let authority = cache
            .insert_snapshot(uri.clone(), test_stamp(), snapshot)
            .unwrap();
        assert_eq!(
            cache.promote(
                &uri,
                authority,
                DiagnosticGeneration(1),
                Arc::clone(&context),
            ),
            AnalysisCachePromotion::Committed
        );
        drop(context);
        assert!(payload.upgrade().is_some());

        cache.downgrade_complete_entries();

        let lease = cache
            .current_without_touch(&uri, test_stamp())
            .expect("snapshot must remain resident");
        assert_eq!(lease.authority(), authority);
        assert!(lease.context().is_none());
        assert_eq!(cache.total_weight(), snapshot_weight);
        assert!(payload.upgrade().is_none());
    }
}
