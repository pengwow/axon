# LLM 端到端测试（E2E Testing）

axon-llm 的 e2e 测试采用"录制 / 回放"模式,避免每次 CI 都消耗真实 token。

## 核心思想

- **录制（Record）**:本地开发时,设 `E2E_MODE=record` + `DEEPSEEK_API_KEY=sk-...`,真实调用 LLM 并把响应落盘到 fixture 文件。
- **回放（Replay）**:CI 默认模式 `E2E_MODE=replay`,从 fixture 文件读响应,**不消耗 token,也不依赖网络**。
- **活路（Live）**:调试用,`E2E_MODE=live`,真实调用但不存盘。

## 三种 E2E_MODE

| 模式 | 行为 | 何时用 |
| --- | --- | --- |
| `replay`（默认） | 命中 fixture → 返响应;miss → panic | CI、回归测试 |
| `record` | 调真实 LLM + 落盘 fixture;本地开发主动录制 | 新增 e2e 用例 / fixture 过期 |
| `live` | 调真实 LLM,不存盘 | 临时调试、验证 API 可用性 |

## 跑 E2E 测试

```bash
# 默认 Replay 模式(需先有 fixture,否则 panic)
cargo test -p axon-llm --features "backends e2e" --test e2e_simple_chat_test

# 录制(本地 + 真实 key)
export DEEPSEEK_API_KEY=sk-...
export E2E_MODE=record
cargo test -p axon-llm --features "backends e2e" --test e2e_simple_chat_test

# 活路
export DEEPSEEK_API_KEY=sk-...
export E2E_MODE=live
cargo test -p axon-llm --features "backends e2e" --test e2e_simple_chat_test
```

## Fixture 目录结构

```
crates/axon-llm/tests/e2e/common/fixtures/
├── simple_chat/
│   └── deepseek-chat/
│       └── greeting_001.json
├── tool_calling/
│   └── deepseek-chat/
│       └── submit_order_001.json
├── react_loop/
│   └── deepseek-chat/
│       └── step1.json
└── explain_e2e/
    └── deepseek-chat/
        └── step1.json
```

每个 fixture 文件:
- `version`: 格式版本,当前为 1
- `recorded_at`: `epoch:<unix_secs>`,便于 freshness 检查
- `model`: 当时调用的模型名
- `request`: URL + method + headers(已脱敏) + body
- `response`: status + headers(已脱敏) + body

## 录制新 Fixture

```bash
export DEEPSEEK_API_KEY=sk-...
./scripts/record-fixtures.sh            # 全部 4 个场景
./scripts/record-fixtures.sh --clean    # 录制前先清空
./scripts/record-fixtures.sh --test e2e_simple_chat_test  # 只录一个
```

## 校验 Fixture

```bash
python3 scripts/validate-fixtures.py crates/axon-llm/tests/e2e/common/fixtures
python3 scripts/validate-fixtures.py --strict crates/axon-llm/tests/e2e/common/fixtures
```

`--strict` 额外检查:
- 请求头不含 `Authorization` / `x-api-key`(已脱敏)
- 响应头不含 `set-cookie`
- 文件名 key = SHA256(url + method + canonical(body)) 前 12 hex,与 `recording.rs::RecordingLayer::fixture_key` 一致
- 单文件 < 5MB

## 添加新 E2E 场景

1. 在 `tests/e2e_<name>_test.rs` 写测试,使用 `RecordingLayer` + `LLMBackend::complete[_with_tools]`
2. 跑 `E2E_MODE=record` 录制首次响应
3. 提交 fixture 文件到 git
4. CI 自动跑 Replay 验证

## CI

`.github/workflows/e2e-real-llm.yml` 包含 3 个 job:

1. **replay-tests**: 跑 4 个 e2e(需要 `DEEPSEEK_API_KEY` secret,无则 skip)
2. **validate-fixtures**: 跑 `validate-fixtures.py`(不需要 key,每次必跑)
3. **record-freshness**: 检查 fixture 是否超过 30 天,过期则 `::warning::`

## 已知限制

- **不同模型/不同 base_url 的 fixture 不通用**:DeepSeek 的 fixture 在 OpenAI 上跑不通(base_url 不同,headers 不一样,响应格式也有差异)。每个 (test, model, base_url) 组合需要单独录制。
- **录制的 body 包含 messages 快照**:若 prompt 模板变化,需要重录。
- **限流响应(429)不在 fixture 中**:遇到限流会落到 `Live`/`Record` 模式下的真实错误。
