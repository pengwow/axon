//! 回测引擎主循环（占位实现）
//!
//! Phase 1A 阶段将实现事件驱动的回测主循环，
//! 详见 [`axon-design/01-tdd/01-phase1-core/08-scheduler.md`](../../../../../axon-design/01-tdd/01-phase1-core/08-scheduler.md)。

/// 回测引擎占位类型
#[derive(Debug, Default, Clone)]
pub struct BacktestEngine {
    // Phase 1A 将填充：事件队列、调度器、撮合器等字段
}

impl BacktestEngine {
    /// 创建新的回测引擎实例（占位）
    pub fn new() -> Self {
        Self::default()
    }

    /// 运行回测（占位实现）
    ///
    /// Phase 1A 将实现完整的事件循环与结果统计。
    pub fn run(&mut self) -> RunResult {
        RunResult::default()
    }
}

/// 回测运行结果（占位）
#[derive(Debug, Default, Clone)]
pub struct RunResult {
    // Phase 1A 将填充：总收益、Sharpe、最大回撤等指标
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backtest_engine_constructs() {
        let mut engine = BacktestEngine::new();
        let result = engine.run();
        assert!(format!("{:?}", result).contains("RunResult"));
    }
}
