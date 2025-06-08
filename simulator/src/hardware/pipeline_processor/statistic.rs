#[derive(Clone, Default)]
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
}
