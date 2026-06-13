//! axon-compliance 集成测试
//!
//! 测试各模块之间的协作：交易记录 + 审计日志 + 文件存储 + 查询统计。

use axon_compliance::{
    AuditEventType, ComplianceConfig, ComplianceModule, LiquidityType, OrderType, TradeFilter,
    TradeRecord, TradeSide, TradeStatus,
};
use chrono::Utc;
use tempfile::TempDir;
use uuid::Uuid;

/// 创建测试配置
fn create_test_config() -> ComplianceConfig {
    ComplianceConfig {
        account_id: "test_account".into(),
        base_currency: "USDT".into(),
        large_trade_threshold: 100000.0,
        position_limit: 1000000.0,
        max_portfolio_concentration: 0.4,
        data_retention_years: 7,
        regulators: vec!["SEC".into()],
    }
}

/// 创建指定参数的测试交易
fn create_test_trade(symbol: &str, side: TradeSide, quantity: f64, price: f64) -> TradeRecord {
    TradeRecord {
        trade_id: Uuid::new_v4(),
        order_id: Uuid::new_v4(),
        strategy_id: "test_strategy".into(),
        symbol: symbol.into(),
        side,
        quantity,
        price,
        notional_value: quantity * price,
        fee: quantity * price * 0.001,
        fee_currency: "USDT".into(),
        exchange: "Binance".into(),
        execution_time: Utc::now(),
        settlement_time: None,
        status: TradeStatus::Filled,
        order_type: OrderType::Market,
        exchange_trade_id: None,
        liquidity: LiquidityType::Taker,
        realized_pnl: None,
        funding_rate: None,
        slippage: None,
        created_at: Utc::now(),
    }
}

/// 测试完整交易生命周期：记录、查询、审计
#[test]
fn test_full_trade_lifecycle() {
    let tmp = TempDir::new().unwrap();
    let config = create_test_config();
    let mut compliance = ComplianceModule::new(config, tmp.path()).unwrap();

    // 记录多笔交易
    let trades = vec![
        create_test_trade("BTCUSDT", TradeSide::Buy, 1.0, 50000.0),
        create_test_trade("BTCUSDT", TradeSide::Sell, 0.5, 51000.0),
        create_test_trade("ETHUSDT", TradeSide::Buy, 10.0, 3000.0),
    ];

    for trade in trades {
        compliance.record_trade(trade).unwrap();
    }

    // 验证交易记录
    assert_eq!(compliance.trade_count(), 3);

    // 验证审计完整性
    assert!(compliance.verify_audit_integrity());
    assert_eq!(compliance.audit_log().len(), 3);

    // 查询 BTCUSDT 交易
    let filter = TradeFilter {
        symbol: Some("BTCUSDT".into()),
        ..Default::default()
    };
    let btc_trades = compliance.query_trades(&filter);
    assert_eq!(btc_trades.len(), 2);

    // 查询买入交易
    let filter = TradeFilter {
        side: Some(TradeSide::Buy),
        ..Default::default()
    };
    let buy_trades = compliance.query_trades(&filter);
    assert_eq!(buy_trades.len(), 2);
}

/// 测试审计日志持久化和查询
#[test]
fn test_audit_log_persistence() {
    let tmp = TempDir::new().unwrap();
    let config = create_test_config();
    let mut compliance = ComplianceModule::new(config, tmp.path()).unwrap();

    // 记录交易
    let trade = create_test_trade("BTCUSDT", TradeSide::Buy, 1.0, 50000.0);
    compliance.record_trade(trade).unwrap();

    // 验证审计日志
    assert!(compliance.verify_audit_integrity());
    assert_eq!(compliance.audit_log().len(), 1);

    // 验证审计事件类型
    let events = compliance
        .audit_log()
        .query_by_type(&AuditEventType::TradeExecuted);
    assert_eq!(events.len(), 1);
}

/// 测试大额交易检测（记录但不阻止）
#[test]
fn test_large_trade_detection() {
    let tmp = TempDir::new().unwrap();
    let config = create_test_config();
    let mut compliance = ComplianceModule::new(config, tmp.path()).unwrap();

    // 记录大额交易（超过阈值 100000）
    let large_trade = create_test_trade("BTCUSDT", TradeSide::Buy, 10.0, 15000.0);
    compliance.record_trade(large_trade).unwrap();

    // 交易应该被记录（不阻止）
    assert_eq!(compliance.trade_count(), 1);
}

/// 测试交易统计计算
#[test]
fn test_trade_stats_calculation() {
    let tmp = TempDir::new().unwrap();
    let config = create_test_config();
    let mut compliance = ComplianceModule::new(config, tmp.path()).unwrap();

    let now = Utc::now();

    // 记录盈利交易
    let winning_trade = TradeRecord {
        execution_time: now,
        realized_pnl: Some(100.0),
        ..create_test_trade("BTCUSDT", TradeSide::Buy, 1.0, 50000.0)
    };

    // 记录亏损交易
    let losing_trade = TradeRecord {
        execution_time: now,
        realized_pnl: Some(-50.0),
        ..create_test_trade("ETHUSDT", TradeSide::Buy, 10.0, 3000.0)
    };

    compliance.record_trade(winning_trade).unwrap();
    compliance.record_trade(losing_trade).unwrap();

    // 获取统计
    let stats = compliance.get_trade_stats(
        now - chrono::Duration::hours(1),
        now + chrono::Duration::hours(1),
    );

    assert_eq!(stats.total_trades, 2);
    assert_eq!(stats.winning_trades, 1);
    assert_eq!(stats.losing_trades, 1);
    assert!((stats.win_rate - 0.5).abs() < f64::EPSILON);
}
