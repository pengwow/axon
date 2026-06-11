"""AXON 实验追踪 Python 类型。"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from pathlib import Path


class RunStatus(Enum):
    """运行状态。"""

    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"
    KILLED = "killed"


class ImageFormat(Enum):
    """图像格式。"""

    PNG = "png"
    JPEG = "jpeg"
    SVG = "svg"


class ParamValueType(Enum):
    """参数值类型。"""

    INT = "int"
    FLOAT = "float"
    STRING = "string"
    BOOL = "bool"
    LIST = "list"


@dataclass
class TrackerConfig:
    """Tracker 配置（从 TOML 加载）。"""

    backend_type: str = "memory"
    capacity: int = 1000
    flush_interval_s: int = 30
    max_retries: int = 3
    base_delay_ms: int = 100
    max_delay_s: int = 10
    backoff_factor: float = 2.0

    def _load_default_toml(self) -> None:
        """从默认 TOML 加载。"""
        toml_path = (
            Path(__file__).parent.parent.parent / "config" / "default_tracker.toml"
        )
        if not toml_path.exists():
            return
        try:
            import tomllib  # Python 3.11+  # noqa: PLC0415
        except ImportError:
            import tomli as tomllib  # type: ignore[no-redef]  # noqa: PLC0415
        with open(toml_path, "rb") as f:
            data = tomllib.load(f)
        backend = data.get("backend", {})
        buf = data.get("buffer", {})
        retry = data.get("retry", {})
        self.backend_type = backend.get("type", self.backend_type)
        self.capacity = buf.get("capacity", self.capacity)
        self.flush_interval_s = buf.get("flush_interval_s", self.flush_interval_s)
        self.max_retries = retry.get("max_retries", self.max_retries)
        self.base_delay_ms = retry.get("base_delay_ms", self.base_delay_ms)
        self.max_delay_s = retry.get("max_delay_s", self.max_delay_s)
        self.backoff_factor = retry.get("backoff_factor", self.backoff_factor)
