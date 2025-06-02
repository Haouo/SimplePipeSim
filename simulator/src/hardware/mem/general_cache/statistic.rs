use std::fmt::Display;

#[derive(Default)]
pub struct StatisticInfo {
    name: String,
    load_cnt: usize,
    store_cnt: usize,
    load_miss_cnt: usize,
    store_miss_cnt: usize,
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

impl Display for StatisticInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "")?;
        writeln!(f, "=============================================")?;
        writeln!(f, "Statistics Information about {}", self.name)?;
        writeln!(f, "Total load count: {}", self.load_cnt)?;
        writeln!(
            f,
            "Load miss rate: {:.2}%",
            (self.load_miss_cnt as f32 / self.load_cnt as f32) * 100f32
        )?;
        writeln!(f, "Total store count: {}", self.store_cnt)?;
        writeln!(
            f,
            "Store miss rate: {:.2}%",
            (self.store_miss_cnt as f32 / self.store_cnt as f32) * 100f32
        )?;
        writeln!(
            f,
            "Overall miss rate: {:.2}%",
            (self.load_miss_cnt + self.store_miss_cnt) as f32
                / (self.load_cnt + self.store_cnt) as f32
                * 100f32
        )?;
        writeln!(f, "=============================================")?;
        writeln!(f, "")?;
        Ok(())
    }
}
