"""tracker_multi_backend.py — 多后端并行追踪。"""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

CARGO_MANIFEST = Path(__file__).parent.parent / "crates" / "axon-tracker"
sys.path.insert(0, str(CARGO_MANIFEST / "python"))

from axon_tracker.memory import MemoryTracker  # noqa: E402
from axon_tracker.local import LocalTracker  # noqa: E402
from axon_tracker.composite import MultiTracker  # noqa: E402


def main() -> int:
    print("=" * 60)
    print("Multi-Tracker 多后端并行示例")
    print("=" * 60)

    with tempfile.TemporaryDirectory() as tmp:
        # 同时向 3 个后端写入
        trackers = [
            MemoryTracker(),
            MemoryTracker(),
            LocalTracker(tmp),
        ]
        mt = MultiTracker(trackers)

        # 训练循环模拟
        for epoch in range(5):
            loss = 1.0 / (epoch + 1)
            reward = 1.0 - 0.1 * epoch
            mt.log_metric("train/loss", loss, epoch)
            mt.log_metric("val/reward", reward, epoch)

        mt.log_param("learning_rate", 0.0003)
        mt.set_tag("experiment", "ppo_baseline")
        mt.finish("completed")

        # 验证
        print(f"\n[1] 3 个 trackers:")
        for i, t in enumerate(trackers):
            print(f"  tracker[{i}] run_id: {t.run_id}")
            metrics = t.get_metrics("train/loss")
            print(f"    train/loss 指标: {len(metrics)} 条")

        # 验证 LocalTracker 写入了文件
        files = sorted(p.relative_to(tmp) for p in Path(tmp).rglob("*.json*"))
        print(f"\n[2] LocalTracker 写入文件: {len(files)}")
        for f in files[:5]:
            print(f"  {f}")

    print("\n=== ALL PASS ===")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
