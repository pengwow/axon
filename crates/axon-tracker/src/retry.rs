//! 重试策略（同步版）

use std::thread::sleep;
use std::time::Duration;

use crate::error::TrackerError;

/// 重试策略
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// 最大重试次数
    pub max_retries: u32,
    /// 基础延迟
    pub base_delay: Duration,
    /// 最大延迟
    pub max_delay: Duration,
    /// 退避因子
    pub backoff_factor: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_factor: 2.0,
        }
    }
}

impl RetryPolicy {
    /// 同步执行（带重试）
    pub fn execute<F, T>(&self, mut f: F) -> Result<T, TrackerError>
    where
        F: FnMut() -> Result<T, TrackerError>,
    {
        let mut last_err: Option<TrackerError> = None;
        for attempt in 0..=self.max_retries {
            match f() {
                Ok(val) => return Ok(val),
                Err(e) if e.is_retryable() && attempt < self.max_retries => {
                    let delay = self
                        .base_delay
                        .mul_f64(self.backoff_factor.powi(attempt as i32))
                        .min(self.max_delay);
                    sleep(delay);
                    last_err = Some(e);
                }
                Err(e) => return Err(e),
            }
        }
        Err(last_err.expect("at least one attempt"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_default_retry_policy() {
        let p = RetryPolicy::default();
        assert_eq!(p.max_retries, 3);
    }

    #[test]
    fn test_retry_eventually_succeeds() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();
        let policy = RetryPolicy {
            max_retries: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            backoff_factor: 2.0,
        };
        let result: Result<i32, TrackerError> = policy.execute(|| {
            let n = counter_clone.fetch_add(1, Ordering::SeqCst);
            if n < 2 {
                Err(TrackerError::Network("transient".into()))
            } else {
                Ok(42)
            }
        });
        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_retry_non_retryable_error() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();
        let policy = RetryPolicy::default();
        let result: Result<(), TrackerError> = policy.execute(|| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Err(TrackerError::Auth("forbidden".into()))
        });
        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 1); // 不重试
    }
}
