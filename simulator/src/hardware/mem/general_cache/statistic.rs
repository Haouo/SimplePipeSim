use std::fmt::Display;

#[derive(Default)]
pub struct StatisticInfo {
    load_cnt: usize,
    store_cnt: usize,
    load_miss_cnt: usize,
    store_miss_cnt: usize,
}

impl StatisticInfo {
    /// constructor
    pub fn new() -> Self {
        Self::default()
    }

    // @TODO
}

impl Display for StatisticInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!();
    }
}
