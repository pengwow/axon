//! axon-data Criterion 基准测试
//!
//! 运行:`cargo bench -p axon-data --features csv-source`
//! 4 个 group: lru_cache / dataset_lazy / csv_parse / mock_generate
//!
//! 关键约束(从项目 lessons learned 提取):
//! - 用 black_box() 包装动态值,避免常量折叠
//! - 控制 batch ≤ 100,避免 OOM
//! - 每个 group 独立 criterion_group

use std::io::Write;
use std::num::NonZeroUsize;

use axon_data::sources::{CsvSource, MockSource};
use axon_data::types::{DataRequest, Frequency};
use axon_data::{DataSource, Dataset, DataService};
use chrono::Utc;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use tempfile::NamedTempFile;

/// 准备 N 个不同 symbol 的预热请求
fn warmup_requests(n: usize) -> Vec<DataRequest> {
    (0..n)
        .map(|i| {
            DataRequest::new(
                format!("SYM{i}"),
                Utc::now(),
                Utc::now(),
                Frequency::Tick,
            )
        })
        .collect()
}

/// group 1: DataService LRU 缓存 — 不同容量下的查询延迟
fn bench_lru_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("lru_cache");
    let rt = tokio::runtime::Runtime::new().unwrap();
    for &cap in &[16usize, 64, 256] {
        let svc = DataService::new()
            .with_cache_capacity(NonZeroUsize::new(cap).unwrap())
            .register_source(Box::new(MockSource::with_tick_series(
                "m", 1, 1, |_| 1.0,
            )));
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
        let ds = Dataset::new(ticks, vec![], "bench".into(), req);

        group.bench_with_input(BenchmarkId::new("filter", n_rows), &n_rows, |b, _| {
            b.iter(|| {
                let r = black_box(&ds).filter(|t| t.price.as_f64() > 150.0);
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
                    let r = black_box(&ds).by_time_range(start, end);
                    black_box(r.len());
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_lru_cache,
    bench_dataset_lazy,
    bench_csv_parse,
    bench_mock_generate
);
criterion_main!(benches);

/// group 4: MockSource::with_tick_series 生成耗时
fn bench_mock_generate(c: &mut Criterion) {
    let mut group = c.benchmark_group("mock_generate");
    let req = DataRequest::new("m", Utc::now(), Utc::now(), Frequency::Tick);
    let rt = tokio::runtime::Runtime::new().unwrap();
    for &n in &[1_000usize, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let m = MockSource::with_tick_series(
                    "m",
                    black_box(n),
                    1_000_000,
                    |i| 100.0 + i as f64,
                );
                // 通过 query() API 拿到行数(bench binary 看不到 pub(crate) 字段)
                let ds = rt.block_on(m.query(&req)).unwrap();
                black_box(ds.len());
            });
        });
    }
    group.finish();
}

/// 写一个 N 行的 CSV(纳秒时间戳,f64 价,1.0 量,buy)
fn make_temp_csv(n_rows: usize) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "timestamp,price,quantity,side").unwrap();
    for i in 0..n_rows {
        writeln!(f, "{},{},1.0,buy", i, 100.0 + (i % 100) as f64).unwrap();
    }
    f.flush().unwrap();
    f
}

/// group 3: CsvSource 解析吞吐
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
