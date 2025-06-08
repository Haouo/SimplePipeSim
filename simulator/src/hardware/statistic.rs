pub trait Statistic {
    type StatisticInfo;
    fn get_statistic_info(&self) -> Self::StatisticInfo;
}
