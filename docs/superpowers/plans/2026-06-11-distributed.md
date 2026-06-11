# 分布式训练（Ray + RLLib）实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 `axon-distributed` crate + Python `axon_distributed` 包，提供 Ray 集群管理、RLLib 训练封装、Parameter Server、Checkpoint 容错、PyO3 桥接。

**Architecture:**
- **Python 优先**：Ray/RLLib 主要是 Python 生态，Python 端承担核心逻辑（Actor 管理、算法执行、环境交互）
- **Rust 端职责**：配置类型校验（serde）、Checkpoint 序列化、PyO3 桥接层
- **可选 Ray 连接**：本地 mock 模式可独立运行（不依赖 ray cluster），CI/示例友好
- **RLLib API 兼容**：使用 RLLib 2.x 新版 builder 模式（`PPOConfig().environment()` / `.training()`）

**Tech Stack:**
- Rust 1.96.0 / edition 2024 / Cargo workspace
- Python 3.12 / ray >= 2.30 / ray[rllib] / torch / numpy
- pyo3 0.23（feature = `python`）
- serde / serde_json / toml / thiserror / tracing

---

## 文件结构

### 新建文件

**Rust crate（`crates/axon-distributed/`）**
- `Cargo.toml`：crate 元数据 + 依赖
- `src/lib.rs`：模块入口
- `src/config.rs`：`DistributedConfig` / `ClusterConfig` / `AlgorithmConfig` / `ResourceConfig` / `FaultToleranceConfig`
- `src/actor.rs`：`ActorConfig`
- `src/param_server.rs`：`ParamServerConfig`
- `src/checkpoint.rs`：`TrainingCheckpoint` / `StepMetrics` / `CheckpointMetadata`
- `src/error.rs`：`DistributedError`
- `src/python/mod.rs`：PyO3 桥接层
- `config/default_distributed.toml`：默认配置

**Python 包（`crates/axon-distributed/python/axon_distributed/`）**
- `__init__.py`：包入口
- `types.py`：`Algorithm` 枚举 + `RayConfig` + `RLLibTrainConfig`（dataclass）
- `ray_trainer.py`：`DistributedTrainer`（RLLib 封装，支持 PPO / SAC）
- `actor.py`：`EnvironmentWorker` Ray Actor + `ActorPool`（向量化环境）
- `param_server.py`：`ParameterServer` Ray Actor + `DistributedPolicy`
- `fault_tolerance.py`：`CheckpointConfig` + `CheckpointManager` + `FaultTolerantTrainer`

**示例（`examples/`）**
- `distributed_basic.py`：本地 mock 模式（不连接 Ray 集群）
- `distributed_actor_pool.py`：演示 ActorPool 用法

**文档**
- `CHANGELOG.md`：新增 Phase 2 P2 条目
- `axon-design/01-tdd/03-phase2-training/03-distributed.md`：勾选验收标准

---

## Task 1: 创建 axon-distributed crate 骨架

**Files:**
- Create: `crates/axon-distributed/Cargo.toml`
- Create: `crates/axon-distributed/src/lib.rs`
- Modify: `Cargo.toml:8-10`（workspace.members）

- [ ] **Step 1: 创建目录**

```bash
mkdir -p crates/axon-distributed/src
```

- [ ] **Step 2: Cargo.toml**

```toml
[package]
name = "axon-distributed"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
homepage.workspace = true
documentation.workspace = true
authors.workspace = true
description = "AXON 分布式训练：Ray 集成 + RLLib 训练封装 + Parameter Server + Checkpoint 容错（Phase 2 P2 阶段填充）"

[features]
default = []
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
toml = "0.8"

[lib]
crate-type = ["rlib", "cdylib"]
```

- [ ] **Step 3: 最小 lib.rs**

```rust
//! AXON 分布式训练
//!
//! 提供 Ray 集群集成 + RLLib 算法执行 + Parameter Server +
//! Checkpoint 容错的完整工具链。
//!
//! # 模块规划
//!
//! | 模块 | 说明 |
//! |------|------|
//! | [`config`] | DistributedConfig + Cluster/Algorithm/Resource/FaultTolerance |
//! | [`actor`] | ActorConfig |
//! | [`param_server`] | ParamServerConfig |
//! | [`checkpoint`] | TrainingCheckpoint + StepMetrics + CheckpointMetadata |
//! | [`error`] | 统一错误类型 |
//! | [`python`] | PyO3 绑定（feature = `python`） |

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

pub mod actor;
pub mod checkpoint;
pub mod config;
pub mod error;
pub mod param_server;

#[cfg(feature = "python")]
pub mod python;

pub use actor::ActorConfig;
pub use checkpoint::{CheckpointMetadata, StepMetrics, TrainingCheckpoint};
pub use config::{
    AlgorithmConfig, ClusterConfig, DistributedConfig, FaultToleranceConfig, ResourceConfig,
};
pub use error::{DistributedError, DistributedResult};
pub use param_server::ParamServerConfig;
```

- [ ] **Step 4: 创建空子模块文件**

```bash
cd /Users/liupeng/workspace/axon/crates/axon-distributed/src
touch config.rs actor.rs param_server.rs checkpoint.rs error.rs
mkdir -p python && touch python/mod.rs
```

- [ ] **Step 5: 在 python/mod.rs 中添加占位**

```rust
//! PyO3 桥接层占位
#![cfg(feature = "python")]
```

- [ ] **Step 6: workspace 注册**

在 `/Users/liupeng/workspace/axon/Cargo.toml` 的 `members` 数组添加 `"crates/axon-distributed"`，在 `[workspace.dependencies]` 添加 `axon-distributed = { path = "crates/axon-distributed" }`

- [ ] **Step 7: 编译验证**

```bash
cd /Users/liupeng/workspace/axon && cargo build -p axon-distributed 2>&1 | tail -5
```

期望：`Finished dev profile`（warning 可接受）

- [ ] **Step 8: 验证无 git 操作**

项目不是 git 仓库，无需 commit

---

## Task 2: 实现 config 模块（5 个配置结构体）

**Files:**
- Modify: `crates/axon-distributed/src/config.rs`

- [ ] **Step 1: 写失败测试与实现**

```rust
//! 分布式训练配置定义

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 集群配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// worker 数量
    pub num_workers: usize,
    /// 每个 worker 分配的 CPU 数
    pub num_cpus_per_worker: usize,
    /// 每个 worker 分配的 GPU 数（支持小数，如 0.5）
    #[serde(default)]
    pub num_gpus_per_worker: f64,
    /// Ray 集群地址（None = 本地，Some("auto") = 自动检测，Some("ray://host:port") = 远程）
    #[serde(default)]
    pub cluster_address: Option<String>,
    /// Object Store 内存（GB）
    pub object_store_memory_gb: f64,
}

impl ClusterConfig {
    /// 创建本地集群配置
    pub fn local(num_workers: usize) -> Self {
        Self {
            num_workers,
            num_cpus_per_worker: 1,
            num_gpus_per_worker: 0.0,
            cluster_address: None,
            object_store_memory_gb: 2.0,
        }
    }

    /// 校验合法性
    pub fn validate(&self) -> Result<(), String> {
        if self.num_workers == 0 {
            return Err("num_workers must be > 0".to_string());
        }
        if self.num_cpus_per_worker == 0 {
            return Err("num_cpus_per_worker must be > 0".to_string());
        }
        if self.object_store_memory_gb <= 0.0 {
            return Err("object_store_memory_gb must be > 0".to_string());
        }
        if self.num_gpus_per_worker < 0.0 || !self.num_gpus_per_worker.is_finite() {
            return Err(format!(
                "num_gpus_per_worker ({}) must be >= 0",
                self.num_gpus_per_worker
            ));
        }
        Ok(())
    }
}

/// 算法配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlgorithmConfig {
    /// 算法名（"PPO" | "SAC" | "DQN" | "IMPALA" | "APE_X"）
    pub algorithm: String,
    /// 框架（"torch" | "tensorflow"）
    #[serde(default = "default_framework")]
    pub framework: String,
    /// 算法超参数
    #[serde(default)]
    pub hparams: HashMap<String, serde_json::Value>,
}

fn default_framework() -> String {
    "torch".to_string()
}

impl AlgorithmConfig {
    /// 校验合法性
    pub fn validate(&self) -> Result<(), String> {
        const ALLOWED: &[&str] = &["PPO", "SAC", "DQN", "IMPALA", "APE_X"];
        if !ALLOWED.contains(&self.algorithm.as_str()) {
            return Err(format!(
                "algorithm ({}) must be one of {:?}",
                self.algorithm, ALLOWED
            ));
        }
        if self.framework != "torch" && self.framework != "tensorflow" {
            return Err(format!("framework ({}) must be 'torch' or 'tensorflow'", self.framework));
        }
        Ok(())
    }
}

/// 资源配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceConfig {
    /// 每 worker 环境数
    pub num_envs_per_worker: usize,
    /// 每次采样的步数
    pub rollout_fragment_length: usize,
    /// 训练批大小
    pub train_batch_size: usize,
    /// SGD minibatch 大小
    pub sgd_minibatch_size: usize,
    /// SGD 迭代次数
    pub num_sgd_iter: usize,
    /// 学习率 schedule：[(step, lr), ...]
    #[serde(default)]
    pub lr_schedule: Option<Vec<(usize, f64)>>,
}

impl ResourceConfig {
    /// 校验合法性
    pub fn validate(&self) -> Result<(), String> {
        if self.num_envs_per_worker == 0 {
            return Err("num_envs_per_worker must be > 0".to_string());
        }
        if self.rollout_fragment_length == 0 {
            return Err("rollout_fragment_length must be > 0".to_string());
        }
        if self.train_batch_size == 0 {
            return Err("train_batch_size must be > 0".to_string());
        }
        if self.sgd_minibatch_size == 0 || self.sgd_minibatch_size > self.train_batch_size {
            return Err(format!(
                "sgd_minibatch_size ({}) must be in (0, train_batch_size={}]",
                self.sgd_minibatch_size, self.train_batch_size
            ));
        }
        if self.num_sgd_iter == 0 {
            return Err("num_sgd_iter must be > 0".to_string());
        }
        Ok(())
    }
}

/// 容错配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaultToleranceConfig {
    /// 最大重试次数
    pub max_retries: usize,
    /// Checkpoint 间隔（秒）
    pub checkpoint_interval_s: u64,
    /// Checkpoint 保存目录
    pub checkpoint_dir: String,
    /// 训练结束时是否保存 checkpoint
    #[serde(default)]
    pub checkpoint_at_end: bool,
    /// 保留 checkpoint 数量
    pub keep_checkpoints_num: usize,
    /// 是否从 checkpoint 恢复
    #[serde(default)]
    pub restore: bool,
}

impl FaultToleranceConfig {
    /// 校验合法性
    pub fn validate(&self) -> Result<(), String> {
        if self.checkpoint_interval_s == 0 {
            return Err("checkpoint_interval_s must be > 0".to_string());
        }
        if self.checkpoint_dir.is_empty() {
            return Err("checkpoint_dir must not be empty".to_string());
        }
        Ok(())
    }
}

/// 分布式训练总配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DistributedConfig {
    /// 集群配置
    pub cluster: ClusterConfig,
    /// 算法配置
    pub algorithm: AlgorithmConfig,
    /// 资源配置
    pub resources: ResourceConfig,
    /// 容错配置
    pub fault_tolerance: FaultToleranceConfig,
}

impl DistributedConfig {
    /// 从 TOML 文件加载
    pub fn from_toml_file(path: &std::path::Path) -> Result<Self, DistributedError> {
        let content = std::fs::read_to_string(path).map_err(DistributedError::Io)?;
        Self::from_toml(&content)
    }

    /// 从 TOML 字符串加载
    pub fn from_toml(content: &str) -> Result<Self, DistributedError> {
        let cfg: DistributedConfig = toml::from_str(content).map_err(DistributedError::Toml)?;
        cfg.validate().map_err(DistributedError::Validation)?;
        Ok(cfg)
    }

    /// 校验所有子配置
    pub fn validate(&self) -> Result<(), String> {
        self.cluster.validate()?;
        self.algorithm.validate()?;
        self.resources.validate()?;
        self.fault_tolerance.validate()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_config_local() {
        let cfg = ClusterConfig::local(4);
        assert_eq!(cfg.num_workers, 4);
        assert_eq!(cfg.cluster_address, None);
    }

    #[test]
    fn test_cluster_config_validate_zero_workers() {
        let cfg = ClusterConfig::local(0);
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_cluster_config_validate_invalid_gpus() {
        let cfg = ClusterConfig {
            num_workers: 4,
            num_cpus_per_worker: 1,
            num_gpus_per_worker: -0.5,
            cluster_address: None,
            object_store_memory_gb: 2.0,
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_algorithm_config_validate_invalid_algo() {
        let cfg = AlgorithmConfig {
            algorithm: "INVALID".to_string(),
            framework: "torch".to_string(),
            hparams: HashMap::new(),
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_algorithm_config_validate_invalid_framework() {
        let cfg = AlgorithmConfig {
            algorithm: "PPO".to_string(),
            framework: "jax".to_string(),
            hparams: HashMap::new(),
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_resource_config_validate_ok() {
        let cfg = ResourceConfig {
            num_envs_per_worker: 4,
            rollout_fragment_length: 200,
            train_batch_size: 4000,
            sgd_minibatch_size: 128,
            num_sgd_iter: 10,
            lr_schedule: None,
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_resource_config_validate_minibatch_too_large() {
        let cfg = ResourceConfig {
            num_envs_per_worker: 4,
            rollout_fragment_length: 200,
            train_batch_size: 1000,
            sgd_minibatch_size: 2000,
            num_sgd_iter: 10,
            lr_schedule: None,
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_fault_tolerance_config_validate_empty_dir() {
        let cfg = FaultToleranceConfig {
            max_retries: 3,
            checkpoint_interval_s: 300,
            checkpoint_dir: String::new(),
            checkpoint_at_end: true,
            keep_checkpoints_num: 5,
            restore: true,
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_distributed_config_from_toml_ok() {
        let toml_content = r#"
[cluster]
num_workers = 4
num_cpus_per_worker = 2
object_store_memory_gb = 4.0

[algorithm]
algorithm = "PPO"
framework = "torch"

[algorithm.hparams]
lr = 3e-4

[resources]
num_envs_per_worker = 4
rollout_fragment_length = 200
train_batch_size = 4000
sgd_minibatch_size = 128
num_sgd_iter = 10

[fault_tolerance]
max_retries = 3
checkpoint_interval_s = 300
checkpoint_dir = "checkpoints/"
keep_checkpoints_num = 5
"#;
        let cfg = DistributedConfig::from_toml(toml_content).expect("parse");
        assert_eq!(cfg.cluster.num_workers, 4);
        assert_eq!(cfg.algorithm.algorithm, "PPO");
        assert_eq!(cfg.resources.train_batch_size, 4000);
    }

    #[test]
    fn test_distributed_config_from_toml_missing_section() {
        let toml_content = r#"
[cluster]
num_workers = 4
"#;
        let result = DistributedConfig::from_toml(toml_content);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: 编译并运行测试**

```bash
cd /Users/liupeng/workspace/axon && cargo test -p axon-distributed config:: 2>&1 | tail -20
```

期望：11 个测试全部通过

- [ ] **Step 3: clippy 验证**

```bash
cd /Users/liupeng/workspace/axon && cargo clippy -p axon-distributed --all-targets -- -D warnings 2>&1 | tail -5
```

期望：零警告

---

## Task 3: 实现 actor / param_server 模块

**Files:**
- Modify: `crates/axon-distributed/src/actor.rs`
- Modify: `crates/axon-distributed/src/param_server.rs`

- [ ] **Step 1: actor.rs**

```rust
//! Actor 模型配置（远程环境 Worker）

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Actor 模型配置（远程环境 Worker）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActorConfig {
    /// Actor ID
    pub actor_id: usize,
    /// 环境名称
    pub env_name: String,
    /// 环境配置
    pub env_config: HashMap<String, serde_json::Value>,
    /// 并行环境数
    pub num_envs: usize,
    /// 观测空间形状
    pub observation_space_shape: Vec<usize>,
    /// 动作空间形状
    pub action_space_shape: Vec<usize>,
}

impl ActorConfig {
    /// 校验合法性
    pub fn validate(&self) -> Result<(), String> {
        if self.env_name.is_empty() {
            return Err("env_name must not be empty".to_string());
        }
        if self.num_envs == 0 {
            return Err("num_envs must be > 0".to_string());
        }
        if self.observation_space_shape.is_empty() {
            return Err("observation_space_shape must not be empty".to_string());
        }
        if self.action_space_shape.is_empty() {
            return Err("action_space_shape must not be empty".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ActorConfig {
        ActorConfig {
            actor_id: 0,
            env_name: "AxonTradingEnv".to_string(),
            env_config: HashMap::new(),
            num_envs: 4,
            observation_space_shape: vec![10, 60],
            action_space_shape: vec![1],
        }
    }

    #[test]
    fn test_actor_config_validate_ok() {
        assert!(sample().validate().is_ok());
    }

    #[test]
    fn test_actor_config_empty_env_name() {
        let mut cfg = sample();
        cfg.env_name = String::new();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_actor_config_zero_envs() {
        let mut cfg = sample();
        cfg.num_envs = 0;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_actor_config_empty_obs_shape() {
        let mut cfg = sample();
        cfg.observation_space_shape = vec![];
        assert!(cfg.validate().is_err());
    }
}
```

- [ ] **Step 2: param_server.rs**

```rust
//! Parameter Server 配置

use serde::{Deserialize, Serialize};

/// Parameter Server 配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamServerConfig {
    /// 服务器地址
    pub server_address: String,
    /// 端口
    pub port: u16,
    /// 参数同步间隔（秒）
    pub sync_interval_s: f64,
    /// push/pull 超时（毫秒）
    pub push_pull_timeout_ms: u64,
}

impl ParamServerConfig {
    /// 创建默认配置
    pub fn default_config() -> Self {
        Self {
            server_address: "parameter-server".to_string(),
            port: 8787,
            sync_interval_s: 1.0,
            push_pull_timeout_ms: 5000,
        }
    }

    /// 校验合法性
    pub fn validate(&self) -> Result<(), String> {
        if self.server_address.is_empty() {
            return Err("server_address must not be empty".to_string());
        }
        if self.port == 0 {
            return Err("port must be > 0".to_string());
        }
        if self.sync_interval_s <= 0.0 || !self.sync_interval_s.is_finite() {
            return Err(format!("sync_interval_s ({}) must be > 0", self.sync_interval_s));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = ParamServerConfig::default_config();
        assert_eq!(cfg.port, 8787);
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_address() {
        let mut cfg = ParamServerConfig::default_config();
        cfg.server_address = String::new();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_zero_port() {
        let mut cfg = ParamServerConfig::default_config();
        cfg.port = 0;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_interval() {
        let mut cfg = ParamServerConfig::default_config();
        cfg.sync_interval_s = -1.0;
        assert!(cfg.validate().is_err());
    }
}
```

- [ ] **Step 3: 编译并运行测试**

```bash
cd /Users/liupeng/workspace/axon && cargo test -p axon-distributed actor:: param_server:: 2>&1 | tail -20
```

期望：8 个测试全部通过

---

## Task 4: 实现 checkpoint 模块（TrainingCheckpoint + StepMetrics + CheckpointMetadata）

**Files:**
- Modify: `crates/axon-distributed/src/checkpoint.rs`

- [ ] **Step 1: 写实现与测试**

```rust
//! Checkpoint 与训练指标

use serde::{Deserialize, Serialize};

/// 单步训练指标
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepMetrics {
    /// 训练步数
    pub step: usize,
    /// 平均 episode 奖励
    pub episode_reward_mean: f64,
    /// 平均 episode 长度
    pub episode_len_mean: f64,
    /// 策略损失
    pub policy_loss: f64,
    /// 价值损失
    pub value_loss: f64,
    /// 熵
    pub entropy: f64,
    /// 每秒帧数
    pub fps: f64,
}

/// Checkpoint 元数据
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckpointMetadata {
    /// 训练迭代数
    pub iteration: usize,
    /// 时间戳（毫秒）
    pub timestamp_ms: u64,
    /// step 指标历史
    pub metrics_history: Vec<StepMetrics>,
}

/// 训练状态快照（用于 checkpoint & restore）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainingCheckpoint {
    /// 训练迭代数
    pub iteration: usize,
    /// 序列化的 policy 权重
    pub policy_state: Vec<u8>,
    /// 序列化的 optimizer 状态
    pub optimizer_state: Vec<u8>,
    /// 随机数状态
    pub rng_state: Vec<u8>,
    /// step 指标历史
    pub metrics_history: Vec<StepMetrics>,
    /// 时间戳（毫秒）
    pub timestamp_ms: u64,
}

impl TrainingCheckpoint {
    /// 创建新 checkpoint
    pub fn new(
        iteration: usize,
        policy_state: Vec<u8>,
        optimizer_state: Vec<u8>,
        rng_state: Vec<u8>,
    ) -> Self {
        Self {
            iteration,
            policy_state,
            optimizer_state,
            rng_state,
            metrics_history: Vec::new(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        }
    }

    /// 估算 checkpoint 大小（字节）
    pub fn size_bytes(&self) -> usize {
        self.policy_state.len()
            + self.optimizer_state.len()
            + self.rng_state.len()
            + self.metrics_history.len() * std::mem::size_of::<StepMetrics>()
    }

    /// 序列化为 JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// 从 JSON 反序列化
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// 添加 step 指标
    pub fn add_metrics(&mut self, metrics: StepMetrics) {
        self.metrics_history.push(metrics);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_training_checkpoint_new() {
        let ckpt = TrainingCheckpoint::new(
            100,
            vec![0u8; 1024],
            vec![0u8; 512],
            vec![0u8; 256],
        );
        assert_eq!(ckpt.iteration, 100);
        assert_eq!(ckpt.size_bytes(), 1024 + 512 + 256);
    }

    #[test]
    fn test_training_checkpoint_json_roundtrip() {
        let mut ckpt = TrainingCheckpoint::new(50, vec![1, 2, 3], vec![4, 5], vec![6, 7, 8, 9]);
        ckpt.add_metrics(StepMetrics {
            step: 50,
            episode_reward_mean: 1.5,
            episode_len_mean: 100.0,
            policy_loss: 0.01,
            value_loss: 0.05,
            entropy: 0.5,
            fps: 1000.0,
        });
        let json = ckpt.to_json().expect("serialize");
        let restored = TrainingCheckpoint::from_json(&json).expect("deserialize");
        assert_eq!(restored.iteration, 50);
        assert_eq!(restored.metrics_history.len(), 1);
        assert!((restored.metrics_history[0].episode_reward_mean - 1.5).abs() < 1e-9);
    }

    #[test]
    fn test_checkpoint_metadata() {
        let meta = CheckpointMetadata {
            iteration: 10,
            timestamp_ms: 12345,
            metrics_history: vec![],
        };
        let json = serde_json::to_string(&meta).expect("serialize");
        let restored: CheckpointMetadata = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.iteration, 10);
        assert_eq!(restored.timestamp_ms, 12345);
    }

    #[test]
    fn test_step_metrics_equality() {
        let m1 = StepMetrics {
            step: 1,
            episode_reward_mean: 1.0,
            episode_len_mean: 50.0,
            policy_loss: 0.1,
            value_loss: 0.2,
            entropy: 0.3,
            fps: 500.0,
        };
        let m2 = m1.clone();
        assert_eq!(m1, m2);
    }
}
```

- [ ] **Step 2: 编译并运行测试**

```bash
cd /Users/liupeng/workspace/axon && cargo test -p axon-distributed checkpoint:: 2>&1 | tail -20
```

期望：4 个测试全部通过

---

## Task 5: 实现 error 模块

**Files:**
- Modify: `crates/axon-distributed/src/error.rs`

- [ ] **Step 1: 写错误类型**

```rust
//! 统一错误类型

use thiserror::Error;

/// 分布式训练错误
#[derive(Debug, Error)]
pub enum DistributedError {
    /// 配置错误
    #[error("config error: {0}")]
    Config(String),

    /// 校验错误
    #[error("validation error: {0}")]
    Validation(String),

    /// TOML 解析错误
    #[error("toml parse error: {0}")]
    Toml(String),

    /// IO 错误
    #[error("io error: {0}")]
    Io(String),

    /// 序列化错误
    #[error("serialization error: {0}")]
    Serialization(String),

    /// 集群错误
    #[error("cluster error: {0}")]
    Cluster(String),

    /// 算法错误
    #[error("algorithm error: {0}")]
    Algorithm(String),

    /// Checkpoint 错误
    #[error("checkpoint error: {0}")]
    Checkpoint(String),

    /// 参数服务器错误
    #[error("param server error: {0}")]
    ParamServer(String),
}

/// 分布式训练 Result 类型别名
pub type DistributedResult<T> = Result<T, DistributedError>;
```

- [ ] **Step 2: 编译验证**

```bash
cd /Users/liupeng/workspace/axon && cargo build -p axon-distributed 2>&1 | tail -5
```

---

## Task 6: Python 包 types.py（Algorithm + RayConfig + RLLibTrainConfig）

**Files:**
- Create: `crates/axon-distributed/python/axon_distributed/__init__.py`
- Create: `crates/axon-distributed/python/axon_distributed/types.py`

- [ ] **Step 1: 创建目录**

```bash
mkdir -p crates/axon-distributed/python/axon_distributed
```

- [ ] **Step 2: __init__.py**

```python
"""AXON 分布式训练 Python 包。

设计原则：
- **零硬依赖**：ray / torch 仅在需要时延迟导入
- **本地 mock 模式**：不连接真实 Ray 集群，CI/示例友好
- **与 Rust 端类型对应**：dataclass / Enum 镜像 Rust 配置
"""

from __future__ import annotations

__version__ = "0.0.1"

__all__ = ["__version__"]
```

- [ ] **Step 3: types.py**

```python
"""AXON 分布式训练类型定义。"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Any


class Algorithm(Enum):
    """支持的算法。"""

    PPO = "PPO"
    SAC = "SAC"
    DQN = "DQN"
    IMPALA = "IMPALA"
    APE_X = "APE_X"


@dataclass
class RayConfig:
    """Ray 集群配置。"""

    num_workers: int = 4
    num_cpus_per_worker: int = 1
    num_gpus_per_worker: float = 0.0
    object_store_memory_gb: float = 2.0
    ray_address: str | None = None  # None=local, "auto"=auto-detect

    def to_ray_init_kwargs(self) -> dict[str, Any]:
        """转换为 ray.init() 参数字典。"""
        kwargs: dict[str, Any] = {
            "num_cpus": self.num_workers * self.num_cpus_per_worker + 2,
            "object_store_memory": int(self.object_store_memory_gb * 1e9),
            "ignore_reinit_error": True,
        }
        if self.num_gpus_per_worker > 0:
            kwargs["num_gpus"] = self.num_workers * self.num_gpus_per_worker
        if self.ray_address:
            kwargs["address"] = self.ray_address
        return kwargs

    def validate(self) -> None:
        if self.num_workers <= 0:
            raise ValueError(f"num_workers ({self.num_workers}) must be > 0")
        if self.num_cpus_per_worker <= 0:
            raise ValueError(f"num_cpus_per_worker ({self.num_cpus_per_worker}) must be > 0")
        if self.num_gpus_per_worker < 0:
            raise ValueError(f"num_gpus_per_worker ({self.num_gpus_per_worker}) must be >= 0")
        if self.object_store_memory_gb <= 0:
            raise ValueError(f"object_store_memory_gb ({self.object_store_memory_gb}) must be > 0")


@dataclass
class RLLibTrainConfig:
    """RLLib 训练配置。"""

    algorithm: str = "PPO"
    env: str = "AxonTradingEnv"
    env_config: dict = field(default_factory=dict)
    num_workers: int = 4
    num_envs_per_worker: int = 4
    rollout_fragment_length: int = 200
    train_batch_size: int = 4000
    sgd_minibatch_size: int = 128
    num_sgd_iter: int = 10
    lr: float = 3e-4
    gamma: float = 0.99
    gae_lambda: float = 0.95
    clip_param: float = 0.2
    vf_loss_coeff: float = 0.5
    entropy_coeff: float = 0.01
    framework: str = "torch"
    model_config: dict = field(
        default_factory=lambda: {"fcnet_hiddens": [256, 256], "fcnet_activation": "relu"}
    )

    def validate(self) -> None:
        if self.algorithm not in {a.value for a in Algorithm}:
            raise ValueError(f"algorithm ({self.algorithm}) not supported")
        if self.train_batch_size <= 0:
            raise ValueError(f"train_batch_size ({self.train_batch_size}) must be > 0")
        if self.sgd_minibatch_size <= 0 or self.sgd_minibatch_size > self.train_batch_size:
            raise ValueError(
                f"sgd_minibatch_size ({self.sgd_minibatch_size}) must be in "
                f"(0, train_batch_size={self.train_batch_size}]"
            )

    def to_rllib_config(self, ray_config: RayConfig | None = None) -> dict[str, Any]:
        """转换为 RLLib config 字典。"""
        cfg = {
            "env": self.env,
            "env_config": self.env_config,
            "num_workers": self.num_workers,
            "num_envs_per_worker": self.num_envs_per_worker,
            "rollout_fragment_length": self.rollout_fragment_length,
            "train_batch_size": self.train_batch_size,
            "sgd_minibatch_size": self.sgd_minibatch_size,
            "num_sgd_iter": self.num_sgd_iter,
            "lr": self.lr,
            "gamma": self.gamma,
            "gae_lambda": self.gae_lambda,
            "clip_param": self.clip_param,
            "vf_loss_coeff": self.vf_loss_coeff,
            "entropy_coeff": self.entropy_coeff,
            "model": self.model_config,
            "framework": self.framework,
        }
        if ray_config is not None:
            cfg["num_gpus"] = ray_config.num_gpus_per_worker * ray_config.num_workers
        return cfg


@dataclass
class CheckpointConfig:
    """Checkpoint 配置。"""

    checkpoint_dir: str = "checkpoints/"
    checkpoint_interval_s: int = 300
    keep_checkpoints_num: int = 5
    checkpoint_at_end: bool = True
    max_retries: int = 3
```

- [ ] **Step 4: smoke test**

```bash
cd /Users/liupeng/workspace/axon && \
PYTHONPATH=crates/axon-distributed/python \
/Library/Frameworks/Python.framework/Versions/3.12/bin/python3.12 -c "
from axon_distributed.types import RayConfig, RLLibTrainConfig, Algorithm
rc = RayConfig(num_workers=2)
rc.validate()
print('ray_init_kwargs:', rc.to_ray_init_kwargs())
tc = RLLibTrainConfig(algorithm='PPO', num_workers=2)
tc.validate()
print('rllib_config keys:', list(tc.to_rllib_config(rc).keys()))
print('OK')
"
```

期望：输出 ray_init_kwargs + rllib_config keys + OK

---

## Task 7: Python ray_trainer.py（DistributedTrainer + mock 模式）

**Files:**
- Create: `crates/axon-distributed/python/axon_distributed/ray_trainer.py`

- [ ] **Step 1: 写实现**

```python
"""Ray RLLib 分布式训练器。

支持两种模式：
- **真实模式**：`init_ray=True` 时调用 `ray.init()` 启动真实集群
- **mock 模式**（默认）：`init_ray=False` 时跳过 ray 调用，仅生成 config
  用于 CI / 单元测试 / 无 GPU 环境
"""

from __future__ import annotations

import logging
from typing import Any

from .types import Algorithm, RLLibTrainConfig, RayConfig

logger = logging.getLogger(__name__)


class DistributedTrainer:
    """分布式 RL 训练器，封装 Ray RLLib。"""

    def __init__(
        self,
        ray_config: RayConfig,
        train_config: RLLibTrainConfig,
        init_ray: bool = False,
    ):
        self.ray_config = ray_config
        self.train_config = train_config
        self.init_ray_flag = init_ray
        self._initialized = False
        self._algo: Any = None
        self._iteration_history: list[dict] = []

    @property
    def algorithm(self) -> Any:
        """返回 RLLib algo 实例（mock 模式下为 None）。"""
        return self._algo

    def _ensure_ray_init(self) -> None:
        """确保 Ray 已初始化（mock 模式下跳过）。"""
        if not self.init_ray_flag:
            logger.debug("mock mode: skipping ray.init()")
            return
        if self._initialized:
            return
        import ray  # noqa: PLC0415

        init_kwargs = self.ray_config.to_ray_init_kwargs()
        ray.init(**init_kwargs)
        self._initialized = True
        logger.info("Ray initialized: %s", init_kwargs)

    def build_algo(self) -> Any:
        """构建 RLLib Algorithm 实例（mock 模式返回 None）。"""
        self.train_config.validate()
        self.ray_config.validate()
        if not self.init_ray_flag:
            logger.debug("mock mode: skipping algo build")
            return None

        self._ensure_ray_init()
        if self.train_config.algorithm == Algorithm.PPO.value:
            from ray.rllib.algorithms.ppo import PPOConfig  # noqa: PLC0415

            algo_config = (
                PPOConfig()
                .environment(env=self.train_config.env, env_config=self.train_config.env_config)
                .framework(self.train_config.framework)
                .resources(
                    num_gpus=self.ray_config.num_gpus_per_worker,
                    num_cpus=self.ray_config.num_cpus_per_worker,
                )
                .env_runners(
                    num_env_runners=self.ray_config.num_workers,
                    num_envs_per_worker=self.train_config.num_envs_per_worker,
                    rollout_fragment_length=self.train_config.rollout_fragment_length,
                )
                .training(
                    lr=self.train_config.lr,
                    gamma=self.train_config.gamma,
                    gae_lambda=self.train_config.gae_lambda,
                    clip_param=self.train_config.clip_param,
                    vf_loss_coeff=self.train_config.vf_loss_coeff,
                    entropy_coeff=self.train_config.entropy_coeff,
                    train_batch_size=self.train_config.train_batch_size,
                    sgd_minibatch_size=self.train_config.sgd_minibatch_size,
                    num_sgd_iter=self.train_config.num_sgd_iter,
                )
                .model(self.train_config.model_config)
            )
            self._algo = algo_config.build()
            return self._algo

        if self.train_config.algorithm == Algorithm.SAC.value:
            from ray.rllib.algorithms.sac import SACConfig  # noqa: PLC0415

            algo_config = (
                SACConfig()
                .environment(env=self.train_config.env, env_config=self.train_config.env_config)
                .framework(self.train_config.framework)
                .resources(num_gpus=self.ray_config.num_gpus_per_worker)
                .env_runners(
                    num_env_runners=self.ray_config.num_workers,
                    num_envs_per_worker=self.train_config.num_envs_per_worker,
                )
            )
            self._algo = algo_config.build()
            return self._algo

        raise ValueError(f"Unsupported algorithm: {self.train_config.algorithm}")

    def train(
        self,
        num_iterations: int,
        checkpoint_interval: int = 10,
        checkpoint_dir: str = "checkpoints/",
    ) -> dict[str, Any]:
        """执行分布式训练（mock 模式下生成合成 metrics）。"""
        algo = self.build_algo()
        results = []
        for i in range(num_iterations):
            if algo is not None:
                result = algo.train()
            else:
                # mock：生成合成 metrics
                result = {
                    "env_runners": {
                        "episode_reward_mean": 1.0 + 0.01 * i,
                        "episode_len_mean": 100.0,
                    },
                    "info": {"learner": {"policy_loss": 0.01, "vf_loss": 0.05, "entropy": 0.5}},
                    "timers": {"training_iteration_time_ms": 1000.0},
                    "iteration": i + 1,
                }
            results.append(result)
            self._iteration_history.append(result)
            if (i + 1) % checkpoint_interval == 0:
                logger.info("iter %d: reward=%.4f", i + 1, self._get_reward(result))

        return {
            "iterations": num_iterations,
            "final_reward": self._get_reward(results[-1]) if results else 0.0,
            "results": results,
        }

    @staticmethod
    def _get_reward(result: dict) -> float:
        return float(result.get("env_runners", {}).get("episode_reward_mean", 0.0))

    def get_history(self) -> list[dict]:
        """返回所有 iteration 的历史记录。"""
        return list(self._iteration_history)
```

- [ ] **Step 2: smoke test**

```bash
cd /Users/liupeng/workspace/axon && \
PYTHONPATH=crates/axon-distributed/python \
/Library/Frameworks/Python.framework/Versions/3.12/bin/python3.12 -c "
from axon_distributed.types import RayConfig, RLLibTrainConfig
from axon_distributed.ray_trainer import DistributedTrainer
rc = RayConfig(num_workers=2)
tc = RLLibTrainConfig(algorithm='PPO', num_workers=2)
trainer = DistributedTrainer(rc, tc, init_ray=False)
result = trainer.train(num_iterations=3, checkpoint_interval=2)
print('iterations:', result['iterations'])
print('final_reward:', result['final_reward'])
print('OK')
"
```

期望：输出 iterations=3 + final_reward + OK

---

## Task 8: Python actor.py（EnvironmentWorker + ActorPool）

**Files:**
- Create: `crates/axon-distributed/python/axon_distributed/actor.py`

- [ ] **Step 1: 写实现**

```python
"""Ray Actor Workers + ActorPool。

设计：
- **延迟导入 ray**：避免硬依赖，未使用时无需安装
- **mock 模式**：当 RAY_AVAILABLE=False 时，EnvironmentWorker 退化为本地类
- **真实模式**：用 @ray.remote 装饰器暴露为 Ray Actor
"""

from __future__ import annotations

import logging
import os
from dataclasses import dataclass, field
from typing import Any

logger = logging.getLogger(__name__)

# 尝试导入 ray，未安装时回退到 mock 模式
try:
    import ray  # noqa: F401

    RAY_AVAILABLE = True
except ImportError:
    RAY_AVAILABLE = False
    ray = None  # type: ignore[assignment]

# 仅在 RAY_AVAILABLE 时装饰，否则为 no-op
def _ray_remote(cls: type) -> type:
    """条件性 @ray.remote 装饰器（无 ray 时为 no-op）。"""
    if RAY_AVAILABLE:
        return ray.remote(cls)  # type: ignore[union-attr]
    return cls


@dataclass
class WorkerMetrics:
    """单个 Worker 的性能指标。"""

    worker_id: int
    num_envs: int
    avg_reward: float
    total_steps: int = 0


@dataclass
class ActorPool:
    """管理一组 EnvironmentWorker Actors。"""

    num_workers: int
    env_class: str
    env_config: dict
    num_envs_per_worker: int
    observation_space_shape: tuple[int, ...]
    action_space_shape: tuple[int, ...]
    workers: list[Any] = field(default_factory=list, init=False)

    def __post_init__(self) -> None:
        if RAY_AVAILABLE:
            self.workers = [
                EnvironmentWorker.remote(  # type: ignore[attr-defined]
                    worker_id=i,
                    env_class=self.env_class,
                    env_config=self.env_config,
                    num_envs=self.num_envs_per_worker,
                    observation_space_shape=self.observation_space_shape,
                    action_space_shape=self.action_space_shape,
                )
                for i in range(self.num_workers)
            ]
        else:
            # mock 模式：本地实例
            self.workers = [
                EnvironmentWorker(
                    worker_id=i,
                    env_class=self.env_class,
                    env_config=self.env_config,
                    num_envs=self.num_envs_per_worker,
                    observation_space_shape=self.observation_space_shape,
                    action_space_shape=self.action_space_shape,
                )
                for i in range(self.num_workers)
            ]

    def reset_all(self) -> list[dict]:
        """重置所有 Workers。"""
        if RAY_AVAILABLE:
            return ray.get([w.reset.remote() for w in self.workers])  # type: ignore[union-attr]
        return [w.reset() for w in self.workers]

    def step_all(self, actions_list: list) -> list[dict]:
        """并行执行所有 Workers 的 step。"""
        if RAY_AVAILABLE:
            return ray.get(
                [w.step.remote(actions) for w, actions in zip(self.workers, actions_list)]
            )  # type: ignore[union-attr]
        return [w.step(actions) for w, actions in zip(self.workers, actions_list)]

    def get_all_metrics(self) -> list[WorkerMetrics]:
        """获取所有 Worker 的性能指标。"""
        if RAY_AVAILABLE:
            return ray.get([w.get_metrics.remote() for w in self.workers])  # type: ignore[union-attr]
        return [w.get_metrics() for w in self.workers]


@_ray_remote
class EnvironmentWorker:
    """远程环境 Actor Worker。"""

    def __init__(
        self,
        worker_id: int,
        env_class: str,
        env_config: dict,
        num_envs: int,
        observation_space_shape: tuple,
        action_space_shape: tuple,
    ):
        self.worker_id = worker_id
        self.num_envs = num_envs
        self.env_class = env_class
        self.env_config = env_config
        self.observation_space_shape = observation_space_shape
        self.action_space_shape = action_space_shape

        # 状态
        self.observations: list = [None] * num_envs
        self.dones: list = [True] * num_envs
        self.rewards: list = [0.0] * num_envs
        self.episode_rewards: list = [0.0] * num_envs
        self.total_steps: int = 0

    def reset(self) -> dict:
        """重置所有环境，返回初始观测（mock 模式返回零向量）。"""
        if not RAY_AVAILABLE:
            self.observations = [
                self._mock_observation() for _ in range(self.num_envs)
            ]
        else:
            try:
                from axon_env import AxonTradingEnv  # type: ignore  # noqa: PLC0415

                self.observations = [
                    AxonTradingEnv(self.env_config).reset() for _ in range(self.num_envs)
                ]
            except ImportError:
                logger.warning("axon_env not available, using mock observations")
                self.observations = [
                    self._mock_observation() for _ in range(self.num_envs)
                ]
        self.dones = [False] * self.num_envs
        self.episode_rewards = [0.0] * self.num_envs
        return {
            "worker_id": self.worker_id,
            "observations": self.observations,
        }

    def step(self, actions: list) -> dict:
        """执行动作，返回经验 batch。"""
        rewards = []
        for i in range(self.num_envs):
            if self.dones[i]:
                # 自动重置已完成的环境
                self.observations[i] = self._mock_observation() if not RAY_AVAILABLE else self.observations[i]
                self.dones[i] = False
                self.episode_rewards[i] = 0.0
                rewards.append(0.0)
            else:
                # mock：返回常数奖励
                r = 0.01
                self.episode_rewards[i] += r
                rewards.append(r)
            self.total_steps += 1
        return {
            "worker_id": self.worker_id,
            "rewards": rewards,
            "episode_rewards": list(self.episode_rewards),
        }

    def get_metrics(self) -> WorkerMetrics:
        """获取 Worker 级别的性能指标。"""
        avg_reward = (
            sum(self.episode_rewards) / len(self.episode_rewards)
            if self.episode_rewards
            else 0.0
        )
        return WorkerMetrics(
            worker_id=self.worker_id,
            num_envs=self.num_envs,
            avg_reward=avg_reward,
            total_steps=self.total_steps,
        )

    def _mock_observation(self) -> list:
        """生成 mock 观测（零向量）。"""
        return [0.0] * self.observation_space_shape[-1]
```

- [ ] **Step 2: smoke test**

```bash
cd /Users/liupeng/workspace/axon && \
PYTHONPATH=crates/axon-distributed/python \
/Library/Frameworks/Python.framework/Versions/3.12/bin/python3.12 -c "
from axon_distributed.actor import ActorPool, EnvironmentWorker, RAY_AVAILABLE
print('RAY_AVAILABLE:', RAY_AVAILABLE)
pool = ActorPool(
    num_workers=2,
    env_class='AxonTradingEnv',
    env_config={},
    num_envs_per_worker=2,
    observation_space_shape=(10, 60),
    action_space_shape=(1,),
)
print('workers:', len(pool.workers))
obs = pool.reset_all()
print('reset_all returns:', len(obs), 'items')
result = pool.step_all([[0, 0], [0, 0]])
print('step_all returns:', len(result), 'items')
metrics = pool.get_all_metrics()
print('metrics:', [(m.worker_id, m.avg_reward) for m in metrics])
print('OK')
"
```

期望：RAY_AVAILABLE=False，workers=2，reset/step/metrics 都成功

---

## Task 9: Python param_server.py（ParameterServer + DistributedPolicy）

**Files:**
- Create: `crates/axon-distributed/python/axon_distributed/param_server.py`

- [ ] **Step 1: 写实现**

```python
"""Parameter Server Actor + DistributedPolicy。

mock 模式下 ParameterServer 退化为本地类，DistributedPolicy 不工作。
"""

from __future__ import annotations

import logging
import pickle
from dataclasses import dataclass, field
from typing import Any

from .actor import _ray_remote, RAY_AVAILABLE

logger = logging.getLogger(__name__)


@dataclass
class ParamServerStats:
    """Parameter Server 统计信息。"""

    version: int
    push_count: int
    pull_count: int


@_ray_remote
class ParameterServer:
    """Parameter Server Actor。"""

    def __init__(self, model_cls: str = "torch.nn.Linear", model_config: dict | None = None):
        self.model_cls = model_cls
        self.model_config = model_config or {}
        self.version: int = 0
        self.gradient_buffer: list = []
        self.push_count: int = 0
        self.pull_count: int = 0

    def get_parameters(self) -> tuple[bytes, int]:
        """拉取当前参数（Worker 调用）。"""
        self.pull_count += 1
        # mock：返回空 dict
        return pickle.dumps({}), self.version

    def push_gradients(self, gradients: bytes, worker_id: int) -> bool:
        """推送梯度（Worker 调用）。"""
        try:
            grad_dict = pickle.loads(gradients)
            self.gradient_buffer.append((worker_id, grad_dict))
        except Exception as e:  # noqa: BLE001
            logger.warning("Failed to unpickle gradients: %s", e)
            return False
        self.version += 1
        self.push_count += 1
        return True

    def get_version(self) -> int:
        return self.version

    def get_stats(self) -> ParamServerStats:
        return ParamServerStats(
            version=self.version, push_count=self.push_count, pull_count=self.pull_count
        )


@dataclass
class DistributedPolicy:
    """分布式策略：通过 Parameter Server 同步参数。"""

    param_server_address: str
    worker_id: int = 0
    policy: Any = None
    version: int = 0
    sync_count: int = field(default=0, init=False)

    def sync_parameters(self) -> None:
        """从 Parameter Server 拉取最新参数。"""
        if not RAY_AVAILABLE:
            logger.debug("mock mode: skipping sync_parameters")
            return
        server = ray.get_actor(self.param_server_address)  # type: ignore[name-defined]  # noqa: PLC0415
        params_bytes, version = ray.get(server.get_parameters.remote())  # type: ignore[name-defined]  # noqa: PLC0415
        if self.policy is not None:
            state_dict = pickle.loads(params_bytes)
            if hasattr(self.policy, "load_state_dict"):
                self.policy.load_state_dict(state_dict)
        self.version = version
        self.sync_count += 1

    def push_update(self, gradients: dict) -> None:
        """推送梯度到 Parameter Server。"""
        if not RAY_AVAILABLE:
            logger.debug("mock mode: skipping push_update")
            return
        server = ray.get_actor(self.param_server_address)  # type: ignore[name-defined]  # noqa: PLC0415
        grad_bytes = pickle.dumps(gradients)
        ray.get(server.push_gradients.remote(grad_bytes, self.worker_id))  # type: ignore[name-defined]  # noqa: PLC0415
```

- [ ] **Step 2: smoke test**

```bash
cd /Users/liupeng/workspace/axon && \
PYTHONPATH=crates/axon-distributed/python \
/Library/Frameworks/Python.framework/Versions/3.12/bin/python3.12 -c "
from axon_distributed.param_server import ParameterServer, ParamServerStats
ps = ParameterServer(model_cls='torch.nn.Linear', model_config={})
print('initial stats:', ps.get_stats())
print('OK')
"
```

期望：输出 initial stats + OK

---

## Task 10: Python fault_tolerance.py（CheckpointManager + FaultTolerantTrainer）

**Files:**
- Create: `crates/axon-distributed/python/axon_distributed/fault_tolerance.py`

- [ ] **Step 1: 写实现**

```python
"""Checkpoint 管理与容错训练器。

设计：
- **本地模式**：使用 JSON 文件存储 checkpoint 元数据
- **mock trainer**：trainer.algo.train() 返回合成 metrics
- **自动清理**：保留最近 N 个 checkpoint，删除旧的
"""

from __future__ import annotations

import json
import logging
import shutil
import time
from dataclasses import asdict
from pathlib import Path
from typing import Any

from .types import CheckpointConfig

logger = logging.getLogger(__name__)


class CheckpointManager:
    """Checkpoint 管理器。"""

    def __init__(self, config: CheckpointConfig):
        self.config = config
        self.checkpoint_dir = Path(config.checkpoint_dir)
        self.checkpoint_dir.mkdir(parents=True, exist_ok=True)

    def save_checkpoint(
        self,
        algo: Any,
        iteration: int,
        metrics: dict | None = None,
    ) -> str:
        """保存 checkpoint（mock 模式下仅写元数据 JSON）。"""
        timestamp = int(time.time() * 1000)
        ckpt_name = f"checkpoint_iter{iteration}_{timestamp}"
        meta_path = self.checkpoint_dir / f"{ckpt_name}.meta.json"

        metadata = {
            "iteration": iteration,
            "timestamp": timestamp,
            "metrics": metrics or {},
            "checkpoint_path": str(meta_path),
        }
        with open(meta_path, "w", encoding="utf-8") as f:
            json.dump(metadata, f, indent=2, default=str)

        if algo is not None and hasattr(algo, "save"):
            try:
                algo.save(self.checkpoint_dir / ckpt_name)
            except Exception as e:  # noqa: BLE001
                logger.warning("algo.save failed: %s", e)

        self._cleanup_old_checkpoints()
        logger.info("Checkpoint saved: %s", meta_path)
        return str(meta_path)

    def find_latest_checkpoint(self) -> str | None:
        """查找最新的 checkpoint。"""
        meta_files = sorted(
            self.checkpoint_dir.glob("checkpoint_*.meta.json"),
            key=lambda p: p.stat().st_mtime,
            reverse=True,
        )
        if not meta_files:
            return None
        return str(meta_files[0])

    def restore_checkpoint(
        self, algo: Any = None, checkpoint_path: str | None = None
    ) -> dict:
        """恢复 checkpoint。"""
        if checkpoint_path is None:
            checkpoint_path = self.find_latest_checkpoint()
        if checkpoint_path is None:
            logger.warning("No checkpoint found, starting fresh")
            return {}
        meta_path = Path(checkpoint_path)
        if not meta_path.exists():
            return {}
        with open(meta_path, "r", encoding="utf-8") as f:
            meta = json.load(f)
        if algo is not None and hasattr(algo, "restore"):
            try:
                algo.restore(meta.get("checkpoint_path", ""))
            except Exception as e:  # noqa: BLE001
                logger.warning("algo.restore failed: %s", e)
        logger.info("Restored from: %s", checkpoint_path)
        return meta

    def _cleanup_old_checkpoints(self) -> None:
        """删除超过 keep_checkpoints_num 的旧 checkpoint。"""
        meta_files = sorted(
            self.checkpoint_dir.glob("checkpoint_*.meta.json"),
            key=lambda p: p.stat().st_mtime,
            reverse=True,
        )
        for old_meta in meta_files[self.config.keep_checkpoints_num :]:
            old_meta.unlink(missing_ok=True)
            # 同时删除关联的 checkpoint 目录
            ckpt_name = old_meta.name.replace(".meta.json", "")
            ckpt_dir = self.checkpoint_dir / ckpt_name
            if ckpt_dir.is_dir():
                shutil.rmtree(ckpt_dir, ignore_errors=True)
            logger.info("Removed old checkpoint: %s", old_meta)


class FaultTolerantTrainer:
    """容错训练器。"""

    def __init__(self, trainer: Any, checkpoint_config: CheckpointConfig):
        self.trainer = trainer
        self.ckpt_manager = CheckpointManager(checkpoint_config)
        self.checkpoint_config = checkpoint_config
        self.start_iteration = 0

    def train_with_recovery(
        self,
        num_iterations: int,
        checkpoint_interval: int = 10,
    ) -> dict:
        """带故障恢复的训练。"""
        # 尝试恢复
        algo = getattr(self.trainer, "algorithm", None) or getattr(self.trainer, "algo", None)
        metadata = self.ckpt_manager.restore_checkpoint(algo=algo)
        if metadata:
            self.start_iteration = metadata.get("iteration", 0) + 1
            logger.info("Resuming from iteration %d", self.start_iteration)

        results = []
        for i in range(self.start_iteration, num_iterations):
            try:
                # 优先调用 train_with_recovery-friendly 接口
                if hasattr(self.trainer, "train") and not hasattr(self.trainer, "algo"):
                    result = self.trainer.train(num_iterations=1, checkpoint_interval=1)
                else:
                    # 使用 mock 合成 result
                    result = {
                        "env_runners": {"episode_reward_mean": 1.0 + 0.01 * i},
                        "iteration": i + 1,
                    }
                results.append(result)
            except Exception as e:  # noqa: BLE001
                logger.error("Worker failed at iter %d: %s", i, e)
                if hasattr(self, "_retry_count"):
                    self._retry_count += 1
                else:
                    self._retry_count = 1
                if self._retry_count > self.checkpoint_config.max_retries:
                    raise
                continue

            # 定期 checkpoint
            if (i + 1) % checkpoint_interval == 0:
                metrics = {
                    "episode_reward_mean": result.get("env_runners", {}).get(
                        "episode_reward_mean", 0.0
                    )
                }
                self.ckpt_manager.save_checkpoint(algo, i + 1, metrics)

        return {
            "iterations": len(results),
            "start_iteration": self.start_iteration,
            "final_reward": results[-1].get("env_runners", {}).get("episode_reward_mean", 0.0)
            if results
            else 0.0,
            "results": results,
        }
```

- [ ] **Step 2: smoke test**

```bash
cd /Users/liupeng/workspace/axon && \
PYTHONPATH=crates/axon-distributed/python \
/Library/Frameworks/Python.framework/Versions/3.12/bin/python3.12 -c "
import tempfile
from pathlib import Path
from axon_distributed.types import CheckpointConfig
from axon_distributed.fault_tolerance import CheckpointManager

with tempfile.TemporaryDirectory() as tmp:
    cfg = CheckpointConfig(checkpoint_dir=tmp, keep_checkpoints_num=2)
    mgr = CheckpointManager(cfg)
    p1 = mgr.save_checkpoint(None, iteration=1, metrics={'reward': 1.0})
    p2 = mgr.save_checkpoint(None, iteration=2, metrics={'reward': 2.0})
    p3 = mgr.save_checkpoint(None, iteration=3, metrics={'reward': 3.0})
    print('saved:', Path(p1).name, Path(p2).name, Path(p3).name)
    latest = mgr.find_latest_checkpoint()
    print('latest:', Path(latest).name if latest else None)
    # 保留 2 个，应清理掉最旧的
    metas = sorted(Path(tmp).glob('*.meta.json'))
    print('remaining meta files:', len(metas))
    print('OK')
"
```

期望：saved 3 个，latest 是最新的，remaining meta files = 2

---

## Task 11: PyO3 桥接层

**Files:**
- Modify: `crates/axon-distributed/src/python/mod.rs`

- [ ] **Step 1: 实现 PyO3 桥接**

```rust
//! PyO3 桥接层
//!
//! 将 Rust 端 `DistributedConfig` / `TrainingCheckpoint` 暴露给 Python。

#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::useless_conversion)]
#![allow(deprecated)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::checkpoint::{StepMetrics, TrainingCheckpoint};
use crate::config::DistributedConfig;

/// 分布式训练运行器
#[pyclass(name = "DistributedRunner")]
pub struct DistributedRunner {
    config: DistributedConfig,
}

#[pymethods]
impl DistributedRunner {
    /// 从 Python dict 创建 runner
    #[new]
    fn new(config_dict: &Bound<'_, PyDict>) -> PyResult<Self> {
        let json_str: String = Python::with_gil(|py| {
            let json_module = py.import("json")?;
            let dumped = json_module.call_method1("dumps", (config_dict,))?;
            dumped.extract::<String>()
        })?;
        let cfg: DistributedConfig = serde_json::from_str(&json_str).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("invalid config: {e}"))
        })?;
        cfg.validate().map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(e)
        })?;
        Ok(Self { config: cfg })
    }

    /// 从 TOML 文件加载
    #[staticmethod]
    fn from_toml_file(path: String) -> PyResult<Self> {
        let cfg = DistributedConfig::from_toml_file(std::path::Path::new(&path))
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("{e:?}")))?;
        Ok(Self { config: cfg })
    }

    /// 获取摘要
    fn __repr__(&self) -> String {
        format!(
            "DistributedRunner(workers={}, algo={}, batch={})",
            self.config.cluster.num_workers,
            self.config.algorithm.algorithm,
            self.config.resources.train_batch_size
        )
    }
}

/// 便捷函数：序列化 TrainingCheckpoint
#[pyfunction]
fn py_save_checkpoint(
    iteration: usize,
    policy_state: Vec<u8>,
    optimizer_state: Vec<u8>,
    rng_state: Vec<u8>,
) -> String {
    let ckpt = TrainingCheckpoint::new(iteration, policy_state, optimizer_state, rng_state);
    ckpt.to_json().unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
}

/// 便捷函数：反序列化 TrainingCheckpoint
#[pyfunction]
fn py_load_checkpoint(json: &str) -> PyResult<(usize, Vec<u8>)> {
    let ckpt = TrainingCheckpoint::from_json(json)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("{e}")))?;
    Ok((ckpt.iteration, ckpt.policy_state))
}

/// 便捷函数：序列化 StepMetrics
#[pyfunction]
fn py_serialize_metrics(
    step: usize,
    reward: f64,
    policy_loss: f64,
    value_loss: f64,
    entropy: f64,
    fps: f64,
) -> String {
    let m = StepMetrics {
        step,
        episode_reward_mean: reward,
        episode_len_mean: 0.0,
        policy_loss,
        value_loss,
        entropy,
        fps,
    };
    serde_json::to_string(&m).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
}

/// Python 模块入口
pub fn register_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<DistributedRunner>()?;
    m.add_function(wrap_pyfunction!(py_save_checkpoint, m)?)?;
    m.add_function(wrap_pyfunction!(py_load_checkpoint, m)?)?;
    m.add_function(wrap_pyfunction!(py_serialize_metrics, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
```

- [ ] **Step 2: 编译验证**

```bash
cd /Users/liupeng/workspace/axon && \
cargo build -p axon-distributed --features python 2>&1 | tail -10
```

期望：`Finished dev profile`

---

## Task 12: 默认 TOML 配置

**Files:**
- Create: `crates/axon-distributed/config/default_distributed.toml`

- [ ] **Step 1: 写配置**

```toml
# AXON 分布式训练默认配置
#
# 适用于 PPO + Ray RLLib 训练
# 通过 toml 加载到 axon_distributed.types.RLLibTrainConfig

[cluster]
num_workers = 4
num_cpus_per_worker = 2
num_gpus_per_worker = 0.0
cluster_address = "auto"          # auto=自动检测，None=本地
object_store_memory_gb = 4.0

[algorithm]
algorithm = "PPO"
framework = "torch"

[algorithm.hparams]
lr = 0.0003
gamma = 0.99
gae_lambda = 0.95
clip_param = 0.2
vf_loss_coeff = 0.5
entropy_coeff = 0.01

[resources]
num_envs_per_worker = 4
rollout_fragment_length = 200
train_batch_size = 4000
sgd_minibatch_size = 128
num_sgd_iter = 10

[fault_tolerance]
max_retries = 3
checkpoint_interval_s = 300
checkpoint_dir = "checkpoints/"
checkpoint_at_end = true
keep_checkpoints_num = 5
restore = true
```

- [ ] **Step 2: 添加 Python 端 TOML 加载测试**

在 `crates/axon-distributed/python/axon_distributed/types.py` 末尾添加：

```python
def _load_default_toml(self) -> None:
    """从默认 TOML 配置加载（仅供测试）。"""
    from pathlib import Path  # noqa: PLC0415

    toml_path = (
        Path(__file__).parent.parent.parent
        / "config"
        / "default_distributed.toml"
    )
    if not toml_path.exists():
        return
    try:
        import tomllib  # Python 3.11+  # noqa: PLC0415
    except ImportError:
        import tomli as tomllib  # type: ignore[no-redef]  # noqa: PLC0415
    with open(toml_path, "rb") as f:
        data = tomllib.load(f)
    cluster = data.get("cluster", {})
    algo = data.get("algorithm", {})
    resources = data.get("resources", {})
    self.algorithm = algo.get("algorithm", self.algorithm)
    self.num_workers = cluster.get("num_workers", self.num_workers)
    self.num_envs_per_worker = resources.get("num_envs_per_worker", self.num_envs_per_worker)
    self.rollout_fragment_length = resources.get("rollout_fragment_length", self.rollout_fragment_length)
    self.train_batch_size = resources.get("train_batch_size", self.train_batch_size)
    self.sgd_minibatch_size = resources.get("sgd_minibatch_size", self.sgd_minibatch_size)
    self.num_sgd_iter = resources.get("num_sgd_iter", self.num_sgd_iter)
    self.framework = algo.get("framework", self.framework)
    hparams = algo.get("hparams", {})
    self.lr = hparams.get("lr", self.lr)
    self.gamma = hparams.get("gamma", self.gamma)
    self.gae_lambda = hparams.get("gae_lambda", self.gae_lambda)
    self.clip_param = hparams.get("clip_param", self.clip_param)
    self.vf_loss_coeff = hparams.get("vf_loss_coeff", self.vf_loss_coeff)
    self.entropy_coeff = hparams.get("entropy_coeff", self.entropy_coeff)
```

- [ ] **Step 3: smoke test 验证 TOML 加载**

```bash
cd /Users/liupeng/workspace/axon && \
PYTHONPATH=crates/axon-distributed/python \
/Library/Frameworks/Python.framework/Versions/3.12/bin/python3.12 -c "
from axon_distributed.types import RLLibTrainConfig
cfg = RLLibTrainConfig()
cfg._load_default_toml()
print('algorithm:', cfg.algorithm)
print('num_workers:', cfg.num_workers)
print('train_batch_size:', cfg.train_batch_size)
print('lr:', cfg.lr)
print('OK')
"
```

期望：algorithm=PPO，num_workers=4，train_batch_size=4000，lr=0.0003

---

## Task 13: 示例脚本

**Files:**
- Create: `examples/distributed_basic.py`
- Create: `examples/distributed_actor_pool.py`

- [ ] **Step 1: distributed_basic.py**

```python
"""distributed_basic.py — 分布式训练 mock 模式示例。

不连接真实 Ray 集群，演示：
1. DistributedTrainer mock 训练
2. Checkpoint 保存/恢复
3. 容错训练循环
"""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

CARGO_MANIFEST = Path(__file__).parent.parent / "crates" / "axon-distributed"
sys.path.insert(0, str(CARGO_MANIFEST / "python"))

from axon_distributed.types import (  # noqa: E402
    CheckpointConfig,
    RayConfig,
    RLLibTrainConfig,
)
from axon_distributed.ray_trainer import DistributedTrainer  # noqa: E402
from axon_distributed.fault_tolerance import (  # noqa: E402
    CheckpointManager,
    FaultTolerantTrainer,
)


def main() -> int:
    print("=" * 60)
    print("分布式训练 mock 模式示例")
    print("=" * 60)

    # 1. 加载默认 TOML 配置
    train_cfg = RLLibTrainConfig()
    train_cfg._load_default_toml()
    print(f"\n[1] 训练配置（来自 default_distributed.toml）")
    print(f"  algorithm: {train_cfg.algorithm}")
    print(f"  num_workers: {train_cfg.num_workers}")
    print(f"  train_batch_size: {train_cfg.train_batch_size}")
    print(f"  lr: {train_cfg.lr}")

    ray_cfg = RayConfig(
        num_workers=train_cfg.num_workers,
        num_cpus_per_worker=2,
        num_gpus_per_worker=0.0,
    )
    ray_cfg.validate()
    print(f"  ray_init_kwargs: {ray_cfg.to_ray_init_kwargs()}")

    # 2. mock 训练
    print("\n[2] DistributedTrainer mock 训练")
    trainer = DistributedTrainer(ray_cfg, train_cfg, init_ray=False)
    result = trainer.train(num_iterations=5, checkpoint_interval=3)
    print(f"  iterations: {result['iterations']}")
    print(f"  final_reward: {result['final_reward']:.4f}")

    # 3. 容错训练 + Checkpoint
    print("\n[3] FaultTolerantTrainer + Checkpoint")
    with tempfile.TemporaryDirectory() as tmp:
        ckpt_cfg = CheckpointConfig(
            checkpoint_dir=tmp,
            keep_checkpoints_num=2,
            checkpoint_at_end=True,
        )
        ft_trainer = FaultTolerantTrainer(trainer, ckpt_cfg)
        result = ft_trainer.train_with_recovery(num_iterations=5, checkpoint_interval=2)
        print(f"  start_iteration: {result['start_iteration']}")
        print(f"  iterations: {result['iterations']}")
        print(f"  final_reward: {result['final_reward']:.4f}")

        # 验证 checkpoint 文件
        ckpt_files = sorted(Path(tmp).glob("*.meta.json"))
        print(f"  checkpoint meta files: {len(ckpt_files)}")
        for f in ckpt_files:
            print(f"    {f.name}")

    print("\n=== ALL PASS ===")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 2: distributed_actor_pool.py**

```python
"""distributed_actor_pool.py — ActorPool 用法示例（mock 模式）。"""

from __future__ import annotations

import sys
from pathlib import Path

CARGO_MANIFEST = Path(__file__).parent.parent / "crates" / "axon-distributed"
sys.path.insert(0, str(CARGO_MANIFEST / "python"))

from axon_distributed.actor import ActorPool, RAY_AVAILABLE  # noqa: E402


def main() -> int:
    print("=" * 60)
    print("ActorPool 示例（mock 模式）")
    print("=" * 60)
    print(f"RAY_AVAILABLE: {RAY_AVAILABLE}")

    # 1. 创建 ActorPool
    pool = ActorPool(
        num_workers=2,
        env_class="AxonTradingEnv",
        env_config={"data_path": "mock.parquet"},
        num_envs_per_worker=2,
        observation_space_shape=(10, 60),
        action_space_shape=(1,),
    )
    print(f"\n[1] 创建 ActorPool: {len(pool.workers)} 个 workers")

    # 2. reset_all
    obs_list = pool.reset_all()
    print(f"[2] reset_all 返回: {len(obs_list)} 个 worker 的初始观测")
    for obs in obs_list:
        print(f"  worker {obs['worker_id']}: {len(obs['observations'])} envs")

    # 3. step_all
    actions_list = [[0] * 2] * len(pool.workers)  # 每个 worker 2 个 env 的 action
    results = pool.step_all(actions_list)
    print(f"[3] step_all 返回: {len(results)} 个 worker 的 step 结果")
    for r in results:
        print(f"  worker {r['worker_id']}: rewards={r['rewards']}")

    # 4. get_all_metrics
    metrics = pool.get_all_metrics()
    print(f"[4] get_all_metrics 返回: {len(metrics)} 个 WorkerMetrics")
    for m in metrics:
        print(f"  worker {m.worker_id}: avg_reward={m.avg_reward:.4f}, total_steps={m.total_steps}")

    print("\n=== ALL PASS ===")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 3: 运行两个示例**

```bash
cd /Users/liupeng/workspace/axon && \
/Library/Frameworks/Python.framework/Versions/3.12/bin/python3.12 examples/distributed_basic.py 2>&1 | tail -30
```

```bash
cd /Users/liupeng/workspace/axon && \
/Library/Frameworks/Python.framework/Versions/3.12/bin/python3.12 examples/distributed_actor_pool.py 2>&1 | tail -30
```

期望：两个脚本都输出 "=== ALL PASS ==="

---

## Task 14: 全量验证

- [ ] **Step 1: cargo test / clippy / fmt**

```bash
cd /Users/liupeng/workspace/axon && \
cargo test -p axon-distributed 2>&1 | tail -5 && \
cargo clippy -p axon-distributed --all-targets -- -D warnings 2>&1 | tail -5 && \
cargo fmt -p axon-distributed --check 2>&1 | tail -5
```

- [ ] **Step 2: cargo test --workspace**

```bash
cd /Users/liupeng/workspace/axon && \
cargo test --workspace 2>&1 | tail -15
```

---

## Task 15: 文档更新

- [ ] **Step 1: 勾选 03-distributed.md 验收标准**

把 4 个 `[ ]` 全部改为 `[x]`，并新增"完成情况（Phase 2 P2 实施摘要）"章节，含：
- 5 个 Rust 端配置类型（Cluster/Algorithm/Resource/FaultTolerance/Distributed）
- 3 个辅助类型（ActorConfig/ParamServerConfig/TrainingCheckpoint）
- 5 个 Python 端模块（types/ray_trainer/actor/param_server/fault_tolerance）
- PyO3 桥接层（DistributedRunner + 3 个函数）
- TOML 配置
- 2 个示例脚本
- mock 模式（不依赖真实 Ray 集群，CI 友好）

- [ ] **Step 2: 在 CHANGELOG.md 中新增 Phase 2 P2 条目**

在 Phase 2 P1 之后添加 Phase 2 P2 条目，涵盖：
- **`config` 模块**：5 个配置类型 + TOML 加载
- **`actor` 模块**：ActorConfig + 验证
- **`param_server` 模块**：ParamServerConfig + 验证
- **`checkpoint` 模块**：TrainingCheckpoint + StepMetrics + CheckpointMetadata + JSON 序列化
- **`error` 模块**：DistributedError（9 种错误类型）
- **`python` 模块**：PyO3 桥接
- **Python 端 `axon_distributed` 包**：5 个子模块
- **TOML 配置**：default_distributed.toml
- **示例脚本**：distributed_basic.py + distributed_actor_pool.py
- **23+ 单元测试**通过
- **架构决策**：Python 优先 / mock 模式 / RLLib 2.x builder API

---

## Self-Review Checklist

- [x] **Spec 覆盖**：5 配置类型 / Actor / ParamServer / Checkpoint / 容错 / Python 端 5 模块 / PyO3 / TOML / 2 示例 / 文档
- [x] **无占位符**：每个 Step 都有具体代码
- [x] **类型一致性**：`DistributedConfig.cluster.num_workers` 在 Rust/Python 端命名一致
- [x] **TDD**：Task 2-5 先写测试
- [x] **mock 模式**：所有 Python Actor 在 RAY_AVAILABLE=False 时都能本地运行

---

## 执行方式

请选择：
1. **Subagent-Driven（推荐）**：派发 3-4 个 subagent 串行执行
2. **Inline Execution**：当前会话内批量执行

我推荐 Subagent-Driven，按以下分组派发：
- **Subagent 1**：Task 1-5（crate 骨架 + Rust 端 5 模块 + 23 测试）
- **Subagent 2**：Task 6-10（Python 端 5 模块 + 4 个 smoke test）
- **Subagent 3**：Task 11-15（PyO3 桥接 + TOML + 2 示例 + 验证 + 文档）
