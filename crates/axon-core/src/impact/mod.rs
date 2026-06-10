//! 市场冲击模型
//!
//! 回测时模拟订单对市场价格的冲击，避免回测结果过于乐观。
//! 实现线性冲击、幂律冲击（square-root law）、自适应冲击三种模型。
//!
//! TDD 规范：[`axon-design/01-tdd/01-phase1-core/06-impact-models.md`](../../../../axon-design/01-tdd/01-phase1-core/06-impact-models.md)
//!
//! # 模块组织
//!
//! - [`types`]：[`Impact`] 冲击结果结构 / [`ImpactModelConfig`] 配置
//! - [`traits`]：[`ImpactModel`] 模型 trait + 工厂函数
//! - [`linear`]：[`LinearImpactModel`] 线性冲击
//! - [`power_law`]：[`PowerLawImpactModel`] 幂律冲击
//! - [`adaptive`]：[`AdaptiveImpactModel`] 自适应冲击
//! - [`error`]：[`ImpactModelError`] 错误类型

pub mod adaptive;
pub mod error;
pub mod linear;
pub mod power_law;
pub mod traits;
pub mod types;

pub use adaptive::AdaptiveImpactModel;
pub use error::{ImpactModelError, ImpactModelResult};
pub use linear::LinearImpactModel;
pub use power_law::PowerLawImpactModel;
pub use traits::{create_model, linear_impact, sqrt_impact, ImpactModel};
pub use types::{Impact, ImpactModelConfig};
