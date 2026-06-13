//! axon-data 集成测试(smoke)
//!
//! 覆盖:
//! - DataRequest 构造 + 校验
//! - Mock 数据源 + DataService 查询
//! - FeaturePipeline fit_transform

use axon_core::market::{Side, Tick};
use axon_core::time::Timestamp;
use axon_core::types::{Price, Quantity};
use axon_data::pipeline::{FeaturePipeline, ZScoreNormalizer};
use axon_data::sources::MockSource;
use axon_data::types::{DataRequest, Frequency};
use axon_data::{DataError, DataService};
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
    let svc = DataService::new().register_source(Box::new(MockSource::with_rows("mock", ticks)));
    let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
    let ds = svc.load(&req).await.expect("load ok");
    assert_eq!(ds.len(), 2);
    assert_eq!(ds.source, "mock");
}

#[tokio::test]
async fn data_service_missing_source_returns_error() {
    let svc = DataService::new();
    let req =
        DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick).with_source("nonexistent");
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
    // PR5:走 from_ticks 桥接入口
    let ds = axon_data::Dataset::from_ticks(ticks, "test".into(), req).expect("from_ticks");

    let mut pipeline = FeaturePipeline::new().with_normalizer(Box::new(ZScoreNormalizer::new()));
    let matrix = pipeline.fit_transform(&ds);
    assert_eq!(matrix.n_samples, 5);
    assert_eq!(matrix.n_features, 1);
    let mean: f32 = matrix.data.iter().sum::<f32>() / matrix.n_samples as f32;
    assert!(mean.abs() < 1e-5, "expected zero mean, got {mean}");
}

// --- CSV fixture 集成测试(需 csv-source feature) ---

#[cfg(feature = "csv-source")]
mod csv_fixtures {
    use super::*;
    use axon_data::DataSource;
    use axon_data::sources::CsvSource;
    use std::path::PathBuf;

    /// 获取 fixture 路径(测试运行时 cwd 可能是 workspace root)
    fn fixture_path(name: &str) -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/fixtures");
        p.push(name);
        p
    }

    #[tokio::test]
    async fn csv_fixture_basic_loads_three_rows() {
        let src = CsvSource::new("basic", fixture_path("sample_basic.csv").to_str().unwrap());
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        let ds = src.query(&req).await.expect("load basic csv");
        assert_eq!(ds.len(), 3);
        // 时间戳:1e9, 2e9, 3e9 纳秒
        let ts: Vec<i64> = ds.iter_rows().map(|t| t.timestamp.nanos).collect();
        assert_eq!(ts, vec![1_000_000_000, 2_000_000_000, 3_000_000_000]);
    }

    #[tokio::test]
    async fn csv_fixture_custom_cols_inferred_via_header() {
        // header: time, close, volume, buy_sell - 推断器应识别
        let src = CsvSource::new(
            "custom",
            fixture_path("sample_custom_cols.csv").to_str().unwrap(),
        );
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        let ds = src.query(&req).await.expect("load custom csv");
        assert_eq!(ds.len(), 2);
        // price 应从 close 列读到
        let prices: Vec<f64> = ds.iter_rows().map(|t| t.price.as_f64()).collect();
        assert_eq!(prices, vec![100.5, 101.0]);
    }

    #[tokio::test]
    async fn csv_fixture_malformed_returns_corrupt_error_with_location() {
        // 第 2 行 price 列是 "not_a_number",应触发 CorruptData 错误并带 location
        let src = CsvSource::new(
            "malformed",
            fixture_path("sample_malformed.csv").to_str().unwrap(),
        );
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        let err = src.query(&req).await.expect_err("should fail on bad row");
        match err {
            DataError::CorruptData {
                expected,
                actual,
                location,
            } => {
                assert!(expected.contains("f64"));
                assert!(actual.contains("line 1")); // 第 2 行(0-indexed header 是 0 行,数据 1 行起)
                let loc = location.expect("location must be present");
                assert!(loc.file.contains("sample_malformed.csv"));
                assert_eq!(loc.column.as_deref(), Some("price"));
            }
            other => panic!("expected CorruptData, got {other:?}"),
        }
    }
}

// --- Parquet fixture 集成测试(需 parquet-source feature) ---

#[cfg(feature = "parquet-source")]
mod parquet_fixtures {
    use super::*;
    use arrow::array::Int64Array;
    use axon_data::DataSource;
    use axon_data::sources::ParquetSource;
    use std::path::PathBuf;

    /// 获取 fixture 路径(测试运行时 cwd 可能是 workspace root)
    fn fixture_path(name: &str) -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/fixtures");
        p.push(name);
        p
    }

    #[tokio::test]
    async fn parquet_fixture_basic_loads_five_rows() {
        let src = ParquetSource::new("basic", fixture_path("sample_basic.parquet"));
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        let ds = src.query(&req).await.expect("load basic parquet");
        assert_eq!(ds.len(), 5);
        // 验证时间戳严格按 0/1e9/2e9/3e9/4e9 纳秒
        let ts: Vec<i64> = ds.iter_rows().map(|t| t.timestamp.nanos).collect();
        assert_eq!(
            ts,
            vec![
                0,
                1_000_000_000,
                2_000_000_000,
                3_000_000_000,
                4_000_000_000
            ]
        );
        // 验证 side 在 buy/sell 间交替
        let sides: Vec<Side> = ds.iter_rows().map(|t| t.side).collect();
        assert_eq!(
            sides,
            vec![Side::Buy, Side::Sell, Side::Buy, Side::Sell, Side::Buy]
        );
    }

    #[tokio::test]
    async fn parquet_fixture_rejects_wrong_schema() {
        // 3 列(缺 side)— 应触发 SchemaMismatch
        let src = ParquetSource::new("bad_schema", fixture_path("sample_bad_schema.parquet"));
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        let err = src
            .query(&req)
            .await
            .expect_err("should fail on 3-column file");
        match err {
            DataError::SchemaMismatch { expected, actual } => {
                assert!(
                    expected.contains("≥4 columns"),
                    "expected contains '≥4 columns', got: {expected}"
                );
                assert!(
                    actual.contains("3 columns"),
                    "actual contains '3 columns', got: {actual}"
                );
            }
            other => panic!("expected SchemaMismatch, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn parquet_fixture_rejects_wrong_column_type() {
        // timestamp 列存成 utf8 — 应触发 column 0 type mismatch
        let src = ParquetSource::new("bad_type", fixture_path("sample_bad_type.parquet"));
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        let err = src
            .query(&req)
            .await
            .expect_err("should fail on wrong type");
        match err {
            DataError::SchemaMismatch { expected, actual } => {
                assert!(
                    expected.contains("column 0"),
                    "expected contains 'column 0', got: {expected}"
                );
                assert!(
                    actual.contains("column 0"),
                    "actual contains 'column 0', got: {actual}"
                );
            }
            other => panic!("expected SchemaMismatch, got {other:?}"),
        }
    }

    // --- PR5 真流式测试(列式 yield RecordBatch) ---

    #[tokio::test]
    async fn parquet_stream_full_yields_n_batches() {
        use axon_data::DataSource;
        use futures::StreamExt;
        let src = ParquetSource::new("basic", fixture_path("sample_basic.parquet"));
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        let stream = src.stream(&req).await.expect("stream basic parquet");
        let batches: Vec<arrow::record_batch::RecordBatch> =
            stream.map(|r| r.expect("ok batch")).collect().await;
        // 5 行 / batch_size=1024 → 单 batch
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].num_rows(), 5);
        // 验证首尾 timestamp
        let ts = batches[0]
            .column(0)
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap();
        assert_eq!(ts.value(0), 0);
        assert_eq!(ts.value(4), 4_000_000_000);
    }

    #[tokio::test]
    async fn parquet_stream_take_1_batch_breaks_early() {
        use axon_data::DataSource;
        use futures::StreamExt;
        let src = ParquetSource::new("basic", fixture_path("sample_basic.parquet"));
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        let stream = src.stream(&req).await.expect("stream basic parquet");
        // take(1) batch 后 stream drop → rx drop → 后台 task 退出
        let batches: Vec<arrow::record_batch::RecordBatch> =
            stream.take(1).map(|r| r.expect("ok batch")).collect().await;
        assert_eq!(batches.len(), 1);
        // 关键:这里不 panic + 测试在合理时间内通过(后台 task 已退出)
    }

    #[tokio::test]
    async fn parquet_stream_propagates_schema_error() {
        use axon_data::DataSource;
        use futures::StreamExt;
        let src = ParquetSource::new("bad_schema", fixture_path("sample_bad_schema.parquet"));
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        let stream = src.stream(&req).await.expect("stream bad_schema parquet");
        // 第一个 item 应该是 Err(SchemaMismatch),然后 stream 结束
        let first = stream
            .into_future()
            .await
            .0
            .expect("stream should yield at least one item");
        match first {
            Err(DataError::SchemaMismatch { .. }) => {}
            other => panic!("expected SchemaMismatch, got {other:?}"),
        }
    }

    // --- PR5 新增:列式 Dataset schema 校验 ---

    #[tokio::test]
    async fn parquet_query_returns_record_batch_dataset() {
        use arrow::datatypes::DataType;
        use axon_data::DataSource;
        let src = ParquetSource::new("basic", fixture_path("sample_basic.parquet"));
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        let ds = src.query(&req).await.expect("query basic parquet");
        // 验证 Dataset 内部结构:Vec<RecordBatch> + Arc<Schema>
        assert_eq!(ds.len(), 5);
        assert!(!ds.batches().is_empty());
        assert_eq!(ds.batches()[0].num_rows(), 5);
        // 验证 schema 4 列(int64, f64, f64, utf8)
        let schema = ds.schema();
        assert_eq!(schema.fields().len(), 4);
        assert_eq!(schema.field(0).data_type(), &DataType::Int64);
        assert_eq!(schema.field(1).data_type(), &DataType::Float64);
        assert_eq!(schema.field(2).data_type(), &DataType::Float64);
        assert_eq!(schema.field(3).data_type(), &DataType::Utf8);
    }

    #[tokio::test]
    async fn parquet_query_preserves_checksum_format() {
        // 关键测试:跨 PR checksum 字节级一致(PR1 行式格式)
        use axon_data::DataSource;
        let src = ParquetSource::new("basic", fixture_path("sample_basic.parquet"));
        let req = DataRequest::new("X", Utc::now(), Utc::now(), Frequency::Tick);
        let ds = src.query(&req).await.expect("query basic parquet");
        // checksum 格式:SHA256 hex 64 字符(行式格式不变)
        assert_eq!(ds.checksum.len(), 64);
        assert!(ds.checksum.chars().all(|c| c.is_ascii_hexdigit()));
        // 两次 query 同 fixture 应得同 checksum(纯函数性)
        let ds2 = src.query(&req).await.expect("query again");
        assert_eq!(ds.checksum, ds2.checksum);
    }
}

// ─── PR6: Bar Aggregator + IPC 集成测试 ────────────────────────

/// PR6 Bar 聚合 + IPC roundtrip 集成测试
#[test]
fn mock_to_bar_aggregate_and_ipc_roundtrip() {
    use axon_data::bar::{BarAggregator, BarDataset};
    use axon_data::ipc::{IpcReader, IpcWriter};
    use axon_data::DataSource;
    use tempfile::NamedTempFile;

    // 1. MockSource 生成 100 个 tick(每秒 1 个)
    let mock = MockSource::with_tick_series("BTCUSDT", 100, 1_000_000_000, |i| {
        100.0 + (i % 10) as f64
    });
    let rt = tokio::runtime::Runtime::new().unwrap();
    let req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Tick);
    let ds = rt.block_on(mock.query(&req)).unwrap();

    // 2. 聚合为 1m Bar
    let bars = BarAggregator::aggregate_ticks(ds.iter_rows(), Frequency::Min1).unwrap();
    assert!(!bars.is_empty());

    // 3. 构造 BarDataset
    let bar_req = DataRequest::new("BTCUSDT", Utc::now(), Utc::now(), Frequency::Min1);
    let bar_ds =
        BarDataset::from_bars(bars, "mock".into(), bar_req, Frequency::Min1).unwrap();

    // 4. IPC roundtrip
    let tmp = NamedTempFile::new().unwrap();
    IpcWriter::write_to_path(tmp.path(), &bar_ds).unwrap();
    let loaded = IpcReader::read_bar(tmp.path()).unwrap();

    assert_eq!(loaded.len(), bar_ds.len());
    assert_eq!(loaded.checksum, bar_ds.checksum);
    assert_eq!(loaded.frequency(), Frequency::Min1);
}

/// PR6 Bar 聚合 OHLCV 正确性测试
#[test]
fn bar_aggregate_ohlc_correctness() {
    use axon_core::market::{Side, Tick};
    use axon_core::time::Timestamp;
    use axon_core::types::{Price, Quantity};
    use axon_data::bar::BarAggregator;

    // 3 个 tick 在同一分钟内:价格 100, 110, 105
    let ticks = vec![
        Tick::new(
            Timestamp::from_nanos(0),
            Price::from_f64(100.0),
            Quantity::from_f64(10.0),
            Side::Buy,
        ),
        Tick::new(
            Timestamp::from_nanos(1_000_000_000),
            Price::from_f64(110.0),
            Quantity::from_f64(20.0),
            Side::Buy,
        ),
        Tick::new(
            Timestamp::from_nanos(2_000_000_000),
            Price::from_f64(105.0),
            Quantity::from_f64(15.0),
            Side::Sell,
        ),
    ];

    let bars = BarAggregator::aggregate_ticks(ticks.into_iter(), Frequency::Min1).unwrap();
    assert_eq!(bars.len(), 1);

    let bar = bars[0];
    assert_eq!(bar.open, Price::from_f64(100.0));
    assert_eq!(bar.high, Price::from_f64(110.0));
    assert_eq!(bar.low, Price::from_f64(100.0));
    assert_eq!(bar.close, Price::from_f64(105.0));
    assert!((bar.volume.as_f64() - 45.0).abs() < f64::EPSILON);
}

/// PR6 不支持频率错误测试
#[test]
fn unsupported_frequency_error() {
    use axon_core::market::{Side, Tick};
    use axon_core::time::Timestamp;
    use axon_core::types::{Price, Quantity};
    use axon_data::bar::BarAggregator;

    let ticks = vec![Tick::new(
        Timestamp::from_nanos(0),
        Price::from_f64(100.0),
        Quantity::from_f64(1.0),
        Side::Buy,
    )];
    let result = BarAggregator::aggregate_ticks(ticks.into_iter(), Frequency::Tick);
    assert!(result.is_err());
}
