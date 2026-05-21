#[derive(Clone, Default, serde::Serialize)]
pub struct StatisticInfo {
    // be recorded during runtime
    pub name: String,
    pub load_cnt: usize,
    pub store_cnt: usize,
    pub load_miss_cnt: usize,
    pub store_miss_cnt: usize,
    /// Number of synthetic prefetch lookups issued by this cache. A
    /// hardware-level prefetcher hint that materialised as a real
    /// next-level memory access.
    pub prefetch_issued_cnt: usize,
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

    /// Compute load / store / overall miss rates from the accumulated counts.
    ///
    /// `load_miss_rate`  = load_miss_cnt  / load_cnt
    /// `store_miss_rate` = store_miss_cnt / store_cnt
    /// `overall_miss_rate` = (load_miss_cnt + store_miss_cnt) / (load_cnt + store_cnt)
    ///
    /// A 0/0 ratio is reported as 0.0 (no traffic of that kind ⇒ no miss).
    pub fn finalize_rates(&mut self) {
        let safe_ratio = |num: usize, den: usize| -> f64 {
            if den == 0 {
                0.0
            } else {
                num as f64 / den as f64
            }
        };
        self.load_miss_rate = safe_ratio(self.load_miss_cnt, self.load_cnt);
        self.store_miss_rate = safe_ratio(self.store_miss_cnt, self.store_cnt);
        self.overall_miss_rate = safe_ratio(
            self.load_miss_cnt + self.store_miss_cnt,
            self.load_cnt + self.store_cnt,
        );
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
        let mut s = StatisticInfo::new("test".to_string());
        s.load_cnt = 10;
        s.load_miss_cnt = 2;
        s.store_cnt = 4;
        s.store_miss_cnt = 1;
        s.finalize_rates();
        assert!(approx_eq(s.load_miss_rate, 0.2));
        assert!(approx_eq(s.store_miss_rate, 0.25));
        assert!(approx_eq(s.overall_miss_rate, 3.0 / 14.0));
    }

    #[test]
    fn finalize_rates_zero_traffic_returns_zero() {
        let mut s = StatisticInfo::new("test".to_string());
        s.finalize_rates();
        assert_eq!(s.load_miss_rate, 0.0);
        assert_eq!(s.store_miss_rate, 0.0);
        assert_eq!(s.overall_miss_rate, 0.0);
    }

    #[test]
    fn finalize_rates_loads_only() {
        let mut s = StatisticInfo::new("test".to_string());
        s.load_cnt = 100;
        s.load_miss_cnt = 25;
        s.finalize_rates();
        assert!(approx_eq(s.load_miss_rate, 0.25));
        assert_eq!(s.store_miss_rate, 0.0);
        assert!(approx_eq(s.overall_miss_rate, 0.25));
    }

    #[test]
    fn finalize_rates_all_miss() {
        let mut s = StatisticInfo::new("test".to_string());
        s.load_cnt = 5;
        s.load_miss_cnt = 5;
        s.store_cnt = 3;
        s.store_miss_cnt = 3;
        s.finalize_rates();
        assert!(approx_eq(s.load_miss_rate, 1.0));
        assert!(approx_eq(s.store_miss_rate, 1.0));
        assert!(approx_eq(s.overall_miss_rate, 1.0));
    }
}
