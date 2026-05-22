#[derive(Clone, Default, serde::Serialize)]
pub struct StatisticInfo {
    // be recorded during runtime
    pub total_ticked_cycle: usize,
    pub inst_fetched: usize,
    pub inst_retire: usize,
    pub branch_inst_cnt: usize,
    pub branch_miss_cnt: usize,
    pub total_flush_cnt: usize,
    pub actual_flushed_inst_cnt: usize,
    // be calculated after ending of simulation
    pub ipc: f64,
    pub branch_miss_rate: f64,
}

impl StatisticInfo {
    pub fn tick(&mut self) {
        self.total_ticked_cycle += 1;
    }

    pub fn inst_fetch(&mut self) {
        self.inst_fetched += 1;
    }

    pub fn inst_ret(&mut self) {
        self.inst_retire += 1;
    }

    pub fn solve_branch(&mut self, is_miss: bool) {
        self.branch_inst_cnt += 1;
        if is_miss {
            self.branch_miss_cnt += 1;
        }
    }

    pub fn inst_flush(&mut self, valid: bool) {
        self.total_flush_cnt += 1;
        if valid {
            self.actual_flushed_inst_cnt += 1;
        }
    }

    pub fn finalize_rates(&mut self) {
        let safe_ratio = |num: usize, den: usize| -> f64 {
            if den == 0 {
                0.0
            } else {
                num as f64 / den as f64
            }
        };
        self.ipc = safe_ratio(self.inst_retire, self.total_ticked_cycle);
        self.branch_miss_rate = safe_ratio(self.branch_miss_cnt, self.branch_inst_cnt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn finalize_rates_basic() {
        let mut s = StatisticInfo::default();
        s.total_ticked_cycle = 40;
        s.inst_retire = 25;
        s.branch_inst_cnt = 10;
        s.branch_miss_cnt = 2;
        s.finalize_rates();
        assert!(approx_eq(s.ipc, 0.625));
        assert!(approx_eq(s.branch_miss_rate, 0.2));
    }

    #[test]
    fn finalize_rates_zero_denominators_return_zero() {
        let mut s = StatisticInfo::default();
        s.finalize_rates();
        assert_eq!(s.ipc, 0.0);
        assert_eq!(s.branch_miss_rate, 0.0);
    }
}
