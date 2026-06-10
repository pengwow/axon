//! 动作平滑器：EMA + delta 限制，防止过度交易

/// 动作平滑器
#[derive(Debug, Clone)]
pub struct ActionSmoother {
    /// 最大单步仓位变化
    pub max_delta: f64,
    /// EMA 平滑系数 `(0, 1)`，越大越跟随原始信号
    pub alpha: f64,
    /// 上一步的平滑后值
    pub prev_smoothed: f64,
}

impl ActionSmoother {
    /// 构造动作平滑器
    pub fn new(max_delta: f64, alpha: f64) -> Self {
        Self {
            max_delta,
            alpha,
            prev_smoothed: 0.0,
        }
    }

    /// 平滑动作
    /// 1. EMA 平滑：`smoothed = α * target + (1 - α) * prev`
    /// 2. delta 限制：`abs(smoothed - prev) > max_delta` 时截断
    pub fn smooth(&mut self, target: f64) -> f64 {
        let smoothed = self.alpha * target + (1.0 - self.alpha) * self.prev_smoothed;
        let delta = smoothed - self.prev_smoothed;
        let clamped = if delta.abs() > self.max_delta {
            self.prev_smoothed + delta.signum() * self.max_delta
        } else {
            smoothed
        };
        self.prev_smoothed = clamped;
        clamped
    }

    /// 重置状态
    pub fn reset(&mut self) {
        self.prev_smoothed = 0.0;
    }
}
