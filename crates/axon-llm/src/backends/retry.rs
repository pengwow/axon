//! LLM 调用重试(指数退避 + jitter)
//!
//! [`with_backoff`] 包装一个返回 `Result<T, LLMError>` 的 future,失败时按
//! `is_retryable()` 决定是否重试,延迟 = `base * 2^attempt + ±10% jitter`。
//!
//! 不会重试:
//! - `LLMError::Auth`(认证错误,token 无效)
//! - `LLMError::Parse`(响应格式错误)
//! - `LLMError::ContextOverflow`(窗口溢出,重试无意义)
//! - `LLMError::MockExhausted`
//!
//! 会重试:
//! - `LLMError::Network(_)`(瞬时网络问题)
//! - `LLMError::RateLimited { retry_after }`(优先用 retry_after)
//! - `LLMError::Backend(_)`(5xx 错误)

use crate::backend::LLMError;
use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;

mod rand_jitter {
    use rand::Rng;
    /// 返回 `±10%` 的 jitter 倍数(0.9 - 1.1)
    pub fn factor() -> f64 {
        let mut rng = rand::thread_rng();
        rng.gen_range(0.9..1.1)
    }
}

/// 重试策略配置
#[derive(Debug, Clone, Copy)]
pub struct BackoffConfig {
    /// 第一次重试前等待
    pub initial_delay: Duration,
    /// 最多重试次数(不含首次)
    pub max_retries: u32,
    /// 单次延迟上限(防止退避过长)
    pub max_delay: Duration,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(500),
            max_retries: 3,
            max_delay: Duration::from_secs(10),
        }
    }
}

impl BackoffConfig {
    /// 不重试(用于测试)
    pub fn no_retry() -> Self {
        Self {
            initial_delay: Duration::from_millis(0),
            max_retries: 0,
            max_delay: Duration::from_millis(0),
        }
    }
}

/// 退避执行
///
/// 失败时若 `err.is_retryable()` 返回 true 且未达 `max_retries`,
/// 等待 `initial_delay * 2^attempt * jitter` 后重试。
/// `RateLimited` 时使用服务端 `retry_after` 覆盖。
pub async fn with_backoff<F, Fut, T>(cfg: BackoffConfig, mut op: F) -> Result<T, LLMError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, LLMError>>,
{
    let mut attempt: u32 = 0;
    loop {
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                if attempt >= cfg.max_retries || !e.is_retryable() {
                    return Err(e);
                }

                // 计算延迟
                let base_ms = cfg.initial_delay.as_millis() as u64;
                let delay_ms = base_ms.saturating_mul(1u64 << attempt.min(20));
                let jittered_ms = (delay_ms as f64 * rand_jitter::factor()) as u64;

                // RateLimited 时:server 给了 retry_after 且小于 backoff,优先用 server
                let final_delay_ms = if let LLMError::RateLimited {
                    retry_after: Some(secs),
                } = &e
                {
                    let server_ms = secs.saturating_mul(1000);
                    if server_ms < jittered_ms {
                        server_ms
                    } else {
                        jittered_ms
                    }
                } else {
                    jittered_ms
                };

                let delay = Duration::from_millis(final_delay_ms.min(cfg.max_delay.as_millis() as u64));
                sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn no_retry_returns_first_error() {
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let res: Result<(), LLMError> = with_backoff(BackoffConfig::no_retry(), move || {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err(LLMError::Network("flaky".into()))
            }
        })
        .await;
        assert!(res.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retries_then_succeeds() {
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let cfg = BackoffConfig {
            initial_delay: Duration::from_millis(1),
            max_retries: 3,
            max_delay: Duration::from_millis(10),
        };
        let res: Result<u32, LLMError> = with_backoff(cfg, move || {
            let c = c.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    Err(LLMError::Network("flaky".into()))
                } else {
                    Ok(42)
                }
            }
        })
        .await;
        assert_eq!(res.unwrap(), 42);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn does_not_retry_non_retryable() {
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let cfg = BackoffConfig {
            initial_delay: Duration::from_millis(1),
            max_retries: 3,
            max_delay: Duration::from_millis(10),
        };
        let res: Result<(), LLMError> = with_backoff(cfg, move || {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err(LLMError::Auth("bad token".into()))
            }
        })
        .await;
        assert!(res.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn rate_limited_uses_retry_after() {
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let cfg = BackoffConfig {
            initial_delay: Duration::from_millis(5000),
            max_retries: 1,
            max_delay: Duration::from_millis(60_000),
        };
        let start = std::time::Instant::now();
        let res: Result<u32, LLMError> = with_backoff(cfg, move || {
            let c = c.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    Err(LLMError::RateLimited {
                        retry_after: Some(0), // 0 秒,立即重试
                    })
                } else {
                    Ok(1)
                }
            }
        })
        .await;
        let elapsed = start.elapsed();
        assert_eq!(res.unwrap(), 1);
        // 0 秒 server retry_after,应远小于 initial_delay(5000ms)
        assert!(elapsed.as_secs() < 2, "took too long: {elapsed:?}");
    }
}
