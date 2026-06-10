//! 环形缓冲区：维护最近 N 个 `MarketState`
//!
//! 提供 push / pop / window / 统计聚合（mean / std）等 O(1) 或 O(N) 操作。

use std::collections::VecDeque;

use crate::observation::types::MarketState;

/// 环形缓冲区
#[derive(Debug, Clone)]
pub struct TickBuffer {
    /// 内部缓冲区
    buffer: VecDeque<MarketState>,
    /// 容量上限
    capacity: usize,
}

impl TickBuffer {
    /// 构造指定容量的缓冲区
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity + 1),
            capacity,
        }
    }

    /// 推入新 tick（满则弹出最旧）
    pub fn push(&mut self, state: MarketState) {
        if self.buffer.len() == self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(state);
    }

    /// 返回最近 `window` 个 tick 的引用
    pub fn window(&self, window: usize) -> Vec<&MarketState> {
        let start = self.buffer.len().saturating_sub(window);
        self.buffer.range(start..).collect()
    }

    /// 窗口内指定字段的均值
    pub fn mean_of(&self, window: usize, extractor: fn(&MarketState) -> f64) -> Option<f64> {
        let ticks = self.window(window);
        if ticks.is_empty() {
            return None;
        }
        let sum: f64 = ticks.iter().map(|t| extractor(t)).sum();
        Some(sum / ticks.len() as f64)
    }

    /// 窗口内指定字段的标准差
    pub fn std_of(&self, window: usize, extractor: fn(&MarketState) -> f64) -> Option<f64> {
        let ticks = self.window(window);
        if ticks.len() < 2 {
            return None;
        }
        let values: Vec<f64> = ticks.iter().map(|t| extractor(t)).collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        Some(var.sqrt())
    }

    /// 当前元素数
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// 清空
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// 容量
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// 返回所有元素的不可变引用
    pub fn iter(&self) -> impl Iterator<Item = &MarketState> {
        self.buffer.iter()
    }

    /// 转为 Vec
    pub fn to_vec(&self) -> Vec<MarketState> {
        self.buffer.iter().cloned().collect()
    }
}
