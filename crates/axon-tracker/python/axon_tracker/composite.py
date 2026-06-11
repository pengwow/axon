"""多后端 Tracker。"""

from __future__ import annotations

from typing import Any


class MultiTracker:
    """同时向多个 Tracker 写入。"""

    def __init__(self, trackers: list) -> None:
        self.trackers = trackers

    def log_param(self, key: str, value: Any) -> None:
        for t in self.trackers:
            t.log_param(key, value)

    def log_metric(self, key: str, value: float, step: int = 0) -> None:
        for t in self.trackers:
            t.log_metric(key, value, step)

    def set_tag(self, key: str, value: str) -> None:
        for t in self.trackers:
            t.set_tag(key, value)

    def finish(self, status: str = "completed") -> None:
        for t in self.trackers:
            t.finish(status)

    def flush(self) -> None:
        for t in self.trackers:
            if hasattr(t, "flush"):
                t.flush()
