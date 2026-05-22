use std::cell::RefCell;
use std::rc::Rc;

use crate::hardware::branch_predictor::{self, BranchPredict};
use crate::hardware::clock::Clocked;
use crate::hardware::mem::abstract_mem::AbstractMemoryInterface;
use crate::hardware::mem::general_cache::prefetcher::PrefetcherKind;
use crate::hardware::mem::general_cache::replacement_policy::{self as rp, ReplacementPolicy};
use crate::hardware::mem::general_cache::statistic::StatisticInfo as CacheStatisticInfo;
use crate::hardware::mem::general_cache::write_policy::WritePolicy;
use crate::hardware::mem::general_cache::GeneralCacheConfig;
use crate::hardware::mem::simple_dram::{DramTiming, SimpleDram};
use crate::hardware::mem::simple_mem::SimpleMem;
use crate::hardware::pipeline_processor::pipe::PipelineProcessor;
use crate::hardware::pipeline_processor::statistic::StatisticInfo as PipelineStatisticInfo;
use crate::hardware::statistic::Statistic;
use crate::sim::elf::ProgramInfo;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplacementPolicyKind {
    Fifo,
    Random,
    Plru,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchPredictorKind {
    Dummy,
    Bimodal,
}

impl BranchPredictorKind {
    fn build(self) -> Box<dyn BranchPredict> {
        match self {
            Self::Dummy => Box::new(branch_predictor::dummy::Predictor::new()),
            Self::Bimodal => Box::new(branch_predictor::bimodal::Predictor::new()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BackingMemoryKind {
    SimpleMem,
    SimpleDram { timing: DramTiming },
}

#[derive(Clone, Copy, Debug)]
pub struct CacheLevelConfig {
    pub total_size: usize,
    pub block_size: usize,
    pub num_of_way: usize,
    pub miss_penalty: usize,
}

impl CacheLevelConfig {
    pub const fn new(
        total_size: usize,
        block_size: usize,
        num_of_way: usize,
        miss_penalty: usize,
    ) -> Self {
        Self {
            total_size,
            block_size,
            num_of_way,
            miss_penalty,
        }
    }

    fn into_general_cache_config(
        self,
        name: &str,
        write_policy: WritePolicy,
        prefetcher_kind: PrefetcherKind,
    ) -> GeneralCacheConfig {
        GeneralCacheConfig::new(name.to_string())
            .with_total_size(self.total_size)
            .with_block_size(self.block_size)
            .with_num_of_way(self.num_of_way)
            .with_miss_penalty(self.miss_penalty)
            .with_write_policy(write_policy)
            .with_prefetcher_kind(prefetcher_kind)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SimulationConfig {
    pub l1i: CacheLevelConfig,
    pub l1d: CacheLevelConfig,
    pub l2: CacheLevelConfig,
    pub replacement_policy: ReplacementPolicyKind,
    pub write_policy: WritePolicy,
    pub prefetcher_kind: PrefetcherKind,
    pub backing_memory: BackingMemoryKind,
    pub branch_predictor: BranchPredictorKind,
}

impl SimulationConfig {
    fn cache_configs(self) -> (GeneralCacheConfig, GeneralCacheConfig, GeneralCacheConfig) {
        (
            self.l1i
                .into_general_cache_config("L1-I$", self.write_policy, self.prefetcher_kind),
            self.l1d
                .into_general_cache_config("L1-D$", self.write_policy, self.prefetcher_kind),
            self.l2
                .into_general_cache_config("L2$", self.write_policy, self.prefetcher_kind),
        )
    }
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            l1i: CacheLevelConfig::new(2048, 32, 4, 2),
            l1d: CacheLevelConfig::new(256, 32, 2, 2),
            l2: CacheLevelConfig::new(16384, 64, 4, 10),
            replacement_policy: ReplacementPolicyKind::Fifo,
            write_policy: WritePolicy::default(),
            prefetcher_kind: PrefetcherKind::default(),
            backing_memory: BackingMemoryKind::SimpleMem,
            branch_predictor: BranchPredictorKind::Bimodal,
        }
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackingMemoryReport {
    SimpleMem,
    SimpleDram {
        row_buffer_hit_cnt: usize,
        row_buffer_miss_cnt: usize,
    },
}

#[derive(Clone, serde::Serialize)]
pub struct SimulationReport {
    pub pipeline: PipelineStatisticInfo,
    pub l1i: CacheStatisticInfo,
    pub l1d: CacheStatisticInfo,
    pub l2: CacheStatisticInfo,
    pub backing_memory: BackingMemoryReport,
    pub final_registers: [u32; 32],
}

pub fn run(program: ProgramInfo, config: SimulationConfig) -> SimulationReport {
    let ProgramInfo {
        entry_pc,
        prog_body,
    } = program;

    match config.replacement_policy {
        ReplacementPolicyKind::Fifo => {
            run_with_policy::<rp::fifo::FifoRP>(entry_pc, prog_body, config)
        }
        ReplacementPolicyKind::Random => {
            run_with_policy::<rp::random::RandomRP>(entry_pc, prog_body, config)
        }
        ReplacementPolicyKind::Plru => {
            run_with_policy::<rp::pseudo_lru::PseudoLruRP>(entry_pc, prog_body, config)
        }
    }
}

fn run_with_policy<RP>(
    entry_pc: u32,
    prog_body: Vec<u8>,
    config: SimulationConfig,
) -> SimulationReport
where
    RP: ReplacementPolicy,
{
    match config.backing_memory {
        BackingMemoryKind::SimpleMem => {
            run_with_memory::<RP, _, _>(entry_pc, SimpleMem::new(prog_body), config, |_| {
                BackingMemoryReport::SimpleMem
            })
        }
        BackingMemoryKind::SimpleDram { timing } => run_with_memory::<RP, _, _>(
            entry_pc,
            SimpleDram::new(prog_body, timing),
            config,
            |dram| BackingMemoryReport::SimpleDram {
                row_buffer_hit_cnt: dram.row_buffer_hit_cnt,
                row_buffer_miss_cnt: dram.row_buffer_miss_cnt,
            },
        ),
    }
}

fn run_with_memory<RP, M, F>(
    entry_pc: u32,
    memory: M,
    config: SimulationConfig,
    memory_report: F,
) -> SimulationReport
where
    RP: ReplacementPolicy,
    M: AbstractMemoryInterface,
    F: FnOnce(&M) -> BackingMemoryReport,
{
    let mem = Rc::new(RefCell::new(memory));
    let (l1i_cfg, l1d_cfg, l2_cfg) = config.cache_configs();
    let mut cpu = PipelineProcessor::<RP, M>::new_with_predictor(
        entry_pc,
        l1i_cfg,
        l1d_cfg,
        l2_cfg,
        &mem,
        config.branch_predictor.build(),
    );

    while !cpu.halt {
        cpu.tick();
        mem.borrow_mut().tick();
    }

    let backing_memory = {
        let mem = mem.borrow();
        memory_report(&mem)
    };
    let l2 = cpu.l2_cache.borrow().get_statistic_info();

    SimulationReport {
        pipeline: cpu.get_statistic_info(),
        l1i: cpu.icache.get_statistic_info(),
        l1d: cpu.dcache.get_statistic_info(),
        l2,
        backing_memory,
        final_registers: cpu.registers(),
    }
}
