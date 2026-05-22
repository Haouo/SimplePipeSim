#!/usr/bin/env python3
"""Aggregate simulator JSON reports and draw the first experiment plots."""

from __future__ import annotations

import argparse
import csv
import json
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any, Iterable


CONFIG_FIELDS = (
    "prog",
    "rp",
    "wp",
    "prefetcher",
    "bp",
    "memory",
    "l1i_size",
    "l1i_block",
    "l1i_ways",
    "l1i_penalty",
    "l1d_size",
    "l1d_block",
    "l1d_ways",
    "l1d_penalty",
    "l2_size",
    "l2_block",
    "l2_ways",
    "l2_penalty",
)
PIPELINE_FIELDS = (
    "total_ticked_cycle",
    "inst_fetched",
    "inst_retire",
    "branch_inst_cnt",
    "branch_miss_cnt",
    "total_flush_cnt",
    "actual_flushed_inst_cnt",
    "ipc",
    "branch_miss_rate",
)
CACHE_FIELDS = (
    "load_cnt",
    "store_cnt",
    "load_miss_cnt",
    "store_miss_cnt",
    "prefetch_issued_cnt",
    "load_miss_rate",
    "store_miss_rate",
    "overall_miss_rate",
)
CACHE_LEVELS = ("l1i", "l1d", "l2")
PLOTS = (
    (
        "l1d_overall_miss_rate",
        "L1-D overall miss rate",
        "l1d_miss_rate_vs_{x}.png",
        "rate",
    ),
    ("pipeline_ipc", "Pipeline IPC", "ipc_vs_{x}.png", "value"),
    (
        "pipeline_total_ticked_cycle",
        "Pipeline cycles",
        "cycles_vs_{x}.png",
        "count",
    ),
)


def report_paths(inputs: Iterable[Path]) -> list[Path]:
    paths: list[Path] = []
    for path in inputs:
        if path.is_dir():
            paths.extend(sorted(path.rglob("*.json")))
        elif path.is_file():
            paths.append(path)
        else:
            raise FileNotFoundError(f"result path does not exist: {path}")
    return sorted(set(paths))


def flatten_report(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        report = json.load(handle)

    row: dict[str, Any] = {"source": str(path)}
    config = report.get("config", {})
    for field in CONFIG_FIELDS:
        row[field] = config.get(field, "")

    pipeline = report.get("pipeline", {})
    for field in PIPELINE_FIELDS:
        row[f"pipeline_{field}"] = pipeline.get(field, "")

    for level in CACHE_LEVELS:
        cache = report.get(level, {})
        row[f"{level}_name"] = cache.get("name", "")
        for field in CACHE_FIELDS:
            row[f"{level}_{field}"] = cache.get(field, "")
    return row


def csv_fields() -> list[str]:
    fields = ["source", *CONFIG_FIELDS]
    fields.extend(f"pipeline_{field}" for field in PIPELINE_FIELDS)
    for level in CACHE_LEVELS:
        fields.append(f"{level}_name")
        fields.extend(f"{level}_{field}" for field in CACHE_FIELDS)
    return fields


def write_csv(rows: list[dict[str, Any]], out_dir: Path) -> Path:
    csv_path = out_dir / "runs.csv"
    with csv_path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=csv_fields())
        writer.writeheader()
        writer.writerows(rows)
    return csv_path


def as_float(value: Any) -> float | None:
    if value == "" or value is None:
        return None
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def series_key(row: dict[str, Any], x_field: str) -> tuple[Any, ...]:
    return tuple(row[field] for field in CONFIG_FIELDS if field != x_field)


def series_label(row: dict[str, Any]) -> str:
    labels = [
        f"prog={row['prog']}",
        f"rp={row['rp']}",
        f"wp={row['wp']}",
        f"pf={row['prefetcher']}",
    ]
    return ", ".join(label for label in labels if not label.endswith("="))


def plot_rows(rows: list[dict[str, Any]], out_dir: Path, x_field: str) -> list[Path]:
    try:
        import matplotlib

        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        from matplotlib.ticker import PercentFormatter
    except ImportError:
        print(
            "matplotlib is not installed; wrote runs.csv but skipped PNG plots.",
            file=sys.stderr,
        )
        return []

    if x_field not in CONFIG_FIELDS:
        raise ValueError(f"plot x-axis must be a config field, got: {x_field}")

    plot_paths: list[Path] = []
    for metric, y_label, file_name, y_kind in PLOTS:
        grouped: defaultdict[tuple[Any, ...], list[tuple[float, float, dict[str, Any]]]]
        grouped = defaultdict(list)
        for row in rows:
            x_value = as_float(row.get(x_field))
            y_value = as_float(row.get(metric))
            if x_value is not None and y_value is not None:
                grouped[series_key(row, x_field)].append((x_value, y_value, row))

        if not grouped:
            continue

        fig, ax = plt.subplots(figsize=(8.5, 5.0), constrained_layout=True)
        for points in grouped.values():
            points.sort(key=lambda point: point[0])
            xs = [point[0] for point in points]
            ys = [point[1] for point in points]
            ax.plot(xs, ys, marker="o", linewidth=1.8, label=series_label(points[0][2]))

        ax.set_title(f"{y_label} by {x_field}")
        ax.set_xlabel(x_field)
        ax.set_ylabel(y_label)
        ax.grid(True, alpha=0.25)
        if y_kind == "rate":
            ax.yaxis.set_major_formatter(PercentFormatter(xmax=1.0))
        if len(grouped) > 1:
            ax.legend(fontsize="small")

        path = out_dir / file_name.format(x=x_field)
        fig.savefig(path, dpi=160)
        plt.close(fig)
        plot_paths.append(path)
    return plot_paths


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "inputs",
        nargs="*",
        type=Path,
        default=[Path("results")],
        help="JSON result files or directories to scan. Defaults to results/.",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path("results/plots"),
        help="Directory for runs.csv and generated PNG files.",
    )
    parser.add_argument(
        "--x",
        default="l1d_block",
        help="Config field for the plot x-axis. Defaults to l1d_block.",
    )
    parser.add_argument(
        "--csv-only",
        action="store_true",
        help="Only aggregate JSON reports into runs.csv.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        paths = report_paths(args.inputs)
        if not paths:
            raise FileNotFoundError("no JSON result files found")
        rows = [flatten_report(path) for path in paths]
        args.out_dir.mkdir(parents=True, exist_ok=True)
        csv_path = write_csv(rows, args.out_dir)
        plot_paths = [] if args.csv_only else plot_rows(rows, args.out_dir, args.x)
    except (FileNotFoundError, json.JSONDecodeError, ValueError) as error:
        print(f"plot_results: {error}", file=sys.stderr)
        return 1

    print(f"aggregated {len(rows)} runs into {csv_path}")
    for path in plot_paths:
        print(f"wrote {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
