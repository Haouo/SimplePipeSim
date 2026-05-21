//! Cache write policy configuration.
//!
//! Two orthogonal axes:
//!
//! - **write-back vs write-through**: on a store HIT, does the cache hold
//!   the new data privately and write it to the next level only when the
//!   block is evicted (write-back, sets a dirty bit), or does it
//!   simultaneously update the cache copy and the next level so the two
//!   always agree (write-through, no dirty bit needed)?
//!
//! - **write-allocate vs no-write-allocate**: on a store MISS, does the
//!   cache fetch the block (write-allocate, after which the store hits
//!   in cache), or does it bypass the cache and send the store straight
//!   to the next level (no-write-allocate)?
//!
//! All four combinations are valid in real designs:
//!
//! | write policy | hit behaviour         | miss behaviour                       |
//! |--------------|-----------------------|--------------------------------------|
//! | WB + WA      | cache + dirty bit     | refill, then store hits in cache     |
//! | WB + NWA     | cache + dirty bit     | bypass cache, store goes to next lvl |
//! | WT + WA      | cache + next-level    | refill, then store hits in cache and propagates |
//! | WT + NWA    | cache + next-level    | bypass cache, store goes to next lvl |

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize)]
pub enum WritePolicy {
    /// Write-back + write-allocate. The historical and default behaviour.
    #[default]
    WriteBackWriteAllocate,
    /// Write-back, but skip cache allocation on store miss.
    WriteBackNoWriteAllocate,
    /// Write-through: every store hit also goes to the next level. Plus
    /// write-allocate on miss (fetch then write).
    WriteThroughWriteAllocate,
    /// Write-through + no-write-allocate. Stores never allocate.
    WriteThroughNoWriteAllocate,
}

impl WritePolicy {
    pub fn is_write_back(self) -> bool {
        matches!(
            self,
            WritePolicy::WriteBackWriteAllocate | WritePolicy::WriteBackNoWriteAllocate
        )
    }

    pub fn is_write_through(self) -> bool {
        !self.is_write_back()
    }

    pub fn is_write_allocate(self) -> bool {
        matches!(
            self,
            WritePolicy::WriteBackWriteAllocate | WritePolicy::WriteThroughWriteAllocate
        )
    }

    pub fn is_no_write_allocate(self) -> bool {
        !self.is_write_allocate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_write_back_write_allocate() {
        let wp = WritePolicy::default();
        assert_eq!(wp, WritePolicy::WriteBackWriteAllocate);
        assert!(wp.is_write_back());
        assert!(wp.is_write_allocate());
    }

    #[test]
    fn axes_are_orthogonal() {
        for wp in [
            WritePolicy::WriteBackWriteAllocate,
            WritePolicy::WriteBackNoWriteAllocate,
            WritePolicy::WriteThroughWriteAllocate,
            WritePolicy::WriteThroughNoWriteAllocate,
        ] {
            assert_ne!(wp.is_write_back(), wp.is_write_through());
            assert_ne!(wp.is_write_allocate(), wp.is_no_write_allocate());
        }
    }
}
