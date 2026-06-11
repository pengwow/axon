//! AXON Phase 2 集成测试
//!
//! 本 crate 验证 Phase 2 各模块的端到端协作：
//! - HPO（超参优化）+ Tracker（实验追踪）：超参搜索过程中的实时指标记录
//! - Walk-forward（滚动前向验证）+ Registry（模型注册）：验证后的最佳模型自动注册
//! - Tracker + Registry：根据追踪的指标决策阶段转换（staging → production）
//! - HPO 多目标 + Pareto + Tracker：多目标优化的指标追踪与前沿选择
//! - 端到端训练管线：HPO → Walk-forward → Tracker → Registry 全链路
//!
//! ## 模块规划
//!
//! | 测试模块 | 涉及 crate | 场景 |
//! |---------|-----------|------|
//! | [`hpo_tracker`] | axon-hpo + axon-tracker | 超参搜索 + 指标记录 |
//! | [`walkforward_registry`] | axon-walk-forward + axon-registry | 验证后注册 |
//! | [`tracker_registry`] | axon-tracker + axon-registry | 指标驱动阶段转换 |
//! | [`multi_objective`] | axon-hpo + axon-tracker | Pareto 前沿追踪 |
//! | [`e2e_pipeline`] | 所有 4 个 Phase 2 crate | 端到端训练管线 |

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

/// 共享的测试辅助函数与 fixture
pub mod fixtures;

/// 集成测试模块（按 crate 维度组织）
pub mod hpo_tracker;
pub mod walkforward_registry;
pub mod tracker_registry;
pub mod multi_objective;
pub mod e2e_pipeline;
