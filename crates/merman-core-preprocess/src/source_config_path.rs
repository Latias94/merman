use std::fmt;
use std::sync::Arc;
#[cfg(feature = "test-support")]
use std::{collections::HashSet, mem::size_of};

/// Persistent configuration-key path shared by parser capture and retained source evidence.
///
/// Each key contributes one segment node and shares its ancestor nodes, so retained path data stays
/// proportional to the source rather than `key_count * nesting_depth`.
#[derive(Clone)]
pub struct SourceConfigPath {
    root: Option<Arc<SourceConfigPathNode>>,
    leaf: Option<Arc<SourceConfigPathNode>>,
}

struct SourceConfigPathNode {
    parent: Option<Arc<SourceConfigPathNode>>,
    segment: String,
    depth: usize,
}

impl Drop for SourceConfigPathNode {
    fn drop(&mut self) {
        let mut parent = self.parent.take();
        while let Some(mut node) = parent.and_then(Arc::into_inner) {
            parent = node.parent.take();
        }
    }
}

impl SourceConfigPath {
    pub const fn root() -> Self {
        Self {
            root: None,
            leaf: None,
        }
    }

    pub fn child(&self, segment: String) -> Self {
        let leaf = Arc::new(SourceConfigPathNode {
            parent: self.leaf.clone(),
            segment,
            depth: self.len().saturating_add(1),
        });
        Self {
            root: self.root.clone().or_else(|| Some(leaf.clone())),
            leaf: Some(leaf),
        }
    }

    pub fn first(&self) -> Option<&str> {
        self.root.as_deref().map(|node| node.segment.as_str())
    }

    fn len(&self) -> usize {
        self.leaf.as_deref().map_or(0, |node| node.depth)
    }

    fn segments_rev(&self) -> SourceConfigPathSegmentsRev<'_> {
        SourceConfigPathSegmentsRev {
            next: self.leaf.as_deref(),
        }
    }

    pub fn segments(&self) -> Vec<&str> {
        let mut segments = self.segments_rev().collect::<Vec<_>>();
        segments.reverse();
        segments
    }

    pub fn matches(&self, expected: &[&str]) -> bool {
        self.len() == expected.len()
            && self
                .segments_rev()
                .zip(expected.iter().rev().copied())
                .all(|(actual, expected)| actual == expected)
    }

    /// Estimates retained bytes across paths, counting each shared ancestor once.
    #[cfg(feature = "test-support")]
    pub fn estimated_paths_owned_heap_bytes<'a>(
        paths: impl IntoIterator<Item = &'a Self>,
    ) -> usize {
        let mut seen = HashSet::new();
        paths
            .into_iter()
            .map(|path| path.add_estimated_owned_heap_bytes(&mut seen))
            .fold(0usize, usize::saturating_add)
    }

    #[cfg(feature = "test-support")]
    fn add_estimated_owned_heap_bytes(
        &self,
        seen: &mut HashSet<*const SourceConfigPathNode>,
    ) -> usize {
        let mut retained = 0usize;
        let mut next = self.leaf.as_deref();
        while let Some(node) = next {
            if !seen.insert(node as *const SourceConfigPathNode) {
                break;
            }
            retained = retained
                .saturating_add(size_of::<SourceConfigPathNode>())
                .saturating_add(node.segment.capacity());
            next = node.parent.as_deref();
        }
        retained
    }
}

struct SourceConfigPathSegmentsRev<'a> {
    next: Option<&'a SourceConfigPathNode>,
}

impl<'a> Iterator for SourceConfigPathSegmentsRev<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.next?;
        self.next = node.parent.as_deref();
        Some(node.segment.as_str())
    }
}

impl fmt::Debug for SourceConfigPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.segments()).finish()
    }
}

impl PartialEq for SourceConfigPath {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .segments_rev()
                .zip(other.segments_rev())
                .all(|(left, right)| left == right)
    }
}

impl Eq for SourceConfigPath {}

#[cfg(test)]
mod tests {
    use super::SourceConfigPath;

    #[test]
    fn deep_shared_paths_drop_on_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                let mut path = SourceConfigPath::root().child("root".into());
                for _ in 0..10_000 {
                    path = path.child("child".into());
                }
                let sibling = path.child("sibling".into());
                drop(path);
                assert_eq!(sibling.first(), Some("root"));
                drop(sibling);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
