#!/usr/bin/env python3
"""
生成 axon-data parquet 测试 fixture(3 个):
- sample_basic.parquet:5 行正常数据(int64/f64/f64/utf8)
- sample_bad_schema.parquet:3 列(缺 side)
- sample_bad_type.parquet:timestamp 列存成 utf8

使用方式:python3 generate_sample.py
要求:pyarrow >= 10
"""
import pyarrow as pa
import pyarrow.parquet as pq
from pathlib import Path

HERE = Path(__file__).parent


def write_basic() -> None:
    """5 行正常 fixture:ts 0..4e9 纳秒,price 100..104,side 交替 buy/sell"""
    table = pa.table({
        "timestamp": pa.array(
            [0, 1_000_000_000, 2_000_000_000, 3_000_000_000, 4_000_000_000],
            type=pa.int64(),
        ),
        "price": pa.array([100.0, 101.0, 102.0, 103.0, 104.0], type=pa.float64()),
        "quantity": pa.array([1.0, 1.0, 1.0, 1.0, 1.0], type=pa.float64()),
        "side": pa.array(["buy", "sell", "buy", "sell", "buy"]),
    })
    pq.write_table(table, HERE / "sample_basic.parquet")
    print(f"wrote sample_basic.parquet: {table.num_rows} rows")


def write_bad_schema() -> None:
    """3 列(缺 side)— 应触发 ≥4 columns SchemaMismatch"""
    table = pa.table({
        "timestamp": pa.array([0, 1_000_000_000], type=pa.int64()),
        "price": pa.array([100.0, 101.0], type=pa.float64()),
        "quantity": pa.array([1.0, 2.0], type=pa.float64()),
    })
    pq.write_table(table, HERE / "sample_bad_schema.parquet")
    print(f"wrote sample_bad_schema.parquet: {table.num_rows} rows")


def write_bad_type() -> None:
    """timestamp 列存成 utf8 — 应触发 column 0 type mismatch"""
    table = pa.table({
        "timestamp": pa.array(["a", "b", "c"]),  # 故意用 string 而非 int64
        "price": pa.array([100.0, 101.0, 102.0], type=pa.float64()),
        "quantity": pa.array([1.0, 2.0, 3.0], type=pa.float64()),
        "side": pa.array(["buy", "sell", "buy"]),
    })
    pq.write_table(table, HERE / "sample_bad_type.parquet")
    print(f"wrote sample_bad_type.parquet: {table.num_rows} rows")


if __name__ == "__main__":
    write_basic()
    write_bad_schema()
    write_bad_type()
    print("done.")
