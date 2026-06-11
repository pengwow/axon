"""registry_rollback.py — 注册多版本 + 阶段转换 + 回滚。"""

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
    print("Model Registry 多版本 + 回滚示例")
    print("=" * 60)

    with tempfile.TemporaryDirectory() as tmp:
        storage = LocalStorageBackend(os.path.join(tmp, "models"))
        registry = ModelRegistry(storage, persist_dir=os.path.join(tmp, "registry"))

        # 注册 3 个版本并依次提升
        versions = []
        for i in range(1, 4):
            model_path = os.path.join(tmp, f"model_v{i}.bin")
            with open(model_path, "wb") as f:
                f.write(f"PPO weights v{i}".encode() * 100)

            mv = registry.register(
                "ppo-momentum",
                model_path,
                ModelMetadata(
                    description=f"PPO v{i}",
                    metrics={"sharpe": 1.0 + 0.2 * i, "max_drawdown": 0.1 * (4 - i)},
                    tags={"iteration": str(i)},
                ),
            )
            # 提升到 Production（自动归档旧版本）
            mv = registry.transition_stage("ppo-momentum", mv.version, ModelStage.PRODUCTION)
            versions.append(mv)
            print(f"\n[{i}] 注册 + 提升 v{i}: {mv}")

        # 查看各阶段状态
        print("\n[状态] 各阶段版本数：")
        for stage in [ModelStage.PRODUCTION, ModelStage.ARCHIVED, ModelStage.STAGING]:
            count = len(registry.list_versions("ppo-momentum", stage=stage))
            print(f"  {stage.value}: {count}")

        # 回滚（v3 被回滚为 RolledBack，v2 提升为 Production）
        print("\n[回滚] 执行 rollback")
        prod = registry.rollback("ppo-momentum")
        print(f"  当前 Production: {prod}")

        # 验证
        rolled_back_versions = registry.list_versions(
            "ppo-momentum", stage=ModelStage.ROLLED_BACK
        )
        assert len(rolled_back_versions) == 1
        print(f"  RolledBack 版本数: {len(rolled_back_versions)}")

    print("\n=== ALL PASS ===")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
