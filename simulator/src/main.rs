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

use plotters::prelude::*; // use plotter to visualize the results

use std::cell::RefCell;
use std::error::Error;
use std::path::Path;
use std::rc::Rc;

fn main() -> Result<(), Box<dyn Error>> {
    // let general_prog_names: Vec<&str> = vec!["hello", "print_nums", "msort", "qsort", "matmul"];
    let general_prog_names: Vec<&str> = vec!["matmul"];

    for prog_name in general_prog_names {
        println!("Run program: {}", prog_name);

        let mut png_name = "cache_result_".to_string();
        png_name.push_str(prog_name);
        png_name.push_str(".png");

        let root = BitMapBackend::new(&png_name, (1024, 768)).into_drawing_area();
        root.fill(&WHITE)?;
        let mut chart = ChartBuilder::on(&root)
            .margin(10)
            .caption(
                "Cache Miss Rate vs. Different Block Size (Total Size of Cache is Fixed)",
                ("sans-serif", 28),
            )
            .set_label_area_size(LabelAreaPosition::Left, 60)
            .set_label_area_size(LabelAreaPosition::Right, 60)
            .set_label_area_size(LabelAreaPosition::Bottom, 40)
            .build_cartesian_2d(0usize..7usize, 0.0..50.0)
            .unwrap();

        chart.configure_mesh().draw()?;

        // simulation part
        let path_prefix = Path::new("../target/riscv32im-unknown-none-elf/debug");
        let elf::ProgramInfo {
            entry_pc,
            prog_body,
        } = elf::elf_loader(&path_prefix.join(prog_name));
        let mut l1d_miss_rate_record: Vec<f64> = vec![];

        // run sim. for different number of ways (only test dcache now)
        let block_size_vec = vec![4usize, 8, 16, 32, 64, 128];
        for block_size in block_size_vec {
            println!("Running simulation with block size: {}", block_size);

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
            l1d_miss_rate_record.push(cpu.dcache.get_statistic_info().overall_miss_rate);

            // draw the result
            chart.draw_series(LineSeries::new(
                l1d_miss_rate_record
                    .iter()
                    .enumerate()
                    .map(|(idx, val)| (idx, *val * 100f64)),
                &BLUE,
            ))?;

            root.present()?;
        }
    }
    Ok(())
}
