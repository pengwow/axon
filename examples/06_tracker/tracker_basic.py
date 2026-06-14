"""tracker_basic.py — Memory + Local Tracker 基本用法。"""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

CARGO_MANIFEST = Path(__file__).parent.parent / "crates" / "axon-tracker"
sys.path.insert(0, str(CARGO_MANIFEST / "python"))

from axon_tracker.memory import MemoryTracker  # noqa: E402
from axon_tracker.local import LocalTracker  # noqa: E402


def main() -> int:
    print("=" * 60)
    print("Tracker 基本用法示例")
    print("=" * 60)

    # 1. Memory Tracker
    print("\n[1] MemoryTracker")
    mt = MemoryTracker()
    mt.log_param("learning_rate", 0.001)
    mt.log_param("batch_size", 256)
    mt.log_param("algorithm", "PPO")
    mt.log_metric("train/loss", 0.5, 0)
    mt.log_metric("train/loss", 0.4, 1)
    mt.log_metric("val/reward", 1.2, 0)
    mt.set_tag("strategy", "momentum")
    mt.set_tag("market_regime", "high_volatility")
    mt.finish("completed")
    print(f"  run_id: {mt.run_id}")
    print(f"  params: {mt.get_all_params()}")
    print(
        f"  loss history: {[(m.step, m.value) for m in mt.get_metrics('train/loss')]}"
    )
    print(f"  status: {mt.get_status()}")

    # 2. Local Tracker
    print("\n[2] LocalTracker")
    with tempfile.TemporaryDirectory() as tmp:
        lt = LocalTracker(tmp)
        lt.log_param("learning_rate", 0.0003)
        lt.log_metric("train/loss", 0.5, 0)
        lt.log_metric("train/loss", 0.4, 1)
        lt.log_metric("val/reward", 1.2, 0)
        lt.set_tag("strategy", "ppo")
        lt.finish("completed")
        print(f"  run_id: {lt.run_id}")
        files = sorted(p.relative_to(tmp) for p in Path(tmp).rglob("*") if p.is_file())
        print(f"  files: {[str(f) for f in files]}")

    print("\n=== ALL PASS ===")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
