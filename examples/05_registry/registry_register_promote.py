"""registry_register_promote.py — 注册 + 提升到 Production。"""

from __future__ import annotations

import os
import sys
import tempfile
from pathlib import Path

CARGO_MANIFEST = Path(__file__).parent.parent / "crates" / "axon-registry"
sys.path.insert(0, str(CARGO_MANIFEST / "python"))

from axon_registry.registry import ModelRegistry  # noqa: E402
from axon_registry.storage import LocalStorageBackend  # noqa: E402
from axon_registry.types import ModelMetadata, ModelStage  # noqa: E402


def main() -> int:
    print("=" * 60)
    print("Model Registry 注册 + 提升到 Production")
    print("=" * 60)

    with tempfile.TemporaryDirectory() as tmp:
        # 准备源文件
        model_path = os.path.join(tmp, "model_v1.bin")
        with open(model_path, "wb") as f:
            f.write(b"PPO policy weights v1 (1024 params)")

        # 创建存储 + 注册表
        storage = LocalStorageBackend(os.path.join(tmp, "models"))
        registry = ModelRegistry(storage, persist_dir=os.path.join(tmp, "registry"))

        # 注册 v1
        metadata = ModelMetadata(
            description="PPO momentum strategy v1",
            metrics={"sharpe": 1.5, "max_drawdown": 0.12},
            dataset_hash="sha256:abc123",
            git_commit="a1b2c3d",
            training_duration_secs=3600.0,
            author="alice",
            tags={"env": "backtest"},
        )
        mv1 = registry.register("ppo-momentum", model_path, metadata)
        print(f"\n[1] 注册 v1: {mv1}")
        print(f"   artifact_hash: {mv1.artifact_hash[:16]}...")

        # 提升到 Production
        mv1_prod = registry.transition_stage("ppo-momentum", mv1.version, ModelStage.PRODUCTION)
        print(f"\n[2] 提升到 Production: {mv1_prod}")

        # 获取 Production 版本
        prod = registry.get_production("ppo-momentum")
        assert prod is not None and prod.version == mv1.version
        print(f"\n[3] 当前 Production: {prod}")

        # 列出所有版本
        all_versions = registry.list_versions("ppo-momentum")
        print(f"\n[4] 所有版本: {len(all_versions)}")
        for v in all_versions:
            print(f"   - {v}")

    print("\n=== ALL PASS ===")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
