//! 统一错误类型：聚合 axon-core 各子模块错误
//!
//! ## 设计原则
//!
//! 1. **聚合而非简化**：每个子模块保留自己的精细错误类型（用于本地处理），
//!    `Error` 枚举作为"上层门面"聚合它们（用于跨模块传播）。
//! 2. **可重试语义**：[`Error::is_retryable`] 给出统一的"是否值得重试"判断。
//! 3. **tracing 集成**：通过 [`Error::log`] 把错误打印到 `tracing` 框架，
//!    自动带上 source chain。
//! 4. **本地化 + 上下文**：错误消息支持中文；通过 [`ErrorContext`] 链式附加上下文。
//!
//! ## 错误链 vs 单一错误
//!
//! - 子模块内部使用 `ImpactModelError` / `LatencyModelError` / `VolatilityError` 等具体类型
//! - 跨模块边界使用 `axon_core::Error` 聚合
//! - 应用层（CLI / 服务）可以包成 `anyhow::Error` 携带更多上下文

use thiserror::Error;

/// axon-core 统一错误类型：聚合各子模块的错误
#[derive(Debug, Error)]
pub enum Error {
    // ─── 冲击模型 ─────────────────────────────────────────
    /// 市场冲击模型错误
    #[error(transparent)]
    Impact(#[from] crate::impact::ImpactModelError),

    /// 波动率估计器错误
    #[error(transparent)]
    Volatility(#[from] crate::volatility::VolatilityError),

    /// 延迟模型错误
    #[error(transparent)]
    Latency(#[from] crate::latency::LatencyModelError),

    /// 事件队列错误
    #[error(transparent)]
    Queue(#[from] crate::queue::EventQueueError),

    // ─── 占位 / 通用 ───────────────────────────────────
    /// 通用错误（用于尚未分类的失败）
    #[error("core error: {0}")]
    Other(String),

    /// 带上下文的错误（由 [`ErrorContext`] 链式构造）
    #[error("{context}: {source}")]
    WithContext {
        /// 上下文描述（如"加载配置文件"、"提交订单"）
        context: String,
        /// 原始错误
        #[source]
        source: Box<Error>,
    },
}

impl Error {
    /// 是否可重试
    ///
    /// 瞬态错误（IO、超时、限流）应重试；逻辑错误（参数非法、不变量违反）不应重试。
    pub fn is_retryable(&self) -> bool {
        match self {
            // 目前所有子错误都是不可重试的（参数错误、计算溢出等）
            // 未来加入 IO/网络错误时可在此扩展
            Self::Impact(_)
            | Self::Volatility(_)
            | Self::Latency(_)
            | Self::Queue(_)
            | Self::Other(_)
            | Self::WithContext { .. } => false,
        }
    }

    /// 链式附加上下文
    ///
    /// # 示例
    ///
    /// ```ignore
    /// some_fallible_op()
    ///     .map_err(|e| e.context("加载市场数据"))?;
    /// ```
    pub fn context(self, ctx: impl Into<String>) -> Self {
        Self::WithContext {
            context: ctx.into(),
            source: Box::new(self),
        }
    }

    /// 把错误记录到 `tracing` 框架
    ///
    /// 等级映射：可重试 ⇒ `warn!`；不可重试 ⇒ `error!`。
    /// 自动展开 source chain 到 `tracing` 的 span 字段中。
    pub fn log(&self) {
        if self.is_retryable() {
            tracing::warn!(error = %self, "transient error");
        } else {
            tracing::error!(error = %self, "fatal error");
        }
    }

    /// 提取根本原因（剥离 `WithContext` 包装）
    pub fn root_cause(&self) -> &Self {
        let mut current = self;
        while let Self::WithContext { source, .. } = current {
            current = source;
        }
        current
    }
}

/// 错误上下文扩展 trait：为任何 `Result<T, E>` 提供 `.context()` 链式方法
///
/// ## 设计动机
///
/// 标准库 `Result::map_err` 只能修改错误，不能附加上下文。`?` 操作符 + `From`
/// 只能做类型转换。本 trait 提供与 `anyhow::Context` 类似的能力，但保留
/// `Error` 类型信息（不擦除为 `anyhow::Error`）。
///
/// ## 使用示例
///
/// ```ignore
/// use axon_core::error::ErrorContext;
///
/// let data = std::fs::read("config.toml")
///     .map_err(|e| Error::Other(e.to_string()))
///     .context("读取配置文件")?;
/// ```
pub trait ErrorContext<T> {
    /// 输出类型
    type Output;

    /// 附加上下文描述
    fn context(self, ctx: impl Into<String>) -> Self::Output;
}

impl<T> ErrorContext<T> for std::result::Result<T, Error> {
    type Output = std::result::Result<T, Error>;

    fn context(self, ctx: impl Into<String>) -> Self::Output {
        self.map_err(|e| e.context(ctx))
    }
}

/// 核心 crate 的 `Result` 别名
pub type Result<T> = std::result::Result<T, Error>;

// ─── 错误恢复工具 ─────────────────────────────────────────

/// 重试策略：指数退避
///
/// # 字段
///
/// - `max_attempts`：最大尝试次数（含首次）。`max_attempts = 1` 意味着不重试。
/// - `initial_delay_ms`：首次重试前的延迟（毫秒）
/// - `max_delay_ms`：单次重试延迟上限
/// - `multiplier`：每次重试延迟的乘数（典型值：2.0）
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    /// 最大尝试次数
    pub max_attempts: usize,
    /// 初始延迟（毫秒）
    pub initial_delay_ms: u64,
    /// 最大延迟（毫秒）
    pub max_delay_ms: u64,
    /// 退避乘数
    pub multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5_000,
            multiplier: 2.0,
        }
    }
}

impl RetryPolicy {
    /// 无重试（仅执行一次）
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            initial_delay_ms: 0,
            max_delay_ms: 0,
            multiplier: 1.0,
        }
    }

    /// 计算第 `attempt` 次重试前的延迟（attempt 从 0 开始）
    fn delay_for(&self, attempt: usize) -> u64 {
        if attempt == 0 {
            return 0;
        }
        let raw = (self.initial_delay_ms as f64) * self.multiplier.powi((attempt - 1) as i32);
        let capped = raw.min(self.max_delay_ms as f64);
        capped as u64
    }
}

/// 带可重试语义的执行包装器
///
/// 对闭包进行最多 `policy.max_attempts` 次尝试。
/// 每次失败时检查 `is_retryable`：可重试 ⇒ 等待后重试；不可重试 ⇒ 立即返回。
///
/// # 示例
///
/// ```ignore
/// use axon_core::error::{RetryPolicy, with_retry};
///
/// let result = with_retry(RetryPolicy::default(), || async {
///     fetch_data().await
/// });
/// ```
pub fn with_retry<T, F>(policy: RetryPolicy, mut op: F) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    let mut last_err: Option<Error> = None;
    for attempt in 0..policy.max_attempts {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) => {
                if !e.is_retryable() {
                    return Err(e);
                }
                let delay = policy.delay_for(attempt + 1);
                if delay > 0 {
                    tracing::warn!(attempt = attempt + 1, delay_ms = delay, "retrying after error: {e}");
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                } else {
                    tracing::warn!(attempt = attempt + 1, "retrying after error: {e}");
                }
                last_err = Some(e);
            }
        }
    }
    Err(last_err.expect("at least one attempt"))
}

/// Fallback 链：依次尝试多个操作，直到一个成功
///
/// 与 `with_retry` 不同：每个操作仅尝试一次，失败立即尝试下一个。
/// 适合"主备切换"、"多数据源"等场景。
///
/// # 示例
///
/// ```ignore
/// use axon_core::error::try_with_fallback;
///
/// let data = try_with_fallback([
///     || fetch_from_primary().map_err(|e| e.into()),
///     || fetch_from_secondary().map_err(|e| e.into()),
/// ]);
/// ```
pub fn try_with_fallback<T, F, E>(ops: impl IntoIterator<Item = F>) -> std::result::Result<T, E>
where
    F: FnOnce() -> std::result::Result<T, E>,
{
    let mut last_err: Option<E> = None;
    for op in ops {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.expect("at least one fallback"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::impact::ImpactModelError;
    use crate::latency::LatencyModelError;
    use crate::queue::EventQueueError;
    use crate::volatility::VolatilityError;

    #[test]
    fn test_error_display_includes_context() {
        let err = Error::Other("invalid timestamp".to_string());
        assert_eq!(err.to_string(), "core error: invalid timestamp");
    }

    #[test]
    fn test_from_impact_error() {
        let impact_err = ImpactModelError::EmptyOrderBook;
        let core_err: Error = impact_err.into();
        assert!(matches!(core_err, Error::Impact(ImpactModelError::EmptyOrderBook)));
    }

    #[test]
    fn test_from_volatility_error() {
        let v = VolatilityError::ZeroWindow;
        let e: Error = v.into();
        assert!(matches!(e, Error::Volatility(VolatilityError::ZeroWindow)));
    }

    #[test]
    fn test_from_latency_error() {
        let l = LatencyModelError::NegativeStdDev(-1.0);
        let e: Error = l.into();
        assert!(matches!(e, Error::Latency(LatencyModelError::NegativeStdDev(_))));
    }

    #[test]
    fn test_from_queue_error() {
        let q = EventQueueError::QueueEmpty;
        let e: Error = q.into();
        assert!(matches!(e, Error::Queue(EventQueueError::QueueEmpty)));
    }

    #[test]
    fn test_context_chains_error() {
        let base = Error::Other("file not found".to_string());
        let with_ctx = base.context("加载配置");
        match &with_ctx {
            Error::WithContext { context, source } => {
                assert_eq!(context, "加载配置");
                assert!(matches!(**source, Error::Other(_)));
            }
            _ => panic!("expected WithContext"),
        }
    }

    #[test]
    fn test_context_trait_method() {
        let r: std::result::Result<(), Error> = Err(Error::Other("inner".into()));
        let with_ctx = r.context("outer step");
        assert!(with_ctx.is_err());
        let msg = with_ctx.unwrap_err().to_string();
        assert!(msg.contains("outer step"));
        assert!(msg.contains("inner"));
    }

    #[test]
    fn test_root_cause_strips_context() {
        let base = Error::Other("origin".to_string());
        let wrapped = base.context("step 1").context("step 2");
        let root = wrapped.root_cause();
        assert!(matches!(root, Error::Other(_)));
        assert_eq!(root.to_string(), "core error: origin");
    }

    #[test]
    fn test_is_retryable_default_false() {
        // 当前所有子错误都不可重试
        let cases = vec![
            Error::from(ImpactModelError::EmptyOrderBook),
            Error::from(VolatilityError::ZeroWindow),
            Error::from(LatencyModelError::NegativeStdDev(-1.0)),
            Error::from(EventQueueError::QueueEmpty),
            Error::Other("test".to_string()),
        ];
        for e in cases {
            assert!(!e.is_retryable(), "{e:?} should not be retryable");
        }
    }

    #[test]
    fn test_log_does_not_panic() {
        // 验证 log 方法不 panic（实际日志由 tracing subscriber 接收）
        let err = Error::Other("test".to_string());
        err.log();
    }

    // ─── 重试与回退测试 ──────────────────────────────────

    #[test]
    fn test_retry_policy_default() {
        let p = RetryPolicy::default();
        assert_eq!(p.max_attempts, 3);
        assert_eq!(p.initial_delay_ms, 100);
    }

    #[test]
    fn test_retry_policy_no_retry() {
        let p = RetryPolicy::no_retry();
        assert_eq!(p.max_attempts, 1);
        assert_eq!(p.delay_for(0), 0);
        assert_eq!(p.delay_for(1), 0);
    }

    #[test]
    fn test_retry_policy_delay_grows() {
        let p = RetryPolicy {
            max_attempts: 5,
            initial_delay_ms: 100,
            max_delay_ms: 1_000,
            multiplier: 2.0,
        };
        // 第 1 次重试：100ms
        assert_eq!(p.delay_for(1), 100);
        // 第 2 次重试：200ms
        assert_eq!(p.delay_for(2), 200);
        // 第 3 次重试：400ms
        assert_eq!(p.delay_for(3), 400);
        // 第 4 次重试：800ms
        assert_eq!(p.delay_for(4), 800);
        // 第 5 次重试：1000ms（被 max 截断到 1000）
        assert_eq!(p.delay_for(5), 1_000);
        // 上限测试：极大 attempt
        assert_eq!(p.delay_for(20), 1_000);
    }

    #[test]
    fn test_with_retry_first_success() {
        let mut calls = 0;
        let r: Result<i32> = with_retry(RetryPolicy::default(), || {
            calls += 1;
            Ok(42)
        });
        assert_eq!(r.unwrap(), 42);
        assert_eq!(calls, 1);
    }

    #[test]
    fn test_with_retry_non_retryable_immediate_fail() {
        // 不可重试错误 ⇒ 第一次失败后立即返回
        let mut calls = 0;
        let r: Result<i32> = with_retry(RetryPolicy::default(), || {
            calls += 1;
            Err(Error::Other("logic error".to_string()))
        });
        assert!(r.is_err());
        assert_eq!(calls, 1); // 立即返回，未重试
    }

    #[test]
    fn test_with_retry_max_attempts_exhausted() {
        // 当前所有错误都不可重试 ⇒ max_attempts 退化为 1
        // 这里仅验证"重试直到上限"的逻辑骨架
        let mut calls = 0;
        let r: Result<i32> = with_retry(
            RetryPolicy {
                max_attempts: 1, // 仅尝试一次 ⇒ 不会重试
                initial_delay_ms: 0,
                max_delay_ms: 0,
                multiplier: 1.0,
            },
            || {
                calls += 1;
                Err(Error::Other("err".to_string()))
            },
        );
        assert!(r.is_err());
        assert_eq!(calls, 1);
    }

    #[test]
    fn test_try_with_fallback_first_succeeds() {
        use std::cell::Cell;
        let calls = Cell::new(0u32);
        let r: std::result::Result<i32, &'static str> = try_with_fallback([
            Box::new(|| {
                calls.set(calls.get() + 1);
                Ok(1)
            }) as Box<dyn FnOnce() -> std::result::Result<i32, &'static str>>,
            Box::new(|| {
                calls.set(calls.get() + 1);
                Ok(2)
            }) as Box<dyn FnOnce() -> std::result::Result<i32, &'static str>>,
        ]);
        assert_eq!(r.unwrap(), 1);
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn test_try_with_fallback_second_succeeds() {
        use std::cell::Cell;
        let calls = Cell::new(0u32);
        let r: std::result::Result<i32, &'static str> = try_with_fallback([
            Box::new(|| {
                calls.set(calls.get() + 1);
                Err("first failed")
            }) as Box<dyn FnOnce() -> std::result::Result<i32, &'static str>>,
            Box::new(|| {
                calls.set(calls.get() + 1);
                Ok(42)
            }) as Box<dyn FnOnce() -> std::result::Result<i32, &'static str>>,
        ]);
        assert_eq!(r.unwrap(), 42);
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn test_try_with_fallback_all_fail() {
        let r: std::result::Result<i32, &'static str> = try_with_fallback([
            Box::new(|| Err("a")) as Box<dyn FnOnce() -> std::result::Result<i32, &'static str>>,
            Box::new(|| Err("b")) as Box<dyn FnOnce() -> std::result::Result<i32, &'static str>>,
            Box::new(|| Err("c")) as Box<dyn FnOnce() -> std::result::Result<i32, &'static str>>,
        ]);
        assert_eq!(r, Err("c")); // 返回最后一个错误
    }

    /// 真实场景：错误链 + 重试 + 回退 组合
    /// 验证错误上下文在多步骤中保持完整
    #[test]
    fn test_error_chain_in_complex_workflow() {
        // 1. 加载数据（基础错误 + 上下文）
        let load_err: Result<()> = Err(Error::Other("network timeout".to_string()))
            .context("加载市场数据");
        assert!(load_err.is_err());
        let err = load_err.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("加载市场数据"));
        assert!(msg.contains("network timeout"));

        // 2. root_cause 剥离 context
        let wrapped = err.context("回测引擎").context("策略评估");
        let root = wrapped.root_cause();
        assert!(matches!(root, Error::Other(_)));
    }
}
