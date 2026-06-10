# 多阶段构建：开发与生产分离
# ─────────────────────────────────────────────
# 阶段 1：构建（带完整工具链）
# 阶段 2：CI（仅运行 cargo test/clippy/fmt）
# 阶段 3：runtime（最小化镜像，仅含 release 二进制）

# ===== 阶段 1：builder =====
FROM rust:1.96-bookworm AS builder

# 安装 sccache 加速增量编译
RUN cargo install sccache --locked \
    && echo '[net]' > /usr/local/cargo/config.toml \
    && echo 'git-fetch-with-cli = true' >> /usr/local/cargo/config.toml

# 启用 sccache（通过 RUSTC_WRAPPER）
ENV RUSTC_WRAPPER=sccache
ENV SCCACHE_DIR=/sccache
ENV CARGO_HOME=/usr/local/cargo
ENV CARGO_TARGET_DIR=/tmp/axon-target
RUN mkdir -p /sccache && chmod 777 /sccache

WORKDIR /build

# 复制依赖清单，单独 layer 以利用 Docker 缓存
COPY Cargo.toml Cargo.lock ./
COPY crates/axon-core/Cargo.toml crates/axon-core/
COPY crates/axon-backtest/Cargo.toml crates/axon-backtest/
COPY crates/axon-cli/Cargo.toml crates/axon-cli/

# 占位源码：仅用于触发依赖编译与缓存
RUN mkdir -p crates/axon-core/src crates/axon-backtest/src crates/axon-cli/src \
    && echo "fn main() {}" > crates/axon-cli/src/main.rs \
    && echo "" > crates/axon-core/src/lib.rs \
    && echo "" > crates/axon-backtest/src/lib.rs \
    && cargo build --release --workspace \
    && rm -rf crates/axon-core/src crates/axon-backtest/src crates/axon-cli/src

# 复制真实源码
COPY crates/ ./crates/
COPY .git/ ./.git/ 2>/dev/null || true

# Release 编译
RUN cargo build --release --workspace \
    && strip target/release/axon

# ===== 阶段 2：runtime =====
FROM debian:bookworm-slim AS runtime

# 安装运行时依赖（ca 证书、tzdata 用于时区）
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        tzdata \
    && rm -rf /var/lib/apt/lists/*

# 创建非 root 用户
RUN groupadd --system --gid 1000 axon \
    && useradd --system --uid 1000 --gid axon --create-home --shell /bin/bash axon

# 从 builder 复制二进制
COPY --from=builder /build/target/release/axon /usr/local/bin/axon

# 切换到非 root 用户
USER axon
WORKDIR /home/axon

ENTRYPOINT ["/usr/local/bin/axon"]
CMD ["--help"]

# 元数据
LABEL org.opencontainers.image.title="axon" \
      org.opencontainers.image.description="AXON event-driven trading engine" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.source="https://github.com/axon-team/axon"
