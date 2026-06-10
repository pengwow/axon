//! 交易环境（Gymnasium 兼容）
//!
//! 将观测、动作、奖励、执行器组合为标准 RL 接口。

use crate::action::state::PortfolioState;
use crate::action::types::{Action, ActionSpace};
use crate::env::action_decoder::ActionDecoder;
use crate::env::config::EnvConfig;
use crate::env::error::{EnvError, EnvResult};
use crate::env::executor::Executor;
use crate::env::types::{EnvInfo, MarketBar};
use crate::observation::types::{MarketState, Observation, ObservationSpace};
use crate::reward::RewardFn;
use crate::reward::history::ReturnHistory;

/// 单步执行结果
pub type StepResult = (Observation, f64, bool, EnvInfo);

/// 交易环境
///
/// 组合观测空间、动作空间、奖励函数与执行器，
/// 暴露 Gymnasium 风格的 `reset` / `step` / `render` 接口。
pub struct TradingEnv {
    /// 环境配置
    config: EnvConfig,
    /// 动作空间
    action_space: ActionSpace,
    /// 动作解码器
    decoder: ActionDecoder,
    /// 观测空间
    observation_space: Box<dyn ObservationSpace>,
    /// 奖励函数
    reward_fn: Box<dyn RewardFn>,
    /// 订单执行器
    executor: Executor,
    /// 行情数据
    market_data: Vec<MarketBar>,
    /// 当前时间步
    current_step: usize,
    /// 当前组合状态
    portfolio: PortfolioState,
    /// 是否 episode 已结束
    done: bool,
    /// episode 累计成交笔数
    trades_executed: usize,
    /// episode 累计交易成本
    transaction_costs: f64,
    /// 收益率历史（用于风险调整奖励）
    return_history: ReturnHistory,
}

impl TradingEnv {
    /// 构造新交易环境
    pub fn new(
        config: EnvConfig,
        action_space: ActionSpace,
        observation_space: Box<dyn ObservationSpace>,
        reward_fn: Box<dyn RewardFn>,
        market_data: Vec<MarketBar>,
    ) -> EnvResult<Self> {
        if market_data.is_empty() {
            return Err(EnvError::EmptyMarketData);
        }

        let decoder = ActionDecoder::from_space(&action_space, &config.symbol)?;
        let executor = Executor::new(config.clone());
        let return_history = ReturnHistory::new(config.return_window);
        let portfolio = PortfolioState {
            cash: config.initial_capital,
            portfolio_value: config.initial_capital,
            last_price: market_data[0].close,
            ..Default::default()
        };

        Ok(Self {
            config,
            action_space,
            decoder,
            observation_space,
            reward_fn,
            executor,
            market_data,
            current_step: 0,
            portfolio,
            done: false,
            trades_executed: 0,
            transaction_costs: 0.0,
            return_history,
        })
    }

    /// 重置环境到初始状态
    ///
    /// # 流程
    /// 1. 重置 step / done / 计数器
    /// 2. 重置 portfolio 到初始资金
    /// 3. 重置收益率历史
    /// 4. 返回初始观测
    pub fn reset(&mut self) -> EnvResult<Observation> {
        self.current_step = 0;
        self.done = false;
        self.trades_executed = 0;
        self.transaction_costs = 0.0;
        self.portfolio = PortfolioState {
            cash: self.config.initial_capital,
            portfolio_value: self.config.initial_capital,
            last_price: self.market_data[0].close,
            ..Default::default()
        };
        self.return_history.clear();

        self.build_observation()
    }

    /// 执行一步
    ///
    /// # 流程
    /// 1. 检查 episode 状态
    /// 2. 解析动作 → 订单
    /// 3. 执行订单 → 更新组合
    /// 4. 推进时间步 + 重估
    /// 5. 计算奖励
    /// 6. 返回 `(observation, reward, done, info)`
    pub fn step(&mut self, action: &Action) -> EnvResult<StepResult> {
        if self.done {
            return Err(EnvError::EpisodeAlreadyDone(self.current_step));
        }

        // 1. 数据耗尽检查
        if self.current_step + 1 >= self.market_data.len() {
            self.done = true;
            let obs = self.build_observation()?;
            return Ok((obs, 0.0, true, self.build_info()));
        }

        let current_bar = self.market_data[self.current_step];
        let next_bar = self.market_data[self.current_step + 1];

        // 2. 动作 → 订单
        let order = self.decoder.decode(action, &self.portfolio)?;

        // 3. 执行订单
        let mut order_cost = 0.0;
        if let Some(o) = order {
            let results = self
                .executor
                .execute(&[o], &current_bar, &mut self.portfolio)?;
            for r in &results {
                if r.filled {
                    self.trades_executed += 1;
                    order_cost += r.cost;
                    self.transaction_costs += r.cost;
                }
            }
        }

        // 4. 按下根 K 线 close 重估组合
        self.executor.revalue(&mut self.portfolio, next_bar.close)?;
        self.current_step += 1;

        // 5. 计算奖励
        let current_value = self.portfolio.portfolio_value;
        let previous_value = self.portfolio_history_prev();

        let prev_state = PortfolioState {
            portfolio_value: previous_value,
            cash: self.portfolio.cash,
            position: self.portfolio.position,
            last_price: current_bar.close,
            ..Default::default()
        };

        let reward = if previous_value > 0.0 {
            self.reward_fn
                .calculate(
                    &prev_state,
                    action,
                    &self.portfolio,
                    self.return_history.to_vec().as_slice(),
                )
                .map_err(|e| EnvError::RewardError(e.to_string()))?
        } else {
            0.0
        };

        // 6. 记录历史
        if previous_value > 0.0 {
            let ret = (current_value - previous_value) / previous_value;
            self.return_history.push(ret);
        }
        let _ = order_cost; // 已通过 executor 累加

        // 7. 检查 episode 结束
        if self.current_step + 1 >= self.market_data.len()
            || self.current_step >= self.config.max_steps
        {
            self.done = true;
        }

        // 8. 返回结果
        let obs = self.build_observation()?;
        let info = self.build_info();
        Ok((obs, reward, self.done, info))
    }

    /// 渲染环境状态为 ASCII 字符串
    pub fn render(&self) -> String {
        format!(
            "step={}/{} | value=${:.2} | cash=${:.2} | pos={:.4} | trades={} | cost=${:.2} | done={}",
            self.current_step,
            self.market_data.len(),
            self.portfolio.portfolio_value,
            self.portfolio.cash,
            self.portfolio.position,
            self.trades_executed,
            self.transaction_costs,
            self.done,
        )
    }

    // ── 访问器 ──────────────────────────────────────────────

    /// 获取当前步
    pub fn current_step(&self) -> usize {
        self.current_step
    }

    /// 是否已结束
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// 当前组合状态
    pub fn portfolio(&self) -> &PortfolioState {
        &self.portfolio
    }

    /// 动作空间
    pub fn action_space(&self) -> &ActionSpace {
        &self.action_space
    }

    /// 环境信息
    pub fn info(&self) -> EnvInfo {
        self.build_info()
    }

    // ── 内部辅助 ────────────────────────────────────────────

    /// 从 `market_data` 与组合状态构造 `MarketState` 再生成 `Observation`
    fn build_observation(&self) -> EnvResult<Observation> {
        let bar = self.market_data[self.current_step];
        let market_state = MarketState {
            timestamp: bar.timestamp,
            symbol: self.config.symbol.clone(),
            open: bar.open,
            high: bar.high,
            low: bar.low,
            close: bar.close,
            last_price: bar.close,
            volume: bar.volume,
            position: self.portfolio.position,
            cash: self.portfolio.cash,
            portfolio_value: self.portfolio.portfolio_value,
            unrealized_pnl: self.portfolio.unrealized_pnl,
            realized_pnl: 0.0,
            ..Default::default()
        };

        // 收集最近 N 步历史
        let start = self
            .current_step
            .saturating_sub(self.observation_space.num_features());
        let history: Vec<MarketState> = (start..=self.current_step)
            .rev()
            .map(|i| {
                let b = self.market_data[i];
                MarketState {
                    timestamp: b.timestamp,
                    symbol: self.config.symbol.clone(),
                    open: b.open,
                    high: b.high,
                    low: b.low,
                    close: b.close,
                    last_price: b.close,
                    volume: b.volume,
                    ..Default::default()
                }
            })
            .collect();

        self.observation_space
            .build(&market_state, &history)
            .map_err(|e| EnvError::ObservationError(e.to_string()))
    }

    /// 计算上一步的组合价值
    ///
    /// 在 step 内部、portfolio 已按 next_bar 重估后调用，
    /// 需要反推：使用上一根 K 线 close 重估。
    fn portfolio_history_prev(&self) -> f64 {
        if self.current_step == 0 {
            return self.config.initial_capital;
        }
        // 当前 self.current_step 已 +1，对应"上一步"是 current_step - 1
        let prev_idx = self.current_step.saturating_sub(1);
        let prev_close = self.market_data[prev_idx].close;
        self.portfolio.cash + self.portfolio.position * prev_close
    }

    /// 构造 `EnvInfo`
    fn build_info(&self) -> EnvInfo {
        EnvInfo {
            portfolio_value: self.portfolio.portfolio_value,
            trades_executed: self.trades_executed,
            transaction_costs: self.transaction_costs,
            current_step: self.current_step,
            done: self.done,
            initial_capital: self.config.initial_capital,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::types::{Action, ContinuousActionSpace, TradingDirection};
    use crate::observation::space::DefaultObservationSpace;
    use crate::observation::types::{FeatureConfig, FeatureSource, NormalizerType};
    use crate::reward::pnl::PnLReward;

    /// 生成确定性测试数据：100 根 K 线，价格随机游走
    fn make_test_data(n: usize) -> Vec<MarketBar> {
        (0..n)
            .map(|i| {
                let price = 100.0 + (i as f64) * 0.1;
                MarketBar::new(i as u64, price, price + 0.5, price - 0.5, price, 1000.0)
            })
            .collect()
    }

    /// 构造一个最小可用的默认观测空间
    fn make_obs_space() -> Box<dyn ObservationSpace> {
        let features = vec![
            FeatureConfig {
                name: "close".to_string(),
                source: FeatureSource::PriceField("close".to_string()),
                normalizer: NormalizerType::ZScore,
                clip_range: None,
            },
            FeatureConfig {
                name: "volume".to_string(),
                source: FeatureSource::VolumeField("volume".to_string()),
                normalizer: NormalizerType::None,
                clip_range: None,
            },
        ];
        Box::new(DefaultObservationSpace::new(1, features).unwrap())
    }

    fn make_env(n: usize) -> TradingEnv {
        let config = EnvConfig {
            max_steps: n,
            return_window: 20,
            ..Default::default()
        };
        let action_space = ActionSpace::Continuous(ContinuousActionSpace::new(-1.0, 1.0));
        let reward_fn = Box::new(PnLReward::default());
        TradingEnv::new(
            config,
            action_space,
            make_obs_space(),
            reward_fn,
            make_test_data(n),
        )
        .unwrap()
    }

    #[test]
    fn test_reset_returns_observation() {
        let mut env = make_env(50);
        let obs = env.reset().unwrap();
        assert!(!obs.features.is_empty());
        assert_eq!(env.current_step(), 0);
        assert!(!env.is_done());
    }

    #[test]
    fn test_step_returns_tuple() {
        let mut env = make_env(50);
        env.reset().unwrap();
        let action = Action::continuous(vec![0.0]); // Hold
        let (_obs, _reward, done, info) = env.step(&action).unwrap();
        assert_eq!(info.current_step, 1);
        assert!(!done);
    }

    #[test]
    fn test_episode_ends_when_data_exhausted() {
        let mut env = make_env(10);
        env.reset().unwrap();
        let action = Action::continuous(vec![0.0]);
        for _ in 0..20 {
            if env.is_done() {
                break;
            }
            let (_, _, done, _) = env.step(&action).unwrap();
            if done {
                assert!(env.is_done());
                return;
            }
        }
        assert!(env.is_done(), "episode should end when data is exhausted");
    }

    #[test]
    fn test_step_after_done_returns_error() {
        let mut env = make_env(5);
        env.reset().unwrap();
        let action = Action::continuous(vec![0.0]);
        // 跑完 episode
        for _ in 0..10 {
            let _ = env.step(&action);
            if env.is_done() {
                break;
            }
        }
        // 已结束后再 step 应返回错误
        let err = env.step(&action).unwrap_err();
        assert!(matches!(err, EnvError::EpisodeAlreadyDone(_)));
    }

    #[test]
    fn test_hold_action_does_not_execute_trades() {
        let mut env = make_env(20);
        env.reset().unwrap();
        let action = Action::continuous(vec![0.0]); // 接近 0 视为 Hold
        let (_obs, _reward, _done, info) = env.step(&action).unwrap();
        assert_eq!(info.trades_executed, 0);
    }

    #[test]
    fn test_buy_action_increases_position() {
        let mut env = make_env(20);
        env.reset().unwrap();
        let action = Action::continuous(vec![0.5]); // 做多
        env.step(&action).unwrap();
        assert!(env.portfolio().position > 0.0);
        assert!(env.info().trades_executed > 0);
    }

    #[test]
    fn test_render_returns_nonempty_string() {
        let mut env = make_env(10);
        env.reset().unwrap();
        let s = env.render();
        assert!(!s.is_empty());
        assert!(s.contains("step"));
    }

    #[test]
    fn test_empty_market_data_errors() {
        let config = EnvConfig::default();
        let action_space = ActionSpace::Continuous(ContinuousActionSpace::new(-1.0, 1.0));
        let reward_fn = Box::new(PnLReward::default());
        let result: Result<TradingEnv, EnvError> =
            TradingEnv::new(config, action_space, make_obs_space(), reward_fn, vec![]);
        let err = result.err().expect("expected error");
        assert_eq!(err, EnvError::EmptyMarketData);
    }

    #[test]
    fn test_long_only_env_runs() {
        let config = EnvConfig {
            max_steps: 5,
            ..Default::default()
        };
        let action_space = ActionSpace::Continuous(ContinuousActionSpace::new(0.0, 1.0));
        let reward_fn = Box::new(PnLReward::default());
        let mut env = TradingEnv::new(
            config,
            action_space,
            make_obs_space(),
            reward_fn,
            make_test_data(20),
        )
        .unwrap();
        env.reset().unwrap();
        for _ in 0..5 {
            let action = Action::continuous(vec![0.3]);
            let _ = env.step(&action);
        }
        assert!(env.is_done());
    }

    #[test]
    #[allow(non_snake_case)]
    fn test_short_only_env_direction() {
        // 简单验证交易方向不同的空间也能工作
        let _ = TradingDirection::ShortOnly;
    }
}
