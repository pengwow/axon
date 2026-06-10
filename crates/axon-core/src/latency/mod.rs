//! 延迟模型
//!
//! 回测时模拟真实世界的网络、交易所处理、硬件延迟。
//! 实现固定、正态、指数、均匀、队列、组合六种延迟模型。
//!
//! TDD 规范：[`axon-design/01-tdd/01-phase1-core/12-latency-models.md`](../../../../axon-design/01-tdd/01-phase1-core/12-latency-models.md)
//!
//! # 模块组织
//!
//! - [`traits`]：[`LatencyModel`] trait + [`PathType`] 路径类型 + [`LatencyParams`] 参数摘要
//! - [`constant`]：[`ConstantLatencyModel`] 固定延迟
//! - [`normal`]：[`NormalLatencyModel`] 正态分布延迟（Box-Muller 变换）
//! - [`exponential`]：[`ExponentialLatencyModel`] 指数分布延迟
//! - [`uniform`]：[`UniformLatencyModel`] 均匀分布延迟
//! - [`queue`]：[`QueueLatencyModel`] 队列延迟
//! - [`composite`]：[`CompositeLatencyModel`] 组合延迟
//! - [`factory`]：[`LatencyModelFactory`] 工厂
//! - [`error`]：[`LatencyModelError`] 错误类型

pub mod composite;
pub mod constant;
pub mod error;
pub mod exponential;
pub mod factory;
pub mod normal;
pub mod queue;
pub mod traits;
pub mod uniform;

pub use composite::CompositeLatencyModel;
pub use constant::ConstantLatencyModel;
pub use error::{LatencyModelError, LatencyModelResult};
pub use exponential::ExponentialLatencyModel;
pub use factory::LatencyModelFactory;
pub use normal::NormalLatencyModel;
pub use queue::QueueLatencyModel;
pub use traits::{LatencyModel, LatencyParams, PathType};
pub use uniform::UniformLatencyModel;
