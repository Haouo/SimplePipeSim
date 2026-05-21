/// # trait ReplacementPolicy for Cache Eviction
///
/// This trait defines the common interface for implementing cache replacement policy.
/// The granularity of replacement policy of a cache is "a single cache set".
/// Hence, the replacement policies should be encapsulated with the cache set unit in cache structures.
pub trait ReplacementPolicy {
    /// constructor
    fn new(num_associativity: usize) -> Self;

    /// insert a new way in the current cache set, which marks the way as valid (true)
    fn insert(&mut self, index: usize);
    /// Invalidate specified way in the current cache set, which marks the way as invalid (false).
    /// Invalidate operation is often used in multiprocessor system (for invalidate-based cache coherence implementation)
    fn invalidate(&mut self, index: usize);
    /// accessing to a way will promote it in a cache set, which is equaling to marking this as recently-accessed
    fn promote(&mut self, index: usize);
    /// replace function will tell that which way should be evicted (replaced) from the current cache set
    fn evict(&mut self) -> (bool, usize);
}

pub mod random {
    use super::ReplacementPolicy;
    use rand::{self, Rng};

    pub struct RandomRP {
        valid: Box<[bool]>, // valid bits for each way
        rng: rand::rngs::ThreadRng,
    }

    impl ReplacementPolicy for RandomRP {
        fn new(num_associativity: usize) -> Self {
            Self {
                valid: vec![false; num_associativity].into_boxed_slice(),
                rng: rand::rng(),
            }
        }

        fn insert(&mut self, index: usize) {
            self.valid[index] = true;
        }

        fn invalidate(&mut self, index: usize) {
            self.valid[index] = false;
        }

        #[allow(unused)]
        fn promote(&mut self, index: usize) {}

        fn evict(&mut self) -> (bool, usize) {
            // find invalid way
            for i in 0..self.valid.len() {
                if !self.valid[i] {
                    return (false, i);
                }
            }
            // generate a ranged usize random number
            let evict_way = self.rng.random_range(0..self.valid.len());
            self.valid[evict_way] = false;
            (true, evict_way)
        }
    }
}

pub mod fifo {
    use super::ReplacementPolicy;
    use std::collections::VecDeque;

    pub struct FifoRP {
        valid: Box<[bool]>,
        queue: VecDeque<usize>,
    }

    impl ReplacementPolicy for FifoRP {
        fn new(num_associativity: usize) -> Self {
            Self {
                valid: vec![false; num_associativity].into_boxed_slice(),
                queue: VecDeque::with_capacity(num_associativity),
            }
        }

        fn insert(&mut self, index: usize) {
            self.valid[index] = true;
            self.queue.push_front(index);
        }

        fn invalidate(&mut self, index: usize) {
            self.valid[index] = false;
            self.queue.retain(|&x| x != index);
        }

        #[allow(unused)]
        fn promote(&mut self, index: usize) {}

        fn evict(&mut self) -> (bool, usize) {
            for i in 0..self.valid.len() {
                if self.valid[i] == false {
                    return (false, i);
                }
            }
            let evict_way_index = self.queue.pop_back().unwrap();
            self.valid[evict_way_index] = false;
            (true, evict_way_index)
        }
    }
}

/// Tree-based pseudo-LRU (PLRU).
///
/// For an N-way set (with N a power of 2), we maintain an N-1 bit binary
/// tree laid out in heap-array form. The N leaves are the ways and the
/// N-1 internal nodes each hold one bit:
///
/// - `tree[i] == false` ⇒ left subtree was the most-recently accessed,
///                        so eviction should go RIGHT
/// - `tree[i] == true`  ⇒ right subtree was the most-recently accessed,
///                        so eviction should go LEFT
///
/// Heap indexing: node 0 is the root, node i's children are 2i+1 and
/// 2i+2. Internal nodes occupy indices `0 .. N-1`; leaves occupy
/// `N-1 .. 2N-1`. Way `w` is the leaf at heap index `w + (N - 1)`.
///
/// On `promote(w)`, we walk from leaf to root and flip each bit along
/// the path to point AWAY from `w`. On `evict()`, we walk root → leaf
/// following each bit (and so going to the older subtree).
pub mod pseudo_lru {
    use super::ReplacementPolicy;

    pub struct PseudoLruRP {
        num_ways: usize,
        valid: Box<[bool]>,
        /// `N - 1` direction bits, one per internal node, in heap layout.
        /// Empty when `num_ways == 1`.
        tree: Box<[bool]>,
    }

    impl PseudoLruRP {
        #[inline]
        fn leaf_index(&self, way: usize) -> usize {
            way + (self.num_ways - 1)
        }
    }

    impl ReplacementPolicy for PseudoLruRP {
        fn new(num_associativity: usize) -> Self {
            assert!(
                num_associativity.is_power_of_two(),
                "PseudoLruRP requires num_associativity to be a power of 2 (got {})",
                num_associativity
            );
            let num_internal = num_associativity.saturating_sub(1);
            Self {
                num_ways: num_associativity,
                valid: vec![false; num_associativity].into_boxed_slice(),
                tree: vec![false; num_internal].into_boxed_slice(),
            }
        }

        fn insert(&mut self, index: usize) {
            self.valid[index] = true;
            // Treat insert as a most-recent access.
            self.promote(index);
        }

        fn invalidate(&mut self, index: usize) {
            self.valid[index] = false;
        }

        fn promote(&mut self, accessed: usize) {
            if self.num_ways == 1 {
                return;
            }
            let mut node = self.leaf_index(accessed);
            while node > 0 {
                let parent = (node - 1) / 2;
                // If `node` is the LEFT child of `parent`, then left
                // subtree was just accessed ⇒ tree[parent] = false
                // ("evict goes right" — away from us).
                let is_left_child = node == 2 * parent + 1;
                self.tree[parent] = !is_left_child;
                node = parent;
            }
        }

        fn evict(&mut self) -> (bool, usize) {
            // Prefer invalid ways: scan once.
            for i in 0..self.num_ways {
                if !self.valid[i] {
                    return (false, i);
                }
            }
            // All valid: walk root → leaf following the PLRU bits.
            let mut node = 0usize;
            while node < self.num_ways - 1 {
                node = if self.tree[node] {
                    // right subtree is newer ⇒ go left
                    2 * node + 1
                } else {
                    // left subtree is newer ⇒ go right
                    2 * node + 2
                };
            }
            let way = node - (self.num_ways - 1);
            self.valid[way] = false;
            (true, way)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::ReplacementPolicy;
        use super::PseudoLruRP;

        #[test]
        #[should_panic]
        fn rejects_non_power_of_two() {
            let _ = PseudoLruRP::new(3);
        }

        #[test]
        fn one_way_always_evicts_zero() {
            let mut p = PseudoLruRP::new(1);
            p.insert(0);
            let (full, way) = p.evict();
            assert_eq!((full, way), (true, 0));
        }

        #[test]
        fn invalid_ways_picked_first() {
            let mut p = PseudoLruRP::new(4);
            p.insert(2);
            let (full, way) = p.evict();
            assert!(!full);
            assert_ne!(way, 2);
        }

        #[test]
        fn evicts_lru_when_full_two_way() {
            let mut p = PseudoLruRP::new(2);
            p.insert(0);
            p.insert(1);
            // Most recent insert was way 1 ⇒ way 0 should be evicted.
            let (full, way) = p.evict();
            assert!(full);
            assert_eq!(way, 0);
        }

        #[test]
        fn evicts_lru_when_full_four_way() {
            let mut p = PseudoLruRP::new(4);
            p.insert(0);
            p.insert(1);
            p.insert(2);
            p.insert(3);
            p.promote(0);
            // Ways 0 (just promoted) and 3 (last inserted in its subtree)
            // are the recent ones; LRU should pick from {1, 2}.
            let (full, way) = p.evict();
            assert!(full);
            assert!(way == 1 || way == 2, "expected 1 or 2, got {}", way);
        }

        #[test]
        fn evicts_lru_when_full_eight_way() {
            let mut p = PseudoLruRP::new(8);
            for w in 0..8 {
                p.insert(w);
            }
            p.promote(0);
            // Way 0 is MRU now; way 7 was MRU before. PLRU must avoid
            // both.
            let (full, way) = p.evict();
            assert!(full);
            assert_ne!(way, 0);
            assert_ne!(way, 7);
        }
    }
}
