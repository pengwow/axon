//! axon-data Criterion 基准测试
//!
//! 运行:`cargo bench -p axon-data --features csv-source --features parquet-source`
//! 8 个 group: lru_cache / dataset_lazy / csv_parse / mock_generate / parquet_load / parquet_stream / bar_aggregate / mmap_cache
//!
//! 关键约束(从项目 lessons learned 提取):
//! - 用 black_box() 包装动态值,避免常量折叠
//! - 控制 batch ≤ 100,避免 OOM
//! - 每个 group 独立 criterion_group

use std::num::NonZeroUsize;

#[cfg(feature = "csv-source")]
use std::io::Write;

#[cfg(feature = "parquet-source")]
use arrow::array::{Float64Array, Int64Array, StringArray};
#[cfg(feature = "parquet-source")]
use arrow::datatypes::{DataType, Field, Schema};
#[cfg(feature = "parquet-source")]
use arrow::record_batch::RecordBatch;
#[cfg(feature = "parquet-source")]
use parquet::arrow::ArrowWriter;
#[cfg(feature = "parquet-source")]
use std::fs::File;
#[cfg(feature = "parquet-source")]
use std::sync::Arc;

#[cfg(feature = "csv-source")]
use axon_data::sources::CsvSource;
use axon_data::sources::MockSource;
use axon_data::types::{DataRequest, Frequency};
use axon_data::{DataService, DataSource, Dataset};
use chrono::Utc;
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
// tempfile::NamedTempFile 仅在 csv-source / parquet-source 下的 bench 中使用
#[cfg(any(feature = "csv-source", feature = "parquet-source"))]
use tempfile::NamedTempFile;

/// 准备 N 个不同 symbol 的预热请求
fn warmup_requests(n: usize) -> Vec<DataRequest> {
    (0..n)
        .map(|i| DataRequest::new(format!("SYM{i}"), Utc::now(), Utc::now(), Frequency::Tick))
        .collect()
}

/// group 1: DataService LRU 缓存 — 不同容量下的查询延迟
fn bench_lru_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("lru_cache");
    let rt = tokio::runtime::Runtime::new().unwrap();
    for &cap in &[16usize, 64, 256] {
        let svc = DataService::new()
            .with_cache_capacity(NonZeroUsize::new(cap).unwrap())
            .register_source(Box::new(MockSource::with_tick_series("m", 1, 1, |_| 1.0)));
        let reqs = warmup_requests(cap * 2); // 触发淘汰
        // 预热(初始化缓存)
        rt.block_on(async {
            for req in &reqs {
                let _ = svc.load(req).await.unwrap();
            }
        });
        group.bench_with_input(BenchmarkId::from_parameter(cap), &cap, |b, _| {
            b.iter(|| {
                let req = &reqs[0]; // 命中
                let ds = rt.block_on(svc.load(black_box(req))).unwrap();
                black_box(ds.len());
            });
        });
    }
    group.finish();
}

/// group 2: Dataset lazy 方法(filter/take/skip/by_time_range)在不同规模下耗时
fn bench_dataset_lazy(c: &mut Criterion) {
    use arrow::array::{Array, Float64Array};
    use arrow::record_batch::RecordBatch;
    use axon_core::market::{Side, Tick};
    use axon_core::time::Timestamp;
    use axon_core::types::{Price, Quantity};

    let mut group = c.benchmark_group("dataset_lazy");
    for &n_rows in &[1_000usize, 10_000, 100_000] {
        // 准备数据
        let ticks: Vec<Tick> = (0..n_rows)
            .map(|i| {
                Tick::new(
                    Timestamp::from_nanos(i as i64 * 1_000_000),
                    Price::from_f64(100.0 + (i % 100) as f64),
                    Quantity::from_f64(1.0),
                    Side::Buy,
                )
            })
            .collect();
        let req = DataRequest::new("BENCH", Utc::now(), Utc::now(), Frequency::Tick);
        // PR5:走 from_ticks 桥接入口
        let ds = Dataset::from_ticks(ticks, "bench".into(), req).expect("from_ticks");

        group.bench_with_input(BenchmarkId::new("filter", n_rows), &n_rows, |b, _| {
            b.iter(|| {
                // PR5:列式 filter(走 `arrow::compute::kernels::cmp::gt` + scalar)
                let r = black_box(&ds)
                    .filter(|batch: &RecordBatch| -> std::sync::Arc<dyn Array> {
                        let px = batch
                            .column(1)
                            .as_any()
                            .downcast_ref::<Float64Array>()
                            .expect("col 1 Float64Array");
                        let mask = arrow::compute::kernels::cmp::gt(
                            px,
                            &Float64Array::new_scalar(150.0_f64),
                        )
                        .expect("gt");
                        std::sync::Arc::new(mask) as std::sync::Arc<dyn Array>
                    })
                    .expect("filter");
                black_box(r.len());
            });
        });
        group.bench_with_input(BenchmarkId::new("take", n_rows), &n_rows, |b, _| {
            b.iter(|| {
                let r = black_box(&ds).take(100);
                black_box(r.len());
            });
        });
        group.bench_with_input(BenchmarkId::new("skip", n_rows), &n_rows, |b, _| {
            b.iter(|| {
                let r = black_box(&ds).skip(100);
                black_box(r.len());
            });
        });
        group.bench_with_input(
            BenchmarkId::new("by_time_range", n_rows),
            &n_rows,
            |b, _| {
                b.iter(|| {
                    let start = Timestamp::from_nanos(10_000_000_000);
                    let end = Timestamp::from_nanos(20_000_000_000);
                    let r = black_box(&ds)
                        .by_time_range(start, end)
                        .expect("by_time_range");
                    black_box(r.len());
                });
            },
        );
    }
    group.finish();
}

// ─── criterion_group / criterion_main 组合 ─────────────────────
//
// 根据 feature 组合选择不同的 benchmark group 集合。
// mmap-cache 作为独立 group，避免与其他 feature 产生组合爆炸。

#[cfg(not(feature = "parquet-source"))]
#[cfg(not(feature = "csv-source"))]
criterion_group!(
    benches,
    bench_lru_cache,
    bench_dataset_lazy,
    bench_mock_generate,
    bench_bar_aggregate
);

#[cfg(feature = "csv-source")]
#[cfg(not(feature = "parquet-source"))]
criterion_group!(
    benches,
    bench_lru_cache,
    bench_dataset_lazy,
    bench_csv_parse,
    bench_mock_generate,
    bench_bar_aggregate
);

#[cfg(feature = "parquet-source")]
#[cfg(not(feature = "csv-source"))]
criterion_group!(
    benches,
    bench_lru_cache,
    bench_dataset_lazy,
    bench_mock_generate,
    bench_parquet_load,
    bench_parquet_stream,
    bench_bar_aggregate
);

#[cfg(feature = "csv-source")]
#[cfg(feature = "parquet-source")]
criterion_group!(
    benches,
    bench_lru_cache,
    bench_dataset_lazy,
    bench_csv_parse,
    bench_mock_generate,
    bench_parquet_load,
    bench_parquet_stream,
    bench_bar_aggregate
);

// mmap-cache 独立 group（feature-gated）
#[cfg(feature = "mmap-cache")]
criterion_group!(mmap_cache_benches, bench_mmap_cache);

// criterion_main 入口：根据 feature 组合选择要运行的 group
#[cfg(not(feature = "mmap-cache"))]
criterion_main!(benches);

#[cfg(feature = "mmap-cache")]
criterion_main!(benches, mmap_cache_benches);

/// group 4: MockSource::with_tick_series 生成耗时
fn bench_mock_generate(c: &mut Criterion) {
    let mut group = c.benchmark_group("mock_generate");
    let req = DataRequest::new("m", Utc::now(), Utc::now(), Frequency::Tick);
    let rt = tokio::runtime::Runtime::new().unwrap();
    for &n in &[1_000usize, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let m = MockSource::with_tick_series("m", black_box(n), 1_000_000, |i| {
                    100.0 + i as f64
                });
                // 通过 query() API 拿到行数(bench binary 看不到 pub(crate) 字段)
                let ds = rt.block_on(m.query(&req)).unwrap();
                black_box(ds.len());
            });
        });
    }
    group.finish();
}

/// 写一个 N 行的 CSV(纳秒时间戳,f64 价,1.0 量,buy)
#[cfg(feature = "csv-source")]
fn make_temp_csv(n_rows: usize) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "timestamp,price,quantity,side").unwrap();
    for i in 0..n_rows {
        writeln!(f, "{},{},1.0,buy", i, 100.0 + (i % 100) as f64).unwrap();
    }
    f.flush().unwrap();
    f
}

/// group 3: CsvSource 解析吞吐(需 csv-source feature)
#[cfg(feature = "csv-source")]
fn bench_csv_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("csv_parse");
    let rt = tokio::runtime::Runtime::new().unwrap();
    for &n_rows in &[1_000usize, 10_000, 100_000] {
        let tmp = make_temp_csv(n_rows);
        let path = tmp.path().to_str().unwrap().to_string();
        let req = DataRequest::new("BENCH", Utc::now(), Utc::now(), Frequency::Tick);
        group.bench_with_input(BenchmarkId::from_parameter(n_rows), &n_rows, |b, _| {
            b.iter(|| {
                let src = CsvSource::new("bench", black_box(&path));
                let ds = rt.block_on(src.query(black_box(&req))).unwrap();
                black_box(ds.len());
            });
        });
    }
    group.finish();
}

/// group 5: ParquetSource::query 加载吞吐(需 parquet-source feature)
///
/// 动态生成临时 parquet 文件,避免常量折叠和文件 IO 缓存;
/// 用 `std::mem::forget` 保留 NamedTempFile(NamedTempFile drop 时会删除文件)
#[cfg(feature = "parquet-source")]
fn bench_parquet_load(c: &mut Criterion) {
    use axon_data::sources::ParquetSource;

    let mut group = c.benchmark_group("parquet_load");
    let rt = tokio::runtime::Runtime::new().unwrap();
    for &n_rows in &[1_000usize, 10_000, 100_000] {
        // 用 Arrow ArrowWriter 动态生成临时 parquet 文件
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        {
            let schema = Arc::new(Schema::new(vec![
                Field::new("timestamp", DataType::Int64, false),
                Field::new("price", DataType::Float64, false),
                Field::new("quantity", DataType::Float64, false),
                Field::new("side", DataType::Utf8, false),
            ]));
            let ts: Int64Array = (0..n_rows as i64).collect();
            let prices: Float64Array = (0..n_rows).map(|i| 100.0 + (i % 100) as f64).collect();
            let qtys: Float64Array = (0..n_rows).map(|_| 1.0_f64).collect();
            let sides_vec: Vec<String> = (0..n_rows)
                .map(|i| {
                    if i % 2 == 0 {
                        "buy".to_string()
                    } else {
                        "sell".to_string()
                    }
                })
                .collect();
            let sides = StringArray::from(sides_vec);
            let batch = RecordBatch::try_new(
                schema.clone(),
                vec![
                    Arc::new(ts),
                    Arc::new(prices),
                    Arc::new(qtys),
                    Arc::new(sides),
                ],
            )
            .unwrap();
            let file = File::create(&path).unwrap();
            let mut writer = ArrowWriter::try_new(file, schema, None).unwrap();
            writer.write(&batch).unwrap();
            writer.close().unwrap();
        }
        // 阻止 NamedTempFile 删除临时文件(必须 leak 到 bench 结束)
        std::mem::forget(tmp);
        let path_str = path.to_string_lossy().into_owned();
        let req = DataRequest::new("BENCH", Utc::now(), Utc::now(), Frequency::Tick);
        group.bench_with_input(BenchmarkId::from_parameter(n_rows), &n_rows, |b, _| {
            b.iter(|| {
                let src = ParquetSource::new("bench", black_box(&path_str));
                let ds = rt.block_on(src.query(black_box(&req))).unwrap();
                black_box(ds.len());
            });
        });
    }
    group.finish();
}

/// group 6: ParquetSource::stream vs query(真流式 vs 伪流式,PR4)
///
/// 对比 `query()` 全量 + `take(10)`(伪流式)vs `stream()` + `take(10)`(真流式)
/// 预期:大文件下 `stream_take_10` 显著快于 `query_take_10`,因为不用加载全文件
#[cfg(feature = "parquet-source")]
fn bench_parquet_stream(c: &mut Criterion) {
    use axon_data::sources::ParquetSource;
    // 使用全限定 `futures::StreamExt::for_each` 调用,避免未使用的 import 警告

    let mut group = c.benchmark_group("parquet_stream");
    let rt = tokio::runtime::Runtime::new().unwrap();
    for &n_rows in &[10_000usize, 100_000] {
        // 复用 parquet_load 同样的 Arrow 写盘逻辑
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        {
            let schema = Arc::new(Schema::new(vec![
                Field::new("timestamp", DataType::Int64, false),
                Field::new("price", DataType::Float64, false),
                Field::new("quantity", DataType::Float64, false),
                Field::new("side", DataType::Utf8, false),
            ]));
            let ts: Int64Array = (0..n_rows as i64).collect();
            let prices: Float64Array = (0..n_rows).map(|i| 100.0 + (i % 100) as f64).collect();
            let qtys: Float64Array = (0..n_rows).map(|_| 1.0_f64).collect();
            let sides_vec: Vec<String> = (0..n_rows)
                .map(|i| {
                    if i % 2 == 0 {
                        "buy".to_string()
                    } else {
                        "sell".to_string()
                    }
                })
                .collect();
            let sides = StringArray::from(sides_vec);
            let batch = RecordBatch::try_new(
                schema.clone(),
                vec![
                    Arc::new(ts),
                    Arc::new(prices),
                    Arc::new(qtys),
                    Arc::new(sides),
                ],
            )
            .unwrap();
            let file = File::create(&path).unwrap();
            let mut writer = ArrowWriter::try_new(file, schema, None).unwrap();
            writer.write(&batch).unwrap();
            writer.close().unwrap();
        }
        // 阻止 NamedTempFile 删除临时文件(必须 leak 到 bench 结束)
        std::mem::forget(tmp);
        let path_str = path.to_string_lossy().into_owned();
        let req = DataRequest::new("BENCH", Utc::now(), Utc::now(), Frequency::Tick);

        // bench A:query() 全量 + take(10) — 加载全部 100k,只取 10 行(伪流式)
        group.bench_with_input(
            BenchmarkId::new("query_take_10", n_rows),
            &n_rows,
            |b, _| {
                b.iter(|| {
                    let src = ParquetSource::new("bench", black_box(&path_str));
                    let ds = rt.block_on(src.query(black_box(&req))).unwrap();
                    black_box(ds.take(10).len());
                });
            },
        );

        // bench B:stream() + take(10) — 边读边取 10 行(真流式,应显著更快)
        // PR5:stream yield RecordBatch,for_each 累加 num_rows
        group.bench_with_input(
            BenchmarkId::new("stream_take_10", n_rows),
            &n_rows,
            |b, _| {
                b.iter(|| {
                    let src = ParquetSource::new("bench", black_box(&path_str));
                    let stream = rt.block_on(src.stream(black_box(&req))).unwrap();
                    let n: usize = rt.block_on(async {
                        let mut total_rows = 0usize;
                        futures::StreamExt::for_each(stream, |batch_res| {
                            total_rows += batch_res.map(|b| b.num_rows()).unwrap_or(0);
                            async {}
                        })
                        .await;
                        total_rows
                    });
                    black_box(n);
                });
            },
        );
    }
    group.finish();
}

/// group 7: BarAggregator 聚合吞吐(PR6)
fn bench_bar_aggregate(c: &mut Criterion) {
    use axon_data::bar::BarAggregator;

    let mut group = c.benchmark_group("bar_aggregate");
    for &n_ticks in &[1_000usize, 10_000, 100_000] {
        // 生成 n_ticks 个 tick(每秒 1 个)
        let ticks: Vec<axon_core::market::Tick> = (0..n_ticks)
            .map(|i| {
                axon_core::market::Tick::new(
                    axon_core::time::Timestamp::from_nanos(i as i64 * 1_000_000_000),
                    axon_core::types::Price::from_f64(100.0 + (i % 100) as f64),
                    axon_core::types::Quantity::from_f64(1.0),
                    axon_core::market::Side::Buy,
                )
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("aggregate_1m", n_ticks),
            &n_ticks,
            |b, _| {
                b.iter(|| {
                    let bars = BarAggregator::aggregate_ticks(
                        black_box(&ticks).clone().into_iter(),
                        Frequency::Min1,
                    )
                    .unwrap();
                    black_box(bars.len());
                });
            },
        );
    }
    group.finish();
}

// ─── group 8: MmapCache 读写吞吐(PR7) ──────────────────────────

/// group 8: MmapCache 缓存读写延迟(PR7)
///
/// 测试 L2 mmap 缓存的 put/get 性能，验证零拷贝读取效果。
#[cfg(feature = "mmap-cache")]
fn bench_mmap_cache(c: &mut Criterion) {
    use axon_core::market::Side;
    use axon_core::time::Timestamp;
    use axon_core::types::{Price, Quantity};
    use axon_data::cache::{MmapCache, MmapCacheConfig};

    let mut group = c.benchmark_group("mmap_cache");

    for &n_rows in &[1_000usize, 10_000, 100_000] {
        // 准备测试数据集
        let ticks: Vec<axon_core::market::Tick> = (0..n_rows)
            .map(|i| {
                axon_core::market::Tick::new(
                    Timestamp::from_nanos(i as i64 * 1_000_000),
                    Price::from_f64(100.0 + (i % 100) as f64),
                    Quantity::from_f64(1.0),
                    Side::Buy,
                )
            })
            .collect();
        let req = DataRequest::new("BENCH", Utc::now(), Utc::now(), Frequency::Tick);
        let dataset = Dataset::from_ticks(ticks, "bench".into(), req).expect("from_ticks");
        let key = MmapCache::cache_key("bench", "BENCH", "Tick");

        // bench: put（写入缓存）
        group.bench_with_input(BenchmarkId::new("put", n_rows), &n_rows, |b, _| {
            b.iter_with_setup(
                || {
                    // 每次迭代创建新缓存，避免容量问题
                    let dir = tempfile::tempdir().unwrap();
                    let config =
                        MmapCacheConfig::new(1024 * 1024 * 100, dir.path().to_str().unwrap());
                    let cache = MmapCache::new(config).unwrap();
                    (dir, cache)
                },
                |(_dir, mut cache)| {
                    black_box(cache.put(&key, &dataset).unwrap());
                },
            );
        });

        // bench: get（读取缓存）— 验证零拷贝性能
        group.bench_with_input(BenchmarkId::new("get", n_rows), &n_rows, |b, _| {
            // 预填充缓存
            let dir = tempfile::tempdir().unwrap();
            let config = MmapCacheConfig::new(1024 * 1024 * 100, dir.path().to_str().unwrap());
            let mut cache = MmapCache::new(config).unwrap();
            cache.put(&key, &dataset).unwrap();

            b.iter(|| {
                let cached = black_box(&mut cache).get(&key);
                black_box(cached.as_ref().map(|d| d.len()));
            });
        });

        // bench: get_zero_copy（零拷贝读取）— 性能目标 <10µs
        group.bench_with_input(
            BenchmarkId::new("get_zero_copy", n_rows),
            &n_rows,
            |b, _| {
                // 预填充缓存
                let dir = tempfile::tempdir().unwrap();
                let config = MmapCacheConfig::new(1024 * 1024 * 100, dir.path().to_str().unwrap());
                let mut cache = MmapCache::new(config).unwrap();
                cache.put(&key, &dataset).unwrap();

                b.iter(|| {
                    let cached = black_box(&cache).get_zero_copy(&key);
                    black_box(cached.as_ref().map(|d| d.len()));
                });
            },
        );
    }
    group.finish();
}
