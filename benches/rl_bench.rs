//! axon-rl 强化学习环境 Criterion 基准测试
//!
//! 运行：`cargo bench -p axon-rl`
//!
//! 覆盖：
//! - 观测空间 build（特征提取 + 归一化 + 窗口聚合）
//! - 奖励函数 calculate
//! - TradingEnv step（端到端）
//! - Action 转换

use std::hint::black_box;

use axon_rl::action::state::PortfolioState;
use axon_rl::action::types::{
    Action, ActionSpace, ContinuousActionSpace, DiscreteActionSpace, TradingDirection,
};
use axon_rl::env::config::EnvConfig;
use axon_rl::env::types::MarketBar;
use axon_rl::env::TradingEnv;
use axon_rl::observation::types::{FeatureConfig, FeatureSource, MarketState, NormalizerType};
use axon_rl::observation::DefaultObservationSpace;
use axon_rl::reward::pnl::PnLReward;
use axon_rl::reward::sharpe::SharpeReward;
use axon_rl::reward::RewardFn;
use axon_rl::ObservationSpace;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

// ─── 辅助函数 ─────────────────────────────────────────

/// 构造一个简单的市场数据序列（线性递增价格）
fn make_market_data(n: usize) -> Vec<MarketBar> {
    (0..n)
        .map(|i| {
            let price = 100.0 + i as f64 * 0.01;
            MarketBar {
                timestamp: i as u64,
                open: price,
                high: price + 0.05,
                low: price - 0.05,
                close: price,
                volume: 1000.0,
            }
        })
        .collect()
}

/// 构造一个简单的 MarketState
fn make_market_state(i: u64) -> MarketState {
    let price = 100.0 + i as f64 * 0.01;
    MarketState {
        timestamp: i,
        symbol: "BTC-USDT".to_string(),
        open: price,
        high: price + 0.05,
        low: price - 0.05,
        close: price,
        last_price: price,
        volume: 1000.0,
        bid: Some(price - 0.01),
        ask: Some(price + 0.01),
        spread: Some(0.02),
        position: 0.0,
        cash: 100_000.0,
        portfolio_value: 100_000.0,
        unrealized_pnl: 0.0,
        realized_pnl: 0.0,
    }
}

/// 闭包适配器：将 usize 转换为 u64 后调用 make_market_state
fn make_market_state_usize(i: usize) -> MarketState {
    make_market_state(i as u64)
}

/// 构造默认特征配置
fn make_features() -> Vec<FeatureConfig> {
    vec![
        FeatureConfig {
            name: "close".to_string(),
            source: FeatureSource::PriceField("close".to_string()),
            normalizer: NormalizerType::ZScore,
            clip_range: Some((-5.0, 5.0)),
        },
        FeatureConfig {
            name: "volume".to_string(),
            source: FeatureSource::VolumeField("volume".to_string()),
            normalizer: NormalizerType::ZScore,
            clip_range: Some((-5.0, 5.0)),
        },
        FeatureConfig {
            name: "position".to_string(),
            source: FeatureSource::PositionField("position".to_string()),
            normalizer: NormalizerType::None,
            clip_range: Some((-1.0, 1.0)),
        },
    ]
}

// ─── 观测空间基准 ─────────────────────────────────────

fn bench_observation_build(c: &mut Criterion) {
    let features = make_features();
    let space = DefaultObservationSpace::new(32, features).unwrap();

    // 准备历史数据
    let history: Vec<MarketState> = (0..32).map(make_market_state_usize).collect();
    let current = make_market_state(32);

    c.bench_function("observation_build_32x3", |b| {
        b.iter(|| {
            let obs = space
                .build(black_box(&current), black_box(&history))
                .unwrap();
            black_box(obs);
        })
    });
}

fn bench_observation_window_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("observation_window_scaling");
    let features = make_features();
    for &window in &[8_usize, 16, 32, 64, 128] {
        let space = DefaultObservationSpace::new(window, features.clone()).unwrap();
        let history: Vec<MarketState> = (0..window).map(make_market_state_usize).collect();
        let current = make_market_state(window as u64);
        group.bench_with_input(BenchmarkId::from_parameter(window), &window, |b, _| {
            b.iter(|| {
                let obs = space
                    .build(black_box(&current), black_box(&history))
                    .unwrap();
                black_box(obs);
            })
        });
    }
    group.finish();
}

// ─── 奖励函数基准 ─────────────────────────────────────

/// 构造一个简单的 PortfolioState
fn make_portfolio_state(value: f64, position: f64) -> PortfolioState {
    let last_price = 100.0;
    PortfolioState {
        position,
        cash: value - position * last_price,
        portfolio_value: value,
        margin_used: 0.0,
        margin_available: value,
        unrealized_pnl: 0.0,
        last_price,
    }
}

fn bench_pnl_reward(c: &mut Criterion) {
    let r = PnLReward::default();
    let s1 = make_portfolio_state(100_000.0, 0.0);
    let s2 = make_portfolio_state(100_500.0, 0.0);
    let action = Action::continuous(vec![0.0]);
    c.bench_function("reward_pnl", |b| {
        b.iter(|| {
            let r = r.calculate(
                black_box(&s1),
                black_box(&action),
                black_box(&s2),
                black_box(&[]),
            );
            let _ = black_box(r);
        })
    });
}

fn bench_sharpe_reward(c: &mut Criterion) {
    let r = SharpeReward {
        window: 32,
        risk_free_rate: 0.0,
        scale: 1.0,
        reward_type: axon_rl::reward::sharpe::RiskAdjustedType::Sharpe,
        clip: 10.0,
    };
    let s1 = make_portfolio_state(100_000.0, 0.0);
    let s2 = make_portfolio_state(100_500.0, 0.0);
    let action = Action::continuous(vec![0.0]);
    // 构造历史收益率
    let history: Vec<f64> = (0..32)
        .map(|i| (i as f64 * 0.0001).sin() * 0.01)
        .collect();
    c.bench_function("reward_sharpe", |b| {
        b.iter(|| {
            let r = r.calculate(
                black_box(&s1),
                black_box(&action),
                black_box(&s2),
                black_box(&history),
            );
            let _ = black_box(r);
        })
    });
}

// ─── TradingEnv 端到端基准 ─────────────────────────────

fn make_env_config(max_steps: usize) -> EnvConfig {
    EnvConfig {
        initial_capital: 100_000.0,
        transaction_cost: 0.001,
        slippage: 0.0005,
        max_position_ratio: 1.0,
        max_steps,
        seed: Some(42),
        symbol: "BTC-USDT".to_string(),
        return_window: 32,
    }
}

fn bench_env_step(c: &mut Criterion) {
    let config = make_env_config(1_000);
    let action_space = ActionSpace::Continuous(ContinuousActionSpace::new(-1.0, 1.0));
    let observation_space: Box<dyn ObservationSpace> =
        Box::new(DefaultObservationSpace::new(32, make_features()).unwrap());
    let reward_fn: Box<dyn RewardFn> = Box::new(PnLReward::default());
    let market_data = make_market_data(1_000);

    let mut env = TradingEnv::new(
        config,
        action_space,
        observation_space,
        reward_fn,
        market_data,
    )
    .unwrap();
    env.reset().unwrap();

    let action = Action::continuous(vec![0.5]);
    c.bench_function("env_step", |b| {
        b.iter(|| {
            let r = env.step(black_box(&action));
            let _ = black_box(r);
        })
    });
}

fn bench_env_episode(c: &mut Criterion) {
    let config = make_env_config(500);
    let action_space = ActionSpace::Continuous(ContinuousActionSpace::new(-1.0, 1.0));
    let market_data = make_market_data(500);

    c.bench_function("env_full_episode_500", |b| {
        b.iter(|| {
            let mut env = TradingEnv::new(
                config.clone(),
                action_space.clone(),
                Box::new(DefaultObservationSpace::new(32, make_features()).unwrap()),
                Box::new(PnLReward::default()),
                market_data.clone(),
            )
            .unwrap();
            env.reset().unwrap();
            let action = Action::continuous(vec![0.5]);
            for _ in 0..500 {
                if env.step(black_box(&action)).unwrap().2 {
                    break;
                }
                let _ = black_box(());
            }
        })
    });
}

// ─── Action 转换基准 ──────────────────────────────────

fn bench_action_continuous_construction(c: &mut Criterion) {
    c.bench_function("action_continuous_construction", |b| {
        b.iter(|| {
            let a = Action::continuous(black_box(vec![0.1, 0.2, 0.3]));
            black_box(a);
        })
    });
}

fn bench_action_discrete_construction(c: &mut Criterion) {
    c.bench_function("action_discrete_construction", |b| {
        b.iter(|| {
            let a = Action::discrete(black_box(2));
            black_box(a);
        })
    });
}

fn bench_action_space_clip(c: &mut Criterion) {
    let space = ContinuousActionSpace::new(-1.0, 1.0);
    c.bench_function("action_space_clip", |b| {
        b.iter(|| {
            // 连续动作空间无 sample 方法，使用 clip 测量其核心方法性能
            let v = black_box(2.5_f64);
            let clipped = space.clip(black_box(v));
            black_box(clipped);
        })
    });
}

fn bench_discrete_action_space_index_to_action(c: &mut Criterion) {
    let space = DiscreteActionSpace::new(5, TradingDirection::Both);
    c.bench_function("discrete_action_space_index_to_action", |b| {
        b.iter(|| {
            let a = space.index_to_action(black_box(2)).unwrap();
            black_box(a);
        })
    });
}

criterion_group!(
    benches,
    // 观测
    bench_observation_build,
    bench_observation_window_scaling,
    // 奖励
    bench_pnl_reward,
    bench_sharpe_reward,
    // 端到端
    bench_env_step,
    bench_env_episode,
    // 动作
    bench_action_continuous_construction,
    bench_action_discrete_construction,
    bench_action_space_clip,
    bench_discrete_action_space_index_to_action,
);
criterion_main!(benches);
