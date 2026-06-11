"""本地文件 Tracker（离线模式）。"""

from __future__ import annotations

import json
import time
from pathlib import Path

from .memory import MetricEntry, MemoryTracker


class LocalTracker(MemoryTracker):
    """本地 Tracker（继承 MemoryTracker + 写文件）。"""

    def __init__(self, base_dir: str) -> None:
        super().__init__()
        self.base_dir = Path(base_dir)
        self.base_dir.mkdir(parents=True, exist_ok=True)
        self.run_dir = self.base_dir / self.run_id
        self.run_dir.mkdir(parents=True, exist_ok=True)
        self._metrics_buffer: list[MetricEntry] = []
        self._params_written: bool = False

    def log_param(self, key: str, value: object) -> None:
        super().log_param(key, value)
        # 立即写入 params.json
        params_path = self.run_dir / "params.json"
        with open(params_path, "w", encoding="utf-8") as f:
            json.dump(self._params, f, indent=2, ensure_ascii=False, default=str)

    def log_metric(self, key: str, value: float, step: int = 0) -> None:
        entry = MetricEntry(key=key, value=value, step=step)
        super().log_metric(key, value, step)
        self._metrics_buffer.append(entry)

    def flush(self) -> None:
        """将缓冲的指标写入 metrics.jsonl。"""
        if not self._metrics_buffer:
            return
        metrics_path = self.run_dir / "metrics.jsonl"
        with open(metrics_path, "a", encoding="utf-8") as f:
            for entry in self._metrics_buffer:
                f.write(
                    json.dumps(
                        {
                            "key": entry.key,
                            "value": entry.value,
                            "step": entry.step,
                            "timestamp": entry.timestamp,
                        },
                        ensure_ascii=False,
                    )
                    + "\n"
                )
        self._metrics_buffer.clear()

    def finish(self, status: str = "completed") -> None:
        super().finish(status)
        self.flush()
        status_path = self.run_dir / "status.json"
        with open(status_path, "w", encoding="utf-8") as f:
            json.dump({"status": status, "end_time": time.time()}, f)
