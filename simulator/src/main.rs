mod hardware;
mod riscv;
mod sim;

use hardware::clock::Clocked;
use hardware::statistic::Statistic;

use hardware::mem::general_cache::replacement_policy as rp;
use hardware::mem::general_cache::GeneralCacheConfig;
use hardware::mem::simple_mem::SimpleMem;
use hardware::pipeline_processor::pipe::PipelineProcessor;
use sim::elf;

use std::cell::RefCell;
use std::error::Error;
use std::path::Path;
use std::rc::Rc;

fn main() -> Result<(), Box<dyn Error>> {
    let general_prog_names: Vec<&str> = vec!["matmul"];

    for prog_name in general_prog_names {
        println!("Run program: {}", prog_name);

        let path_prefix = Path::new("../target/riscv32im-unknown-none-elf/debug");
        let elf::ProgramInfo {
            entry_pc,
            prog_body,
        } = elf::elf_loader(&path_prefix.join(prog_name));

        let block_size_vec = vec![4usize, 8, 16, 32, 64, 128];
        for block_size in block_size_vec {
            println!("  Running simulation with block size: {}", block_size);

            let l1i_cache_config = GeneralCacheConfig::new("L1-I$".to_string())
                .with_total_size(2048)
                .with_block_size(32)
                .with_num_of_way(4);
            let l1d_cache_config = GeneralCacheConfig::new("L1-D$".to_string())
                .with_total_size(256)
                .with_block_size(block_size)
                .with_num_of_way(2);
            let l2_cache_config = GeneralCacheConfig::new("L2$".to_string())
                .with_total_size(16384)
                .with_block_size(64)
                .with_num_of_way(4);
            let mem = Rc::new(RefCell::new(SimpleMem::new(prog_body.clone())));
            let mut cpu = PipelineProcessor::<rp::fifo::FifoRP>::new(
                entry_pc,
                l1i_cache_config,
                l1d_cache_config,
                l2_cache_config,
                &mem,
            );

            while !cpu.halt {
                cpu.tick();
                mem.borrow_mut().tick();
            }
            let stats = cpu.dcache.get_statistic_info();
            println!(
                "    L1-D$ block_size={}: load_miss_rate={:.4} store_miss_rate={:.4} overall_miss_rate={:.4}",
                block_size, stats.load_miss_rate, stats.store_miss_rate, stats.overall_miss_rate
            );
        }
    }
    Ok(())
}
