//! 波动率估计器 trait

use super::error::VolatilityResult;

/// 波动率估计源
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolatilitySource {
    /// 简单收益率：`r_t = (P_t - P_{t-1}) / P_{t-1}`
    SimpleReturn,
    /// 对数收益率：`r_t = ln(P_t / P_{t-1})`
    LogReturn,
    /// 直接提供收益率（跳过价格 → 收益率转换）
    Precomputed,
}

/// 波动率估计器 trait
///
/// 实现方提供：
/// - `update(returns)`：增量更新内部状态
/// - `current_volatility()`：返回当前估计的波动率（年化或原始，取决于实现）
/// - `reset()`：重置状态
pub trait VolatilityEstimator: Send + Sync {
    /// 用新收益率更新估计器
    fn update(&mut self, return_value: f64) -> VolatilityResult<()>;

    /// 用新价格增量更新（自动转换为收益率）
    fn update_price(&mut self, price: f64) -> VolatilityResult<()> {
        // 默认实现：维护上一个价格，转换为简单收益率后调用 update
        if price <= 0.0 {
            return Err(super::error::VolatilityError::InvalidInput(
                "价格必须 > 0".to_string(),
            ));
        }
        // 子类应重写以维护自身状态
        self.update(0.0)
    }

    /// 返回当前波动率估计（每 sqrt(period) 的标准差）
    ///
    /// `period` 通常是 1（每步），具体语义由实现方定义
    fn current_volatility(&self) -> VolatilityResult<f64>;

    /// 估计器是否已就绪（有足够数据产生有意义估计）
    fn is_ready(&self) -> bool;

    /// 重置状态
    fn reset(&mut self);

    /// 模型名称
    fn name(&self) -> &str;
}
