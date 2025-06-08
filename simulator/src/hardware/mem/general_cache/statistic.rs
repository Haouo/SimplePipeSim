#[derive(Clone, Default)]
pub struct StatisticInfo {
    // be recorded during runtime
    pub name: String,
    pub load_cnt: usize,
    pub store_cnt: usize,
    pub load_miss_cnt: usize,
    pub store_miss_cnt: usize,
    // be calculated after ending of simulation
    pub load_miss_rate: f64,
    pub store_miss_rate: f64,
    pub overall_miss_rate: f64,
}

impl StatisticInfo {
    /// constructor
    pub fn new(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }

    pub fn load(&mut self, is_miss: bool) {
        self.load_cnt += 1;
        if is_miss {
            self.load_miss_cnt += 1;
        }
    }

    pub fn store(&mut self, is_miss: bool) {
        self.store_cnt += 1;
        if is_miss {
            self.store_miss_cnt += 1;
        }
    }
}
