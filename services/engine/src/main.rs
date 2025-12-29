use anyhow::Context;
use dotenvy::dotenv;
use std::env;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use quant_core::enums::Side;
// 引入内部模块
use quant_core::oms::Order;
use quant_core::primitive::{Price, Quantity};

// =========================================================================
// 1. 日志配置 (输出到控制台 + 文件)
// =========================================================================
fn init_logging() -> tracing_appender::non_blocking::WorkerGuard {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    // 1. 文件输出器：每天生成一个新的日志文件 (logs/photon.2025-xx-xx.log)
    let file_appender = tracing_appender::rolling::daily("logs", "photon.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // 2. 控制台层 (Console Layer)
    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_target(false) // 不显示模块路径，保持清爽
        .with_thread_ids(true)
        .compact(); // 紧凑模式

    // 3. 文件层 (File Layer)
    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false) // 文件里不要颜色代码
        .with_file(true)
        .with_line_number(true);

    // 4. 注册全局订阅者
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with(stdout_layer)
        .with(file_layer)
        .init();

    guard // 必须返回 guard，否则日志线程会立即销毁
}

// =========================================================================
// 2. 模拟一个简单的事件循环 (这是未来的核心)
// =========================================================================
async fn run_event_loop() {
    info!("🚀 Event Loop Started...");

    let (tx, _rx) = broadcast::channel::<String>(1000);

    // 1. 行情任务
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            info!("📡 [Mock] Feed received a heartbeat...");
        }
    });

    // 2. 策略任务
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        info!("💡 Strategy triggered! Placing a test order...");
        // ... 之前的逻辑
    });

    // ✅ 新增：让这个函数永远等待，不要退出！
    // std::future::pending() 会创建一个永远不会完成的 Future
    std::future::pending::<()>().await;
}

// =========================================================================
// 3. 主入口 (Main Entry)
// =========================================================================
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // A. 加载配置与日志
    dotenv().ok(); // 读取 .env 文件
    let _log_guard = init_logging(); // 初始化日志，_guard 不能丢

    info!("Starting Photon Quant Engine ⚡️");

    info!("📦 Storage module initialized.");

    // D. 启动主逻辑
    // 使用 tokio::select! 监听系统信号，实现优雅停机
    tokio::select! {
        _ = run_event_loop() => {
            error!("Event loop exited unexpectedly!");
        }
        _ = signal::ctrl_c() => {
            warn!("🛑 Ctrl+C received! Shutting down gracefully...");
        }
    }

    // E. 清理工作 (如果有)
    info!("👋 Photon Engine Shutdown Complete.");
    Ok(())
}