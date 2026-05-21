//! DRAM timing constants, expressed in core clock cycles.
//!
//! Real DRAM chips publish timings in nanoseconds (e.g. tRCD = 13.75 ns
//! for DDR4-2400). Whoever instantiates [`SimpleDram`] is responsible
//! for converting those numbers into the appropriate cycle count for
//! the simulated core clock frequency.

/// Per-bank DRAM timing parameters. All values are in *core clock
/// cycles* and must each be at least 1 — a value of zero would mean the
/// state transition completes synchronously with command issue, which
/// the [`crate::hardware::mem::simple_dram::bank::Bank`] state machine
/// does not model.
#[derive(Debug, Clone, Copy)]
pub struct DramTiming {
    /// tRCD — Row to Column Delay. After an `ACT` is issued, the bank
    /// must spend this many cycles latching the row into the sense
    /// amplifiers before a `READ` or `WRITE` can target it.
    pub t_rcd: usize,
    /// tCL — CAS Latency. After a `READ`/`WRITE` is issued, the bank
    /// drives (or accepts) data this many cycles later.
    pub t_cl: usize,
    /// tRP — Row Precharge time. After a `PRE` is issued, the bank
    /// must spend this many cycles closing the open row before it can
    /// accept a new `ACT`.
    pub t_rp: usize,
}

impl DramTiming {
    /// Sanity-check that every value is non-zero. A timing of 0 would
    /// produce undefined behaviour in the bank FSM, which expects a
    /// strictly positive cycle count for every state with a countdown.
    pub fn assert_valid(&self) {
        assert!(self.t_rcd >= 1, "t_rcd must be >= 1");
        assert!(self.t_cl >= 1, "t_cl must be >= 1");
        assert!(self.t_rp >= 1, "t_rp must be >= 1");
    }

    /// Small, equal timings chosen so cycle traces stay readable when
    /// printed by hand. This is the default used by tests; it is not
    /// representative of any real DDR generation.
    pub fn educational_default() -> Self {
        Self {
            t_rcd: 4,
            t_cl: 4,
            t_rp: 4,
        }
    }

    /// Approximate DDR4-2400 timings rounded to 17 cycles, which is
    /// the ballpark JEDEC value of 13.75 ns at a 1 GHz core clock.
    /// Provided as a realistic preset for sweep experiments.
    pub fn ddr4_2400() -> Self {
        Self {
            t_rcd: 17,
            t_cl: 17,
            t_rp: 17,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn educational_default_passes_validity_check() {
        DramTiming::educational_default().assert_valid();
    }

    #[test]
    fn ddr4_2400_passes_validity_check() {
        DramTiming::ddr4_2400().assert_valid();
    }

    #[test]
    #[should_panic(expected = "t_rcd must be >= 1")]
    fn zero_t_rcd_is_rejected() {
        DramTiming {
            t_rcd: 0,
            t_cl: 4,
            t_rp: 4,
        }
        .assert_valid();
    }
}
