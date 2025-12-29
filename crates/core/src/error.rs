use crate::primitive::{Price, Quantity};
use std::backtrace::Backtrace;
use thiserror::Error;

/// 统一的量化系统错误定义
/// 使用 `thiserror` 宏自动生成 Display 和 Error trait
#[derive(Error, Debug)]
pub enum QuantError {
    // =================================================================
    // 1. 系统与配置类 (System & Config)
    // =================================================================
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("System time error: {0}")]
    TimeError(String),

    #[error("Unknown internal error: {0}")]
    InternalError(String),

    // =================================================================
    // 2. 数据与解析类 (Data & Serialization)
    // =================================================================
    #[error("Failed to serialize/deserialize data: {0}")]
    SerializationError(String), // 包装 serde_json::Error

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid data format: {0}")]
    InvalidData(String),

    // =================================================================
    // 3. 交易业务类 (Trading & OMS) - 最重要
    // =================================================================
    #[error("Invalid order price: {0}")]
    InvalidPrice(Price),

    #[error("Invalid order quantity: {0}")]
    InvalidQuantity(Quantity),

    #[error("Insufficient funds/balance")]
    InsufficientFunds,

    #[error("Order not found: {0}")]
    OrderNotFound(String), // 传入 OrderID

    #[error("Order status is invalid for this operation: {0}")]
    InvalidOrderStatus(String),

    #[error("Unsupported symbol or exchange: {0}")]
    UnsupportedSymbol(String),

    // =================================================================
    // 4. 基础设施类 (Infrastructure)
    // 注意：Core 不直接依赖 sqlx/reqwest，用 String 包装错误信息
    // =================================================================
    #[error("Exchange network error: {0}")]
    ExchangeError(String), // 包装 HTTP/WebSocket 错误

    #[error("Database storage error: {0}")]
    StorageError(#[from] sqlx::Error), // 包装 SQLx/Redis 错误

    #[error("Redis storage error: {0}")]
    RedisError(#[from] redis::RedisError),

    #[error("Redis storage error: {0}")]
    RedisSetError(String),

    #[error("Data feed disconnected")]
    FeedDisconnected,
}

// ---------------------------------------------------------------------
// 辅助转换 (可选)
// 方便将标准库的 IO 错误转为我们的 InternalError
// ---------------------------------------------------------------------
impl From<std::io::Error> for QuantError {
    fn from(err: std::io::Error) -> Self {
        QuantError::InternalError(err.to_string())
    }
}
