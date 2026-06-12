//! E2E 测试公共辅助
//!
//! 提供:
//! - `fixtures_dir()` — fixtures 根目录绝对路径
//! - `fixture_path(test, model, id)` — 单条 fixture 完整路径(用于 assert 文本内容)
//! - `has_key_or_fixture()` — 在 `cargo test` 默认走 Replay 模式,在 `E2E_MODE=live` 时要求有 API key
//! - `deepseek_backend()` — 真实 OpenAI 兼容 backend(需要 `DEEPSEEK_API_KEY`,否则返回 None)
//! - `assert_cost_under()` — 简易成本预算断言(基于 `backends::cost::pricing_for`)
//!
//! ## 运行模式
//!
//! 默认 `E2E_MODE=replay`,所有调用从 `tests/e2e/common/fixtures/{test}/{model}/{key}.json` 读取;
//! 缺 fixture 立即 panic(开发期 loud failure)。
//! 设 `E2E_MODE=record` + `DEEPSEEK_API_KEY=sk-...` 走真实 API + 落盘新 fixture。
//! 设 `E2E_MODE=live` 走真实 API 但不存盘(临时调试)。
//!
//! ## 并发约束
//!
//! 修改环境变量的测试(`has_key_or_fixture_*` / `deepseek_backend_*`)必须配合
//! `cargo test -- --test-threads=1` 跑,否则会因 env var 污染产生误报。
//! 不要在生产代码中使用自定义 `Mutex` 保护 env var —— `std::env::set_var` 内部
//! 已经持有运行时锁,自定义锁会与之死锁。

#![cfg(feature = "e2e")]

use std::path::PathBuf;

use axon_llm::backends::OpenAICompatBackend;
use axon_llm::types::TokenUsage;

// ─── 路径辅助 ─────────────────────────────────────────────

/// fixtures 根目录(`<crate>/tests/e2e/common/fixtures`)
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("e2e")
        .join("common")
        .join("fixtures")
}

/// 单条 fixture 路径(`<root>/<test>/<model>/<id>.json`)
pub fn fixture_path(test: &str, model: &str, id: &str) -> PathBuf {
    fixtures_dir().join(test).join(model).join(format!("{id}.json"))
}

/// 是否具备执行条件:有 API key(fixture 仅是录制产物,无 key 时 skip)
pub fn has_key_or_fixture(_test: &str, _model: &str) -> bool {
    std::env::var("DEEPSEEK_API_KEY").is_ok()
}

/// 构造真实 backend(若缺 API key 返回 None,测试选择 skip)
pub fn deepseek_backend() -> Option<OpenAICompatBackend> {
    let cfg = axon_llm::backends::OpenAICompatConfig::from_env().ok()?;
    Some(OpenAICompatBackend::new(cfg))
}

// ─── 成本断言 ─────────────────────────────────────────────

/// 断言 `usage` 折算 USD 成本 < `budget_usd`
///
/// 未知模型直接 panic(测试 fail loud,不要 silently skip)
pub fn assert_cost_under(usage: &TokenUsage, model: &str, budget_usd: f64) {
    let pricing = axon_llm::backends::pricing_for(model)
        .unwrap_or_else(|| panic!("no pricing for model {model} (test must register first)"));
    let cost = pricing.compute(usage);
    assert!(
        cost < budget_usd,
        "cost ${cost:.6} exceeds budget ${budget_usd} (model={model}, usage={usage:?})"
    );
}
