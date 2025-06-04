#[derive(Default)]
pub struct StatisticInfo {
    total_ticked_cycle: usize,
    inst_fetched: usize,
    inst_retire: usize,
    branch_inst_cnt: usize,
    branch_miss_cnt: usize,
    total_flush_cnt: usize,
    actual_flushed_inst_cnt: usize,
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

impl std::fmt::Display for StatisticInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "")?;
        writeln!(f, "=============================================")?;
        writeln!(f, "Statistics Information about PipelineProcessor")?;
        writeln!(f, "Total ticked cycle count: {}", self.total_ticked_cycle)?;
        writeln!(f, "Total instruction fetched: {}", self.inst_fetched)?;
        writeln!(f, "Total instruction retired: {}", self.inst_retire)?;
        writeln!(
            f,
            "Total control-flow instruction count: {}",
            self.branch_inst_cnt
        )?;
        writeln!(
            f,
            "Total branch miss rate: {:.2}%",
            (self.branch_miss_cnt as f64) / (self.branch_inst_cnt as f64)
        )?;
        writeln!(f, "Total flush count: {}", self.total_flush_cnt)?;
        writeln!(
            f,
            "Actual flushed instruction count: {}",
            self.actual_flushed_inst_cnt
        )?;
        writeln!(f, "=============================================")?;
        writeln!(f, "")?;
        Ok(())
    }
}
