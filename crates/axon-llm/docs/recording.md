# LLM HTTP 录制 / 回放中间件

`axon_llm::backends::recording` 提供 vcr 风格的 HTTP 录制与回放中间件,用于 e2e 测试。

## 模块组成

- [`RecordingLayer`](https://docs.rs/axon-llm) — 主入口,持有 `Mode` + fixture 目录
- [`Mode`] — `Replay` / `Record` / `Passthrough`
- [`RecordedRequest`] / [`RecordedResponse`] — 录制协议(脱敏前)
- [`Fixture`] — 落盘格式
- `sanitize_request` / `sanitize_response` — 落盘前脱敏工具

## Fixture 命名

`RecordingLayer::fixture_path(req)` 返回:

```
<fixtures_dir>/<test_name>/<model>/<key>.json
```

`key = sha256(url + "|" + method + "|" + canonical(body))` 前 12 hex。

- 字段顺序无关(canonicalize 步骤)
- 同一请求命中相同 key → 同一文件(便于 reuse)

## 使用模式

### 1. Replay(默认)

```rust
let layer = RecordingLayer::from_env("simple_chat");
let resp = layer.send(req, &backend).await?;
// fixture miss → LLMError::Parse("fixture missing: ...")
```

### 2. Record

```rust
std::env::set_var("E2E_MODE", "record");
let layer = RecordingLayer::from_env("simple_chat");
let resp = layer.send(req, &backend).await?;
// 调 backend + 落盘 fixture + 缓存内存
```

### 3. Passthrough / Live

```rust
std::env::set_var("E2E_MODE", "live");
let layer = RecordingLayer::from_env("simple_chat");
let resp = layer.send(req, &backend).await?;
// 调 backend + 缓存内存(不落盘)
```

## 脱敏规则

落盘前自动删除:

- Request: `Authorization` / `authorization` / `x-api-key` / `host` / `Host`
- Response: `set-cookie` / `Set-Cookie` / `authorization` / `Authorization`

这样 fixture 可安全提交到 git(不泄露 API key)。

## 内存缓存

同一进程内,`RecordingLayer` 用 `Mutex<HashMap<key, RecordedResponse>>` 缓存,避免:
- Replay 模式下重复 IO
- Record 模式下同一 key 多次请求导致 fixture 反复被覆盖

测试用 `cargo test -- --test-threads=1` 可保证每 test 单独的 RecordingLayer 实例(单测中应 `RecordingLayer::new(...)` 显式构造)。

## fixture_key 的稳定性

- URL 完全一致(包括 query string 顺序)
- method 大小写敏感(代码里统一 `POST` 即可)
- body 用 `serde_json` 排序后序列化 → 字段顺序不影响 key

例:两个 body
```json
{"model":"x","messages":[]}
{"messages":[],"model":"x"}
```
对应同一 key。

## 与 wiremock 的关系

- `wiremock`:HTTP server mock,只用于回放 stage
- `recording` module:在 backend trait 之上做录制/回放,**完全不需要 wiremock**

## 添加新场景的流程

1. 写测试代码,使用 `RecordingLayer::from_env("test_name")`
2. 跑 `E2E_MODE=record` 录制
3. `git add tests/e2e/common/fixtures/`
4. CI 自动 replay 验证

参考 `e2e_simple_chat_test.rs` / `e2e_tool_calling_test.rs`。
