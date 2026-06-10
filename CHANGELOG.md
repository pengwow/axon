# Changelog

All notable changes to AXON will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Phase 0 项目骨架：Cargo workspace 初始化
- 三个基础 crate：`axon-core`、`axon-backtest`、`axon-cli`
- CI 验证工作流（GitHub Actions）
- 架构决策记录（ADR）框架
- Apache-2.0 单许可
- Docker 多阶段构建配置（runtime + builder + docker-compose）
- **Phase 1A P0**：`axon-core::time` 模块
  - `Timestamp`：纳秒精度时间戳（i64 纳秒，serde transparent，运算符重载，精度截断）
  - `MonotonicClock`：基于 `Instant` 的单调时钟
  - `TimePrecision`：时间精度枚举（秒/毫秒/微秒/纳秒）
  - `TimestampError` / `TimestampResult`：统一错误类型
  - 23 单元测试 + 1 文档测试，遵循 TDD 流程
- **Phase 1A P1**：`axon-core::types` 与 `axon-core::market` 模块
  - `Price` / `Quantity`：newtype 包装 `f64`，提供 `Eq`/`Ord`/`Hash` 手工实现（非 NaN 保证）
  - `Symbol`：字符串包装，支持 `From<&str>` / `From<String>` / `HashSet`
  - `Side`：买卖方向枚举（`Buy`/`Sell`），含 `opposite`/`sign`/`Display`/`Default`
  - `Tick`：逐笔成交，`#[repr(C)]` 固定布局（32 字节含 padding）
  - `Bar` / `BarPeriod`：OHLCV K线 + 9 种时间周期枚举（分钟/小时/天/周/月）
  - `Bar::from_ticks`：从 Tick 序列聚合生成 K线，含 OHLC 校验
  - `OrderBookLevel` / `OrderBookSnapshot`：订单簿层 + 快照（best_bid/ask/mid/spread/depth）
  - `OrderBookSnapshot::from_l2`：自动排序 + 过滤零数量层
  - `OrderBookSnapshot::validate_sorting`：检测排序错误
  - `Trade`：成交记录（含买卖双方 OrderId），`#[repr(C)]` 40 字节
  - `MarketDataError` / `MarketDataResult`：市场数据错误类型
  - `lib.rs` 顶层 re-export：`Tick`/`Bar`/`Trade`/`Price`/`Quantity`/`Symbol`/`Timestamp` 等
  - 42 单元测试覆盖 + 1 文档测试（合计 77 个），全部通过
- **Phase 1A P0**：`axon-core::order` 模块
  - `OrderType`：`Market` / `Limit` / `Stop` / `StopLimit` / `Iceberg` 五种订单类型
  - `TimeInForce`：`GTC` / `IOC` / `FOK` / `GFD` / `FAK` 5 种有效期
  - `OrderStatus`：`Created` / `Pending` / `PartiallyFilled` / `Filled` / `Cancelled` / `Rejected` / `Expired` 7 态状态机
  - `RejectReason`：8 种拒绝原因枚举
  - `Order`：订单主体结构（`id` / `symbol` / `side` / `order_type` / `quantity` / `filled_quantity` / `time_in_force` / `status` / `created_at` / `updated_at` / `reject_reason` / `client_order_id`）
  - 订单生命周期方法：`new` / `activate` / `apply_fill` / `cancel` / `reject` / `remaining_quantity` / `is_filled` / `can_cancel` / `fill_ratio`
  - 状态机合法转换检查（`can_transition_to`），非法转换返回 `OrderError::InvalidStateTransition`
  - 超量成交防护（`OrderError::OverFill`）
  - `OrderError` / `OrderResult`：订单模块错误类型（`InvalidStateTransition` / `OrderNotActive` / `OverFill` / `FokPartialFill` / `IocPartialFill` / `Expired` / `Cancelled`）
  - 33 单元测试覆盖（合计 110 个），全部通过
  - `lib.rs` 顶层 re-export 扩展：新增 `Order` / `OrderType` / `OrderStatus` / `TimeInForce` / `RejectReason` / `OrderError` / `OrderId` / `OrderResult`
- **Phase 1A P0**：`axon-core::event` 模块
  - `EventType`：1 字节位掩码（`MARKET_DATA` / `ORDER` / `FILL` / `SYSTEM` / `ALL` / `NONE`），支持位运算 `|`/`&`/`contains`/`union`/`intersects`
  - `Event`：4 路枚举（`MarketData` / `Order` / `Fill` / `System`），提供 `timestamp()` / `seq()` / `event_type()` / `is_before()` 方法
  - `MarketDataEvent` / `MarketDataPayload`：承载 `Tick` / `Bar` / `OrderBookSnapshot`
  - `OrderEvent` / `OrderAction`：承载 `Submitted` / `Cancelled` / `Modified` / `Rejected` 操作
  - `FillEvent`：承载 `Trade` 成交记录
  - `SystemEvent` / `SystemAction`：`Heartbeat` / `SessionStart` / `SessionEnd` / `Error` / `Custom`
  - `EventHandler` trait：`on_event` / `event_types` / `is_interested`（默认） / `on_events`（默认批量过滤）
  - `EventBuilder`：自增序列号、4 种事件类型便捷构造
  - `EventRouter`：多处理器分发 + 类型位掩码过滤
  - `EventCollector`：事件收集器，缓存感兴趣事件用于回放/审计
  - `EventError` / `EventResult`：`SequenceNotMonotonic` / `TimestampNotMonotonic` / `InvalidEventType` / `HandlerRegistration`
  - 31 单元测试覆盖（合计 141 个），全部通过
  - `lib.rs` 顶层 re-export 扩展：新增 `Event` / `EventBuilder` / `EventCollector` / `EventError` / `EventHandler` / `EventResult` / `EventRouter` / `EventType` / `FillEvent` / `MarketDataEvent` / `MarketDataPayload` / `OrderAction` / `OrderEvent` / `SystemAction` / `SystemEvent`
- **Phase 1A P0**：`axon-core::queue` 模块（事件队列）
  - `EventQueue`：基于 `BinaryHeap` 的最小堆优先级队列（反转 `Ord` 实现）
  - `QueuedEvent`：队列条目（`timestamp` / `seq` / `event`）
  - `QueueMode`：`Normal` / `Paused` / `StepOnce` 三态模式
  - `QueueStats`：统计信息（`total_pushed` / `total_popped` / `total_skipped` / `replay_count`）
  - `EventQueueError`：`QueueEmpty` / `ReplayNotEnabled` / `ReplayLogEmpty`
  - 核心方法：`new` / `with_replay_log` / `push` / `push_at` / `push_batch` / `from_sorted` / `next` / `peek` / `peek_time` / `is_empty` / `len` / `current_time` / `fast_forward_to` / `fast_forward_collect` / `drain_until` / `pause` / `resume` / `step` / `mode` / `reset` / `replay` / `replay_log` / `clear_replay_log` / `stats`
  - 严格时间戳排序 + 同一时间戳内 `seq` 升序（FIFO 语义）
  - 批量加载（`from_sorted`）走 O(n) 建堆，单次插入 O(log n)
  - 可选重放日志：完整记录入队事件，支持 `reset` 后从日志重放
  - 29 单元测试覆盖（合计 170 个），全部通过
  - `lib.rs` 顶层 re-export 扩展：新增 `EventQueue` / `EventQueueError` / `EventQueueResult` / `QueueMode` / `QueueStats` / `QueuedEvent`
- **Phase 1A P1**：`axon-backtest::matching` 模块（L1 撮合引擎）
  - `MatchingEngine` trait：`submit` / `cancel` / `best_bid` / `best_ask` / `spread` / `depth` / `active_order_count`
  - `L1MatchingEngine`：价格-时间优先撮合，支持 `Limit` / `Market` / `IOC` / `FOK`
  - 数据结构：`BTreeMap<Price, VecDeque<Order>>` 订单簿 + `HashMap<OrderId, (Side, Price)>` 订单索引
  - FOK 预检：撮合前先检查订单簿深度，避免部分成交
  - IOC 取消：未完全成交的剩余部分自动 `cancel`
  - `MatchFill`：撮合成交记录（`fill_id` / `taker_order_id` / `maker_order_id` / `price` / `quantity` / `taker_side` / `timestamp`），含 `turnover()` 便捷方法
  - `OrderBookLevel`：订单簿层（`price` / `quantity` / `order_count`）
  - `SubmitResult`：提交结果（`fills` / `is_filled` / `is_partially_filled` / `remaining_quantity`），提供 `empty` / `filled` / `partial` 工厂
  - `TradeRole`：`Maker` / `Taker` 撮合角色
  - `MatchingError`：`OrderNotFound` / `InvalidModification` / `OrderAlreadyFilled` / `InvalidPrice` / `InvalidQuantity` / `OrderBookEmpty` / `FokPartialFill` / `UnsupportedOrderType`
  - `axon-backtest` re-export 扩展：新增 `MatchFill` / `SubmitResult` / `MatchingError`
  - 30 单元测试覆盖（合计 200 个），全部通过
- **Phase 1A P1**：`axon-backtest::matching::l2` 模块（L2 撮合引擎）
  - `L2MatchingEngine`：在 L1 基础上增加修改/统计/O(1) 取消/订单簿导入导出
  - `MatchingStats`：累计统计（`total_fills` / `total_volume` / `total_turnover` / `matched_orders`）
  - `OrderLocation`：订单位置（`side` / `price` / `offset`）
  - `OrderAmend`：订单修改请求（`order_id` / `new_price` / `new_quantity`）
  - `OrderBookEntry`：订单簿重建条目（`order_id` / `side` / `price` / `quantity` / `filled_quantity`）
  - 核心方法：`new` / `with_symbol` / `submit` / `cancel` / `modify` / `volume_at_price` / `depth` / `best_bid` / `best_ask` / `spread` / `active_order_count` / `stats` / `location` / `contains` / `from_entries` / `export_entries`
  - `build_limit_order` 便捷工厂方法
  - 17 单元测试覆盖（合计 220 个），全部通过
  - `axon-backtest` re-export 扩展：新增 `L2MatchingEngine` / `MatchingStats` / `OrderAmend` / `OrderBookEntry` / `OrderLocation` / `build_limit_order`
- **Phase 1A P1**：`axon-core::scheduler` 模块（调度器）
  - `SimulatedClock`：模拟时钟（`start` / `end` / `time_scale` / `now` / `set` / `advance` / `is_exhausted`）
  - `TaskId` / `Task` / `TaskStatus`（`Pending` / `Running` / `Completed` / `Cancelled` / `Scheduled`）/ `RepeatPolicy`（`Once` / `Interval` / `Cron`）
  - `TaskCallback` trait + `ClosureCallback<F>` 适配器
  - `SchedulerContext`：任务回调上下文（`current_time` / `event_queue` / `user_data`）
  - `SchedulerError`：`TaskNotFound` / `ScheduleInPast` / `ClockExhausted` / `InvalidInterval` / `TaskAlreadyCancelled` / `CallbackExecution`
  - `SchedulerStats`：`total_registered` / `total_fired` / `total_cancelled` / `total_ticks`
  - `Scheduler`：核心调度器，支持定时/周期/延时任务、取消、时钟推进、批量执行
  - 核心方法：`new` / `with_end` / `now` / `schedule_at` / `schedule_after` / `schedule_interval` / `cancel` / `task_status` / `task` / `active_count` / `task_count` / `run_until` / `tick` / `stats` / `clock` / `next_fire_time` / `reset`
  - 任务存储：HashMap（按 ID 查找）+ BTreeMap（按时间索引），Vec TaskId 时间槽合并
  - 回调存储：单独 HashMap TaskId → Box dyn TaskCallback，serde 跳过（不可序列化）
  - `SchedulerContext` 使用 `*mut EventQueue` 避免生命周期约束（仅单线程事件循环）
  - 时钟耗尽检查：任务触发时间超过 `clock.end()` 时停止执行
  - 42 单元测试覆盖（合计 303 个），全部通过
  - `lib.rs` 顶层 re-export 扩展：新增 11 个 scheduler 类型
- **Phase 1A P2**：`axon-core::impact` 模块（市场冲击模型）
  - `Impact`：冲击结果（`instantaneous` / `permanent` / `total` / `adjusted_price`）
  - `ImpactModel` trait：`compute_impact` / `name` / `params`
  - `LinearImpactModel`：线性冲击 `impact = coefficient × (qty / depth)`
  - `PowerLawImpactModel`：幂律冲击 `impact = coefficient × (qty / depth)^exponent`（默认 square-root law）
  - `AdaptiveImpactModel`：自适应冲击 `base × (volatility_scale × (1 + current_volatility))`
  - `ImpactModelConfig`：`Linear` / `PowerLaw` tagged enum
  - 工厂函数：`linear_impact`（coefficient=0.05）、`sqrt_impact`（coefficient=0.1, exponent=0.5）、`create_model(config)`
  - `ImpactModelError`：`EmptyOrderBook` / `InvalidParameter` / `InsufficientDepth` / `ComputationOverflow`
  - `AdaptiveImpactModel` 限制说明：因 `Box<dyn ImpactModel>` 不支持 derive，**不实现** `Clone` / `PartialEq` / `Serialize` / `Deserialize`（需使用 `ImpactModelConfig` + `create_model` 工厂路径序列化）
  - 44 单元测试覆盖（合计 347 个），全部通过
  - `lib.rs` 顶层 re-export 扩展：新增 10 个 impact 类型
- **Phase 1A P2**：`axon-core::latency` 模块（延迟模型）
  - `LatencyModel` trait：`sample_delay(path)` / `name` / `params`，`Send + Sync` 约束
  - `PathType`：`MarketData` / `OrderSubmit` / `OrderCancel` / `AccountQuery` / `Heartbeat`（含 `ALL` 常量与 `as_str`）
  - `LatencyParams`：模型参数摘要（`model_type` / `base_delay_ms` / `jitter_ms` / `path_overrides`）
  - `ConstantLatencyModel`：固定延迟（`uniform` / `with_paths` / `set_path` / `get`）
  - `NormalLatencyModel`：正态分布延迟，**Box-Muller 变换**实现，截断为非负
  - `ExponentialLatencyModel`：指数分布延迟（逆变换采样），`from_mean_ms` 便捷构造
  - `UniformLatencyModel`：均匀分布延迟，`max <= min` 时回退为 `min`
  - `QueueLatencyModel`：队列延迟模型（基础 + 队列长度 × 处理时间），`Mutex<usize>` 保护状态，`enqueue` / `dequeue` / `set_queue_length` / `queue_length` / `with_max_queue_length`，路径权重 `OrderSubmit=4` / `OrderCancel=2` / 其他=1
  - `CompositeLatencyModel`：组合延迟模型（`HashMap<PathType, Box<dyn LatencyModel>>` + `default_model`），未配置路径回退到默认
  - `LatencyModelFactory`：`constant` / `normal` / `exponential` / `uniform` / `queue` / `realistic_combo`（毫秒参数）
  - `LatencyModelError`：`InvalidParameter` / `PathNotConfigured` / `NegativeStdDev` / `NonPositiveRate` / `InvalidRange` / `QueueOverflow`
  - `CompositeLatencyModel` 限制说明：因 `Box<dyn LatencyModel>` 不支持 derive，**不实现** `Clone` / `PartialEq` / `Serialize` / `Deserialize`
  - `QueueLatencyModel` 手动实现 `Clone` / `Serialize` / `Deserialize`（因 `Mutex` 字段无法自动 derive，但语义上可序列化为 `(base, processing, max, current)`）
  - 38 单元测试覆盖（合计 335 个），全部通过
  - `lib.rs` 顶层 re-export 扩展：新增 12 个 latency 类型
  - 工作区新增 `rand = "0.8"` 依赖（仅 axon-core 使用）

### Changed

- `Price` / `Quantity` 公开 API 扩展：增加 `Eq` / `Ord` / `Hash` 实现
  - 手工实现以应对 `f64` 不支持这些 trait 的限制
  - 使用 `f64::to_bits()` 计算 Hash，避免 NaN 一致性问题
- `Bar` / `Side` / `OrderType` 使用 `#[derive(Default)]` 替代手写实现，消除 E0119 冲突
- `order.rs` 单文件重构为 `order/` 模块目录（`mod.rs` + `types.rs` + `tif.rs` + `status.rs` + `core.rs` + `error.rs`），符合 Rust 命名约定
- `event.rs` 单文件重构为 `event/` 模块目录（`mod.rs` + `types.rs` + `market.rs` + `order.rs` + `fill.rs` + `system.rs` + `handler.rs` + `builder.rs` + `router.rs` + `error.rs`），符合规范要求
- `EventHandler::is_interested` / `on_events` 移除 `Self: Sized` 约束，兼容 `dyn EventHandler` 对象安全
- **`order/order.rs` → `order/core.rs`**：消除 `clippy::module_inception` 警告（父模块与子模块同名），遵循 Rust 惯用规范
- **`portfolio/portfolio.rs` → `portfolio/core.rs`**：消除 `clippy::module_inception` 警告
- **`scheduler/scheduler.rs` → `scheduler/core.rs`**：消除 `clippy::module_inception` 警告
- **`Scheduler::run_until` 重构**：消除 `while_let_loop` 警告（改为 `while let`），提取 `fire_task` 私有方法消除代码重复与 `map_clone` 警告

### Deprecated

### Removed

- 旧占位 `axon-core::timestamp` 模块（替换为 `axon-core::time`）

### Fixed

- `axon-cli` 中 `env!("TARGET")` 编译期不可用，改用 `std::env::consts::*`
- `rust-toolchain.toml` 从 MSRV 1.75.0 升级到 stable（CI 仍强制 MSRV 校验）
- `Price` / `Quantity` 的 `Eq`/`Ord`/`Hash` 错误地从 `f64` 派生（`f64` 不实现这些 trait），改为手工实现并使用 `to_bits` 保证 NaN 场景下的一致性
- `Bar` 的 `Default` 与 `#[derive(Default)]` 冲突（E0119），通过删除手写实现解决
- Clippy 警告 `derivable_impls`（`Bar`/`Side`/`OrderType` 的手写 `Default`）— 改用 `#[derive(Default)]` + `#[default]` 标记
- Clippy 警告 `derive_ord_xor_partial_ord`（`Price`/`Quantity` 派生 `PartialOrd` + 手动 `Ord`）— 通过 `#[allow(clippy::derive_ord_xor_partial_ord)]` 在保持手工实现的前提下豁免（**技术约束，不可消除**：`f64` 根本性不实现 `Ord`；详细原因/风险/未来路径见 [price.rs:45-67](crates/axon-core/src/types/price.rs#L45-L67) 与 [quantity.rs:80-105](crates/axon-core/src/types/quantity.rs#L80-L105)）
- L1 撮合引擎重构以适配 `axon-core::order` 重构后的 API：
  - 价格通过 `OrderType::limit_price()` 获取（`Order` 不再持有 `price` 字段）
  - 终态判断改用 `Order::status.is_terminal()`
  - 状态转换通过 `Order::apply_fill()` 公开 API 完成
  - 修复 `transition_to` 私有方法被外部调用的访问错误
  - 修复 `iter_mut()` 与 `self.method()` 借用冲突，改用 `AtomicU64::fetch_add` 直接访问
  - 修复 `BTreeMap::iter_mut().rev()` 不存在的问题，改为先收集价格列表再迭代
- FOK 语义修正：撮合前先 `check_fok_fillable()` 预检订单簿深度，避免部分成交
- IOC 语义修正：未完全成交的剩余部分自动调用 `Order::cancel()`
- `Position::default()` 与 `#[derive(Default)]` 冲突（E0119）— 改用 derive 自动生成
- `Clippy::derivable_impls` 警告（`Position` 手写 `Default`）— 改用 `#[derive(Default)]`
- `Clippy::module_inception` 警告（`order::order` / `portfolio::portfolio` / `scheduler::scheduler` 模块与父模块同名）— 通过文件重命名 `module_name.rs` → `core.rs` 彻底消除
- `Quantity::from_f64` 拒绝负数与 `Position` 需求冲突 — 解除负数限制，允许 `Position` 用负数表示空头持仓；同步更新 `Tick` 验证测试以反映新语义
- `Symbol` 缺少 `Default` 实现 — 派生 `Default` 使其可用于 `Position::default()`
- `Currency::default()` 期望返回 `USD` 而非 `[0, 0, 0]` — 手动实现 `Default` 返回 `Self::USD`

### Security

## [0.0.1] - 2026-06-10

### Added

- 项目初始化：工作区、根 Cargo.toml、统一 lint/profile 配置
