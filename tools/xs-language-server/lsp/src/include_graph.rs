//! Project-wide reverse-include graph: for every `.xs` file `F`, the set of
//! other `.xs` files that have `include "F"` (or a path resolving to `F`)
//! in their source text.
//!
//! Built by [`ReverseIncludeGraph::build`] from the forward [`IncludeGraph`]
//! already produced by [`crate::merged_view`]. The reverse view is read-only
//! after construction; the workspace orchestrator is responsible for
//! invalidating and rebuilding on edits (out of scope for PR-1 — PR-2 adds the
//! per-root cache and invalidation hooks).
//!
//! Used by `publish_diagnostics` to answer: "for an opened file `F`, which
//! roots (transitively) include `F`?" An empty result means `F` is an orphan
//! and should be treated as its own root.
//!
//! # Cycle policy
//!
//! The input forward graph already breaks cycles by omitting the re-entry
//! edge (see [`IncludeGraph::is_cyclic`]). The reverse BFS therefore walks
//! only the edges that are present and uses a `visited` guard, so a cyclic
//! forward graph cannot cause an infinite loop. If every reachable node is
//! part of a cycle, the query returns an empty root list; callers are
//! expected to fall back to treating the queried file as its own root.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use smallvec::SmallVec;

/// Direct includers of a single file.
///
/// Most `.xs` files have a small number of includers (often zero or one), so
/// `SmallVec<[PathBuf; 4]>` keeps the common case inline and avoids a separate
/// heap allocation for the vector header. This matters for PR-2's cache size
/// estimate: with a few hundred files and ~1-2 includers each, the inline
/// storage saves several kilobytes and reduces allocator pressure.
pub type DirectIncluders = SmallVec<[PathBuf; 4]>;

/// Reverse view of a forward include graph.
#[derive(Debug, Default, Clone)]
pub struct ReverseIncludeGraph {
    /// file_path → direct includers of file_path (i.e. files whose source
    /// contains an `include "..."` directive resolving to file_path).
    dependents: HashMap<PathBuf, DirectIncluders>,
    /// Every file that appears on either side of an include edge.
    files: HashSet<PathBuf>,
}

impl ReverseIncludeGraph {
    /// Build from the project's forward include graph. Pure function over the
    /// input — no I/O, no parsing.
    pub fn build(forward: &crate::merged_view::IncludeGraph) -> Self {
        let mut dependents: HashMap<PathBuf, DirectIncluders> = HashMap::new();
        let mut files: HashSet<PathBuf> = HashSet::new();

        for edge in forward.edges() {
            files.insert(edge.from.clone());
            files.insert(edge.to.clone());
            dependents
                .entry(edge.to.clone())
                .or_default()
                .push(edge.from.clone());
        }

        for list in dependents.values_mut() {
            list.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
            list.dedup();
        }

        Self { dependents, files }
    }

    /// Reverse-reachability: returns every file `P` such that there is a path
    /// `P → ... → F` in the forward include graph and `P` has no further
    /// includers (i.e. `P` is a root of `F`'s include chain).
    ///
    /// Order is sorted lexicographically by full path and duplicates are
    /// removed.
    pub fn roots_that_include(&self, file: &Path) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        visited.insert(file.to_path_buf());

        if let Some(parents) = self.dependents.get(file) {
            for parent in parents {
                if visited.insert(parent.clone()) {
                    queue.push_back(parent.clone());
                }
            }
        }

        while let Some(cur) = queue.pop_front() {
            let parents = self.dependents.get(&cur).map(|v| v.as_slice()).unwrap_or(&[]);
            if parents.is_empty() {
                roots.push(cur);
            } else {
                for parent in parents {
                    if visited.insert(parent.clone()) {
                        queue.push_back(parent.clone());
                    }
                }
            }
        }

        roots.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
        roots.dedup();
        roots
    }

    /// Number of files known to the graph. Useful for the cache key
    /// invalidation logic in PR-2.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// True if the graph contains no files.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Direct (non-transitive) includers of `F`. Mostly for tests and for
    /// the `len`-style diagnostics.
    pub fn direct_includers(&self, file: &Path) -> &[PathBuf] {
        self.dependents
            .get(file)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}
