//! 延迟模型 trait 与公共类型

use std::collections::HashMap;
use std::time::Duration;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// 通信路径类型：不同路径具有不同的延迟特征
///
/// 行情数据通常延迟低且稳定，订单提交/取消需要往返交易所，
/// 账户查询与心跳各自具有独立的网络往返特征。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PathType {
    /// 行情数据接收
    MarketData,
    /// 订单提交
    OrderSubmit,
    /// 订单取消
    OrderCancel,
    /// 账户查询
    AccountQuery,
    /// WebSocket 心跳
    Heartbeat,
}

impl PathType {
    /// 所有路径类型（用于遍历初始化 HashMap）
    pub const ALL: [PathType; 5] = [
        PathType::MarketData,
        PathType::OrderSubmit,
        PathType::OrderCancel,
        PathType::AccountQuery,
        PathType::Heartbeat,
    ];

    /// 路径名称（用于日志与序列化）
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            PathType::MarketData => "market_data",
            PathType::OrderSubmit => "order_submit",
            PathType::OrderCancel => "order_cancel",
            PathType::AccountQuery => "account_query",
            PathType::Heartbeat => "heartbeat",
        }
    }
}

/// 延迟参数摘要
///
/// 提供统一的模型元信息，方便日志、报告与可观测性。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub struct LatencyParams {
    /// 模型类型标识（"constant" / "normal" / ...）
    pub model_type: String,
    /// 基础延迟（毫秒）
    pub base_delay_ms: f64,
    /// 抖动（标准差或区间半宽，毫秒；固定模型为 None）
    pub jitter_ms: Option<f64>,
    /// 各路径延迟覆盖（毫秒）
    pub path_overrides: HashMap<PathType, f64>,
}

impl LatencyParams {
    /// 构造空参数（用于占位）
    pub fn empty(model_type: &str) -> Self {
        Self {
            model_type: model_type.to_string(),
            base_delay_ms: 0.0,
            jitter_ms: None,
            path_overrides: HashMap::new(),
        }
    }
}

/// 延迟模型 trait
///
/// 实现方需提供 `sample_delay`（按路径采样延迟）与元信息。
/// 要求 `Send + Sync` 以便在多线程回测中安全共享。
pub trait LatencyModel: Send + Sync {
    /// 根据路径类型采样延迟
    fn sample_delay(&self, path: PathType) -> Duration;

    /// 模型名称（用于日志与调试）
    fn name(&self) -> &str;

    /// 模型参数摘要
    fn params(&self) -> LatencyParams;
}
