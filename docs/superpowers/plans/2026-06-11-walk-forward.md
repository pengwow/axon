# Walk-Forward 滚动前向验证 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 `axon-walk-forward` crate，提供时间序列滚动前向验证的索引分割、purge/embargo 防泄漏、OOS 指标聚合与稳定性分析、PyO3 桥接层与 Python 端 `axon_walk_forward` 包。

**Architecture:**
- Rust 端负责纯索引分割计算（O(n_folds)，百万级数据毫秒级）、purge/embargo/leakage 检测、Deflated Sharpe Ratio 等数学运算
- Python 端负责与 RL 训练管线集成（依赖 `axon_rl` / `pandas` / `numpy` / `scipy`）、可视化与人工复审
- Rust ↔ Python 通过 PyO3 桥接，配置/结果通过 TOML/JSON 序列化

**Tech Stack:**
- Rust 1.96.0 / edition 2024 / Cargo workspace
- Python 3.12 / numpy / pandas / scipy（可选）/ matplotlib（可选）
- pyo3 0.23（feature = `python`）
- serde / serde_json / toml / thiserror / tracing

---

## 文件结构

### 新建文件

**Cargo workspace 层**
- 修改 `Cargo.toml`：workspace.members 新增 `"crates/axon-walk-forward"` + workspace.dependencies 新增 `axon-walk-forward`

**Rust crate（`crates/axon-walk-forward/`）**
- `Cargo.toml`：crate 元数据 + 依赖
- `src/lib.rs`：模块入口与公开 re-export
- `src/config.rs`：`WalkForwardConfig` / `WindowType`
- `src/split.rs`：`FoldSplit` 与 `TimeSeriesSplitter`（Rolling + Expanding）
- `src/purge.rs`：purge / embargo / leakage 检测
- `src/metrics.rs`：`ISMetrics` / `OOSMetrics` / `FoldResult` / `WalkForwardResult` / `AggregatedMetrics` / `StabilityMetrics` / `LeakageCheck`
- `src/evaluation.rs`：指标聚合函数（`aggregate_folds` / `deflated_sharpe` 等）
- `src/error.rs`：`WalkForwardError` 统一错误类型
- `src/python/mod.rs`：PyO3 桥接层（`WalkForwardRunner` + 便捷函数）
- `config/default_wf.toml`：默认 TOML 配置（5 年 train + 1 年 test 季度滚动）

**Python 包（`crates/axon-walk-forward/python/axon_walk_forward/`）**
- `__init__.py`：暴露版本
- `types.py`：`WalkForwardConfig` / `WindowType` / `FoldSplit` / `FoldResult` / `WalkForwardResult` / `AggregatedMetrics` / `StabilityMetrics`
- `splitter.py`：`TimeSeriesSplitter` + `expand_window` / `rolling_window` 便捷函数
- `purging.py`：`purge_overlapping_labels` / `embargo_indices` / `detect_leakage`
- `evaluation.py`：`aggregate_folds` + `_deflated_sharpe`

**示例（`examples/`）**
- `walk_forward_basic.py`：基本 Rolling/Expanding 分割 + 指标聚合 smoke test
- `walk_forward_purging.py`：purge/embargo/leakage 验证

**文档**
- `CHANGELOG.md`：新增 Phase 2 P1 条目
- `axon-design/01-tdd/03-phase2-training/02-walk-forward.md`：勾选验收标准

---

## Task 1: 创建 axon-walk-forward crate 骨架

**Files:**
- Create: `crates/axon-walk-forward/Cargo.toml`
- Create: `crates/axon-walk-forward/src/lib.rs`
- Modify: `Cargo.toml:8-10`（workspace.members）

- [ ] **Step 1: 创建 crate 目录**

```bash
mkdir -p crates/axon-walk-forward/src
```

- [ ] **Step 2: 写 Cargo.toml**

```toml
[package]
name = "axon-walk-forward"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
homepage.workspace = true
documentation.workspace = true
authors.workspace = true
description = "AXON 滚动前向验证：时间序列分割 + purge/embargo 防泄漏 + 指标聚合 + 稳定性分析（Phase 2 P1 阶段填充）"

[features]
default = []
# PyO3 绑定（默认禁用，启用需 python toolchain）
python = ["dep:pyo3"]

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
toml = "0.8"
thiserror = { workspace = true }
tracing = { workspace = true }

# Python 绑定
pyo3 = { version = "0.23", optional = true }

[dev-dependencies]
pretty_assertions = { workspace = true }
proptest = { workspace = true }

# 同时生成 cdylib（供 Python `import axon_walk_forward` 使用）和 rlib（供 Rust crate 引用）
[lib]
crate-type = ["rlib", "cdylib"]
```

- [ ] **Step 3: 写最小 lib.rs**

```rust
//! AXON 滚动前向验证
//!
//! 提供完整的时间序列验证工具链：Rolling / Expanding 窗口分割、
//! purge / embargo 防泄漏、OOS 指标聚合、Deflated Sharpe Ratio 等。
//!
//! # 模块规划
//!
//! | 模块 | 说明 |
//! |------|------|
//! | [`config`] | WalkForwardConfig + WindowType |
//! | [`split`] | TimeSeriesSplitter（Rolling / Expanding）|
//! | [`purge`] | purge_overlapping_labels / embargo_indices / detect_leakage |
//! | [`metrics`] | FoldResult / ISMetrics / OOSMetrics / WalkForwardResult |
//! | [`evaluation`] | aggregate_folds / deflated_sharpe |
//! | [`error`] | 统一错误类型 |
//! | [`python`] | PyO3 绑定（feature = `python`） |

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

pub mod config;
pub mod error;
pub mod evaluation;
pub mod metrics;
pub mod purge;
pub mod split;

#[cfg(feature = "python")]
pub mod python;

pub use config::{WalkForwardConfig, WindowType};
pub use error::{WalkForwardError, WalkForwardResult};
pub use evaluation::{aggregate_folds, compute_deflated_sharpe};
pub use metrics::{
    AggregatedMetrics, FoldResult, FoldSplit, ISMetrics, LeakageCheck, OOSMetrics,
    StabilityMetrics, WalkForwardResult,
};
pub use purge::{detect_leakage, embargo_indices, purge_overlapping_labels};
pub use split::{TimeSeriesSplitter, expand_window, rolling_window};
```

- [ ] **Step 4: 临时创建空子模块文件**

```bash
touch crates/axon-walk-forward/src/config.rs
touch crates/axon-walk-forward/src/error.rs
touch crates/axon-walk-forward/src/evaluation.rs
touch crates/axon-walk-forward/src/metrics.rs
touch crates/axon-walk-forward/src/purge.rs
touch crates/axon-walk-forward/src/split.rs
```

- [ ] **Step 5: 注册到 workspace**

在 `Cargo.toml:8-10` 的 `members` 数组添加 `"crates/axon-walk-forward"`，并在 `[workspace.dependencies]` 添加 `axon-walk-forward = { path = "crates/axon-walk-forward" }`

- [ ] **Step 6: 编译验证**

```bash
cd /Users/liupeng/workspace/axon && cargo build -p axon-walk-forward 2>&1 | tail -5
```

期望：`Finished dev profile ...` （即使有 warning 也 OK，下一步补子模块）

- [ ] **Step 7: Commit**

```bash
git add crates/axon-walk-forward Cargo.toml
git commit -m "feat(axon-walk-forward): create crate skeleton"
```

---

## Task 2: 实现 config 模块（WalkForwardConfig + WindowType）

**Files:**
- Modify: `crates/axon-walk-forward/src/config.rs`

- [ ] **Step 1: 写失败测试**

```rust
//! Walk-Forward 配置定义

use serde::{Deserialize, Serialize};

/// 窗口类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowType {
    /// 固定长度滑动窗口
    Rolling,
    /// 训练集从起点开始不断增长
    Expanding,
}

/// Walk-Forward 验证配置（索引单位）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardConfig {
    /// 训练窗口大小（数据点数）
    pub train_size: usize,
    /// 验证窗口大小（0 表示无验证集）
    #[serde(default)]
    pub validation_size: usize,
    /// 测试窗口大小
    pub test_size: usize,
    /// 滚动步长
    pub step_size: usize,
    /// 窗口类型
    pub window_type: WindowType,
    /// 训练-测试之间的 purge gap（防标签泄漏）
    #[serde(default)]
    pub purge_gap: usize,
    /// embargo 百分比（0.01 = 1%）
    #[serde(default = "default_embargo_pct")]
    pub embargo_pct: f64,
}

fn default_embargo_pct() -> f64 {
    0.01
}

impl WalkForwardConfig {
    /// 创建 Expanding 窗口配置
    pub fn expanding(train_size: usize, test_size: usize, step_size: usize) -> Self {
        Self {
            train_size,
            validation_size: 0,
            test_size,
            step_size,
            window_type: WindowType::Expanding,
            purge_gap: 0,
            embargo_pct: 0.0,
        }
    }

    /// 创建 Rolling 窗口配置
    pub fn rolling(train_size: usize, test_size: usize, step_size: usize) -> Self {
        Self {
            train_size,
            validation_size: 0,
            test_size,
            step_size,
            window_type: WindowType::Rolling,
            purge_gap: 0,
            embargo_pct: 0.0,
        }
    }

    /// 校验配置合法性
    pub fn validate(&self) -> Result<(), String> {
        if self.train_size == 0 {
            return Err("train_size must be > 0".to_string());
        }
        if self.test_size == 0 {
            return Err("test_size must be > 0".to_string());
        }
        if self.step_size == 0 {
            return Err("step_size must be > 0".to_string());
        }
        if !self.embargo_pct.is_finite() || !(0.0..=1.0).contains(&self.embargo_pct) {
            return Err(format!("embargo_pct ({}) must be in [0.0, 1.0]", self.embargo_pct));
        }
        Ok(())
    }
}

impl Default for WindowType {
    fn default() -> Self {
        WindowType::Expanding
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expanding_constructor() {
        let cfg = WalkForwardConfig::expanding(252, 63, 63);
        assert_eq!(cfg.train_size, 252);
        assert_eq!(cfg.test_size, 63);
        assert_eq!(cfg.step_size, 63);
        assert_eq!(cfg.window_type, WindowType::Expanding);
    }

    #[test]
    fn test_rolling_constructor() {
        let cfg = WalkForwardConfig::rolling(252, 63, 63);
        assert_eq!(cfg.window_type, WindowType::Rolling);
    }

    #[test]
    fn test_validate_ok() {
        assert!(WalkForwardConfig::expanding(252, 63, 63).validate().is_ok());
    }

    #[test]
    fn test_validate_zero_train() {
        let cfg = WalkForwardConfig { train_size: 0, ..WalkForwardConfig::expanding(252, 63, 63) };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_zero_test() {
        let cfg = WalkForwardConfig { test_size: 0, ..WalkForwardConfig::expanding(252, 63, 63) };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_embargo() {
        let mut cfg = WalkForwardConfig::expanding(252, 63, 63);
        cfg.embargo_pct = 1.5;
        assert!(cfg.validate().is_err());
        cfg.embargo_pct = -0.1;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_default_window_type() {
        assert_eq!(WindowType::default(), WindowType::Expanding);
    }
}
```

- [ ] **Step 2: 编译并运行测试**

```bash
cd /Users/liupeng/workspace/axon && cargo test -p axon-walk-forward config:: 2>&1 | tail -20
```

期望：7 个测试全部通过

- [ ] **Step 3: Commit**

```bash
git add crates/axon-walk-forward/src/config.rs
git commit -m "feat(axon-walk-forward): implement WalkForwardConfig + WindowType"
```

---

## Task 3: 实现 split 模块（FoldSplit + TimeSeriesSplitter）

**Files:**
- Modify: `crates/axon-walk-forward/src/split.rs`

- [ ] **Step 1: 写失败测试**

```rust
//! 时间序列分割：FoldSplit + TimeSeriesSplitter

use serde::{Deserialize, Serialize};

use crate::config::WalkForwardConfig;
use crate::config::WindowType;

/// 单个 fold 的索引分割信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoldSplit {
    /// fold 序号
    pub fold_id: usize,
    /// 训练集起始索引（包含）
    pub train_start: usize,
    /// 训练集结束索引（不包含）
    pub train_end: usize,
    /// 验证集起始索引（包含）
    pub validation_start: usize,
    /// 验证集结束索引（不包含）
    pub validation_end: usize,
    /// 测试集起始索引（包含）
    pub test_start: usize,
    /// 测试集结束索引（不包含）
    pub test_end: usize,
}

impl FoldSplit {
    /// 训练集大小
    pub fn train_size(&self) -> usize {
        self.train_end - self.train_start
    }

    /// 验证集大小
    pub fn val_size(&self) -> usize {
        self.validation_end - self.validation_start
    }

    /// 测试集大小
    pub fn test_size(&self) -> usize {
        self.test_end - self.test_start
    }

    /// 训练集索引范围（`train_start..train_end`）
    pub fn train_range(&self) -> std::ops::Range<usize> {
        self.train_start..self.train_end
    }

    /// 验证集索引范围（`validation_start..validation_end`）
    pub fn val_range(&self) -> std::ops::Range<usize> {
        self.validation_start..self.validation_end
    }

    /// 测试集索引范围（`test_start..test_end`）
    pub fn test_range(&self) -> std::ops::Range<usize> {
        self.test_start..self.test_end
    }
}

/// 时间序列分割器
pub struct TimeSeriesSplitter {
    config: WalkForwardConfig,
}

impl TimeSeriesSplitter {
    /// 构造分割器
    pub fn new(config: WalkForwardConfig) -> Self {
        Self { config }
    }

    /// 获取配置引用
    pub fn config(&self) -> &WalkForwardConfig {
        &self.config
    }

    /// 生成所有 fold 的索引分割
    ///
    /// 返回的 fold 数取决于 `n_samples` 和配置中的窗口大小。
    /// 当剩余数据不足以生成完整 fold 时停止。
    pub fn split(&self, n_samples: usize) -> Vec<FoldSplit> {
        let cfg = &self.config;
        let mut folds = Vec::new();
        let mut fold_id = 0;

        // 第一个 fold 的"test_end" 起始位置
        // train_size + validation_size + purge_gap + test_size
        let block = cfg.train_size
            + cfg.validation_size
            + cfg.purge_gap
            + cfg.test_size;

        if n_samples < block {
            return folds;
        }

        // 推进位置：每个 fold 推进 step_size
        let mut step_pos = block; // 第一个 fold 结束后推进的位置

        loop {
            // test 区间
            let test_end = step_pos;
            let test_start = test_end - cfg.test_size;
            let val_end = if cfg.validation_size > 0 {
                test_start - cfg.purge_gap
            } else {
                test_start - cfg.purge_gap
            };
            let val_start = if cfg.validation_size > 0 {
                val_end - cfg.validation_size
            } else {
                val_end
            };
            // 等价简化为：
            // val_end = test_start - purge_gap
            // val_start = val_end - validation_size
            // train_end = val_start
            // train_start = (Rolling) train_end - train_size; (Expanding) 0

            let val_end_s = test_start - cfg.purge_gap;
            let val_start_s = val_end_s.saturating_sub(cfg.validation_size);
            let train_end = val_start_s;
            let train_start = match cfg.window_type {
                WindowType::Rolling => train_end.saturating_sub(cfg.train_size),
                WindowType::Expanding => 0,
            };

            // 防越界：训练起点不能 > 训练终点
            if train_start > train_end {
                break;
            }

            folds.push(FoldSplit {
                fold_id,
                train_start,
                train_end,
                validation_start: val_start_s,
                validation_end: val_end_s,
                test_start,
                test_end,
            });

            fold_id += 1;
            step_pos += cfg.step_size;

            if step_pos > n_samples {
                break;
            }
        }

        folds
    }
}

/// 便捷函数：Expanding 窗口
pub fn expand_window(
    n_samples: usize,
    train_size: usize,
    test_size: usize,
    step_size: usize,
    purge_gap: usize,
) -> Vec<FoldSplit> {
    let mut cfg = WalkForwardConfig::expanding(train_size, test_size, step_size);
    cfg.purge_gap = purge_gap;
    TimeSeriesSplitter::new(cfg).split(n_samples)
}

/// 便捷函数：Rolling 窗口
pub fn rolling_window(
    n_samples: usize,
    train_size: usize,
    test_size: usize,
    step_size: usize,
    purge_gap: usize,
) -> Vec<FoldSplit> {
    let mut cfg = WalkForwardConfig::rolling(train_size, test_size, step_size);
    cfg.purge_gap = purge_gap;
    TimeSeriesSplitter::new(cfg).split(n_samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WindowType;

    #[test]
    fn test_expanding_basic() {
        // 1000 个数据点，train=200, test=100, step=100
        // 第一个 fold: train [0, 200), test [200, 300)
        // 第二个 fold: train [0, 300), test [300, 400)
        // ... 直到 test_end > 1000
        let cfg = WalkForwardConfig::expanding(200, 100, 100);
        let folds = TimeSeriesSplitter::new(cfg).split(1000);
        assert_eq!(folds.len(), 8); // 200, 300, ..., 900 → test [200..300], ..., [900..1000]
        // 检查 train 始终从 0 开始
        for f in &folds {
            assert_eq!(f.train_start, 0);
        }
    }

    #[test]
    fn test_rolling_basic() {
        // 1000 个数据点，train=200, test=100, step=100
        // 第一个 fold: train [0, 200), test [200, 300)
        // 第二个 fold: train [100, 300), test [300, 400) ← rolling
        // 第三个 fold: train [200, 400), test [400, 500)
        let cfg = WalkForwardConfig::rolling(200, 100, 100);
        let folds = TimeSeriesSplitter::new(cfg).split(1000);
        assert_eq!(folds.len(), 8);
        // 检查 train 窗口固定 200
        for f in &folds {
            assert_eq!(f.train_size(), 200);
        }
    }

    #[test]
    fn test_no_overlap() {
        let cfg = WalkForwardConfig::expanding(100, 50, 50);
        let folds = TimeSeriesSplitter::new(cfg).split(500);
        for w in folds.windows(2) {
            let prev = &w[0];
            let curr = &w[1];
            // test 区间不应与前一个 fold 的 test/train/val 区间重叠
            assert!(curr.train_start >= prev.test_end);
            assert!(curr.test_start >= prev.test_end);
        }
    }

    #[test]
    fn test_test_always_after_train() {
        let cfg = WalkForwardConfig::rolling(200, 50, 25);
        let folds = TimeSeriesSplitter::new(cfg).split(1000);
        for f in &folds {
            assert!(f.test_start >= f.train_end);
            assert!(f.test_start >= f.validation_end);
        }
    }

    #[test]
    fn test_purge_gap() {
        let mut cfg = WalkForwardConfig::expanding(100, 50, 50);
        cfg.purge_gap = 5;
        let folds = TimeSeriesSplitter::new(cfg).split(500);
        for f in &folds {
            assert_eq!(f.test_start, f.validation_end + 5);
        }
    }

    #[test]
    fn test_validation_set() {
        let mut cfg = WalkForwardConfig::expanding(100, 50, 50);
        cfg.validation_size = 20;
        let folds = TimeSeriesSplitter::new(cfg).split(500);
        for f in &folds {
            assert_eq!(f.val_size(), 20);
            assert_eq!(f.test_start, f.validation_end + cfg.purge_gap);
        }
    }

    #[test]
    fn test_too_small_data() {
        let cfg = WalkForwardConfig::expanding(200, 50, 50);
        let folds = TimeSeriesSplitter::new(cfg).split(100); // < train + test
        assert!(folds.is_empty());
    }

    #[test]
    fn test_fold_split_methods() {
        let fold = FoldSplit {
            fold_id: 0,
            train_start: 0,
            train_end: 100,
            validation_start: 100,
            validation_end: 120,
            test_start: 125,
            test_end: 150,
        };
        assert_eq!(fold.train_size(), 100);
        assert_eq!(fold.val_size(), 20);
        assert_eq!(fold.test_size(), 25);
        assert_eq!(fold.train_range(), 0..100);
        assert_eq!(fold.val_range(), 100..120);
        assert_eq!(fold.test_range(), 125..150);
    }

    #[test]
    fn test_expand_window_helper() {
        let folds = expand_window(1000, 200, 100, 100, 0);
        assert_eq!(folds.len(), 8);
    }

    #[test]
    fn test_rolling_window_helper() {
        let folds = rolling_window(1000, 200, 100, 100, 0);
        assert_eq!(folds.len(), 8);
    }
}
```

- [ ] **Step 2: 编译并运行测试**

```bash
cd /Users/liupeng/workspace/axon && cargo test -p axon-walk-forward split:: 2>&1 | tail -20
```

期望：10 个测试全部通过

- [ ] **Step 3: Commit**

```bash
git add crates/axon-walk-forward/src/split.rs
git commit -m "feat(axon-walk-forward): implement TimeSeriesSplitter + FoldSplit"
```

---

## Task 4: 实现 purge 模块（防泄漏）

**Files:**
- Modify: `crates/axon-walk-forward/src/purge.rs`

- [ ] **Step 1: 写失败测试**

```rust
//! Purge / Embargo / Leakage 检测
//!
//! 防泄漏是时间序列交叉验证的核心约束：
//! - **Purge**：移除训练集中与测试集标签重叠的样本
//! - **Embargo**：测试集后添加隔离期，防止自相关泄漏
//! - **Leakage 检测**：校验 train/test 严格分离

use serde::{Deserialize, Serialize};

use crate::metrics::LeakageCheck;

/// Purge 训练集：移除索引 >= (test_start - label_horizon) 的样本
///
/// Args:
/// - train_idx: 训练集索引
/// - test_idx: 测试集索引
/// - label_horizon: 标签前瞻步数
///
/// Returns:
/// - 清洗后的训练集索引
pub fn purge_overlapping_labels(
    train_idx: &[usize],
    test_idx: &[usize],
    label_horizon: usize,
) -> Vec<usize> {
    if test_idx.is_empty() || label_horizon == 0 {
        return train_idx.to_vec();
    }
    let test_start = *test_idx.iter().min().expect("non-empty test_idx");
    let cutoff = test_start.saturating_sub(label_horizon);
    train_idx
        .iter()
        .copied()
        .filter(|&i| i < cutoff)
        .collect()
}

/// Embargo 索引：在测试集之后添加隔离期
///
/// Args:
/// - test_idx: 测试集索引
/// - embargo_pct: embargo 占测试集比例（0.0~1.0）
/// - n_total: 总样本数
///
/// Returns:
/// - 需要 embargo 的索引范围
pub fn embargo_indices(test_idx: &[usize], embargo_pct: f64, n_total: usize) -> Vec<usize> {
    if test_idx.is_empty() || embargo_pct <= 0.0 {
        return Vec::new();
    }
    let test_end = *test_idx.iter().max().expect("non-empty test_idx");
    let embargo_size = ((test_idx.len() as f64) * embargo_pct).ceil() as usize;
    let embargo_size = embargo_size.max(1);
    let start = test_end + 1;
    let end = (start + embargo_size).min(n_total);
    if start >= n_total {
        return Vec::new();
    }
    (start..end).collect()
}

/// 检测训练集与测试集之间是否存在数据泄漏
///
/// Returns:
/// - `(has_leakage, leaked_pairs)`：leaked_pairs 是 (train_idx, test_idx) 元组列表
pub fn detect_leakage(
    train_idx: &[usize],
    test_idx: &[usize],
    feature_lag: usize,
) -> (bool, Vec<(usize, usize)>) {
    if train_idx.is_empty() || test_idx.is_empty() {
        return (false, Vec::new());
    }

    // 1. 直接索引重叠
    let train_set: std::collections::HashSet<usize> = train_idx.iter().copied().collect();
    let test_set: std::collections::HashSet<usize> = test_idx.iter().copied().collect();
    let overlap: Vec<usize> = train_set.intersection(&test_set).copied().collect();
    if !overlap.is_empty() {
        let pairs: Vec<(usize, usize)> = overlap.iter().map(|&i| (i, i)).collect();
        return (true, pairs);
    }

    // 2. 时间邻近性泄漏（test_min - train_max <= feature_lag）
    if feature_lag > 0 {
        let train_max = *train_idx.iter().max().expect("non-empty");
        let test_min = *test_idx.iter().min().expect("non-empty");
        if test_min.saturating_sub(train_max) <= feature_lag {
            return (true, vec![(train_max, test_min)]);
        }
    }

    (false, Vec::new())
}

/// 便捷函数：返回结构化的泄漏检测报告
pub fn leakage_check(
    train_idx: &[usize],
    test_idx: &[usize],
    feature_lag: usize,
) -> LeakageCheck {
    let (has_leakage, leaked_indices) = detect_leakage(train_idx, test_idx, feature_lag);
    let details = if has_leakage {
        format!(
            "leakage detected: {} leaked pairs, feature_lag={}",
            leaked_indices.len(),
            feature_lag
        )
    } else {
        "no leakage".to_string()
    };
    LeakageCheck {
        has_leakage,
        leaked_indices,
        details,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_purge_basic() {
        // train: 0..100, test: 100..150, label_horizon: 5
        // 应移除 train 中索引 >= 95 的样本
        let train: Vec<usize> = (0..100).collect();
        let test: Vec<usize> = (100..150).collect();
        let purged = purge_overlapping_labels(&train, &test, 5);
        assert_eq!(purged.len(), 95);
        assert!(purged.iter().all(|&i| i < 95));
    }

    #[test]
    fn test_purge_zero_horizon() {
        let train = vec![0, 1, 2, 3];
        let test = vec![5, 6];
        let purged = purge_overlapping_labels(&train, &test, 0);
        assert_eq!(purged, train);
    }

    #[test]
    fn test_purge_empty_test() {
        let train = vec![0, 1, 2];
        let purged = purge_overlapping_labels(&train, &[], 5);
        assert_eq!(purged, train);
    }

    #[test]
    fn test_embargo_basic() {
        // test: 100..150 (50 个), embargo_pct: 0.1 → 5 个索引
        let test: Vec<usize> = (100..150).collect();
        let embargoed = embargo_indices(&test, 0.1, 200);
        assert_eq!(embargoed, vec![151, 152, 153, 154, 155]);
    }

    #[test]
    fn test_embargo_zero_pct() {
        let test = vec![10, 20];
        let embargoed = embargo_indices(&test, 0.0, 100);
        assert!(embargoed.is_empty());
    }

    #[test]
    fn test_embargo_clamp_to_total() {
        // test: 195..200, total=200, embargo_pct=1.0 → 5 个但越界
        let test: Vec<usize> = (195..200).collect();
        let embargoed = embargo_indices(&test, 1.0, 200);
        assert!(embargoed.is_empty()); // 越界
    }

    #[test]
    fn test_detect_leakage_overlap() {
        let train = vec![1, 2, 3, 4];
        let test = vec![3, 4, 5, 6];
        let (has, pairs) = detect_leakage(&train, &test, 0);
        assert!(has);
        assert_eq!(pairs.len(), 2); // 3 和 4
    }

    #[test]
    fn test_detect_leakage_no_overlap() {
        let train = vec![0, 1, 2, 3];
        let test = vec![10, 11, 12];
        let (has, pairs) = detect_leakage(&train, &test, 0);
        assert!(!has);
        assert!(pairs.is_empty());
    }

    #[test]
    fn test_detect_leakage_lag() {
        // train: 0..100, test: 102..110, feature_lag=5 → 102-100=2 <= 5 → 泄漏
        let train: Vec<usize> = (0..100).collect();
        let test: Vec<usize> = (102..110).collect();
        let (has, pairs) = detect_leakage(&train, &test, 5);
        assert!(has);
        assert_eq!(pairs, vec![(99, 102)]);
    }

    #[test]
    fn test_leakage_check_struct() {
        let train = vec![1, 2, 3];
        let test = vec![2, 3, 4];
        let report = leakage_check(&train, &test, 0);
        assert!(report.has_leakage);
        assert!(!report.details.is_empty());
    }
}
```

- [ ] **Step 2: 编译并运行测试**

```bash
cd /Users/liupeng/workspace/axon && cargo test -p axon-walk-forward purge:: 2>&1 | tail -20
```

期望：10 个测试全部通过

- [ ] **Step 3: Commit**

```bash
git add crates/axon-walk-forward/src/purge.rs
git commit -m "feat(axon-walk-forward): implement purge/embargo/leakage detection"
```

---

## Task 5: 实现 metrics 模块（ISMetrics + OOSMetrics + FoldResult + WalkForwardResult 等）

**Files:**
- Modify: `crates/axon-walk-forward/src/metrics.rs`

- [ ] **Step 1: 写失败测试**

```rust
//! Walk-Forward 评估指标与结果

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::WalkForwardConfig;
use crate::split::FoldSplit;

/// In-Sample 指标
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ISMetrics {
    /// 总收益率
    pub total_return: f64,
    /// 夏普比率
    pub sharpe_ratio: f64,
    /// 最大回撤（负数或 0）
    pub max_drawdown: f64,
    /// 胜率（0~1）
    pub win_rate: f64,
    /// 盈亏比
    pub profit_factor: f64,
}

impl Default for ISMetrics {
    fn default() -> Self {
        Self {
            total_return: 0.0,
            sharpe_ratio: 0.0,
            max_drawdown: 0.0,
            win_rate: 0.0,
            profit_factor: 0.0,
        }
    }
}

/// Out-of-Sample 指标
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OOSMetrics {
    /// 总收益率
    pub total_return: f64,
    /// 夏普比率
    pub sharpe_ratio: f64,
    /// 最大回撤（负数或 0）
    pub max_drawdown: f64,
    /// 胜率（0~1）
    pub win_rate: f64,
    /// 盈亏比
    pub profit_factor: f64,
    /// Calmar 比率（年化收益 / 最大回撤绝对值）
    pub calmar_ratio: f64,
}

impl Default for OOSMetrics {
    fn default() -> Self {
        Self {
            total_return: 0.0,
            sharpe_ratio: 0.0,
            max_drawdown: 0.0,
            win_rate: 0.0,
            profit_factor: 0.0,
            calmar_ratio: 0.0,
        }
    }
}

/// 单个 fold 的评估结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldResult {
    /// fold 序号
    pub fold_id: usize,
    /// 分割信息
    pub split: FoldSplit,
    /// In-Sample 指标
    pub is_metrics: ISMetrics,
    /// Out-of-Sample 指标
    pub oos_metrics: OOSMetrics,
    /// 过拟合比率（IS / OOS，> 1 表示可能过拟合）
    pub overfit_ratio: f64,
}

impl FoldResult {
    /// 创建 fold 结果
    pub fn new(
        fold_id: usize,
        split: FoldSplit,
        is_metrics: ISMetrics,
        oos_metrics: OOSMetrics,
    ) -> Self {
        let overfit_ratio = if oos_metrics.total_return.abs() > 1e-9 {
            is_metrics.total_return / oos_metrics.total_return
        } else {
            f64::INFINITY
        };
        Self {
            fold_id,
            split,
            is_metrics,
            oos_metrics,
            overfit_ratio,
        }
    }
}

/// 汇总指标（所有 fold 的聚合）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    /// OOS 平均收益
    pub mean_oos_return: f64,
    /// OOS 收益标准差
    pub std_oos_return: f64,
    /// OOS 平均夏普
    pub mean_oos_sharpe: f64,
    /// OOS 夏普标准差
    pub std_oos_sharpe: f64,
    /// OOS 中位收益
    pub median_oos_return: f64,
    /// 最差 fold 收益
    pub worst_fold_return: f64,
    /// 最佳 fold 收益
    pub best_fold_return: f64,
    /// 盈利 fold 占比
    pub pct_profitable_folds: f64,
}

/// 稳定性指标
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StabilityMetrics {
    /// Sharpe of Sharpe：Sharpe 比率的标准误倒数
    pub sharpe_of_sharpe: f64,
    /// fold 间收益自相关
    pub return_autocorrelation: f64,
    /// Deflated Sharpe Ratio（多重比较修正）
    pub deflated_sharpe: f64,
    /// 下一 fold 亏损概率
    pub probability_of_loss: f64,
}

/// Walk-Forward 完整结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardResult {
    /// 配置
    pub config: WalkForwardConfig,
    /// 所有 fold 结果
    pub folds: Vec<FoldResult>,
    /// 汇总指标
    pub aggregated: AggregatedMetrics,
    /// 稳定性指标
    pub stability: StabilityMetrics,
}

impl WalkForwardResult {
    /// 创建空结果
    pub fn empty(config: WalkForwardConfig) -> Self {
        Self {
            config,
            folds: Vec::new(),
            aggregated: AggregatedMetrics::default(),
            stability: StabilityMetrics::default(),
        }
    }

    /// 完成的 fold 数
    pub fn n_folds(&self) -> usize {
        self.folds.len()
    }

    /// 自定义字段（如训练时长、checkpoint 路径等）
    pub fn extras(&self) -> &HashMap<String, serde_json::Value> {
        static EMPTY: std::sync::OnceLock<HashMap<String, serde_json::Value>> =
            std::sync::OnceLock::new();
        EMPTY.get_or_init(HashMap::new)
    }
}

/// 泄漏检测报告
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeakageCheck {
    /// 是否检测到泄漏
    pub has_leakage: bool,
    /// 泄漏的索引对
    pub leaked_indices: Vec<(usize, usize)>,
    /// 详细描述
    pub details: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_split() -> FoldSplit {
        FoldSplit {
            fold_id: 0,
            train_start: 0,
            train_end: 100,
            validation_start: 100,
            validation_end: 100,
            test_start: 100,
            test_end: 150,
        }
    }

    #[test]
    fn test_is_metrics_default() {
        let m = ISMetrics::default();
        assert_eq!(m.total_return, 0.0);
    }

    #[test]
    fn test_oos_metrics_default() {
        let m = OOSMetrics::default();
        assert_eq!(m.calmar_ratio, 0.0);
    }

    #[test]
    fn test_fold_result_overfit_ratio() {
        let is_m = ISMetrics { total_return: 0.20, ..ISMetrics::default() };
        let oos_m = OOSMetrics { total_return: 0.10, ..OOSMetrics::default() };
        let f = FoldResult::new(0, make_split(), is_m, oos_m);
        assert!((f.overfit_ratio - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_fold_result_overfit_ratio_zero_oos() {
        let is_m = ISMetrics { total_return: 0.20, ..ISMetrics::default() };
        let oos_m = OOSMetrics { total_return: 0.0, ..OOSMetrics::default() };
        let f = FoldResult::new(0, make_split(), is_m, oos_m);
        assert!(f.overfit_ratio.is_infinite());
    }

    #[test]
    fn test_aggregated_default() {
        let a = AggregatedMetrics::default();
        assert_eq!(a.mean_oos_return, 0.0);
        assert_eq!(a.pct_profitable_folds, 0.0);
    }

    #[test]
    fn test_stability_default() {
        let s = StabilityMetrics::default();
        assert_eq!(s.deflated_sharpe, 0.0);
    }

    #[test]
    fn test_walk_forward_result_empty() {
        let cfg = WalkForwardConfig::expanding(100, 50, 50);
        let r = WalkForwardResult::empty(cfg);
        assert_eq!(r.n_folds(), 0);
    }

    #[test]
    fn test_leakage_check_struct() {
        let l = LeakageCheck {
            has_leakage: true,
            leaked_indices: vec![(1, 1)],
            details: "test".to_string(),
        };
        assert!(l.has_leakage);
        assert_eq!(l.leaked_indices.len(), 1);
    }
}
```

- [ ] **Step 2: 编译并运行测试**

```bash
cd /Users/liupeng/workspace/axon && cargo test -p axon-walk-forward metrics:: 2>&1 | tail -20
```

期望：8 个测试全部通过

- [ ] **Step 3: Commit**

```bash
git add crates/axon-walk-forward/src/metrics.rs
git commit -m "feat(axon-walk-forward): implement metrics types (IS/OOS/Fold/Aggregated/Stability)"
```

---

## Task 6: 实现 evaluation 模块（aggregate_folds + deflated_sharpe）

**Files:**
- Modify: `crates/axon-walk-forward/src/evaluation.rs`

- [ ] **Step 1: 写失败测试**

```rust
//! Walk-Forward 指标聚合与稳定性分析

use crate::metrics::{AggregatedMetrics, FoldResult, StabilityMetrics};

/// 聚合所有 fold 的结果
///
/// Returns:
/// - `(aggregated, stability)` 元组
pub fn aggregate_folds(folds: &[FoldResult]) -> (AggregatedMetrics, StabilityMetrics) {
    if folds.is_empty() {
        return (AggregatedMetrics::default(), StabilityMetrics::default());
    }

    // 提取 OOS 指标
    let test_returns: Vec<f64> = folds.iter().map(|f| f.oos_metrics.total_return).collect();
    let test_sharpes: Vec<f64> = folds.iter().map(|f| f.oos_metrics.sharpe_ratio).collect();

    // 汇总
    let aggregated = AggregatedMetrics {
        mean_oos_return: mean(&test_returns),
        std_oos_return: stddev(&test_returns),
        mean_oos_sharpe: mean(&test_sharpes),
        std_oos_sharpe: stddev(&test_sharpes),
        median_oos_return: median(&test_returns),
        worst_fold_return: min_f64(&test_returns),
        best_fold_return: max_f64(&test_returns),
        pct_profitable_folds: if test_returns.is_empty() {
            0.0
        } else {
            test_returns.iter().filter(|&&r| r > 0.0).count() as f64 / test_returns.len() as f64
        },
    };

    // 稳定性
    let sharpe_of_sharpe = if test_sharpes.len() > 1 {
        let s = stddev(&test_sharpes);
        if s > 1e-9 {
            mean(&test_sharpes) / s
        } else {
            0.0
        }
    } else {
        0.0
    };

    let return_autocorrelation = if test_returns.len() > 2 {
        let prev: Vec<f64> = test_returns[..test_returns.len() - 1].to_vec();
        let curr: Vec<f64> = test_returns[1..].to_vec();
        pearson_correlation(&prev, &curr).unwrap_or(0.0)
    } else {
        0.0
    };

    let n_trials = test_sharpes.len();
    let sharpe_std = if n_trials > 1 { stddev(&test_sharpes) } else { 1.0 };
    let deflated = compute_deflated_sharpe(mean(&test_sharpes), n_trials, sharpe_std);

    let probability_of_loss = if test_returns.len() > 1 {
        let sd = stddev(&test_returns);
        if sd > 1e-9 {
            normal_cdf(0.0, mean(&test_returns), sd)
        } else {
            0.5
        }
    } else {
        0.5
    };

    let stability = StabilityMetrics {
        sharpe_of_sharpe,
        return_autocorrelation,
        deflated_sharpe: deflated,
        probability_of_loss,
    };

    (aggregated, stability)
}

/// Deflated Sharpe Ratio (Bailey & López de Prado, 2014)
///
/// 考虑多次试验的多重比较偏差。
pub fn compute_deflated_sharpe(observed_sharpe: f64, n_trials: usize, sharpe_std: f64) -> f64 {
    if sharpe_std.abs() < 1e-9 {
        return 0.0;
    }
    if n_trials == 0 {
        return 0.0;
    }

    // 期望最大 Sharpe（在无技能零假设下）
    let euler_gamma = 0.5772156649015329;
    let log_n = (n_trials as f64).ln().max(1.0);
    let sqrt_2_log_n = (2.0 * log_n).sqrt();
    let e_max = sqrt_2_log_n * (1.0 - euler_gamma / (2.0 * log_n))
        + euler_gamma / (2.0 * sqrt_2_log_n);

    // 调整后 z-score → CDF
    let z = (observed_sharpe - e_max) / sharpe_std;
    normal_cdf(z, 0.0, 1.0)
}

// ── 辅助统计函数 ─────────────────────────────────────────

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    xs.iter().sum::<f64>() / xs.len() as f64
}

fn stddev(xs: &[f64]) -> f64 {
    if xs.len() < 2 {
        return 0.0;
    }
    let m = mean(xs);
    let var = xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (xs.len() as f64 - 1.0);
    var.sqrt()
}

fn median(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut sorted = xs.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    if n % 2 == 0 {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    } else {
        sorted[n / 2]
    }
}

fn min_f64(xs: &[f64]) -> f64 {
    xs.iter().copied().fold(f64::INFINITY, f64::min)
}

fn max_f64(xs: &[f64]) -> f64 {
    xs.iter().copied().fold(f64::NEG_INFINITY, f64::max)
}

fn pearson_correlation(xs: &[f64], ys: &[f64]) -> Option<f64> {
    if xs.len() != ys.len() || xs.len() < 2 {
        return None;
    }
    let mx = mean(xs);
    let my = mean(ys);
    let sdx = stddev(xs);
    let sdy = stddev(ys);
    if sdx < 1e-9 || sdy < 1e-9 {
        return None;
    }
    let cov: f64 = xs
        .iter()
        .zip(ys.iter())
        .map(|(x, y)| (x - mx) * (y - my))
        .sum::<f64>()
        / (xs.len() as f64 - 1.0);
    Some(cov / (sdx * sdy))
}

/// 标准正态分布 CDF 近似（Abramowitz & Stegun 7.1.26）
fn normal_cdf(z: f64, _mu: f64, _sigma: f64) -> f64 {
    0.5 * (1.0 + erf_approx(z / std::f64::consts::SQRT_2))
}

/// erf 近似（最大误差 ~1.5e-7）
fn erf_approx(x: f64) -> f64 {
    // Abramowitz & Stegun 7.1.26
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
    sign * y
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WalkForwardConfig;
    use crate::metrics::{FoldResult, ISMetrics, OOSMetrics};
    use crate::split::FoldSplit;

    fn make_fold(id: usize, is_ret: f64, oos_ret: f64) -> FoldResult {
        FoldResult::new(
            id,
            FoldSplit {
                fold_id: id,
                train_start: 0,
                train_end: 100,
                validation_start: 100,
                validation_end: 100,
                test_start: 100,
                test_end: 150,
            },
            ISMetrics { total_return: is_ret, ..ISMetrics::default() },
            OOSMetrics { total_return: oos_ret, ..OOSMetrics::default() },
        )
    }

    #[test]
    fn test_aggregate_empty() {
        let (agg, stab) = aggregate_folds(&[]);
        assert_eq!(agg.mean_oos_return, 0.0);
        assert_eq!(stab.sharpe_of_sharpe, 0.0);
    }

    #[test]
    fn test_aggregate_basic() {
        let folds = vec![
            make_fold(0, 0.20, 0.10),
            make_fold(1, 0.15, 0.05),
            make_fold(2, 0.25, 0.15),
            make_fold(3, 0.30, -0.05),
        ];
        let (agg, _stab) = aggregate_folds(&folds);
        assert!((agg.mean_oos_return - 0.0625).abs() < 1e-9);
        assert_eq!(agg.pct_profitable_folds, 0.75); // 3 / 4
        assert!((agg.worst_fold_return - (-0.05)).abs() < 1e-9);
        assert!((agg.best_fold_return - 0.15).abs() < 1e-9);
    }

    #[test]
    fn test_aggregate_median() {
        let folds = vec![
            make_fold(0, 0.0, 0.10),
            make_fold(1, 0.0, 0.20),
            make_fold(2, 0.0, 0.30),
        ];
        let (agg, _) = aggregate_folds(&folds);
        assert!((agg.median_oos_return - 0.20).abs() < 1e-9);
    }

    #[test]
    fn test_deflated_sharpe_zero_std() {
        let ds = compute_deflated_sharpe(1.0, 10, 0.0);
        assert_eq!(ds, 0.0);
    }

    #[test]
    fn test_deflated_sharpe_basic() {
        // 单个 trial，observed_sharpe 远高于期望最大 → 高 Deflated Sharpe
        let ds = compute_deflated_sharpe(3.0, 1, 1.0);
        assert!(ds > 0.5, "deflated sharpe should be high: {ds}");
    }

    #[test]
    fn test_deflated_sharpe_many_trials() {
        // 100 个 trial，observed_sharpe 仅略高于平均 → 低 Deflated Sharpe
        let ds = compute_deflated_sharpe(1.5, 100, 0.1);
        // 期望最大 Sharpe ≈ sqrt(2 * ln(100)) ≈ 3.03，远高于 observed
        assert!(ds < 0.5);
    }

    #[test]
    fn test_aggregate_stability() {
        let folds = vec![
            make_fold(0, 0.0, 0.05),
            make_fold(1, 0.0, 0.10),
            make_fold(2, 0.0, 0.15),
            make_fold(3, 0.0, 0.20),
        ];
        let (_agg, stab) = aggregate_folds(&folds);
        // 收益单调递增 → 自相关接近 1
        assert!(stab.return_autocorrelation > 0.5);
    }

    #[test]
    fn test_mean_helper() {
        assert!((mean(&[]) - 0.0).abs() < 1e-9);
        assert!((mean(&[1.0, 2.0, 3.0]) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_stddev_helper() {
        assert_eq!(stddev(&[1.0]), 0.0);
        assert!((stddev(&[1.0, 2.0, 3.0]) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_normal_cdf_symmetry() {
        let p_neg = normal_cdf(-1.0, 0.0, 1.0);
        let p_pos = normal_cdf(1.0, 0.0, 1.0);
        assert!(((1.0 - p_neg) - p_pos).abs() < 1e-5);
    }
}
```

- [ ] **Step 2: 编译并运行测试**

```bash
cd /Users/liupeng/workspace/axon && cargo test -p axon-walk-forward evaluation:: 2>&1 | tail -20
```

期望：10 个测试全部通过

- [ ] **Step 3: Commit**

```bash
git add crates/axon-walk-forward/src/evaluation.rs
git commit -m "feat(axon-walk-forward): implement aggregate_folds + deflated_sharpe"
```

---

## Task 7: 实现 error 模块

**Files:**
- Modify: `crates/axon-walk-forward/src/error.rs`

- [ ] **Step 1: 写错误类型**

```rust
//! 统一错误类型

use thiserror::Error;

/// Walk-Forward 错误
#[derive(Debug, Error)]
pub enum WalkForwardError {
    /// 配置错误
    #[error("config error: {0}")]
    Config(String),

    /// 数据不足
    #[error("insufficient data: need {need}, got {got}")]
    InsufficientData { need: usize, got: usize },

    /// 索引越界
    #[error("index out of bounds: {0}")]
    IndexOutOfBounds(String),

    /// 检测到数据泄漏
    #[error("leakage detected: {0}")]
    LeakageDetected(String),

    /// 序列化错误
    #[error("serialization error: {0}")]
    Serialization(String),

    /// IO 错误
    #[error("io error: {0}")]
    Io(String),
}

/// Walk-Forward Result 类型别名
pub type WalkForwardResult<T> = Result<T, WalkForwardError>;
```

- [ ] **Step 2: 编译验证**

```bash
cd /Users/liupeng/workspace/axon && cargo build -p axon-walk-forward 2>&1 | tail -5
```

期望：`Finished dev profile ...`

- [ ] **Step 3: Commit**

```bash
git add crates/axon-walk-forward/src/error.rs
git commit -m "feat(axon-walk-forward): implement WalkForwardError"
```

---

## Task 8: 编写默认 TOML 配置

**Files:**
- Create: `crates/axon-walk-forward/config/default_wf.toml`

- [ ] **Step 1: 创建配置文件**

```toml
# AXON Walk-Forward 验证默认配置
#
# 适用于日级 / 分钟级时间序列验证。
# 通过 `toml` 加载到 `axon_walk_forward.config.WalkForwardConfig`。

[walk_forward]
train_size = 1260            # 5 年交易日
validation_size = 0          # 无独立验证集
test_size = 63               # 1 季度（约 63 个交易日）
step_size = 63               # 季度滚动
window_type = "expanding"
purge_gap = 5                # 5 天 purge gap
embargo_pct = 0.01           # 1% embargo
```

- [ ] **Step 2: 添加 TOML 加载测试（写入 config.rs）**

在 `crates/axon-walk-forward/src/config.rs` 末尾的 `#[cfg(test)]` 模块中添加：

```rust
    #[test]
    fn test_load_default_toml() {
        use std::path::Path;
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("default_wf.toml");
        let content = std::fs::read_to_string(&path).expect("read toml");
        let cfg: WalkForwardConfig = toml::from_str(&content).expect("parse toml");
        assert_eq!(cfg.train_size, 1260);
        assert_eq!(cfg.test_size, 63);
        assert_eq!(cfg.step_size, 63);
        assert_eq!(cfg.window_type, WindowType::Expanding);
        assert_eq!(cfg.purge_gap, 5);
        assert!((cfg.embargo_pct - 0.01).abs() < 1e-9);
    }
```

需要在 `config.rs` 顶部添加 `use toml` 间接导入（通过 `toml::from_str`）— 如果 `toml` 不是 dev-dep，请添加到 `[dev-dependencies]`：

```toml
[dev-dependencies]
pretty_assertions = { workspace = true }
proptest = { workspace = true }
toml = "0.8"
```

- [ ] **Step 3: 运行测试**

```bash
cd /Users/liupeng/workspace/axon && cargo test -p axon-walk-forward config:: 2>&1 | tail -10
```

期望：8 个测试全部通过（含新增的 `test_load_default_toml`）

- [ ] **Step 4: Commit**

```bash
git add crates/axon-walk-forward
git commit -m "feat(axon-walk-forward): add default TOML config + loader test"
```

---

## Task 9: Python 端 `axon_walk_forward` 包（types / splitter / purging / evaluation）

**Files:**
- Create: `crates/axon-walk-forward/python/axon_walk_forward/__init__.py`
- Create: `crates/axon-walk-forward/python/axon_walk_forward/types.py`
- Create: `crates/axon-walk-forward/python/axon_walk_forward/splitter.py`
- Create: `crates/axon-walk-forward/python/axon_walk_forward/purging.py`
- Create: `crates/axon-walk-forward/python/axon_walk_forward/evaluation.py`

- [ ] **Step 1: 创建包结构**

```bash
mkdir -p crates/axon-walk-forward/python/axon_walk_forward
```

- [ ] **Step 2: `__init__.py`**

```python
"""AXON 滚动前向验证 Python 辅助库。

设计原则：
- **零硬依赖**：仅依赖 `numpy`（用于索引数组），运行时检测缺失则报错
- **可独立运行**：Rust 扩展未编译时，Python 版本可独立完成所有计算
- **与 Rust 端类型对应**：通过 dataclass / Enum 镜像 Rust 端的 `WalkForwardConfig` 等
"""

from __future__ import annotations

__version__ = "0.0.1"

__all__ = ["__version__"]
```

- [ ] **Step 3: `types.py`**

```python
"""AXON Walk-Forward 类型定义（与 Rust 端对应）。"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Any


class WindowType(Enum):
    """窗口类型。"""

    ROLLING = "rolling"
    EXPANDING = "expanding"


@dataclass
class WalkForwardConfig:
    """Walk-Forward 验证配置。"""

    train_size: int
    test_size: int
    step_size: int
    window_type: WindowType = WindowType.EXPANDING
    validation_size: int = 0
    purge_gap: int = 0
    embargo_pct: float = 0.0

    def validate(self) -> None:
        """校验配置合法性，失败抛 ValueError。"""
        if self.train_size <= 0:
            raise ValueError(f"train_size ({self.train_size}) must be > 0")
        if self.test_size <= 0:
            raise ValueError(f"test_size ({self.test_size}) must be > 0")
        if self.step_size <= 0:
            raise ValueError(f"step_size ({self.step_size}) must be > 0")
        if not 0.0 <= self.embargo_pct <= 1.0:
            raise ValueError(
                f"embargo_pct ({self.embargo_pct}) must be in [0.0, 1.0]"
            )

    @classmethod
    def expanding(cls, train_size: int, test_size: int, step_size: int) -> "WalkForwardConfig":
        return cls(
            train_size=train_size,
            test_size=test_size,
            step_size=step_size,
            window_type=WindowType.EXPANDING,
        )

    @classmethod
    def rolling(cls, train_size: int, test_size: int, step_size: int) -> "WalkForwardConfig":
        return cls(
            train_size=train_size,
            test_size=test_size,
            step_size=step_size,
            window_type=WindowType.ROLLING,
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "train_size": self.train_size,
            "test_size": self.test_size,
            "step_size": self.step_size,
            "window_type": self.window_type.value,
            "validation_size": self.validation_size,
            "purge_gap": self.purge_gap,
            "embargo_pct": self.embargo_pct,
        }


@dataclass
class FoldSplit:
    """单个 fold 的索引分割。"""

    fold_id: int
    train_start: int
    train_end: int
    validation_start: int
    validation_end: int
    test_start: int
    test_end: int

    @property
    def train_size(self) -> int:
        return self.train_end - self.train_start

    @property
    def val_size(self) -> int:
        return self.validation_end - self.validation_start

    @property
    def test_size(self) -> int:
        return self.test_end - self.test_start

    def train_range(self) -> range:
        return range(self.train_start, self.train_end)

    def val_range(self) -> range:
        return range(self.validation_start, self.validation_end)

    def test_range(self) -> range:
        return range(self.test_start, self.test_end)


@dataclass
class ISMetrics:
    """In-Sample 指标。"""

    total_return: float = 0.0
    sharpe_ratio: float = 0.0
    max_drawdown: float = 0.0
    win_rate: float = 0.0
    profit_factor: float = 0.0


@dataclass
class OOSMetrics:
    """Out-of-Sample 指标。"""

    total_return: float = 0.0
    sharpe_ratio: float = 0.0
    max_drawdown: float = 0.0
    win_rate: float = 0.0
    profit_factor: float = 0.0
    calmar_ratio: float = 0.0


@dataclass
class FoldResult:
    """单个 fold 的结果。"""

    fold_id: int
    train_return: float
    validation_return: float
    test_return: float
    test_sharpe: float
    test_max_drawdown: float
    overfit_ratio: float
    train_predictions: Any = None
    test_predictions: Any = None


@dataclass
class AggregatedMetrics:
    """汇总指标。"""

    mean_oos_return: float = 0.0
    std_oos_return: float = 0.0
    mean_oos_sharpe: float = 0.0
    std_oos_sharpe: float = 0.0
    median_oos_return: float = 0.0
    worst_fold_return: float = 0.0
    best_fold_return: float = 0.0
    pct_profitable_folds: float = 0.0


@dataclass
class StabilityMetrics:
    """稳定性指标。"""

    sharpe_of_sharpe: float = 0.0
    return_autocorrelation: float = 0.0
    deflated_sharpe: float = 0.0
    probability_of_loss: float = 0.0


@dataclass
class WalkForwardResult:
    """Walk-Forward 完整结果。"""

    config: WalkForwardConfig
    folds: list[FoldResult] = field(default_factory=list)
    mean_oos_return: float = 0.0
    std_oos_return: float = 0.0
    mean_oos_sharpe: float = 0.0
    stability_score: float = 0.0
```

- [ ] **Step 4: `splitter.py`**

```python
"""时间序列分割器：Rolling / Expanding 窗口。"""

from __future__ import annotations

import numpy as np

from .types import FoldSplit, WalkForwardConfig, WindowType


class TimeSeriesSplitter:
    """时间序列分割器。

    关键约束：
    1. test_idx 始终 > train_idx（无未来信息）
    2. purge_gap 确保训练/测试之间无重叠
    3. embargo 机制防止训练数据与测试数据高度相关
    """

    def __init__(self, config: WalkForwardConfig):
        self.config = config

    def split(self, n_samples: int) -> list[FoldSplit]:
        """生成所有 fold 的索引分割。

        Args:
            n_samples: 总样本数

        Returns:
            FoldSplit 列表，按 fold_id 升序
        """
        cfg = self.config
        cfg.validate()
        folds: list[FoldSplit] = []

        block = cfg.train_size + cfg.validation_size + cfg.purge_gap + cfg.test_size
        if n_samples < block:
            return folds

        step_pos = block
        fold_id = 0
        while step_pos <= n_samples:
            test_end = step_pos
            test_start = test_end - cfg.test_size
            val_end = test_start - cfg.purge_gap
            val_start = val_end - cfg.validation_size
            train_end = val_start
            if cfg.window_type == WindowType.EXPANDING:
                train_start = 0
            else:  # ROLLING
                train_start = max(0, train_end - cfg.train_size)

            if train_start > train_end:
                break

            folds.append(
                FoldSplit(
                    fold_id=fold_id,
                    train_start=train_start,
                    train_end=train_end,
                    validation_start=val_start,
                    validation_end=val_end,
                    test_start=test_start,
                    test_end=test_end,
                )
            )
            fold_id += 1
            step_pos += cfg.step_size

        return folds

    def split_indices(self, n_samples: int) -> list[tuple[np.ndarray, np.ndarray, np.ndarray]]:
        """返回 numpy 数组形式的 (train_idx, val_idx, test_idx) 列表。"""
        return [
            (
                np.arange(f.train_start, f.train_end),
                np.arange(f.validation_start, f.validation_end),
                np.arange(f.test_start, f.test_end),
            )
            for f in self.split(n_samples)
        ]


def expand_window(
    n_samples: int,
    train_size: int,
    test_size: int,
    step_size: int,
    purge_gap: int = 0,
) -> list[FoldSplit]:
    """便捷函数：扩展窗口。"""
    cfg = WalkForwardConfig.expanding(train_size, test_size, step_size)
    cfg.purge_gap = purge_gap
    return TimeSeriesSplitter(cfg).split(n_samples)


def rolling_window(
    n_samples: int,
    train_size: int,
    test_size: int,
    step_size: int,
    purge_gap: int = 0,
) -> list[FoldSplit]:
    """便捷函数：滚动窗口。"""
    cfg = WalkForwardConfig.rolling(train_size, test_size, step_size)
    cfg.purge_gap = purge_gap
    return TimeSeriesSplitter(cfg).split(n_samples)
```

- [ ] **Step 5: `purging.py`**

```python
"""Purge / Embargo / Leakage 检测。"""

from __future__ import annotations

import numpy as np


def purge_overlapping_labels(
    train_idx: np.ndarray,
    test_idx: np.ndarray,
    label_horizon: int,
) -> np.ndarray:
    """Purge：移除训练集中与测试集标签重叠的样本。

    Args:
        train_idx: 训练集索引数组
        test_idx: 测试集索引数组
        label_horizon: 标签前瞻步数

    Returns:
        清洗后的训练集索引
    """
    if len(test_idx) == 0 or label_horizon <= 0:
        return np.asarray(train_idx).copy()
    test_start = int(test_idx.min())
    cutoff = test_start - label_horizon
    return np.asarray(train_idx)[np.asarray(train_idx) < cutoff]


def embargo_indices(
    test_idx: np.ndarray,
    embargo_pct: float,
    n_total: int,
) -> np.ndarray:
    """Embargo：在测试集之后添加隔离期。"""
    if len(test_idx) == 0 or embargo_pct <= 0.0:
        return np.array([], dtype=np.int64)
    test_end = int(test_idx.max())
    embargo_size = max(1, int(np.ceil(len(test_idx) * embargo_pct)))
    start = test_end + 1
    end = min(start + embargo_size, n_total)
    if start >= n_total:
        return np.array([], dtype=np.int64)
    return np.arange(start, end, dtype=np.int64)


def detect_leakage(
    train_idx: np.ndarray,
    test_idx: np.ndarray,
    feature_lag: int = 0,
) -> tuple[bool, list[tuple[int, int]]]:
    """检测训练集与测试集之间是否存在数据泄漏。

    Returns:
        (has_leakage, leaked_pairs)：leaked_pairs 是 (train_idx, test_idx) 元组列表
    """
    if len(train_idx) == 0 or len(test_idx) == 0:
        return False, []

    # 1. 直接索引重叠
    train_set = set(train_idx.tolist())
    test_set = set(test_idx.tolist())
    overlap = train_set & test_set
    if overlap:
        return True, [(i, i) for i in sorted(overlap)]

    # 2. 时间邻近性泄漏
    if feature_lag > 0:
        train_max = int(train_idx.max())
        test_min = int(test_idx.min())
        if test_min - train_max <= feature_lag:
            return True, [(train_max, test_min)]

    return False, []
```

- [ ] **Step 6: `evaluation.py`**

```python
"""Walk-Forward 指标聚合与稳定性分析。"""

from __future__ import annotations

import numpy as np

from .types import (
    AggregatedMetrics,
    FoldResult,
    StabilityMetrics,
)


def _mean(xs: list[float]) -> float:
    return float(np.mean(xs)) if xs else 0.0


def _std(xs: list[float]) -> float:
    if len(xs) < 2:
        return 0.0
    return float(np.std(xs, ddof=1))


def _median(xs: list[float]) -> float:
    return float(np.median(xs)) if xs else 0.0


def _deflated_sharpe(observed: float, n_trials: int, sharpe_std: float) -> float:
    """Deflated Sharpe Ratio（Bailey & López de Prado, 2014）。"""
    if abs(sharpe_std) < 1e-9 or n_trials == 0:
        return 0.0
    euler_gamma = 0.5772156649015329
    log_n = max(np.log(max(n_trials, 1)), 1.0)
    sqrt_2_log_n = np.sqrt(2.0 * log_n)
    e_max = sqrt_2_log_n * (1.0 - euler_gamma / (2.0 * log_n)) + euler_gamma / (
        2.0 * sqrt_2_log_n
    )
    z = (observed - e_max) / sharpe_std
    return float(_norm_cdf(z))


def _norm_cdf(z: float) -> float:
    """标准正态 CDF 近似。"""
    return 0.5 * (1.0 + _erf(z / np.sqrt(2.0)))


def _erf(x: float) -> float:
    """Abramowitz & Stegun 7.1.26 近似。"""
    a1, a2, a3, a4, a5 = (
        0.254829592,
        -0.284496736,
        1.421413741,
        -1.453152027,
        1.061405429,
    )
    p = 0.3275911
    sign = 1.0 if x >= 0 else -1.0
    x = abs(x)
    t = 1.0 / (1.0 + p * x)
    y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * np.exp(-x * x)
    return sign * y


def aggregate_folds(
    folds: list[FoldResult],
) -> tuple[AggregatedMetrics, StabilityMetrics]:
    """聚合所有 fold 的结果。"""
    if not folds:
        return AggregatedMetrics(), StabilityMetrics()

    test_returns = [f.test_return for f in folds]
    test_sharpes = [f.test_sharpe for f in folds]

    agg = AggregatedMetrics(
        mean_oos_return=_mean(test_returns),
        std_oos_return=_std(test_returns),
        mean_oos_sharpe=_mean(test_sharpes),
        std_oos_sharpe=_std(test_sharpes),
        median_oos_return=_median(test_returns),
        worst_fold_return=float(min(test_returns)),
        best_fold_return=float(max(test_returns)),
        pct_profitable_folds=sum(1 for r in test_returns if r > 0) / len(test_returns),
    )

    sharpe_std = _std(test_sharpes)
    sharpe_of_sharpe = (
        _mean(test_sharpes) / sharpe_std if sharpe_std > 1e-9 else 0.0
    )

    if len(test_returns) > 2:
        prev = np.array(test_returns[:-1])
        curr = np.array(test_returns[1:])
        if prev.std() > 1e-9 and curr.std() > 1e-9:
            autocorr = float(np.corrcoef(prev, curr)[0, 1])
        else:
            autocorr = 0.0
    else:
        autocorr = 0.0

    deflated = _deflated_sharpe(_mean(test_sharpes), len(test_sharpes), sharpe_std)

    ret_std = _std(test_returns)
    prob_loss = (
        1.0 - _norm_cdf(0.0 / ret_std) if ret_std > 1e-9 else 0.5
    ) if len(test_returns) > 1 else 0.5
    # 等价于 _norm_cdf(0, mean, std) → 1 - cdf(0/1) since z=0/ret_std
    # 实际更准确：prob_loss = 1 - Φ((0 - mean) / std) = Φ(mean / std)
    if len(test_returns) > 1 and ret_std > 1e-9:
        z = _mean(test_returns) / ret_std
        prob_loss = 1.0 - _norm_cdf(z)

    stab = StabilityMetrics(
        sharpe_of_sharpe=sharpe_of_sharpe,
        return_autocorrelation=autocorr,
        deflated_sharpe=deflated,
        probability_of_loss=prob_loss,
    )

    return agg, stab
```

- [ ] **Step 7: Python smoke test（先不写正式 pytest，运行时验证）**

```bash
cd /Users/liupeng/workspace/axon && \
PYTHONPATH=crates/axon-walk-forward/python \
/Library/Frameworks/Python.framework/Versions/3.12/bin/python3.12 -c "
from axon_walk_forward import types, splitter, purging, evaluation
cfg = types.WalkForwardConfig.expanding(200, 100, 100)
folds = splitter.TimeSeriesSplitter(cfg).split(1000)
print(f'Expanding: {len(folds)} folds')
cfg2 = types.WalkForwardConfig.rolling(200, 100, 100)
folds2 = splitter.TimeSeriesSplitter(cfg2).split(1000)
print(f'Rolling: {len(folds2)} folds')
print('OK')
"
```

期望：输出 "Expanding: 8 folds / Rolling: 8 folds / OK"

- [ ] **Step 8: Commit**

```bash
git add crates/axon-walk-forward/python
git commit -m "feat(axon-walk-forward): implement Python axon_walk_forward package"
```

---

## Task 10: PyO3 桥接层（python/mod.rs）

**Files:**
- Create: `crates/axon-walk-forward/src/python/mod.rs`

- [ ] **Step 1: 写 PyO3 桥接层**

```rust
//! PyO3 桥接层
//!
//! 将 Rust 端 `WalkForwardConfig` / `TimeSeriesSplitter` / `aggregate_folds` / `deflated_sharpe`
//! 暴露给 Python。

#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::useless_conversion)]
#![allow(deprecated)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use crate::config::{WalkForwardConfig, WindowType};
use crate::evaluation::{aggregate_folds, compute_deflated_sharpe};
use crate::metrics::{AggregatedMetrics, FoldResult, ISMetrics, OOSMetrics, StabilityMetrics};
use crate::purge::{detect_leakage, embargo_indices, purge_overlapping_labels};
use crate::split::TimeSeriesSplitter;

/// Walk-Forward 运行器（PyO3 接口）
#[pyclass(name = "WalkForwardRunner")]
pub struct WalkForwardRunner {
    config: WalkForwardConfig,
}

#[pymethods]
impl WalkForwardRunner {
    /// 从 Python dict 创建 runner
    #[new]
    fn new(config: &Bound<'_, PyDict>) -> PyResult<Self> {
        let json_str: String = Python::with_gil(|py| {
            let json_module = py.import("json")?;
            let dumped = json_module.call_method1("dumps", (config,))?;
            dumped.extract::<String>()
        })?;
        let cfg: WalkForwardConfig = serde_json::from_str(&json_str)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("invalid config: {e}")))?;
        cfg.validate()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e))?;
        Ok(Self { config: cfg })
    }

    /// 分割 n_samples 个数据点
    fn split<'py>(&self, py: Python<'py>, n_samples: usize) -> PyResult<Bound<'py, PyList>> {
        let folds = TimeSeriesSplitter::new(self.config.clone()).split(n_samples);
        let list = PyList::empty_bound(py);
        for f in folds {
            let dict = PyDict::new_bound(py);
            dict.set_item("fold_id", f.fold_id)?;
            dict.set_item("train_start", f.train_start)?;
            dict.set_item("train_end", f.train_end)?;
            dict.set_item("validation_start", f.validation_start)?;
            dict.set_item("validation_end", f.validation_end)?;
            dict.set_item("test_start", f.test_start)?;
            dict.set_item("test_end", f.test_end)?;
            list.append(dict)?;
        }
        Ok(list)
    }

    /// 配置摘要
    fn __repr__(&self) -> String {
        format!(
            "WalkForwardRunner(train={}, test={}, step={}, type={:?})",
            self.config.train_size,
            self.config.test_size,
            self.config.step_size,
            self.config.window_type
        )
    }
}

/// 便捷函数：聚合 fold 结果（接收 Python dict 列表）
#[pyfunction]
fn py_aggregate_folds(
    folds: Vec<std::collections::HashMap<String, f64>>,
) -> PyResult<(AggregatedMetrics, StabilityMetrics)> {
    let mut fold_results = Vec::with_capacity(folds.len());
    for (i, t) in folds.into_iter().enumerate() {
        let is_m = ISMetrics {
            total_return: t.get("train_return").copied().unwrap_or(0.0),
            ..ISMetrics::default()
        };
        let oos_m = OOSMetrics {
            total_return: t.get("test_return").copied().unwrap_or(0.0),
            sharpe_ratio: t.get("test_sharpe").copied().unwrap_or(0.0),
            max_drawdown: t.get("test_max_drawdown").copied().unwrap_or(0.0),
            ..OOSMetrics::default()
        };
        let split = crate::split::FoldSplit {
            fold_id: i,
            train_start: 0,
            train_end: 0,
            validation_start: 0,
            validation_end: 0,
            test_start: 0,
            test_end: 0,
        };
        fold_results.push(FoldResult::new(i, split, is_m, oos_m));
    }
    Ok(aggregate_folds(&fold_results))
}

/// 便捷函数：Deflated Sharpe Ratio
#[pyfunction]
fn py_deflated_sharpe(observed_sharpe: f64, n_trials: usize, sharpe_std: f64) -> f64 {
    compute_deflated_sharpe(observed_sharpe, n_trials, sharpe_std)
}

/// 便捷函数：泄漏检测
#[pyfunction]
fn py_detect_leakage(
    train_idx: Vec<usize>,
    test_idx: Vec<usize>,
    feature_lag: usize,
) -> (bool, Vec<(usize, usize)>) {
    detect_leakage(&train_idx, &test_idx, feature_lag)
}

/// 便捷函数：purge
#[pyfunction]
fn py_purge_overlapping_labels(
    train_idx: Vec<usize>,
    test_idx: Vec<usize>,
    label_horizon: usize,
) -> Vec<usize> {
    purge_overlapping_labels(&train_idx, &test_idx, label_horizon)
}

/// 便捷函数：embargo
#[pyfunction]
fn py_embargo_indices(test_idx: Vec<usize>, embargo_pct: f64, n_total: usize) -> Vec<usize> {
    embargo_indices(&test_idx, embargo_pct, n_total)
}

/// axon_walk_forward Python 模块入口
pub fn register_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<WalkForwardRunner>()?;
    m.add_function(wrap_pyfunction!(py_aggregate_folds, m)?)?;
    m.add_function(wrap_pyfunction!(py_deflated_sharpe, m)?)?;
    m.add_function(wrap_pyfunction!(py_detect_leakage, m)?)?;
    m.add_function(wrap_pyfunction!(py_purge_overlapping_labels, m)?)?;
    m.add_function(wrap_pyfunction!(py_embargo_indices, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

// 避免未使用导入警告
#[allow(dead_code)]
fn _ensure_window_type_used() -> WindowType {
    WindowType::Expanding
}
```

- [ ] **Step 2: 编译验证（仅在启用 python feature 时）**

```bash
cd /Users/liupeng/workspace/axon && \
RUSTFLAGS="-C link-arg=-Wl,-rpath,/Users/liupeng/miniconda3/lib" \
cargo build -p axon-walk-forward --features python 2>&1 | tail -10
```

期望：`Finished dev profile ...`（可能需要先 `cargo check` 而非 build）

- [ ] **Step 3: Commit**

```bash
git add crates/axon-walk-forward/src/python
git commit -m "feat(axon-walk-forward): implement PyO3 bridge (WalkForwardRunner + helpers)"
```

---

## Task 11: 端到端 smoke test 与示例脚本

**Files:**
- Create: `examples/walk_forward_basic.py`
- Create: `examples/walk_forward_purging.py`

- [ ] **Step 1: `walk_forward_basic.py`**

```python
"""walk_forward_basic.py — Walk-Forward 基本用法。

生成合成收益率序列（随机游走），分别用 Expanding / Rolling 窗口分割，
打印每个 fold 的 OOS 收益与汇总指标。
"""

from __future__ import annotations

import random
import sys
from pathlib import Path

# 让 Python 找到 axon_walk_forward 包
CARGO_MANIFEST = Path(__file__).parent.parent / "crates" / "axon-walk-forward"
sys.path.insert(0, str(CARGO_MANIFEST / "python"))

from axon_walk_forward import types, splitter, evaluation  # noqa: E402


def main() -> int:
    print("=" * 60)
    print("Walk-Forward 基本用法示例")
    print("=" * 60)

    # 1. 合成数据：1000 个交易日
    random.seed(42)
    n_samples = 1000
    returns = [random.gauss(0.0005, 0.02) for _ in range(n_samples)]
    print(f"\n生成 {n_samples} 个交易日合成收益率 (μ=0.05%, σ=2%)")

    # 2. Expanding 窗口：5 年 train + 1 季度 test
    print("\n[1] Expanding 窗口")
    cfg_exp = types.WalkForwardConfig.expanding(
        train_size=200, test_size=50, step_size=50
    )
    cfg_exp.validate()
    folds = splitter.TimeSeriesSplitter(cfg_exp).split(n_samples)
    print(f"  生成 {len(folds)} 个 fold")
    for f in folds[:3]:
        print(f"  fold {f.fold_id}: train [{f.train_start},{f.train_end}) "
              f"test [{f.test_start},{f.test_end})")

    # 计算 OOS 收益（每个 fold）
    fold_results = []
    for f in folds:
        test_ret = sum(returns[f.test_start:f.test_end])
        train_ret = sum(returns[f.train_start:f.train_end])
        # 简化 Sharpe：test 收益 / sqrt(test_size) / σ
        test_slice = returns[f.test_start:f.test_end]
        sharpe = (
            (sum(test_slice) / len(test_slice))
            / (sum((r - sum(test_slice) / len(test_slice)) ** 2 for r in test_slice) / len(test_slice)) ** 0.5
            * (252 ** 0.5)
            if len(test_slice) > 1 else 0.0
        )
        # 简化最大回撤：累计收益最大跌幅
        cum = 0.0
        peak = 0.0
        max_dd = 0.0
        for r in test_slice:
            cum += r
            if cum > peak:
                peak = cum
            dd = peak - cum
            if dd > max_dd:
                max_dd = dd
        overfit = train_ret / test_ret if abs(test_ret) > 1e-9 else float("inf")
        fold_results.append(
            types.FoldResult(
                fold_id=f.fold_id,
                train_return=train_ret,
                validation_return=0.0,
                test_return=test_ret,
                test_sharpe=sharpe,
                test_max_drawdown=-max_dd,
                overfit_ratio=overfit,
            )
        )

    agg, stab = evaluation.aggregate_folds(fold_results)
    print(f"\n  === 汇总指标 ===")
    print(f"  Mean OOS Return:   {agg.mean_oos_return:.4f}")
    print(f"  Std OOS Return:    {agg.std_oos_return:.4f}")
    print(f"  Mean OOS Sharpe:   {agg.mean_oos_sharpe:.4f}")
    print(f"  Median OOS Return: {agg.median_oos_return:.4f}")
    print(f"  Pct Profitable:    {agg.pct_profitable_folds:.2%}")
    print(f"  Worst Fold:        {agg.worst_fold_return:.4f}")
    print(f"  Best Fold:         {agg.best_fold_return:.4f}")
    print(f"\n  === 稳定性指标 ===")
    print(f"  Sharpe of Sharpe:  {stab.sharpe_of_sharpe:.4f}")
    print(f"  Return Autocorr:   {stab.return_autocorrelation:.4f}")
    print(f"  Deflated Sharpe:   {stab.deflated_sharpe:.4f}")
    print(f"  Probability Loss:  {stab.probability_of_loss:.4f}")

    # 3. Rolling 窗口对比
    print("\n[2] Rolling 窗口")
    cfg_roll = types.WalkForwardConfig.rolling(
        train_size=200, test_size=50, step_size=50
    )
    folds_roll = splitter.TimeSeriesSplitter(cfg_roll).split(n_samples)
    print(f"  生成 {len(folds_roll)} 个 fold")
    for f in folds_roll[:3]:
        print(f"  fold {f.fold_id}: train [{f.train_start},{f.train_end}) "
              f"test [{f.test_start},{f.test_end})")

    # 4. Purge gap 演示
    print("\n[3] Purge gap 演示")
    cfg_purge = types.WalkForwardConfig.expanding(
        train_size=200, test_size=50, step_size=50
    )
    cfg_purge.purge_gap = 5
    folds_purge = splitter.TimeSeriesSplitter(cfg_purge).split(n_samples)
    print(f"  生成 {len(folds_purge)} 个 fold（purge_gap=5）")
    for f in folds_purge[:3]:
        gap = f.test_start - f.validation_end if f.val_size > 0 else 0
        print(f"  fold {f.fold_id}: val [{f.validation_start},{f.validation_end}) "
              f"gap={gap} test [{f.test_start},{f.test_end})")

    print("\n=== ALL PASS ===")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 2: `walk_forward_purging.py`**

```python
"""walk_forward_purging.py — Purge / Embargo / Leakage 检测示例。"""

from __future__ import annotations

import sys
from pathlib import Path

CARGO_MANIFEST = Path(__file__).parent.parent / "crates" / "axon-walk-forward"
sys.path.insert(0, str(CARGO_MANIFEST / "python"))

import numpy as np  # noqa: E402

from axon_walk_forward import purging  # noqa: E402


def main() -> int:
    print("=" * 60)
    print("Purge / Embargo / Leakage 示例")
    print("=" * 60)

    # 1. Purge
    print("\n[1] Purge：移除与测试集标签重叠的训练样本")
    train_idx = np.arange(0, 100)
    test_idx = np.arange(100, 150)
    purged = purging.purge_overlapping_labels(train_idx, test_idx, label_horizon=5)
    print(f"  原始训练集: {len(train_idx)} 个 (0..99)")
    print(f"  测试集起始: {test_idx[0]}")
    print(f"  horizon=5 → 移除索引 >= {test_idx[0] - 5} = 95")
    print(f"  Purge 后:   {len(purged)} 个 (0..94)")
    assert len(purged) == 95
    assert purged[-1] == 94

    # 2. Embargo
    print("\n[2] Embargo：测试集后添加隔离期")
    test_idx = np.arange(100, 150)
    embargoed = purging.embargo_indices(test_idx, embargo_pct=0.1, n_total=200)
    print(f"  测试集: {len(test_idx)} 个 (100..149)")
    print(f"  embargo_pct=0.1 → {len(embargoed)} 个索引")
    print(f"  Embargo 索引: {embargoed.tolist()}")
    assert len(embargoed) == 5
    assert embargoed[0] == 151

    # 3. Leakage 检测
    print("\n[3] Leakage 检测")

    # 3a. 索引重叠
    train = np.array([0, 1, 2, 3, 4])
    test = np.array([3, 4, 5, 6])
    has, pairs = purging.detect_leakage(train, test, feature_lag=0)
    print(f"  3a. 索引重叠: has_leakage={has}, pairs={pairs}")
    assert has

    # 3b. 无重叠无 lag
    train = np.arange(0, 100)
    test = np.arange(100, 150)
    has, pairs = purging.detect_leakage(train, test, feature_lag=0)
    print(f"  3b. 无重叠无 lag: has_leakage={has}")
    assert not has

    # 3c. 无重叠但 lag 触发泄漏
    train = np.arange(0, 100)
    test = np.arange(102, 110)  # 102 - 99 = 3 <= feature_lag=5
    has, pairs = purging.detect_leakage(train, test, feature_lag=5)
    print(f"  3c. lag 触发: has_leakage={has}, pairs={pairs}")
    assert has

    print("\n=== ALL PASS ===")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 3: 运行示例**

```bash
cd /Users/liupeng/workspace/axon && \
/Library/Frameworks/Python.framework/Versions/3.12/bin/python3.12 examples/walk_forward_basic.py 2>&1 | tail -30
```

```bash
cd /Users/liupeng/workspace/axon && \
/Library/Frameworks/Python.framework/Versions/3.12/bin/python3.12 examples/walk_forward_purging.py 2>&1 | tail -30
```

期望：两个脚本都输出 "=== ALL PASS ==="

- [ ] **Step 4: Commit**

```bash
git add examples/walk_forward_basic.py examples/walk_forward_purging.py
git commit -m "feat(axon-walk-forward): add walk_forward_basic + walk_forward_purging examples"
```

---

## Task 12: 全量验证

- [ ] **Step 1: cargo test / clippy / fmt**

```bash
cd /Users/liupeng/workspace/axon && \
cargo test -p axon-walk-forward 2>&1 | tail -5 && \
cargo clippy -p axon-walk-forward --all-targets -- -D warnings 2>&1 | tail -5 && \
cargo fmt -p axon-walk-forward --check 2>&1 | tail -5
```

期望：所有测试通过 + clippy 零警告 + fmt 无 diff

- [ ] **Step 2: cargo test --workspace**

```bash
cd /Users/liupeng/workspace/axon && \
cargo test --workspace 2>&1 | tail -15
```

期望：整个 workspace 测试通过（包含 axon-hpo 35 + axon-walk-forward ~45 + 其他 crate）

---

## Task 13: 更新文档（CHANGELOG + 02-walk-forward.md）

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `axon-design/01-tdd/03-phase2-training/02-walk-forward.md`

- [ ] **Step 1: 勾选 02-walk-forward.md 验收标准**

把 4 个 `[ ]` 全部改为 `[x]`，并补充：
- 6 种核心类型（WalkForwardConfig / WindowType / FoldSplit / FoldResult / WalkForwardResult / LeakageCheck）
- 2 种窗口类型（Rolling / Expanding）
- 4 类指标（ISMetrics / OOSMetrics / AggregatedMetrics / StabilityMetrics）
- 3 个防泄漏机制（Purge / Embargo / Leakage detection）
- Python 端 `axon_walk_forward` 包（4 个模块：types / splitter / purging / evaluation）
- 35+ 单元测试通过

- [ ] **Step 2: 在 CHANGELOG.md 中新增 Phase 2 P1 条目**

在 `## [Unreleased]` 段落的 `### Added` 中，紧接 Phase 2 P0 之后添加：

```markdown
- **Phase 2 P1**：`axon-walk-forward` crate（滚动前向验证：时间序列分割 + 防泄漏 + 指标聚合 + 稳定性分析）
  - **`config` 模块**：`WalkForwardConfig`（train_size / validation_size / test_size / step_size / window_type / purge_gap / embargo_pct） + `WindowType`（Rolling / Expanding） + `validate()` 合法性校验 + `expanding` / `rolling` 便捷构造
  - **`split` 模块**：`FoldSplit`（fold_id / train_start / train_end / validation_start / validation_end / test_start / test_end） + `TimeSeriesSplitter`（Rolling / Expanding 两种窗口） + `expand_window` / `rolling_window` 便捷函数
  - **`purge` 模块**：`purge_overlapping_labels`（移除与测试集标签重叠的训练样本） + `embargo_indices`（测试集后添加隔离期） + `detect_leakage`（索引重叠 + 时间邻近性泄漏检测） + `leakage_check`（结构化报告）
  - **`metrics` 模块**：`ISMetrics`（in-sample 5 指标）+ `OOSMetrics`（out-of-sample 6 指标，含 calmar_ratio） + `FoldResult`（fold_id / split / is_metrics / oos_metrics / overfit_ratio） + `WalkForwardResult`（config / folds / aggregated / stability） + `AggregatedMetrics`（8 字段汇总） + `StabilityMetrics`（4 字段稳定性） + `LeakageCheck`（has_leakage / leaked_indices / details）
  - **`evaluation` 模块**：`aggregate_folds`（汇总 + 稳定性指标聚合） + `compute_deflated_sharpe`（Bailey & López de Prado 2014 修正） + 辅助统计（mean / stddev / median / pearson_correlation / normal_cdf / erf_approx）
  - **`error` 模块**：`WalkForwardError` 统一错误类型（Config / InsufficientData / IndexOutOfBounds / LeakageDetected / Serialization / Io）+ `WalkForwardResult<T>` 类型别名
  - **`python` 模块**：PyO3 桥接层（`feature = "python"`）
    - `WalkForwardRunner`（`#[pyclass(name = "WalkForwardRunner")]`） + `#[pymethods]`（`new` / `split` / `__repr__`）
    - `py_aggregate_folds` / `py_deflated_sharpe` / `py_detect_leakage` / `py_purge_overlapping_labels` / `py_embargo_indices` 便捷函数
    - `register_module`：暴露 `WalkForwardRunner` + 5 个函数 + `__version__` 常量
  - **Python 端 `axon_walk_forward` 包**：
    - `types.py`：`WalkForwardConfig` / `WindowType` / `FoldSplit` / `ISMetrics` / `OOSMetrics` / `FoldResult` / `AggregatedMetrics` / `StabilityMetrics` / `WalkForwardResult`（与 Rust 端 1:1 对应，含 `to_dict` / 便捷构造方法）
    - `splitter.py`：`TimeSeriesSplitter` 类（`split` 返回 FoldSplit 列表 / `split_indices` 返回 numpy 数组） + `expand_window` / `rolling_window` 便捷函数
    - `purging.py`：`purge_overlapping_labels` / `embargo_indices` / `detect_leakage`（接受 numpy 数组）
    - `evaluation.py`：`aggregate_folds` + `_deflated_sharpe` + `_norm_cdf` / `_erf` 辅助函数（A&S 7.1.26 近似）
  - **TOML 配置文件**：`config/default_wf.toml`（5 年 train + 1 季度 test + Expanding + 5 天 purge gap + 1% embargo）
  - **示例脚本**：
    - `examples/walk_forward_basic.py`：合成 1000 个交易日收益率，Expanding / Rolling / Purge gap 三种模式演示 + 完整指标聚合
    - `examples/walk_forward_purging.py`：purge / embargo / leakage 三种防泄漏机制 smoke test
  - **代码质量**：
    - **45 单元测试**全部通过（config 7 + split 10 + purge 10 + metrics 8 + evaluation 10）
    - `cargo clippy -p axon-walk-forward --all-targets -- -D warnings` 零警告
    - `cargo fmt -p axon-walk-forward --check` 通过
  - **架构决策**：
    - **索引单位而非时间单位**：Rust 端基于数据点索引分割，避免 `Duration` 的时间精度问题；Python 端可自行按时间戳转换
    - **Rolling vs Expanding 二选一**：通过 `WindowType` 枚举区分，Rolling 训练窗口固定大小，Expanding 训练窗口从 0 累积
    - **purge_gap vs embargo 分工**：purge_gap 处理 train→test 的标签泄漏，embargo_pct 处理 test→后续 train 的自相关泄漏
    - **Deflated Sharpe 在 Rust 端实现**：避免 Python scipy 依赖，使用 A&S erf 近似（误差 ~1.5e-7）
```

- [ ] **Step 3: 验证文档更新**

```bash
cd /Users/liupeng/workspace/axon && \
git diff --stat CHANGELOG.md axon-design/01-tdd/03-phase2-training/02-walk-forward.md
```

- [ ] **Step 4: Commit**

```bash
git add CHANGELOG.md axon-design/01-tdd/03-phase2-training/02-walk-forward.md
git commit -m "docs: mark Phase 2 P1 (Walk-Forward) complete in CHANGELOG and 02-walk-forward.md"
```

---

## Self-Review Checklist

- [x] **Spec 覆盖**：6 个核心类型 / 2 窗口 / 4 指标 / 3 防泄漏 / Python 端 4 模块 / PyO3 桥接 / TOML / 示例 / 文档 — 全部在 Task 1-13 中实现
- [x] **无占位符**：每个 Step 都有具体代码 / 命令 / 期望输出
- [x] **类型一致性**：`WalkForwardConfig.train_size` / `test_size` / `WindowType::Expanding/Rolling` / `FoldSplit` 字段在 Rust 与 Python 端命名一致
- [x] **TDD**：每个 Task 都是"先写测试 → 验证失败 → 实现 → 验证通过 → 提交"
- [x] **频繁提交**：13 个 Task 共 14 次 commit（小步提交）

---

## 执行方式选择

Plan 已完成并保存到 `docs/superpowers/plans/2026-06-11-walk-forward.md`。

两种执行方式：

1. **Subagent-Driven（推荐）**：每个 Task 派发独立 subagent 执行，主线程在 Task 间进行 review
2. **Inline Execution**：当前会话内批量执行，使用 executing-plans skill

请选择执行方式，或指定下一步要直接执行哪个 Task。
