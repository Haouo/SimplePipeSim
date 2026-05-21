use clap::Parser;
use std::path::PathBuf;

/// SimplePipeSim — a teaching RISC-V pipeline simulator focused on the
/// cache subsystem. Configure the three cache levels and the replacement
/// policy from the command line, then either print stats to stdout or
/// emit JSON for offline plotting.
#[derive(Parser, Debug, Clone, serde::Serialize)]
#[command(name = "simulator", about, long_about = None)]
pub struct Args {
    /// Name of the runtime binary to run (e.g. "matmul", "qsort", "hello").
    /// Looked up relative to `--elf-dir`.
    #[arg(long)]
    pub prog: String,

    /// Directory containing pre-built RISC-V ELF binaries.
    #[arg(
        long,
        default_value = "../target/riscv32im-unknown-none-elf/debug"
    )]
    pub elf_dir: PathBuf,

    // -------- L1 Instruction cache --------
    #[arg(long, default_value_t = 2048)]
    pub l1i_size: usize,
    #[arg(long, default_value_t = 32)]
    pub l1i_block: usize,
    #[arg(long, default_value_t = 4)]
    pub l1i_ways: usize,
    #[arg(long, default_value_t = 2)]
    pub l1i_penalty: usize,

    // -------- L1 Data cache --------
    #[arg(long, default_value_t = 256)]
    pub l1d_size: usize,
    #[arg(long, default_value_t = 32)]
    pub l1d_block: usize,
    #[arg(long, default_value_t = 2)]
    pub l1d_ways: usize,
    #[arg(long, default_value_t = 2)]
    pub l1d_penalty: usize,

    // -------- L2 unified cache --------
    #[arg(long, default_value_t = 16384)]
    pub l2_size: usize,
    #[arg(long, default_value_t = 64)]
    pub l2_block: usize,
    #[arg(long, default_value_t = 4)]
    pub l2_ways: usize,
    #[arg(long, default_value_t = 10)]
    pub l2_penalty: usize,

    /// Replacement policy for all caches.
    #[arg(long, value_enum, default_value_t = ReplacementPolicyArg::Fifo)]
    pub rp: ReplacementPolicyArg,

    /// Write per-cache statistics to this JSON file.
    /// Schema: { "l1i": StatisticInfo, "l1d": StatisticInfo, "l2": StatisticInfo,
    /// "config": { mirror of CLI args } }.
    #[arg(long)]
    pub stats_out: Option<PathBuf>,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplacementPolicyArg {
    Fifo,
    Random,
}
