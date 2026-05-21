//! Pluggable cache prefetcher.
//!
//! The cache calls `Prefetcher::predict` after each demand-access lookup
//! and queues the returned block addresses as synthetic loads. Those
//! synthetic loads pass through the same FSM as demand misses, so they
//! benefit from existing write-back, allocate, and miss-penalty
//! machinery without special-casing.

use std::fmt::Debug;

/// A prefetcher inspects each demand-access (`addr`, hit/miss, load/store)
/// and returns a list of block-aligned addresses it wants the cache to
/// fetch ahead of time. Empty Vec means "no prefetch this cycle".
pub trait Prefetcher: Debug {
    fn predict(&mut self, addr: u32, is_hit: bool, is_store: bool) -> Vec<u32>;
}

/// Disabled. Always returns no predictions.
#[derive(Debug, Default)]
pub struct NullPrefetcher;

impl Prefetcher for NullPrefetcher {
    fn predict(&mut self, _addr: u32, _is_hit: bool, _is_store: bool) -> Vec<u32> {
        Vec::new()
    }
}

/// Stride-1 next-line prefetcher: always prefetches `addr + block_size`.
///
/// Effective for sequential access patterns (e.g. row-major matmul).
#[derive(Debug)]
pub struct NextLinePrefetcher {
    pub block_size: usize,
}

impl Prefetcher for NextLinePrefetcher {
    fn predict(&mut self, addr: u32, _is_hit: bool, _is_store: bool) -> Vec<u32> {
        let block_size = self.block_size as u32;
        let block_aligned = addr & !(block_size - 1);
        // Saturate at u32::MAX to avoid overflow on the very last block.
        let next = block_aligned.saturating_add(block_size);
        vec![next]
    }
}

/// Configuration-time selector. Maps to a concrete `Box<dyn Prefetcher>`
/// via [`PrefetcherKind::build`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize)]
pub enum PrefetcherKind {
    #[default]
    Null,
    NextLine,
}

impl PrefetcherKind {
    pub fn build(self, block_size: usize) -> Box<dyn Prefetcher> {
        match self {
            PrefetcherKind::Null => Box::new(NullPrefetcher),
            PrefetcherKind::NextLine => Box::new(NextLinePrefetcher { block_size }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_returns_no_predictions() {
        let mut p = NullPrefetcher;
        assert_eq!(p.predict(0x100, true, false), Vec::<u32>::new());
        assert_eq!(p.predict(0x100, false, true), Vec::<u32>::new());
    }

    #[test]
    fn next_line_predicts_next_block_aligned_address() {
        let mut p = NextLinePrefetcher { block_size: 32 };
        // 0x40 is already block-aligned ⇒ predict 0x60
        assert_eq!(p.predict(0x40, false, false), vec![0x60]);
        // 0x55 lives in block 0x40..0x60 ⇒ predict 0x60 too
        assert_eq!(p.predict(0x55, true, false), vec![0x60]);
    }

    #[test]
    fn next_line_handles_high_address_without_panic() {
        let mut p = NextLinePrefetcher { block_size: 32 };
        // Near top of u32: saturating_add prevents overflow panic.
        let predictions = p.predict(0xFFFF_FFE0, false, false);
        assert_eq!(predictions.len(), 1);
    }

    #[test]
    fn kind_builds_appropriate_prefetcher() {
        let mut null_b = PrefetcherKind::Null.build(32);
        assert_eq!(null_b.predict(0x10, false, false), Vec::<u32>::new());

        let mut nl_b = PrefetcherKind::NextLine.build(32);
        assert_eq!(nl_b.predict(0x10, false, false), vec![0x20]);
    }
}
