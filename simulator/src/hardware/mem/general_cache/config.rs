use std::error::Error;
use std::fmt;

use super::prefetcher::PrefetcherKind;
use super::write_policy::WritePolicy;

const DEFAULT_MISS_PENALTY_CYCLES: usize = 5;

#[derive(Clone, Copy, Debug)]
pub(super) struct CacheGeometry {
    pub(super) block_size: usize,
    pub(super) num_of_way: usize,
    pub(super) num_sets: usize,
    pub(super) offset_bit_width: usize,
    pub(super) index_bit_width: usize,
}

impl CacheGeometry {
    fn new(
        total_size: usize,
        block_size: usize,
        num_of_way: usize,
    ) -> Result<Self, GeneralCacheConfigError> {
        if total_size == 0 {
            return Err(GeneralCacheConfigError::ZeroTotalSize);
        }
        if block_size == 0 {
            return Err(GeneralCacheConfigError::ZeroBlockSize);
        }
        if !block_size.is_power_of_two() {
            return Err(GeneralCacheConfigError::BlockSizeNotPowerOfTwo { block_size });
        }
        if num_of_way == 0 {
            return Err(GeneralCacheConfigError::ZeroAssociativity);
        }

        let set_capacity = block_size.checked_mul(num_of_way).ok_or(
            GeneralCacheConfigError::SetCapacityOverflow {
                block_size,
                num_of_way,
            },
        )?;
        if total_size % set_capacity != 0 {
            return Err(GeneralCacheConfigError::TotalSizeNotSetMultiple {
                total_size,
                block_size,
                num_of_way,
            });
        }

        let num_sets = total_size / set_capacity;
        if !num_sets.is_power_of_two() {
            return Err(GeneralCacheConfigError::NumSetsNotPowerOfTwo { num_sets });
        }

        Ok(Self {
            block_size,
            num_of_way,
            num_sets,
            offset_bit_width: block_size.ilog2() as usize,
            index_bit_width: num_sets.ilog2() as usize,
        })
    }
}

#[derive(Clone, Debug)]
pub struct GeneralCacheConfig {
    pub(super) name: String,
    pub(super) geometry: CacheGeometry,
    pub(super) miss_penalty: usize,
    pub(super) write_policy: WritePolicy,
    pub(super) prefetcher_kind: PrefetcherKind,
}

impl GeneralCacheConfig {
    pub fn new(
        name: impl Into<String>,
        total_size: usize,
        block_size: usize,
        num_of_way: usize,
    ) -> Result<Self, GeneralCacheConfigError> {
        Ok(Self {
            name: name.into(),
            geometry: CacheGeometry::new(total_size, block_size, num_of_way)?,
            miss_penalty: DEFAULT_MISS_PENALTY_CYCLES,
            write_policy: WritePolicy::default(),
            prefetcher_kind: PrefetcherKind::default(),
        })
    }

    /// Set the additional miss penalty in cycles after a refill resolves.
    pub fn with_miss_penalty(mut self, cycles: usize) -> Self {
        self.miss_penalty = cycles;
        self
    }

    /// Set the write policy. Defaults to write-back + write-allocate.
    pub fn with_write_policy(mut self, wp: WritePolicy) -> Self {
        self.write_policy = wp;
        self
    }

    /// Set the hardware prefetcher kind. Defaults to `Null`.
    pub fn with_prefetcher_kind(mut self, kind: PrefetcherKind) -> Self {
        self.prefetcher_kind = kind;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneralCacheConfigError {
    ZeroTotalSize,
    ZeroBlockSize,
    BlockSizeNotPowerOfTwo {
        block_size: usize,
    },
    ZeroAssociativity,
    SetCapacityOverflow {
        block_size: usize,
        num_of_way: usize,
    },
    TotalSizeNotSetMultiple {
        total_size: usize,
        block_size: usize,
        num_of_way: usize,
    },
    NumSetsNotPowerOfTwo {
        num_sets: usize,
    },
}

impl fmt::Display for GeneralCacheConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroTotalSize => write!(f, "total size must be greater than zero"),
            Self::ZeroBlockSize => write!(f, "block size must be greater than zero"),
            Self::BlockSizeNotPowerOfTwo { block_size } => {
                write!(f, "block size must be a power of two, got {block_size}")
            }
            Self::ZeroAssociativity => write!(f, "associativity must be greater than zero"),
            Self::SetCapacityOverflow {
                block_size,
                num_of_way,
            } => write!(
                f,
                "block size {block_size} and associativity {num_of_way} overflow set capacity"
            ),
            Self::TotalSizeNotSetMultiple {
                total_size,
                block_size,
                num_of_way,
            } => write!(
                f,
                "total size {total_size} must be a multiple of block size {block_size} x associativity {num_of_way}"
            ),
            Self::NumSetsNotPowerOfTwo { num_sets } => {
                write!(f, "number of sets must be a power of two, got {num_sets}")
            }
        }
    }
}

impl Error for GeneralCacheConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_power_of_two_geometry_for_address_slicing() {
        let config = GeneralCacheConfig::new("L1-D$", 4096, 32, 2).expect("valid geometry");

        assert_eq!(config.geometry.num_sets, 64);
        assert_eq!(config.geometry.offset_bit_width, 5);
        assert_eq!(config.geometry.index_bit_width, 6);
    }

    #[test]
    fn rejects_non_power_of_two_block_size() {
        let err = GeneralCacheConfig::new("L1-D$", 4096, 24, 2).expect_err("invalid geometry");

        assert_eq!(
            err,
            GeneralCacheConfigError::BlockSizeNotPowerOfTwo { block_size: 24 }
        );
    }
}
