"""内存 Tracker（纯 Python，测试用）。"""

from __future__ import annotations

import time
from dataclasses import dataclass, field
from typing import Any


@dataclass
class MetricEntry:
    """指标条目。"""

    key: str
    value: float
    step: int
    timestamp: float = field(default_factory=time.time)


class MemoryTracker:
    """内存 Tracker（无外部依赖）。"""

    def __init__(self) -> None:
        self.run_id = f"run_{int(time.time() * 1000)}"
        self._params: dict[str, Any] = {}
        self._metrics: list[MetricEntry] = []
        self._tags: dict[str, str] = {}
        self._status: str = "running"

    def log_param(self, key: str, value: Any) -> None:
        """记录参数。"""
        self._params[key] = value

    def log_metric(self, key: str, value: float, step: int = 0) -> None:
        """记录标量指标。"""
        self._metrics.append(MetricEntry(key=key, value=value, step=step))

    def set_tag(self, key: str, value: str) -> None:
        """设置标签。"""
        self._tags[key] = value

    def finish(self, status: str = "completed") -> None:
        """结束运行。"""
        self._status = status

    def get_metrics(self, key: str | None = None) -> list[MetricEntry]:
        """获取指标。"""
        if key is None:
            return list(self._metrics)
        return [m for m in self._metrics if m.key == key]

    def get_param(self, key: str) -> Any:
        """获取参数。"""
        return self._params.get(key)

    def get_all_params(self) -> dict[str, Any]:
        return dict(self._params)

    def get_status(self) -> str:
        return self._status
