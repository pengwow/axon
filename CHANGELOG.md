# Changelog

All notable changes to AXON will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-06-13

### Added

#### Phase 0: 架构与基础设施
- Cargo workspace 初始化，17 个 crate
- CI/CD 验证工作流（GitHub Actions）
- Docker 多阶段构建配置

#### Phase 1: 核心引擎 + RL 环境
- `axon-core`：时间戳、类型、市场数据、订单、事件、队列、投资组合、调度器
- `axon-backtest`：L1/L2/L3 撮合引擎、市场冲击模型、延迟模型
- `axon-rl`：Gymnasium 环境、VecEnv、PyO3 绑定

#### Phase 2: 训练与优化
- `axon-hpo`：超参优化（TPE/CMA-ES/NSGA-II）
- `axon-walk-forward`：滚动前向验证、Purged 交叉验证
- `axon-tracker`：实验追踪（MLflow/WandB/Local）
- `axon-registry`：模型版本管理
- `axon-distributed`：Ray Actor 分布式训练

#### Phase 3: AI 增强
- `axon-llm`：ReAct 智能体、Tool Calling
- `axon-explain`：SHAP 可解释性、反事实分析
- `axon-ensemble`：投票/堆叠/动态加权集成
- `axon-data`：Arrow IPC、Bar 聚合、Mmap 缓存
- `axon-compliance`：审计日志、合规报表

#### Phase 4: 生产部署
- `axon-risk`：风控引擎（熔断器、VaR、仓位/杠杆/回撤检查）
- `axon-inference`：推理引擎（ONNX/tch/Candle、批推理、热更新）
- `axon-exchange`：交易所对接（WebSocket、限流、订单生命周期）
- `axon-oms`：订单管理（状态机、幂等性、快照恢复）
- `axon-monitor`：监控告警（Counter/Gauge/Histogram、告警规则）

#### Phase 5: 性能深度优化
- `axon-core::simd`：SIMD 加速（AVX2 归一化/VaR/深度计算）
- 零拷贝优化（Symbol/Price into_inner）
- 流式回测引擎（StreamingEngine + PaperTrading）

#### 横向任务
- 并发测试、模糊测试（proptest）、契约测试
- 端到端集成测试（36 个集成测试）
- 性能基准测试（15 个 Criterion 基准）
- 用户指南、架构设计文档、API 文档

### Fixed
- PyO3 0.28 兼容性修复（PyDict::new_bound → PyDict::new）
- VaR 计算修复：全正收益时返回 0（而非负值）
- CI 测试改用 `cargo test --workspace`（避免 libtorch 依赖）

### Changed
- 版本号从 0.0.1 升级到 0.1.0

## [0.0.1] - 2026-06-10

### Added
- 项目初始化：工作区、根 Cargo.toml、统一 lint/profile 配置
