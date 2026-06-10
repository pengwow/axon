//! AXON 命令行工具
//!
//! 提供 `axon` 可执行入口。
//! Phase 0 阶段仅打印版本与编译时信息，后续阶段接入子命令（回测、训练、运行等）。

use axon_core::Result;

/// 程序入口
fn main() -> Result<()> {
    print_banner();
    Ok(())
}

/// 打印欢迎信息与编译时常量
fn print_banner() {
    println!("axon {}", env!("CARGO_PKG_VERSION"));
    println!("Rust {} ({})", rustc_version(), target_triple());
    println!("阶段：Phase 0 — 架构与基础设施");
    println!();
    println!("可用子命令尚未实现，敬请期待 Phase 1A。");
}

/// 获取 rustc 版本字符串
fn rustc_version() -> &'static str {
    // 编译期常量：Phase 0 阶段无需引入 build.rs
    // 后续阶段可使用 `rustc_version_runtime` crate
    match option_env!("RUSTC_VERSION") {
        Some(v) => v,
        None => "stable",
    }
}

/// 获取目标三元组
///
/// 使用 `std::env::consts` 提供的编译期常量，
/// 避免依赖 cargo 注入的 `TARGET` 环境变量
/// （`TARGET` 在 `build.rs` 运行前不可用，在 `main.rs` 中使用会编译失败）
fn target_triple() -> String {
    format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rustc_version_returns_non_empty() {
        assert!(!rustc_version().is_empty());
    }

    #[test]
    fn test_target_triple_returns_non_empty() {
        let triple = target_triple();
        assert!(!triple.is_empty());
        assert!(triple.contains('-'));
    }
}
