mod hardware;
mod riscv;
mod sim;

use clap::Parser;

use hardware::clock::Clocked;
use hardware::mem::general_cache::replacement_policy::{self as rp, ReplacementPolicy};
use hardware::mem::general_cache::statistic::StatisticInfo as CacheStatisticInfo;
use hardware::mem::general_cache::GeneralCacheConfig;
use hardware::mem::simple_mem::SimpleMem;
use hardware::pipeline_processor::pipe::PipelineProcessor;
use hardware::pipeline_processor::statistic::StatisticInfo as PipelineStatisticInfo;
use hardware::statistic::Statistic;
use sim::cli::{Args, ReplacementPolicyArg};
use sim::elf;

use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

#[derive(serde::Serialize)]
struct RunReport<'a> {
    pipeline: PipelineStatisticInfo,
    l1i: CacheStatisticInfo,
    l1d: CacheStatisticInfo,
    l2: CacheStatisticInfo,
    config: &'a Args,
}

fn build_configs(args: &Args) -> (GeneralCacheConfig, GeneralCacheConfig, GeneralCacheConfig) {
    let wp = args.wp.into();
    let pf = args.prefetcher.into();
    let l1i = GeneralCacheConfig::new("L1-I$".to_string())
        .with_total_size(args.l1i_size)
        .with_block_size(args.l1i_block)
        .with_num_of_way(args.l1i_ways)
        .with_miss_penalty(args.l1i_penalty)
        .with_write_policy(wp)
        .with_prefetcher_kind(pf);
    let l1d = GeneralCacheConfig::new("L1-D$".to_string())
        .with_total_size(args.l1d_size)
        .with_block_size(args.l1d_block)
        .with_num_of_way(args.l1d_ways)
        .with_miss_penalty(args.l1d_penalty)
        .with_write_policy(wp)
        .with_prefetcher_kind(pf);
    let l2 = GeneralCacheConfig::new("L2$".to_string())
        .with_total_size(args.l2_size)
        .with_block_size(args.l2_block)
        .with_num_of_way(args.l2_ways)
        .with_miss_penalty(args.l2_penalty)
        .with_write_policy(wp)
        .with_prefetcher_kind(pf);
    (l1i, l1d, l2)
}

fn run_simulation<RP: ReplacementPolicy>(
    entry_pc: u32,
    prog_body: Vec<u8>,
    l1i_cfg: GeneralCacheConfig,
    l1d_cfg: GeneralCacheConfig,
    l2_cfg: GeneralCacheConfig,
) -> (
    PipelineStatisticInfo,
    CacheStatisticInfo,
    CacheStatisticInfo,
    CacheStatisticInfo,
) {
    let mem = Rc::new(RefCell::new(SimpleMem::new(prog_body)));
    let mut cpu =
        PipelineProcessor::<RP>::new(entry_pc, l1i_cfg, l1d_cfg, l2_cfg, &mem);
    while !cpu.halt {
        cpu.tick();
        mem.borrow_mut().tick();
    }
    let pipeline_stats = cpu.get_statistic_info();
    let l1i_stats = cpu.icache.get_statistic_info();
    let l1d_stats = cpu.dcache.get_statistic_info();
    let l2_stats = cpu.l2_cache.borrow().get_statistic_info();
    (pipeline_stats, l1i_stats, l1d_stats, l2_stats)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let elf_path = args.elf_dir.join(&args.prog);
    let elf::ProgramInfo {
        entry_pc,
        prog_body,
    } = elf::elf_loader(&elf_path);

    let (l1i_cfg, l1d_cfg, l2_cfg) = build_configs(&args);

    let (pipeline_stats, l1i_stats, l1d_stats, l2_stats) = match args.rp {
        ReplacementPolicyArg::Fifo => {
            run_simulation::<rp::fifo::FifoRP>(entry_pc, prog_body, l1i_cfg, l1d_cfg, l2_cfg)
        }
        ReplacementPolicyArg::Random => {
            run_simulation::<rp::random::RandomRP>(entry_pc, prog_body, l1i_cfg, l1d_cfg, l2_cfg)
        }
        ReplacementPolicyArg::Plru => run_simulation::<rp::pseudo_lru::PseudoLruRP>(
            entry_pc, prog_body, l1i_cfg, l1d_cfg, l2_cfg,
        ),
    };

    println!("Program: {}", args.prog);
    println!("Replacement policy: {:?}", args.rp);
    println!(
        "Pipeline: cycles={} retired={} ipc={:.4} branch_miss={:.4}",
        pipeline_stats.total_ticked_cycle,
        pipeline_stats.inst_retire,
        pipeline_stats.ipc,
        pipeline_stats.branch_miss_rate
    );
    for (label, s) in [
        ("L1-I$", &l1i_stats),
        ("L1-D$", &l1d_stats),
        ("L2$  ", &l2_stats),
    ] {
        println!(
            "  {} load_cnt={} load_miss={} ({:.4})  store_cnt={} store_miss={} ({:.4})  overall_miss={:.4}",
            label,
            s.load_cnt,
            s.load_miss_cnt,
            s.load_miss_rate,
            s.store_cnt,
            s.store_miss_cnt,
            s.store_miss_rate,
            s.overall_miss_rate
        );
    }

    if let Some(path) = &args.stats_out {
        let report = RunReport {
            pipeline: pipeline_stats,
            l1i: l1i_stats,
            l1d: l1d_stats,
            l2: l2_stats,
            config: &args,
        };
        let json = serde_json::to_string_pretty(&report)?;
        std::fs::write(path, json)?;
        println!("wrote stats JSON to {}", path.display());
    }

    Ok(())
}
