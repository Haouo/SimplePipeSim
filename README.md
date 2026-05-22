# SimplePipeSim

A teaching-oriented, cycle-accurate **RISC-V (RV32IM) pipeline simulator**
written in Rust. The focus is the memory hierarchy: configurable multi-level
caches with pluggable replacement, write, and prefetch policies on top of a
single-bank DRAM controller that exposes row-buffer hit/miss timing.

The repository is organised as a Cargo workspace with two crates:

| Crate       | Target            | Purpose                                                                 |
|-------------|-------------------|-------------------------------------------------------------------------|
| `simulator` | host (native)     | The simulator itself. Loads an ELF, simulates a 5-stage pipeline + cache hierarchy, prints / dumps statistics. |
| `runtime`   | `riscv32im-unknown-none-elf` | `#![no_std]` programs compiled to RV32IM ELFs that the simulator runs as workloads. |

## Modelled hardware

### Pipeline core

- Classic 5-stage in-order RISC-V pipeline: **IF / ID / EXE / MEM / WB**.
- RV32IM instruction set, with extra stall cycles for integer `MUL`
  (8 cycles) and `DIV` / `REM` (32 cycles).
- Branch prediction (selectable in code): `dummy` always-not-taken,
  `bimodal`, and `two_level_adaptive`. Misprediction triggers a pipeline
  flush counted as `branch_flushes`.

### Memory hierarchy

The pipeline is wired to a three-level cache stack, optionally backed by a
cycle-accurate DRAM controller:

```
  IF ──► L1-I$ ─┐
               ├─► L2$ (unified) ──► SimpleMem (flat 15-cycle)  or
  MEM ─► L1-D$ ┘                     SimpleDram (single-bank, row-buffer aware)
```

Each cache level is a `GeneralCache` driven by an FSM with these states:
`Idle → Lookup → {WriteBack, Allocate, WriteThroughCommit, WriteAround} →
AdditionalMissPenalty → Lookup`. A hit completes in the same cycle as the
requesting `try_register_req` call (the FSM is pre-ticked twice on
registration to model 1-cycle hit latency to the caller).

Three orthogonal cache knobs are configurable **per level** from the CLI:

| Knob                 | Options (CLI value)                              | Where                                            |
|----------------------|--------------------------------------------------|--------------------------------------------------|
| Replacement policy   | `fifo`, `random`, `plru` (PLRU needs power-of-2 ways) | `--rp`                                           |
| Write policy         | `wb-wa`, `wb-nwa`, `wt-wa`, `wt-nwa`             | `--wp`                                           |
| Prefetcher           | `null`, `next-line`                              | `--prefetcher`                                   |

Cache size, block size, associativity, and additional miss penalty are
configured independently for `l1i`, `l1d`, and `l2`.

### `SimpleDram` (cycle-accurate substrate)

`SimpleDram` is a drop-in replacement for the flat-latency `SimpleMem`. It
models a single bank with `tRCD`, `tCL`, and `tRP` (in core clock cycles)
and exposes the three latency paths a student should be able to observe:

| Path                 | Cycles to completion | Trigger                                |
|----------------------|----------------------|----------------------------------------|
| Row buffer **hit**   | `tCL`                | Bank already `Active(req_row)`.        |
| Row buffer **miss**  | `tRP + tRCD + tCL`   | Different row open ⇒ PRE then ACT.     |
| Cold open            | `tRCD + tCL`         | First access (counted as a miss).      |

Counts of hits / misses are tracked in `row_buffer_hit_cnt` and
`row_buffer_miss_cnt`. Two presets are provided: `educational_default()`
(`tRCD = tCL = tRP = 4`) and `ddr4_2400()` (17 cycles each, ~13.75 ns at
1 GHz core clock).

## Repository layout

```
SimplePipeSim/
├── Cargo.toml                 # workspace
├── simulator/                 # the simulator (host-native binary)
│   └── src/
│       ├── main.rs            # entry point, argument plumbing, stats output
│       ├── hardware/
│       │   ├── pipeline_processor/   # 5-stage pipeline FSM
│       │   ├── branch_predictor/     # dummy / bimodal / two_level_adaptive
│       │   ├── mem/
│       │   │   ├── general_cache/    # GeneralCache + RP / write / prefetch
│       │   │   ├── simple_mem.rs     # flat 15-cycle main memory
│       │   │   └── simple_dram/      # single-bank DRAM controller
│       │   ├── clock.rs              # Clocked trait
│       │   └── uop.rs                # micro-op definitions
│       ├── riscv/             # encoding, instruction, format types
│       └── sim/               # CLI, ELF loader, Konata log dumper, shell
├── runtime/                   # workloads, cross-compiled to RV32IM
│   ├── src/                   # entry stub, allocator, basic_io (stdout / exit)
│   └── bin/                   # hello, print_nums, qsort, msort, matmul, factorial
├── scripts/
│   ├── sweep_block_size.sh    # example: sweep L1-D$ block size, dump JSON
│   └── plot_results.py        # aggregate reports and plot first comparisons
├── results/                   # JSON dumps from sweeps
└── riscv-tests/               # (placeholder for RISC-V test binaries)
```

## Build and run

### Prerequisites

- Stable Rust (host toolchain).
- The `riscv32im-unknown-none-elf` target installed for the `runtime`
  crate:

  ```sh
  rustup target add riscv32im-unknown-none-elf
  ```

### Build the runtime ELFs

The `runtime` crate is configured via `runtime/.cargo/config.toml` to
build for `riscv32im-unknown-none-elf` by default. From inside the
`runtime/` directory:

```sh
cd runtime
cargo build              # debug build  -> target/riscv32im-unknown-none-elf/debug/<prog>
cargo build --release    # release build -> .../release/<prog>
```

This produces ELF binaries named after each `[[bin]]` entry in
`runtime/Cargo.toml`: `hello`, `print_nums`, `qsort`, `msort`, `matmul`,
`factorial`.

### Run the simulator

From inside the `simulator/` directory:

```sh
cd simulator
cargo run --release -- --prog matmul
```

By default the simulator looks for ELFs in
`../target/riscv32im-unknown-none-elf/debug` — override with `--elf-dir`
if you built the runtime in release mode.

### Example — a configured run

```sh
cargo run --release -- \
    --prog matmul \
    --rp plru \
    --wp wb-wa \
    --prefetcher next-line \
    --l1d-size 512 --l1d-block 32 --l1d-ways 4 --l1d-penalty 2 \
    --l2-size 32768 --l2-block 64 --l2-ways 8 --l2-penalty 10 \
    --stats-out ../results/matmul_plru_nextline.json
```

Per-cache statistics print to stdout. If `--stats-out` is set, a JSON
report is written with the shape:

```json
{
  "pipeline": { "total_ticked_cycle": ..., "inst_retire": ..., "ipc": ..., "branch_miss_rate": ... },
  "l1i":   { "name": "L1-I$", "load_cnt": ..., "load_miss_cnt": ..., "overall_miss_rate": ..., "prefetch_issued_cnt": ... },
  "l1d":   { ... },
  "l2":    { ... },
  "config": { "prog": "matmul", "rp": "plru", "wp": "wb-wa", ... }
}
```

### CLI reference

Defaults are picked to give a non-trivial workload with all three levels
visible.

| Flag                     | Default | Description                                                   |
|--------------------------|---------|---------------------------------------------------------------|
| `--prog`                 | _(required)_ | Runtime binary name (e.g. `matmul`, `qsort`, `hello`). |
| `--elf-dir`              | `../target/riscv32im-unknown-none-elf/debug` | Directory to resolve `--prog` against. |
| `--l1i-size/-block/-ways/-penalty` | `2048 / 32 / 4 / 2` | L1 instruction cache geometry and post-miss penalty. |
| `--l1d-size/-block/-ways/-penalty` | `256 / 32 / 2 / 2` | L1 data cache geometry. |
| `--l2-size/-block/-ways/-penalty`  | `16384 / 64 / 4 / 10` | L2 unified cache geometry. |
| `--rp`                   | `fifo`  | Replacement policy: `fifo`, `random`, `plru`. |
| `--wp`                   | `wb-wa` | Write policy: `wb-wa`, `wb-nwa`, `wt-wa`, `wt-nwa`. |
| `--prefetcher`           | `null`  | `null` or `next-line`. |
| `--stats-out`            | _(unset)_ | Path to write the JSON report. |

## Sweep scripts

`scripts/sweep_block_size.sh` shows the intended workflow for parameter
studies: run the simulator across a range of one knob, dump JSON for each
configuration, and feed the result into the first plotting workflow.

```sh
scripts/sweep_block_size.sh hello fifo
# produces results/blk_<size>_hello_fifo.json for size ∈ {4, 8, 16, 32, 64, 128}
python3 scripts/plot_results.py results
```

`plot_results.py` writes a tidy `results/plots/runs.csv` plus PNG comparisons
for L1-D miss rate, IPC, and cycle count against `l1d_block`. It uses
`matplotlib` for PNG output; `--csv-only` keeps aggregation available without
that dependency. Use `--x` with another cache config field, such as
`l1d_ways`, when plotting a different sweep.

## Testing

Unit tests live next to each module under `#[cfg(test)]` (cache FSM,
DRAM timing paths, prefetcher behaviour, etc.). Run them from the
workspace root:

```sh
cargo test -p simulator
```

## Status

Implemented:

- 5-stage RV32IM pipeline with bimodal / two-level adaptive branch prediction.
- Three-level cache hierarchy (L1-I, L1-D, unified L2).
- Replacement: FIFO, Random, tree-PLRU.
- Write policy: write-back / write-through × write-allocate / no-write-allocate.
- Prefetcher trait + null and next-line implementations.
- Single-bank cycle-accurate DRAM controller (`SimpleDram`) with
  row-buffer hit / miss accounting.
- JSON statistics export and a parameter-sweep helper script.

Out of scope for now: multi-bank DRAM, virtual memory / TLB,
out-of-order execution, SMP / cache coherence.
