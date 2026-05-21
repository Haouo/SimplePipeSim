#!/usr/bin/env bash
#
# Sweep L1-D$ block size and capture per-run JSON statistics so they can be
# fed to an offline plotter (gnuplot, matplotlib, etc.).
#
# Usage:   scripts/sweep_block_size.sh [prog] [rp]
# Example: scripts/sweep_block_size.sh matmul fifo
#
# Output:  results/blk_<size>_<prog>_<rp>.json  (one file per block size)

set -euo pipefail

PROG="${1:-matmul}"
RP="${2:-fifo}"
BLOCKS=(4 8 16 32 64 128)

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="${ROOT}/results"
mkdir -p "${OUT_DIR}"

echo "Sweeping L1-D\$ block size for prog=${PROG}, rp=${RP}"
for blk in "${BLOCKS[@]}"; do
    out="${OUT_DIR}/blk_${blk}_${PROG}_${RP}.json"
    echo "  block_size=${blk} -> ${out}"
    (cd "${ROOT}/simulator" && cargo run --release --quiet -- \
        --prog "${PROG}" \
        --rp "${RP}" \
        --l1d-block "${blk}" \
        --stats-out "${out}")
done

echo "Done. JSON files in ${OUT_DIR}/"
