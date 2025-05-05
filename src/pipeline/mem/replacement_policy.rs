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
            let evict_way = self.rng.random_range(0..self.valid.len()) as usize;
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

        fn promote(&mut self, index: usize) {
            self.queue.retain(|&x| x != index);
            self.queue.push_front(index);
        }

        fn evict(&mut self) -> (bool, usize) {
            for i in 0..self.valid.len() {
                if !self.valid[i] {
                    return (false, i);
                }
            }
            (true, self.queue.pop_back().unwrap())
        }
    }
}

pub mod pseudo_lru {
    use super::ReplacementPolicy;

    pub struct PseudoLruRP {
        //
    }

    impl PseudoLruRP {
        //
    }

    impl ReplacementPolicy for PseudoLruRP {
        fn new(num_associativity: usize) -> Self {
            todo!();
        }

        fn insert(&mut self, index: usize) {
            todo!();
        }

        fn invalidate(&mut self, index: usize) {
            todo!();
        }

        fn promote(&mut self, index: usize) {
            todo!();
        }

        fn evict(&mut self) -> (bool, usize) {
            todo!();
        }
    }
}
