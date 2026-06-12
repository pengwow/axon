//! axon-data 集成测试(smoke)
//!
//! 覆盖:
//! - DataRequest 构造 + 校验
//! - Mock 数据源 + DataService 查询
//! - FeaturePipeline fit_transform

use axon_data::pipeline::{FeaturePipeline, ZScoreNormalizer};
use axon_data::sources::MockSource;
use axon_data::types::{DataRequest, Frequency};
use axon_data::{DataError, DataService};
use axon_core::market::{Side, Tick};
use axon_core::time::Timestamp;
use axon_core::types::{Price, Quantity};
use chrono::Utc;

fn make_tick(price: f64, nanos: i64) -> Tick {
    Tick::new(
        Timestamp::from_nanos(nanos),
        Price::from_f64(price),
        Quantity::from(1.0),
        Side::Buy,
    )
}

#[test]
fn data_request_is_valid_for_chronological_range() {
    let start = Utc::now();
    let end = start + chrono::Duration::hours(1);
    let req = DataRequest::new("BTCUSDT", start, end, Frequency::Hour1);
    assert!(req.is_valid());
    assert_eq!(req.frequency, Frequency::Hour1);
}

#[test]
fn data_request_is_invalid_for_inverted_range() {
    let start = Utc::now();
    let end = start - chrono::Duration::hours(1);
    let req = DataRequest::new("BTCUSDT", start, end, Frequency::Hour1);
    assert!(!req.is_valid());
}

#[tokio::test]
async fn data_service_loads_via_mock_source() {
    let ticks = vec![make_tick(100.0, 0), make_tick(101.0, 1_000_000_000)];
    let svc = DataService::new()
        .register_source(Box::new(MockSource::with_rows("mock", ticks)));
    let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
    let ds = svc.load(&req).await.expect("load ok");
    assert_eq!(ds.len(), 2);
    assert_eq!(ds.source, "mock");
}

#[tokio::test]
async fn data_service_missing_source_returns_error() {
    let svc = DataService::new();
    let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick)
        .with_source("nonexistent");
    let err = svc.load(&req).await.expect_err("should fail");
    assert!(matches!(err, DataError::SourceNotFound(_)));
}

#[test]
fn feature_pipeline_fit_then_transform() {
    let ticks = vec![
        make_tick(1.0, 0),
        make_tick(2.0, 1_000_000_000),
        make_tick(3.0, 2_000_000_000),
        make_tick(4.0, 3_000_000_000),
        make_tick(5.0, 4_000_000_000),
    ];
    let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
    let ds = axon_data::Dataset::new(ticks, vec![], "test".into(), req);

    let mut pipeline = FeaturePipeline::new()
        .with_normalizer(Box::new(ZScoreNormalizer::new()));
    let matrix = pipeline.fit_transform(&ds);
    assert_eq!(matrix.n_samples, 5);
    assert_eq!(matrix.n_features, 1);
    let mean: f32 = matrix.data.iter().sum::<f32>() / matrix.n_samples as f32;
    assert!(mean.abs() < 1e-5, "expected zero mean, got {mean}");
}
