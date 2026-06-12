# Changelog

axon-llm 的所有重要变更都会记录在这里。格式基于 [Keep a Changelog](https://keepachangelog.com/)。

## [Unreleased]

### Added

- **`backends` feature**:OpenAI 兼容 LLM backend(`OpenAICompatBackend`),支持 DeepSeek / OpenAI / 本地推理服务
  - Bearer 认证、JSON 协议、SSE 流式响应
  - 指数退避 + jitter 重试(`backoff` 模块)
  - Token → USD 成本跟踪(`cost` 模块,含 `pricing_for` / `register_pricing` / `CostTracker`)
  - HTTP 录制/回放中间件(`recording` 模块,`Mode::Replay` / `Record` / `Passthrough`)
  - SSE 字节流 → `TokenDelta` 解析(`streaming` 模块)
- **`e2e` feature**:wiremock 接入(目前用于 e2e 调试)
- **`demo` feature**:真实 LLM 端到端 demo
  - `examples/live_trading_demo.rs`:从 `demo/bin/config.toml` 读配置,跑一次"对话 + 工具调用 + 解析"全流程
  - `demo/bin/config.toml`:DeepSeek 默认配置(base_url / model / max_tokens / temperature / timeout)
- **`MockBackend`**:预编程响应序列的测试 backend(`backends::MockBackend::text_only` / `with_responses`)
- **真实 LLM e2e 测试**:
  - `tests/e2e_simple_chat_test.rs` — 单轮对话
  - `tests/e2e_tool_calling_test.rs` — 工具调用
  - `tests/e2e_react_loop_test.rs` — 多轮 ReAct
  - `tests/e2e_explain_e2e_test.rs` — 解释集成(LLM 主动调 `compute_explanation`)
  - `tests/e2e/common/mod.rs` — 公共辅助(fixtures_dir / has_key_or_fixture / deepseek_backend / assert_cost_under / serial)
  - `tests/e2e/common/fixtures/{test}/{model}/{key}.json` — 录制产物(4 个场景)
  - `tests/e2e_common_smoke_test.rs` — `common` 模块自身 smoke test
- **CI 集成**:
  - `.github/workflows/e2e-real-llm.yml` — 3 个 job(replay-tests / validate-fixtures / record-freshness)
  - `scripts/record-fixtures.sh` — 录制入口
  - `scripts/validate-fixtures.py` — 8 项基本 + 4 项 `--strict` 校验
- **文档**:
  - `crates/axon-llm/docs/e2e-testing.md` — E2E 测试总览
  - `crates/axon-llm/docs/recording.md` — 录制/回放中间件详解

### Changed

- `LLMError::RateLimited` 从 unit variant 改为 struct variant `{ retry_after: Option<u64> }`,携带服务端 `Retry-After` 头
- `axon-llm` 新增 `backends` / `e2e` / `demo` feature(默认关闭,保持基础构建精简)
- `Cargo.toml` 加入 `reqwest` / `tokio-stream` / `async-stream` / `sha2` / `hex` / `rand` / `wiremock` / `toml` / `bytes` / `futures-core` 作为可选依赖

### Fixed

- 修复了 2026-06-12 对话上下文丢失后重建的"两路并行" e2e 测试覆盖
  - 之前 4 个 fixture 文件 + 录制/校验脚本 + CI workflow + demo 全部丢失,本次重生成
  - 修复 MockBackend 缺失问题(原代码注释说"在测试代码中"但实际未实现)

## 验证

```bash
# 单元测试(workspace 整体)
cargo test --workspace

# e2e 测试(需 DEEPSEEK_API_KEY)
cargo test -p axon-llm --features "backends e2e" --test e2e_simple_chat_test

# 录制
./crates/axon-llm/scripts/record-fixtures.sh

# 校验
python3 scripts/validate-fixtures.py --strict crates/axon-llm/tests/e2e/common/fixtures
```

## 已知问题

- `live_trading_demo` 在 `cargo build --features demo` 下可能有 4 处编译错误(async_trait_trait import / reqwest 依赖 / execute trait lifetime 等),与本次变更无关,留待下一轮清理。
